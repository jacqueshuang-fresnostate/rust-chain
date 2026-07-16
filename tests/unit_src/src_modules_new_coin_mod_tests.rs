use super::*;
use bigdecimal::BigDecimal;
use std::str::FromStr;

fn at(seconds: i64) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(seconds, 0).unwrap()
}

fn amount(value: i64) -> BigDecimal {
    BigDecimal::from(value)
}

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

fn unlock_source(
    source_id: &str,
    quantity: i64,
    source_time: chrono::DateTime<chrono::Utc>,
) -> UnlockSource {
    UnlockSource {
        user_id: "user-1".to_owned(),
        asset_id: "NEW".to_owned(),
        source_id: source_id.to_owned(),
        amount: amount(quantity),
        source_time,
    }
}

#[test]
fn lifecycle_transitions_are_strictly_forward() {
    assert_eq!(
        LifecycleStatus::Preheat.transition_to(LifecycleStatus::Subscription),
        Ok(LifecycleStatus::Subscription)
    );
    assert_eq!(
        LifecycleStatus::Subscription.transition_to(LifecycleStatus::Distribution),
        Ok(LifecycleStatus::Distribution)
    );
    assert_eq!(
        LifecycleStatus::Distribution.transition_to(LifecycleStatus::Listed),
        Ok(LifecycleStatus::Listed)
    );

    assert_eq!(
        LifecycleStatus::Preheat.transition_to(LifecycleStatus::Listed),
        Err(NewCoinDomainError::InvalidLifecycleTransition {
            from: LifecycleStatus::Preheat,
            to: LifecycleStatus::Listed,
        })
    );
    assert_eq!(
        LifecycleStatus::Listed.transition_to(LifecycleStatus::Distribution),
        Err(NewCoinDomainError::InvalidLifecycleTransition {
            from: LifecycleStatus::Listed,
            to: LifecycleStatus::Distribution,
        })
    );
}

#[test]
fn only_subscription_status_accepts_primary_subscription() {
    assert_eq!(
        ensure_subscription_allowed(LifecycleStatus::Preheat),
        Err(NewCoinDomainError::SubscriptionNotOpen {
            status: LifecycleStatus::Preheat,
        })
    );
    assert_eq!(
        ensure_subscription_allowed(LifecycleStatus::Subscription),
        Ok(())
    );
    assert_eq!(
        ensure_subscription_allowed(LifecycleStatus::Listed),
        Err(NewCoinDomainError::SubscriptionNotOpen {
            status: LifecycleStatus::Listed,
        })
    );
}

#[test]
fn listed_post_listing_purchase_is_named_purchase_and_immediate_unlock_is_available() {
    let listed_at = at(1_700_000_000);
    let source = unlock_source("purchase-1", 50, listed_at + chrono::Duration::seconds(60));

    let plan = plan_post_listing_purchase(
        LifecycleStatus::Listed,
        true,
        &UnlockRule::ImmediateOnListing { listed_at },
        source,
    )
    .unwrap();

    assert_eq!(plan.order_kind, NewCoinOrderKind::Purchase);
    assert_eq!(plan.order_kind.chinese_name(), "认购");
    assert_eq!(plan.order_kind.api_action(), "purchase");
    assert_eq!(plan.unlock.available_amount, amount(50));
    assert_eq!(plan.unlock.locked_amount, amount(0));
    assert!(plan.unlock.lock_positions.is_empty());
}

#[test]
fn listed_purchase_with_fixed_time_unlock_creates_locked_position() {
    let source_time = at(1_700_000_000);
    let unlock_at = source_time + chrono::Duration::days(7);

    let plan = plan_post_listing_purchase(
        LifecycleStatus::Listed,
        true,
        &UnlockRule::FixedTime { unlock_at },
        unlock_source("purchase-1", 25, source_time),
    )
    .unwrap();

    assert_eq!(plan.unlock.available_amount, amount(0));
    assert_eq!(plan.unlock.locked_amount, amount(25));
    assert_eq!(plan.unlock.lock_positions.len(), 1);
    assert_eq!(plan.unlock.lock_positions[0].unlock_type, "fixed_time");
    assert_eq!(plan.unlock.lock_positions[0].unlock_at, unlock_at);
    assert_eq!(plan.unlock.lock_positions[0].remaining_amount, amount(25));
    assert_eq!(plan.unlock.lock_positions[0].source_id, None);
}

#[test]
fn relative_period_unlock_splits_by_purchase_source_time() {
    let source_time = at(1_700_000_000);
    let plan = apply_unlock_rule(
        &UnlockRule::RelativePeriod {
            seconds_after_source: 86_400,
        },
        vec![
            unlock_source("purchase-1", 10, source_time),
            unlock_source(
                "purchase-2",
                15,
                source_time + chrono::Duration::seconds(30),
            ),
        ],
    )
    .unwrap();

    assert_eq!(plan.available_amount, amount(0));
    assert_eq!(plan.locked_amount, amount(25));
    assert_eq!(plan.lock_positions.len(), 2);
    assert_eq!(
        plan.lock_positions[0].source_id.as_deref(),
        Some("purchase-1")
    );
    assert_eq!(
        plan.lock_positions[0].unlock_at,
        source_time + chrono::Duration::seconds(86_400)
    );
    assert_eq!(
        plan.lock_positions[1].source_id.as_deref(),
        Some("purchase-2")
    );
    assert_ne!(
        plan.lock_positions[0].merge_key,
        plan.lock_positions[1].merge_key
    );
}

#[test]
fn unlock_fee_supports_market_value_basis_and_blocks_release_until_paid() {
    let fee = calculate_unlock_fee(
        &UnlockFeeRule {
            enabled: true,
            rate: decimal("0.04"),
            basis: UnlockFeeBasis::MarketValue,
            payment_asset: Some("USDT".to_owned()),
        },
        UnlockFeeInput {
            unlock_quantity: amount(10),
            unlock_price: amount(5),
            purchase_cost: amount(30),
        },
    )
    .unwrap();

    assert!(fee.required);
    assert_eq!(fee.payment_asset.as_deref(), Some("USDT"));
    assert_eq!(fee.amount, decimal("2.00"));
    assert_eq!(
        ensure_unlock_release_allowed(&fee, false),
        Err(NewCoinDomainError::UnlockFeePaymentRequired {
            payment_asset: "USDT".to_owned(),
            amount: decimal("2.00"),
        })
    );
    assert_eq!(ensure_unlock_release_allowed(&fee, true), Ok(()));
}

#[test]
fn unlock_fee_supports_profit_basis_and_disabled_fee_releases_without_payment() {
    let profit_fee = calculate_unlock_fee(
        &UnlockFeeRule {
            enabled: true,
            rate: decimal("0.10"),
            basis: UnlockFeeBasis::Profit,
            payment_asset: Some("USDT".to_owned()),
        },
        UnlockFeeInput {
            unlock_quantity: amount(10),
            unlock_price: amount(5),
            purchase_cost: amount(30),
        },
    )
    .unwrap();

    assert_eq!(profit_fee.amount, decimal("2.00"));
    assert_eq!(profit_fee.payment_asset.as_deref(), Some("USDT"));

    let disabled_fee = calculate_unlock_fee(
        &UnlockFeeRule {
            enabled: false,
            rate: decimal("0.99"),
            basis: UnlockFeeBasis::MarketValue,
            payment_asset: Some("USDT".to_owned()),
        },
        UnlockFeeInput {
            unlock_quantity: amount(10),
            unlock_price: amount(5),
            purchase_cost: amount(30),
        },
    )
    .unwrap();

    assert!(!disabled_fee.required);
    assert_eq!(disabled_fee.amount, amount(0));
    assert_eq!(ensure_unlock_release_allowed(&disabled_fee, false), Ok(()));
}
