//! new_coin bounded context repository layer.
//!
//! 仓储层：定义持久化边界、仓储接口和面向领域的读写契约。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的仓储契约逐步迁入。

use super::domain::NewCoinOrderKind;
use crate::{error::AppResult, modules::wallet::LockPosition};
use axum::async_trait;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewCoinRepositoryError {
    Storage(String),
    InvalidStatus(String),
}

#[derive(Debug, Clone)]
pub struct WalletLockCommandOutput {
    pub user_id: String,
    pub asset_id: String,
    pub available_delta: BigDecimal,
    pub locked_delta: BigDecimal,
    pub lock_positions: Vec<LockPosition>,
}

#[derive(Debug, Clone)]
pub struct PostListingPurchaseRecord {
    pub project_id: String,
    pub order_id: String,
    pub user_id: String,
    pub asset_id: String,
    pub quantity: BigDecimal,
    pub order_kind: NewCoinOrderKind,
    pub purchased_at: DateTime<Utc>,
    pub wallet_lock: WalletLockCommandOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockFeePaymentRecord {
    pub unlock_id: String,
    pub user_id: String,
    pub payment_asset: String,
    pub amount: BigDecimal,
    pub paid_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockReleaseRecord {
    pub unlock_id: String,
    pub user_id: String,
    pub asset_id: String,
    pub released_at: DateTime<Utc>,
}

// 兼容旧同步服务测试的仓储契约；新路由侧事务仓储继续使用下面的异步 trait。
pub trait NewCoinPurchaseRepository {
    fn save_post_listing_purchase(
        &mut self,
        record: PostListingPurchaseRecord,
    ) -> Result<(), NewCoinRepositoryError>;
}

pub trait UnlockFeeRepository {
    fn save_unlock_fee_payment(
        &mut self,
        record: UnlockFeePaymentRecord,
    ) -> Result<(), NewCoinRepositoryError>;

    fn unlock_fee_paid(
        &self,
        unlock_id: &str,
        user_id: &str,
    ) -> Result<bool, NewCoinRepositoryError>;

    fn mark_unlock_released(
        &mut self,
        record: UnlockReleaseRecord,
    ) -> Result<(), NewCoinRepositoryError>;
}

#[derive(Debug, Clone)]
pub struct NewCoinPurchaseOrderInsert {
    pub project_id: u64,
    pub user_id: u64,
    pub pair_id: u64,
    pub base_asset_id: u64,
    pub quote_asset_id: u64,
    pub price: BigDecimal,
    pub quantity: BigDecimal,
    pub quote_amount: BigDecimal,
    pub lock_position_id: Option<u64>,
    pub status: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCoinPurchaseOrderInsertResult {
    pub order_id: u64,
    pub inserted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnlockFeePaidStatus {
    NotRequired,
    Pending,
    Paid,
}

#[derive(Debug, Clone)]
pub struct UnlockFeePaymentUpdate {
    pub unlock_idempotency_key: String,
    pub user_id: u64,
    pub payment_asset_id: u64,
    pub amount: BigDecimal,
}

#[derive(Debug, Clone)]
pub(crate) struct NewCoinProjectRead {
    pub(crate) id: u64,
    pub(crate) asset_id: u64,
    pub(crate) symbol: String,
    pub(crate) lifecycle_status: String,
    pub(crate) total_supply: BigDecimal,
    pub(crate) issue_price: BigDecimal,
    pub(crate) listed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub(crate) unlock_type: String,
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

#[derive(Debug, Clone)]
pub(crate) struct NewCoinSubscriptionRead {
    pub(crate) id: u64,
    pub(crate) project_id: u64,
    pub(crate) user_id: u64,
    pub(crate) quote_asset: u64,
    pub(crate) quote_amount: BigDecimal,
    pub(crate) requested_quantity: BigDecimal,
    pub(crate) allocated_quantity: BigDecimal,
    pub(crate) status: String,
    pub(crate) idempotency_key: String,
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub(crate) struct NewCoinDistributionRead {
    pub(crate) id: u64,
    pub(crate) project_id: u64,
    pub(crate) user_id: u64,
    pub(crate) subscription_id: Option<u64>,
    pub(crate) asset_id: u64,
    pub(crate) quantity: BigDecimal,
    pub(crate) lock_position_id: Option<u64>,
    pub(crate) status: String,
    pub(crate) idempotency_key: String,
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub(crate) struct NewCoinPurchaseRead {
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
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub(crate) struct NewCoinUnlockRead {
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
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub(crate) struct UnlockFeeExpectation {
    pub(crate) unlock_fee_enabled: bool,
    pub(crate) unlock_fee_asset: Option<u64>,
    pub(crate) unlock_fee_amount: Option<BigDecimal>,
}

#[derive(Debug, Clone)]
pub(crate) struct UnlockFeePaymentWrite {
    pub(crate) unlock_idempotency_key: String,
    pub(crate) user_id: u64,
    pub(crate) payment_asset_id: u64,
    pub(crate) amount: BigDecimal,
}

#[derive(Debug, Clone)]
pub(crate) struct ReleaseUnlockOutcome {
    pub(crate) asset_id: u64,
    pub(crate) unlock_quantity: BigDecimal,
    pub(crate) released: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct NewCoinProjectRuleRead {
    pub(crate) id: u64,
    pub(crate) asset_id: u64,
    pub(crate) lifecycle_status: String,
    pub(crate) issue_price: BigDecimal,
    pub(crate) listed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub(crate) unlock_type: String,
    pub(crate) fixed_unlock_at: Option<chrono::DateTime<chrono::Utc>>,
    pub(crate) relative_unlock_seconds: Option<u64>,
    pub(crate) unlock_fee_enabled: bool,
    pub(crate) unlock_fee_rate: Option<BigDecimal>,
    pub(crate) unlock_fee_basis: Option<String>,
    pub(crate) unlock_fee_asset: Option<u64>,
    pub(crate) post_listing_purchase_enabled: bool,
    pub(crate) post_listing_pair_id: Option<u64>,
}

#[derive(Debug, Clone)]
pub(crate) struct NewCoinPairRead {
    pub(crate) base_asset_id: u64,
    pub(crate) quote_asset_id: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct NewCoinWalletRead {
    pub(crate) available: BigDecimal,
    pub(crate) frozen: BigDecimal,
    pub(crate) locked: BigDecimal,
}

#[derive(Debug, Clone)]
pub(crate) struct NewCoinLockPositionWrite {
    pub(crate) user_id: u64,
    pub(crate) asset_id: u64,
    pub(crate) unlock_type: String,
    pub(crate) unlock_at: chrono::DateTime<chrono::Utc>,
    pub(crate) amount: BigDecimal,
    pub(crate) merge_key: String,
    pub(crate) source_type: String,
    pub(crate) source_id: String,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct NewCoinLedgerMetadata<'a> {
    pub(crate) change_type: &'a str,
    pub(crate) ref_type: &'a str,
    pub(crate) ref_id: &'a str,
}

#[derive(Debug, Clone)]
pub(crate) struct NewCoinSubscriptionOrderWrite {
    pub(crate) user_id: u64,
    pub(crate) project: NewCoinProjectRuleRead,
    pub(crate) quote_asset_id: u64,
    pub(crate) quote_amount: BigDecimal,
    pub(crate) quantity: BigDecimal,
    pub(crate) idempotency_key: String,
    pub(crate) lock_positions: Vec<NewCoinLockPositionWrite>,
}

#[derive(Debug, Clone)]
pub(crate) struct NewCoinPurchaseOrderWrite {
    pub(crate) user_id: u64,
    pub(crate) project: NewCoinProjectRuleRead,
    pub(crate) pair_id: u64,
    pub(crate) price: BigDecimal,
    pub(crate) quantity: BigDecimal,
    pub(crate) quote_amount: BigDecimal,
    pub(crate) idempotency_key: String,
}

#[async_trait]
pub(crate) trait NewCoinReadRepository: Clone + Send + Sync + 'static {
    async fn list_active_projects(&self, limit: u32) -> AppResult<Vec<NewCoinProjectRead>>;

    async fn find_active_project_by_symbol(
        &self,
        symbol: &str,
    ) -> AppResult<Option<NewCoinProjectRead>>;

    async fn list_user_subscriptions(
        &self,
        user_id: u64,
        limit: u32,
    ) -> AppResult<Vec<NewCoinSubscriptionRead>>;

    async fn list_user_distributions(
        &self,
        user_id: u64,
        limit: u32,
    ) -> AppResult<Vec<NewCoinDistributionRead>>;

    async fn list_user_purchases(
        &self,
        user_id: u64,
        limit: u32,
    ) -> AppResult<Vec<NewCoinPurchaseRead>>;

    async fn list_user_unlocks(
        &self,
        user_id: u64,
        limit: u32,
    ) -> AppResult<Vec<NewCoinUnlockRead>>;
}

#[async_trait]
pub(crate) trait NewCoinUnlockFeeRepository: Clone + Send + Sync + 'static {
    async fn find_unlock_fee_expectation(
        &self,
        unlock_idempotency_key: &str,
        user_id: u64,
    ) -> AppResult<Option<UnlockFeeExpectation>>;

    async fn mark_unlock_fee_paid(&self, payment: UnlockFeePaymentWrite) -> AppResult<bool>;
}

#[async_trait]
pub(crate) trait NewCoinUnlockReleaseRepository: Clone + Send + Sync + 'static {
    async fn release_due_paid_unlock(
        &self,
        unlock_idempotency_key: &str,
        user_id: u64,
    ) -> AppResult<ReleaseUnlockOutcome>;
}

#[async_trait]
pub(crate) trait NewCoinOrderRepository: Clone + Send + Sync + 'static {
    async fn find_project_rule_by_symbol(
        &self,
        symbol: &str,
    ) -> AppResult<Option<NewCoinProjectRuleRead>>;

    async fn find_pair_for_purchase(
        &self,
        pair_id: u64,
        project_asset_id: u64,
    ) -> AppResult<Option<NewCoinPairRead>>;

    async fn create_subscription_order(
        &self,
        order: NewCoinSubscriptionOrderWrite,
    ) -> AppResult<Option<u64>>;

    async fn create_purchase_order(
        &self,
        order: NewCoinPurchaseOrderWrite,
    ) -> AppResult<Option<u64>>;
}
