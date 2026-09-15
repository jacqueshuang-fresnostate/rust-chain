use super::*;
use chrono::TimeZone;

fn decimal(value: &str) -> BigDecimal {
    value.parse::<BigDecimal>().unwrap()
}

#[test]
fn redemption_amounts_are_truncated_toward_zero_to_asset_precision() {
    let subscribed_at = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let matures_at = subscribed_at + chrono::TimeDelta::days(365);
    let amounts = calculate_earn_redemption_amounts(
        EarnRedemptionTerms {
            amount: &decimal("100.000000000000000000"),
            apr_rate: &decimal("0.03333000"),
            term_days: 365,
            subscribed_at,
            matures_at,
            redemption_fee_rate: &decimal("0.00100000"),
            maturity_profit_fee_rate: &decimal("0"),
            early_redeem_fee_basis: EARLY_REDEEM_FEE_BASIS_NONE,
            early_redeem_fee_rate: &decimal("0"),
        },
        matures_at,
    );
    let quantized = quantize_earn_redemption_amounts(amounts, 2);

    assert_eq!(quantized.principal_amount, decimal("100.00"));
    assert_eq!(quantized.gross_yield_amount, decimal("3.33"));
    assert_eq!(quantized.redemption_fee_amount, decimal("0.10"));
    assert_eq!(quantized.fee_amount, decimal("0.10"));
    assert_eq!(quantized.redeem_amount, decimal("103.23"));
    assert_eq!(
        quantized.redeem_amount,
        quantized.principal_amount.clone() + quantized.gross_yield_amount.clone()
            - quantized.fee_amount.clone()
    );
}

#[test]
fn calculates_maturity_profit_fee_on_full_term_yield() {
    let subscribed_at = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let matures_at = subscribed_at + chrono::TimeDelta::days(365);
    let amounts = calculate_earn_redemption_amounts(
        EarnRedemptionTerms {
            amount: &decimal("100.000000000000000000"),
            apr_rate: &decimal("0.10000000"),
            term_days: 365,
            subscribed_at,
            matures_at,
            redemption_fee_rate: &decimal("0"),
            maturity_profit_fee_rate: &decimal("0.10000000"),
            early_redeem_fee_basis: EARLY_REDEEM_FEE_BASIS_NONE,
            early_redeem_fee_rate: &decimal("0"),
        },
        matures_at,
    );

    assert_eq!(amounts.gross_yield_amount, decimal("10.000000000000000000"));
    assert_eq!(
        amounts.maturity_profit_fee_amount,
        decimal("1.000000000000000000")
    );
    assert_eq!(amounts.yield_amount, decimal("9.000000000000000000"));
    assert_eq!(amounts.redeem_amount, decimal("109.000000000000000000"));
}

#[test]
fn calculates_early_redeem_principal_fee_with_accrued_yield() {
    let subscribed_at = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let matures_at = subscribed_at + chrono::TimeDelta::days(365);
    let now = subscribed_at + chrono::TimeDelta::days(182);
    let amounts = calculate_earn_redemption_amounts(
        EarnRedemptionTerms {
            amount: &decimal("100.000000000000000000"),
            apr_rate: &decimal("0.10000000"),
            term_days: 365,
            subscribed_at,
            matures_at,
            redemption_fee_rate: &decimal("0"),
            maturity_profit_fee_rate: &decimal("0"),
            early_redeem_fee_basis: EARLY_REDEEM_FEE_BASIS_PRINCIPAL,
            early_redeem_fee_rate: &decimal("0.02000000"),
        },
        now,
    );

    assert_eq!(amounts.gross_yield_amount, decimal("4.986301369863013698"));
    assert_eq!(
        amounts.early_redeem_fee_amount,
        decimal("2.000000000000000000")
    );
    assert_eq!(amounts.redeem_amount, decimal("102.986301369863013698"));
}
