//! 行情缓存基础设施。
//!
//! Redis 中的 ticker、depth 与最新 K 线是实时消费方读取的权威快照；本模块统一 DTO、
//! key 命名和写入校验，不负责 provider 解析、业务价格决策或历史 K 线查询。
//!
//! key 命名固定为三种模式：`market:ticker:<SYMBOL>`、`market:depth:<SYMBOL>`、`market:kline:<SYMBOL>:<INTERVAL>`，
//! 交易对一律先经 `sanitize_symbol` 规范化，写入方与下单、结算、强平等读取方必须共用本模块的生成函数。
//! 所有 key 都不设 TTL，行情靠持续覆盖保持新鲜，因此消费端只能依据载荷里的 `observed_at` 判断是否陈旧。
//! ticker 与 K 线的覆盖走 Lua 脚本做原子防倒退：ticker 比较 `observed_at`，K 线先比 `open_time` 再比 `observed_at`，
//! 后者的时序另存在伴随 key `market:kline-sequence:<SYMBOL>:<INTERVAL>`，以免改动对外 JSON 合同。
//! ticker 在时间相同且序列化载荷逐字节相同时返回 `ReplayedIdentical`，
//! 仅用于修复先写 Redis 后写 MySQL 失败的归档；同时间不同载荷和更旧载荷仍返回 `RejectedStale`。
//! 被判定为陈旧的写入不是错误，调用方必须据此中止广播、撮合和检查点推进等派生副作用。
//! depth 没有防倒退保护，采用直接覆盖，因为盘口本身就是可丢弃的瞬时数据。

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
    /// Redis key 在构造时就由规范化交易对固定下来，外部载荷无法指定写入位置，避免行情被写进任意键。
    /// 结构体的序列化结果就是 Redis 中的 JSON 合同，字段名和 `observed_at` 的毫秒时间戳格式必须保持稳定，
    /// 因为 Lua 防倒退脚本要从这段 JSON 里读回 `observed_at`，前端与撮合侧也按同一格式解析。
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

    /// 返回构造时规范化过的交易对，写入路径会用它重新推导 key，不会直接信任 DTO 里缓存的 `redis_key`。
    /// 该字符串同时是 JSON 载荷中的 `symbol` 字段，前端据此匹配订阅的行情。
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// 返回将写入缓存的最新成交价，精度沿用领域快照，序列化时按十进制输出而非浮点。
    /// 这是下单校验、限价单触发和强平判断从 Redis 读到的价格，任何精度损失都会直接传导到资金计算。
    pub fn last_price(&self) -> &BigDecimal {
        &self.last_price
    }

    /// 返回写入缓存的 24 小时最高价，仅供行情展示，不参与任何资金或风控判断。
    /// provider 缺该字段时上游已回填为最新价，因此这里不会出现 0 值占位。
    pub fn high_24h(&self) -> &BigDecimal {
        &self.high_24h
    }

    /// 返回写入缓存的 24 小时最低价，与最高价成对用于前端行情区间展示。
    /// 缓存层不重算该值，也不会在覆盖写入时与旧快照做区间合并，每次都是整段替换。
    pub fn low_24h(&self) -> &BigDecimal {
        &self.low_24h
    }

    /// 返回写入缓存的 24 小时成交量，口径与 provider 的基础资产成交量字段一致。
    /// 覆盖式写入意味着它总是某一时刻的滚动值，不能跨快照累加得到区间成交量。
    pub fn volume_24h(&self) -> &BigDecimal {
        &self.volume_24h
    }

    /// 返回写入缓存的 24 小时涨跌额，单位与价格相同，负值代表下跌。
    /// 该值由 provider 适配器算好后原样透传，缓存层不会依据高低价重新推导。
    pub fn price_change_24h(&self) -> &BigDecimal {
        &self.price_change_24h
    }

    /// 返回写入缓存的 24 小时涨跌幅，已是百分数形式，前端直接追加百分号即可展示。
    /// 缓存层不做四舍五入，小数位数完全取决于上游计算结果。
    pub fn price_change_percent_24h(&self) -> &BigDecimal {
        &self.price_change_percent_24h
    }

    /// 返回 provider 侧的观察时间，它既序列化进 JSON 供消费端判断陈旧，也作为原子写入脚本的比较基准。
    /// 时间相等时只有完整 JSON 载荷逐字节相同才会被标记为可修复回放，任何字段差异都拒写。
    pub fn observed_at(&self) -> DateTime<Utc> {
        self.observed_at
    }

    /// 返回该 ticker DTO 对应的统一 Redis key，形如 `market:ticker:BTCUSDT`，不执行缓存读取或写入。
    /// 它在构造时生成，主要供测试与日志核对；真实写入会依据 `symbol` 重新生成一次，两者结果必然一致。
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
    /// 构造盘口缓存 DTO，规范化交易对并生成 `market:depth:<SYMBOL>` key，档位顺序和观察时间保持不变。
    /// 构造过程不访问 Redis，也不重新排序、合并或过滤盘口档位，交叉盘口与增量档位都会原样进入缓存。
    /// depth 的写入是直接覆盖，没有 ticker 与 K 线那样的原子防倒退，乱序到达的旧盘口有可能盖掉新盘口。
    /// 这是有意的取舍：盘口是高频且可丢弃的瞬时数据，消费端本就应该按 `observed_at` 自行判断可用性。
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

    /// 将领域盘口快照复制为缓存 DTO，买卖档位整段克隆，顺序与 provider 观察时间都保持原样。
    /// 交易对会重新走一次规范化校验，因此理论上不会失败，但仍以 `Result` 暴露以免绕过格式约束。
    pub fn from_snapshot(snapshot: &MarketDepthSnapshot) -> Result<Self, MarketSymbolError> {
        Self::new(
            snapshot.symbol(),
            snapshot.bids().to_vec(),
            snapshot.asks().to_vec(),
            snapshot.observed_at(),
        )
    }

    /// 返回规范化交易对，写入时会用它重新推导 depth key，确保盘口只能落到本交易对的固定位置。
    /// 它同时是 JSON 载荷里的 `symbol` 字段，供订阅同一交易对的客户端匹配。
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// 返回缓存载荷原有顺序的买盘档位，序列化后原样进入 Redis，未按价格排序也未截断档数。
    /// 消费端要取买一价必须自行求最大值，不能默认首档就是最优买价。
    pub fn bids(&self) -> &[MarketDepthLevel] {
        &self.bids
    }

    /// 返回缓存载荷原有顺序的卖盘档位；行情摄取正是从这份数据取最小价作为现货限价单触发价候选。
    /// 档位数量由 provider 推送决定，本层既不补齐也不裁剪，空数组会照常写入缓存。
    pub fn asks(&self) -> &[MarketDepthLevel] {
        &self.asks
    }

    /// 返回 provider 侧的盘口观察时间，序列化为毫秒时间戳写入缓存。
    /// depth 写入不比较这个时间，它纯粹是给消费端判断盘口是否过时用的，不承担防倒退职责。
    pub fn observed_at(&self) -> DateTime<Utc> {
        self.observed_at
    }

    /// 返回该盘口 DTO 的统一 Redis key，形如 `market:depth:BTCUSDT`，不触发缓存 I/O。
    /// 与 ticker 一样，实际写入会依据 `symbol` 重新生成，本字段只是构造时的留存副本。
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
    /// 该字段标注了 `#[serde(skip)]`，因此对外 JSON 合同保持不变，时序改由伴随 key 单独保存。
    /// 交易对先规范化，周期再经 [`KlineUpsertKey`] 校验，两者共同决定 `market:kline:<SYMBOL>:<INTERVAL>` 这个 key。
    /// 该时间必须取领域快照的真实观察时间；同槽相等或更旧时间都会拒绝，避免重复广播与 forming 值倒退。
    /// 传入本机时间会让每次推送都显得更新，防倒退随之失效，同分钟内的旧 owner 就能覆盖新数据。
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

    /// 返回规范化交易对，写入时与周期一起重新推导 K 线 key，同时也是伴随时序 key 的组成部分。
    /// 每个交易对与周期占用一个独立缓存槽，缓存中只保留该槽最新的那一根蜡烛。
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// 返回已校验的周期文本，它是 Redis key 的最后一段，决定这份 DTO 覆盖哪一个缓存槽。
    /// 取值保持平台内部写法，未转换成 provider 专有格式，跨周期不会互相覆盖。
    pub fn interval(&self) -> &str {
        &self.interval
    }

    /// 返回蜡烛所属时间槽的开盘时间，序列化为毫秒时间戳进入 JSON，是防倒退比较的第一优先级。
    /// 脚本先比较它，开盘时间更早的推送会被直接拒绝，从而阻止跨分钟的回退覆盖。
    pub fn open_time(&self) -> DateTime<Utc> {
        self.open_time
    }

    /// 返回该时间槽的开盘价，写入 JSON 后由前端 K 线图直接使用。
    /// 蜡烛形成过程中反复覆盖时这个值通常保持不变，变化的主要是最高、最低、收盘价与成交量。
    pub fn open(&self) -> &BigDecimal {
        &self.open
    }

    /// 返回该时间槽的最高价，每次覆盖写入都是整段替换，缓存层不会与旧值取较大者。
    /// 因此正确性依赖 provider 自身给出的累计极值，本层不做修正。
    pub fn high(&self) -> &BigDecimal {
        &self.high
    }

    /// 返回该时间槽的最低价，同样按整段替换写入，缓存层不与旧值取较小者。
    /// 若上游给出异常值，缓存会原样保留，需要在 provider 解析阶段拦截。
    pub fn low(&self) -> &BigDecimal {
        &self.low
    }

    /// 返回该时间槽的收盘价；蜡烛未收线时等于当前最新成交价，会随每次覆盖而变化。
    /// 这是缓存中最频繁变动的字段，也是防倒退保护要重点避免回退的值。
    pub fn close(&self) -> &BigDecimal {
        &self.close
    }

    /// 返回该时间槽的累计成交量，写入 JSON 供图表展示成交柱。
    /// 与价格字段一样整段覆盖，跨快照不做累加，读到的永远是上游给出的当前累计值。
    pub fn volume(&self) -> &BigDecimal {
        &self.volume
    }

    /// 返回该 K 线 DTO 的 symbol+interval Redis key，形如 `market:kline:BTCUSDT:1m`，不触发缓存 I/O。
    /// 与之配套的时序伴随键由写入路径按同样的交易对和周期单独生成，并不保存在本 DTO 里。
    /// 因此仅凭这个字段无法完成防倒退比较，判断新旧必须走缓存的原子写入接口。
    pub fn redis_key(&self) -> &str {
        &self.redis_key
    }

    /// 返回仅供 Redis CAS 比较的观察时间；该字段跳过 JSON 序列化以保持现有消费者合同。
    /// 它被单独写进伴随 key，与 `open_time` 组成 `开盘时间:观察时间` 形式的时序串。
    /// 同一开盘时间下，观察时间相等或更早的推送会被判为陈旧，这正是拦截同分钟旧 owner 的关键。
    pub fn observed_at(&self) -> DateTime<Utc> {
        self.observed_at
    }

    /// 将领域 K 线快照映射为缓存 DTO，保留周期、开盘时间、OHLC 和成交量精度。
    /// 与直接调用 [`Self::new`] 的关键差别是这里透传快照真实的 `observed_at`，而不是用开盘时间顶替，
    /// 因此只有走本入口的写入才具备同一时间槽内的防倒退能力。
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

/// 生成 `market:depth:<SYMBOL>` 深度快照 key，交易对同样先按稳定规则规范化，写入与读取共用本入口。
/// 只负责命名，不验证快照新鲜度或内容，也不保证该 key 当前一定存在。
pub fn market_depth_redis_key(symbol: &str) -> String {
    format!("market:depth:{}", sanitize_symbol(symbol))
}

/// 生成 `market:kline:<SYMBOL>:<INTERVAL>` 形式的最新 K 线 key，每个交易对与周期组合独占一个缓存槽。
/// 交易对会被规范化，但周期原样拼接，调用前必须已通过 Kline 规则校验，否则会造出一个永远无人读取的孤儿 key。
pub fn market_kline_redis_key(symbol: &str, interval: &str) -> String {
    format!("market:kline:{}:{}", sanitize_symbol(symbol), interval)
}

/// Redis 权威快照写入结果；`ReplayedIdentical` 表示 ticker 时间与载荷完全相同，可继续修复归档。
/// `RejectedStale` 表示缓存保持了更新值或同时间载荷不同，调用方必须停止派生副作用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketCacheWriteOutcome {
    Accepted,
    ReplayedIdentical,
    RejectedStale,
}

impl MarketCacheWriteOutcome {
    /// 只有原子脚本实际接受本次快照时返回 true；被拒写者不得触发订单、广播或推进 worker 检查点。
    /// 返回 false 表示缓存里已有时间更新的值，属于并发下的正常结果而非错误，调用方不应重试或告警。
    pub fn is_accepted(self) -> bool {
        matches!(self, Self::Accepted)
    }

    /// 返回 ticker 是否命中同时间、同载荷回放；该分支不重写 Redis，只可用来补齐尚未成功的持久化。
    /// K 线脚本不返回此状态，因此它不会改变既有的 K 线严格递增契约。
    pub fn is_identical_replay(self) -> bool {
        matches!(self, Self::ReplayedIdentical)
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
    local incoming_observed_at = tonumber(ARGV[1])
    if decoded.observed_at > incoming_observed_at then
        return 0
    end
    if decoded.observed_at == incoming_observed_at then
        if current == ARGV[2] then
            return 2
        end
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

    /// 以 Redis Lua 原子比较 `observed_at` 后写入 ticker；同时间且载荷逐字节相同返回 `ReplayedIdentical`，
    /// 使 synthetic 路径能在不重写缓存的情况下补齐 MySQL 归档；同时间不同载荷或较旧实例返回 `RejectedStale`。
    /// 脚本在单条命令内完成读旧值、解析 JSON、比较时间、覆盖四步，消除了先查后写之间的竞态窗口。
    /// 缓存中已有载荷若不是合法 JSON 或缺少数值型 `observed_at`，脚本会直接报错而不是当作空槽覆盖，
    /// 这样被人工污染或格式不兼容的键会明确暴露出来，而不是被静默改写。
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
    /// 脚本按两级顺序判定：开盘时间更早直接拒绝；开盘时间相同再比观察时间，相等或更早同样拒绝。
    /// 首次写入时伴随 key 尚不存在，观察时间无从比较，此时只要开盘时间不倒退就予以接受。
    /// 接受后会在同一次脚本执行里同时更新行情 JSON 与 `<开盘毫秒>:<观察毫秒>` 时序串，两者不会出现半写状态。
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

    /// 把 DTO 序列化成 JSON 后无条件 `SET` 到指定 key，是没有防倒退需求的写入路径的共用底座。
    /// 不设 TTL、不比较旧值、不区分新建与覆盖，因此只适用于 depth 这类可丢弃的瞬时快照。
    /// 序列化失败会在发出命令前返回，此时缓存完全未被触碰；Redis 侧失败则可能留下旧值继续被读取。
    async fn save_json<T: Serialize>(&self, key: &str, entry: &T) -> Result<(), MarketCacheError> {
        use redis::AsyncCommands;

        let payload = serde_json::to_string(entry)?;
        let mut connection = self.manager.clone();
        let _: () = connection.set(key, payload).await?;
        Ok(())
    }
}

/// 生成 `market:kline-sequence:<SYMBOL>:<INTERVAL>` 伴随 key，专门存放 `<开盘毫秒>:<观察毫秒>` 时序串。
/// 之所以另开一个 key，是为了在不改动对外 K 线 JSON 合同的前提下补上同一时间槽内的先后判定依据。
/// 它与行情 key 由同一段 Lua 脚本一起更新，因此保持严格同步；本函数只负责命名，不做任何 Redis 访问。
fn market_kline_sequence_redis_key(symbol: &str, interval: &str) -> String {
    format!(
        "market:kline-sequence:{}:{}",
        sanitize_symbol(symbol),
        interval
    )
}

/// 把 Lua 脚本返回的整数翻译成写入结果：1 表示首次接受，2 表示同时间同载荷回放，其余按陈旧拒写。
/// 采用白名单式判定可避免脚本将来返回新状态码时被误判成可继续执行副作用。
fn cache_write_outcome(accepted: i64) -> MarketCacheWriteOutcome {
    match accepted {
        1 => MarketCacheWriteOutcome::Accepted,
        2 => MarketCacheWriteOutcome::ReplayedIdentical,
        _ => MarketCacheWriteOutcome::RejectedStale,
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
