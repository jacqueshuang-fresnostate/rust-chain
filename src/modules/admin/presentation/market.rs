//! 后台交易对与做市策略 DTO。

use super::*;

#[derive(Debug, Deserialize)]
pub(crate) struct AdminTradingPairQuery {
    pub(crate) symbol: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) market_type: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminTradingPairQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateTradingPairRequest {
    pub(crate) base_asset_id: u64,
    pub(crate) quote_asset_id: u64,
    pub(crate) symbol: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) price_precision: i32,
    pub(crate) qty_precision: i32,
    pub(crate) min_order_value: BigDecimal,
    pub(crate) status: Option<String>,
    pub(crate) market_type: Option<String>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateTradingPairRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateTradingPairStatusRequest {
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateTradingPairStatusRequest {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateTradingPairRequest {
    pub(crate) logo_url: Option<String>,
    pub(crate) price_precision: i32,
    pub(crate) qty_precision: i32,
    pub(crate) min_order_value: BigDecimal,
    pub(crate) status: String,
    pub(crate) market_type: String,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateTradingPairRequest {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminTradingPairResponse {
    pub(crate) id: u64,
    pub(crate) base_asset_id: u64,
    pub(crate) quote_asset_id: u64,
    pub(crate) symbol: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) base_asset: String,
    pub(crate) quote_asset: String,
    pub(crate) price_precision: i32,
    pub(crate) qty_precision: i32,
    pub(crate) min_order_value: BigDecimal,
    pub(crate) status: String,
    pub(crate) market_type: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for AdminTradingPairResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminTradingPairsResponse {
    pub(crate) pairs: Vec<AdminTradingPairResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for AdminTradingPairsResponse {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminMarketStrategyQuery {
    pub(crate) pair_id: Option<u64>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminMarketStrategyQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateMarketStrategyRequest {
    pub(crate) pair_id: u64,
    pub(crate) strategy_type: String,
    pub(crate) start_price: BigDecimal,
    pub(crate) target_price: BigDecimal,
    #[serde(with = "unix_millis")]
    pub(crate) start_time: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) end_time: DateTime<Utc>,
    pub(crate) volatility: BigDecimal,
    pub(crate) volume_min: BigDecimal,
    pub(crate) volume_max: BigDecimal,
    pub(crate) status: Option<String>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateMarketStrategyRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateMarketStrategyRequest {
    pub(crate) strategy_type: String,
    pub(crate) start_price: BigDecimal,
    pub(crate) target_price: BigDecimal,
    #[serde(with = "unix_millis")]
    pub(crate) start_time: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) end_time: DateTime<Utc>,
    pub(crate) volatility: BigDecimal,
    pub(crate) volume_min: BigDecimal,
    pub(crate) volume_max: BigDecimal,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateMarketStrategyRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateMarketStrategyStatusRequest {
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateMarketStrategyStatusRequest {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminMarketStrategyResponse {
    pub(crate) id: u64,
    pub(crate) pair_id: u64,
    pub(crate) symbol: String,
    pub(crate) market_type: String,
    pub(crate) strategy_type: String,
    pub(crate) start_price: BigDecimal,
    pub(crate) target_price: BigDecimal,
    #[serde(with = "unix_millis")]
    pub(crate) start_time: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) end_time: DateTime<Utc>,
    pub(crate) volatility: BigDecimal,
    pub(crate) volume_min: BigDecimal,
    pub(crate) volume_max: BigDecimal,
    pub(crate) status: String,
    pub(crate) run_status: Option<String>,
    pub(crate) current_price: Option<BigDecimal>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) last_generated_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) last_kline_open_time: Option<DateTime<Utc>>,
    pub(crate) recovery_status: Option<String>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for AdminMarketStrategyResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminMarketStrategiesResponse {
    pub(crate) strategies: Vec<AdminMarketStrategyResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for AdminMarketStrategiesResponse {}
