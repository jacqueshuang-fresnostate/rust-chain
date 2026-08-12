//! market bounded context domain layer.
//!
//! 领域层：放置市场符号、行情快照、K线查询和值对象等不依赖 I/O 的业务规则。

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

    /// 返回规范化字符串。
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

    /// 返回 K 线周期。
    pub fn interval(&self) -> &str {
        &self.interval
    }

    /// 返回 K 线开盘时间。
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
    /// 构造历史 K 线查询条件，复用 K 线键规则校验周期，并把条数限制收敛到 1..=100。
    /// 起止时间原样保留，具体范围关系、排序和数据源选择由查询应用层与 Mongo 适配器负责。
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
    /// 汇集 provider 已解析出的最新价、24 小时高低价、成交量和涨跌指标。
    /// 此构造器不修正负数或字段间关系，数据合法性应由具体 provider 解析器保证。
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

    /// 在 provider 只给出最新价与成交量时生成平盘统计：高低价等于最新价，涨跌额与涨跌幅为零。
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

    /// 用完整 24 小时统计构造 ticker 快照，仅负责交易对规范化和字段封装。
    /// 价格、成交量及涨跌字段的业务一致性由 provider 适配器负责，消费者仍须检查正数与新鲜度。
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

    /// 返回行情提供方。
    pub fn provider(&self) -> MarketDataProvider {
        self.provider
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketDepthLevel {
    pub price: BigDecimal,
    pub quantity: BigDecimal,
}

impl MarketDepthLevel {
    /// 封装一档盘口价格与数量；不在此处排序，也不自动过滤零值或负值。
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
    /// 构造指定 provider 的盘口快照并规范化交易对，买卖盘顺序保持 provider 解析后的结果。
    /// 本函数不重排档位或合并同价数量，观察时间与档位有效性由上游适配器负责。
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

    /// 返回行情提供方。
    pub fn provider(&self) -> MarketDataProvider {
        self.provider
    }

    /// 返回交易对符号。
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// 返回 provider 解析器保留顺序的买盘档位。
    pub fn bids(&self) -> &[MarketDepthLevel] {
        &self.bids
    }

    /// 返回 provider 解析器保留顺序的卖盘档位。
    pub fn asks(&self) -> &[MarketDepthLevel] {
        &self.asks
    }

    /// 返回行情观测时间。
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
    /// 构造标准 K 线快照：规范化交易对，并用 [`KlineUpsertKey`] 校验周期。
    /// OHLC、成交量和时间戳原样保留；该函数不校验高低价关系或开盘时间对齐。
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

    /// 返回行情提供方。
    pub fn provider(&self) -> MarketDataProvider {
        self.provider
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

    /// 返回行情观测时间。
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
    /// 本函数不推导买卖方向或校验数值正数，具体 provider 适配器必须先完成字段语义转换。
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

    /// 返回行情提供方。
    pub fn provider(&self) -> MarketDataProvider {
        self.provider
    }

    /// 返回交易对符号。
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// 返回 provider 成交编号。
    pub fn trade_id(&self) -> &str {
        &self.trade_id
    }

    /// 返回方向。
    pub fn side(&self) -> MarketTradeSide {
        self.side
    }

    /// 返回价格。
    pub fn price(&self) -> &BigDecimal {
        &self.price
    }

    /// 返回成交数量。
    pub fn quantity(&self) -> &BigDecimal {
        &self.quantity
    }

    /// 返回 provider 成交时间。
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
