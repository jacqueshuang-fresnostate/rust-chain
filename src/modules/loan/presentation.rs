//! loan bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。

use crate::time::{option_unix_millis, unix_millis};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json as SqlxJson;

#[derive(Debug, Deserialize)]
pub(crate) struct ListQuery {
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminLoanOrdersQuery {
    pub(crate) limit: Option<u32>,
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) product_id: Option<u64>,
    pub(crate) loan_type: Option<String>,
    pub(crate) status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UserLoanOrdersQuery {
    pub(crate) limit: Option<u32>,
    pub(crate) status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateLoanProductRequest {
    pub(crate) loan_type: String,
    pub(crate) asset_id: u64,
    pub(crate) name: String,
    pub(crate) name_json: Option<Value>,
    pub(crate) term_days: u32,
    pub(crate) interest_rate: BigDecimal,
    pub(crate) interest_calculation_mode: String,
    pub(crate) min_kyc_level: i32,
    pub(crate) min_amount: BigDecimal,
    pub(crate) max_amount: Option<BigDecimal>,
    pub(crate) status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateLoanProductRequest {
    pub(crate) loan_type: String,
    pub(crate) asset_id: u64,
    pub(crate) name: String,
    pub(crate) name_json: Option<Value>,
    pub(crate) term_days: u32,
    pub(crate) interest_rate: BigDecimal,
    pub(crate) interest_calculation_mode: String,
    pub(crate) min_kyc_level: i32,
    pub(crate) min_amount: BigDecimal,
    pub(crate) max_amount: Option<BigDecimal>,
    pub(crate) status: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateLoanProductStatusRequest {
    pub(crate) status: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateLoanOrderRequest {
    pub(crate) product_id: u64,
    pub(crate) amount: BigDecimal,
    pub(crate) collateral_asset_id: Option<u64>,
    pub(crate) collateral_amount: Option<BigDecimal>,
    pub(crate) idempotency_key: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ReviewLoanOrderRequest {
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct LoanProductsResponse {
    pub(crate) products: Vec<LoanProductResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct LoanOrdersResponse {
    pub(crate) orders: Vec<LoanOrderResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct LoanOrderActionResponse {
    pub(crate) order: LoanOrderResponse,
    pub(crate) changed: bool,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct LoanProductResponse {
    id: u64,
    loan_type: String,
    asset_id: u64,
    asset_symbol: String,
    name: String,
    name_json: SqlxJson<Value>,
    term_days: u32,
    interest_rate: BigDecimal,
    interest_calculation_mode: String,
    min_kyc_level: i32,
    min_amount: BigDecimal,
    max_amount: Option<BigDecimal>,
    status: String,
    #[serde(with = "unix_millis")]
    created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct LoanOrderResponse {
    id: u64,
    user_id: u64,
    user_email: Option<String>,
    product_id: u64,
    product_name: String,
    product_name_json: SqlxJson<Value>,
    loan_type: String,
    asset_id: u64,
    asset_symbol: String,
    amount: BigDecimal,
    interest_rate: BigDecimal,
    interest_calculation_mode: String,
    term_days: u32,
    min_kyc_level: i32,
    collateral_asset_id: Option<u64>,
    collateral_asset_symbol: Option<String>,
    collateral_amount: Option<BigDecimal>,
    status: String,
    interest_amount: BigDecimal,
    repayment_amount: BigDecimal,
    approved_by: Option<u64>,
    rejected_by: Option<u64>,
    rejected_reason: Option<String>,
    #[serde(default, with = "option_unix_millis")]
    approved_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    rejected_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    disbursed_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    due_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    cancelled_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    repaid_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    collateral_released_at: Option<DateTime<Utc>>,
    #[serde(with = "unix_millis")]
    created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    updated_at: DateTime<Utc>,
}
