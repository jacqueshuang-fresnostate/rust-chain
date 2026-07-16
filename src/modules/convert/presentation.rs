//! convert bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。

use crate::time::unix_millis;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub(crate) struct ListQuery {
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ConvertOrdersQuery {
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateConvertQuoteRequest {
    pub(crate) from_asset_id: u64,
    pub(crate) to_asset_id: u64,
    pub(crate) from_amount: BigDecimal,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ConfirmConvertQuoteRequest {
    pub(crate) quote_id: String,
}

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

#[derive(Debug, Serialize)]
pub(crate) struct ConvertPairsResponse {
    pub(crate) pairs: Vec<ConvertPairResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct ConvertOrderResponse {
    pub(crate) id: u64,
    pub(crate) quote_id: String,
    pub(crate) convert_pair_id: u64,
    pub(crate) from_asset_id: u64,
    pub(crate) to_asset_id: u64,
    pub(crate) from_amount: BigDecimal,
    pub(crate) to_amount: BigDecimal,
    pub(crate) rate: BigDecimal,
    pub(crate) fee_rate: BigDecimal,
    pub(crate) fee_amount: BigDecimal,
    pub(crate) status: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ConvertOrdersResponse {
    pub(crate) orders: Vec<ConvertOrderResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ConvertQuoteResponse {
    pub(crate) quote_id: String,
    pub(crate) convert_pair_id: u64,
    pub(crate) from_asset_id: u64,
    pub(crate) to_asset_id: u64,
    pub(crate) from_amount: BigDecimal,
    pub(crate) to_amount: BigDecimal,
    pub(crate) rate: BigDecimal,
    pub(crate) spread_rate: BigDecimal,
    pub(crate) fee_rate: BigDecimal,
    pub(crate) fee_amount: BigDecimal,
    #[serde(with = "unix_millis")]
    pub(crate) expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ConfirmConvertQuoteResponse {
    pub(crate) quote_id: String,
    pub(crate) confirmed: bool,
}
