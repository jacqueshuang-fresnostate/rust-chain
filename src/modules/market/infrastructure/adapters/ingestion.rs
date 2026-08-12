//! 权威行情 ingestion 基础设施。
//!
//! 仅接收已由 provider 适配器归一化且通过领域校验的快照，按原顺序写 Redis 与 Mongo；
//! ticker/depth 缓存成功后才尝试触发现货订单，撮合失败不回滚已落地行情。

use super::provider::provider_name;
use crate::{
    error::{AppError, AppResult},
    infra::mongo::{ensure_kline_indexes, kline_collection_name},
    modules::{
        events::EventBroadcastHub,
        market::{
            KlineUpsertKey, MarketDepthSnapshot, MarketKlineSnapshot, MarketTickerSnapshot,
            ValidatedMarketSymbol,
            infrastructure::{
                MarketCacheError, MarketDepthCacheEntry, MarketKlineCacheEntry,
                MarketTickerCacheEntry, RedisMarketCache,
            },
        },
        spot::application::execute_triggered_spot_limit_orders_with_hub as execute_triggered_spot_limit_orders,
    },
    state::AppState,
};
use axum::async_trait;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use mongodb::bson::{DateTime as BsonDateTime, Document, doc};
use sqlx::{MySql, Pool};

#[async_trait]
pub trait MarketIngestionSink: Clone + Send + Sync + 'static {
    /// 持久化标准 ticker 快照；实现成功返回后，该价格才可供下单、结算和强平消费者读取。
    async fn ingest_ticker(&self, snapshot: &MarketTickerSnapshot) -> AppResult<()>;
    /// 持久化标准深度快照；不得绕过交易对和数值校验。
    async fn ingest_depth(&self, snapshot: &MarketDepthSnapshot) -> AppResult<()>;
    /// 持久化标准 K 线快照；实现必须保持交易对+周期+开盘时间的幂等写入。
    async fn ingest_kline(&self, snapshot: &MarketKlineSnapshot) -> AppResult<()>;
}

#[derive(Clone)]
pub struct MarketIngestionService {
    cache: RedisMarketCache,
    database: mongodb::Database,
    mysql: Option<Pool<MySql>>,
    broadcast_hub: Option<EventBroadcastHub>,
}

impl MarketIngestionService {
    pub fn new(cache: RedisMarketCache, database: mongodb::Database) -> Self {
        Self {
            cache,
            database,
            mysql: None,
            broadcast_hub: None,
        }
    }

    pub fn from_state(state: &AppState) -> AppResult<Self> {
        let redis = state.redis.clone().ok_or_else(|| {
            AppError::Internal("redis connection is not configured for market ingestion".to_owned())
        })?;
        let mongo = state.mongo.clone().ok_or_else(|| {
            AppError::Internal("mongo database is not configured for market ingestion".to_owned())
        })?;
        Ok(Self::new(RedisMarketCache::new(redis), mongo)
            .with_mysql(state.mysql.clone())
            .with_broadcast_hub(state.event_broadcast_hub.clone()))
    }

    pub fn with_mysql(mut self, mysql: Option<Pool<MySql>>) -> Self {
        self.mysql = mysql;
        self
    }

    pub fn with_broadcast_hub(mut self, broadcast_hub: Option<EventBroadcastHub>) -> Self {
        self.broadcast_hub = broadcast_hub;
        self
    }

    /// 将供应商 ticker 快照写入 Redis 权威缓存，成功后再尝试触发现货限价撮合。
    /// 快照必须已由 provider adapter 校验交易对、价格与时间；缓存失败时不得触发订单，撮合失败只告警且不撤销已写行情。
    pub async fn ingest_ticker(&self, snapshot: &MarketTickerSnapshot) -> AppResult<()> {
        let entry = MarketTickerCacheEntry::from_snapshot(snapshot)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        self.cache
            .save_ticker(entry)
            .await
            .map_err(market_cache_error)?;
        self.trigger_spot_limit_orders(snapshot.symbol(), snapshot.last_price(), "ticker")
            .await;
        Ok(())
    }

    /// 写入深度快照，并在存在卖一价时把它作为现货触发价候选；深度解析或缓存失败时不触发订单。
    /// 撮合是缓存成功后的独立副作用，失败只告警，不回滚已持久化的市场深度。
    pub async fn ingest_depth(&self, snapshot: &MarketDepthSnapshot) -> AppResult<()> {
        let entry = MarketDepthCacheEntry::from_snapshot(snapshot)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        self.cache
            .save_depth(entry)
            .await
            .map_err(market_cache_error)?;
        if let Some(best_ask) = snapshot.asks().iter().map(|level| &level.price).min() {
            self.trigger_spot_limit_orders(snapshot.symbol(), best_ask, "depth")
                .await;
        }
        Ok(())
    }

    /// 将 K 线同时写入 Redis 最新快照与 Mongo 历史集合；交易对、周期和开盘时间必须先通过领域校验。
    /// Mongo 以 interval+open_time 唯一索引 upsert，重放只覆盖同一根蜡烛；任一持久化错误原样上抛，不广播成功事件。
    pub async fn ingest_kline(&self, snapshot: &MarketKlineSnapshot) -> AppResult<()> {
        let entry = MarketKlineCacheEntry::from_snapshot(snapshot)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        let mongo_write = MarketKlineMongoWrite::from_snapshot(snapshot)?;
        ensure_kline_indexes(&self.database, mongo_write.symbol()).await?;
        self.cache
            .save_kline(entry)
            .await
            .map_err(market_cache_error)?;
        self.database
            .collection::<Document>(&mongo_write.collection_name())
            .update_one(mongo_write.upsert_filter(), mongo_write.upsert_update())
            .with_options(
                mongodb::options::UpdateOptions::builder()
                    .upsert(true)
                    .build(),
            )
            .await?;
        Ok(())
    }

    async fn trigger_spot_limit_orders(
        &self,
        symbol: &str,
        market_price: &BigDecimal,
        source: &str,
    ) {
        if let Some(pool) = &self.mysql
            && let Err(error) = execute_triggered_spot_limit_orders(
                pool,
                symbol,
                market_price,
                self.broadcast_hub.as_ref(),
            )
            .await
        {
            tracing::warn!(
                error = %error,
                symbol,
                market_price = %market_price,
                source,
                "spot limit order trigger failed after market ingestion"
            );
        }
    }
}

#[async_trait]
impl MarketIngestionSink for MarketIngestionService {
    async fn ingest_ticker(&self, snapshot: &MarketTickerSnapshot) -> AppResult<()> {
        MarketIngestionService::ingest_ticker(self, snapshot).await
    }

    async fn ingest_depth(&self, snapshot: &MarketDepthSnapshot) -> AppResult<()> {
        MarketIngestionService::ingest_depth(self, snapshot).await
    }

    async fn ingest_kline(&self, snapshot: &MarketKlineSnapshot) -> AppResult<()> {
        MarketIngestionService::ingest_kline(self, snapshot).await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketKlineMongoWrite {
    symbol: ValidatedMarketSymbol,
    interval: String,
    open_time: DateTime<Utc>,
    open: String,
    high: String,
    low: String,
    close: String,
    volume: String,
    source: String,
    updated_at: DateTime<Utc>,
}

impl MarketKlineMongoWrite {
    pub fn from_snapshot(snapshot: &MarketKlineSnapshot) -> AppResult<Self> {
        let symbol = ValidatedMarketSymbol::from_raw(snapshot.symbol())
            .map_err(|error| AppError::Validation(error.to_string()))?;
        KlineUpsertKey::new(snapshot.interval(), snapshot.open_time())
            .map_err(|error| AppError::Validation(error.to_string()))?;
        Ok(Self {
            symbol,
            interval: snapshot.interval().to_owned(),
            open_time: snapshot.open_time(),
            open: snapshot.open().to_string(),
            high: snapshot.high().to_string(),
            low: snapshot.low().to_string(),
            close: snapshot.close().to_string(),
            volume: snapshot.volume().to_string(),
            source: provider_name(snapshot.provider()).to_owned(),
            updated_at: snapshot.observed_at(),
        })
    }

    pub fn collection_name(&self) -> String {
        kline_collection_name(&self.symbol)
    }

    pub fn symbol(&self) -> &ValidatedMarketSymbol {
        &self.symbol
    }

    pub fn upsert_filter(&self) -> Document {
        doc! {
            "interval": &self.interval,
            "open_time": BsonDateTime::from_millis(self.open_time.timestamp_millis()),
        }
    }

    pub fn upsert_update(&self) -> Document {
        doc! {
            "$set": {
                "interval": &self.interval,
                "open_time": BsonDateTime::from_millis(self.open_time.timestamp_millis()),
                "open": &self.open,
                "high": &self.high,
                "low": &self.low,
                "close": &self.close,
                "volume": &self.volume,
                "source": &self.source,
                "updated_at": BsonDateTime::from_millis(self.updated_at.timestamp_millis()),
            }
        }
    }
}

fn market_cache_error(error: MarketCacheError) -> AppError {
    match error {
        MarketCacheError::Redis(error) => AppError::Redis(error),
        MarketCacheError::Json(error) => AppError::Internal(error.to_string()),
        MarketCacheError::Entry(error) => AppError::Validation(error.to_string()),
    }
}
