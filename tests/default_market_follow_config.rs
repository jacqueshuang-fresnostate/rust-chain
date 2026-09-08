//! 跟随配置的纯参数合同；不读取数据库或外部行情，不改旧独立生成器的输出。

use chrono::{TimeZone, Utc};
use exchange_api::modules::market::synthetic_default::{
    DefaultMarketMode, DefaultMarketParameters, generate_default_1m,
};
use serde_json::{Value, json};

fn configuration(value: Value) -> DefaultMarketParameters {
    serde_json::from_value(value).unwrap()
}

#[test]
fn legacy_config_defaults_to_independent_and_keeps_identical_generation() {
    let legacy = configuration(json!({}));
    assert_eq!(legacy.mode, DefaultMarketMode::Independent);
    assert!(legacy.follow.is_none());
    let explicit = configuration(json!({"mode":"independent","follow":null}));
    assert_eq!(legacy, explicit);
    let at = Utc.with_ymd_and_hms(2026, 9, 7, 10, 0, 0).unwrap();
    let old = generate_default_1m(
        "DFTUSDT",
        "legacy-seed",
        1,
        6,
        2,
        at,
        &1.into(),
        &1.into(),
        &legacy,
    )
    .unwrap();
    let current = generate_default_1m(
        "DFTUSDT",
        "legacy-seed",
        1,
        6,
        2,
        at,
        &1.into(),
        &1.into(),
        &explicit,
    )
    .unwrap();
    assert_eq!(old, current);
}

#[test]
fn follow_minimal_json_has_explicit_conservative_defaults() {
    let parameters = configuration(json!({"mode":"follow","follow":{"reference_pair_id":42}}));
    parameters.validate(6, 2).unwrap();
    let follow = parameters.follow.unwrap();
    assert_eq!(follow.reference_pair_id, 42);
    assert_eq!(follow.multiplier, bigdecimal::BigDecimal::from(1));
    assert_eq!(
        follow.max_move_ratio,
        bigdecimal::BigDecimal::new(5.into(), 2)
    );
    assert_eq!(follow.stale_after_seconds, 60);
}

#[test]
fn follow_configuration_rejects_mode_mismatch_and_invalid_ranges() {
    for value in [
        json!({"mode":"follow"}),
        json!({"follow":{"reference_pair_id":1}}),
        json!({"mode":"follow","follow":{"reference_pair_id":0}}),
    ] {
        assert!(
            configuration(value.clone()).validate(6, 2).is_err(),
            "{value}"
        );
    }
    for (field, value) in [
        ("multiplier", json!("0")),
        ("multiplier", json!("-1")),
        ("multiplier", json!("3.00001")),
        ("multiplier", json!("0.0000000000000000001")),
        ("max_move_ratio", json!("0")),
        ("max_move_ratio", json!("1.00001")),
        ("max_move_ratio", json!("0.0000000000000000001")),
        ("stale_after_seconds", json!(14)),
        ("stale_after_seconds", json!(301)),
    ] {
        let mut input = json!({"mode":"follow","follow":{"reference_pair_id":42}});
        input["follow"][field] = value;
        assert!(
            configuration(input.clone()).validate(6, 2).is_err(),
            "{input}"
        );
    }
    for seconds in [15, 300] {
        configuration(json!({"mode":"follow","follow":{"reference_pair_id":42,"multiplier":"3","max_move_ratio":"1","stale_after_seconds":seconds}})).validate(6,2).unwrap();
    }
}

#[test]
fn unknown_modes_fields_or_missing_reference_are_not_silently_defaulted() {
    for value in [
        json!({"mode":"inverse"}),
        json!({"mode":"follow","follow":{}}),
        json!({"mode":"follow","follow":{"reference_pair_id":42,"unknown":1}}),
    ] {
        assert!(
            serde_json::from_value::<DefaultMarketParameters>(value.clone()).is_err(),
            "{value}"
        );
    }
}
