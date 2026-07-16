//! new_coin bounded context repository layer.
//!
//! 仓储层：定义持久化边界、仓储接口和面向领域的读写契约。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的仓储契约逐步迁入。

use super::domain::NewCoinOrderKind;
use crate::{architecture::RepositoryLayer, error::AppResult, modules::wallet::LockPosition};
use axum::async_trait;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use sqlx::{MySql, Pool};

#[derive(Debug)]
pub struct RepositoryLayerMarker;

impl RepositoryLayer for RepositoryLayerMarker {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewCoinRepositoryError {
    Storage(String),
    InvalidStatus(String),
}

impl From<sqlx::Error> for NewCoinRepositoryError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error.to_string())
    }
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
pub struct MySqlNewCoinRepository {
    pool: Pool<MySql>,
}

impl MySqlNewCoinRepository {
    pub fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &Pool<MySql> {
        &self.pool
    }

    pub async fn insert_purchase_order(
        &self,
        order: NewCoinPurchaseOrderInsert,
    ) -> Result<NewCoinPurchaseOrderInsertResult, NewCoinRepositoryError> {
        let insert_result = sqlx::query(
            r#"INSERT INTO new_coin_purchase_orders
               (project_id, user_id, pair_id, base_asset, quote_asset, price, quantity,
                quote_amount, lock_position_id, status, idempotency_key)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON DUPLICATE KEY UPDATE idempotency_key = idempotency_key"#,
        )
        .bind(order.project_id)
        .bind(order.user_id)
        .bind(order.pair_id)
        .bind(order.base_asset_id)
        .bind(order.quote_asset_id)
        .bind(order.price)
        .bind(order.quantity)
        .bind(order.quote_amount)
        .bind(order.lock_position_id)
        .bind(order.status)
        .bind(&order.idempotency_key)
        .execute(&self.pool)
        .await?;

        let order_id = insert_result.last_insert_id();
        Ok(NewCoinPurchaseOrderInsertResult {
            order_id: if order_id == 0 {
                self.purchase_order_id(&order.idempotency_key).await?
            } else {
                order_id
            },
            inserted: order_id != 0,
        })
    }

    pub async fn unlock_fee_paid_status(
        &self,
        unlock_idempotency_key: &str,
        user_id: u64,
    ) -> Result<Option<UnlockFeePaidStatus>, NewCoinRepositoryError> {
        let row = sqlx::query_as::<_, (String,)>(
            r#"SELECT fee_paid_status
               FROM asset_unlock_records
               WHERE idempotency_key = ? AND user_id = ?
               LIMIT 1"#,
        )
        .bind(unlock_idempotency_key)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|(status,)| UnlockFeePaidStatus::from_storage(&status))
            .transpose()
    }

    pub async fn mark_unlock_fee_paid(
        &self,
        payment: UnlockFeePaymentUpdate,
    ) -> Result<bool, NewCoinRepositoryError> {
        let result = sqlx::query(
            r#"UPDATE asset_unlock_records
               SET fee_paid_status = 'paid',
                   unlock_fee_asset = ?,
                   unlock_fee_amount = ?
               WHERE idempotency_key = ?
                 AND user_id = ?
                 AND fee_paid_status <> 'paid'"#,
        )
        .bind(payment.payment_asset_id)
        .bind(payment.amount)
        .bind(payment.unlock_idempotency_key)
        .bind(payment.user_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() == 1)
    }

    async fn purchase_order_id(
        &self,
        idempotency_key: &str,
    ) -> Result<u64, NewCoinRepositoryError> {
        let row = sqlx::query_as::<_, (u64,)>(
            "SELECT id FROM new_coin_purchase_orders WHERE idempotency_key = ? LIMIT 1",
        )
        .bind(idempotency_key)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }
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

impl UnlockFeePaidStatus {
    fn from_storage(value: &str) -> Result<Self, NewCoinRepositoryError> {
        match value {
            "not_required" => Ok(Self::NotRequired),
            "pending" => Ok(Self::Pending),
            "paid" => Ok(Self::Paid),
            _ => Err(NewCoinRepositoryError::InvalidStatus(value.to_owned())),
        }
    }
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
