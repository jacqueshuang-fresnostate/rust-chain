//! new_coin bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。

use crate::{
    architecture::ServiceLayer,
    error::{AppError, AppResult},
    modules::new_coin::{
        LifecycleStatus, NewCoinDomainError, NewCoinOrderKind, UnlockFeeInput, UnlockFeeQuote,
        UnlockFeeRule, UnlockRule, UnlockSource, apply_unlock_rule, calculate_unlock_fee,
        ensure_unlock_release_allowed, plan_post_listing_purchase,
        repository::{NewCoinLockPositionWrite, NewCoinProjectRuleRead, UnlockFeeExpectation},
    },
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use std::cmp::{Ordering, max};

use super::repository::{
    NewCoinPurchaseRepository, NewCoinRepositoryError, PostListingPurchaseRecord,
    UnlockFeePaymentRecord, UnlockFeeRepository, UnlockReleaseRecord, WalletLockCommandOutput,
};

#[derive(Debug)]
pub struct ServiceLayerMarker;

impl ServiceLayer for ServiceLayerMarker {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewCoinServiceError {
    Domain(NewCoinDomainError),
    Repository(NewCoinRepositoryError),
}

impl From<NewCoinDomainError> for NewCoinServiceError {
    fn from(error: NewCoinDomainError) -> Self {
        Self::Domain(error)
    }
}

impl From<NewCoinRepositoryError> for NewCoinServiceError {
    fn from(error: NewCoinRepositoryError) -> Self {
        Self::Repository(error)
    }
}

#[derive(Debug, Clone)]
pub struct PostListingPurchaseCommand {
    pub project_id: String,
    pub order_id: String,
    pub user_id: String,
    pub asset_id: String,
    pub quantity: BigDecimal,
    pub purchased_at: DateTime<Utc>,
    pub lifecycle_status: LifecycleStatus,
    pub post_listing_purchase_enabled: bool,
    pub unlock_rule: UnlockRule,
}

#[derive(Debug, Clone)]
pub struct PostListingPurchaseResult {
    pub order_kind: NewCoinOrderKind,
    pub wallet_lock: WalletLockCommandOutput,
}

#[derive(Debug, Clone)]
pub struct UnlockFeeQuoteCommand {
    pub unlock_id: String,
    pub user_id: String,
    pub asset_id: String,
    pub unlock_quantity: BigDecimal,
    pub unlock_price: BigDecimal,
    pub purchase_cost: BigDecimal,
    pub fee_rule: UnlockFeeRule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockFeeQuoteResult {
    pub unlock_id: String,
    pub user_id: String,
    pub asset_id: String,
    pub quote: UnlockFeeQuote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayUnlockFeeCommand {
    pub unlock_id: String,
    pub user_id: String,
    pub payment_asset: String,
    pub amount: BigDecimal,
    pub paid_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseUnlockCommand {
    pub unlock_id: String,
    pub user_id: String,
    pub asset_id: String,
    pub fee_quote: UnlockFeeQuote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockReleaseResult {
    pub unlock_id: String,
    pub released: bool,
}

#[derive(Debug, Clone)]
pub struct NewCoinService<R> {
    repository: R,
}

impl<R> NewCoinService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn repository(&self) -> &R {
        &self.repository
    }

    pub fn repository_mut(&mut self) -> &mut R {
        &mut self.repository
    }
}

impl<R> NewCoinService<R>
where
    R: NewCoinPurchaseRepository,
{
    pub fn create_post_listing_purchase(
        &mut self,
        command: PostListingPurchaseCommand,
    ) -> Result<PostListingPurchaseResult, NewCoinServiceError> {
        let unlock_source = UnlockSource {
            user_id: command.user_id.clone(),
            asset_id: command.asset_id.clone(),
            source_id: command.order_id.clone(),
            amount: command.quantity.clone(),
            source_time: command.purchased_at,
        };
        let plan = plan_post_listing_purchase(
            command.lifecycle_status,
            command.post_listing_purchase_enabled,
            &command.unlock_rule,
            unlock_source,
        )?;
        let wallet_lock = WalletLockCommandOutput {
            user_id: command.user_id.clone(),
            asset_id: command.asset_id.clone(),
            available_delta: plan.unlock.available_amount,
            locked_delta: plan.unlock.locked_amount,
            lock_positions: plan.unlock.lock_positions,
        };
        let record = PostListingPurchaseRecord {
            project_id: command.project_id,
            order_id: command.order_id,
            user_id: command.user_id,
            asset_id: command.asset_id,
            quantity: command.quantity,
            order_kind: plan.order_kind,
            purchased_at: command.purchased_at,
            wallet_lock: wallet_lock.clone(),
        };

        self.repository.save_post_listing_purchase(record)?;

        Ok(PostListingPurchaseResult {
            order_kind: plan.order_kind,
            wallet_lock,
        })
    }
}

impl<R> NewCoinService<R>
where
    R: UnlockFeeRepository,
{
    pub fn quote_unlock_fee(
        &self,
        command: UnlockFeeQuoteCommand,
    ) -> Result<UnlockFeeQuoteResult, NewCoinServiceError> {
        let quote = calculate_unlock_fee(
            &command.fee_rule,
            UnlockFeeInput {
                unlock_quantity: command.unlock_quantity,
                unlock_price: command.unlock_price,
                purchase_cost: command.purchase_cost,
            },
        )?;

        Ok(UnlockFeeQuoteResult {
            unlock_id: command.unlock_id,
            user_id: command.user_id,
            asset_id: command.asset_id,
            quote,
        })
    }

    pub fn pay_unlock_fee(
        &mut self,
        command: PayUnlockFeeCommand,
    ) -> Result<UnlockFeePaymentRecord, NewCoinServiceError> {
        let record = UnlockFeePaymentRecord {
            unlock_id: command.unlock_id,
            user_id: command.user_id,
            payment_asset: command.payment_asset,
            amount: command.amount,
            paid_at: command.paid_at,
        };

        self.repository.save_unlock_fee_payment(record.clone())?;
        Ok(record)
    }

    pub fn release_unlock(
        &mut self,
        command: ReleaseUnlockCommand,
    ) -> Result<UnlockReleaseResult, NewCoinServiceError> {
        let fee_paid = if command.fee_quote.required {
            self.repository
                .unlock_fee_paid(&command.unlock_id, &command.user_id)?
        } else {
            false
        };
        ensure_unlock_release_allowed(&command.fee_quote, fee_paid)?;

        let record = UnlockReleaseRecord {
            unlock_id: command.unlock_id.clone(),
            user_id: command.user_id,
            asset_id: command.asset_id,
            released_at: Utc::now(),
        };
        self.repository.mark_unlock_released(record)?;

        Ok(UnlockReleaseResult {
            unlock_id: command.unlock_id,
            released: true,
        })
    }
}

pub(crate) fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

pub(crate) fn ensure_unlock_fee_payment_matches(
    expectation: &UnlockFeeExpectation,
    payment_asset_id: u64,
    amount: &BigDecimal,
) -> AppResult<()> {
    if !expectation.unlock_fee_enabled {
        return Err(AppError::Validation(
            "unlock fee payment is not required for this unlock".to_owned(),
        ));
    }
    if expectation.unlock_fee_asset != Some(payment_asset_id) {
        return Err(AppError::Validation(
            "unlock fee payment asset does not match required asset".to_owned(),
        ));
    }
    let Some(expected_amount) = &expectation.unlock_fee_amount else {
        return Err(AppError::Validation(
            "unlock fee amount is not configured".to_owned(),
        ));
    };
    // 金额比较使用 normalized，避免同一数值因 scale 不同导致合法支付被拒。
    if amount <= &BigDecimal::default()
        || amount.normalized().cmp(&expected_amount.normalized()) != Ordering::Equal
    {
        return Err(AppError::Validation(
            "unlock fee payment amount does not match required amount".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn ensure_positive_amount(amount: &BigDecimal, field: &str) -> AppResult<()> {
    if amount <= &BigDecimal::default() {
        Err(AppError::Validation(format!("{field} must be positive")))
    } else {
        Ok(())
    }
}

pub(crate) fn ensure_idempotency_key(value: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        Err(AppError::Validation(
            "idempotency_key must not be empty".to_owned(),
        ))
    } else {
        Ok(())
    }
}

pub(crate) fn lifecycle_status(value: &str) -> AppResult<LifecycleStatus> {
    match value {
        "preheat" => Ok(LifecycleStatus::Preheat),
        "subscription" => Ok(LifecycleStatus::Subscription),
        "distribution" => Ok(LifecycleStatus::Distribution),
        "listed" => Ok(LifecycleStatus::Listed),
        _ => Err(AppError::Validation(
            "unsupported lifecycle_status".to_owned(),
        )),
    }
}

pub(crate) fn ensure_post_listing_purchase_enabled(
    project: &NewCoinProjectRuleRead,
    requested_pair_id: u64,
) -> AppResult<()> {
    if !project.post_listing_purchase_enabled
        || project.post_listing_pair_id != Some(requested_pair_id)
    {
        return Err(AppError::Validation(
            "post-listing new coin purchase is not open for this project".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn lock_positions_for_project(
    project: &NewCoinProjectRuleRead,
    user_id: u64,
    asset_id: u64,
    source_id: &str,
    quantity: BigDecimal,
    source_time: chrono::DateTime<Utc>,
    source_type: &str,
) -> AppResult<Vec<NewCoinLockPositionWrite>> {
    let unlock_rule = unlock_rule_from_project(project)?;
    let application = apply_unlock_rule(
        &unlock_rule,
        vec![UnlockSource {
            user_id: user_id.to_string(),
            asset_id: asset_id.to_string(),
            source_id: source_id.to_owned(),
            amount: quantity,
            source_time,
        }],
    )
    .map_err(|error| AppError::Validation(format!("invalid new coin unlock rule: {error:?}")))?;

    Ok(application
        .lock_positions
        .into_iter()
        .map(|position| NewCoinLockPositionWrite {
            user_id,
            asset_id,
            unlock_type: position.unlock_type,
            unlock_at: position.unlock_at,
            amount: position.remaining_amount,
            merge_key: position.merge_key,
            source_type: source_type.to_owned(),
            source_id: source_id.to_owned(),
        })
        .collect())
}

pub(crate) fn unlock_rule_from_project(project: &NewCoinProjectRuleRead) -> AppResult<UnlockRule> {
    match project.unlock_type.as_str() {
        "immediate_on_listing" => Ok(UnlockRule::ImmediateOnListing {
            listed_at: project.listed_at.ok_or_else(|| {
                AppError::Validation("listed_at is required for immediate unlock".to_owned())
            })?,
        }),
        "fixed_time" => Ok(UnlockRule::FixedTime {
            unlock_at: project.fixed_unlock_at.ok_or_else(|| {
                AppError::Validation("fixed_unlock_at is required for fixed unlock".to_owned())
            })?,
        }),
        "relative_period" => Ok(UnlockRule::RelativePeriod {
            seconds_after_source: project
                .relative_unlock_seconds
                .ok_or_else(|| {
                    AppError::Validation(
                        "relative_unlock_seconds is required for relative unlock".to_owned(),
                    )
                })?
                .try_into()
                .map_err(|_| {
                    AppError::Validation("relative unlock period is too large".to_owned())
                })?,
        }),
        _ => Err(AppError::Validation(
            "unsupported new coin unlock_type".to_owned(),
        )),
    }
}

pub(crate) fn unlock_fee_fields(
    project: &NewCoinProjectRuleRead,
    quantity: &BigDecimal,
    unlock_price: &BigDecimal,
    purchase_cost: &BigDecimal,
) -> AppResult<(&'static str, Option<BigDecimal>)> {
    if !project.unlock_fee_enabled {
        return Ok(("not_required", None));
    }
    let fee_rate = project.unlock_fee_rate.clone().unwrap_or_default();
    if fee_rate <= BigDecimal::default() {
        return Ok(("not_required", Some(BigDecimal::default())));
    }
    if project.unlock_fee_asset.is_none() {
        return Err(AppError::Validation(
            "unlock_fee_asset is required when unlock fee is enabled".to_owned(),
        ));
    }
    let market_value = quantity.clone() * unlock_price.clone();
    let basis_amount = match project
        .unlock_fee_basis
        .as_deref()
        .unwrap_or("market_value")
    {
        "market_value" => market_value,
        "profit" => max(market_value - purchase_cost.clone(), BigDecimal::default()),
        _ => {
            return Err(AppError::Validation(
                "unsupported unlock_fee_basis".to_owned(),
            ));
        }
    };
    let fee_amount = basis_amount * fee_rate;
    let fee_paid_status = if fee_amount > BigDecimal::default() {
        "pending"
    } else {
        "not_required"
    };
    Ok((fee_paid_status, Some(fee_amount)))
}
