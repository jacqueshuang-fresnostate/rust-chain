//! 后台闪兑交易对与闪兑订单 DTO。

use super::*;

#[derive(Debug, Deserialize)]
pub(crate) struct AdminConvertPairQuery {
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminConvertPairQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminConvertOrdersQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminConvertOrdersQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateConvertPairRequest {
    pub(crate) from_asset_id: u64,
    pub(crate) to_asset_id: u64,
    pub(crate) pricing_mode: String,
    pub(crate) spread_rate: BigDecimal,
    pub(crate) fee_rate: Option<BigDecimal>,
    pub(crate) min_amount: BigDecimal,
    pub(crate) max_amount: Option<BigDecimal>,
    pub(crate) target_min_amount: Option<BigDecimal>,
    pub(crate) target_max_amount: Option<BigDecimal>,
    pub(crate) enabled: Option<bool>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateConvertPairRequest {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateConvertPairRequest {
    pub(crate) from_asset_id: Option<u64>,
    pub(crate) to_asset_id: Option<u64>,
    pub(crate) pricing_mode: Option<String>,
    pub(crate) spread_rate: Option<BigDecimal>,
    pub(crate) fee_rate: Option<BigDecimal>,
    pub(crate) min_amount: Option<BigDecimal>,
    #[serde(default, deserialize_with = "double_option")]
    pub(crate) max_amount: Option<Option<BigDecimal>>,
    pub(crate) target_min_amount: Option<BigDecimal>,
    #[serde(default, deserialize_with = "double_option")]
    pub(crate) target_max_amount: Option<Option<BigDecimal>>,
    pub(crate) enabled: Option<bool>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateConvertPairRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct DeleteConvertPairRequest {
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for DeleteConvertPairRequest {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct ConvertPairResponse {
    pub(crate) id: u64,
    pub(crate) from_asset_id: u64,
    pub(crate) from_asset_symbol: String,
    pub(crate) to_asset_id: u64,
    pub(crate) to_asset_symbol: String,
    pub(crate) pricing_mode: String,
    pub(crate) spread_rate: BigDecimal,
    pub(crate) fee_rate: BigDecimal,
    pub(crate) min_amount: BigDecimal,
    pub(crate) max_amount: Option<BigDecimal>,
    pub(crate) target_min_amount: BigDecimal,
    pub(crate) target_max_amount: Option<BigDecimal>,
    pub(crate) enabled: bool,
}

impl PresentationLayer for ConvertPairResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct ConvertPairsResponse {
    pub(crate) pairs: Vec<ConvertPairResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for ConvertPairsResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct ConvertOrderResponse {
    pub(crate) id: u64,
    pub(crate) user_email: String,
    pub(crate) from_asset_symbol: String,
    pub(crate) to_asset_symbol: String,
    pub(crate) from_amount: BigDecimal,
    pub(crate) to_amount: BigDecimal,
    pub(crate) rate: BigDecimal,
    pub(crate) fee_rate: BigDecimal,
    pub(crate) fee_amount: BigDecimal,
    pub(crate) status: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for ConvertOrderResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct ConvertOrdersResponse {
    pub(crate) orders: Vec<ConvertOrderResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for ConvertOrdersResponse {}
