//! wallet bounded context domain layer.
//!
//! 领域层：放置钱包领域实体、值对象和不依赖 I/O 的业务规则。
//! 核心不变量是 available、frozen、locked 三桶各自非负，任一桶变负即整次领域变更作废，账户快照保持原值。
//! 资金精度统一按资产 precision_scale 向零截断，最大 18 位；提现手续费阶梯必须无重叠且开放区间只能收尾。
//! 账本条目由已应用变更的账户快照反向生成，因此每条流水的三桶 after 描述的都是同一时刻的账后状态。
//! 本层为纯函数与纯值对象，不访问数据库、不加锁、不保证任何幂等，持久化与并发控制全部由仓储和基础设施承担。

use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// 钱包资产允许的最大小数位（数据库与链上金额展示统一约束）。
pub const MAX_ASSET_PRECISION_SCALE: i32 = 18;

/// 单一用户提现手续费层级上限，避免规则体膨胀。
pub const MAX_WITHDRAW_FEE_TIER_COUNT: usize = 50;

/// 提现手续费阶梯。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WithdrawFeeTier {
    pub min_amount: BigDecimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_amount: Option<BigDecimal>,
    pub fee_rate_percent: BigDecimal,
}

/// 钱包余额区分。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BalanceBucket {
    Available,
    Frozen,
    Locked,
}

/// 钱包领域错误：余额更新、锁仓创建等规则级错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WalletDomainError {
    NegativeBalance {
        bucket: BalanceBucket,
    },
    NonPositiveLockAmount,
    LockedBalanceInvariantMismatch {
        account_locked: BigDecimal,
        active_positions_remaining: BigDecimal,
    },
}

/// 用户钱包账户快照。
#[derive(Debug, Clone)]
pub struct WalletAccount {
    pub user_id: String,
    pub asset_id: String,
    pub available: BigDecimal,
    pub frozen: BigDecimal,
    pub locked: BigDecimal,
}

impl WalletAccount {
    /// 按变更量同时计算可用、冻结与锁定三桶的新余额。
    /// 任一余额变成负数即拒绝整次领域变更，账户快照保持原值。
    pub fn apply_balance_change(&mut self, change: BalanceChange) -> Result<(), WalletDomainError> {
        let next_available = self.available.clone() + change.available;
        let next_frozen = self.frozen.clone() + change.frozen;
        let next_locked = self.locked.clone() + change.locked;

        ensure_non_negative(&next_available, BalanceBucket::Available)?;
        ensure_non_negative(&next_frozen, BalanceBucket::Frozen)?;
        ensure_non_negative(&next_locked, BalanceBucket::Locked)?;

        self.available = next_available;
        self.frozen = next_frozen;
        self.locked = next_locked;
        Ok(())
    }
}

/// 余额变更值对象。
#[derive(Debug, Clone)]
pub struct BalanceChange {
    pub available: BigDecimal,
    pub frozen: BigDecimal,
    pub locked: BigDecimal,
}

impl BalanceChange {
    /// 构造包含可用、冻结与锁定三桶增量的余额变更值对象，三个分量都是有符号增量而非目标余额。
    /// 构造阶段不校验正负、不做精度截断、也不要求三桶增量之和为零，合法性完全交给账户应用变更时判定。
    pub fn new(available: BigDecimal, frozen: BigDecimal, locked: BigDecimal) -> Self {
        Self {
            available,
            frozen,
            locked,
        }
    }
}

/// 钱包服务错误：在服务/仓储交互场景中也需要表达的通用错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WalletServiceError {
    Domain(WalletDomainError),
    MissingLedgerMetadata(&'static str),
    NonPositiveAmount,
    Repository(String),
}

impl From<WalletDomainError> for WalletServiceError {
    /// 把领域规则错误原样提升为服务层错误，保留负余额桶或锁仓不变量等原始判定信息。
    /// 该转换只改变错误类型，不吞掉错误、不改变余额快照，也不会补写任何流水。
    fn from(error: WalletDomainError) -> Self {
        Self::Domain(error)
    }
}

/// 判断金额的有效小数位是否不超过资产 precision_scale；尾随零不计入精度。
/// precision_scale 超出 0..=18 时直接返回 false，本函数不截断或修改金额。
pub fn amount_fits_asset_precision(amount: &BigDecimal, precision_scale: i32) -> bool {
    if !(0..=MAX_ASSET_PRECISION_SCALE).contains(&precision_scale) {
        return false;
    }
    asset_amount_fractional_scale(amount) <= precision_scale as u32
}

/// 将金额向零截断到资产 precision_scale；配置越界时钳制到 0..=18。
/// 该纯函数不校验正负，也不写余额；调用方须让订单金额、钱包增量和流水使用同一结果。
pub fn truncate_amount_to_asset_precision(amount: &BigDecimal, precision_scale: i32) -> BigDecimal {
    let bounded_scale = precision_scale.clamp(0, MAX_ASSET_PRECISION_SCALE);
    amount.with_scale(i64::from(bounded_scale))
}

/// 返回 BigDecimal 去除尾随零后的有效小数位数，整数和负指数结果均按零位处理。
/// 先做规范化再取指数，因此形如一点一零零的金额只计一位，避免因存储标度不同而误判精度越界。
/// 该纯函数仅用于精度判定，不修改入参，也不承担金额是否为正或是否符合业务限额的检查。
pub fn asset_amount_fractional_scale(amount: &BigDecimal) -> u32 {
    let (_, scale) = amount.normalized().as_bigint_and_exponent();
    scale.max(0) as u32
}

/// 检查并规范提现阶梯。
/// 校验提现费率阶梯的数量、非负边界与开闭区间，并按起始金额排序。
/// 阶梯数量上限为五十条，起始金额与费率均不得为负，显式上界必须严格大于同条的起始金额。
/// 排序后逐条比对：后一条起始金额小于前一条上界即判定区间重叠，无上界的开放阶梯之后不允许再出现任何阶梯。
/// 区间重叠或开放区间不在末尾时整体报错，避免同一提现金额命中多条规则。
/// 校验失败返回可直接透传给调用方的英文原因串，且不返回部分规范化结果；本函数不计算费用，也不读取资产配置。
pub fn normalize_withdraw_fee_tiers(
    mut tiers: Vec<WithdrawFeeTier>,
) -> Result<Vec<WithdrawFeeTier>, String> {
    if tiers.len() > MAX_WITHDRAW_FEE_TIER_COUNT {
        return Err(format!(
            "withdraw_fee_tiers must contain at most {MAX_WITHDRAW_FEE_TIER_COUNT} tiers"
        ));
    }

    for tier in &tiers {
        if tier.min_amount < 0 {
            return Err("withdraw_fee_tiers min_amount must be non-negative".to_owned());
        }
        if tier.fee_rate_percent < 0 {
            return Err("withdraw_fee_tiers fee_rate_percent must be non-negative".to_owned());
        }
        if let Some(max_amount) = tier.max_amount.as_ref()
            && max_amount <= &tier.min_amount
        {
            return Err("withdraw_fee_tiers max_amount must be greater than min_amount".to_owned());
        }
    }

    tiers.sort_by(|left, right| decimal_order(&left.min_amount, &right.min_amount));

    let mut previous_max: Option<BigDecimal> = None;
    let mut previous_unbounded = false;
    for tier in &tiers {
        if previous_unbounded {
            return Err("withdraw_fee_tiers open-ended tier must be last".to_owned());
        }
        if let Some(max_amount) = previous_max.as_ref()
            && tier.min_amount < *max_amount
        {
            return Err("withdraw_fee_tiers ranges must not overlap".to_owned());
        }

        match tier.max_amount.as_ref() {
            Some(max_amount) => {
                previous_max = Some(max_amount.clone());
            }
            None => {
                previous_max = None;
                previous_unbounded = true;
            }
        }
    }

    Ok(tiers)
}

/// 按首个满足 `min <= amount < max` 的阶梯计算百分比费用，开放上界表示无最大值。
/// 无匹配阶梯时使用固定费用；百分比值 1 表示 1%，最终费用按资产精度向零截断。
/// 本函数只计算服务端费用，不冻结余额，也不校验提现本金是否符合资产精度。
pub fn calculate_withdraw_fee(
    amount: &BigDecimal,
    fixed_fee: &BigDecimal,
    tiers: &[WithdrawFeeTier],
    precision_scale: i32,
) -> BigDecimal {
    let raw_fee = tiers
        .iter()
        .find(|tier| withdraw_fee_tier_matches_amount(tier, amount))
        .map(|tier| amount.clone() * tier.fee_rate_percent.clone() / BigDecimal::from(100))
        .unwrap_or_else(|| fixed_fee.clone());
    truncate_amount_to_asset_precision(&raw_fee, precision_scale)
}

/// 判定提现金额是否落在单条阶梯的左闭右开区间内，起始金额取等命中、上界取等不命中。
/// 上界缺省表示开放阶梯，此时只要金额不低于起始金额即命中，保证最大额提现始终有规则可用。
fn withdraw_fee_tier_matches_amount(tier: &WithdrawFeeTier, amount: &BigDecimal) -> bool {
    if amount < &tier.min_amount {
        return false;
    }
    match tier.max_amount.as_ref() {
        Some(max_amount) => amount < max_amount,
        None => true,
    }
}

/// 账务变更前的元数据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerMetadata {
    change_type: String,
    ref_type: String,
    ref_id: String,
}

impl LedgerMetadata {
    /// 构造账本业务引用元数据，并拒绝空的变更类型、引用类型或引用编号。
    /// 三个字段共同构成流水审计身份，上层应使用稳定引用实现业务幂等重放。
    /// 判空按去除首尾空白后是否为空串处理，纯空格视为缺失并返回携带字段名的元数据缺失错误。
    /// 本构造只做存在性校验，不校验变更类型是否属于已登记分类，也不检查引用编号在业务表中真实存在。
    pub fn new(
        change_type: impl Into<String>,
        ref_type: impl Into<String>,
        ref_id: impl Into<String>,
    ) -> Result<Self, WalletServiceError> {
        let change_type = change_type.into();
        let ref_type = ref_type.into();
        let ref_id = ref_id.into();

        ensure_required_metadata_field("change_type", &change_type)?;
        ensure_required_metadata_field("ref_type", &ref_type)?;
        ensure_required_metadata_field("ref_id", &ref_id)?;

        Ok(Self {
            change_type,
            ref_type,
            ref_id,
        })
    }
}

/// 单条账本记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletLedgerEntry {
    pub user_id: String,
    pub asset_id: String,
    pub change_type: String,
    pub amount: BigDecimal,
    pub balance_type: BalanceBucket,
    pub balance_after: BigDecimal,
    pub available_after: BigDecimal,
    pub frozen_after: BigDecimal,
    pub locked_after: BigDecimal,
    pub ref_type: String,
    pub ref_id: String,
}

/// 账本批次：聚合生成一系列变更。
#[derive(Debug, Clone)]
pub struct LedgerBatch {
    entries: Vec<WalletLedgerEntry>,
}

impl LedgerBatch {
    /// 根据“已应用变更”的账户快照生成流水：available、frozen、locked 按固定顺序各写至多一条。
    /// 零增量不写流水；每条记录的 amount 是对应桶增量，balance_after 与三桶 after 均取同一账后快照。
    /// 本函数不验证账户是否真的持久化，也不提供 ref_type/ref_id 唯一性；原子写入由仓储负责。
    pub fn from_account_change(
        account: &WalletAccount,
        change: BalanceChange,
        metadata: &LedgerMetadata,
    ) -> Self {
        let mut entries = Vec::new();
        push_ledger_entry(
            &mut entries,
            account,
            metadata,
            BalanceBucket::Available,
            change.available,
            account.available.clone(),
        );
        push_ledger_entry(
            &mut entries,
            account,
            metadata,
            BalanceBucket::Frozen,
            change.frozen,
            account.frozen.clone(),
        );
        push_ledger_entry(
            &mut entries,
            account,
            metadata,
            BalanceBucket::Locked,
            change.locked,
            account.locked.clone(),
        );

        Self { entries }
    }

    /// 只读借用本批次条目，顺序固定为可用、冻结、锁定，零增量的桶不会出现在结果中。
    /// 借用用于断言与审计核对，不代表这些条目已经落库。
    pub fn entries(&self) -> &[WalletLedgerEntry] {
        &self.entries
    }

    /// 消费账本批次并交出全部条目所有权，供仓储在同一事务中逐条插入。
    /// 取走后批次不再可用，条目顺序与借用视图一致，调用方不应重排以免账后快照语义错位。
    pub fn into_entries(self) -> Vec<WalletLedgerEntry> {
        self.entries
    }
}

/// 锁仓聚合。
#[derive(Debug, Clone)]
pub struct LockPosition {
    pub user_id: String,
    pub asset_id: String,
    pub unlock_type: String,
    pub unlock_at: chrono::DateTime<chrono::Utc>,
    pub remaining_amount: BigDecimal,
    pub merge_key: String,
    pub source_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LockPositionSource {
    pub source_id: String,
    pub amount: BigDecimal,
    pub unlock_at: chrono::DateTime<chrono::Utc>,
}

/// 锁仓调度类型。
#[derive(Debug, Clone)]
pub enum LockSchedule {
    ImmediateOnListing {
        listed_at: chrono::DateTime<chrono::Utc>,
    },
    FixedTime {
        unlock_at: chrono::DateTime<chrono::Utc>,
    },
    RelativePeriod,
}

/// 以用户、资产和 UTC 秒级解锁时刻生成固定时间锁仓合并键；相同键由仓储合并来源。
/// 时间戳按秒取整，因此同一秒内的多个来源会落到同一把锁仓聚合，秒级以下差异不会拆分记录。
/// 该键只提供合并身份，不校验用户或资产是否存在，也不直接改变账户的 locked 余额。
pub fn fixed_time_merge_key(
    user_id: &str,
    asset_id: &str,
    unlock_at: chrono::DateTime<chrono::Utc>,
) -> String {
    format!("fixed_time:{user_id}:{asset_id}:{}", unlock_at.timestamp())
}

/// 以上市 UTC 秒级时刻生成立即解锁合并键；键只标识锁仓聚合，不直接增加 locked。
/// 键前缀与固定时间锁仓不同，因此同一用户同一资产在同一秒的两类计划不会互相合并。
/// 该场景下上市时刻同时充当解锁时刻，函数本身不判断上市是否已经发生，也不触发解锁。
pub fn immediate_on_listing_merge_key(
    user_id: &str,
    asset_id: &str,
    listed_at: chrono::DateTime<chrono::Utc>,
) -> String {
    format!(
        "immediate_on_listing:{user_id}:{asset_id}:{}",
        listed_at.timestamp()
    )
}

/// 按规则批量创建锁仓记录。
/// 依据解锁计划生成锁仓明细：同一固定时点合并，相对周期保留来源粒度。
/// 上市即解锁与固定时间两类计划把全部来源金额累加成单条记录，解锁时刻取计划给定值而非来源自带时刻。
/// 相对周期计划按来源逐条生成，各自沿用来源的解锁时刻并以来源编号构成合并键，因此不会互相合并。
/// 每个来源金额必须为正；校验失败时不返回部分锁仓记录，也不触发持久化副作用。
pub fn create_lock_positions(
    user_id: &str,
    asset_id: &str,
    schedule: LockSchedule,
    sources: Vec<LockPositionSource>,
) -> Result<Vec<LockPosition>, WalletDomainError> {
    match schedule {
        LockSchedule::ImmediateOnListing { listed_at } => merged_lock_position(
            user_id,
            asset_id,
            "immediate_on_listing",
            listed_at,
            immediate_on_listing_merge_key(user_id, asset_id, listed_at),
            sources,
        ),
        LockSchedule::FixedTime { unlock_at } => merged_lock_position(
            user_id,
            asset_id,
            "fixed_time",
            unlock_at,
            fixed_time_merge_key(user_id, asset_id, unlock_at),
            sources,
        ),
        LockSchedule::RelativePeriod => sources
            .into_iter()
            .map(|source| {
                ensure_positive_lock_amount(&source.amount)?;
                Ok(LockPosition {
                    user_id: user_id.to_owned(),
                    asset_id: asset_id.to_owned(),
                    unlock_type: "relative_period".to_owned(),
                    unlock_at: source.unlock_at,
                    remaining_amount: source.amount,
                    merge_key: relative_period_merge_key(user_id, asset_id, &source.source_id),
                    source_id: Some(source.source_id),
                })
            })
            .collect(),
    }
}

/// 复核账户锁仓剩余量与活动锁仓明细的一致性。
/// 汇总指定用户资产的活动锁仓剩余量并与账户 locked 桶核对。
/// 只累计用户与资产同时匹配账户快照的明细，传入其他用户或其他资产的记录会被直接忽略而非报错。
/// 明细集合是否只含活动状态由调用方保证，本函数不过滤已解锁记录，也不会去重同一合并键的重复条目。
/// 不一致时返回两侧金额供审计，调用方不得在校验失败后继续结算或解锁。
pub fn verify_locked_balance_invariant(
    account: &WalletAccount,
    active_positions: &[LockPosition],
) -> Result<(), WalletDomainError> {
    let active_remaining = active_positions
        .iter()
        .filter(|position| {
            position.user_id == account.user_id && position.asset_id == account.asset_id
        })
        .fold(BigDecimal::from(0), |sum, position| {
            sum + position.remaining_amount.clone()
        });

    if account.locked == active_remaining {
        Ok(())
    } else {
        Err(WalletDomainError::LockedBalanceInvariantMismatch {
            account_locked: account.locked.clone(),
            active_positions_remaining: active_remaining,
        })
    }
}

/// 把多个锁仓来源折叠成共享同一解锁时刻和合并键的单条锁仓记录。
/// 累加过程逐个来源校验金额为正，遇到非正金额立刻中断并返回错误，不会输出已累加的部分结果。
/// 合并后来源编号置空，来源粒度信息在此层丢失，需要逐来源追溯的调用方应改用相对周期计划。
fn merged_lock_position(
    user_id: &str,
    asset_id: &str,
    unlock_type: &str,
    unlock_at: chrono::DateTime<chrono::Utc>,
    merge_key: String,
    sources: Vec<LockPositionSource>,
) -> Result<Vec<LockPosition>, WalletDomainError> {
    let remaining_amount = sources
        .into_iter()
        .try_fold(BigDecimal::from(0), |sum, source| {
            ensure_positive_lock_amount(&source.amount)?;
            Ok(sum + source.amount)
        })?;

    Ok(vec![LockPosition {
        user_id: user_id.to_owned(),
        asset_id: asset_id.to_owned(),
        unlock_type: unlock_type.to_owned(),
        unlock_at,
        remaining_amount,
        merge_key,
        source_id: None,
    }])
}

/// 守卫单个余额桶的非负不变量，负值时携带出错的桶标识返回负余额错误。
/// 零余额视为合法，只有严格小于零才拒绝；调用方须在写回账户之前对三桶逐一执行本检查。
fn ensure_non_negative(
    amount: &BigDecimal,
    bucket: BalanceBucket,
) -> Result<(), WalletDomainError> {
    if amount < &BigDecimal::from(0) {
        Err(WalletDomainError::NegativeBalance { bucket })
    } else {
        Ok(())
    }
}

/// 校验单个账本元数据字段非空，把静态字段名回填进错误以便定位缺失的是哪一项引用信息。
/// 判空前先去除首尾空白，因此只含空格的取值同样按缺失处理，避免空引用流入流水审计身份。
fn ensure_required_metadata_field(
    field: &'static str,
    value: &str,
) -> Result<(), WalletServiceError> {
    if value.trim().is_empty() {
        Err(WalletServiceError::MissingLedgerMetadata(field))
    } else {
        Ok(())
    }
}

/// 为单个余额桶追加一条账本条目，增量为零时直接跳过，避免生成没有资金含义的空流水。
/// 条目 amount 记录本桶有符号增量，balance_after 取该桶账后余额，三桶 after 一律取自同一账户快照。
/// 变更类型与业务引用整体复制自元数据，因此同一次余额变更产生的多条流水共享同一审计身份。
fn push_ledger_entry(
    entries: &mut Vec<WalletLedgerEntry>,
    account: &WalletAccount,
    metadata: &LedgerMetadata,
    balance_type: BalanceBucket,
    amount: BigDecimal,
    balance_after: BigDecimal,
) {
    if amount == 0 {
        return;
    }

    entries.push(WalletLedgerEntry {
        user_id: account.user_id.clone(),
        asset_id: account.asset_id.clone(),
        change_type: metadata.change_type.clone(),
        amount,
        balance_type,
        balance_after,
        available_after: account.available.clone(),
        frozen_after: account.frozen.clone(),
        locked_after: account.locked.clone(),
        ref_type: metadata.ref_type.clone(),
        ref_id: metadata.ref_id.clone(),
    });
}

/// 拒绝零和负数的锁仓来源金额，防止空锁仓或反向解锁被当作正常锁仓写入。
/// 与余额桶的非负校验不同，这里零同样不合法，因为零金额来源既不改变 locked 也无审计价值。
fn ensure_positive_lock_amount(amount: &BigDecimal) -> Result<(), WalletDomainError> {
    if amount <= &BigDecimal::from(0) {
        Err(WalletDomainError::NonPositiveLockAmount)
    } else {
        Ok(())
    }
}

/// 用来源编号而非解锁时刻构成相对周期锁仓的合并键，使每个来源保留独立的锁仓聚合。
/// 因此同一用户同一资产的多笔相对周期锁仓不会互相合并，重复投递同一来源编号才会命中同一条记录。
fn relative_period_merge_key(user_id: &str, asset_id: &str, source_id: &str) -> String {
    format!("relative_period:{user_id}:{asset_id}:{source_id}")
}

/// 为提现阶梯排序提供全序比较，定点数不可比时退化为相等以保证排序过程不会中途崩溃。
/// 退化只影响两条阶梯的相对次序，区间重叠与开放阶梯位置仍由后续逐条校验兜底。
fn decimal_order(left: &BigDecimal, right: &BigDecimal) -> Ordering {
    left.partial_cmp(right).unwrap_or(Ordering::Equal)
}
