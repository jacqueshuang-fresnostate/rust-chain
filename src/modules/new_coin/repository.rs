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
    /// 记录中的两个增量之和等于购买数量，实现方必须同时应用，不得只写其一。
    /// 幂等以订单标识为准，同一订单重复保存不得二次改动余额，
    /// 也不得写出第二条购买记录或重复的锁仓来源。
    fn save_post_listing_purchase(
        &mut self,
        record: PostListingPurchaseRecord,
    ) -> Result<(), NewCoinRepositoryError>;
}

pub trait UnlockFeeRepository {
    /// 保存解锁费支付；实现方须把钱包扣款、流水与 paid 状态置位放在同一事务并阻止重复收费。
    /// 支付资产与金额已由调用方与解禁记录的收费快照比对通过，实现方不必重复校验口径，
    /// 但必须自行保证同一解禁记录不会被扣第二次费。
    /// 缴费时间取记录携带的时刻而非当前时钟，以支持补录历史缴费。
    fn save_unlock_fee_payment(
        &mut self,
        record: UnlockFeePaymentRecord,
    ) -> Result<(), NewCoinRepositoryError>;

    /// 查询指定用户解锁记录是否已有费用支付；结果只用于领域放行，不得跨用户读取。
    /// 用户标识必须参与查询条件而非事后过滤，否则会让持他人解锁编号的请求读到真实缴费状态。
    /// 记录不存在时应返回未缴费而不是报错，由后续的放行判定统一给出缴费要求错误。
    /// 本查询只回答缴费与否，不涉及应收金额，也不承担到期判定。
    fn unlock_fee_paid(
        &self,
        unlock_id: &str,
        user_id: &str,
    ) -> Result<bool, NewCoinRepositoryError>;

    /// 标记解锁释放；实现方须锁定解锁和钱包，在同一事务扣减锁仓、增加可用额并写资金流水。
    /// 资金流向固定为锁仓额转可用额，等额一进一出，不经过冻结中转，也不改变资产总量。
    /// 缴费放行已由领域侧判定，但解禁时点是否已到必须由实现方在事务内自行校验，
    /// 因为只有加锁重读才能避免并发下按过期状态放行。
    /// 重复释放同一记录不得二次入账，实现方须以解锁标识作为幂等依据。
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
    pub(crate) name: Option<String>,
    pub(crate) logo_url: Option<String>,
    pub(crate) lifecycle_status: String,
    pub(crate) total_supply: BigDecimal,
    pub(crate) issue_price: BigDecimal,
    pub(crate) quote_asset_id: Option<u64>,
    pub(crate) quote_asset_symbol: Option<String>,
    pub(crate) quote_asset_logo_url: Option<String>,
    pub(crate) reserved_supply: BigDecimal,
    pub(crate) allocated_supply: BigDecimal,
    pub(crate) remaining_supply: BigDecimal,
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
    pub(crate) issue_price: BigDecimal,
    pub(crate) quote_amount: BigDecimal,
    pub(crate) settlement_mode: String,
    pub(crate) frozen_quote_amount: BigDecimal,
    pub(crate) settled_quote_amount: Option<BigDecimal>,
    pub(crate) refunded_quote_amount: Option<BigDecimal>,
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
    pub(crate) status: String,
    pub(crate) lifecycle_status: String,
    pub(crate) total_supply: BigDecimal,
    pub(crate) issue_price: BigDecimal,
    pub(crate) quote_asset_id: Option<u64>,
    pub(crate) reserved_supply: BigDecimal,
    pub(crate) allocated_supply: BigDecimal,
    pub(crate) remaining_supply: BigDecimal,
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
    pub(crate) listing_project_id: Option<u64>,
    pub(crate) user_id: u64,
    pub(crate) asset_id: u64,
    pub(crate) unlock_type: String,
    pub(crate) unlock_at: chrono::DateTime<chrono::Utc>,
    pub(crate) amount: BigDecimal,
    pub(crate) merge_key: String,
    pub(crate) source_time: chrono::DateTime<chrono::Utc>,
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
    pub(crate) request_fingerprint: String,
}

#[derive(Debug, Clone)]
pub(crate) struct NewCoinPurchaseOrderWrite {
    pub(crate) user_id: u64,
    pub(crate) project: NewCoinProjectRuleRead,
    pub(crate) pair_id: u64,
    pub(crate) price: BigDecimal,
    pub(crate) quantity: BigDecimal,
    pub(crate) idempotency_key: String,
    pub(crate) request_fingerprint: String,
}

/// 新币下单事务的最终结果；`created=false` 表示同参数幂等重放。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NewCoinOrderWriteOutcome {
    pub(crate) project_id: u64,
    pub(crate) asset_id: u64,
    pub(crate) quote_asset_id: u64,
    pub(crate) lock_position_id: Option<u64>,
    pub(crate) status: String,
    pub(crate) authoritative_price: BigDecimal,
    pub(crate) authoritative_quote_amount: BigDecimal,
    pub(crate) created: bool,
}

#[async_trait]
pub(crate) trait NewCoinReadRepository: Clone + Send + Sync + 'static {
    /// 按上限读取公开启用项目，返回后台配置的生命周期、解锁规则及上市后购买交易对。
    /// 结果面向所有访客，实现方不得按调用者身份裁剪字段，也不得包含停用或草稿项目。
    /// `limit` 由调用方预先夹取过，实现方直接应用即可，无需再次校验取值范围。
    async fn list_active_projects(&self, limit: u32) -> AppResult<Vec<NewCoinProjectRead>>;

    /// 按符号读取公开启用项目；停用或不存在返回 `None`，不得回退到后台草稿。
    /// 返回的字段集合必须与列表查询一致，使详情页与列表页展示同一份配置口径。
    /// 符号在启用项目中视为唯一，实现方遇到重复数据应取确定的一条而不是报错。
    async fn find_active_project_by_symbol(
        &self,
        symbol: &str,
    ) -> AppResult<Option<NewCoinProjectRead>>;

    /// 读取指定用户的申购订单，必须以 `user_id` 隔离并应用条数上限。
    /// 隔离条件要进入查询本身而非在内存中过滤，避免把他人订单读进进程再丢弃。
    /// 每条订单需同时给出申请数量与实际配额数量，实现方不得用其一覆盖另一个。
    /// 这是只读契约，实现方不得在查询过程中推进订单状态或重算配额。
    async fn list_user_subscriptions(
        &self,
        user_id: u64,
        limit: u32,
    ) -> AppResult<Vec<NewCoinSubscriptionRead>>;

    /// 读取指定用户的分发记录，不在查询时重新计算或推进分发状态。
    /// 关联申购单与锁仓位置两个字段允许为空，实现方须原样保留空值：
    /// 前者为空表示分发不来自申购流程，后者为空表示资产当时直接进入了可用余额。
    /// 实现方不得因引用的锁仓已释放就隐藏该条分发记录。
    async fn list_user_distributions(
        &self,
        user_id: u64,
        limit: u32,
    ) -> AppResult<Vec<NewCoinDistributionRead>>;

    /// 读取指定用户的上市后购买记录，结果必须保留订单时价格、数量与计价金额快照。
    /// 快照要按落库原值返回，实现方不得用当前行情或最新项目配置重算其中任何一项，
    /// 否则历史订单将无法与资金流水对账。
    /// 锁仓位置为空表示该笔买入按规则无需锁仓，实现方不得把它折叠为零。
    async fn list_user_purchases(
        &self,
        user_id: u64,
        limit: u32,
    ) -> AppResult<Vec<NewCoinPurchaseRead>>;

    /// 读取指定用户的锁仓解禁记录及费用状态，不执行缴费或释放。
    /// 需一并返回该批次固化的收费口径，即是否收费、费率、计费基准、支付资产与应付金额，
    /// 这些是分配当时写死的快照，实现方不得按项目最新费率重算。
    /// 缴费状态与释放状态是两个独立字段，实现方须分别如实返回，不得合并或相互推断。
    async fn list_user_unlocks(
        &self,
        user_id: u64,
        limit: u32,
    ) -> AppResult<Vec<NewCoinUnlockRead>>;
}

#[async_trait]
pub(crate) trait NewCoinUnlockFeeRepository: Clone + Send + Sync + 'static {
    /// 按解锁幂等键与用户读取是否收费及配置的支付资产、金额；结果不包含当前 paid 状态。
    /// 刻意排除缴费状态是为了让「应收」与「已收」两个概念在类型层面就分开，
    /// 调用方无法凭本结果直接放行释放，必须另行确认缴费。
    /// 记录不存在返回 `None`，调用方据此同时判定记录缺失与越权访问两种情况。
    async fn find_unlock_fee_expectation(
        &self,
        unlock_idempotency_key: &str,
        user_id: u64,
    ) -> AppResult<Option<UnlockFeeExpectation>>;

    /// 在单事务内锁定应收、扣减用户可用余额、写钱包流水与平台收入分录后标记 paid。
    /// 同键重放返回 false；资产、金额不符或余额不足必须整体回滚。
    async fn mark_unlock_fee_paid(&self, payment: UnlockFeePaymentWrite) -> AppResult<bool>;
}

#[async_trait]
pub(crate) trait NewCoinUnlockReleaseRepository: Clone + Send + Sync + 'static {
    /// 锁定并释放指定用户已到期且满足缴费要求的解锁记录；钱包入账、流水和状态须原子提交。
    /// 到期判定与缴费判定都必须在事务内加锁后完成，不得依赖调用方传入的预读结果。
    /// 资金只能从锁仓额转入可用额，等额一进一出，不经过冻结中转。
    /// 返回值的 `released` 用于区分本次真实释放与幂等重放：
    /// 记录已是已释放状态时须返回假值并回吐既有资产与数量，而不是报错或二次入账。
    /// 记录不存在返回 `NotFound`，未到期或未缴费返回校验错误且不得留下部分写入。
    async fn release_due_paid_unlock(
        &self,
        unlock_idempotency_key: &str,
        user_id: u64,
    ) -> AppResult<ReleaseUnlockOutcome>;
}

#[async_trait]
pub(crate) trait NewCoinOrderRepository: Clone + Send + Sync + 'static {
    /// 按项目符号读取完整下单规则，包括生命周期、购买开关、批准交易对和解锁费配置。
    /// 结果供下单前解析项目主键和生成请求指纹，实现方不必在此加锁；
    /// 真正扣款前必须由下单事务内的加锁重读再确认一次，避免按过期配置成交。
    /// 必须保留停用项目，使已经成功的订单在项目停用后仍可按同键同参回放；
    /// 新请求是否允许成交由事务内的 active 与生命周期守卫决定。
    async fn find_project_rule_by_symbol(
        &self,
        symbol: &str,
    ) -> AppResult<Option<NewCoinProjectRuleRead>>;

    /// 创建申购订单；事务内重新锁项目，核对计价资产、发行价与剩余供给后原子扣款分配。
    /// 同键同指纹回吐原结果，同键异指纹返回 Conflict，重放不重复扣款或占用供给。
    async fn create_subscription_order(
        &self,
        order: NewCoinSubscriptionOrderWrite,
    ) -> AppResult<NewCoinOrderWriteOutcome>;

    /// 创建上市后购买订单；事务内锁定项目与交易对，仅接受项目发行价和绑定计价资产。
    /// 供给预留、钱包扣款、新币入账与供给确认共享事务；幂等重放语义与申购一致。
    async fn create_purchase_order(
        &self,
        order: NewCoinPurchaseOrderWrite,
    ) -> AppResult<NewCoinOrderWriteOutcome>;
}
