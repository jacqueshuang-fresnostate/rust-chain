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
    /// 保存上市后购买记录及其钱包可用/锁仓增量；实现方须原子落地并按订单标识幂等去重。
    fn save_post_listing_purchase(
        &mut self,
        record: PostListingPurchaseRecord,
    ) -> Result<(), NewCoinRepositoryError>;
}

pub trait UnlockFeeRepository {
    /// 保存解锁费支付；实现方须把钱包扣款、流水与 paid 状态置位放在同一事务并阻止重复收费。
    fn save_unlock_fee_payment(
        &mut self,
        record: UnlockFeePaymentRecord,
    ) -> Result<(), NewCoinRepositoryError>;

    /// 查询指定用户解锁记录是否已有费用支付；结果只用于领域放行，不得跨用户读取。
    fn unlock_fee_paid(
        &self,
        unlock_id: &str,
        user_id: &str,
    ) -> Result<bool, NewCoinRepositoryError>;

    /// 标记解锁释放；实现方须锁定解锁和钱包，在同一事务扣减锁仓、增加可用额并写资金流水。
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
    /// 按上限读取公开启用项目，返回后台配置的生命周期、解锁规则及上市后购买交易对。
    async fn list_active_projects(&self, limit: u32) -> AppResult<Vec<NewCoinProjectRead>>;

    /// 按符号读取公开启用项目；停用或不存在返回 `None`，不得回退到后台草稿。
    async fn find_active_project_by_symbol(
        &self,
        symbol: &str,
    ) -> AppResult<Option<NewCoinProjectRead>>;

    /// 读取指定用户的申购订单，必须以 `user_id` 隔离并应用条数上限。
    async fn list_user_subscriptions(
        &self,
        user_id: u64,
        limit: u32,
    ) -> AppResult<Vec<NewCoinSubscriptionRead>>;

    /// 读取指定用户的分发记录，不在查询时重新计算或推进分发状态。
    async fn list_user_distributions(
        &self,
        user_id: u64,
        limit: u32,
    ) -> AppResult<Vec<NewCoinDistributionRead>>;

    /// 读取指定用户的上市后购买记录，结果必须保留订单时价格、数量与计价金额快照。
    async fn list_user_purchases(
        &self,
        user_id: u64,
        limit: u32,
    ) -> AppResult<Vec<NewCoinPurchaseRead>>;

    /// 读取指定用户的锁仓解锁记录及费用状态，不执行缴费或释放。
    async fn list_user_unlocks(
        &self,
        user_id: u64,
        limit: u32,
    ) -> AppResult<Vec<NewCoinUnlockRead>>;
}

#[async_trait]
pub(crate) trait NewCoinUnlockFeeRepository: Clone + Send + Sync + 'static {
    /// 按解锁幂等键与用户读取是否收费及配置的支付资产、金额；结果不包含当前 paid 状态。
    async fn find_unlock_fee_expectation(
        &self,
        unlock_idempotency_key: &str,
        user_id: u64,
    ) -> AppResult<Option<UnlockFeeExpectation>>;

    /// 把匹配用户的解锁记录从非 paid 更新为 paid；返回值表示本次是否改变一行。
    /// 当前 MySQL 实现不扣钱包、不写资金流水，调用方必须把它视为状态置位而非资金支付。
    async fn mark_unlock_fee_paid(&self, payment: UnlockFeePaymentWrite) -> AppResult<bool>;
}

#[async_trait]
pub(crate) trait NewCoinUnlockReleaseRepository: Clone + Send + Sync + 'static {
    /// 锁定并释放指定用户已到期且满足缴费要求的解锁记录；钱包入账、流水和状态须原子提交。
    async fn release_due_paid_unlock(
        &self,
        unlock_idempotency_key: &str,
        user_id: u64,
    ) -> AppResult<ReleaseUnlockOutcome>;
}

#[async_trait]
pub(crate) trait NewCoinOrderRepository: Clone + Send + Sync + 'static {
    /// 按项目符号读取完整下单规则，包括生命周期、购买开关、批准交易对和解锁费配置。
    async fn find_project_rule_by_symbol(
        &self,
        symbol: &str,
    ) -> AppResult<Option<NewCoinProjectRuleRead>>;

    /// 读取指定交易对并确认其基础资产等于项目资产；不匹配时返回 `None`。
    async fn find_pair_for_purchase(
        &self,
        pair_id: u64,
        project_asset_id: u64,
    ) -> AppResult<Option<NewCoinPairRead>>;

    /// 创建申购订单；当前 MySQL 实现在事务中锁定计价钱包，原子扣款、写流水、分配资产并 upsert 锁仓。
    /// 项目规则在事务前读取且不会重新锁定；重复幂等键返回 Conflict，返回值是首个锁仓位置编号或 `None`（直接到账）。
    async fn create_subscription_order(
        &self,
        order: NewCoinSubscriptionOrderWrite,
    ) -> AppResult<Option<u64>>;

    /// 创建上市后购买订单；当前 MySQL 实现锁定项目、交易对和钱包并原子落地扣款、流水、分配与锁仓。
    /// 任意重复幂等键均返回 Conflict，不比较重放参数；返回值是首个锁仓位置编号或 `None`（直接到账）。
    async fn create_purchase_order(
        &self,
        order: NewCoinPurchaseOrderWrite,
    ) -> AppResult<Option<u64>>;
}
