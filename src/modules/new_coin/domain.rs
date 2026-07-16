//! new_coin bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。

use crate::{
    architecture::DomainLayer,
    modules::wallet::{
        LockPosition, LockPositionSource, LockSchedule, WalletDomainError, create_lock_positions,
    },
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct DomainLayerMarker;

impl DomainLayer for DomainLayerMarker {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleStatus {
    Preheat,
    Subscription,
    Distribution,
    Listed,
}

impl LifecycleStatus {
    pub fn transition_to(self, to: LifecycleStatus) -> Result<LifecycleStatus, NewCoinDomainError> {
        match (self, to) {
            (LifecycleStatus::Preheat, LifecycleStatus::Subscription)
            | (LifecycleStatus::Subscription, LifecycleStatus::Distribution)
            | (LifecycleStatus::Distribution, LifecycleStatus::Listed) => Ok(to),
            (from, to) => Err(NewCoinDomainError::InvalidLifecycleTransition { from, to }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnlockType {
    ImmediateOnListing,
    FixedTime,
    RelativePeriod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnlockFeeBasis {
    MarketValue,
    Profit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewCoinDomainError {
    InvalidLifecycleTransition {
        from: LifecycleStatus,
        to: LifecycleStatus,
    },
    SubscriptionNotOpen {
        status: LifecycleStatus,
    },
    PostListingPurchaseNotOpen {
        status: LifecycleStatus,
    },
    PostListingPurchaseDisabled,
    NonPositiveUnlockAmount,
    NonPositiveRelativePeriod,
    NegativeUnlockFeeRate,
    NegativeUnlockPrice,
    NegativePurchaseCost,
    MissingUnlockFeePaymentAsset,
    WalletLock(WalletDomainError),
    UnlockFeePaymentRequired {
        payment_asset: String,
        amount: BigDecimal,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewCoinOrderKind {
    Subscription,
    Purchase,
}

impl NewCoinOrderKind {
    pub fn chinese_name(self) -> &'static str {
        match self {
            NewCoinOrderKind::Subscription => "申购",
            NewCoinOrderKind::Purchase => "认购",
        }
    }

    pub fn api_action(self) -> &'static str {
        match self {
            NewCoinOrderKind::Subscription => "subscription",
            NewCoinOrderKind::Purchase => "purchase",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnlockRule {
    ImmediateOnListing { listed_at: DateTime<Utc> },
    FixedTime { unlock_at: DateTime<Utc> },
    RelativePeriod { seconds_after_source: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockSource {
    pub user_id: String,
    pub asset_id: String,
    pub source_id: String,
    pub amount: BigDecimal,
    pub source_time: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct PendingLockSource {
    user_id: String,
    asset_id: String,
    wallet_source: LockPositionSource,
}

#[derive(Debug, Clone)]
pub struct UnlockApplication {
    pub available_amount: BigDecimal,
    pub locked_amount: BigDecimal,
    pub lock_positions: Vec<LockPosition>,
}

#[derive(Debug, Clone)]
pub struct PostListingPurchasePlan {
    pub order_kind: NewCoinOrderKind,
    pub unlock: UnlockApplication,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockFeeRule {
    pub enabled: bool,
    pub rate: BigDecimal,
    pub basis: UnlockFeeBasis,
    pub payment_asset: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockFeeInput {
    pub unlock_quantity: BigDecimal,
    pub unlock_price: BigDecimal,
    pub purchase_cost: BigDecimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockFeeQuote {
    pub required: bool,
    pub amount: BigDecimal,
    pub basis: UnlockFeeBasis,
    pub payment_asset: Option<String>,
}

pub fn ensure_subscription_allowed(status: LifecycleStatus) -> Result<(), NewCoinDomainError> {
    if status == LifecycleStatus::Subscription {
        Ok(())
    } else {
        Err(NewCoinDomainError::SubscriptionNotOpen { status })
    }
}

pub fn plan_post_listing_purchase(
    status: LifecycleStatus,
    enabled: bool,
    unlock_rule: &UnlockRule,
    source: UnlockSource,
) -> Result<PostListingPurchasePlan, NewCoinDomainError> {
    if status != LifecycleStatus::Listed {
        return Err(NewCoinDomainError::PostListingPurchaseNotOpen { status });
    }

    if !enabled {
        return Err(NewCoinDomainError::PostListingPurchaseDisabled);
    }

    Ok(PostListingPurchasePlan {
        order_kind: NewCoinOrderKind::Purchase,
        unlock: apply_unlock_rule(unlock_rule, vec![source])?,
    })
}

pub fn apply_unlock_rule(
    unlock_rule: &UnlockRule,
    sources: Vec<UnlockSource>,
) -> Result<UnlockApplication, NewCoinDomainError> {
    ensure_positive_sources(&sources)?;

    match unlock_rule {
        UnlockRule::ImmediateOnListing { listed_at } => {
            let mut available_amount = BigDecimal::from(0);
            let mut locked_sources = Vec::new();

            for source in sources {
                if source.source_time >= *listed_at {
                    available_amount += source.amount;
                } else {
                    locked_sources.push(to_lock_source(source, *listed_at));
                }
            }

            let lock_positions = if locked_sources.is_empty() {
                Vec::new()
            } else {
                let user_id = locked_sources[0].user_id.clone();
                let asset_id = locked_sources[0].asset_id.clone();
                let wallet_sources = to_wallet_sources(locked_sources);
                create_lock_positions(
                    &user_id,
                    &asset_id,
                    LockSchedule::ImmediateOnListing {
                        listed_at: *listed_at,
                    },
                    wallet_sources,
                )
                .map_err(NewCoinDomainError::WalletLock)?
            };

            let locked_amount = sum_remaining(&lock_positions);
            Ok(UnlockApplication {
                available_amount,
                locked_amount,
                lock_positions,
            })
        }
        UnlockRule::FixedTime { unlock_at } => {
            let lock_sources = sources
                .into_iter()
                .map(|source| to_lock_source(source, *unlock_at))
                .collect::<Vec<_>>();
            let user_id = lock_sources[0].user_id.clone();
            let asset_id = lock_sources[0].asset_id.clone();
            let wallet_sources = to_wallet_sources(lock_sources);
            let lock_positions = create_lock_positions(
                &user_id,
                &asset_id,
                LockSchedule::FixedTime {
                    unlock_at: *unlock_at,
                },
                wallet_sources,
            )
            .map_err(NewCoinDomainError::WalletLock)?;
            let locked_amount = sum_remaining(&lock_positions);

            Ok(UnlockApplication {
                available_amount: BigDecimal::from(0),
                locked_amount,
                lock_positions,
            })
        }
        UnlockRule::RelativePeriod {
            seconds_after_source,
        } => {
            if *seconds_after_source <= 0 {
                return Err(NewCoinDomainError::NonPositiveRelativePeriod);
            }

            let lock_sources = sources
                .into_iter()
                .map(|source| {
                    let unlock_at = source.source_time + Duration::seconds(*seconds_after_source);
                    to_lock_source(source, unlock_at)
                })
                .collect::<Vec<_>>();
            let user_id = lock_sources[0].user_id.clone();
            let asset_id = lock_sources[0].asset_id.clone();
            let wallet_sources = to_wallet_sources(lock_sources);
            let lock_positions = create_lock_positions(
                &user_id,
                &asset_id,
                LockSchedule::RelativePeriod,
                wallet_sources,
            )
            .map_err(NewCoinDomainError::WalletLock)?;
            let locked_amount = sum_remaining(&lock_positions);

            Ok(UnlockApplication {
                available_amount: BigDecimal::from(0),
                locked_amount,
                lock_positions,
            })
        }
    }
}

pub fn calculate_unlock_fee(
    rule: &UnlockFeeRule,
    input: UnlockFeeInput,
) -> Result<UnlockFeeQuote, NewCoinDomainError> {
    let zero = BigDecimal::from(0);

    if input.unlock_quantity <= zero {
        return Err(NewCoinDomainError::NonPositiveUnlockAmount);
    }
    if input.unlock_price < zero {
        return Err(NewCoinDomainError::NegativeUnlockPrice);
    }
    if input.purchase_cost < zero {
        return Err(NewCoinDomainError::NegativePurchaseCost);
    }
    if rule.rate < zero {
        return Err(NewCoinDomainError::NegativeUnlockFeeRate);
    }

    if !rule.enabled {
        return Ok(UnlockFeeQuote {
            required: false,
            amount: BigDecimal::from(0),
            basis: rule.basis,
            payment_asset: rule.payment_asset.clone(),
        });
    }

    let payment_asset = rule
        .payment_asset
        .as_ref()
        .filter(|asset| !asset.trim().is_empty())
        .cloned()
        .ok_or(NewCoinDomainError::MissingUnlockFeePaymentAsset)?;
    let market_value = input.unlock_quantity * input.unlock_price;
    let basis_amount = match rule.basis {
        UnlockFeeBasis::MarketValue => market_value,
        UnlockFeeBasis::Profit => {
            max_decimal(market_value - input.purchase_cost, BigDecimal::from(0))
        }
    };
    let amount = basis_amount * rule.rate.clone();
    let required = amount > 0;

    Ok(UnlockFeeQuote {
        required,
        amount,
        basis: rule.basis,
        payment_asset: Some(payment_asset),
    })
}

pub fn ensure_unlock_release_allowed(
    fee: &UnlockFeeQuote,
    fee_paid: bool,
) -> Result<(), NewCoinDomainError> {
    if fee.required && !fee_paid {
        Err(NewCoinDomainError::UnlockFeePaymentRequired {
            payment_asset: fee.payment_asset.clone().unwrap_or_default(),
            amount: fee.amount.clone(),
        })
    } else {
        Ok(())
    }
}

fn ensure_positive_sources(sources: &[UnlockSource]) -> Result<(), NewCoinDomainError> {
    if sources.is_empty() || sources.iter().any(|source| source.amount <= 0) {
        Err(NewCoinDomainError::NonPositiveUnlockAmount)
    } else {
        Ok(())
    }
}

fn to_lock_source(source: UnlockSource, unlock_at: DateTime<Utc>) -> PendingLockSource {
    PendingLockSource {
        user_id: source.user_id,
        asset_id: source.asset_id,
        wallet_source: LockPositionSource {
            source_id: source.source_id,
            amount: source.amount,
            unlock_at,
        },
    }
}

fn to_wallet_sources(sources: Vec<PendingLockSource>) -> Vec<LockPositionSource> {
    sources
        .into_iter()
        .map(|source| source.wallet_source)
        .collect()
}

fn sum_remaining(lock_positions: &[LockPosition]) -> BigDecimal {
    lock_positions
        .iter()
        .fold(BigDecimal::from(0), |sum, position| {
            sum + position.remaining_amount.clone()
        })
}

fn max_decimal(left: BigDecimal, right: BigDecimal) -> BigDecimal {
    if left >= right { left } else { right }
}
