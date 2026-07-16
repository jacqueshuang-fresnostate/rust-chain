//! new_coin bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。

use crate::{
    modules::new_coin::repository::{
        NewCoinDistributionRead, NewCoinProjectRead, NewCoinPurchaseRead, NewCoinSubscriptionRead,
        NewCoinUnlockRead,
    },
    time::{option_unix_millis, unix_millis},
};
use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub(crate) struct ListQuery {
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinProjectResponse {
    pub(crate) id: u64,
    pub(crate) asset_id: u64,
    pub(crate) symbol: String,
    pub(crate) lifecycle_status: String,
    pub(crate) total_supply: BigDecimal,
    pub(crate) issue_price: BigDecimal,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) listed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub(crate) unlock_type: String,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) fixed_unlock_at: Option<chrono::DateTime<chrono::Utc>>,
    pub(crate) relative_unlock_seconds: Option<u64>,
    pub(crate) unlock_fee_enabled: bool,
    pub(crate) unlock_fee_rate: Option<BigDecimal>,
    pub(crate) unlock_fee_basis: Option<String>,
    pub(crate) unlock_fee_asset: Option<u64>,
    pub(crate) post_listing_purchase_enabled: bool,
    pub(crate) post_listing_pair_id: Option<u64>,
    pub(crate) status: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinProjectsResponse {
    pub(crate) projects: Vec<NewCoinProjectResponse>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateSubscriptionRequest {
    pub(crate) quote_asset_id: u64,
    pub(crate) quote_amount: BigDecimal,
    pub(crate) quantity: BigDecimal,
    pub(crate) idempotency_key: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreatePurchaseRequest {
    pub(crate) pair_id: u64,
    pub(crate) price: BigDecimal,
    pub(crate) quantity: BigDecimal,
    pub(crate) idempotency_key: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinOrderCreationResponse {
    pub(crate) idempotency_key: String,
    pub(crate) status: String,
    pub(crate) lock_position_id: Option<u64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinSubscriptionResponse {
    pub(crate) id: u64,
    pub(crate) project_id: u64,
    pub(crate) user_id: u64,
    pub(crate) quote_asset: u64,
    pub(crate) quote_amount: BigDecimal,
    pub(crate) requested_quantity: BigDecimal,
    pub(crate) allocated_quantity: BigDecimal,
    pub(crate) status: String,
    pub(crate) idempotency_key: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinSubscriptionsResponse {
    pub(crate) subscriptions: Vec<NewCoinSubscriptionResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinDistributionResponse {
    pub(crate) id: u64,
    pub(crate) project_id: u64,
    pub(crate) user_id: u64,
    pub(crate) subscription_id: Option<u64>,
    pub(crate) asset_id: u64,
    pub(crate) quantity: BigDecimal,
    pub(crate) lock_position_id: Option<u64>,
    pub(crate) status: String,
    pub(crate) idempotency_key: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinDistributionsResponse {
    pub(crate) distributions: Vec<NewCoinDistributionResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinPurchaseResponse {
    pub(crate) id: u64,
    pub(crate) project_id: u64,
    pub(crate) user_id: u64,
    pub(crate) pair_id: u64,
    pub(crate) base_asset: u64,
    pub(crate) quote_asset: u64,
    pub(crate) price: BigDecimal,
    pub(crate) quantity: BigDecimal,
    pub(crate) quote_amount: BigDecimal,
    pub(crate) lock_position_id: Option<u64>,
    pub(crate) status: String,
    pub(crate) idempotency_key: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinPurchasesResponse {
    pub(crate) purchases: Vec<NewCoinPurchaseResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinUnlockResponse {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    pub(crate) asset_id: u64,
    pub(crate) lock_position_id: u64,
    pub(crate) unlock_quantity: BigDecimal,
    pub(crate) unlock_price: Option<BigDecimal>,
    pub(crate) unlock_fee_enabled: bool,
    pub(crate) unlock_fee_rate: Option<BigDecimal>,
    pub(crate) unlock_fee_basis: Option<String>,
    pub(crate) unlock_fee_asset: Option<u64>,
    pub(crate) unlock_fee_amount: Option<BigDecimal>,
    pub(crate) fee_paid_status: String,
    pub(crate) status: String,
    pub(crate) idempotency_key: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinUnlocksResponse {
    pub(crate) unlocks: Vec<NewCoinUnlockResponse>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PayUnlockFeeRequest {
    pub(crate) payment_asset_id: u64,
    pub(crate) amount: BigDecimal,
}

#[derive(Debug, Serialize)]
pub(crate) struct PayUnlockFeeResponse {
    pub(crate) unlock_idempotency_key: String,
    pub(crate) paid: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct ReleaseUnlockResponse {
    pub(crate) unlock_idempotency_key: String,
    pub(crate) released: bool,
}

impl From<NewCoinProjectRead> for NewCoinProjectResponse {
    fn from(read: NewCoinProjectRead) -> Self {
        Self {
            id: read.id,
            asset_id: read.asset_id,
            symbol: read.symbol,
            lifecycle_status: read.lifecycle_status,
            total_supply: read.total_supply,
            issue_price: read.issue_price,
            listed_at: read.listed_at,
            unlock_type: read.unlock_type,
            fixed_unlock_at: read.fixed_unlock_at,
            relative_unlock_seconds: read.relative_unlock_seconds,
            unlock_fee_enabled: read.unlock_fee_enabled,
            unlock_fee_rate: read.unlock_fee_rate,
            unlock_fee_basis: read.unlock_fee_basis,
            unlock_fee_asset: read.unlock_fee_asset,
            post_listing_purchase_enabled: read.post_listing_purchase_enabled,
            post_listing_pair_id: read.post_listing_pair_id,
            status: read.status,
        }
    }
}

impl From<NewCoinSubscriptionRead> for NewCoinSubscriptionResponse {
    fn from(read: NewCoinSubscriptionRead) -> Self {
        Self {
            id: read.id,
            project_id: read.project_id,
            user_id: read.user_id,
            quote_asset: read.quote_asset,
            quote_amount: read.quote_amount,
            requested_quantity: read.requested_quantity,
            allocated_quantity: read.allocated_quantity,
            status: read.status,
            idempotency_key: read.idempotency_key,
            created_at: read.created_at,
        }
    }
}

impl From<NewCoinDistributionRead> for NewCoinDistributionResponse {
    fn from(read: NewCoinDistributionRead) -> Self {
        Self {
            id: read.id,
            project_id: read.project_id,
            user_id: read.user_id,
            subscription_id: read.subscription_id,
            asset_id: read.asset_id,
            quantity: read.quantity,
            lock_position_id: read.lock_position_id,
            status: read.status,
            idempotency_key: read.idempotency_key,
            created_at: read.created_at,
        }
    }
}

impl From<NewCoinPurchaseRead> for NewCoinPurchaseResponse {
    fn from(read: NewCoinPurchaseRead) -> Self {
        Self {
            id: read.id,
            project_id: read.project_id,
            user_id: read.user_id,
            pair_id: read.pair_id,
            base_asset: read.base_asset,
            quote_asset: read.quote_asset,
            price: read.price,
            quantity: read.quantity,
            quote_amount: read.quote_amount,
            lock_position_id: read.lock_position_id,
            status: read.status,
            idempotency_key: read.idempotency_key,
            created_at: read.created_at,
        }
    }
}

impl From<NewCoinUnlockRead> for NewCoinUnlockResponse {
    fn from(read: NewCoinUnlockRead) -> Self {
        Self {
            id: read.id,
            user_id: read.user_id,
            asset_id: read.asset_id,
            lock_position_id: read.lock_position_id,
            unlock_quantity: read.unlock_quantity,
            unlock_price: read.unlock_price,
            unlock_fee_enabled: read.unlock_fee_enabled,
            unlock_fee_rate: read.unlock_fee_rate,
            unlock_fee_basis: read.unlock_fee_basis,
            unlock_fee_asset: read.unlock_fee_asset,
            unlock_fee_amount: read.unlock_fee_amount,
            fee_paid_status: read.fee_paid_status,
            status: read.status,
            idempotency_key: read.idempotency_key,
            created_at: read.created_at,
        }
    }
}
