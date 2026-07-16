//! seconds_contract bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务逻辑逐步迁入。

use crate::time::unix_millis;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub(crate) struct ListQuery {
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminOrdersQuery {
    pub(crate) limit: Option<u32>,
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenSecondsContractOrderRequest {
    pub(crate) product_id: u64,
    pub(crate) duration_seconds: Option<u32>,
    pub(crate) direction: String,
    pub(crate) stake_amount: BigDecimal,
    pub(crate) idempotency_key: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SecondsContractProductCycleInput {
    pub(crate) duration_seconds: Option<u32>,
    pub(crate) payout_rate: Option<BigDecimal>,
    pub(crate) min_stake: Option<BigDecimal>,
    pub(crate) max_stake: Option<BigDecimal>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct SecondsContractProductCycleResponse {
    pub(crate) id: u64,
    pub(crate) product_id: u64,
    pub(crate) duration_seconds: u32,
    pub(crate) payout_rate: BigDecimal,
    pub(crate) min_stake: BigDecimal,
    pub(crate) max_stake: Option<BigDecimal>,
    pub(crate) sort_order: u32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateSecondsContractProductRequest {
    pub(crate) pair_id: u64,
    pub(crate) stake_asset: u64,
    pub(crate) logo_url: Option<String>,
    pub(crate) duration_seconds: Option<u32>,
    pub(crate) payout_rate: Option<BigDecimal>,
    pub(crate) min_stake: Option<BigDecimal>,
    pub(crate) max_stake: Option<BigDecimal>,
    pub(crate) cycles: Option<Vec<SecondsContractProductCycleInput>>,
    pub(crate) status: Option<String>,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateSecondsContractProductRequest {
    pub(crate) pair_id: u64,
    pub(crate) stake_asset: u64,
    pub(crate) logo_url: Option<String>,
    pub(crate) duration_seconds: Option<u32>,
    pub(crate) payout_rate: Option<BigDecimal>,
    pub(crate) min_stake: Option<BigDecimal>,
    pub(crate) max_stake: Option<BigDecimal>,
    pub(crate) cycles: Option<Vec<SecondsContractProductCycleInput>>,
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateSecondsContractProductStatusRequest {
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DeleteSecondsContractProductRequest {
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SecondsContractProductResponse {
    pub(crate) id: u64,
    pub(crate) pair_id: u64,
    pub(crate) symbol: String,
    pub(crate) stake_asset: u64,
    pub(crate) stake_asset_symbol: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) duration_seconds: u32,
    pub(crate) payout_rate: BigDecimal,
    pub(crate) min_stake: BigDecimal,
    pub(crate) max_stake: Option<BigDecimal>,
    pub(crate) cycles: Vec<SecondsContractProductCycleResponse>,
    pub(crate) status: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CachedTickerPayload {
    pub(crate) last_price: BigDecimal,
    #[serde(with = "unix_millis")]
    pub(crate) observed_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SecondsContractProductsResponse {
    pub(crate) products: Vec<SecondsContractProductResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SecondsContractOrdersResponse {
    pub(crate) orders: Vec<SecondsContractOrderResponse>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct SecondsContractOrderResponse {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    pub(crate) email: Option<String>,
    pub(crate) product_id: u64,
    pub(crate) pair_id: u64,
    pub(crate) symbol: String,
    pub(crate) stake_asset: u64,
    pub(crate) stake_asset_symbol: String,
    pub(crate) direction: String,
    pub(crate) stake_amount: BigDecimal,
    pub(crate) duration_seconds: u32,
    pub(crate) payout_rate: BigDecimal,
    pub(crate) entry_price: Option<BigDecimal>,
    pub(crate) settlement_price: Option<BigDecimal>,
    pub(crate) status: String,
    pub(crate) result: Option<String>,
    pub(crate) idempotency_key: String,
    #[serde(with = "unix_millis")]
    pub(crate) expires_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct OpenSecondsContractOrderResponse {
    pub(crate) order: SecondsContractOrderResponse,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SettleSecondsContractOrderRequest {
    pub(crate) result: String,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SettleSecondsContractOrderResponse {
    pub(crate) order: SecondsContractOrderResponse,
    pub(crate) payout_amount: BigDecimal,
}
