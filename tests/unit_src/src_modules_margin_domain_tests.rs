use super::{
    CrossMarginPositionRisk, MarginOrderType, allocate_cross_margin_payouts, evaluate_cross_margin,
    margin_limit_order_is_triggered, margin_position_payout_amount, validate_margin_limit_price,
};
use bigdecimal::BigDecimal;
use std::str::FromStr;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).expect("valid decimal")
}

#[test]
fn margin_limit_trigger_boundaries_match_long_and_short_intent() {
    let limit = decimal("100");

    assert!(margin_limit_order_is_triggered("long", &limit, &decimal("99")).unwrap());
    assert!(margin_limit_order_is_triggered("long", &limit, &decimal("100")).unwrap());
    assert!(!margin_limit_order_is_triggered("long", &limit, &decimal("101")).unwrap());

    assert!(!margin_limit_order_is_triggered("short", &limit, &decimal("99")).unwrap());
    assert!(margin_limit_order_is_triggered("short", &limit, &decimal("100")).unwrap());
    assert!(margin_limit_order_is_triggered("short", &limit, &decimal("101")).unwrap());
}

#[test]
fn margin_limit_rules_reject_invalid_prices_direction_and_precision() {
    assert!(margin_limit_order_is_triggered("sideways", &decimal("100"), &decimal("100")).is_err());
    assert!(margin_limit_order_is_triggered("long", &decimal("0"), &decimal("100")).is_err());
    assert!(margin_limit_order_is_triggered("short", &decimal("100"), &decimal("0")).is_err());

    assert!(validate_margin_limit_price(&decimal("1.2300"), 2).is_ok());
    assert!(validate_margin_limit_price(&decimal("1.234"), 2).is_err());
    assert!(validate_margin_limit_price(&decimal("0"), 8).is_err());
    assert!(validate_margin_limit_price(&decimal("1"), -1).is_err());
}

#[test]
fn margin_order_type_keeps_market_compatibility_and_rejects_unknown_values() {
    assert_eq!(
        MarginOrderType::parse(None).unwrap(),
        MarginOrderType::Market
    );
    assert_eq!(
        MarginOrderType::parse(Some(" LIMIT ")).unwrap(),
        MarginOrderType::Limit
    );
    assert!(MarginOrderType::parse(Some("stop")).is_err());
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
