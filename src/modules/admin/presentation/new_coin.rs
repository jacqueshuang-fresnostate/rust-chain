//! 后台新币项目、分配、购买、锁仓与解锁 DTO。

use super::*;

#[derive(Debug, Deserialize)]
pub(crate) struct AdminNewCoinProjectQuery {
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminNewCoinProjectQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminNewCoinScopedListQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminNewCoinScopedListQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminNewCoinFlatListQuery {
    pub(crate) project_id: Option<u64>,
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminNewCoinFlatListQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminNewCoinPurchaseQuery {
    pub(crate) project_id: Option<u64>,
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminNewCoinPurchaseQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminNewCoinLockPositionQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) asset_id: Option<u64>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminNewCoinLockPositionQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminNewCoinUnlockQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) asset_id: Option<u64>,
    pub(crate) status: Option<String>,
    pub(crate) fee_paid_status: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminNewCoinUnlockQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateNewCoinProjectRequest {
    pub(crate) asset_id: u64,
    pub(crate) symbol: String,
    pub(crate) lifecycle_status: String,
    pub(crate) total_supply: BigDecimal,
    pub(crate) issue_price: BigDecimal,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) listed_at: Option<DateTime<Utc>>,
    pub(crate) unlock_type: String,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) fixed_unlock_at: Option<DateTime<Utc>>,
    pub(crate) relative_unlock_seconds: Option<u64>,
    pub(crate) unlock_fee_enabled: Option<bool>,
    pub(crate) unlock_fee_rate: Option<BigDecimal>,
    pub(crate) unlock_fee_basis: Option<String>,
    pub(crate) unlock_fee_asset: Option<u64>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateNewCoinProjectRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateNewCoinLifecycleRequest {
    pub(crate) lifecycle_status: String,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) listed_at: Option<DateTime<Utc>>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateNewCoinLifecycleRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct DistributeNewCoinRequest {
    pub(crate) user_id: u64,
    pub(crate) subscription_id: Option<u64>,
    pub(crate) quantity: BigDecimal,
    pub(crate) idempotency_key: String,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for DistributeNewCoinRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateNewCoinUnlockRuleRequest {
    pub(crate) unlock_type: String,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) listed_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) fixed_unlock_at: Option<DateTime<Utc>>,
    pub(crate) relative_unlock_seconds: Option<u64>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateNewCoinUnlockRuleRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateNewCoinUnlockFeeRuleRequest {
    pub(crate) unlock_fee_enabled: bool,
    pub(crate) unlock_fee_rate: Option<BigDecimal>,
    pub(crate) unlock_fee_basis: Option<String>,
    pub(crate) unlock_fee_asset: Option<u64>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateNewCoinUnlockFeeRuleRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateNewCoinPostListingPurchaseRequest {
    pub(crate) enabled: bool,
    pub(crate) pair_id: Option<u64>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateNewCoinPostListingPurchaseRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpsertNewCoinConvertRuleRequest {
    pub(crate) convert_pair_id: u64,
    pub(crate) rate_source: String,
    pub(crate) fixed_rate: Option<BigDecimal>,
    pub(crate) floating_rate_json: Option<Value>,
    pub(crate) status: Option<String>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpsertNewCoinConvertRuleRequest {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct NewCoinProjectResponse {
    pub(crate) id: u64,
    pub(crate) asset_id: u64,
    pub(crate) symbol: String,
    pub(crate) lifecycle_status: String,
    pub(crate) total_supply: BigDecimal,
    pub(crate) issue_price: BigDecimal,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) listed_at: Option<DateTime<Utc>>,
    pub(crate) unlock_type: String,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) fixed_unlock_at: Option<DateTime<Utc>>,
    pub(crate) relative_unlock_seconds: Option<u64>,
    pub(crate) unlock_fee_enabled: bool,
    pub(crate) unlock_fee_rate: Option<BigDecimal>,
    pub(crate) unlock_fee_basis: Option<String>,
    pub(crate) unlock_fee_asset: Option<u64>,
    pub(crate) status: String,
    pub(crate) post_listing_purchase_enabled: bool,
    pub(crate) post_listing_pair_id: Option<u64>,
    pub(crate) post_listing_pair_status: Option<String>,
}

impl PresentationLayer for NewCoinProjectResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinProjectsResponse {
    pub(crate) projects: Vec<NewCoinProjectResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for NewCoinProjectsResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
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
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for NewCoinSubscriptionResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinSubscriptionsResponse {
    pub(crate) subscriptions: Vec<NewCoinSubscriptionResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for NewCoinSubscriptionsResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
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
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for NewCoinDistributionResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinDistributionsResponse {
    pub(crate) distributions: Vec<NewCoinDistributionResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for NewCoinDistributionsResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
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
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for NewCoinPurchaseResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinPurchasesResponse {
    pub(crate) purchases: Vec<NewCoinPurchaseResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for NewCoinPurchasesResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct NewCoinLockPositionResponse {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    pub(crate) asset_id: u64,
    pub(crate) unlock_type: String,
    #[serde(with = "unix_millis")]
    pub(crate) unlock_at: DateTime<Utc>,
    pub(crate) locked_amount: BigDecimal,
    pub(crate) released_amount: BigDecimal,
    pub(crate) remaining_amount: BigDecimal,
    pub(crate) merge_key: String,
    pub(crate) status: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for NewCoinLockPositionResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinLockPositionsResponse {
    pub(crate) lock_positions: Vec<NewCoinLockPositionResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for NewCoinLockPositionsResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
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
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for NewCoinUnlockResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinUnlocksResponse {
    pub(crate) unlocks: Vec<NewCoinUnlockResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for NewCoinUnlocksResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct NewCoinConvertRuleResponse {
    pub(crate) id: u64,
    pub(crate) convert_pair_id: u64,
    pub(crate) rate_source: String,
    pub(crate) fixed_rate: Option<BigDecimal>,
    pub(crate) floating_rate_json: Option<SqlxJson<Value>>,
    pub(crate) status: String,
    pub(crate) created_by: Option<u64>,
}

impl PresentationLayer for NewCoinConvertRuleResponse {}
