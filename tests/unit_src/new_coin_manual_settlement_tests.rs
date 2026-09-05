use super::*;
use std::str::FromStr;

fn d(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

#[test]
fn manual_settlement_full_partial_and_zero_conserve_frozen_quote() {
    for (quantity, payment, refund) in [("10", "25", "0"), ("4", "10", "15"), ("0", "0", "25")] {
        let (paid, returned) =
            manual_new_coin_settlement_amounts(&d("10"), &d(quantity), &d("2.5"), &d("25"), 2)
                .unwrap();
        assert_eq!(paid, d(payment));
        assert_eq!(returned, d(refund));
        assert_eq!(paid + returned, d("25"));
    }
}

#[test]
fn manual_settlement_rejects_overallocation_negative_and_invalid_snapshot() {
    for quantity in ["-1", "11"] {
        assert!(
            manual_new_coin_settlement_amounts(&d("10"), &d(quantity), &d("2.5"), &d("25"), 2)
                .is_err()
        );
    }
    assert!(manual_new_coin_settlement_amounts(&d("10"), &d("4"), &d("2.5"), &d("26"), 2).is_err());
    assert!(
        manual_new_coin_settlement_amounts(&d("10"), &d("0.001"), &d("2.5"), &d("25"), 2).is_err()
    );
}
