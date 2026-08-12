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
use redis::Script;
use serde::Serialize;
use std::sync::LazyLock;
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
    #[serde(skip)]
    observed_at: DateTime<Utc>,
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
        Self::with_observed_at(symbol, interval, open_time, values, open_time)
    }

    /// 构造携带内部观察时序的最新 K 线缓存 DTO；`observed_at` 只用于 Redis 原子防倒退，不进入既有消费者 JSON。
    /// 该时间必须取领域快照的真实观察时间；同槽相等或更旧时间都会拒绝，避免重复广播与 forming 值倒退。
    pub fn with_observed_at(
        symbol: &str,
        interval: &str,
        open_time: DateTime<Utc>,
        values: MarketKlineValues,
        observed_at: DateTime<Utc>,
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
            observed_at,
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

    /// 返回仅供 Redis CAS 比较的观察时间；该字段跳过 JSON 序列化以保持现有消费者合同。
    pub fn observed_at(&self) -> DateTime<Utc> {
        self.observed_at
    }

    /// 将领域 K 线快照映射为缓存 DTO，保留周期、开盘时间、OHLC 和成交量精度。
    /// 映射会重新执行交易对与周期校验，实际写入由 [`RedisMarketCache::save_kline`] 完成。
    pub fn from_snapshot(snapshot: &MarketKlineSnapshot) -> Result<Self, MarketCacheEntryError> {
        Self::with_observed_at(
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
            snapshot.observed_at(),
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

/// Redis 权威快照写入结果；`RejectedStale` 表示缓存保持了时间更新的值，调用方必须停止派生副作用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketCacheWriteOutcome {
    Accepted,
    RejectedStale,
}

impl MarketCacheWriteOutcome {
    /// 只有原子脚本实际接受本次快照时返回 true；被拒写者不得触发订单、广播或推进 worker 检查点。
    pub fn is_accepted(self) -> bool {
        matches!(self, Self::Accepted)
    }
}

/// ticker 的时间戳保存在既有 JSON 中；Lua 在单条 Redis 命令内比较并覆盖，消除租约检查到 `SET` 之间的竞态。
static SAVE_TICKER_IF_FRESH_SCRIPT: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"local current = redis.call('GET', KEYS[1])
if current then
    local ok, decoded = pcall(cjson.decode, current)
    if not ok or type(decoded.observed_at) ~= 'number' then
        return redis.error_reply('invalid cached ticker observed_at')
    end
    if decoded.observed_at >= tonumber(ARGV[1]) then
        return 0
    end
end
redis.call('SET', KEYS[1], ARGV[2])
return 1"#,
    )
});

/// K 线对外 JSON 保持原合同，另用伴随时序 key 保存 `(open_time, observed_at)`；单机 Redis Lua 原子更新两者。
static SAVE_KLINE_IF_FRESH_SCRIPT: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"local current_open = nil
local current_observed = nil
local current = redis.call('GET', KEYS[1])
if current then
    local ok, decoded = pcall(cjson.decode, current)
    if not ok or type(decoded.open_time) ~= 'number' then
        return redis.error_reply('invalid cached kline open_time')
    end
    current_open = tonumber(decoded.open_time)
end
local sequence = redis.call('GET', KEYS[2])
if sequence then
    local separator = string.find(sequence, ':', 1, true)
    if not separator then
        return redis.error_reply('invalid cached kline sequence')
    end
    current_open = tonumber(string.sub(sequence, 1, separator - 1))
    current_observed = tonumber(string.sub(sequence, separator + 1))
end
local incoming_open = tonumber(ARGV[1])
local incoming_observed = tonumber(ARGV[2])
if current_open and
   (current_open > incoming_open or
    (current_open == incoming_open and current_observed and current_observed >= incoming_observed)) then
    return 0
end
redis.call('SET', KEYS[1], ARGV[3])
redis.call('SET', KEYS[2], ARGV[1] .. ':' .. ARGV[2])
return 1"#,
    )
});

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

    /// 以 Redis Lua 原子比较 `observed_at` 后写入 ticker；相等或较旧实例返回 `RejectedStale`，不会重复派生副作用。
    /// key 由规范化交易对重建且不设 TTL；拒写不会改变 JSON，调用方必须同步停止订单、广播和检查点副作用。
    pub async fn save_ticker_if_fresh(
        &self,
        entry: MarketTickerCacheEntry,
    ) -> Result<MarketCacheWriteOutcome, MarketCacheError> {
        let symbol =
            ValidatedMarketSymbol::from_raw(entry.symbol()).map_err(MarketCacheEntryError::from)?;
        let key = market_ticker_redis_key(symbol.as_str());
        let payload = serde_json::to_string(&entry)?;
        let mut connection = self.manager.clone();
        let accepted: i64 = SAVE_TICKER_IF_FRESH_SCRIPT
            .key(key)
            .arg(entry.observed_at().timestamp_millis())
            .arg(payload)
            .invoke_async(&mut connection)
            .await?;
        Ok(cache_write_outcome(accepted))
    }

    /// 兼容既有调用者的无返回值 API，但底层仍执行原子防倒退；stale 写入视为成功且保留当前缓存。
    /// 需要控制后续副作用的 synthetic/统一摄取路径必须调用 [`Self::save_ticker_if_fresh`] 读取明确结果。
    pub async fn save_ticker(&self, entry: MarketTickerCacheEntry) -> Result<(), MarketCacheError> {
        self.save_ticker_if_fresh(entry).await.map(|_| ())
    }

    /// 覆盖写入指定交易对的最新盘口 JSON；key 重新由规范化 symbol 生成，不能由外部载荷指定。
    /// Redis 或序列化失败直接返回，旧快照可能继续存在；消费者需依据 `observed_at` 判断新鲜度。
    pub async fn save_depth(&self, entry: MarketDepthCacheEntry) -> Result<(), MarketCacheError> {
        let symbol =
            ValidatedMarketSymbol::from_raw(entry.symbol()).map_err(MarketCacheEntryError::from)?;
        let key = market_depth_redis_key(symbol.as_str());
        self.save_json(&key, &entry).await
    }

    /// 以 `(open_time, observed_at)` 严格递增顺序原子更新最新 K 线 JSON；跨分钟与同分钟形成中快照都不会倒退或重复广播。
    /// 外部 JSON 字段保持不变，内部时序保存在伴随 Redis hash；拒写者必须停止 Mongo、广播及检查点副作用。
    pub async fn save_kline_if_fresh(
        &self,
        entry: MarketKlineCacheEntry,
    ) -> Result<MarketCacheWriteOutcome, MarketCacheError> {
        let symbol =
            ValidatedMarketSymbol::from_raw(entry.symbol()).map_err(MarketCacheEntryError::from)?;
        KlineUpsertKey::new(entry.interval(), entry.open_time())
            .map_err(MarketCacheEntryError::from)?;
        let key = market_kline_redis_key(symbol.as_str(), entry.interval());
        let sequence_key = market_kline_sequence_redis_key(symbol.as_str(), entry.interval());
        let payload = serde_json::to_string(&entry)?;
        let mut connection = self.manager.clone();
        let accepted: i64 = SAVE_KLINE_IF_FRESH_SCRIPT
            .key(key)
            .key(sequence_key)
            .arg(entry.open_time().timestamp_millis())
            .arg(entry.observed_at().timestamp_millis())
            .arg(payload)
            .invoke_async(&mut connection)
            .await?;
        Ok(cache_write_outcome(accepted))
    }

    /// 保留既有无返回值 K 线 API，内部使用原子时序门禁；较旧快照被忽略而不是覆盖最新缓存。
    /// synthetic ingestion 使用 [`Self::save_kline_if_fresh`] 取得拒写结果，以阻断同分钟旧 owner 的后续广播。
    pub async fn save_kline(&self, entry: MarketKlineCacheEntry) -> Result<(), MarketCacheError> {
        self.save_kline_if_fresh(entry).await.map(|_| ())
    }

    async fn save_json<T: Serialize>(&self, key: &str, entry: &T) -> Result<(), MarketCacheError> {
        use redis::AsyncCommands;

        let payload = serde_json::to_string(entry)?;
        let mut connection = self.manager.clone();
        let _: () = connection.set(key, payload).await?;
        Ok(())
    }
}

fn market_kline_sequence_redis_key(symbol: &str, interval: &str) -> String {
    format!(
        "market:kline-sequence:{}:{}",
        sanitize_symbol(symbol),
        interval
    )
}

fn cache_write_outcome(accepted: i64) -> MarketCacheWriteOutcome {
    if accepted == 1 {
        MarketCacheWriteOutcome::Accepted
    } else {
        MarketCacheWriteOutcome::RejectedStale
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
