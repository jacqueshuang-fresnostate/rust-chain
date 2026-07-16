//! market bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。

use crate::{
    modules::market::{ValidatedMarketSymbol, repository::KlineDocumentRecord},
    time::{option_unix_millis, unix_millis},
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub(crate) struct MarketsResponse {
    pub(crate) markets: Vec<MarketResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct MarketResponse {
    pub(crate) id: u64,
    pub(crate) symbol: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) base_asset: String,
    pub(crate) quote_asset: String,
    pub(crate) price_precision: i32,
    pub(crate) qty_precision: i32,
    pub(crate) min_order_value: String,
    pub(crate) status: String,
    pub(crate) market_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct TickerResponse {
    pub(crate) symbol: String,
    pub(crate) last_price: String,
    pub(crate) high_24h: Option<String>,
    pub(crate) low_24h: Option<String>,
    pub(crate) volume_24h: String,
    pub(crate) price_change_24h: Option<String>,
    pub(crate) price_change_percent_24h: Option<String>,
    #[serde(with = "unix_millis")]
    pub(crate) observed_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct KlineQueryParams {
    pub(crate) interval: String,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) start: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) end: Option<DateTime<Utc>>,
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Serialize)]
pub(crate) struct KlineResponse {
    pub(crate) symbol: String,
    pub(crate) interval: String,
    #[serde(with = "unix_millis")]
    pub(crate) open_time: DateTime<Utc>,
    pub(crate) open: String,
    pub(crate) high: String,
    pub(crate) low: String,
    pub(crate) close: String,
    pub(crate) volume: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TradesQueryParams {
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DepthResponse {
    pub(crate) symbol: String,
    pub(crate) bids: Vec<DepthLevelResponse>,
    pub(crate) asks: Vec<DepthLevelResponse>,
    #[serde(with = "unix_millis")]
    pub(crate) observed_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DepthLevelResponse {
    pub(crate) price: String,
    pub(crate) amount: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DepthCachePayload {
    pub(crate) symbol: String,
    pub(crate) bids: Vec<DepthCacheLevel>,
    pub(crate) asks: Vec<DepthCacheLevel>,
    #[serde(with = "unix_millis")]
    pub(crate) observed_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DepthCacheLevel {
    pub(crate) price: BigDecimal,
    pub(crate) quantity: BigDecimal,
}

#[derive(Debug, Serialize)]
pub(crate) struct TradesResponse {
    pub(crate) trades: Vec<TradeResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct TradeResponse {
    pub(crate) id: String,
    pub(crate) symbol: String,
    pub(crate) price: String,
    pub(crate) amount: String,
    pub(crate) direction: String,
    #[serde(with = "unix_millis")]
    pub(crate) time: DateTime<Utc>,
}

impl DepthResponse {
    pub(crate) fn from_cache(depth: DepthCachePayload) -> Self {
        Self {
            symbol: depth.symbol,
            bids: depth
                .bids
                .into_iter()
                .map(DepthLevelResponse::from)
                .collect(),
            asks: depth
                .asks
                .into_iter()
                .map(DepthLevelResponse::from)
                .collect(),
            observed_at: depth.observed_at,
        }
    }
}

impl From<DepthCacheLevel> for DepthLevelResponse {
    fn from(level: DepthCacheLevel) -> Self {
        Self {
            price: level.price.to_string(),
            amount: level.quantity.to_string(),
        }
    }
}

impl MarketResponse {
    pub(crate) fn fallback(
        symbol: &str,
        base_asset: &str,
        quote_asset: &str,
        market_type: &str,
    ) -> Self {
        Self {
            id: 0,
            symbol: symbol.to_owned(),
            logo_url: None,
            base_asset: base_asset.to_owned(),
            quote_asset: quote_asset.to_owned(),
            price_precision: 8,
            qty_precision: 8,
            min_order_value: "1".to_owned(),
            status: "active".to_owned(),
            market_type: market_type.to_owned(),
        }
    }
}

impl KlineResponse {
    pub(crate) fn from_document(symbol: &str, document: KlineDocumentRecord) -> Self {
        Self {
            symbol: symbol.to_owned(),
            interval: document.interval,
            open_time: DateTime::<Utc>::from(document.open_time.to_system_time()),
            open: document.open,
            high: document.high,
            low: document.low,
            close: document.close,
            volume: document.volume,
        }
    }
}

impl TradeResponse {
    pub(crate) fn from_record(row: crate::modules::market::repository::SpotTradeRecord) -> Self {
        Self {
            id: row.id.to_string(),
            symbol: ValidatedMarketSymbol::from_raw(&row.symbol)
                .map(|symbol| symbol.as_str().to_owned())
                .unwrap_or(row.symbol),
            price: row.price.to_string(),
            amount: row.quantity.to_string(),
            direction: "BUY".to_owned(),
            time: row.created_at,
        }
    }
}
