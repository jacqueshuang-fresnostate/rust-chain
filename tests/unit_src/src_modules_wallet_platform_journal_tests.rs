use super::*;

fn decimal(value: &str) -> BigDecimal {
    value.parse::<BigDecimal>().unwrap()
}

fn legs_total(legs: &[WalletPlatformJournalLeg]) -> BigDecimal {
    legs.iter()
        .fold(BigDecimal::from(0), |total, leg| total + leg.amount.clone())
}

fn account_codes(legs: &[WalletPlatformJournalLeg]) -> Vec<&'static str> {
    legs.iter().map(|leg| leg.account_code).collect()
}

#[test]
fn deposit_credit_legs_balance_custody_liability_and_fee() {
    let legs = deposit_credit_journal_legs(
        &decimal("3.000000000000000000"),
        &decimal("2.990000000000000000"),
    );

    assert_eq!(
        account_codes(&legs),
        vec![
            "platform_deposit_cash_received",
            "user_deposit_liability_open",
            "platform_deposit_fee_income",
        ]
    );
    assert_eq!(legs[0].amount, decimal("3.000000000000000000"));
    assert_eq!(legs[1].amount, decimal("-2.990000000000000000"));
    assert_eq!(legs[2].amount, decimal("-0.010000000000000000"));
    assert_eq!(legs_total(&legs), decimal("0"));
}

#[test]
fn deposit_reversal_legs_are_the_exact_negation_of_the_credit_legs() {
    let gross = decimal("3.000000000000000000");
    let net = decimal("2.990000000000000000");
    let credit = deposit_credit_journal_legs(&gross, &net);
    let reversal = deposit_reversal_journal_legs(&gross, &net);

    assert_eq!(account_codes(&credit), account_codes(&reversal));
    for (credited, reversed) in credit.iter().zip(reversal.iter()) {
        assert_eq!(credited.amount, -reversed.amount.clone());
    }
    assert_eq!(legs_total(&reversal), decimal("0"));
}

#[test]
fn fee_free_deposits_omit_the_zero_fee_leg() {
    let gross = decimal("1.000000000000000000");
    let net = gross.clone();
    let legs = deposit_credit_journal_legs(&gross, &net);

    assert_eq!(
        account_codes(&legs),
        vec![
            "platform_deposit_cash_received",
            "user_deposit_liability_open",
        ]
    );
    assert_eq!(legs_total(&legs), decimal("0"));
    assert_eq!(
        legs_total(&deposit_reversal_journal_legs(&gross, &net)),
        decimal("0")
    );
}

/// 入账与冲正两条路径都必须把平台腿写进同一事务，并用各自独立的 transaction_key 区分。
#[test]
fn deposit_credit_and_reversal_both_write_platform_legs() {
    let source = include_str!("../../src/modules/wallet/infrastructure/deposits.rs");

    assert!(source.contains("deposit_credit_journal_legs"));
    assert!(source.contains("deposit_reversal_journal_legs"));
    assert!(source.contains("insert_wallet_platform_journal_legs_in_tx"));
    assert!(source.contains("wallet_deposit:{}:credit"));
    assert!(source.contains("wallet_deposit:{}:reverse"));
}

#[test]
fn withdrawal_confirmation_legs_balance_payout_and_retained_fee() {
    let legs = withdrawal_confirm_journal_legs(
        &decimal("10.000000000000000000"),
        &decimal("0.500000000000000000"),
        &decimal("10.500000000000000000"),
    );

    assert_eq!(
        account_codes(&legs),
        vec![
            "user_withdrawal_liability_close",
            "platform_withdrawal_cash_paid",
            "platform_withdrawal_fee_income",
        ]
    );
    assert_eq!(legs[0].amount, decimal("10.500000000000000000"));
    assert_eq!(legs[1].amount, decimal("-10.000000000000000000"));
    assert_eq!(legs[2].amount, decimal("-0.500000000000000000"));
    assert_eq!(legs_total(&legs), decimal("0"));
}

/// 只有链上确认这一步资金真正离开平台，平台腿必须与那次 frozen 扣除写在同一事务。
#[test]
fn withdrawal_confirmation_writes_platform_legs() {
    let source = include_str!("../../src/modules/wallet/infrastructure/withdrawals.rs");

    assert!(source.contains("withdrawal_confirm_journal_legs"));
    assert!(source.contains("WALLET_WITHDRAWAL_JOURNAL_CONTEXT"));
    assert!(source.contains("wallet_withdrawal:{}:confirm"));
}
