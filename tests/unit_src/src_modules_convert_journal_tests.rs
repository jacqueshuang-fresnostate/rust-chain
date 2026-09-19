use super::*;

#[test]
fn convert_source_and_target_balance_separately_and_fee_is_not_extra_debit() {
    for fee in [0, 2] {
        let source = source_legs(&BigDecimal::from(100), &BigDecimal::from(fee));
        assert_eq!(source[0].amount, BigDecimal::from(100));
        assert_eq!(source[1].amount, BigDecimal::from(fee - 100));
        assert_eq!(
            source
                .iter()
                .map(|leg| leg.amount.clone())
                .sum::<BigDecimal>(),
            BigDecimal::from(0)
        );
        assert!(source.iter().all(|leg| leg.amount != 0));
    }
    let target = target_legs(&BigDecimal::from(7));
    assert_eq!(
        target
            .iter()
            .map(|leg| leg.amount.clone())
            .sum::<BigDecimal>(),
        BigDecimal::from(0)
    );
}
