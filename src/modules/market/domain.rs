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
    pub fn new(
        interval: impl Into<String>,
        open_time: DateTime<Utc>,
    ) -> Result<Self, KlineKeyError> {
        let interval = interval.into();
        if matches!(interval.as_str(), "1m" | "5m" | "15m" | "1h" | "1d") {
            Ok(Self {
                interval,
                open_time,
            })
        } else {
            Err(KlineKeyError::InvalidInterval)
        }
    }

    pub fn interval(&self) -> &str {
        &self.interval
    }

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

    pub fn provider(&self) -> MarketDataProvider {
        self.provider
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketDepthLevel {
    pub price: BigDecimal,
    pub quantity: BigDecimal,
}

impl MarketDepthLevel {
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

    pub fn provider(&self) -> MarketDataProvider {
        self.provider
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

    pub fn provider(&self) -> MarketDataProvider {
        self.provider
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

    pub fn provider(&self) -> MarketDataProvider {
        self.provider
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub fn trade_id(&self) -> &str {
        &self.trade_id
    }

    pub fn side(&self) -> MarketTradeSide {
        self.side
    }

    pub fn price(&self) -> &BigDecimal {
        &self.price
    }

    pub fn quantity(&self) -> &BigDecimal {
        &self.quantity
    }

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
