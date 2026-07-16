//! earn bounded context presentation layer.
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
pub(crate) struct AdminCategoriesQuery {
    pub(crate) limit: Option<u32>,
    pub(crate) status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminSubscriptionsQuery {
    pub(crate) limit: Option<u32>,
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SubscribeEarnRequest {
    pub(crate) product_id: u64,
    pub(crate) amount: BigDecimal,
    pub(crate) idempotency_key: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateEarnProductRequest {
    pub(crate) asset_id: u64,
    pub(crate) name: String,
    pub(crate) banner_url: Option<String>,
    pub(crate) small_logo_url: Option<String>,
    pub(crate) category: Option<String>,
    pub(crate) introduction_json: Option<Value>,
    pub(crate) term_days: u32,
    pub(crate) apr_rate: BigDecimal,
    pub(crate) redemption_fee_rate: Option<BigDecimal>,
    pub(crate) maturity_profit_fee_rate: Option<BigDecimal>,
    pub(crate) early_redeem_fee_basis: Option<String>,
    pub(crate) early_redeem_fee_rate: Option<BigDecimal>,
    pub(crate) min_subscribe: BigDecimal,
    pub(crate) max_subscribe: Option<BigDecimal>,
    pub(crate) status: Option<String>,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateEarnProductRequest {
    pub(crate) asset_id: u64,
    pub(crate) name: String,
    pub(crate) banner_url: Option<String>,
    pub(crate) small_logo_url: Option<String>,
    pub(crate) category: Option<String>,
    pub(crate) introduction_json: Option<Value>,
    pub(crate) term_days: u32,
    pub(crate) apr_rate: BigDecimal,
    pub(crate) redemption_fee_rate: Option<BigDecimal>,
    pub(crate) maturity_profit_fee_rate: Option<BigDecimal>,
    pub(crate) early_redeem_fee_basis: Option<String>,
    pub(crate) early_redeem_fee_rate: Option<BigDecimal>,
    pub(crate) min_subscribe: BigDecimal,
    pub(crate) max_subscribe: Option<BigDecimal>,
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateEarnProductStatusRequest {
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateEarnCategoryRequest {
    pub(crate) code: String,
    pub(crate) name_json: Option<Value>,
    pub(crate) sort_order: Option<i32>,
    pub(crate) status: Option<String>,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateEarnCategoryRequest {
    pub(crate) name_json: Option<Value>,
    pub(crate) sort_order: i32,
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateEarnCategoryStatusRequest {
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct EarnCategoryResponse {
    pub(crate) id: u64,
    pub(crate) code: String,
    pub(crate) name_json: SqlxJson<Value>,
    pub(crate) default_name: String,
    pub(crate) sort_order: i32,
    pub(crate) status: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct EarnCategoriesResponse {
    pub(crate) categories: Vec<EarnCategoryResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct EarnProductResponse {
    pub(crate) id: u64,
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    pub(crate) name: String,
    pub(crate) banner_url: Option<String>,
    pub(crate) small_logo_url: Option<String>,
    pub(crate) category: String,
    pub(crate) category_name: String,
    pub(crate) category_name_json: Option<SqlxJson<Value>>,
    pub(crate) introduction_json: SqlxJson<Value>,
    pub(crate) term_days: u32,
    pub(crate) apr_rate: BigDecimal,
    pub(crate) redemption_fee_rate: BigDecimal,
    pub(crate) maturity_profit_fee_rate: BigDecimal,
    pub(crate) early_redeem_fee_basis: String,
    pub(crate) early_redeem_fee_rate: BigDecimal,
    pub(crate) min_subscribe: BigDecimal,
    pub(crate) max_subscribe: Option<BigDecimal>,
    pub(crate) status: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct EarnProductsResponse {
    pub(crate) products: Vec<EarnProductResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct EarnSubscriptionsResponse {
    pub(crate) subscriptions: Vec<EarnSubscriptionResponse>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct EarnSubscriptionResponse {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    pub(crate) product_id: u64,
    pub(crate) asset_id: u64,
    pub(crate) amount: BigDecimal,
    pub(crate) apr_rate: BigDecimal,
    pub(crate) redemption_fee_rate: BigDecimal,
    pub(crate) maturity_profit_fee_rate: BigDecimal,
    pub(crate) early_redeem_fee_basis: String,
    pub(crate) early_redeem_fee_rate: BigDecimal,
    pub(crate) term_days: u32,
    pub(crate) status: String,
    pub(crate) idempotency_key: String,
    #[serde(with = "unix_millis")]
    pub(crate) subscribed_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) matures_at: DateTime<Utc>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) redeemed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SubscribeEarnResponse {
    pub(crate) subscription: EarnSubscriptionResponse,
}

#[derive(Debug, Serialize)]
pub(crate) struct RedeemEarnResponse {
    pub(crate) subscription: EarnSubscriptionResponse,
    pub(crate) principal_amount: BigDecimal,
    pub(crate) gross_yield_amount: BigDecimal,
    pub(crate) yield_amount: BigDecimal,
    pub(crate) redemption_fee_amount: BigDecimal,
    pub(crate) maturity_profit_fee_amount: BigDecimal,
    pub(crate) early_redeem_fee_amount: BigDecimal,
    pub(crate) fee_amount: BigDecimal,
    pub(crate) redeem_amount: BigDecimal,
}
