use super::*;
use crate::modules::admin::presentation::MarketStrategyGeneratorRequest;
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

fn generator_request(seed_mode: &str, seed: Option<&str>) -> MarketStrategyGeneratorRequest {
    MarketStrategyGeneratorRequest {
        scenario: "trend_up".to_owned(),
        seed_mode: seed_mode.to_owned(),
        seed: seed.map(str::to_owned),
        regenerate_seed: false,
        mean_reversion_strength: decimal("0.55"),
        noise_scale: decimal("1"),
        wick_scale: decimal("0.75"),
        volume_shape: "trend".to_owned(),
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

#[test]
fn market_strategy_generator_validates_fixed_seed_and_numeric_bounds() {
    let fixed = validate_market_strategy_generator(&generator_request(
        "fixed",
        Some(" deterministic-seed "),
    ))
    .unwrap();
    assert_eq!(fixed.requested_seed.as_deref(), Some("deterministic-seed"));
    assert_eq!(
        resolve_new_market_strategy_seed(&fixed),
        "deterministic-seed"
    );

    let missing = validate_market_strategy_generator(&generator_request("fixed", None));
    assert!(matches!(missing, Err(AppError::Validation(message)) if message.contains("必须填写")));

    let mut invalid_noise = generator_request("auto", None);
    invalid_noise.noise_scale = decimal("5.0001");
    assert!(matches!(
        validate_market_strategy_generator(&invalid_noise),
        Err(AppError::Validation(message)) if message == "噪声强度必须在 0～5 之间"
    ));
}

#[test]
fn market_strategy_auto_seed_inherits_unless_regeneration_is_explicit() {
    let inherited = validate_market_strategy_generator(&generator_request("auto", None)).unwrap();
    assert_eq!(
        resolve_updated_market_strategy_seed(&inherited, "existing-seed").unwrap(),
        "existing-seed"
    );

    let mut regenerated_request = generator_request("auto", None);
    regenerated_request.regenerate_seed = true;
    let regenerated = validate_market_strategy_generator(&regenerated_request).unwrap();
    let new_seed = resolve_updated_market_strategy_seed(&regenerated, "existing-seed").unwrap();
    assert!(!new_seed.is_empty());
    assert_ne!(new_seed, "existing-seed");
}

#[test]
fn market_strategy_presets_expose_all_stable_scenarios_and_explicit_parameters() {
    let presets = market_strategy_presets().presets;
    assert_eq!(presets.len(), 7);
    assert_eq!(presets[0].code, "custom_path");
    assert_eq!(presets[1].code, "trend_up");
    assert_eq!(presets[5].code, "crash_recovery");
    assert_eq!(presets[6].code, "pump_then_dump");
    for preset in presets {
        assert!(!preset.name.trim().is_empty());
        assert!(!preset.description.trim().is_empty());
        assert_eq!(preset.generator.scenario, preset.code);
        assert_eq!(preset.generator.seed_mode, "auto");
    }
}
