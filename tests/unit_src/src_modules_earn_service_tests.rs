use super::*;

fn decimal(value: &str) -> BigDecimal {
    value.parse::<BigDecimal>().unwrap()
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
