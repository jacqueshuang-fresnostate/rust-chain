use super::*;
use chrono::TimeZone;

fn decimal(value: &str) -> BigDecimal {
    value.parse().unwrap()
}

#[test]
fn margin_interest_batch_partition_is_exact_at_18_and_8_digits() {
    for (principal, rate, precision, hours) in [
        ("0.00000000015", "0.00000001", 18, 2),
        ("0.00000015", "0.01", 8, 20),
        ("123.45678901", "0.00123456", 8, 101),
    ] {
        let mut total = decimal("0");
        let mut remainder = decimal("0");
        for _ in 0..hours {
            let (delta, next) = interest_increment(
                &decimal(principal),
                &decimal(rate),
                &remainder,
                1,
                precision,
            )
            .unwrap();
            total += delta;
            remainder = next;
        }
        let (whole, whole_remainder) = interest_increment(
            &decimal(principal),
            &decimal(rate),
            &decimal("0"),
            hours,
            precision,
        )
        .unwrap();
        assert_eq!(total, whole);
        assert_eq!(remainder, whole_remainder);
    }
}

#[test]
fn margin_interest_carry_survives_partial_close_without_rebilling_accrued_debt() {
    let (first, carry) = interest_increment(
        &decimal("0.00000000015"),
        &decimal("0.00000001"),
        &decimal("0"),
        1,
        18,
    )
    .unwrap();
    assert_eq!(first, decimal("0.000000000000000001"));
    let historical = decimal("1.000000000000000001");
    let remaining_debt = historical + &first - decimal("0.5");
    let (next, residual) = interest_increment(
        &decimal("0.000000000075"),
        &decimal("0.00000001"),
        &carry,
        2,
        18,
    )
    .unwrap();
    assert_eq!(next, decimal("0.000000000000000002"));
    assert_eq!(residual, decimal("0"));
    assert_eq!(remaining_debt + next, decimal("0.500000000000000004"));
}

#[test]
fn margin_interest_storage_and_duration_fail_closed() {
    assert!(
        interest_increment(
            &decimal("99999999999999999999"),
            &decimal("1"),
            &decimal("0"),
            2,
            18,
        )
        .is_err()
    );
    let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    assert!(billed_window_end(start, u64::MAX).is_err());
    assert!(billed_window_end(DateTime::<Utc>::MAX_UTC, 1).is_err());
    assert!(billed_window_end(start, 1_000_001).is_err());
    let edge = Utc.with_ymd_and_hms(2038, 1, 19, 2, 14, 7).unwrap();
    assert!(billed_window_end(edge, 1).is_ok());
    assert!(billed_window_end(edge, 2).is_err());
}

#[test]
fn margin_interest_migration_is_forward_only_and_keeps_old_checkpoints() {
    let migration = include_str!("../../migrations/0141_margin_interest_remainder.sql");
    assert!(migration.contains("DECIMAL(26,26)"));
    assert!(migration.contains("SET interest_remainder = 0"));
    assert!(!migration.contains("SET interest_amount"));
    assert!(!migration.contains("SET interest_accrued_at"));
    assert!(!migration.contains("wallet_accounts"));
}
