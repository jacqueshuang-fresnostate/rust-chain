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

#[test]
fn admin_recharge_fingerprint_normalizes_decimal_and_reason() {
    let first = admin_recharge_request_fingerprint(3, 7, 11, &decimal("25.5000"), " support ");
    let equivalent = admin_recharge_request_fingerprint(3, 7, 11, &decimal("25.5"), "support");
    let changed = admin_recharge_request_fingerprint(3, 8, 11, &decimal("25.5"), "support");

    assert_eq!(first, equivalent);
    assert_ne!(first, changed);
    assert_eq!(first.len(), 64);
    assert!(normalize_admin_recharge_idempotency_key("   ").is_err());
    assert!(normalize_admin_recharge_idempotency_key(&"x".repeat(129)).is_err());
}

#[test]
fn new_coin_unlock_fee_rate_must_fit_persisted_precision() {
    let request = UpdateNewCoinUnlockFeeRuleRequest {
        expected_config: None,
        unlock_fee_enabled: true,
        unlock_fee_rate: Some(decimal("0.123456789")),
        unlock_fee_basis: Some("market_value".to_owned()),
        unlock_fee_asset: Some(1),
        reason: Some("precision regression".to_owned()),
    };

    let error = validate_update_new_coin_unlock_fee_rule(&request)
        .expect_err("DECIMAL(18,8) fee rates must not be silently rounded");
    assert!(format!("{error:?}").contains("precision_scale 8"));
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

fn config_change_record(
    status: &str,
) -> crate::modules::admin::repository::AdminConfigChangeRecord {
    let now = Utc::now();
    crate::modules::admin::repository::AdminConfigChangeRecord {
        id: 7,
        request_no: "ACR-test".to_owned(),
        config_domain: "prediction".to_owned(),
        target_type: "settings".to_owned(),
        target_id: "default".to_owned(),
        action: "update".to_owned(),
        base_revision: Some(3),
        before_json: Some(serde_json::json!({"enabled": false})),
        proposed_json: serde_json::json!({"enabled": true}),
        reason: "调整预测配置".to_owned(),
        risk_level: "high".to_owned(),
        status: status.to_owned(),
        created_by: 11,
        reviewed_by: None,
        review_reason: None,
        applied_by: None,
        reviewed_at: None,
        applied_at: None,
        created_at: now,
        updated_at: now,
    }
}

#[test]
fn config_change_snapshot_redacts_nested_credentials() {
    let sanitized = sanitize_admin_config_snapshot(serde_json::json!({
        "host": "smtp.example.test",
        "password": "plain-password",
        "nested": {"api_key": "plain-key", "key_prefix": "public-prefix"},
        "items": [{"refresh_token": "plain-token"}]
    }));

    assert_eq!(sanitized["host"], "smtp.example.test");
    assert_eq!(sanitized["password"], "***REDACTED***");
    assert_eq!(sanitized["nested"]["api_key"], "***REDACTED***");
    assert_eq!(sanitized["nested"]["key_prefix"], "public-prefix");
    assert_eq!(sanitized["items"][0]["refresh_token"], "***REDACTED***");
}

#[test]
fn config_change_review_rejects_self_review_and_replays_same_result() {
    let pending = config_change_record("pending");
    assert!(matches!(
        validate_config_review_transition(
            &pending,
            11,
            AdminConfigReviewDecision::Approve,
            "复核通过"
        ),
        Err(AppError::Forbidden)
    ));

    let mut approved = config_change_record("approved");
    approved.reviewed_by = Some(22);
    approved.review_reason = Some("复核通过".to_owned());
    assert_eq!(
        validate_config_review_transition(
            &approved,
            22,
            AdminConfigReviewDecision::Approve,
            "复核通过"
        )
        .unwrap(),
        AdminConfigTransition::Replay
    );
    assert!(matches!(
        validate_config_review_transition(
            &approved,
            23,
            AdminConfigReviewDecision::Reject,
            "改为拒绝"
        ),
        Err(AppError::Conflict(_))
    ));
}

#[test]
fn config_change_apply_transition_is_idempotent() {
    assert_eq!(
        validate_config_apply_transition(&config_change_record("approved")).unwrap(),
        AdminConfigTransition::Apply
    );
    assert_eq!(
        validate_config_apply_transition(&config_change_record("applied")).unwrap(),
        AdminConfigTransition::Replay
    );
    assert!(matches!(
        validate_config_apply_transition(&config_change_record("rejected")),
        Err(AppError::Conflict(_))
    ));
}

#[test]
fn risk_rule_targets_are_normalized_and_reject_unusable_shapes() {
    let mut request = CreateRiskRuleRequest {
        rule_type: " withdraw_limit ".to_owned(),
        target_type: " ASSET ".to_owned(),
        target_id: Some(" usdt ".to_owned()),
        config_json: serde_json::json!({"max_amount": "100"}),
        enabled: Some(true),
        reason: Some("配置限额".to_owned()),
    };
    assert_eq!(
        validate_create_risk_rule(&request).unwrap(),
        ValidatedRiskRuleTarget {
            target_type: "asset".to_owned(),
            target_id: Some("USDT".to_owned()),
        }
    );

    request.target_type = "global".to_owned();
    assert!(matches!(
        validate_create_risk_rule(&request),
        Err(AppError::Validation(message)) if message == "global risk target must not include target_id"
    ));

    request.target_type = "user".to_owned();
    request.target_id = Some("disabled-user".to_owned());
    assert!(matches!(
        validate_create_risk_rule(&request),
        Err(AppError::Validation(message)) if message == "user and pair risk targets require a positive target_id"
    ));
}

#[test]
fn admin_permission_mapping_is_fail_closed_and_action_aware() {
    assert_eq!(
        required_admin_permission("GET", "/admin/api/v1/config-change-requests").as_deref(),
        Some("governance.changes.read")
    );
    assert_eq!(
        required_admin_permission("POST", "/admin/api/v1/config-change-requests/7/review")
            .as_deref(),
        Some("governance.changes.review")
    );
    assert_eq!(
        required_admin_permission("POST", "/admin/api/v1/config-change-requests/7/apply")
            .as_deref(),
        Some("governance.changes.operate")
    );
    assert_eq!(
        required_admin_permission("GET", "/admin/api/v1/new-unmapped-route").as_deref(),
        Some("admin.unmapped.read")
    );
    assert_eq!(
        required_admin_permission("GET", "/admin/api/v1/support/conversations").as_deref(),
        Some("support.conversations.read")
    );
    assert_eq!(
        required_admin_permission("POST", "/admin/api/v1/support/conversations/7/messages")
            .as_deref(),
        Some("support.conversations.write")
    );
}
