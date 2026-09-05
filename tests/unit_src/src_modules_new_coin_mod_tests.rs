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

#[test]
fn authoritative_new_coin_quote_requires_an_exact_positive_asset_amount() {
    use super::service::authoritative_new_coin_quote_amount;

    assert_eq!(
        authoritative_new_coin_quote_amount(&decimal("1.25"), &decimal("4"), 2)
            .expect("exact quote"),
        decimal("5.00")
    );
    let precision_error = authoritative_new_coin_quote_amount(&decimal("0.333"), &decimal("1"), 2)
        .expect_err("non representable quote must fail");
    assert!(format!("{precision_error:?}").contains("precision_scale"));
    let zero_error = authoritative_new_coin_quote_amount(&decimal("0.001"), &decimal("1"), 0)
        .expect_err("a quote may not be truncated into a free issuance");
    assert!(format!("{zero_error:?}").contains("precision_scale"));
    let metadata_error = authoritative_new_coin_quote_amount(&decimal("1"), &decimal("1"), 19)
        .expect_err("unsupported database precision must fail closed");
    assert!(format!("{metadata_error:?}").contains("between 0 and 18"));
}

#[test]
fn generated_unlock_fee_is_truncated_to_payment_asset_precision() {
    use super::service::quantize_unlock_fee_amount;

    assert_eq!(
        quantize_unlock_fee_amount(&decimal("1.234567899"), 8).expect("valid precision"),
        decimal("1.23456789")
    );
    assert_eq!(
        quantize_unlock_fee_amount(&decimal("0.000000009"), 8).expect("sub-unit fee"),
        decimal("0.00000000")
    );
    assert!(quantize_unlock_fee_amount(&decimal("1"), 19).is_err());
}

#[test]
fn enabled_unlock_fee_must_use_the_project_quote_asset() {
    use super::service::ensure_unlock_fee_asset_matches_quote_asset;

    assert!(ensure_unlock_fee_asset_matches_quote_asset(true, Some(22), Some(22)).is_ok());
    assert!(ensure_unlock_fee_asset_matches_quote_asset(true, Some(11), Some(22)).is_err());
    assert!(ensure_unlock_fee_asset_matches_quote_asset(true, Some(22), None).is_err());
    assert!(ensure_unlock_fee_asset_matches_quote_asset(false, Some(11), None).is_ok());
}

#[test]
fn new_coin_request_fingerprints_normalize_decimals_and_bind_all_parameters() {
    use super::service::{new_coin_purchase_fingerprint, new_coin_subscription_fingerprint};

    let first = new_coin_subscription_fingerprint(7, 11, 13, &decimal("10.00"), &decimal("4.0"));
    let same = new_coin_subscription_fingerprint(7, 11, 13, &decimal("10"), &decimal("4"));
    let changed = new_coin_subscription_fingerprint(7, 11, 13, &decimal("10"), &decimal("5"));
    assert_eq!(first, same);
    assert_ne!(first, changed);

    assert_ne!(
        new_coin_purchase_fingerprint(7, 11, 17, &decimal("2"), &decimal("3")),
        new_coin_purchase_fingerprint(7, 11, 18, &decimal("2"), &decimal("3"))
    );
}

#[test]
fn unlock_idempotency_keys_are_namespaced_by_issuance_flow() {
    use super::service::new_coin_unlock_idempotency_key;

    let subscription = new_coin_unlock_idempotency_key("new_coin_subscription", "same-key")
        .expect("subscription unlock key");
    let purchase = new_coin_unlock_idempotency_key("new_coin_purchase", "same-key")
        .expect("purchase unlock key");
    assert_ne!(subscription, purchase);
    assert_eq!(subscription, "new_coin_subscription:same-key");
}

#[test]
fn actual_listing_gate_never_uses_planned_time_and_keeps_project_identity() {
    let sources = vec![
        unlock_source("a", 4, at(100)),
        unlock_source("b", 6, at(200)),
    ];
    let before = apply_unlock_rule(
        &UnlockRule::OnActualListing {
            project_id: "7".into(),
            listed: false,
        },
        sources.clone(),
    )
    .unwrap();
    assert_eq!(before.available_amount, amount(0));
    assert_eq!(before.locked_amount, amount(10));
    assert_eq!(before.lock_positions.len(), 1);
    let other = apply_unlock_rule(
        &UnlockRule::OnActualListing {
            project_id: "8".into(),
            listed: false,
        },
        sources.clone(),
    )
    .unwrap();
    assert_ne!(
        before.lock_positions[0].merge_key,
        other.lock_positions[0].merge_key
    );
    let after = apply_unlock_rule(
        &UnlockRule::OnActualListing {
            project_id: "7".into(),
            listed: true,
        },
        sources,
    )
    .unwrap();
    assert_eq!(after.available_amount, amount(10));
    assert_eq!(after.locked_amount, amount(0));
    assert!(after.lock_positions.is_empty());
}
