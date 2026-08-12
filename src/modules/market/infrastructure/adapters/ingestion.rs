//! 权威行情 ingestion 基础设施。
//!
//! 仅接收已由 provider 适配器归一化且通过领域校验的快照，按原顺序写 Redis 与 Mongo；
//! ticker/depth 缓存成功后才尝试触发现货订单，撮合失败不回滚已落地行情。

use super::feed::MarketFeedEvent;
use super::provider::provider_name;
use crate::{
    error::{AppError, AppResult},
    infra::mongo::{ensure_kline_indexes, kline_collection_name},
    modules::{
        events::{EventBroadcastHub, EventBroadcastMessage},
        market::{
            KlineUpsertKey, MarketDepthSnapshot, MarketKlineSnapshot, MarketTickerSnapshot,
            ValidatedMarketSymbol,
            infrastructure::{
                MarketCacheError, MarketCacheWriteOutcome, MarketDepthCacheEntry,
                MarketKlineCacheEntry, MarketTickerCacheEntry, RedisMarketCache,
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

/// synthetic 行情摄取的时序结果；拒绝表示 Redis 已有更新快照，调用方不得继续任何派生副作用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntheticIngestionOutcome {
    Accepted,
    RejectedStale,
}

impl SyntheticIngestionOutcome {
    /// 返回本次快照是否成为 Redis 权威值；worker 仅可在 true 时广播并推进检查点。
    pub fn is_accepted(self) -> bool {
        matches!(self, Self::Accepted)
    }
}

impl From<MarketCacheWriteOutcome> for SyntheticIngestionOutcome {
    fn from(value: MarketCacheWriteOutcome) -> Self {
        match value {
            MarketCacheWriteOutcome::Accepted => Self::Accepted,
            MarketCacheWriteOutcome::RejectedStale => Self::RejectedStale,
        }
    }
}

#[derive(Clone)]
pub struct MarketIngestionService {
    cache: RedisMarketCache,
    database: mongodb::Database,
    mysql: Option<Pool<MySql>>,
    broadcast_hub: Option<EventBroadcastHub>,
}

impl MarketIngestionService {
    /// 以 Redis 实时缓存和 Mongo 历史库构造行情摄取器，默认不触发现货订单，也不持有广播中心。
    /// 构造阶段不探测连接；缓存、索引或写入错误会在对应 `ingest_*` 调用时返回。
    pub fn new(cache: RedisMarketCache, database: mongodb::Database) -> Self {
        Self {
            cache,
            database,
            mysql: None,
            broadcast_hub: None,
        }
    }

    /// 从应用状态取得必需的 Redis、Mongo 以及可选 MySQL、广播中心；缺少前两者立即返回配置错误。
    /// 构造过程不发送 Redis/Mongo/WS 命令，也不探测连接可用性。
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

    /// 注入可选 MySQL 池，供 ticker/depth 缓存成功后触发现货限价单；不测试连接或立即执行 SQL。
    pub fn with_mysql(mut self, mysql: Option<Pool<MySql>>) -> Self {
        self.mysql = mysql;
        self
    }

    /// 注入进程内广播中心，供现货触发链发布订单事件；本方法本身不订阅或发布 WS 消息。
    pub fn with_broadcast_hub(mut self, broadcast_hub: Option<EventBroadcastHub>) -> Self {
        self.broadcast_hub = broadcast_hub;
        self
    }

    /// 将供应商 ticker 快照写入 Redis 权威缓存，成功后再尝试触发现货限价撮合。
    /// 快照必须已由 provider adapter 校验交易对、价格与时间；缓存失败时不得触发订单，撮合失败只告警且不撤销已写行情。
    pub async fn ingest_ticker(&self, snapshot: &MarketTickerSnapshot) -> AppResult<()> {
        let entry = MarketTickerCacheEntry::from_snapshot(snapshot)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        let outcome = self
            .cache
            .save_ticker_if_fresh(entry)
            .await
            .map_err(market_cache_error)?;
        if outcome.is_accepted() {
            self.trigger_spot_limit_orders(snapshot.symbol(), snapshot.last_price(), "ticker")
                .await;
        }
        Ok(())
    }

    /// 为 synthetic ticker 执行 Redis `observed_at` 原子 CAS；只有 accepted 才触发现货订单并按统一事件合同广播。
    /// stale/rejected 作为正常结果返回，Redis 保持更新值且本调用不广播，worker 也必须据此停止检查点推进。
    pub async fn ingest_and_publish_synthetic_ticker(
        &self,
        snapshot: &MarketTickerSnapshot,
    ) -> AppResult<SyntheticIngestionOutcome> {
        let entry = MarketTickerCacheEntry::from_snapshot(snapshot)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        let outcome = self
            .cache
            .save_ticker_if_fresh(entry)
            .await
            .map_err(market_cache_error)?;
        if !outcome.is_accepted() {
            return Ok(SyntheticIngestionOutcome::RejectedStale);
        }
        self.trigger_spot_limit_orders(snapshot.symbol(), snapshot.last_price(), "ticker")
            .await;
        self.publish(MarketFeedEvent::from_ticker_snapshot(snapshot)?)?;
        Ok(SyntheticIngestionOutcome::Accepted)
    }

    /// 兼容现有内部调用名并委托 synthetic 时序摄取；返回值显式要求调用方处理拒写。
    /// 新代码应优先使用 [`Self::ingest_and_publish_synthetic_ticker`] 表达仅供策略行情的副作用门禁。
    pub async fn ingest_and_publish_ticker(
        &self,
        snapshot: &MarketTickerSnapshot,
    ) -> AppResult<SyntheticIngestionOutcome> {
        self.ingest_and_publish_synthetic_ticker(snapshot).await
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

    /// 将 K 线依次写入 Redis 最新快照与 Mongo 历史集合；交易对、周期和开盘时间必须先通过领域校验。
    /// Mongo 以 interval+open_time 唯一索引 upsert，重放只覆盖同一根蜡烛；两次写入不在同一事务中，Mongo 失败时 Redis 快照不会回滚。
    pub async fn ingest_kline(&self, snapshot: &MarketKlineSnapshot) -> AppResult<()> {
        let entry = MarketKlineCacheEntry::from_snapshot(snapshot)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        let outcome = self
            .cache
            .save_kline_if_fresh(entry)
            .await
            .map_err(market_cache_error)?;
        if outcome.is_accepted() {
            self.upsert_kline_mongo(snapshot).await?;
        }
        Ok(())
    }

    /// 为 synthetic K 线执行 `(open_time, observed_at)` 原子时序门禁，accepted 后才 upsert Mongo 并广播。
    /// 同分钟旧 owner 或旧分钟快照被拒后不会覆盖 Mongo、不会发布陈旧 forming candle，也不会允许 worker 推进检查点。
    pub async fn ingest_and_publish_synthetic_kline(
        &self,
        snapshot: &MarketKlineSnapshot,
    ) -> AppResult<SyntheticIngestionOutcome> {
        let entry = MarketKlineCacheEntry::from_snapshot(snapshot)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        let outcome = self
            .cache
            .save_kline_if_fresh(entry)
            .await
            .map_err(market_cache_error)?;
        if !outcome.is_accepted() {
            return Ok(SyntheticIngestionOutcome::RejectedStale);
        }
        self.upsert_kline_mongo(snapshot).await?;
        self.publish(MarketFeedEvent::from_kline_snapshot(snapshot)?)?;
        Ok(SyntheticIngestionOutcome::Accepted)
    }

    /// 兼容现有内部调用名并返回 synthetic 时序结果；调用方必须处理 `RejectedStale`，不得默认推进检查点。
    pub async fn ingest_and_publish_kline(
        &self,
        snapshot: &MarketKlineSnapshot,
    ) -> AppResult<SyntheticIngestionOutcome> {
        self.ingest_and_publish_synthetic_kline(snapshot).await
    }

    async fn upsert_kline_mongo(&self, snapshot: &MarketKlineSnapshot) -> AppResult<()> {
        let mongo_write = MarketKlineMongoWrite::from_snapshot(snapshot)?;
        ensure_kline_indexes(&self.database, mongo_write.symbol()).await?;
        let collection = self
            .database
            .collection::<Document>(&mongo_write.collection_name());
        match collection.find_one(mongo_write.upsert_filter()).await? {
            Some(existing) if mongo_write.existing_is_newer(&existing) => {
                return Err(AppError::Conflict(
                    "market kline rejected because a newer Mongo snapshot already exists"
                        .to_owned(),
                ));
            }
            Some(_) => {
                let result = collection
                    .update_one(
                        mongo_write.fresh_existing_filter(),
                        mongo_write.upsert_update(),
                    )
                    .await?;
                if result.matched_count == 0 {
                    return Err(AppError::Conflict(
                        "market kline lost a concurrent freshness race".to_owned(),
                    ));
                }
            }
            None if !collection.insert_if_absent(&mongo_write).await? => {
                return Err(AppError::Conflict(
                    "market kline lost a concurrent first-write race".to_owned(),
                ));
            }
            None => {}
        }
        Ok(())
    }

    fn publish(&self, event: MarketFeedEvent) -> AppResult<()> {
        if let Some(hub) = &self.broadcast_hub {
            hub.publish(EventBroadcastMessage::from_market_feed_event(&event)?);
        }
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
    /// 把领域 K 线转换为 Mongo 写模型：数值存十进制字符串，`source` 存 provider 名，`updated_at` 取观察时间。
    /// 交易对和周期会重新校验；此步骤不访问 Mongo。
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

    /// 返回 K 线集合名称。
    pub fn collection_name(&self) -> String {
        kline_collection_name(&self.symbol)
    }

    /// 返回交易对符号。
    pub fn symbol(&self) -> &ValidatedMarketSymbol {
        &self.symbol
    }

    /// 生成 Mongo K 线 upsert 条件，仅以 `interval + open_time` 命中交易对专属集合中的同一根蜡烛。
    pub fn upsert_filter(&self) -> Document {
        doc! {
            "interval": &self.interval,
            "open_time": BsonDateTime::from_millis(self.open_time.timestamp_millis()),
        }
    }

    /// 生成仅更新既有且观察时间不晚于传入值的 Mongo 条件；同槽旧 owner 不能在 Redis CAS 之后倒退历史值。
    /// 首次插入由独立唯一键 insert 完成，避免带时序条件的 upsert 在竞争失败时触发重复键错误。
    pub fn fresh_existing_filter(&self) -> Document {
        doc! {
            "interval": &self.interval,
            "open_time": BsonDateTime::from_millis(self.open_time.timestamp_millis()),
            "$or": [
                { "updated_at": { "$exists": false } },
                { "updated_at": { "$lte": BsonDateTime::from_millis(self.updated_at.timestamp_millis()) } },
            ],
        }
    }

    /// 判断已存在同槽文档的 `updated_at` 是否严格晚于本次快照；缺失或非法时间按旧数据处理并允许修复。
    pub fn existing_is_newer(&self, document: &Document) -> bool {
        document
            .get_datetime("updated_at")
            .ok()
            .is_some_and(|value| value.timestamp_millis() > self.updated_at.timestamp_millis())
    }

    /// 构造首次插入文档；字段与 `$set` 合同一致，唯一索引冲突表示已有并发写者而非第二条历史记录。
    pub fn insert_document(&self) -> Document {
        doc! {
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

    /// 生成 Mongo `$set` 文档，覆盖同一周期和开盘时间的 OHLC、成交量、provider 与观察时间。
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

trait KlineCollectionFreshInsert {
    /// 仅在当前周期尚无行情时插入，并将并发唯一键冲突视为未接受。
    async fn insert_if_absent(&self, write: &MarketKlineMongoWrite) -> AppResult<bool>;
}

impl KlineCollectionFreshInsert for mongodb::Collection<Document> {
    async fn insert_if_absent(&self, write: &MarketKlineMongoWrite) -> AppResult<bool> {
        match self.insert_one(write.insert_document()).await {
            Ok(_) => Ok(true),
            Err(error) if error.to_string().contains("E11000") => Ok(false),
            Err(error) => Err(AppError::Mongo(error)),
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
