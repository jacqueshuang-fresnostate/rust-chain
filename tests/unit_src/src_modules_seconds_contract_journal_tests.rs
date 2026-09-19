use super::*;

fn sum(legs: &[WalletPlatformJournalLeg]) -> BigDecimal {
    legs.iter()
        .fold(BigDecimal::from(0), |sum, leg| sum + &leg.amount)
}

#[test]
fn opening_is_liability_transfer_not_revenue() {
    let opening = open_legs(&BigDecimal::from(100));
    assert_eq!(sum(&opening), BigDecimal::from(0));
    assert_eq!(opening.len(), 2);
    assert!(
        opening
            .iter()
            .all(|leg| !leg.account_code.ends_with("income"))
    );
}

#[test]
fn settlement_records_loss_win_and_break_even_without_zero_legs() {
    let stake = BigDecimal::from(100);
    for payout in [0, 100, 180] {
        let settlement = settlement_legs(&stake, &BigDecimal::from(payout));
        assert_eq!(sum(&settlement), BigDecimal::from(0));
        assert!(settlement.iter().all(|leg| leg.amount != 0));
        let pending = settlement
            .iter()
            .find(|leg| leg.account_code == "platform_seconds_pending_liability")
            .unwrap();
        assert_eq!(pending.amount, stake);
        if payout == 0 {
            assert_eq!(
                settlement
                    .iter()
                    .find(|leg| leg.account_code == "platform_seconds_income")
                    .unwrap()
                    .amount,
                BigDecimal::from(-100)
            );
        }
        if payout == 180 {
            assert_eq!(
                settlement
                    .iter()
                    .find(|leg| leg.account_code == "platform_seconds_payout_expense")
                    .unwrap()
                    .amount,
                BigDecimal::from(80)
            );
        }
    }
}
