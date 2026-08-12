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
    /// 用最新价和成交量创建平盘 ticker 缓存 DTO，并从规范化交易对生成固定 Redis key。
    /// 这里只构造序列化数据，不连接 Redis；价格正数与观察时间新鲜度由摄取和消费端分别保证。
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

    /// 用完整 24 小时统计创建 ticker 缓存 DTO，交易对和 key 均按统一市场规则规范化。
    /// 构造阶段不执行 Redis 写入；字段来自服务端行情摄取链，资金用例读取后仍须检查正数与新鲜度。
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

    /// 返回交易对符号。
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// 返回最新成交价。
    pub fn last_price(&self) -> &BigDecimal {
        &self.last_price
    }

    /// 返回二十四小时最高价。
    pub fn high_24h(&self) -> &BigDecimal {
        &self.high_24h
    }

    /// 返回二十四小时最低价。
    pub fn low_24h(&self) -> &BigDecimal {
        &self.low_24h
    }

    /// 返回二十四小时成交量。
    pub fn volume_24h(&self) -> &BigDecimal {
        &self.volume_24h
    }

    /// 返回二十四小时涨跌额。
    pub fn price_change_24h(&self) -> &BigDecimal {
        &self.price_change_24h
    }

    /// 返回二十四小时涨跌幅。
    pub fn price_change_percent_24h(&self) -> &BigDecimal {
        &self.price_change_percent_24h
    }

    /// 返回行情观测时间。
    pub fn observed_at(&self) -> DateTime<Utc> {
        self.observed_at
    }

    /// 返回该 ticker DTO 对应的统一 Redis key，不执行缓存读取或写入。
    pub fn redis_key(&self) -> &str {
        &self.redis_key
    }

    /// 将领域 ticker 快照完整映射为缓存 DTO，保留服务端观察时间与 24 小时统计精度。
    /// 映射只校验交易对格式；实际写入由 [`RedisMarketCache::save_ticker`] 完成。
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
    /// 构造盘口缓存 DTO，规范化交易对并生成 depth Redis key，档位顺序和观察时间保持不变。
    /// 构造过程不访问 Redis，也不重新排序、合并或过滤盘口档位。
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

    /// 将领域盘口快照复制为缓存 DTO，保留买卖盘顺序和 provider 观察时间。
    pub fn from_snapshot(snapshot: &MarketDepthSnapshot) -> Result<Self, MarketSymbolError> {
        Self::new(
            snapshot.symbol(),
            snapshot.bids().to_vec(),
            snapshot.asks().to_vec(),
            snapshot.observed_at(),
        )
    }

    /// 返回交易对符号。
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// 返回缓存载荷原有顺序的买盘档位。
    pub fn bids(&self) -> &[MarketDepthLevel] {
        &self.bids
    }

    /// 返回缓存载荷原有顺序的卖盘档位。
    pub fn asks(&self) -> &[MarketDepthLevel] {
        &self.asks
    }

    /// 返回行情观测时间。
    pub fn observed_at(&self) -> DateTime<Utc> {
        self.observed_at
    }

    /// 返回该盘口 DTO 的统一 Redis key，不触发缓存 I/O。
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
    /// 构造最新 K 线缓存 DTO，规范化交易对、校验周期并生成 symbol+interval Redis key。
    /// OHLC 与成交量原样保留；该步骤不连接 Redis，也不校验蜡烛内部价格关系。
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

    /// 返回交易对符号。
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// 返回 K 线周期。
    pub fn interval(&self) -> &str {
        &self.interval
    }

    /// 返回 K 线开盘时间。
    pub fn open_time(&self) -> DateTime<Utc> {
        self.open_time
    }

    /// 返回 K 线开盘价。
    pub fn open(&self) -> &BigDecimal {
        &self.open
    }

    /// 返回 K 线最高价。
    pub fn high(&self) -> &BigDecimal {
        &self.high
    }

    /// 返回 K 线最低价。
    pub fn low(&self) -> &BigDecimal {
        &self.low
    }

    /// 返回 K 线收盘价。
    pub fn close(&self) -> &BigDecimal {
        &self.close
    }

    /// 返回 K 线成交量。
    pub fn volume(&self) -> &BigDecimal {
        &self.volume
    }

    /// 返回该 K 线 DTO 的 symbol+interval Redis key，不触发缓存 I/O。
    pub fn redis_key(&self) -> &str {
        &self.redis_key
    }

    /// 将领域 K 线快照映射为缓存 DTO，保留周期、开盘时间、OHLC 和成交量精度。
    /// 映射会重新执行交易对与周期校验，实际写入由 [`RedisMarketCache::save_kline`] 完成。
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
    /// 保存可克隆的 Redis 连接管理器，供每次行情写入独立取得异步连接句柄。
    /// 构造时不发送命令；连接或认证错误会在具体 `save_*` 调用时返回。
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

    /// 覆盖写入指定交易对的最新盘口 JSON；key 重新由规范化 symbol 生成，不能由外部载荷指定。
    /// Redis 或序列化失败直接返回，旧快照可能继续存在；消费者需依据 `observed_at` 判断新鲜度。
    pub async fn save_depth(&self, entry: MarketDepthCacheEntry) -> Result<(), MarketCacheError> {
        let symbol =
            ValidatedMarketSymbol::from_raw(entry.symbol()).map_err(MarketCacheEntryError::from)?;
        let key = market_depth_redis_key(symbol.as_str());
        self.save_json(&key, &entry).await
    }

    /// 覆盖写入交易对与周期对应的最新 K 线 JSON，并在发送 Redis 命令前复核 symbol 与周期。
    /// 写入不设置 TTL；失败保留原缓存，由摄取任务的重试与消费者新鲜度检查处理。
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
