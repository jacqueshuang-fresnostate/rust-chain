use super::{CrossMarginPositionRisk, evaluate_cross_margin};
use bigdecimal::BigDecimal;
use std::str::FromStr;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).expect("valid decimal")
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
    assert_eq!(state.unrealized_pnl, decimal("5.000000000000000000"));
    assert_eq!(state.interest_amount, decimal("3.000000000000000000"));
    assert_eq!(state.maintenance_margin, decimal("40.000000000000000000"));
    assert!(!state.should_liquidate);
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
