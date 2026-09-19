use super::*;

fn decimal(value: &str) -> BigDecimal {
    value.parse().unwrap()
}

#[test]
fn margin_source_rejects_asset_and_storage_precision_without_rounding() {
    assert!(validate_input_amount(&decimal("1.0000000000000000001"), 18, "margin").is_err());
    assert!(validate_input_amount(&decimal("1.000000001"), 8, "margin").is_err());
    assert!(validate_input_amount(&decimal("1.2300000000000000000"), 8, "margin").is_ok());
    assert!(validate_input_amount(&decimal("0.000000000000000001"), 18, "margin").is_ok());
    assert!(validate_input_amount(&decimal("100000000000000000000"), 18, "margin").is_err());
    assert!(validate_input_amount(&decimal("1e100000"), 18, "margin").is_err());
    assert!(validate_input_amount(&decimal("1"), 19, "margin").is_err());
}

#[test]
fn margin_generated_notional_checks_overflow_and_preserves_existing_wallet_dust() {
    assert!(
        generated_amount(
            &(decimal("99999999999999999999") * decimal("2")),
            8,
            "notional"
        )
        .is_err()
    );
    let pnl = generated_amount(&decimal("-0.123456789"), 8, "pnl").unwrap();
    assert_eq!(pnl, decimal("-0.12345678"));
    let before = decimal("10.000000000000000001");
    let after = &before + &pnl;
    assert_eq!(&after - &before, pnl);
    assert_eq!(after, decimal("9.876543220000000001"));
}

#[test]
fn margin_asset_slice_preserves_historical_subasset_amounts() {
    use crate::modules::margin::domain::allocate_margin_close_slice_at_precision;
    let margin = decimal("1.000000000000000001");
    let notional = decimal("3.000000000000000003");
    let borrowed = &notional - &margin;
    let interest = decimal("0.010000000000000001");
    let slice =
        allocate_margin_close_slice_at_precision(&margin, &notional, &borrowed, &interest, 37, 8)
            .unwrap();
    assert_eq!(slice.close_margin_amount, decimal("0.37"));
    assert_eq!(
        &slice.close_margin_amount + &slice.remaining_margin_amount,
        margin
    );
    assert_eq!(
        &slice.close_notional_amount + &slice.remaining_notional_amount,
        notional
    );
    assert_eq!(
        &slice.close_borrowed_amount + &slice.remaining_borrowed_amount,
        borrowed
    );
    assert_eq!(
        &slice.close_interest_amount + &slice.remaining_interest_amount,
        interest
    );
    let final_slice = allocate_margin_close_slice_at_precision(
        &slice.remaining_margin_amount,
        &slice.remaining_notional_amount,
        &slice.remaining_borrowed_amount,
        &slice.remaining_interest_amount,
        100,
        8,
    )
    .unwrap();
    assert_eq!(
        final_slice.close_margin_amount,
        slice.remaining_margin_amount
    );
    assert_eq!(
        final_slice.close_interest_amount,
        slice.remaining_interest_amount
    );
    assert_eq!(final_slice.remaining_margin_amount, decimal("0"));
    assert!(
        allocate_margin_close_slice_at_precision(
            &decimal("0.00000001"),
            &decimal("0.00000002"),
            &decimal("0.00000001"),
            &decimal("0"),
            1,
            8,
        )
        .is_err()
    );
}
