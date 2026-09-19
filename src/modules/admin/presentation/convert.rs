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
    pub(crate) inventory: Option<ConvertInventoryConfig>,
    pub(crate) from_asset_id: u64,
    pub(crate) to_asset_id: u64,
    pub(crate) pricing_mode: String,
    #[serde(deserialize_with = "crate::numeric::deserialize_decimal")]
    pub(crate) spread_rate: BigDecimal,
    #[serde(
        default,
        deserialize_with = "crate::numeric::deserialize_optional_decimal"
    )]
    pub(crate) fee_rate: Option<BigDecimal>,
    #[serde(deserialize_with = "crate::numeric::deserialize_decimal")]
    pub(crate) min_amount: BigDecimal,
    #[serde(
        default,
        deserialize_with = "crate::numeric::deserialize_optional_decimal"
    )]
    pub(crate) max_amount: Option<BigDecimal>,
    #[serde(
        default,
        deserialize_with = "crate::numeric::deserialize_optional_decimal"
    )]
    pub(crate) target_min_amount: Option<BigDecimal>,
    #[serde(
        default,
        deserialize_with = "crate::numeric::deserialize_optional_decimal"
    )]
    pub(crate) target_max_amount: Option<BigDecimal>,
    pub(crate) enabled: Option<bool>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateConvertPairRequest {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateConvertPairRequest {
    pub(crate) inventory: Option<ConvertInventoryConfig>,
    pub(crate) from_asset_id: Option<u64>,
    pub(crate) to_asset_id: Option<u64>,
    pub(crate) pricing_mode: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::numeric::deserialize_optional_decimal"
    )]
    pub(crate) spread_rate: Option<BigDecimal>,
    #[serde(
        default,
        deserialize_with = "crate::numeric::deserialize_optional_decimal"
    )]
    pub(crate) fee_rate: Option<BigDecimal>,
    #[serde(
        default,
        deserialize_with = "crate::numeric::deserialize_optional_decimal"
    )]
    pub(crate) min_amount: Option<BigDecimal>,
    #[serde(
        default,
        deserialize_with = "crate::numeric::deserialize_patch_decimal"
    )]
    pub(crate) max_amount: Option<Option<BigDecimal>>,
    #[serde(
        default,
        deserialize_with = "crate::numeric::deserialize_optional_decimal"
    )]
    pub(crate) target_min_amount: Option<BigDecimal>,
    #[serde(
        default,
        deserialize_with = "crate::numeric::deserialize_patch_decimal"
    )]
    pub(crate) target_max_amount: Option<Option<BigDecimal>>,
    pub(crate) enabled: Option<bool>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateConvertPairRequest {}

/// 显式资金配置，空对象不允许；省略或 null 表示不修改，停用使用 enabled=false 并保留总额。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConvertInventoryConfig {
    pub(crate) revision: u64,
    pub(crate) enabled: bool,
    #[serde(deserialize_with = "crate::numeric::deserialize_decimal")]
    pub(crate) funded_amount: BigDecimal,
    pub(crate) funding_reference: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DeleteConvertPairRequest {
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for DeleteConvertPairRequest {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct ConvertPairResponse {
    pub(crate) inventory_enabled: bool,
    pub(crate) inventory_funded_amount: Option<BigDecimal>,
    pub(crate) inventory_consumed_amount: Option<BigDecimal>,
    pub(crate) inventory_revision: Option<u64>,
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
