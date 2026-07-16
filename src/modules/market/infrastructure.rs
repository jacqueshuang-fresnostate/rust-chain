//! market bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。

use crate::{
    architecture::InfrastructureLayer,
    error::{AppError, AppResult},
    modules::market::{
        KlineQuery, KlineUpsertKey, MarketCacheEntryError, MarketDataProvider, MarketDepthLevel,
        MarketDepthSnapshot, MarketKlineSnapshot, MarketKlineValues, MarketSymbolError,
        MarketTickerSnapshot, MarketTickerValues, MarketTradeSide, MarketTradeTick,
        ValidatedMarketSymbol,
        presentation::{
            DepthCachePayload, DepthResponse, KlineResponse, MarketResponse, TickerResponse,
            TradeResponse,
        },
        repository::{KlineDocumentRecord, SpotTradeRecord},
        sanitize_symbol,
    },
    time::unix_millis,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use mongodb::{
    Database,
    bson::{DateTime as BsonDateTime, Document, doc},
};
use redis::AsyncCommands;
use serde::Serialize;
use sqlx::{MySql, Pool};
use thiserror::Error;

#[derive(Debug)]
pub struct InfrastructureLayerMarker;

impl InfrastructureLayer for InfrastructureLayerMarker {}

// Redis 缓存 DTO 保持和现有前端/撮合读取格式兼容，key 生成集中在基础设施层。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MarketTickerCacheEntry {
    symbol: String,
    last_price: BigDecimal,
    high_24h: BigDecimal,
    low_24h: BigDecimal,
    volume_24h: BigDecimal,
    price_change_24h: BigDecimal,
    price_change_percent_24h: BigDecimal,
    #[serde(with = "unix_millis")]
    observed_at: DateTime<Utc>,
    redis_key: String,
}

impl MarketTickerCacheEntry {
    pub fn new(
        symbol: &str,
        last_price: BigDecimal,
        volume_24h: BigDecimal,
        observed_at: DateTime<Utc>,
    ) -> Result<Self, MarketSymbolError> {
        Self::with_24h(
            symbol,
            MarketTickerValues::flat(last_price, volume_24h),
            observed_at,
        )
    }

    pub fn with_24h(
        symbol: &str,
        values: MarketTickerValues,
        observed_at: DateTime<Utc>,
    ) -> Result<Self, MarketSymbolError> {
        let symbol = ValidatedMarketSymbol::from_raw(symbol)?.as_str().to_owned();
        let redis_key = market_ticker_redis_key(&symbol);
        Ok(Self {
            symbol,
            last_price: values.last_price,
            high_24h: values.high_24h,
            low_24h: values.low_24h,
            volume_24h: values.volume_24h,
            price_change_24h: values.price_change_24h,
            price_change_percent_24h: values.price_change_percent_24h,
            observed_at,
            redis_key,
        })
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub fn last_price(&self) -> &BigDecimal {
        &self.last_price
    }

    pub fn high_24h(&self) -> &BigDecimal {
        &self.high_24h
    }

    pub fn low_24h(&self) -> &BigDecimal {
        &self.low_24h
    }

    pub fn volume_24h(&self) -> &BigDecimal {
        &self.volume_24h
    }

    pub fn price_change_24h(&self) -> &BigDecimal {
        &self.price_change_24h
    }

    pub fn price_change_percent_24h(&self) -> &BigDecimal {
        &self.price_change_percent_24h
    }

    pub fn observed_at(&self) -> DateTime<Utc> {
        self.observed_at
    }

    pub fn redis_key(&self) -> &str {
        &self.redis_key
    }

    pub fn from_snapshot(snapshot: &MarketTickerSnapshot) -> Result<Self, MarketSymbolError> {
        Self::with_24h(
            snapshot.symbol(),
            MarketTickerValues::new(
                snapshot.last_price().clone(),
                snapshot.high_24h().clone(),
                snapshot.low_24h().clone(),
                snapshot.volume_24h().clone(),
                snapshot.price_change_24h().clone(),
                snapshot.price_change_percent_24h().clone(),
            ),
            snapshot.observed_at(),
        )
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MarketDepthCacheEntry {
    symbol: String,
    bids: Vec<MarketDepthLevel>,
    asks: Vec<MarketDepthLevel>,
    #[serde(with = "unix_millis")]
    observed_at: DateTime<Utc>,
    redis_key: String,
}

impl MarketDepthCacheEntry {
    pub fn new(
        symbol: &str,
        bids: Vec<MarketDepthLevel>,
        asks: Vec<MarketDepthLevel>,
        observed_at: DateTime<Utc>,
    ) -> Result<Self, MarketSymbolError> {
        let symbol = ValidatedMarketSymbol::from_raw(symbol)?.as_str().to_owned();
        let redis_key = market_depth_redis_key(&symbol);
        Ok(Self {
            symbol,
            bids,
            asks,
            observed_at,
            redis_key,
        })
    }

    pub fn from_snapshot(snapshot: &MarketDepthSnapshot) -> Result<Self, MarketSymbolError> {
        Self::new(
            snapshot.symbol(),
            snapshot.bids().to_vec(),
            snapshot.asks().to_vec(),
            snapshot.observed_at(),
        )
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub fn bids(&self) -> &[MarketDepthLevel] {
        &self.bids
    }

    pub fn asks(&self) -> &[MarketDepthLevel] {
        &self.asks
    }

    pub fn observed_at(&self) -> DateTime<Utc> {
        self.observed_at
    }

    pub fn redis_key(&self) -> &str {
        &self.redis_key
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MarketKlineCacheEntry {
    symbol: String,
    interval: String,
    #[serde(with = "unix_millis")]
    open_time: DateTime<Utc>,
    open: BigDecimal,
    high: BigDecimal,
    low: BigDecimal,
    close: BigDecimal,
    volume: BigDecimal,
    redis_key: String,
}

impl MarketKlineCacheEntry {
    pub fn new(
        symbol: &str,
        interval: &str,
        open_time: DateTime<Utc>,
        values: MarketKlineValues,
    ) -> Result<Self, MarketCacheEntryError> {
        let symbol = ValidatedMarketSymbol::from_raw(symbol)?.as_str().to_owned();
        KlineUpsertKey::new(interval, open_time)?;
        let interval = interval.to_owned();
        let redis_key = market_kline_redis_key(&symbol, &interval);
        Ok(Self {
            symbol,
            interval,
            open_time,
            open: values.open,
            high: values.high,
            low: values.low,
            close: values.close,
            volume: values.volume,
            redis_key,
        })
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub fn interval(&self) -> &str {
        &self.interval
    }

    pub fn open_time(&self) -> DateTime<Utc> {
        self.open_time
    }

    pub fn open(&self) -> &BigDecimal {
        &self.open
    }

    pub fn high(&self) -> &BigDecimal {
        &self.high
    }

    pub fn low(&self) -> &BigDecimal {
        &self.low
    }

    pub fn close(&self) -> &BigDecimal {
        &self.close
    }

    pub fn volume(&self) -> &BigDecimal {
        &self.volume
    }

    pub fn redis_key(&self) -> &str {
        &self.redis_key
    }

    pub fn from_snapshot(snapshot: &MarketKlineSnapshot) -> Result<Self, MarketCacheEntryError> {
        Self::new(
            snapshot.symbol(),
            snapshot.interval(),
            snapshot.open_time(),
            MarketKlineValues {
                open: snapshot.open().clone(),
                high: snapshot.high().clone(),
                low: snapshot.low().clone(),
                close: snapshot.close().clone(),
                volume: snapshot.volume().clone(),
            },
        )
    }
}

pub fn market_ticker_redis_key(symbol: &str) -> String {
    format!("market:ticker:{}", sanitize_symbol(symbol))
}

pub fn market_depth_redis_key(symbol: &str) -> String {
    format!("market:depth:{}", sanitize_symbol(symbol))
}

pub fn market_kline_redis_key(symbol: &str, interval: &str) -> String {
    format!("market:kline:{}:{}", sanitize_symbol(symbol), interval)
}

#[derive(Clone)]
pub struct RedisMarketCache {
    manager: redis::aio::ConnectionManager,
}

impl RedisMarketCache {
    pub fn new(manager: redis::aio::ConnectionManager) -> Self {
        Self { manager }
    }

    pub async fn save_ticker(&self, entry: MarketTickerCacheEntry) -> Result<(), MarketCacheError> {
        let symbol =
            ValidatedMarketSymbol::from_raw(entry.symbol()).map_err(MarketCacheEntryError::from)?;
        let key = market_ticker_redis_key(symbol.as_str());
        self.save_json(&key, &entry).await
    }

    pub async fn save_depth(&self, entry: MarketDepthCacheEntry) -> Result<(), MarketCacheError> {
        let symbol =
            ValidatedMarketSymbol::from_raw(entry.symbol()).map_err(MarketCacheEntryError::from)?;
        let key = market_depth_redis_key(symbol.as_str());
        self.save_json(&key, &entry).await
    }

    pub async fn save_kline(&self, entry: MarketKlineCacheEntry) -> Result<(), MarketCacheError> {
        let symbol =
            ValidatedMarketSymbol::from_raw(entry.symbol()).map_err(MarketCacheEntryError::from)?;
        KlineUpsertKey::new(entry.interval(), entry.open_time())
            .map_err(MarketCacheEntryError::from)?;
        let key = market_kline_redis_key(symbol.as_str(), entry.interval());
        self.save_json(&key, &entry).await
    }

    async fn save_json<T: Serialize>(&self, key: &str, entry: &T) -> Result<(), MarketCacheError> {
        use redis::AsyncCommands;

        let payload = serde_json::to_string(entry)?;
        let mut connection = self.manager.clone();
        let _: () = connection.set(key, payload).await?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum MarketCacheError {
    #[error(transparent)]
    Redis(#[from] redis::RedisError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Entry(#[from] MarketCacheEntryError),
}

pub fn kline_collection_name(symbol: &ValidatedMarketSymbol) -> String {
    format!("market_klines_{}", symbol.as_str())
}

pub(crate) async fn list_active_markets(pool: &Pool<MySql>) -> AppResult<Vec<MarketResponse>> {
    let markets = sqlx::query_as::<_, MarketResponse>(
        r#"SELECT pairs.id,
                  pairs.symbol,
                  pairs.logo_url,
                  base.symbol AS base_asset,
                  quote.symbol AS quote_asset,
                  pairs.price_precision,
                  pairs.qty_precision,
                  CAST(pairs.min_order_value AS CHAR) AS min_order_value,
                  pairs.status,
                  pairs.market_type
           FROM trading_pairs pairs
           INNER JOIN assets base ON base.id = pairs.base_asset
           INNER JOIN assets quote ON quote.id = pairs.quote_asset
           WHERE pairs.status = 'active'
           ORDER BY pairs.symbol ASC"#,
    )
    .fetch_all(pool)
    .await?;

    Ok(markets)
}

pub(crate) async fn market_symbol_is_listed(pool: &Pool<MySql>, symbol: &str) -> AppResult<bool> {
    let listed = sqlx::query_as::<_, (i64,)>(
        r#"SELECT COUNT(*)
           FROM trading_pairs
           WHERE status = 'active'
             AND REPLACE(REPLACE(REPLACE(UPPER(symbol), '-', ''), '/', ''), '_', '') = ?"#,
    )
    .bind(symbol)
    .fetch_one(pool)
    .await?
    .0 > 0;

    Ok(listed)
}

pub(crate) async fn load_cached_ticker(
    redis: redis::aio::ConnectionManager,
    symbol: &str,
) -> AppResult<TickerResponse> {
    let mut connection = redis;
    let payload: Option<String> = connection.get(market_ticker_redis_key(symbol)).await?;
    let payload = payload.ok_or(AppError::NotFound)?;
    let ticker = serde_json::from_str::<TickerResponse>(&payload)
        .map_err(|error| AppError::Internal(format!("invalid cached ticker payload: {error}")))?;

    Ok(ticker)
}

pub(crate) async fn load_cached_depth(
    redis: redis::aio::ConnectionManager,
    symbol: &str,
) -> AppResult<DepthResponse> {
    let mut connection = redis;
    let payload: Option<String> = connection.get(market_depth_redis_key(symbol)).await?;
    let payload = payload.ok_or(AppError::NotFound)?;
    let depth = serde_json::from_str::<DepthCachePayload>(&payload)
        .map_err(|error| AppError::Internal(format!("invalid cached depth payload: {error}")))?;

    Ok(DepthResponse::from_cache(depth))
}

pub(crate) async fn list_recent_trades(
    pool: &Pool<MySql>,
    symbol: &str,
    limit: u32,
) -> AppResult<Vec<TradeResponse>> {
    let rows = sqlx::query_as::<_, SpotTradeRecord>(
        r#"SELECT trades.id,
                  pairs.symbol,
                  trades.price,
                  trades.quantity,
                  trades.created_at
           FROM spot_trades trades
           INNER JOIN trading_pairs pairs ON pairs.id = trades.pair_id
           WHERE REPLACE(REPLACE(REPLACE(UPPER(pairs.symbol), '-', ''), '/', ''), '_', '') = ?
           ORDER BY trades.created_at DESC, trades.id DESC
           LIMIT ?"#,
    )
    .bind(symbol)
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(TradeResponse::from_record).collect())
}

pub(crate) async fn list_klines(
    database: Database,
    symbol: &ValidatedMarketSymbol,
    query: KlineQuery,
) -> AppResult<Vec<KlineResponse>> {
    let collection = database.collection::<KlineDocumentRecord>(&kline_collection_name(symbol));
    let mut filter = doc! { "interval": &query.interval };
    let time_filter = kline_time_filter(query.start, query.end);
    if !time_filter.is_empty() {
        filter.insert("open_time", time_filter);
    }
    let options = mongodb::options::FindOptions::builder()
        .sort(doc! { "open_time": 1 })
        .limit(i64::from(query.limit))
        .build();
    let mut cursor = collection.find(filter).with_options(options).await?;
    let mut rows = Vec::new();
    while cursor.advance().await? {
        let document = cursor.deserialize_current()?;
        rows.push(KlineResponse::from_document(symbol.as_str(), document));
    }

    Ok(rows)
}

fn kline_time_filter(start: Option<DateTime<Utc>>, end: Option<DateTime<Utc>>) -> Document {
    let mut filter = Document::new();
    if let Some(start) = start {
        filter.insert("$gte", BsonDateTime::from_millis(start.timestamp_millis()));
    }
    if let Some(end) = end {
        filter.insert("$lte", BsonDateTime::from_millis(end.timestamp_millis()));
    }
    filter
}

pub mod adapters {
    use super::*;
    use crate::{
        config::Settings,
        error::{AppError, AppResult},
        infra::mongo::{ensure_kline_indexes, kline_collection_name},
        modules::{
            events::{EventBroadcastHub, EventBroadcastMessage, EventOutboxWriter},
            spot::application::execute_triggered_spot_limit_orders_with_hub as execute_triggered_spot_limit_orders,
        },
        state::AppState,
    };
    use axum::async_trait;
    use bigdecimal::BigDecimal;
    use chrono::{DateTime, Utc};
    use futures_util::{Stream, StreamExt};
    use mongodb::bson::{DateTime as BsonDateTime, Document, doc};
    use serde_json::{Value, json};
    use sqlx::{MySql, Pool};
    use std::{collections::VecDeque, str::FromStr};

    pub struct BitgetMarketAdapter;
    pub struct HtxMarketAdapter;
    pub struct CoinbaseMarketAdapter;

    #[async_trait]
    pub trait MarketIngestionSink: Clone + Send + Sync + 'static {
        async fn ingest_ticker(&self, snapshot: &MarketTickerSnapshot) -> AppResult<()>;
        async fn ingest_depth(&self, snapshot: &MarketDepthSnapshot) -> AppResult<()>;
        async fn ingest_kline(&self, snapshot: &MarketKlineSnapshot) -> AppResult<()>;
    }

    #[async_trait]
    pub trait MarketFeedRestFallbackHttpClient: Clone + Send + Sync + 'static {
        async fn get_text(&self, url: &str) -> AppResult<String>;
    }

    const REST_FALLBACK_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

    #[derive(Clone)]
    pub struct ReqwestMarketFeedRestFallbackHttpClient {
        client: reqwest::Client,
        timeout: std::time::Duration,
    }

    impl Default for ReqwestMarketFeedRestFallbackHttpClient {
        fn default() -> Self {
            Self::with_timeout(REST_FALLBACK_REQUEST_TIMEOUT)
        }
    }

    impl ReqwestMarketFeedRestFallbackHttpClient {
        pub fn new(client: reqwest::Client) -> Self {
            Self {
                client,
                timeout: REST_FALLBACK_REQUEST_TIMEOUT,
            }
        }

        pub fn with_timeout(timeout: std::time::Duration) -> Self {
            Self {
                client: reqwest::Client::new(),
                timeout,
            }
        }

        pub fn from_settings(settings: &Settings) -> Self {
            Self::with_timeout(std::time::Duration::from_secs(
                settings.market_feed_rest_fallback_timeout_seconds,
            ))
        }

        pub fn timeout(&self) -> std::time::Duration {
            self.timeout
        }
    }

    #[async_trait]
    impl MarketFeedRestFallbackHttpClient for ReqwestMarketFeedRestFallbackHttpClient {
        async fn get_text(&self, url: &str) -> AppResult<String> {
            let response = self
                .client
                .get(url)
                .timeout(self.timeout)
                .send()
                .await
                .map_err(|error| {
                    AppError::Internal(format!("market feed REST fallback request failed: {error}"))
                })?
                .error_for_status()
                .map_err(|error| {
                    AppError::Internal(format!("market feed REST fallback status failed: {error}"))
                })?;
            response.text().await.map_err(|error| {
                AppError::Internal(format!("market feed REST fallback body failed: {error}"))
            })
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
                AppError::Internal(
                    "redis connection is not configured for market ingestion".to_owned(),
                )
            })?;
            let mongo = state.mongo.clone().ok_or_else(|| {
                AppError::Internal(
                    "mongo database is not configured for market ingestion".to_owned(),
                )
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

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum MarketFeedProvider {
        Bitget,
        Htx,
        Coinbase,
    }

    impl MarketFeedProvider {
        pub fn from_code(code: &str) -> AppResult<Self> {
            let normalized = code.trim().to_ascii_lowercase();
            for provider in Self::available_providers() {
                if provider.aliases().contains(&normalized.as_str()) {
                    return Ok(*provider);
                }
            }
            Err(AppError::Validation(format!(
                "unsupported market feed provider: {normalized}"
            )))
        }

        pub const fn code(&self) -> &'static str {
            match self {
                Self::Bitget => "bitget",
                Self::Htx => "htx",
                Self::Coinbase => "coinbase",
            }
        }

        pub const fn aliases(&self) -> &'static [&'static str] {
            match self {
                Self::Bitget => &["bitget"],
                Self::Htx => &["htx", "huobi"],
                Self::Coinbase => &[
                    "coinbase",
                    "coinbase_advanced_trade",
                    "coinbase-advanced-trade",
                ],
            }
        }

        pub const fn default_providers() -> [Self; 2] {
            [Self::Bitget, Self::Htx]
        }

        pub const fn available_providers() -> &'static [Self] {
            &[Self::Bitget, Self::Htx, Self::Coinbase]
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum MarketFeedChannel {
        Ticker,
        Depth,
        Kline,
        Trade,
        None,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct MarketFeedFrame {
        provider: MarketFeedProvider,
        channel: MarketFeedChannel,
        payload: String,
    }

    impl MarketFeedFrame {
        pub fn new(
            provider: MarketFeedProvider,
            channel: MarketFeedChannel,
            payload: impl Into<String>,
        ) -> Self {
            Self {
                provider,
                channel,
                payload: payload.into(),
            }
        }

        pub fn bitget_ticker(payload: impl Into<String>) -> Self {
            Self::new(
                MarketFeedProvider::Bitget,
                MarketFeedChannel::Ticker,
                payload,
            )
        }

        pub fn bitget_depth(payload: impl Into<String>) -> Self {
            Self::new(
                MarketFeedProvider::Bitget,
                MarketFeedChannel::Depth,
                payload,
            )
        }

        pub fn bitget_kline(payload: impl Into<String>) -> Self {
            Self::new(
                MarketFeedProvider::Bitget,
                MarketFeedChannel::Kline,
                payload,
            )
        }

        pub fn bitget_trade(payload: impl Into<String>) -> Self {
            Self::new(
                MarketFeedProvider::Bitget,
                MarketFeedChannel::Trade,
                payload,
            )
        }

        pub fn htx_ticker(payload: impl Into<String>) -> Self {
            Self::new(MarketFeedProvider::Htx, MarketFeedChannel::Ticker, payload)
        }

        pub fn htx_depth(payload: impl Into<String>) -> Self {
            Self::new(MarketFeedProvider::Htx, MarketFeedChannel::Depth, payload)
        }

        pub fn htx_kline(payload: impl Into<String>) -> Self {
            Self::new(MarketFeedProvider::Htx, MarketFeedChannel::Kline, payload)
        }

        pub fn htx_trade(payload: impl Into<String>) -> Self {
            Self::new(MarketFeedProvider::Htx, MarketFeedChannel::Trade, payload)
        }

        pub fn coinbase_ticker(payload: impl Into<String>) -> Self {
            Self::new(
                MarketFeedProvider::Coinbase,
                MarketFeedChannel::Ticker,
                payload,
            )
        }

        pub fn coinbase_depth(payload: impl Into<String>) -> Self {
            Self::new(
                MarketFeedProvider::Coinbase,
                MarketFeedChannel::Depth,
                payload,
            )
        }

        pub fn coinbase_kline(payload: impl Into<String>) -> Self {
            Self::new(
                MarketFeedProvider::Coinbase,
                MarketFeedChannel::Kline,
                payload,
            )
        }

        pub fn coinbase_trade(payload: impl Into<String>) -> Self {
            Self::new(
                MarketFeedProvider::Coinbase,
                MarketFeedChannel::Trade,
                payload,
            )
        }

        pub fn provider(&self) -> MarketFeedProvider {
            self.provider
        }

        pub fn channel(&self) -> MarketFeedChannel {
            self.channel
        }

        pub fn payload(&self) -> &str {
            &self.payload
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct MarketFeedConfig {
        provider: MarketFeedProvider,
        url: String,
        subscription_messages: Vec<String>,
        symbols: Vec<String>,
        intervals: Vec<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct MarketFeedRestFallbackTickerRequest {
        symbol: String,
        url: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct MarketFeedRestFallbackKlineRequest {
        symbol: String,
        interval: String,
        url: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct MarketFeedRestFallbackConfig {
        provider: MarketFeedProvider,
        ticker_requests: Vec<MarketFeedRestFallbackTickerRequest>,
        kline_requests: Vec<MarketFeedRestFallbackKlineRequest>,
    }

    impl MarketFeedRestFallbackTickerRequest {
        pub fn new(symbol: impl Into<String>, url: impl Into<String>) -> Self {
            Self {
                symbol: symbol.into(),
                url: url.into(),
            }
        }

        pub fn symbol(&self) -> &str {
            &self.symbol
        }

        pub fn url(&self) -> &str {
            &self.url
        }
    }

    impl MarketFeedRestFallbackKlineRequest {
        pub fn new(
            symbol: impl Into<String>,
            interval: impl Into<String>,
            url: impl Into<String>,
        ) -> Self {
            Self {
                symbol: symbol.into(),
                interval: interval.into(),
                url: url.into(),
            }
        }

        pub fn symbol(&self) -> &str {
            &self.symbol
        }

        pub fn interval(&self) -> &str {
            &self.interval
        }

        pub fn url(&self) -> &str {
            &self.url
        }
    }

    impl MarketFeedRestFallbackConfig {
        pub fn new(
            provider: MarketFeedProvider,
            ticker_requests: Vec<MarketFeedRestFallbackTickerRequest>,
            kline_requests: Vec<MarketFeedRestFallbackKlineRequest>,
        ) -> Self {
            Self {
                provider,
                ticker_requests,
                kline_requests,
            }
        }

        pub fn provider(&self) -> MarketFeedProvider {
            self.provider
        }

        pub fn ticker_requests(&self) -> &[MarketFeedRestFallbackTickerRequest] {
            &self.ticker_requests
        }

        pub fn ticker_url(&self) -> &str {
            self.ticker_requests
                .first()
                .map(MarketFeedRestFallbackTickerRequest::url)
                .unwrap_or_default()
        }

        pub fn ticker_urls(&self) -> Vec<String> {
            self.ticker_requests
                .iter()
                .map(|request| request.url.clone())
                .collect()
        }

        pub fn kline_requests(&self) -> &[MarketFeedRestFallbackKlineRequest] {
            &self.kline_requests
        }

        pub fn kline_urls(&self) -> Vec<String> {
            self.kline_requests
                .iter()
                .map(|request| request.url.clone())
                .collect()
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct MarketFeedRestFallbackFrameRequest {
        channel: MarketFeedChannel,
        symbol: String,
        interval: Option<String>,
        url: String,
    }

    impl MarketFeedRestFallbackFrameRequest {
        fn ticker(request: &MarketFeedRestFallbackTickerRequest) -> Self {
            Self {
                channel: MarketFeedChannel::Ticker,
                symbol: request.symbol().to_owned(),
                interval: None,
                url: request.url().to_owned(),
            }
        }

        fn kline(request: &MarketFeedRestFallbackKlineRequest) -> Self {
            Self {
                channel: MarketFeedChannel::Kline,
                symbol: request.symbol().to_owned(),
                interval: Some(request.interval().to_owned()),
                url: request.url().to_owned(),
            }
        }
    }

    struct MarketFeedRestFallbackFrameResult {
        request: MarketFeedRestFallbackFrameRequest,
        result: Result<MarketFeedFrame, AppError>,
    }

    impl MarketFeedRestFallbackFrameResult {
        fn new(
            request: &MarketFeedRestFallbackFrameRequest,
            result: Result<MarketFeedFrame, AppError>,
        ) -> Self {
            Self {
                request: request.clone(),
                result,
            }
        }
    }

    impl MarketFeedConfig {
        pub fn new(
            provider: MarketFeedProvider,
            url: impl Into<String>,
            subscription_messages: Vec<String>,
            symbols: Vec<String>,
            intervals: Vec<String>,
        ) -> Self {
            Self {
                provider,
                url: url.into(),
                subscription_messages,
                symbols,
                intervals,
            }
        }

        pub fn provider(&self) -> MarketFeedProvider {
            self.provider
        }

        pub fn url(&self) -> &str {
            &self.url
        }

        pub fn subscription_messages(&self) -> &[String] {
            &self.subscription_messages
        }

        pub fn symbols(&self) -> &[String] {
            &self.symbols
        }

        pub fn intervals(&self) -> &[String] {
            &self.intervals
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct MarketFeedFailureContext {
        provider: MarketFeedProvider,
        channel: MarketFeedChannel,
        symbol: String,
        interval: Option<String>,
        url: String,
        error: String,
    }

    impl MarketFeedFailureContext {
        fn new(
            provider: MarketFeedProvider,
            request: &MarketFeedRestFallbackFrameRequest,
            error: &AppError,
        ) -> Self {
            Self {
                provider,
                channel: request.channel,
                symbol: request.symbol.clone(),
                interval: request.interval.clone(),
                url: request.url.clone(),
                error: error.to_string(),
            }
        }

        pub fn provider(&self) -> MarketFeedProvider {
            self.provider
        }

        pub fn channel(&self) -> MarketFeedChannel {
            self.channel
        }

        pub fn symbol(&self) -> &str {
            &self.symbol
        }

        pub fn interval(&self) -> Option<&str> {
            self.interval.as_deref()
        }

        pub fn url(&self) -> &str {
            &self.url
        }

        pub fn error(&self) -> &str {
            &self.error
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct MarketFeedSummary {
        pub received: u32,
        pub ingested: u32,
        pub failed: u32,
        failure_contexts: Vec<MarketFeedFailureContext>,
    }

    impl MarketFeedSummary {
        pub fn new(received: u32, ingested: u32, failed: u32) -> Self {
            Self {
                received,
                ingested,
                failed,
                failure_contexts: Vec::new(),
            }
        }

        fn record_failure(&mut self) {
            self.failed += 1;
        }

        fn record_failure_context(&mut self, context: MarketFeedFailureContext) {
            self.record_failure();
            self.failure_contexts.push(context);
        }

        pub fn failure_contexts(&self) -> &[MarketFeedFailureContext] {
            &self.failure_contexts
        }
    }

    #[derive(Clone)]
    pub struct MarketFeedWorker<S> {
        sink: S,
        broadcast_hub: Option<EventBroadcastHub>,
    }

    impl<S> MarketFeedWorker<S> {
        pub fn new(sink: S) -> Self {
            Self {
                sink,
                broadcast_hub: None,
            }
        }

        pub fn with_broadcast_hub(mut self, hub: EventBroadcastHub) -> Self {
            self.broadcast_hub = Some(hub);
            self
        }

        pub fn with_outbox_writer<N>(self, _outbox_writer: EventOutboxWriter<N>) -> Self {
            self
        }

        pub fn provider_configs(
            settings: &Settings,
            symbols: &[&str],
            intervals: &[&str],
        ) -> AppResult<Vec<MarketFeedConfig>> {
            Self::provider_configs_for(
                settings,
                &MarketFeedProvider::default_providers(),
                symbols,
                intervals,
            )
        }

        pub fn provider_configs_for(
            settings: &Settings,
            providers: &[MarketFeedProvider],
            symbols: &[&str],
            intervals: &[&str],
        ) -> AppResult<Vec<MarketFeedConfig>> {
            if providers.is_empty() {
                return Err(AppError::Validation(
                    "market feed providers are required".to_owned(),
                ));
            }
            let symbols = validate_feed_symbols(symbols)?;
            let intervals = validate_feed_intervals(intervals)?;
            providers
                .iter()
                .map(|provider| provider.feed_config(settings, &symbols, &intervals))
                .collect()
        }

        pub fn provider_rest_fallback_configs(
            settings: &Settings,
            symbols: &[&str],
            intervals: &[&str],
        ) -> AppResult<Vec<MarketFeedRestFallbackConfig>> {
            Self::provider_rest_fallback_configs_for(
                settings,
                &MarketFeedProvider::default_providers(),
                symbols,
                intervals,
            )
        }

        pub fn provider_rest_fallback_configs_for(
            settings: &Settings,
            providers: &[MarketFeedProvider],
            symbols: &[&str],
            intervals: &[&str],
        ) -> AppResult<Vec<MarketFeedRestFallbackConfig>> {
            if providers.is_empty() {
                return Err(AppError::Validation(
                    "market feed providers are required".to_owned(),
                ));
            }
            let symbols = validate_feed_symbols(symbols)?;
            let intervals = validate_feed_intervals(intervals)?;
            providers
                .iter()
                .map(|provider| provider.rest_fallback_config(settings, &symbols, &intervals))
                .collect()
        }
    }

    impl MarketFeedWorker<MarketIngestionService> {
        pub fn from_state(state: &AppState) -> AppResult<Self> {
            let worker = Self::new(MarketIngestionService::from_state(state)?);
            Ok(match state.event_broadcast_hub.clone() {
                Some(hub) => worker.with_broadcast_hub(hub),
                None => worker,
            })
        }
    }

    impl<S> MarketFeedWorker<S>
    where
        S: MarketIngestionSink,
    {
        pub async fn run_stream<E, St>(&self, frames: St) -> AppResult<MarketFeedSummary>
        where
            E: ToString,
            St: Stream<Item = Result<MarketFeedFrame, E>> + Send,
        {
            futures_util::pin_mut!(frames);
            let mut summary = MarketFeedSummary::default();

            while let Some(frame) = frames.next().await {
                summary.received += 1;
                match frame {
                    Ok(frame) => match self.ingest_frame(&frame).await {
                        Ok(()) => summary.ingested += 1,
                        Err(_) => summary.record_failure(),
                    },
                    Err(_) => summary.record_failure(),
                }
            }

            Ok(summary)
        }

        pub async fn run_rest_fallback_config<C>(
            &self,
            config: &MarketFeedRestFallbackConfig,
            http_client: &C,
        ) -> AppResult<MarketFeedSummary>
        where
            C: MarketFeedRestFallbackHttpClient,
        {
            let frames = fetch_rest_fallback_frames(config, http_client).await?;
            let mut summary = MarketFeedSummary::default();
            for frame in frames {
                summary.received += 1;
                match frame.result {
                    Ok(frame) => match self.ingest_frame(&frame).await {
                        Ok(()) => summary.ingested += 1,
                        Err(_) => summary.record_failure(),
                    },
                    Err(error) => summary.record_failure_context(MarketFeedFailureContext::new(
                        config.provider(),
                        &frame.request,
                        &error,
                    )),
                }
            }
            Ok(summary)
        }

        pub async fn ingest_frame(&self, frame: &MarketFeedFrame) -> AppResult<()> {
            let parsed = parse_feed_frame(frame)?;
            let event = MarketFeedEvent::from_parsed(&parsed)?;
            match &parsed {
                ParsedMarketFeed::Ticker(snapshot) => self.sink.ingest_ticker(snapshot).await?,
                ParsedMarketFeed::Depth(snapshot) => self.sink.ingest_depth(snapshot).await?,
                ParsedMarketFeed::Kline(snapshot) => self.sink.ingest_kline(snapshot).await?,
                ParsedMarketFeed::Trade(_) => {}
            }
            if let Some(hub) = &self.broadcast_hub {
                hub.publish(EventBroadcastMessage::from_market_feed_event(&event)?);
            }
            Ok(())
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct MarketFeedEvent {
        aggregate_type: String,
        aggregate_id: String,
        event_type: String,
        routing_key: String,
        idempotency_key: String,
        public_ws_namespace: String,
        public_ws_topic: String,
        payload: Value,
    }

    impl MarketFeedEvent {
        pub fn from_frame(frame: &MarketFeedFrame) -> AppResult<Self> {
            let parsed = parse_feed_frame(frame)?;
            Self::from_parsed(&parsed)
        }

        fn from_parsed(parsed: &ParsedMarketFeed) -> AppResult<Self> {
            match parsed {
                ParsedMarketFeed::Ticker(snapshot) => Ok(Self {
                    aggregate_type: "market_ticker".to_owned(),
                    aggregate_id: snapshot.symbol().to_owned(),
                    event_type: "ticker_updated".to_owned(),
                    routing_key: format!("market.{}.ticker", snapshot.symbol()),
                    idempotency_key: format!(
                        "market_feed:{}:{}:ticker:{}",
                        provider_name(snapshot.provider()),
                        snapshot.symbol(),
                        snapshot.observed_at().timestamp_millis()
                    ),
                    public_ws_namespace: "ticker".to_owned(),
                    public_ws_topic: snapshot.symbol().to_owned(),
                    payload: json!({
                        "symbol": snapshot.symbol(),
                        "last_price": snapshot.last_price().to_string(),
                        "high_24h": snapshot.high_24h().to_string(),
                        "low_24h": snapshot.low_24h().to_string(),
                        "volume_24h": snapshot.volume_24h().to_string(),
                        "price_change_24h": snapshot.price_change_24h().to_string(),
                        "price_change_percent_24h": snapshot.price_change_percent_24h().to_string(),
                        "observed_at": snapshot.observed_at().timestamp_millis(),
                        "provider": provider_name(snapshot.provider()),
                    }),
                }),
                ParsedMarketFeed::Depth(snapshot) => Ok(Self {
                    aggregate_type: "market_depth".to_owned(),
                    aggregate_id: snapshot.symbol().to_owned(),
                    event_type: "depth_updated".to_owned(),
                    routing_key: format!("market.{}.depth", snapshot.symbol()),
                    idempotency_key: format!(
                        "market_feed:{}:{}:depth:{}",
                        provider_name(snapshot.provider()),
                        snapshot.symbol(),
                        snapshot.observed_at().timestamp_millis()
                    ),
                    public_ws_namespace: "depth".to_owned(),
                    public_ws_topic: snapshot.symbol().to_owned(),
                    payload: json!({
                        "symbol": snapshot.symbol(),
                        "bids": snapshot.bids(),
                        "asks": snapshot.asks(),
                        "observed_at": snapshot.observed_at().timestamp_millis(),
                        "provider": provider_name(snapshot.provider()),
                    }),
                }),
                ParsedMarketFeed::Kline(snapshot) => Ok(Self {
                    aggregate_type: "market_kline".to_owned(),
                    aggregate_id: format!("{}:{}", snapshot.symbol(), snapshot.interval()),
                    event_type: "kline_updated".to_owned(),
                    routing_key: format!(
                        "market.{}.kline.{}",
                        snapshot.symbol(),
                        snapshot.interval()
                    ),
                    idempotency_key: format!(
                        "market_feed:{}:{}:kline:{}:{}:{}",
                        provider_name(snapshot.provider()),
                        snapshot.symbol(),
                        snapshot.interval(),
                        snapshot.open_time().timestamp_millis(),
                        market_feed_payload_hash(&json!({
                            "open": snapshot.open().to_string(),
                            "high": snapshot.high().to_string(),
                            "low": snapshot.low().to_string(),
                            "close": snapshot.close().to_string(),
                            "volume": snapshot.volume().to_string(),
                            "observed_at": snapshot.observed_at().timestamp_millis(),
                        }))
                    ),
                    public_ws_namespace: "kline".to_owned(),
                    public_ws_topic: format!("{}_{}", snapshot.symbol(), snapshot.interval()),
                    payload: json!({
                        "symbol": snapshot.symbol(),
                        "interval": snapshot.interval(),
                        "open_time": snapshot.open_time().timestamp_millis(),
                        "open": snapshot.open().to_string(),
                        "high": snapshot.high().to_string(),
                        "low": snapshot.low().to_string(),
                        "close": snapshot.close().to_string(),
                        "volume": snapshot.volume().to_string(),
                        "observed_at": snapshot.observed_at().timestamp_millis(),
                        "provider": provider_name(snapshot.provider()),
                    }),
                }),
                ParsedMarketFeed::Trade(tick) => Ok(Self {
                    aggregate_type: "market_trade".to_owned(),
                    aggregate_id: tick.trade_id().to_owned(),
                    event_type: "trade_created".to_owned(),
                    routing_key: format!("market.{}.trade", tick.symbol()),
                    idempotency_key: format!(
                        "market_feed:{}:{}:trade:{}",
                        provider_name(tick.provider()),
                        tick.symbol(),
                        tick.trade_id()
                    ),
                    public_ws_namespace: "trade".to_owned(),
                    public_ws_topic: tick.symbol().to_owned(),
                    payload: json!({
                        "symbol": tick.symbol(),
                        "trade_id": tick.trade_id(),
                        "side": tick.side(),
                        "price": tick.price().to_string(),
                        "quantity": tick.quantity().to_string(),
                        "traded_at": tick.traded_at().timestamp_millis(),
                        "provider": provider_name(tick.provider()),
                    }),
                }),
            }
        }

        pub fn aggregate_type(&self) -> &str {
            &self.aggregate_type
        }

        pub fn aggregate_id(&self) -> &str {
            &self.aggregate_id
        }

        pub fn event_type(&self) -> &str {
            &self.event_type
        }

        pub fn routing_key(&self) -> &str {
            &self.routing_key
        }

        pub fn idempotency_key(&self) -> &str {
            &self.idempotency_key
        }

        pub fn public_ws_namespace(&self) -> &str {
            &self.public_ws_namespace
        }

        pub fn public_ws_topic(&self) -> &str {
            &self.public_ws_topic
        }

        pub fn payload(&self) -> &Value {
            &self.payload
        }
    }

    enum ParsedMarketFeed {
        Ticker(MarketTickerSnapshot),
        Depth(MarketDepthSnapshot),
        Kline(MarketKlineSnapshot),
        Trade(MarketTradeTick),
    }

    impl MarketFeedProvider {
        fn feed_config(
            &self,
            settings: &Settings,
            symbols: &[String],
            intervals: &[String],
        ) -> AppResult<MarketFeedConfig> {
            Ok(MarketFeedConfig::new(
                *self,
                self.feed_url(settings),
                self.subscription_messages(symbols, intervals),
                symbols.to_vec(),
                intervals.to_vec(),
            ))
        }

        fn rest_fallback_config(
            &self,
            settings: &Settings,
            symbols: &[String],
            intervals: &[String],
        ) -> AppResult<MarketFeedRestFallbackConfig> {
            Ok(MarketFeedRestFallbackConfig::new(
                *self,
                self.ticker_fallback_requests(settings, symbols),
                self.kline_fallback_requests(settings, symbols, intervals),
            ))
        }

        fn feed_url(&self, settings: &Settings) -> String {
            match self {
                Self::Bitget => settings.bitget_ws_url.clone(),
                Self::Htx => settings.htx_ws_url.clone(),
                Self::Coinbase => settings.coinbase_ws_url.clone(),
            }
        }

        fn ticker_fallback_requests(
            &self,
            settings: &Settings,
            symbols: &[String],
        ) -> Vec<MarketFeedRestFallbackTickerRequest> {
            symbols
                .iter()
                .map(|symbol| {
                    MarketFeedRestFallbackTickerRequest::new(
                        symbol.clone(),
                        self.ticker_fallback_url(settings, symbol),
                    )
                })
                .collect()
        }

        fn ticker_fallback_url(&self, settings: &Settings, symbol: &str) -> String {
            match self {
                Self::Bitget => format!(
                    "{}/api/v2/spot/market/tickers?symbol={symbol}",
                    settings.bitget_rest_base_url.trim_end_matches('/')
                ),
                Self::Htx => format!(
                    "{}/market/detail/merged?symbol={}",
                    settings.htx_rest_base_url.trim_end_matches('/'),
                    symbol.to_ascii_lowercase()
                ),
                Self::Coinbase => format!(
                    "{}/api/v3/brokerage/market/products/{}",
                    settings.coinbase_rest_base_url.trim_end_matches('/'),
                    coinbase_product_id(symbol)
                ),
            }
        }

        fn kline_fallback_requests(
            &self,
            settings: &Settings,
            symbols: &[String],
            intervals: &[String],
        ) -> Vec<MarketFeedRestFallbackKlineRequest> {
            symbols
                .iter()
                .flat_map(|symbol| {
                    intervals.iter().map(move |interval| {
                        MarketFeedRestFallbackKlineRequest::new(
                            symbol.clone(),
                            interval.clone(),
                            self.kline_fallback_url(settings, symbol, interval),
                        )
                    })
                })
                .collect()
        }

        fn kline_fallback_url(&self, settings: &Settings, symbol: &str, interval: &str) -> String {
            match self {
                Self::Bitget => format!(
                    "{}/api/v2/spot/market/candles?symbol={symbol}&granularity={}",
                    settings.bitget_rest_base_url.trim_end_matches('/'),
                    bitget_rest_interval(interval)
                ),
                Self::Htx => format!(
                    "{}/market/history/kline?symbol={}&period={}",
                    settings.htx_rest_base_url.trim_end_matches('/'),
                    symbol.to_ascii_lowercase(),
                    htx_subscription_interval(interval)
                ),
                Self::Coinbase => {
                    let (granularity, seconds) = coinbase_rest_granularity(interval);
                    let end = Utc::now().timestamp();
                    let start = end.saturating_sub(seconds * 300);
                    format!(
                        "{}/api/v3/brokerage/market/products/{}/candles?start={start}&end={end}&granularity={granularity}",
                        settings.coinbase_rest_base_url.trim_end_matches('/'),
                        coinbase_product_id(symbol)
                    )
                }
            }
        }

        fn subscription_messages(&self, symbols: &[String], intervals: &[String]) -> Vec<String> {
            match self {
                Self::Bitget => bitget_subscriptions(symbols, intervals),
                Self::Htx => htx_subscriptions(symbols, intervals),
                Self::Coinbase => coinbase_subscriptions(symbols, intervals),
            }
        }
    }

    async fn fetch_rest_fallback_frames<C>(
        config: &MarketFeedRestFallbackConfig,
        http_client: &C,
    ) -> AppResult<Vec<MarketFeedRestFallbackFrameResult>>
    where
        C: MarketFeedRestFallbackHttpClient,
    {
        let mut requests =
            VecDeque::with_capacity(config.ticker_requests().len() + config.kline_requests().len());
        requests.extend(
            config
                .ticker_requests()
                .iter()
                .map(MarketFeedRestFallbackFrameRequest::ticker),
        );
        requests.extend(
            config
                .kline_requests()
                .iter()
                .map(MarketFeedRestFallbackFrameRequest::kline),
        );

        let mut frames = Vec::with_capacity(requests.len());
        while let Some(request) = requests.pop_front() {
            match http_client.get_text(&request.url).await {
                Ok(payload) => match rest_fallback_frames(config.provider(), &request, &payload) {
                    Ok(payload_frames) => {
                        frames.extend(payload_frames.into_iter().map(|frame| {
                            MarketFeedRestFallbackFrameResult::new(&request, Ok(frame))
                        }))
                    }
                    Err(error) => {
                        frames.push(MarketFeedRestFallbackFrameResult::new(&request, Err(error)))
                    }
                },
                Err(error) => {
                    frames.push(MarketFeedRestFallbackFrameResult::new(&request, Err(error)))
                }
            }
        }
        Ok(frames)
    }

    fn rest_fallback_frames(
        provider: MarketFeedProvider,
        request: &MarketFeedRestFallbackFrameRequest,
        payload: &str,
    ) -> AppResult<Vec<MarketFeedFrame>> {
        let channel = request.channel;
        let payloads = match (provider, channel) {
            (MarketFeedProvider::Bitget, MarketFeedChannel::Ticker) => {
                vec![bitget_rest_ticker_payload(payload, &request.symbol)?]
            }
            (MarketFeedProvider::Bitget, MarketFeedChannel::Kline) => bitget_rest_kline_payloads(
                payload,
                &request.symbol,
                required_rest_fallback_interval(request)?,
            )?,
            (MarketFeedProvider::Htx, MarketFeedChannel::Ticker) => {
                vec![htx_rest_ticker_payload(payload, &request.symbol)?]
            }
            (MarketFeedProvider::Htx, MarketFeedChannel::Kline) => htx_rest_kline_payloads(
                payload,
                &request.symbol,
                required_rest_fallback_interval(request)?,
            )?,
            (MarketFeedProvider::Coinbase, MarketFeedChannel::Ticker) => {
                vec![coinbase_rest_ticker_payload(payload, &request.symbol)?]
            }
            (MarketFeedProvider::Coinbase, MarketFeedChannel::Kline) => {
                coinbase_rest_kline_payloads(
                    payload,
                    &request.symbol,
                    required_rest_fallback_interval(request)?,
                )?
            }
            (_, MarketFeedChannel::Depth | MarketFeedChannel::Trade | MarketFeedChannel::None) => {
                return Err(AppError::Validation(
                    "unsupported market feed REST fallback channel".to_owned(),
                ));
            }
        };
        Ok(payloads
            .into_iter()
            .map(|payload| MarketFeedFrame::new(provider, channel, payload))
            .collect())
    }

    fn required_rest_fallback_interval(
        request: &MarketFeedRestFallbackFrameRequest,
    ) -> AppResult<&str> {
        request.interval.as_deref().ok_or_else(|| {
            AppError::Validation("market feed REST fallback interval is required".to_owned())
        })
    }

    fn parse_feed_frame(frame: &MarketFeedFrame) -> AppResult<ParsedMarketFeed> {
        match (frame.provider(), frame.channel()) {
            (MarketFeedProvider::Bitget, MarketFeedChannel::Ticker) => {
                BitgetMarketAdapter::ticker_from_ws(frame.payload()).map(ParsedMarketFeed::Ticker)
            }
            (MarketFeedProvider::Bitget, MarketFeedChannel::Depth) => {
                BitgetMarketAdapter::depth_from_ws(frame.payload()).map(ParsedMarketFeed::Depth)
            }
            (MarketFeedProvider::Bitget, MarketFeedChannel::Kline) => {
                BitgetMarketAdapter::kline_from_ws(frame.payload()).map(ParsedMarketFeed::Kline)
            }
            (MarketFeedProvider::Bitget, MarketFeedChannel::Trade) => {
                BitgetMarketAdapter::trade_from_ws(frame.payload()).map(ParsedMarketFeed::Trade)
            }
            (MarketFeedProvider::Htx, MarketFeedChannel::Ticker) => {
                HtxMarketAdapter::ticker_from_ws(frame.payload()).map(ParsedMarketFeed::Ticker)
            }
            (MarketFeedProvider::Htx, MarketFeedChannel::Depth) => {
                HtxMarketAdapter::depth_from_ws(frame.payload()).map(ParsedMarketFeed::Depth)
            }
            (MarketFeedProvider::Htx, MarketFeedChannel::Kline) => {
                HtxMarketAdapter::kline_from_ws(frame.payload()).map(ParsedMarketFeed::Kline)
            }
            (MarketFeedProvider::Htx, MarketFeedChannel::Trade) => {
                HtxMarketAdapter::trade_from_ws(frame.payload()).map(ParsedMarketFeed::Trade)
            }
            (MarketFeedProvider::Coinbase, MarketFeedChannel::Ticker) => {
                CoinbaseMarketAdapter::ticker_from_ws(frame.payload()).map(ParsedMarketFeed::Ticker)
            }
            (MarketFeedProvider::Coinbase, MarketFeedChannel::Depth) => {
                CoinbaseMarketAdapter::depth_from_ws(frame.payload()).map(ParsedMarketFeed::Depth)
            }
            (MarketFeedProvider::Coinbase, MarketFeedChannel::Kline) => {
                CoinbaseMarketAdapter::kline_from_ws(frame.payload()).map(ParsedMarketFeed::Kline)
            }
            (MarketFeedProvider::Coinbase, MarketFeedChannel::Trade) => {
                CoinbaseMarketAdapter::trade_from_ws(frame.payload()).map(ParsedMarketFeed::Trade)
            }
            (_, MarketFeedChannel::None) => Err(AppError::Validation(
                "unsupported market feed channel".to_owned(),
            )),
        }
    }

    fn validate_feed_symbols(symbols: &[&str]) -> AppResult<Vec<String>> {
        if symbols.is_empty() {
            return Err(AppError::Validation(
                "market feed symbols are required".to_owned(),
            ));
        }

        symbols
            .iter()
            .map(|symbol| {
                ValidatedMarketSymbol::from_raw(symbol)
                    .map(|symbol| symbol.as_str().to_owned())
                    .map_err(validation_error)
            })
            .collect()
    }

    fn validate_feed_intervals(intervals: &[&str]) -> AppResult<Vec<String>> {
        intervals
            .iter()
            .map(|interval| {
                KlineUpsertKey::new(*interval, Utc::now())
                    .map(|key| key.interval().to_owned())
                    .map_err(validation_error)
            })
            .collect()
    }

    fn bitget_subscriptions(symbols: &[String], intervals: &[String]) -> Vec<String> {
        symbols
            .iter()
            .flat_map(|symbol| {
                let mut messages = vec![
                    json!({"op":"subscribe","args":[{"instType":"SPOT","channel":"ticker","instId":symbol}]}).to_string(),
                    json!({"op":"subscribe","args":[{"instType":"SPOT","channel":"books50","instId":symbol}]}).to_string(),
                    json!({"op":"subscribe","args":[{"instType":"SPOT","channel":"trade","instId":symbol}]}).to_string(),
                ];
                messages.extend(intervals.iter().map(|interval| {
                    json!({"op":"subscribe","args":[{"instType":"SPOT","channel":format!("candle{}", bitget_subscription_interval(interval)),"instId":symbol}]}).to_string()
                }));
                messages
            })
            .collect()
    }

    fn htx_subscriptions(symbols: &[String], intervals: &[String]) -> Vec<String> {
        symbols
            .iter()
            .flat_map(|symbol| {
                let symbol = symbol.to_ascii_lowercase();
                let mut messages = vec![
                    json!({"sub":format!("market.{symbol}.detail")}).to_string(),
                    json!({"sub":format!("market.{symbol}.depth.step0")}).to_string(),
                    json!({"sub":format!("market.{symbol}.trade.detail")}).to_string(),
                ];
                messages.extend(intervals.iter().map(|interval| {
                    json!({"sub":format!("market.{symbol}.kline.{}", htx_subscription_interval(interval))}).to_string()
                }));
                messages
            })
            .collect()
    }

    fn coinbase_subscriptions(symbols: &[String], intervals: &[String]) -> Vec<String> {
        let product_ids: Vec<String> = symbols
            .iter()
            .map(|symbol| coinbase_product_id(symbol))
            .collect();
        let mut messages = vec![
            json!({"type":"subscribe","product_ids": product_ids.clone(), "channel":"ticker"})
                .to_string(),
            json!({"type":"subscribe","product_ids": product_ids.clone(), "channel":"level2"})
                .to_string(),
            json!({"type":"subscribe","product_ids": product_ids.clone(), "channel":"market_trades"})
                .to_string(),
            json!({"type":"subscribe","product_ids": product_ids.clone(), "channel":"heartbeats"})
                .to_string(),
        ];
        if intervals.iter().any(|interval| interval == "5m") {
            messages.push(
                json!({"type":"subscribe","product_ids": product_ids.clone(), "channel":"candles"})
                    .to_string(),
            );
        }
        messages
    }

    fn bitget_subscription_interval(interval: &str) -> &str {
        match interval {
            "1h" => "1H",
            "1d" => "1D",
            value => value,
        }
    }

    fn bitget_rest_interval(interval: &str) -> &str {
        match interval {
            "1m" => "1min",
            "5m" => "5min",
            "15m" => "15min",
            "1d" => "1day",
            value => value,
        }
    }

    fn htx_subscription_interval(interval: &str) -> &str {
        match interval {
            "1m" => "1min",
            "5m" => "5min",
            "15m" => "15min",
            "1h" => "60min",
            "1d" => "1day",
            value => value,
        }
    }

    fn coinbase_rest_granularity(interval: &str) -> (&'static str, i64) {
        match interval {
            "1m" => ("ONE_MINUTE", 60),
            "5m" => ("FIVE_MINUTE", 300),
            "15m" => ("FIFTEEN_MINUTE", 900),
            "1h" => ("ONE_HOUR", 3_600),
            "1d" => ("ONE_DAY", 86_400),
            _ => ("ONE_MINUTE", 60),
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

    impl BitgetMarketAdapter {
        pub fn ticker_from_ws(payload: &str) -> AppResult<MarketTickerSnapshot> {
            let value = parse_json(payload)?;
            let item = first_data_object(&value)?;
            let symbol = bitget_symbol(&value, item)?;
            let last_price = decimal_field(item, &["lastPr", "last"])?;
            let values = ticker_24h_values(
                last_price,
                optional_decimal_field(item, &["high24h", "high"])?,
                optional_decimal_field(item, &["low24h", "low"])?,
                decimal_field(item, &["baseVolume", "baseVol", "vol24h"])?,
                optional_decimal_field(item, &["open24h", "open"])?,
                optional_decimal_field(item, &["change24h", "changeUtc24h"])?,
            );
            MarketTickerSnapshot::with_24h(
                MarketDataProvider::Bitget,
                symbol,
                values,
                millis_field(item, &["ts"])
                    .or_else(|_| millis_field_from_value(&value, &["ts"]))?,
            )
            .map_err(validation_error)
        }

        pub fn depth_from_ws(payload: &str) -> AppResult<MarketDepthSnapshot> {
            let value = parse_json(payload)?;
            let item = first_data_object(&value)?;
            let symbol = bitget_symbol(&value, item)?;
            MarketDepthSnapshot::new(
                MarketDataProvider::Bitget,
                symbol,
                levels(item.get("bids"))?,
                levels(item.get("asks"))?,
                millis_field(item, &["ts"])
                    .or_else(|_| millis_field_from_value(&value, &["ts"]))?,
            )
            .map_err(validation_error)
        }

        pub fn kline_from_ws(payload: &str) -> AppResult<MarketKlineSnapshot> {
            let value = parse_json(payload)?;
            let row = value
                .get("data")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(Value::as_array)
                .ok_or_else(|| AppError::Validation("bitget kline data is required".to_owned()))?;
            let arg = value.get("arg").and_then(Value::as_object);
            let symbol = arg
                .and_then(|arg| arg.get("instId"))
                .and_then(Value::as_str)
                .ok_or_else(|| AppError::Validation("bitget instId is required".to_owned()))?;
            let channel = arg
                .and_then(|arg| arg.get("channel"))
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    AppError::Validation("bitget kline channel is required".to_owned())
                })?;
            let open_time = millis_value(row.first())?;
            MarketKlineSnapshot::new(
                MarketDataProvider::Bitget,
                symbol,
                bitget_interval(channel)?,
                open_time,
                MarketKlineValues {
                    open: decimal_value(row.get(1))?,
                    high: decimal_value(row.get(2))?,
                    low: decimal_value(row.get(3))?,
                    close: decimal_value(row.get(4))?,
                    volume: decimal_value(row.get(5))?,
                },
                millis_field_from_value(&value, &["ts"]).unwrap_or(open_time),
            )
            .map_err(validation_error)
        }

        pub fn trade_from_ws(payload: &str) -> AppResult<MarketTradeTick> {
            let value = parse_json(payload)?;
            let item = first_data_object(&value)?;
            let symbol = bitget_symbol(&value, item)?;
            MarketTradeTick::new(
                MarketDataProvider::Bitget,
                symbol,
                string_field(item, &["tradeId", "id"])?,
                trade_side(&string_field(item, &["side", "direction"])?)?,
                decimal_field(item, &["price", "px"])?,
                decimal_field(item, &["size", "qty", "amount"])?,
                millis_field(item, &["ts"])
                    .or_else(|_| millis_field_from_value(&value, &["ts"]))?,
            )
            .map_err(validation_error)
        }
    }

    impl HtxMarketAdapter {
        pub fn ticker_from_ws(payload: &str) -> AppResult<MarketTickerSnapshot> {
            let value = parse_json(payload)?;
            let tick = required_object(value.get("tick"), "htx tick")?;
            let last_price = decimal_field(tick, &["close", "last"])?;
            let values = ticker_24h_values(
                last_price,
                optional_decimal_field(tick, &["high"])?,
                optional_decimal_field(tick, &["low"])?,
                decimal_field(tick, &["amount", "vol"])?,
                optional_decimal_field(tick, &["open"])?,
                None,
            );
            MarketTickerSnapshot::with_24h(
                MarketDataProvider::Htx,
                htx_symbol(&value)?,
                values,
                millis_field_from_value(&value, &["ts"])
                    .or_else(|_| millis_field(tick, &["ts"]))?,
            )
            .map_err(validation_error)
        }

        pub fn depth_from_ws(payload: &str) -> AppResult<MarketDepthSnapshot> {
            let value = parse_json(payload)?;
            let tick = required_object(value.get("tick"), "htx tick")?;
            MarketDepthSnapshot::new(
                MarketDataProvider::Htx,
                htx_symbol(&value)?,
                levels(tick.get("bids"))?,
                levels(tick.get("asks"))?,
                millis_field(tick, &["ts"])
                    .or_else(|_| millis_field_from_value(&value, &["ts"]))?,
            )
            .map_err(validation_error)
        }

        pub fn kline_from_ws(payload: &str) -> AppResult<MarketKlineSnapshot> {
            let value = parse_json(payload)?;
            let tick = required_object(value.get("tick"), "htx tick")?;
            let interval = htx_interval(
                value
                    .get("ch")
                    .and_then(Value::as_str)
                    .ok_or_else(|| AppError::Validation("htx channel is required".to_owned()))?,
            )?;
            let open_time = seconds_field(tick, &["id"])?;
            MarketKlineSnapshot::new(
                MarketDataProvider::Htx,
                htx_symbol(&value)?,
                interval,
                open_time,
                MarketKlineValues {
                    open: decimal_field(tick, &["open"])?,
                    high: decimal_field(tick, &["high"])?,
                    low: decimal_field(tick, &["low"])?,
                    close: decimal_field(tick, &["close"])?,
                    volume: decimal_field(tick, &["amount", "vol"])?,
                },
                millis_field_from_value(&value, &["ts"]).unwrap_or(open_time),
            )
            .map_err(validation_error)
        }

        pub fn trade_from_ws(payload: &str) -> AppResult<MarketTradeTick> {
            let value = parse_json(payload)?;
            let tick = required_object(value.get("tick"), "htx tick")?;
            let item = tick
                .get("data")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(Value::as_object)
                .ok_or_else(|| AppError::Validation("htx trade data is required".to_owned()))?;
            MarketTradeTick::new(
                MarketDataProvider::Htx,
                htx_symbol(&value)?,
                string_field(item, &["id", "tradeId"])?,
                trade_side(&string_field(item, &["direction", "side"])?)?,
                decimal_field(item, &["price"])?,
                decimal_field(item, &["amount", "quantity"])?,
                millis_field(item, &["ts"])
                    .or_else(|_| millis_field_from_value(&value, &["ts"]))?,
            )
            .map_err(validation_error)
        }
    }

    impl CoinbaseMarketAdapter {
        pub fn ticker_from_ws(payload: &str) -> AppResult<MarketTickerSnapshot> {
            let value = parse_json(payload)?;
            let item = coinbase_first_collection_object(&value, "tickers", "coinbase ticker")?;
            let symbol = coinbase_symbol_from_item(item)?;
            let last_price = decimal_field(item, &["price"])?;
            let price_change_percent_24h = coinbase_optional_percent_field(
                item,
                &["price_percent_chg_24_h", "price_percentage_change_24h"],
            )?
            .unwrap_or_else(|| BigDecimal::from(0));
            let values = MarketTickerValues::new(
                last_price.clone(),
                optional_decimal_field(item, &["high_24_h", "high_24h"])?
                    .unwrap_or_else(|| last_price.clone()),
                optional_decimal_field(item, &["low_24_h", "low_24h"])?
                    .unwrap_or_else(|| last_price.clone()),
                decimal_field(item, &["volume_24_h", "volume_24h"])?,
                BigDecimal::from(0),
                price_change_percent_24h,
            );
            MarketTickerSnapshot::with_24h(
                MarketDataProvider::Coinbase,
                &symbol,
                values,
                coinbase_observed_at(&value, item)?,
            )
            .map_err(validation_error)
        }

        pub fn depth_from_ws(payload: &str) -> AppResult<MarketDepthSnapshot> {
            let value = parse_json(payload)?;
            let event = coinbase_first_event_object(&value)?;
            let updates = required_array(event.get("updates"), "coinbase level2 updates")?;
            let symbol = event
                .get("product_id")
                .and_then(Value::as_str)
                .or_else(|| {
                    updates
                        .first()
                        .and_then(|update| update.get("product_id"))
                        .and_then(Value::as_str)
                })
                .map(coinbase_symbol_from_product_id)
                .ok_or_else(|| {
                    AppError::Validation("coinbase product_id is required".to_owned())
                })?;
            let first_update = updates.first().and_then(Value::as_object);
            MarketDepthSnapshot::new(
                MarketDataProvider::Coinbase,
                &symbol,
                coinbase_depth_levels(updates, true)?,
                coinbase_depth_levels(updates, false)?,
                first_update
                    .map(|item| coinbase_observed_at(&value, item))
                    .transpose()?
                    .unwrap_or_else(Utc::now),
            )
            .map_err(validation_error)
        }

        pub fn kline_from_ws(payload: &str) -> AppResult<MarketKlineSnapshot> {
            let value = parse_json(payload)?;
            let candle = coinbase_first_collection_object(&value, "candles", "coinbase candle")?;
            let symbol = coinbase_symbol_from_item(candle)?;
            let open_time = seconds_field(candle, &["start"])?;
            MarketKlineSnapshot::new(
                MarketDataProvider::Coinbase,
                &symbol,
                coinbase_candle_interval(candle),
                open_time,
                MarketKlineValues {
                    open: decimal_field(candle, &["open"])?,
                    high: decimal_field(candle, &["high"])?,
                    low: decimal_field(candle, &["low"])?,
                    close: decimal_field(candle, &["close"])?,
                    volume: decimal_field(candle, &["volume"])?,
                },
                coinbase_observed_at(&value, candle).unwrap_or(open_time),
            )
            .map_err(validation_error)
        }

        pub fn trade_from_ws(payload: &str) -> AppResult<MarketTradeTick> {
            let value = parse_json(payload)?;
            let trade = coinbase_first_collection_object(&value, "trades", "coinbase trade")?;
            let symbol = coinbase_symbol_from_item(trade)?;
            MarketTradeTick::new(
                MarketDataProvider::Coinbase,
                &symbol,
                string_field(trade, &["trade_id", "tradeId"])?,
                trade_side(&string_field(trade, &["side"])?)?,
                decimal_field(trade, &["price"])?,
                decimal_field(trade, &["size", "quantity"])?,
                coinbase_observed_at(&value, trade)?,
            )
            .map_err(validation_error)
        }
    }

    fn market_cache_error(error: MarketCacheError) -> AppError {
        match error {
            MarketCacheError::Redis(error) => AppError::Redis(error),
            MarketCacheError::Json(error) => AppError::Internal(error.to_string()),
            MarketCacheError::Entry(error) => AppError::Validation(error.to_string()),
        }
    }

    fn bitget_rest_ticker_payload(payload: &str, symbol: &str) -> AppResult<String> {
        let value = parse_json(payload)?;
        Ok(json!({
            "arg": {"channel": "ticker", "instId": symbol},
            "data": value.get("data").cloned().unwrap_or(Value::Null),
            "ts": rest_payload_observed_millis(&value),
        })
        .to_string())
    }

    fn bitget_rest_kline_payloads(
        payload: &str,
        symbol: &str,
        interval: &str,
    ) -> AppResult<Vec<String>> {
        let value = parse_json(payload)?;
        let rows = required_array(value.get("data"), "bitget REST kline data")?;
        Ok(rows
            .iter()
            .map(|row| {
                json!({
                    "arg": {"channel": format!("candle{}", bitget_subscription_interval(interval)), "instId": symbol},
                    "data": [row.clone()],
                    "ts": rest_payload_observed_millis(&value),
                })
                .to_string()
            })
            .collect())
    }

    fn htx_rest_ticker_payload(payload: &str, symbol: &str) -> AppResult<String> {
        let value = parse_json(payload)?;
        let tick = value.get("tick").cloned().unwrap_or(Value::Null);
        Ok(json!({
            "ch": format!("market.{}.detail", symbol.to_ascii_lowercase()),
            "tick": tick,
            "ts": rest_payload_observed_millis(&value),
        })
        .to_string())
    }

    fn htx_rest_kline_payloads(
        payload: &str,
        symbol: &str,
        interval: &str,
    ) -> AppResult<Vec<String>> {
        let value = parse_json(payload)?;
        let rows = required_array(value.get("data"), "htx REST kline data")?;
        Ok(rows
            .iter()
            .map(|row| {
                json!({
                    "ch": format!("market.{}.kline.{}", symbol.to_ascii_lowercase(), htx_subscription_interval(interval)),
                    "tick": row.clone(),
                    "ts": rest_payload_observed_millis(&value),
                })
                .to_string()
            })
            .collect())
    }

    fn coinbase_rest_ticker_payload(payload: &str, symbol: &str) -> AppResult<String> {
        let value = parse_json(payload)?;
        let product_id = value
            .get("product_id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| coinbase_product_id(symbol));
        let mut ticker = value
            .as_object()
            .cloned()
            .ok_or_else(|| AppError::Validation("coinbase REST product is required".to_owned()))?;
        ticker.insert("product_id".to_owned(), Value::String(product_id));
        Ok(json!({
            "channel": "ticker",
            "timestamp": Utc::now().to_rfc3339(),
            "events": [{
                "type": "snapshot",
                "tickers": [Value::Object(ticker)]
            }]
        })
        .to_string())
    }

    fn coinbase_rest_kline_payloads(
        payload: &str,
        symbol: &str,
        interval: &str,
    ) -> AppResult<Vec<String>> {
        let value = parse_json(payload)?;
        let rows = required_array(value.get("candles"), "coinbase REST candle data")?;
        let product_id = coinbase_product_id(symbol);
        Ok(rows
            .iter()
            .map(|row| {
                let mut candle = row.clone();
                if let Some(object) = candle.as_object_mut() {
                    object.insert("product_id".to_owned(), Value::String(product_id.clone()));
                    object.insert("interval".to_owned(), Value::String(interval.to_owned()));
                }
                json!({
                    "channel": "candles",
                    "timestamp": Utc::now().to_rfc3339(),
                    "events": [{
                        "type": "snapshot",
                        "candles": [candle],
                    }]
                })
                .to_string()
            })
            .collect())
    }

    fn coinbase_first_event_object(value: &Value) -> AppResult<&serde_json::Map<String, Value>> {
        value
            .get("events")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(Value::as_object)
            .ok_or_else(|| AppError::Validation("coinbase event is required".to_owned()))
    }

    fn coinbase_first_collection_object<'a>(
        value: &'a Value,
        collection: &str,
        name: &str,
    ) -> AppResult<&'a serde_json::Map<String, Value>> {
        value
            .get("events")
            .and_then(Value::as_array)
            .and_then(|events| {
                events
                    .iter()
                    .filter_map(Value::as_object)
                    .find_map(|event| event.get(collection).and_then(Value::as_array))
            })
            .and_then(|items| items.first())
            .and_then(Value::as_object)
            .ok_or_else(|| AppError::Validation(format!("{name} is required")))
    }

    fn coinbase_symbol_from_item(item: &serde_json::Map<String, Value>) -> AppResult<String> {
        item.get("product_id")
            .and_then(Value::as_str)
            .map(coinbase_symbol_from_product_id)
            .ok_or_else(|| AppError::Validation("coinbase product_id is required".to_owned()))
    }

    fn coinbase_product_id(symbol: &str) -> String {
        let normalized = sanitize_symbol(symbol);
        const QUOTES: &[&str] = &[
            "USDT", "USDC", "USD", "EUR", "GBP", "BTC", "ETH", "SOL", "DAI",
        ];
        for quote in QUOTES {
            if normalized.len() > quote.len() && normalized.ends_with(quote) {
                let base = &normalized[..normalized.len() - quote.len()];
                return format!("{base}-{quote}");
            }
        }
        normalized
    }

    fn coinbase_symbol_from_product_id(product_id: &str) -> String {
        product_id.replace('-', "").to_ascii_uppercase()
    }

    fn coinbase_candle_interval(candle: &serde_json::Map<String, Value>) -> &str {
        match candle
            .get("interval")
            .and_then(Value::as_str)
            .or_else(|| candle.get("granularity").and_then(Value::as_str))
        {
            Some("ONE_MINUTE") => "1m",
            Some("FIVE_MINUTE") => "5m",
            Some("FIFTEEN_MINUTE") => "15m",
            Some("ONE_HOUR") => "1h",
            Some("ONE_DAY") => "1d",
            Some(value @ ("1m" | "5m" | "15m" | "1h" | "1d")) => value,
            _ => "5m",
        }
    }

    fn coinbase_depth_levels(updates: &[Value], bids: bool) -> AppResult<Vec<MarketDepthLevel>> {
        updates
            .iter()
            .filter_map(Value::as_object)
            .filter(|item| {
                let side = item.get("side").and_then(Value::as_str).unwrap_or_default();
                if bids {
                    side.eq_ignore_ascii_case("bid") || side.eq_ignore_ascii_case("buy")
                } else {
                    side.eq_ignore_ascii_case("offer")
                        || side.eq_ignore_ascii_case("ask")
                        || side.eq_ignore_ascii_case("sell")
                }
            })
            .map(|item| {
                Ok(MarketDepthLevel::new(
                    decimal_field(item, &["price_level", "price"])?,
                    decimal_field(item, &["new_quantity", "quantity", "size"])?,
                ))
            })
            .collect()
    }

    fn coinbase_observed_at(
        value: &Value,
        item: &serde_json::Map<String, Value>,
    ) -> AppResult<DateTime<Utc>> {
        coinbase_optional_rfc3339_field(item, &["time", "event_time"])
            .or_else(|| value.get("timestamp").and_then(coinbase_rfc3339_value))
            .map(Ok)
            .unwrap_or_else(|| millis_field(item, &["ts", "time"]))
    }

    fn coinbase_optional_rfc3339_field(
        item: &serde_json::Map<String, Value>,
        names: &[&str],
    ) -> Option<DateTime<Utc>> {
        names
            .iter()
            .find_map(|name| item.get(*name))
            .and_then(coinbase_rfc3339_value)
    }

    fn coinbase_rfc3339_value(value: &Value) -> Option<DateTime<Utc>> {
        value_as_string(value)
            .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())
            .map(|value| value.with_timezone(&Utc))
    }

    fn coinbase_optional_percent_field(
        item: &serde_json::Map<String, Value>,
        names: &[&str],
    ) -> AppResult<Option<BigDecimal>> {
        names
            .iter()
            .find_map(|name| item.get(*name))
            .map(|value| {
                let value = value_as_string(value).ok_or_else(|| {
                    AppError::Validation("coinbase percent value is required".to_owned())
                })?;
                BigDecimal::from_str(value.trim_end_matches('%')).map_err(|error| {
                    AppError::Validation(format!("coinbase percent value is invalid: {error}"))
                })
            })
            .transpose()
    }

    fn rest_payload_observed_millis(value: &Value) -> i64 {
        value
            .get("ts")
            .and_then(value_as_i64)
            .unwrap_or_else(|| Utc::now().timestamp_millis())
    }

    fn parse_json(payload: &str) -> AppResult<Value> {
        serde_json::from_str(payload)
            .map_err(|error| AppError::Validation(format!("invalid market payload json: {error}")))
    }

    fn first_data_object(value: &Value) -> AppResult<&serde_json::Map<String, Value>> {
        value
            .get("data")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(Value::as_object)
            .ok_or_else(|| AppError::Validation("market data item is required".to_owned()))
    }

    fn required_object<'a>(
        value: Option<&'a Value>,
        name: &str,
    ) -> AppResult<&'a serde_json::Map<String, Value>> {
        value
            .and_then(Value::as_object)
            .ok_or_else(|| AppError::Validation(format!("{name} is required")))
    }

    fn required_array<'a>(value: Option<&'a Value>, name: &str) -> AppResult<&'a Vec<Value>> {
        value
            .and_then(Value::as_array)
            .ok_or_else(|| AppError::Validation(format!("{name} is required")))
    }

    fn bitget_symbol<'a>(
        value: &'a Value,
        item: &'a serde_json::Map<String, Value>,
    ) -> AppResult<&'a str> {
        item.get("instId")
            .and_then(Value::as_str)
            .or_else(|| {
                value
                    .get("arg")
                    .and_then(|arg| arg.get("instId"))
                    .and_then(Value::as_str)
            })
            .ok_or_else(|| AppError::Validation("bitget instId is required".to_owned()))
    }

    fn htx_symbol(value: &Value) -> AppResult<&str> {
        value
            .get("ch")
            .and_then(Value::as_str)
            .and_then(|channel| channel.split('.').nth(1))
            .ok_or_else(|| AppError::Validation("htx channel symbol is required".to_owned()))
    }

    fn bitget_interval(channel: &str) -> AppResult<&str> {
        match channel.strip_prefix("candle").unwrap_or(channel) {
            "1m" => Ok("1m"),
            "5m" => Ok("5m"),
            "15m" => Ok("15m"),
            "1H" | "1h" => Ok("1h"),
            "1D" | "1d" => Ok("1d"),
            _ => Err(AppError::Validation(
                "bitget kline interval is invalid".to_owned(),
            )),
        }
    }

    fn htx_interval(channel: &str) -> AppResult<&str> {
        match channel.rsplit('.').next().unwrap_or_default() {
            "1min" => Ok("1m"),
            "5min" => Ok("5m"),
            "15min" => Ok("15m"),
            "60min" | "1hour" => Ok("1h"),
            "1day" => Ok("1d"),
            _ => Err(AppError::Validation(
                "htx kline interval is invalid".to_owned(),
            )),
        }
    }

    fn provider_name(provider: MarketDataProvider) -> &'static str {
        match provider {
            MarketDataProvider::Bitget => "bitget",
            MarketDataProvider::Htx => "htx",
            MarketDataProvider::Strategy => "strategy",
            MarketDataProvider::Coinbase => "coinbase",
        }
    }

    fn market_feed_payload_hash(payload: &Value) -> String {
        let mut hash = 0xcbf29ce484222325_u64;
        for byte in payload.to_string().as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        format!("{hash:016x}")
    }

    fn levels(value: Option<&Value>) -> AppResult<Vec<MarketDepthLevel>> {
        value
            .and_then(Value::as_array)
            .ok_or_else(|| AppError::Validation("depth levels are required".to_owned()))?
            .iter()
            .map(|level| {
                let values = level.as_array().ok_or_else(|| {
                    AppError::Validation("depth level must be an array".to_owned())
                })?;
                Ok(MarketDepthLevel::new(
                    decimal_value(values.first())?,
                    decimal_value(values.get(1))?,
                ))
            })
            .collect()
    }

    fn string_field(item: &serde_json::Map<String, Value>, names: &[&str]) -> AppResult<String> {
        names
            .iter()
            .find_map(|name| item.get(*name))
            .and_then(value_as_string)
            .ok_or_else(|| AppError::Validation(format!("market field {} is required", names[0])))
    }

    fn decimal_field(
        item: &serde_json::Map<String, Value>,
        names: &[&str],
    ) -> AppResult<BigDecimal> {
        names
            .iter()
            .find_map(|name| item.get(*name))
            .map(|value| decimal_value(Some(value)))
            .transpose()?
            .ok_or_else(|| AppError::Validation(format!("market decimal {} is required", names[0])))
    }

    fn optional_decimal_field(
        item: &serde_json::Map<String, Value>,
        names: &[&str],
    ) -> AppResult<Option<BigDecimal>> {
        names
            .iter()
            .find_map(|name| item.get(*name))
            .map(|value| decimal_value(Some(value)))
            .transpose()
    }

    fn ticker_24h_values(
        last_price: BigDecimal,
        high_24h: Option<BigDecimal>,
        low_24h: Option<BigDecimal>,
        volume_24h: BigDecimal,
        open_24h: Option<BigDecimal>,
        change_ratio_24h: Option<BigDecimal>,
    ) -> MarketTickerValues {
        let high_24h = high_24h.unwrap_or_else(|| last_price.clone());
        let low_24h = low_24h.unwrap_or_else(|| last_price.clone());
        let price_change_24h = open_24h
            .as_ref()
            .map(|open| last_price.clone() - open.clone())
            .unwrap_or_else(|| BigDecimal::from(0));
        let price_change_percent_24h = change_ratio_24h
            .map(|change| change * BigDecimal::from(100))
            .unwrap_or_else(|| {
                let Some(open) = open_24h else {
                    return BigDecimal::from(0);
                };
                if open == 0 {
                    return BigDecimal::from(0);
                }
                price_change_24h.clone() / open * BigDecimal::from(100)
            });

        MarketTickerValues::new(
            last_price,
            high_24h,
            low_24h,
            volume_24h,
            price_change_24h,
            price_change_percent_24h,
        )
    }

    fn millis_field(
        item: &serde_json::Map<String, Value>,
        names: &[&str],
    ) -> AppResult<DateTime<Utc>> {
        names
            .iter()
            .find_map(|name| item.get(*name))
            .map(|value| millis_value(Some(value)))
            .transpose()?
            .ok_or_else(|| {
                AppError::Validation(format!("market timestamp {} is required", names[0]))
            })
    }

    fn millis_field_from_value(value: &Value, names: &[&str]) -> AppResult<DateTime<Utc>> {
        let item = value
            .as_object()
            .ok_or_else(|| AppError::Validation("market payload object is required".to_owned()))?;
        millis_field(item, names)
    }

    fn seconds_field(
        item: &serde_json::Map<String, Value>,
        names: &[&str],
    ) -> AppResult<DateTime<Utc>> {
        names
            .iter()
            .find_map(|name| item.get(*name))
            .and_then(value_as_i64)
            .and_then(|seconds| DateTime::<Utc>::from_timestamp(seconds, 0))
            .ok_or_else(|| {
                AppError::Validation(format!("market timestamp {} is invalid", names[0]))
            })
    }

    fn decimal_value(value: Option<&Value>) -> AppResult<BigDecimal> {
        value
            .and_then(value_as_string)
            .ok_or_else(|| AppError::Validation("market decimal value is required".to_owned()))
            .and_then(|value| {
                BigDecimal::from_str(&value).map_err(|error| {
                    AppError::Validation(format!("market decimal is invalid: {error}"))
                })
            })
    }

    fn millis_value(value: Option<&Value>) -> AppResult<DateTime<Utc>> {
        value
            .and_then(value_as_i64)
            .and_then(DateTime::<Utc>::from_timestamp_millis)
            .ok_or_else(|| AppError::Validation("market timestamp millis is invalid".to_owned()))
    }

    fn value_as_string(value: &Value) -> Option<String> {
        match value {
            Value::String(value) => Some(value.clone()),
            Value::Number(number) => Some(number.to_string()),
            _ => None,
        }
    }

    fn value_as_i64(value: &Value) -> Option<i64> {
        match value {
            Value::Number(number) => number.as_i64(),
            Value::String(value) => value.parse::<i64>().ok(),
            _ => None,
        }
    }

    fn trade_side(value: &str) -> AppResult<MarketTradeSide> {
        if value.eq_ignore_ascii_case("buy") || value.eq_ignore_ascii_case("bid") {
            Ok(MarketTradeSide::Buy)
        } else if value.eq_ignore_ascii_case("sell") || value.eq_ignore_ascii_case("ask") {
            Ok(MarketTradeSide::Sell)
        } else {
            Err(AppError::Validation(
                "market trade side is invalid".to_owned(),
            ))
        }
    }

    fn validation_error(error: impl ToString) -> AppError {
        AppError::Validation(error.to_string())
    }
}
