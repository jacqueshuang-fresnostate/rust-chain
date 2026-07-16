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

fn source(id: &str, value: i64, unlock_at: chrono::DateTime<chrono::Utc>) -> LockPositionSource {
    LockPositionSource {
        source_id: id.to_owned(),
        amount: amount(value),
        unlock_at,
    }
}

fn account(available: i64, frozen: i64, locked: i64) -> WalletAccount {
    WalletAccount {
        user_id: "user-1".to_owned(),
        asset_id: "ASSET".to_owned(),
        available: amount(available),
        frozen: amount(frozen),
        locked: amount(locked),
    }
}

#[test]
fn asset_amount_precision_ignores_trailing_zeros() {
    assert_eq!(
        asset_amount_fractional_scale(&decimal("1.230000000000000000")),
        2
    );
    assert!(amount_fits_asset_precision(
        &decimal("1.230000000000000000"),
        2
    ));
    assert!(!amount_fits_asset_precision(&decimal("1.234"), 2));
}

#[test]
fn truncate_amount_to_asset_precision_drops_extra_digits() {
    assert_eq!(
        truncate_amount_to_asset_precision(&decimal("0.019600192108874474"), 8).normalized(),
        decimal("0.01960019").normalized()
    );
    assert_eq!(
        truncate_amount_to_asset_precision(&decimal("-1.239"), 2).normalized(),
        decimal("-1.23").normalized()
    );
}

#[test]
fn tiered_withdraw_fee_uses_min_inclusive_and_max_exclusive_ranges() {
    let tiers = normalize_withdraw_fee_tiers(vec![
        WithdrawFeeTier {
            min_amount: decimal("100"),
            max_amount: Some(decimal("500")),
            fee_rate_percent: decimal("2"),
        },
        WithdrawFeeTier {
            min_amount: decimal("1"),
            max_amount: Some(decimal("100")),
            fee_rate_percent: decimal("1"),
        },
        WithdrawFeeTier {
            min_amount: decimal("500"),
            max_amount: None,
            fee_rate_percent: decimal("3"),
        },
    ])
    .unwrap();

    assert_eq!(
        calculate_withdraw_fee(&decimal("50"), &decimal("0.5"), &tiers, 18).normalized(),
        decimal("0.5").normalized()
    );
    assert_eq!(
        calculate_withdraw_fee(&decimal("100"), &decimal("0.5"), &tiers, 18).normalized(),
        decimal("2").normalized()
    );
    assert_eq!(
        calculate_withdraw_fee(&decimal("500"), &decimal("0.5"), &tiers, 18).normalized(),
        decimal("15").normalized()
    );
}

#[test]
fn tiered_withdraw_fee_falls_back_to_fixed_fee_when_no_range_matches() {
    let tiers = normalize_withdraw_fee_tiers(vec![WithdrawFeeTier {
        min_amount: decimal("10"),
        max_amount: Some(decimal("20")),
        fee_rate_percent: decimal("1"),
    }])
    .unwrap();

    assert_eq!(
        calculate_withdraw_fee(&decimal("5"), &decimal("0.25"), &tiers, 8).normalized(),
        decimal("0.25").normalized()
    );
}

#[test]
fn tiered_withdraw_fee_rejects_overlapping_ranges() {
    let result = normalize_withdraw_fee_tiers(vec![
        WithdrawFeeTier {
            min_amount: decimal("1"),
            max_amount: Some(decimal("100")),
            fee_rate_percent: decimal("1"),
        },
        WithdrawFeeTier {
            min_amount: decimal("99"),
            max_amount: Some(decimal("500")),
            fee_rate_percent: decimal("2"),
        },
    ]);

    assert_eq!(
        result.unwrap_err(),
        "withdraw_fee_tiers ranges must not overlap"
    );
}

#[test]
fn fixed_time_positions_share_aggregation_key() {
    let unlock_at = at(1_700_000_000);

    let positions = create_lock_positions(
        "user-1",
        "ASSET",
        LockSchedule::FixedTime { unlock_at },
        vec![
            source("order-1", 10, unlock_at),
            source("order-2", 15, unlock_at),
        ],
    )
    .unwrap();

    assert_eq!(positions.len(), 1);
    assert_eq!(positions[0].unlock_type, "fixed_time");
    assert_eq!(positions[0].remaining_amount, amount(25));
    assert_eq!(
        positions[0].merge_key,
        fixed_time_merge_key("user-1", "ASSET", unlock_at)
    );
    assert_eq!(positions[0].source_id, None);
}

#[test]
fn relative_period_positions_stay_split_by_source() {
    let unlock_at = at(1_700_000_000);

    let positions = create_lock_positions(
        "user-1",
        "ASSET",
        LockSchedule::RelativePeriod,
        vec![
            source("order-1", 10, unlock_at),
            source("order-2", 15, unlock_at),
        ],
    )
    .unwrap();

    assert_eq!(positions.len(), 2);
    assert_eq!(positions[0].unlock_type, "relative_period");
    assert_eq!(positions[0].source_id.as_deref(), Some("order-1"));
    assert_eq!(positions[0].unlock_at, unlock_at);
    assert_eq!(positions[0].remaining_amount, amount(10));
    assert_eq!(positions[1].source_id.as_deref(), Some("order-2"));
    assert_eq!(positions[1].unlock_at, unlock_at);
    assert_eq!(positions[1].remaining_amount, amount(15));
    assert_ne!(positions[0].merge_key, positions[1].merge_key);
}

#[test]
fn balance_change_rejects_negative_bucket() {
    let mut account = account(10, 2, 3);

    let result =
        account.apply_balance_change(BalanceChange::new(amount(-11), amount(0), amount(0)));

    assert_eq!(
        result,
        Err(WalletDomainError::NegativeBalance {
            bucket: BalanceBucket::Available
        })
    );
    assert_eq!(account.available, amount(10));
    assert_eq!(account.frozen, amount(2));
    assert_eq!(account.locked, amount(3));
}

#[test]
fn locked_balance_matches_active_lock_positions() {
    let account = account(10, 0, 25);
    let unlock_at = at(1_700_000_000);
    let active_positions = vec![
        LockPosition {
            user_id: "user-1".to_owned(),
            asset_id: "ASSET".to_owned(),
            unlock_type: "fixed_time".to_owned(),
            unlock_at,
            remaining_amount: amount(10),
            merge_key: "key-1".to_owned(),
            source_id: None,
        },
        LockPosition {
            user_id: "user-1".to_owned(),
            asset_id: "ASSET".to_owned(),
            unlock_type: "relative_period".to_owned(),
            unlock_at,
            remaining_amount: amount(15),
            merge_key: "key-2".to_owned(),
            source_id: Some("order-1".to_owned()),
        },
        LockPosition {
            user_id: "user-2".to_owned(),
            asset_id: "ASSET".to_owned(),
            unlock_type: "fixed_time".to_owned(),
            unlock_at,
            remaining_amount: amount(99),
            merge_key: "other-user".to_owned(),
            source_id: None,
        },
    ];

    assert_eq!(
        verify_locked_balance_invariant(&account, &active_positions),
        Ok(())
    );

    let mut mismatched = account;
    mismatched.locked = amount(26);

    assert!(matches!(
        verify_locked_balance_invariant(&mismatched, &active_positions),
        Err(WalletDomainError::LockedBalanceInvariantMismatch { .. })
    ));
}
