use super::*;
use chrono::TimeZone;

fn decimal(value: &str) -> BigDecimal {
    value.parse::<BigDecimal>().unwrap()
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
