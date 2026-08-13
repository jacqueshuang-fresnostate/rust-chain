//! market bounded context domain layer.
//!
//! 领域层：放置市场符号、行情快照、K线查询和值对象等不依赖 I/O 的业务规则。
//!
//! 本文件是行情限界上下文的最内层，只做纯内存判定：交易对规范化与白名单、K 线周期白名单与幂等键、
//! ticker/depth/kline/trade 四类快照的字段封装。所有快照都携带 provider 与观察时间，
//! 这两项决定了下游 Redis 防倒退比较和消费端的陈旧判定，因此构造器一律要求调用方传入 provider 侧时间。
//! 这里不访问 Redis、Mongo、MySQL 或任何外部行情源，也不做价格正数、高低价区间、时间对齐等业务合理性校验：
//! 数值语义由各 provider 适配器负责转换，缓存 key 与集合命名由基础设施层负责，本层只保证格式合法与字段不可变。

use crate::time::unix_millis;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
// 行情符号在入库、Redis key 和外部接口之间统一使用去分隔符的大写格式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedMarketSymbol(String);

impl ValidatedMarketSymbol {
    /// 将用户或 provider 输入的交易对去除 `/`、`-`、`_` 后转为大写统一格式。
    /// 空值、超过 32 字符或含非 ASCII 交易对字符时拒绝；本值对象不查询允许列表，也不访问行情源。
    pub fn from_raw(symbol: &str) -> Result<Self, MarketSymbolError> {
        let symbol = symbol.trim();
        let normalized = sanitize_symbol(symbol);
        if normalized.is_empty() {
            return Err(MarketSymbolError::Empty);
        }
        if normalized.len() > 32 || !symbol.chars().all(is_symbol_char) {
            return Err(MarketSymbolError::InvalidFormat);
        }
        Ok(Self(normalized))
    }

    /// 先按 [`Self::from_raw`] 规范化交易对，再与同样规范化的允许列表逐项比较。
    /// 未命中白名单返回 `NotAllowed`；该判断是纯内存规则，不替调用方加载后台交易对配置。
    pub fn from_allowed<'a>(
        symbol: &str,
        allowed_symbols: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self, MarketSymbolError> {
        let normalized = Self::from_raw(symbol)?;
        if allowed_symbols
            .into_iter()
            .any(|allowed| sanitize_symbol(allowed) == normalized.0)
        {
            Ok(normalized)
        } else {
            Err(MarketSymbolError::NotAllowed)
        }
    }

    /// 借出已规范化的交易对文本，可直接用于拼接 Redis key、Mongo 集合名与 provider 订阅参数。
    /// 构造器已保证返回值非空、长度不超过 32、且只含大写 ASCII 字母数字，不含 `/`、`-`、`_` 等分隔符。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MarketSymbolError {
    #[error("market symbol is empty")]
    Empty,
    #[error("market symbol format is invalid")]
    InvalidFormat,
    #[error("market symbol is not whitelisted")]
    NotAllowed,
}

/// 生成 Redis key、provider 订阅和数据库查询共用的交易对格式：仅保留 ASCII 字母数字并转大写。
/// 本函数有意忽略分隔符和其他字符；需要拒绝非法输入时应改用 [`ValidatedMarketSymbol::from_raw`]。
pub fn sanitize_symbol(symbol: &str) -> String {
    symbol
        .trim()
        .chars()
        .filter(|ch| is_symbol_char(*ch) && ch.is_ascii_alphanumeric())
        .flat_map(char::to_uppercase)
        .collect()
}

/// 判定单个字符能否出现在调用方给出的原始交易对里：ASCII 字母数字，或 `/`、`-`、`_` 三种分隔符。
/// 这是格式校验用的白名单，规范化阶段会把三种分隔符一并丢弃，因此本判定比 [`sanitize_symbol`] 的保留集更宽。
fn is_symbol_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '/' | '-' | '_')
}

// K线唯一键只由周期和开盘时间决定，避免重复采集覆盖同一根蜡烛。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KlineUpsertKey {
    interval: String,
    open_time: DateTime<Utc>,
}

impl KlineUpsertKey {
    /// 以周期和开盘时间组成 K 线幂等写入键，仅接受平台支持的 `1m/5m/15m/1h/4h/1d` 周期。
    /// 不校验时间是否对齐周期边界；采集或恢复任务仍需负责生成正确的 `open_time`。
    pub fn new(
        interval: impl Into<String>,
        open_time: DateTime<Utc>,
    ) -> Result<Self, KlineKeyError> {
        let interval = interval.into();
        if matches!(interval.as_str(), "1m" | "5m" | "15m" | "1h" | "4h" | "1d") {
            Ok(Self {
                interval,
                open_time,
            })
        } else {
            Err(KlineKeyError::InvalidInterval)
        }
    }

    /// 借出构造时已通过白名单校验的周期文本，取值必属于 `1m/5m/15m/1h/4h/1d`。
    /// 该字符串会原样出现在 Mongo 文档的 `interval` 字段和 Redis K 线 key 的后缀里，不再二次转换大小写。
    pub fn interval(&self) -> &str {
        &self.interval
    }

    /// 返回这根蜡烛所属时间槽的开盘时间，它与周期共同构成幂等键，决定重放时覆盖历史中的哪一条记录。
    /// 该时间由采集方给出，本类型不保证它已对齐周期边界，未对齐会直接产生一个错位的独立时间槽。
    pub fn open_time(&self) -> DateTime<Utc> {
        self.open_time
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KlineKeyError {
    #[error("kline interval is invalid")]
    InvalidInterval,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KlineQuery {
    pub interval: String,
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
    pub limit: u32,
}

impl KlineQuery {
    /// 构造历史 K 线查询条件，借用 [`KlineUpsertKey`] 的周期白名单做校验，不受支持的周期返回 `InvalidInterval`。
    /// 校验时传入当前时间只是为了复用键构造器，这个时间不会进入查询条件，也不会影响 `start`/`end`。
    /// `limit` 缺省取 100 并夹紧到 1..=100，防止单次拉取压垮 Mongo；起止时间原样保留，可以同时为空表示不限范围。
    /// 本类型不检查 `start` 是否早于 `end`，也不决定排序方向与数据源，这些由查询应用层与 Mongo 适配器负责。
    pub fn new(
        interval: impl Into<String>,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
        limit: Option<u32>,
    ) -> Result<Self, KlineKeyError> {
        let interval = interval.into();
        KlineUpsertKey::new(interval.clone(), Utc::now())?;
        Ok(Self {
            interval,
            start,
            end,
            limit: limit.unwrap_or(100).clamp(1, 100),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketEvent {
    pub symbol: String,
    pub event_type: MarketEventType,
    pub price: Option<BigDecimal>,
    pub volume: Option<BigDecimal>,
    #[serde(with = "unix_millis")]
    pub ts: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketDataProvider {
    Bitget,
    Htx,
    Strategy,
    Coinbase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketTradeSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketTickerSnapshot {
    provider: MarketDataProvider,
    symbol: String,
    last_price: BigDecimal,
    high_24h: BigDecimal,
    low_24h: BigDecimal,
    volume_24h: BigDecimal,
    price_change_24h: BigDecimal,
    price_change_percent_24h: BigDecimal,
    #[serde(with = "unix_millis")]
    observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketTickerValues {
    pub last_price: BigDecimal,
    pub high_24h: BigDecimal,
    pub low_24h: BigDecimal,
    pub volume_24h: BigDecimal,
    pub price_change_24h: BigDecimal,
    pub price_change_percent_24h: BigDecimal,
}

impl MarketTickerValues {
    /// 汇集 provider 已解析出的最新价、24 小时高低价、成交量与涨跌额、涨跌幅，构成 ticker 快照的数值部分。
    /// 六个字段都以 `BigDecimal` 原样保留，不做四舍五入或统一小数位，精度完全取决于上游返回的十进制字符串。
    /// 此构造器不修正负数、不检查最新价是否落在高低价区间内、也不重算涨跌幅与涨跌额的自洽性。
    /// 这些一致性由具体 provider 解析器保证；缺字段时应先用 [`Self::flat`] 或解析层的回填规则补齐再调用本函数。
    pub fn new(
        last_price: BigDecimal,
        high_24h: BigDecimal,
        low_24h: BigDecimal,
        volume_24h: BigDecimal,
        price_change_24h: BigDecimal,
        price_change_percent_24h: BigDecimal,
    ) -> Self {
        Self {
            last_price,
            high_24h,
            low_24h,
            volume_24h,
            price_change_24h,
            price_change_percent_24h,
        }
    }

    /// 在 provider 只给出最新价与 24 小时成交量时生成平盘统计：最高价和最低价都取最新价，涨跌额与涨跌幅记为 0。
    /// 这是信息缺失下的保守填充而非真实行情，消费端不能据此断定该交易对 24 小时内没有波动。
    pub fn flat(last_price: BigDecimal, volume_24h: BigDecimal) -> Self {
        Self {
            high_24h: last_price.clone(),
            low_24h: last_price.clone(),
            last_price,
            volume_24h,
            price_change_24h: BigDecimal::from(0),
            price_change_percent_24h: BigDecimal::from(0),
        }
    }
}

impl MarketTickerSnapshot {
    /// 用最新价与成交量构造平盘 ticker 快照，并规范化交易对符号。
    /// provider、价格和观察时间原样保留；该函数不检查价格正数或行情新鲜度。
    pub fn new(
        provider: MarketDataProvider,
        symbol: &str,
        last_price: BigDecimal,
        volume_24h: BigDecimal,
        observed_at: DateTime<Utc>,
    ) -> Result<Self, MarketSymbolError> {
        Self::with_24h(
            provider,
            symbol,
            MarketTickerValues::flat(last_price, volume_24h),
            observed_at,
        )
    }

    /// 用完整 24 小时统计构造 ticker 快照：交易对经 [`ValidatedMarketSymbol`] 规范化后存储，格式非法直接返回错误。
    /// provider、六项数值与观察时间原样封装成不可变快照；`observed_at` 必须取 provider 载荷中的时间而非本机时间，
    /// 否则 Redis 的防倒退比较和消费端的陈旧判定都会失真，旧行情会被当作新行情接受。
    /// 本函数只做格式校验：不检查价格为正、不检查最新价落在 24 小时高低区间内，也不判断快照是否已经过期。
    /// 价格、成交量及涨跌字段的业务一致性由 provider 适配器负责，消费者读取后仍须自行检查正数与新鲜度。
    pub fn with_24h(
        provider: MarketDataProvider,
        symbol: &str,
        values: MarketTickerValues,
        observed_at: DateTime<Utc>,
    ) -> Result<Self, MarketSymbolError> {
        let symbol = ValidatedMarketSymbol::from_raw(symbol)?.as_str().to_owned();
        Ok(Self {
            provider,
            symbol,
            last_price: values.last_price,
            high_24h: values.high_24h,
            low_24h: values.low_24h,
            volume_24h: values.volume_24h,
            price_change_24h: values.price_change_24h,
            price_change_percent_24h: values.price_change_percent_24h,
            observed_at,
        })
    }

    /// 返回这份 ticker 的来源，取值为 Bitget、HTX、Coinbase 或内部 Strategy 策略行情。
    /// 该值会写进公开事件载荷的 `provider` 字段，也是日志归因与多源同交易对取舍的依据。
    pub fn provider(&self) -> MarketDataProvider {
        self.provider
    }

    /// 返回去分隔符并转大写后的规范化交易对，与 `market:ticker:*` 缓存 key 和公开事件 topic 使用同一书写形式。
    /// 构造时已完成校验，调用方无需再规范化一次，也不会拿到含 `/`、`-` 的原始 provider 写法。
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// 返回最新成交价，单位是计价资产，小数位沿用 provider 原始字符串，未做统一截断或补零。
    /// 这是现货下单校验、限价单触发与强平判断读取的价格基准，使用前应结合 `observed_at` 排除陈旧快照。
    pub fn last_price(&self) -> &BigDecimal {
        &self.last_price
    }

    /// 返回滚动 24 小时最高成交价；provider 未提供该字段时会被回填成最新价而不是 0。
    /// 统计口径完全由行情源定义，本项目不按本地成交流重算，因此不同 provider 之间的取值不可直接比较。
    pub fn high_24h(&self) -> &BigDecimal {
        &self.high_24h
    }

    /// 返回滚动 24 小时最低成交价；同样在 provider 缺字段时回填为最新价，避免出现 0 价被误当作真实低点。
    /// 与最高价成对用于展示价格区间，不参与撮合、保证金或风控阈值计算。
    pub fn low_24h(&self) -> &BigDecimal {
        &self.low_24h
    }

    /// 返回滚动 24 小时成交量，各适配器优先取基础资产口径字段，例如 Bitget 的 `baseVolume`、HTX 的 `amount`。
    /// 由于口径由行情源自行定义，本层不做跨 provider 换算，该值只用于展示与流动性观察，不参与资金结算。
    pub fn volume_24h(&self) -> &BigDecimal {
        &self.volume_24h
    }

    /// 返回 24 小时涨跌额，即最新价减去 24 小时前开盘价，单位与价格一致，负值表示下跌。
    /// provider 未给出开盘价时该值记为 0，表示信息缺失而非真实持平。
    pub fn price_change_24h(&self) -> &BigDecimal {
        &self.price_change_24h
    }

    /// 返回 24 小时涨跌幅，已换算成百分数，例如 1.5 表示上涨 1.5%，而不是 0.015 这样的比率。
    /// provider 直接给出比率时乘以 100 得到；否则由涨跌额除以开盘价推算，开盘价缺失或为 0 时记为 0。
    pub fn price_change_percent_24h(&self) -> &BigDecimal {
        &self.price_change_percent_24h
    }

    /// 返回该行情在 provider 侧的观察时间，不是本服务收到帧或写入 Redis 的时间。
    /// 它是缓存原子防倒退比较和消费端陈旧判定的唯一依据；跨 provider 比较时需要考虑各家时钟偏差。
    pub fn observed_at(&self) -> DateTime<Utc> {
        self.observed_at
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketDepthLevel {
    pub price: BigDecimal,
    pub quantity: BigDecimal,
}

impl MarketDepthLevel {
    /// 封装盘口中的一档报价及该价位上的挂单数量，两个值都按 provider 给出的十进制精度原样保留。
    /// 不在此处排序，也不自动过滤零值或负值；合并同价档、裁剪档数等清洗动作由解析器和消费端各自决定。
    pub fn new(price: BigDecimal, quantity: BigDecimal) -> Self {
        Self { price, quantity }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketDepthSnapshot {
    provider: MarketDataProvider,
    symbol: String,
    bids: Vec<MarketDepthLevel>,
    asks: Vec<MarketDepthLevel>,
    #[serde(with = "unix_millis")]
    observed_at: DateTime<Utc>,
}

impl MarketDepthSnapshot {
    /// 构造某一时刻的盘口快照并规范化交易对，买卖盘顺序保持 provider 解析后的原始结果。
    /// 本函数不重排档位、不合并同价数量、不裁剪档数，也不校验买一价低于卖一价，交叉盘口会原样留存。
    /// 档位可能只是增量更新而非完整订单簿，Coinbase 的 level2 即属此类，是否可当全量由消费端结合来源判断。
    /// `observed_at` 应取 provider 时间；上游在缺少该时间时会退化成本机时间，那种情况下新鲜度判定会偏乐观。
    pub fn new(
        provider: MarketDataProvider,
        symbol: &str,
        bids: Vec<MarketDepthLevel>,
        asks: Vec<MarketDepthLevel>,
        observed_at: DateTime<Utc>,
    ) -> Result<Self, MarketSymbolError> {
        let symbol = ValidatedMarketSymbol::from_raw(symbol)?.as_str().to_owned();
        Ok(Self {
            provider,
            symbol,
            bids,
            asks,
            observed_at,
        })
    }

    /// 返回这份盘口的来源行情源，用于区分同一交易对来自不同 provider 的深度，避免混用口径不同的档位。
    /// 该值同样写入 depth 事件载荷的 `provider` 字段，供订阅方识别数据出处。
    pub fn provider(&self) -> MarketDataProvider {
        self.provider
    }

    /// 返回规范化后的交易对，`market:depth:*` 缓存 key 与公开 WebSocket topic 都由这同一字符串派生。
    /// 格式在构造时已校验，不会出现分隔符或小写字母，可直接参与字符串拼接。
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// 返回买盘档位切片，顺序沿用 provider 解析结果，并未按价格重新降序排列。
    /// 需要买一价时应自行取价格最大值；若来源是增量推送，这里可能只包含本次变更的档位而非完整买盘。
    pub fn bids(&self) -> &[MarketDepthLevel] {
        &self.bids
    }

    /// 返回卖盘档位切片，同样保持 provider 原顺序，因此取最优卖价必须求价格最小值而不能直接取首档。
    /// 行情摄取正是对本切片取最小价作为现货限价单的触发价候选，档位为空则跳过该次触发。
    pub fn asks(&self) -> &[MarketDepthLevel] {
        &self.asks
    }

    /// 返回盘口在 provider 侧的观察时间，用来判断这份深度是否仍值得参考。
    /// 写入缓存不会刷新该时间，因此 Redis 中读回的旧快照仍然带着它原来的观察时间。
    pub fn observed_at(&self) -> DateTime<Utc> {
        self.observed_at
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketKlineValues {
    pub open: BigDecimal,
    pub high: BigDecimal,
    pub low: BigDecimal,
    pub close: BigDecimal,
    pub volume: BigDecimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketKlineSnapshot {
    provider: MarketDataProvider,
    symbol: String,
    interval: String,
    #[serde(with = "unix_millis")]
    open_time: DateTime<Utc>,
    open: BigDecimal,
    high: BigDecimal,
    low: BigDecimal,
    close: BigDecimal,
    volume: BigDecimal,
    #[serde(with = "unix_millis")]
    observed_at: DateTime<Utc>,
}

impl MarketKlineSnapshot {
    /// 构造标准 K 线快照：规范化交易对，并用 [`KlineUpsertKey`] 把周期限定在平台支持的六种之内。
    /// `open_time` 标识蜡烛所属的时间槽，与交易对、周期一起构成 Redis 与 Mongo 的幂等写入键；
    /// `observed_at` 表示这一版数值的观察时间，形成中的蜡烛会以相同 `open_time` 反复更新，靠它区分先后。
    /// OHLC、成交量和两个时间戳都原样保留；该函数不校验最高价不低于最低价、收盘价是否落在区间内，
    /// 也不检查开盘时间是否对齐周期边界，错位的开盘时间会直接形成一个独立且无法与历史对齐的时间槽。
    pub fn new(
        provider: MarketDataProvider,
        symbol: &str,
        interval: &str,
        open_time: DateTime<Utc>,
        values: MarketKlineValues,
        observed_at: DateTime<Utc>,
    ) -> Result<Self, MarketCacheEntryError> {
        let symbol = ValidatedMarketSymbol::from_raw(symbol)?.as_str().to_owned();
        KlineUpsertKey::new(interval, open_time)?;
        Ok(Self {
            provider,
            symbol,
            interval: interval.to_owned(),
            open_time,
            open: values.open,
            high: values.high,
            low: values.low,
            close: values.close,
            volume: values.volume,
            observed_at,
        })
    }

    /// 返回这根蜡烛的采集来源，它会作为 `source` 字段写入 Mongo 历史文档，用于区分第三方行情与策略行情。
    /// 同一交易对切换 provider 后历史里会并存不同来源的记录，回溯时需要据此判断口径是否连续。
    pub fn provider(&self) -> MarketDataProvider {
        self.provider
    }

    /// 返回规范化后的交易对，它同时决定 `market:kline:*` 缓存 key 前缀与 `market_klines_<SYMBOL>` 集合名。
    /// 每个交易对拥有独立的 Mongo 集合，因此这个字符串必须稳定，改变写法会导致历史落到另一个集合。
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// 返回已通过白名单校验的周期文本，取值属于 `1m/5m/15m/1h/4h/1d`，缓存 key 与 Mongo 唯一索引都依赖它。
    /// 该值保持平台内部写法，未转换成 Bitget 的 `1H` 或 HTX 的 `60min` 等 provider 专有格式。
    pub fn interval(&self) -> &str {
        &self.interval
    }

    /// 返回蜡烛所属时间槽的开盘时间，它与周期共同构成 Mongo 唯一索引键，决定重放时覆盖哪一条历史记录。
    /// 同一根形成中的蜡烛在收线前会多次上报同一个开盘时间，因此它不能单独用来判断数据新旧。
    pub fn open_time(&self) -> DateTime<Utc> {
        self.open_time
    }

    /// 返回该时间槽的开盘价，单位为计价资产，精度沿用 provider 原始字符串。
    /// 写入 Mongo 时会转成十进制字符串保存，避免浮点转换在长周期回放中累积误差。
    pub fn open(&self) -> &BigDecimal {
        &self.open
    }

    /// 返回该时间槽内出现过的最高成交价；蜡烛仍在形成时，这个值会随后续推送单调上抬。
    /// 本类型不保证它不小于最低价，异常数据需要由 provider 解析层或消费端自行识别。
    pub fn high(&self) -> &BigDecimal {
        &self.high
    }

    /// 返回该时间槽内出现过的最低成交价；形成中的蜡烛会随后续推送继续下探。
    /// 与最高价一样按 provider 精度原样保留，不做统一小数位对齐。
    pub fn low(&self) -> &BigDecimal {
        &self.low
    }

    /// 返回该时间槽的收盘价；蜡烛未收线时它等于当前最新成交价，会随每次推送变化。
    /// 因此不能仅凭这个字段判断蜡烛是否已经结束，需要结合开盘时间与当前时间推算。
    pub fn close(&self) -> &BigDecimal {
        &self.close
    }

    /// 返回该时间槽内的累计成交量，口径与 provider 的 K 线字段一致，通常为基础资产数量。
    /// 形成中的蜡烛该值只增不减，跨 provider 之间不做换算，不能直接相加汇总。
    pub fn volume(&self) -> &BigDecimal {
        &self.volume
    }

    /// 返回本版 OHLCV 在 provider 侧的观察时间，用于区分同一 `open_time` 下先后上报的多个版本。
    /// 缓存的原子防倒退比较正是先比 `open_time` 再比这个时间，缺少它就无法阻止同分钟内的旧值覆盖新值。
    pub fn observed_at(&self) -> DateTime<Utc> {
        self.observed_at
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketTradeTick {
    provider: MarketDataProvider,
    symbol: String,
    trade_id: String,
    side: MarketTradeSide,
    price: BigDecimal,
    quantity: BigDecimal,
    #[serde(with = "unix_millis")]
    traded_at: DateTime<Utc>,
}

impl MarketTradeTick {
    /// 构造标准逐笔成交并规范化交易对，保留 provider 成交编号、方向、价格、数量与成交时间。
    /// `trade_id` 直接取自 provider 且只在该来源内唯一，它同时充当成交事件的幂等键，跨 provider 可能重号。
    /// `traded_at` 是成交发生时间而非快照观察时间，逐笔成交不进入 Redis 权威缓存，因此没有防倒退比较。
    /// 本函数不推导买卖方向、不校验数值为正，也不判断成交是否重复，具体适配器必须先完成字段语义转换。
    pub fn new(
        provider: MarketDataProvider,
        symbol: &str,
        trade_id: impl Into<String>,
        side: MarketTradeSide,
        price: BigDecimal,
        quantity: BigDecimal,
        traded_at: DateTime<Utc>,
    ) -> Result<Self, MarketSymbolError> {
        let symbol = ValidatedMarketSymbol::from_raw(symbol)?.as_str().to_owned();
        Ok(Self {
            provider,
            symbol,
            trade_id: trade_id.into(),
            side,
            price,
            quantity,
            traded_at,
        })
    }

    /// 返回这笔成交的行情来源；成交编号只在同一来源内唯一，判重时必须连同该值一起考虑。
    /// 这里的成交来自外部行情源，与本平台撮合产生的成交是两套数据，不可混用。
    pub fn provider(&self) -> MarketDataProvider {
        self.provider
    }

    /// 返回规范化后的交易对，公开成交事件的 topic 与路由键都基于它生成。
    /// 逐笔成交不写 Redis，也不写 Mongo，这个交易对只用于事件分发与前端归类。
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// 返回 provider 给出的成交编号，原样保留为字符串以兼容各家的数字或字母编号格式。
    /// 该编号构成成交事件幂等键的最后一段，重复推送同一编号会生成完全相同的事件标识。
    pub fn trade_id(&self) -> &str {
        &self.trade_id
    }

    /// 返回买卖方向，由适配器把各家的 buy/sell、bid/ask、direction 等写法统一映射而来。
    /// 该方向表示这笔成交的主动方，无法识别的取值在解析阶段就已被拒绝，不会退化成默认买入。
    pub fn side(&self) -> MarketTradeSide {
        self.side
    }

    /// 返回这笔成交的成交价，单位为计价资产，精度沿用 provider 原始十进制字符串。
    /// 它反映的是外部市场的历史成交，不作为本平台下单校验或强平判断的价格基准。
    pub fn price(&self) -> &BigDecimal {
        &self.price
    }

    /// 返回这笔成交的数量，口径为基础资产，取自各家的 size、qty 或 amount 字段。
    /// 数值未做归一或最小变动量对齐，展示前需按交易对精度自行格式化。
    pub fn quantity(&self) -> &BigDecimal {
        &self.quantity
    }

    /// 返回成交在 provider 侧发生的时间；缺少逐笔时间时适配器会退回帧级时间戳，精度因此可能变粗。
    /// 该时间只用于展示与排序，不参与任何缓存新鲜度比较。
    pub fn traded_at(&self) -> DateTime<Utc> {
        self.traded_at
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MarketCacheEntryError {
    #[error(transparent)]
    Symbol(#[from] MarketSymbolError),
    #[error(transparent)]
    Kline(#[from] KlineKeyError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketEventType {
    Ticker,
    Depth,
    Trade,
    Kline,
    Strategy,
}
