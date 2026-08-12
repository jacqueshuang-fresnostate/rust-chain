use super::{
    CrossMarginPositionRisk, allocate_cross_margin_payouts, evaluate_cross_margin,
    margin_position_payout_amount,
};
use bigdecimal::BigDecimal;
use std::str::FromStr;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).expect("valid decimal")
}

#[test]
fn position_payout_deducts_interest_and_never_returns_a_negative_amount() {
    assert_eq!(
        margin_position_payout_amount(&decimal("100"), Some(&decimal("25")), &decimal("3")),
        decimal("122.000000000000000000")
    );
    assert_eq!(
        margin_position_payout_amount(&decimal("100"), Some(&decimal("-105")), &decimal("3")),
        decimal("0.000000000000000000")
    );
    assert_eq!(
        margin_position_payout_amount(&decimal("100"), None, &decimal("3")),
        decimal("0.000000000000000000")
    );
}

#[test]
fn cross_margin_aggregates_equity_pnl_interest_and_maintenance() {
    let state = evaluate_cross_margin(
        &decimal("100"),
        &decimal("50"),
        &[
            CrossMarginPositionRisk {
                unrealized_pnl: decimal("20"),
                interest_amount: decimal("2"),
                maintenance_margin: decimal("10"),
            },
            CrossMarginPositionRisk {
                unrealized_pnl: decimal("-15"),
                interest_amount: decimal("1"),
                maintenance_margin: decimal("30"),
            },
        ],
    );

    assert_eq!(state.equity, decimal("152.000000000000000000"));
    assert_eq!(state.portfolio_equity, decimal("52.000000000000000000"));
    assert_eq!(state.unrealized_pnl, decimal("5.000000000000000000"));
    assert_eq!(state.interest_amount, decimal("3.000000000000000000"));
    assert_eq!(state.maintenance_margin, decimal("40.000000000000000000"));
    assert!(!state.should_liquidate);
}

#[test]
fn cross_margin_payout_allocation_never_exceeds_portfolio_equity() {
    let payouts = allocate_cross_margin_payouts(
        &[decimal("70"), decimal("-100"), decimal("30")],
        &decimal("0"),
    );
    assert_eq!(payouts, vec![decimal("0"), decimal("0"), decimal("0")]);

    let payouts = allocate_cross_margin_payouts(
        &[decimal("70"), decimal("-50"), decimal("30")],
        &decimal("50"),
    );
    assert_eq!(
        payouts.iter().cloned().sum::<BigDecimal>(),
        decimal("50.000000000000000000")
    );
    assert_eq!(payouts[0], decimal("35.000000000000000000"));
    assert_eq!(payouts[1], decimal("0.000000000000000000"));
    assert_eq!(payouts[2], decimal("15.000000000000000000"));
}

#[test]
fn cross_margin_liquidates_when_combined_equity_reaches_maintenance() {
    let state = evaluate_cross_margin(
        &decimal("0"),
        &decimal("100"),
        &[CrossMarginPositionRisk {
            unrealized_pnl: decimal("-80"),
            interest_amount: decimal("5"),
            maintenance_margin: decimal("15"),
        }],
    );

    assert_eq!(state.equity, decimal("15.000000000000000000"));
    assert!(state.should_liquidate);
}
