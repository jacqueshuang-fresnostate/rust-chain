//! 行情缓存基础设施。
//!
//! Redis 中的 ticker、depth 与最新 K 线是实时消费方读取的权威快照；本模块统一 DTO、
//! key 命名和写入校验，不负责 provider 解析、业务价格决策或历史 K 线查询。

use crate::{
    modules::market::{
        KlineUpsertKey, MarketCacheEntryError, MarketDepthLevel, MarketDepthSnapshot,
        MarketKlineSnapshot, MarketKlineValues, MarketSymbolError, MarketTickerSnapshot,
        MarketTickerValues, ValidatedMarketSymbol, sanitize_symbol,
    },
    time::unix_millis,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::Serialize;
use thiserror::Error;

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

/// 生成全系统统一的 ticker Redis key；交易对先按稳定规则规范化，行情写入与下单/结算/强平读取必须共用该入口。
pub fn market_ticker_redis_key(symbol: &str) -> String {
    format!("market:ticker:{}", sanitize_symbol(symbol))
}

/// 生成深度快照 Redis key；只负责命名，不验证快照新鲜度或内容。
pub fn market_depth_redis_key(symbol: &str) -> String {
    format!("market:depth:{}", sanitize_symbol(symbol))
}

/// 生成指定交易对与周期的 K 线 Redis key；调用前周期必须已通过 Kline 规则校验。
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

    /// 写入经验证的权威 ticker 快照；key 从快照交易对重新生成，避免外部携带任意缓存位置。
    /// 该写入不设置假 TTL，消费者必须依据 observed_at 自行执行价格新鲜度政策。
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
