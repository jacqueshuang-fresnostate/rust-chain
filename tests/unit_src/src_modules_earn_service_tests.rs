use super::*;

fn decimal(value: &str) -> BigDecimal {
    value.parse::<BigDecimal>().unwrap()
}

#[test]
fn numeric_safety_earn_maturity_rejects_timestamp_overflow() {
    earn_matures_at(1).expect("ordinary maturity fits storage");
    assert!(earn_matures_at(100_000).is_err());
    assert!(earn_matures_at(u32::MAX).is_err());
}

#[test]
fn numeric_safety_earn_accepts_trailing_zeros_but_rejects_real_overprecision() {
    for value in [
        "1e-18",
        "99999999999999999999.999999999999999999",
        "1.230000000000000000000",
    ] {
        validate_amount(&decimal(value)).unwrap();
    }
    for value in ["1e20", "1e-19", "1e100000", "1e-100000"] {
        assert!(validate_amount(&decimal(value)).is_err(), "{value}");
    }
    validate_apr_rate(&decimal("0.100000000000")).unwrap();
    assert!(validate_apr_rate(&decimal("1e10")).is_err());
    assert!(validate_fee_rate(&decimal("0.000000001"), "fee").is_err());
}

#[test]
fn subscription_amount_must_fit_asset_precision() {
    validate_amount_asset_precision(&decimal("100.12"), 2)
        .expect("two decimals fit a two-decimal asset");
    validate_amount_asset_precision(&decimal("100.00"), 2).expect("trailing zeros still fit");
    validate_amount_asset_precision(&decimal("100"), 0).expect("integers fit a zero-decimal asset");

    validate_amount_asset_precision(&decimal("100.123"), 2)
        .expect_err("three decimals must not silently truncate on a two-decimal asset");
    validate_amount_asset_precision(&decimal("100.1"), 0)
        .expect_err("a fraction must not fit a zero-decimal asset");
}

#[test]
fn liability_budget_uses_full_term_gross_yield_and_rounds_up() {
    assert_eq!(
        earn_liability_reservation(&decimal("100"), &decimal("0.1"), 365),
        decimal("110")
    );
    assert_eq!(
        earn_liability_reservation(&decimal("1"), &decimal("1"), 1),
        decimal("1.002739726027397261")
    );
    assert_eq!(
        earn_liability_reservation(&decimal("2"), &decimal("0"), 30),
        decimal("2")
    );
}

#[test]
fn exposure_capacity_is_optional_nonnegative_and_precise() {
    for value in [None, Some(decimal("0")), Some(decimal("1.2300"))] {
        validate_exposure_capacity(value.as_ref(), 2).unwrap();
    }
    for value in ["-1", "1.001", "100000000000000000000"] {
        assert!(validate_exposure_capacity(Some(&decimal(value)), 2).is_err());
    }
}
