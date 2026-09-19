use super::*;

#[test]
fn fee_refund_reverses_exact_accounts_and_zero_fees_do_not_create_rows() {
    assert!(fee_legs(&BigDecimal::from(0)).is_empty());
    assert!(refund_legs(&BigDecimal::from(0)).is_empty());
    let fee = BigDecimal::from(2);
    for (original, reversal) in fee_legs(&fee).iter().zip(refund_legs(&fee).iter()) {
        assert_eq!(original.account_code, reversal.account_code);
        assert_eq!(&original.amount + &reversal.amount, BigDecimal::from(0));
    }
}

#[test]
fn actual_prediction_payout_is_balanced_for_win_loss_and_capped_return() {
    for payout in [0, 70, 100, 180] {
        let entries = settlement_legs(&BigDecimal::from(100), &BigDecimal::from(payout));
        let sum = entries
            .iter()
            .fold(BigDecimal::from(0), |sum, leg| sum + &leg.amount);
        assert_eq!(sum, BigDecimal::from(0));
        assert!(entries.iter().all(|leg| leg.amount != 0));
    }
}
