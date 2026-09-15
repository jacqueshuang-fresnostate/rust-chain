use super::*;

fn decimal(value: &str) -> BigDecimal {
    value.parse::<BigDecimal>().unwrap()
}

fn legs_total(legs: &[EarnPlatformJournalLeg]) -> BigDecimal {
    legs.iter()
        .fold(BigDecimal::from(0), |total, leg| total + leg.amount.clone())
}

fn account_codes(legs: &[EarnPlatformJournalLeg]) -> Vec<&'static str> {
    legs.iter().map(|leg| leg.account_code).collect()
}

#[test]
fn subscription_legs_keep_the_platform_book_balanced() {
    let legs = earn_subscription_journal_legs(&decimal("100.000000000000000000"));

    assert_eq!(
        account_codes(&legs),
        vec!["platform_earn_cash_received", "earn_principal_payable_open"]
    );
    assert_eq!(legs[0].amount, decimal("100.000000000000000000"));
    assert_eq!(legs[1].amount, decimal("-100.000000000000000000"));
    assert_eq!(legs_total(&legs), decimal("0"));
}

#[test]
fn redemption_legs_close_the_payable_and_recognize_yield_and_fee() {
    let legs = earn_redemption_journal_legs(
        &decimal("100.000000000000000000"),
        &decimal("5.000000000000000000"),
        &decimal("1.000000000000000000"),
        &decimal("104.000000000000000000"),
    );

    assert_eq!(
        account_codes(&legs),
        vec![
            "earn_principal_payable_close",
            "platform_earn_redemption_cash",
            "earn_yield_expense",
            "platform_earn_fee_income",
        ]
    );
    assert_eq!(legs[0].amount, decimal("100.000000000000000000"));
    assert_eq!(legs[1].amount, decimal("-104.000000000000000000"));
    assert_eq!(legs[2].amount, decimal("5.000000000000000000"));
    assert_eq!(legs[3].amount, decimal("-1.000000000000000000"));
    assert_eq!(legs_total(&legs), decimal("0"));
}

#[test]
fn zero_amount_legs_are_omitted_because_the_journal_rejects_them() {
    let legs = earn_redemption_journal_legs(
        &decimal("100.000000000000000000"),
        &decimal("0"),
        &decimal("0"),
        &decimal("100.000000000000000000"),
    );

    assert_eq!(
        account_codes(&legs),
        vec![
            "earn_principal_payable_close",
            "platform_earn_redemption_cash",
        ]
    );
    assert_eq!(legs_total(&legs), decimal("0"));
}

#[test]
fn redemption_legs_stay_balanced_when_fees_exceed_the_whole_position() {
    // 三项费率叠加超过本金加毛收益时，用户实收被兜底到零，费用收入腿必须按可收上限截断。
    let legs = earn_redemption_journal_legs(
        &decimal("100.000000000000000000"),
        &decimal("1000.000000000000000000"),
        &decimal("1200.000000000000000000"),
        &decimal("0"),
    );

    assert_eq!(
        account_codes(&legs),
        vec![
            "earn_principal_payable_close",
            "earn_yield_expense",
            "platform_earn_fee_income",
        ]
    );
    assert_eq!(legs[2].amount, decimal("-1100.000000000000000000"));
    assert_eq!(legs_total(&legs), decimal("0"));
}
