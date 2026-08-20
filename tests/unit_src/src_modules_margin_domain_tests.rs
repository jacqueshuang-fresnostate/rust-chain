use super::{
    CROSS_NET_DELTA_EPSILON, CrossMarginEstimateStatus, CrossMarginPositionRisk,
    CrossMarginReferencePosition, MarginOrderType, MarginPositionDisplayInput,
    cross_margin_liquidation_settlement, estimate_cross_margin_conditional_price,
    evaluate_cross_margin, margin_limit_order_is_triggered, margin_position_display_metrics,
    margin_position_payout_amount, validate_margin_limit_price,
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
fn position_display_metrics_match_isolated_long_risk_geometry() {
    let margin_amount = decimal("20");
    let notional_amount = decimal("100");
    let interest_amount = decimal("1.5");
    let entry_price = decimal("100");
    let mark_price = decimal("84");
    let unrealized_pnl = decimal("-16");
    let equity = decimal("2.5");
    let maintenance_margin = decimal("5");
    let metrics = margin_position_display_metrics(MarginPositionDisplayInput {
        margin_mode: "isolated",
        direction: "long",
        margin_amount: &margin_amount,
        notional_amount: &notional_amount,
        interest_amount: &interest_amount,
        entry_price: &entry_price,
        mark_price: &mark_price,
        unrealized_pnl: &unrealized_pnl,
        equity: &equity,
        maintenance_margin: &maintenance_margin,
    })
    .unwrap();

    assert_eq!(metrics.position_quantity, decimal("1.000000000000000000"));
    assert_eq!(metrics.return_rate, Some(decimal("-0.800000000000000000")));
    assert_eq!(metrics.margin_ratio, Some(decimal("0.500000000000000000")));
    assert_eq!(
        metrics.estimated_liquidation_price,
        Some(decimal("86.500000000000000000"))
    );
    assert_eq!(
        metrics.liquidation_distance_rate,
        Some(decimal("0.029761904761904761"))
    );
}

#[test]
fn position_display_metrics_do_not_invent_cross_liquidation_price() {
    let margin_amount = decimal("20");
    let notional_amount = decimal("100");
    let interest_amount = decimal("1");
    let entry_price = decimal("100");
    let mark_price = decimal("110");
    let unrealized_pnl = decimal("-10");
    let equity = decimal("9");
    let maintenance_margin = decimal("5");
    let metrics = margin_position_display_metrics(MarginPositionDisplayInput {
        margin_mode: "cross",
        direction: "short",
        margin_amount: &margin_amount,
        notional_amount: &notional_amount,
        interest_amount: &interest_amount,
        entry_price: &entry_price,
        mark_price: &mark_price,
        unrealized_pnl: &unrealized_pnl,
        equity: &equity,
        maintenance_margin: &maintenance_margin,
    })
    .unwrap();

    assert_eq!(metrics.position_quantity, decimal("1.000000000000000000"));
    assert_eq!(metrics.estimated_liquidation_price, None);
    assert_eq!(metrics.liquidation_distance_rate, None);
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

#[test]
fn cross_margin_exact_hedge_keeps_gross_maintenance_and_interest_boundary() {
    let hedge_risks = |interest_per_leg: &str| {
        [
            CrossMarginPositionRisk {
                unrealized_pnl: decimal("-20"),
                interest_amount: decimal(interest_per_leg),
                maintenance_margin: decimal("5"),
            },
            CrossMarginPositionRisk {
                unrealized_pnl: decimal("20"),
                interest_amount: decimal(interest_per_leg),
                maintenance_margin: decimal("5"),
            },
        ]
    };
    let safe = evaluate_cross_margin(&decimal("0"), &decimal("40"), &hedge_risks("10"));
    assert_eq!(safe.unrealized_pnl, decimal("0.000000000000000000"));
    assert_eq!(safe.maintenance_margin, decimal("10.000000000000000000"));
    assert_eq!(safe.equity, decimal("20.000000000000000000"));
    assert!(!safe.should_liquidate);

    // 在标记价净敏感度为零时，再增加恰好等于 Buffer=10 的总利息仍会触发等号强平。
    let at_interest_boundary =
        evaluate_cross_margin(&decimal("0"), &decimal("40"), &hedge_risks("15"));
    assert_eq!(
        at_interest_boundary.equity,
        decimal("10.000000000000000000")
    );
    assert_eq!(
        at_interest_boundary.maintenance_margin,
        decimal("10.000000000000000000")
    );
    assert!(at_interest_boundary.should_liquidate);
}

#[test]
fn cross_margin_conditional_price_solves_partial_long_and_short_hedges() {
    let mark = decimal("100");
    let buffer = decimal("30");
    let entry = decimal("100");
    let long_notional = decimal("100");
    let short_notional = decimal("50");
    let long_leaning = [
        CrossMarginReferencePosition {
            pair_id: 7,
            direction: "long",
            notional_amount: &long_notional,
            entry_price: &entry,
        },
        CrossMarginReferencePosition {
            pair_id: 7,
            direction: "short",
            notional_amount: &short_notional,
            entry_price: &entry,
        },
    ];
    let estimate =
        estimate_cross_margin_conditional_price(7, &mark, &buffer, 2, &long_leaning).unwrap();

    assert_eq!(estimate.status, CrossMarginEstimateStatus::Estimated);
    assert_eq!(estimate.net_quantity, decimal("0.500000000000000000"));
    assert_eq!(estimate.gross_quantity, decimal("1.500000000000000000"));
    assert_eq!(estimate.price, Some(decimal("40.000000000000000000")));
    assert_eq!(
        estimate.distance_rate,
        Some(decimal("0.600000000000000000"))
    );
    let solved = buffer.clone()
        + estimate.net_quantity.clone() * (estimate.price.clone().unwrap() - mark.clone());
    let adverse =
        buffer.clone() + estimate.net_quantity.clone() * (decimal("39.99") - mark.clone());
    let favorable =
        buffer.clone() + estimate.net_quantity.clone() * (decimal("40.01") - mark.clone());
    assert_eq!(solved.with_scale(18), decimal("0.000000000000000000"));
    assert!(adverse < 0);
    assert!(favorable > 0);

    let short_leaning = [
        CrossMarginReferencePosition {
            pair_id: 7,
            direction: "short",
            notional_amount: &long_notional,
            entry_price: &entry,
        },
        CrossMarginReferencePosition {
            pair_id: 7,
            direction: "long",
            notional_amount: &short_notional,
            entry_price: &entry,
        },
    ];
    let estimate =
        estimate_cross_margin_conditional_price(7, &mark, &buffer, 2, &short_leaning).unwrap();
    assert_eq!(estimate.status, CrossMarginEstimateStatus::Estimated);
    assert_eq!(estimate.net_quantity, decimal("-0.500000000000000000"));
    assert_eq!(estimate.price, Some(decimal("160.000000000000000000")));
    let solved = buffer.clone()
        + estimate.net_quantity.clone() * (estimate.price.clone().unwrap() - mark.clone());
    let adverse =
        buffer.clone() + estimate.net_quantity.clone() * (decimal("160.01") - mark.clone());
    let favorable =
        buffer.clone() + estimate.net_quantity.clone() * (decimal("159.99") - mark.clone());
    assert_eq!(solved.with_scale(18), decimal("0.000000000000000000"));
    assert!(adverse < 0);
    assert!(favorable > 0);
}

#[test]
fn cross_margin_conditional_price_rounds_one_tick_toward_the_conservative_side() {
    let mark = decimal("100");
    let buffer = decimal("10.001");
    let entry = decimal("100");
    let notional = decimal("100");
    let long = [CrossMarginReferencePosition {
        pair_id: 8,
        direction: "long",
        notional_amount: &notional,
        entry_price: &entry,
    }];
    let long_estimate =
        estimate_cross_margin_conditional_price(8, &mark, &buffer, 2, &long).unwrap();
    assert_eq!(long_estimate.status, CrossMarginEstimateStatus::Estimated);
    assert_eq!(long_estimate.price, Some(decimal("90.000000000000000000")));
    let at_displayed_long = buffer.clone()
        + long_estimate.net_quantity.clone()
            * (long_estimate.price.clone().unwrap() - mark.clone());
    let one_adverse_long =
        buffer.clone() + long_estimate.net_quantity.clone() * (decimal("89.99") - mark.clone());
    assert!(at_displayed_long > 0);
    assert!(one_adverse_long < 0);

    let short = [CrossMarginReferencePosition {
        pair_id: 8,
        direction: "short",
        notional_amount: &notional,
        entry_price: &entry,
    }];
    let short_estimate =
        estimate_cross_margin_conditional_price(8, &mark, &buffer, 2, &short).unwrap();
    assert_eq!(short_estimate.status, CrossMarginEstimateStatus::Estimated);
    assert_eq!(
        short_estimate.price,
        Some(decimal("110.000000000000000000"))
    );
    let at_displayed_short = buffer.clone()
        + short_estimate.net_quantity.clone()
            * (short_estimate.price.clone().unwrap() - mark.clone());
    let one_adverse_short =
        buffer + short_estimate.net_quantity.clone() * (decimal("110.01") - mark.clone());
    assert!(at_displayed_short > 0);
    assert!(one_adverse_short < 0);
}

#[test]
fn cross_margin_conditional_price_names_zero_near_zero_and_triggered_states() {
    assert_eq!(CROSS_NET_DELTA_EPSILON, "0.000001");
    let mark = decimal("100");
    let entry = decimal("1");
    let exact_long = decimal("0.5");
    let exact_short = decimal("0.5");
    let exact_hedge = [
        CrossMarginReferencePosition {
            pair_id: 9,
            direction: "long",
            notional_amount: &exact_long,
            entry_price: &entry,
        },
        CrossMarginReferencePosition {
            pair_id: 9,
            direction: "short",
            notional_amount: &exact_short,
            entry_price: &entry,
        },
    ];
    let zero_delta =
        estimate_cross_margin_conditional_price(9, &mark, &decimal("10"), 8, &exact_hedge).unwrap();
    assert_eq!(zero_delta.status, CrossMarginEstimateStatus::NetDeltaZero);
    assert_eq!(zero_delta.price, None);
    assert_eq!(zero_delta.distance_rate, None);

    let triggered =
        estimate_cross_margin_conditional_price(9, &mark, &decimal("0"), 8, &exact_hedge).unwrap();
    assert_eq!(
        triggered.status,
        CrossMarginEstimateStatus::AlreadyLiquidatable
    );
    assert_eq!(triggered.price, None);

    let near_long = decimal("0.5000005");
    let near_short = decimal("0.4999995");
    let epsilon_boundary = [
        CrossMarginReferencePosition {
            pair_id: 9,
            direction: "long",
            notional_amount: &near_long,
            entry_price: &entry,
        },
        CrossMarginReferencePosition {
            pair_id: 9,
            direction: "short",
            notional_amount: &near_short,
            entry_price: &entry,
        },
    ];
    let near_zero =
        estimate_cross_margin_conditional_price(9, &mark, &decimal("10"), 8, &epsilon_boundary)
            .unwrap();
    assert_eq!(
        near_zero.status,
        CrossMarginEstimateStatus::NetDeltaNearZero
    );
    assert_eq!(near_zero.net_quantity, decimal("0.000001000000000000"));
    assert_eq!(near_zero.gross_quantity, decimal("1.000000000000000000"));
    assert_eq!(near_zero.price, None);
}

#[test]
fn cross_margin_conditional_price_rejects_non_positive_and_rounded_wrong_direction() {
    let mark = decimal("100");
    let entry = decimal("100");
    let notional = decimal("100");
    let positions = [CrossMarginReferencePosition {
        pair_id: 11,
        direction: "long",
        notional_amount: &notional,
        entry_price: &entry,
    }];

    let invalid_exposure =
        estimate_cross_margin_conditional_price(12, &mark, &decimal("10"), 2, &positions).unwrap();
    assert_eq!(
        invalid_exposure.status,
        CrossMarginEstimateStatus::InvalidExposure
    );
    assert_eq!(invalid_exposure.price, None);

    let no_positive =
        estimate_cross_margin_conditional_price(11, &mark, &decimal("110"), 2, &positions).unwrap();
    assert_eq!(
        no_positive.status,
        CrossMarginEstimateStatus::NoPositiveBoundary
    );
    assert_eq!(no_positive.price, None);

    let rounded_to_current =
        estimate_cross_margin_conditional_price(11, &mark, &decimal("0.001"), 2, &positions)
            .unwrap();
    assert_eq!(
        rounded_to_current.status,
        CrossMarginEstimateStatus::WrongAdverseDirection
    );
    assert_eq!(rounded_to_current.price, None);
}

#[test]
fn cross_margin_liquidation_policy_zeros_available_and_uses_negative_equity_for_bad_debt() {
    let already_zero = cross_margin_liquidation_settlement(&decimal("0"), &decimal("0")).unwrap();
    assert_eq!(already_zero.wallet_delta, decimal("0.000000000000000000"));
    assert_eq!(already_zero.bad_debt, decimal("0.000000000000000000"));

    let positive_equity =
        cross_margin_liquidation_settlement(&decimal("3"), &decimal("9")).unwrap();
    assert_eq!(
        positive_equity.available_after,
        decimal("0.000000000000000000")
    );
    assert_eq!(
        positive_equity.wallet_delta,
        decimal("-3.000000000000000000")
    );
    assert_eq!(positive_equity.bad_debt, decimal("0.000000000000000000"));

    let negative_equity =
        cross_margin_liquidation_settlement(&decimal("40"), &decimal("-7")).unwrap();
    assert_eq!(
        negative_equity.available_after,
        decimal("0.000000000000000000")
    );
    assert_eq!(
        negative_equity.wallet_delta,
        decimal("-40.000000000000000000")
    );
    assert_eq!(negative_equity.bad_debt, decimal("7.000000000000000000"));
    assert!(cross_margin_liquidation_settlement(&decimal("-1"), &decimal("0")).is_err());
}
