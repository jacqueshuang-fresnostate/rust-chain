//! wallet bounded context domain layer.
//!
//! 领域层：放置钱包领域实体、值对象和不依赖 I/O 的业务规则。

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
    fn from(error: WalletDomainError) -> Self {
        Self::Domain(error)
    }
}

/// 计算金额的小数位数。
pub fn amount_fits_asset_precision(amount: &BigDecimal, precision_scale: i32) -> bool {
    if !(0..=MAX_ASSET_PRECISION_SCALE).contains(&precision_scale) {
        return false;
    }
    asset_amount_fractional_scale(amount) <= precision_scale as u32
}

/// 资产金额的小数位裁剪。
pub fn truncate_amount_to_asset_precision(amount: &BigDecimal, precision_scale: i32) -> BigDecimal {
    let bounded_scale = precision_scale.clamp(0, MAX_ASSET_PRECISION_SCALE);
    amount.with_scale(i64::from(bounded_scale))
}

/// BigDecimal 标准化后的有效小数位数。
pub fn asset_amount_fractional_scale(amount: &BigDecimal) -> u32 {
    let (_, scale) = amount.normalized().as_bigint_and_exponent();
    scale.max(0) as u32
}

/// 检查并规范提现阶梯。
pub fn normalize_withdraw_fee_tiers(
    mut tiers: Vec<WithdrawFeeTier>,
) -> Result<Vec<WithdrawFeeTier>, String> {
    if tiers.len() > MAX_WITHDRAW_FEE_TIER_COUNT {
        return Err(format!(
            "withdraw_fee_tiers must contain at most {MAX_WITHDRAW_FEE_TIER_COUNT} tiers"
        ));
    }

    for tier in &tiers {
        if tier.min_amount < BigDecimal::from(0) {
            return Err("withdraw_fee_tiers min_amount must be non-negative".to_owned());
        }
        if tier.fee_rate_percent < BigDecimal::from(0) {
            return Err("withdraw_fee_tiers fee_rate_percent must be non-negative".to_owned());
        }
        if let Some(max_amount) = tier.max_amount.as_ref() {
            if max_amount <= &tier.min_amount {
                return Err(
                    "withdraw_fee_tiers max_amount must be greater than min_amount".to_owned(),
                );
            }
        }
    }

    tiers.sort_by(|left, right| decimal_order(&left.min_amount, &right.min_amount));

    let mut previous_max: Option<BigDecimal> = None;
    let mut previous_unbounded = false;
    for tier in &tiers {
        if previous_unbounded {
            return Err("withdraw_fee_tiers open-ended tier must be last".to_owned());
        }
        if let Some(max_amount) = previous_max.as_ref() {
            if tier.min_amount < *max_amount {
                return Err("withdraw_fee_tiers ranges must not overlap".to_owned());
            }
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

/// 计算提现手续费。
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

    pub fn entries(&self) -> &[WalletLedgerEntry] {
        &self.entries
    }

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

/// 固定时间解锁键。
pub fn fixed_time_merge_key(
    user_id: &str,
    asset_id: &str,
    unlock_at: chrono::DateTime<chrono::Utc>,
) -> String {
    format!("fixed_time:{user_id}:{asset_id}:{}", unlock_at.timestamp())
}

/// 上市后立即解锁键。
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

fn ensure_positive_lock_amount(amount: &BigDecimal) -> Result<(), WalletDomainError> {
    if amount <= &BigDecimal::from(0) {
        Err(WalletDomainError::NonPositiveLockAmount)
    } else {
        Ok(())
    }
}

fn relative_period_merge_key(user_id: &str, asset_id: &str, source_id: &str) -> String {
    format!("relative_period:{user_id}:{asset_id}:{source_id}")
}

fn decimal_order(left: &BigDecimal, right: &BigDecimal) -> Ordering {
    left.partial_cmp(right).unwrap_or(Ordering::Equal)
}
