use super::*;

fn assert_balanced(entries: &[WalletPlatformJournalLeg]) {
    assert_eq!(
        entries
            .iter()
            .map(|leg| leg.amount.clone())
            .sum::<BigDecimal>(),
        BigDecimal::from(0)
    );
    assert!(entries.iter().all(|leg| leg.amount != 0));
}

#[test]
fn margin_open_cancel_are_opposite_and_do_not_book_income() {
    let open = opening_legs(&100.into());
    let cancel = cancellation_legs(&100.into());
    assert_balanced(&open);
    assert_balanced(&cancel);
    for (a, b) in open.iter().zip(cancel) {
        assert_eq!(a.account_code, b.account_code);
        assert_eq!(&a.amount + b.amount, BigDecimal::from(0));
    }
}

#[test]
fn close_records_actual_signed_payout_and_only_real_shortfall() {
    for (payout, pnl, debt) in [(108, 10, 0), (78, -20, 0), (0, -120, 22), (-22, -120, 0)] {
        let entries = closing_legs(&100.into(), &payout.into(), &pnl.into(), &2.into());
        assert_balanced(&entries);
        let actual_debt = entries
            .iter()
            .find(|leg| leg.account_code == "platform_margin_bad_debt_expense")
            .map(|leg| leg.amount.clone())
            .unwrap_or_default();
        assert_eq!(actual_debt, BigDecimal::from(debt));
    }
}

#[test]
fn cross_liquidation_consumes_account_wallet_and_counts_bad_debt_once() {
    for (pnl, debt, income) in [(-120, 7, 0), (-110, 0, -3)] {
        let entries = cross_liquidation_legs(&100.into(), &15.into(), &pnl.into(), &2.into());
        assert_balanced(&entries);
        for (code, expected) in [
            ("platform_margin_bad_debt_expense", debt),
            ("platform_margin_liquidation_income", income),
        ] {
            assert_eq!(
                entries
                    .iter()
                    .find(|leg| leg.account_code == code)
                    .map(|leg| leg.amount.clone())
                    .unwrap_or_default(),
                BigDecimal::from(expected)
            );
        }
    }
}
