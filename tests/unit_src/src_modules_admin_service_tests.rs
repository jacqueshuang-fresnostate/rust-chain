use super::*;
use chrono::{TimeDelta, TimeZone};
use std::str::FromStr;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

fn strategy_node(
    target_time: DateTime<Utc>,
    target_type: &str,
    target_value: &str,
) -> MarketStrategyNodeRequest {
    MarketStrategyNodeRequest {
        target_time,
        target_type: target_type.to_owned(),
        target_value: decimal(target_value),
        execution_mode: "hard".to_owned(),
        tolerance: decimal("0"),
        volatility: decimal("0.01"),
        volume_min: None,
        volume_max: None,
    }
}

fn validation_message(result: AppResult<()>) -> String {
    match result.expect_err("strategy node validation should fail") {
        AppError::Validation(message) => message,
        error => panic!("expected validation error, got {error:?}"),
    }
}

#[test]
fn market_strategy_nodes_resolve_all_target_types_from_start_price() {
    let start_time = Utc.with_ymd_and_hms(2026, 8, 12, 10, 0, 0).unwrap();
    let nodes = vec![
        strategy_node(start_time + TimeDelta::minutes(10), "absolute_price", "120"),
        strategy_node(
            start_time + TimeDelta::minutes(20),
            "percent_from_start",
            "-50",
        ),
        strategy_node(
            start_time + TimeDelta::minutes(30),
            "percent_from_previous",
            "100",
        ),
    ];

    validate_market_strategy_nodes(
        &nodes,
        &decimal("100"),
        start_time,
        start_time + TimeDelta::hours(1),
    )
    .unwrap();
}

#[test]
fn market_strategy_nodes_reject_non_positive_percent_from_start_price() {
    let start_time = Utc.with_ymd_and_hms(2026, 8, 12, 10, 0, 0).unwrap();
    let nodes = vec![strategy_node(
        start_time + TimeDelta::minutes(10),
        "percent_from_start",
        "-100",
    )];

    assert_eq!(
        validation_message(validate_market_strategy_nodes(
            &nodes,
            &decimal("100"),
            start_time,
            start_time + TimeDelta::hours(1),
        )),
        "market strategy node 1 resolves to a non-positive price"
    );
}

#[test]
fn market_strategy_nodes_reject_chained_non_positive_previous_price() {
    let start_time = Utc.with_ymd_and_hms(2026, 8, 12, 10, 0, 0).unwrap();
    let nodes = vec![
        strategy_node(
            start_time + TimeDelta::minutes(10),
            "percent_from_start",
            "-50",
        ),
        strategy_node(
            start_time + TimeDelta::minutes(20),
            "percent_from_previous",
            "-100",
        ),
    ];

    assert_eq!(
        validation_message(validate_market_strategy_nodes(
            &nodes,
            &decimal("100"),
            start_time,
            start_time + TimeDelta::hours(1),
        )),
        "market strategy node 2 resolves to a non-positive price"
    );
}
