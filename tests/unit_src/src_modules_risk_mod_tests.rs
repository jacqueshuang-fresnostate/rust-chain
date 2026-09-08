use crate::modules::risk::{
    RiskDecision, RiskReject, RiskRequest, RiskRules, RiskScope, StoredRiskRule, evaluate_risk,
    resolve_risk_policy,
};
use bigdecimal::BigDecimal;
use serde_json::json;

const SPOT_ORDER: &str = "spot.order.create";
const WITHDRAWAL: &str = "wallet.withdrawal.create";

fn amount(value: i64) -> BigDecimal {
    BigDecimal::from(value)
}

fn rules() -> RiskRules {
    RiskRules {
        max_requests: Some(3),
        max_amount: Some(amount(1_000)),
        max_price_deviation_bps: Some(500),
        blocked_operations: Some(vec!["market_strategy.stop".to_owned()]),
    }
}

fn request(operation: &str, request_amount: i64, price: i64) -> RiskRequest {
    RiskRequest {
        operation: operation.to_owned(),
        request_count: Some(1),
        amount: Some(amount(request_amount)),
        price: Some(amount(price)),
        reference_price: Some(amount(100)),
    }
}

#[test]
fn risk_guard_approves_request_within_limits() {
    let decision = evaluate_risk(&request(SPOT_ORDER, 100, 104), &rules());

    assert_eq!(decision, RiskDecision::Approved);
    assert!(decision.is_approved());
}

#[test]
fn risk_guard_rejects_rate_amount_price_and_blocked_operation() {
    let rules = rules();
    let mut rate_limited = request(SPOT_ORDER, 100, 100);
    rate_limited.request_count = Some(4);

    assert_eq!(
        evaluate_risk(&rate_limited, &rules),
        RiskDecision::Rejected(RiskReject::RateLimit)
    );
    assert_eq!(
        evaluate_risk(&request(SPOT_ORDER, 1_001, 100), &rules),
        RiskDecision::Rejected(RiskReject::AmountLimit)
    );
    assert_eq!(
        evaluate_risk(&request(SPOT_ORDER, 100, 106), &rules),
        RiskDecision::Rejected(RiskReject::PriceDeviation)
    );
    assert_eq!(
        evaluate_risk(&request("market_strategy.stop", 100, 100), &rules),
        RiskDecision::Rejected(RiskReject::OperationNotAllowed)
    );
}

#[test]
fn risk_guard_skips_checks_without_rules_or_facts() {
    let mut unsourced = request(WITHDRAWAL, 1_001, 106);
    unsourced.request_count = None;
    unsourced.price = None;
    unsourced.reference_price = None;

    assert_eq!(
        evaluate_risk(&unsourced, &RiskRules::default()),
        RiskDecision::Approved
    );
    assert_eq!(
        evaluate_risk(
            &unsourced,
            &RiskRules {
                max_price_deviation_bps: Some(1),
                ..RiskRules::default()
            }
        ),
        RiskDecision::Approved
    );
}

#[test]
fn risk_policy_resolves_scoped_rules_with_strictest_limits() {
    let stored = vec![
        StoredRiskRule {
            target_type: "global".to_owned(),
            target_id: None,
            config: json!({
                "operations": [SPOT_ORDER],
                "max_amount": "500",
                "max_requests": 10,
                "window_seconds": 30
            }),
        },
        StoredRiskRule {
            target_type: "pair".to_owned(),
            target_id: Some("BTC-USDT".to_owned()),
            config: json!({
                "operations": [SPOT_ORDER],
                "max_amount": 200,
                "max_price_deviation_bps": 100
            }),
        },
        StoredRiskRule {
            target_type: "pair".to_owned(),
            target_id: Some("ETH-USDT".to_owned()),
            config: json!({ "operations": [SPOT_ORDER], "max_amount": "1" }),
        },
    ];

    let policy = resolve_risk_policy(&stored, SPOT_ORDER, &[RiskScope::new("pair", "btc-usdt")]);

    assert_eq!(policy.rules.max_amount, Some(amount(200)));
    assert_eq!(policy.rules.max_price_deviation_bps, Some(100));
    assert_eq!(policy.rules.max_requests, Some(10));
    assert_eq!(policy.rate_limit_window_seconds, 30);
    assert_eq!(policy.rules.blocked_operations, None);
}

#[test]
fn risk_policy_is_unrestricted_without_configured_rules() {
    assert!(
        resolve_risk_policy(&[], SPOT_ORDER, &[])
            .rules
            .is_unrestricted()
    );
    assert!(
        resolve_risk_policy(
            &[StoredRiskRule {
                target_type: "user".to_owned(),
                target_id: Some("7".to_owned()),
                config: json!({ "note": "未识别的配置键" }),
            }],
            SPOT_ORDER,
            &[RiskScope::new("user", "7")]
        )
        .rules
        .is_unrestricted()
    );
}

/// 两条各自只声明自身操作的规则不得叠加成全面拒绝：交易所不能被配置组合停摆。
#[test]
fn risk_policy_never_combines_into_deny_all() {
    let stored = vec![
        StoredRiskRule {
            target_type: "global".to_owned(),
            target_id: None,
            config: json!({ "operations": [SPOT_ORDER], "max_amount": "500" }),
        },
        StoredRiskRule {
            target_type: "global".to_owned(),
            target_id: None,
            config: json!({ "operations": [WITHDRAWAL], "max_amount": "10" }),
        },
    ];

    let spot = resolve_risk_policy(&stored, SPOT_ORDER, &[]);
    let withdrawal = resolve_risk_policy(&stored, WITHDRAWAL, &[]);

    assert_eq!(spot.rules.max_amount, Some(amount(500)));
    assert_eq!(withdrawal.rules.max_amount, Some(amount(10)));
    assert_eq!(
        evaluate_risk(&request(SPOT_ORDER, 100, 100), &spot.rules),
        RiskDecision::Approved
    );
    assert_eq!(
        evaluate_risk(&request(WITHDRAWAL, 1, 100), &withdrawal.rules),
        RiskDecision::Approved
    );
}

/// 金额限额按操作口径生效：限现货名义额的规则不能顺带拦截提现数量。
#[test]
fn risk_policy_amount_limit_only_applies_to_matching_operation_unit() {
    let spot_only = vec![StoredRiskRule {
        target_type: "global".to_owned(),
        target_id: None,
        config: json!({ "operations": [SPOT_ORDER], "max_amount": "1000" }),
    }];

    assert_eq!(
        resolve_risk_policy(&spot_only, SPOT_ORDER, &[])
            .rules
            .max_amount,
        Some(amount(1_000))
    );
    assert_eq!(
        resolve_risk_policy(&spot_only, WITHDRAWAL, &[])
            .rules
            .max_amount,
        None
    );

    // 未声明操作口径的规则不套用金额限额，避免按错误单位拦截。
    let unscoped = vec![StoredRiskRule {
        target_type: "global".to_owned(),
        target_id: None,
        config: json!({ "max_amount": "1000" }),
    }];
    assert_eq!(
        resolve_risk_policy(&unscoped, SPOT_ORDER, &[])
            .rules
            .max_amount,
        None
    );
}

/// 限频取最严候选且与规则书写顺序无关，窗口不会随加载顺序漂移。
#[test]
fn risk_policy_rate_limit_precedence_is_order_independent() {
    let lenient = StoredRiskRule {
        target_type: "global".to_owned(),
        target_id: None,
        config: json!({ "max_requests": 10, "window_seconds": 3600 }),
    };
    let strict = StoredRiskRule {
        target_type: "pair".to_owned(),
        target_id: Some("BTC-USDT".to_owned()),
        config: json!({ "max_requests": 5, "window_seconds": 120 }),
    };
    let scopes = [RiskScope::new("pair", "BTC-USDT")];

    let forward = resolve_risk_policy(&[lenient.clone(), strict.clone()], SPOT_ORDER, &scopes);
    let reversed = resolve_risk_policy(&[strict, lenient], SPOT_ORDER, &scopes);

    assert_eq!(forward.rules.max_requests, Some(5));
    assert_eq!(forward.rate_limit_window_seconds, 120);
    assert_eq!(forward.rules.max_requests, reversed.rules.max_requests);
    assert_eq!(
        forward.rate_limit_window_seconds,
        reversed.rate_limit_window_seconds
    );
    // 计数键随生效规则的作用域隔离，不同作用域不共用配额。
    assert_eq!(forward.rate_limit_scope, reversed.rate_limit_scope);
    assert_ne!(
        forward.rate_limit_scope,
        resolve_risk_policy(&[], SPOT_ORDER, &[]).rate_limit_scope
    );
}

#[test]
fn risk_policy_numeric_pair_alias_merges_once_without_rewriting_legacy_counter_scope() {
    let legacy = StoredRiskRule {
        target_type: "pair".to_owned(),
        target_id: Some("BTC-USDT".to_owned()),
        config: json!({"operations": [SPOT_ORDER], "max_amount": "50", "max_requests": 2, "window_seconds": 120}),
    };
    let numeric = StoredRiskRule {
        target_type: "pair".to_owned(),
        target_id: Some("42".to_owned()),
        config: json!({"operations": [SPOT_ORDER], "max_amount": "10", "max_requests": 3, "blocked_operations": [SPOT_ORDER]}),
    };
    let scopes = [
        RiskScope::new("user", "7"),
        RiskScope::new("pair", "BTC-USDT"),
        RiskScope::new("pair", "42"),
    ];
    let old_policy = resolve_risk_policy(std::slice::from_ref(&legacy), SPOT_ORDER, &scopes[..2]);
    let aliased_policy = resolve_risk_policy(std::slice::from_ref(&legacy), SPOT_ORDER, &scopes);
    assert_eq!(old_policy.rules, aliased_policy.rules);
    assert_eq!(old_policy.rate_limit_scope, "pair:btc-usdt");
    assert_eq!(aliased_policy.rate_limit_scope, old_policy.rate_limit_scope);
    let combined = resolve_risk_policy(&[legacy.clone(), numeric.clone()], SPOT_ORDER, &scopes);
    assert_eq!(combined.rules.max_amount, Some(amount(10)));
    assert_eq!(combined.rules.max_requests, Some(2));
    assert_eq!(combined.rate_limit_scope, "pair:btc-usdt");
    assert_eq!(combined.rate_limit_window_seconds, 120);
    assert_eq!(
        combined.rules.blocked_operations,
        Some(vec![SPOT_ORDER.to_owned()])
    );
    let reversed = resolve_risk_policy(&[numeric.clone(), legacy], SPOT_ORDER, &scopes);
    assert_eq!(combined.rules, reversed.rules);
    assert_eq!(combined.rate_limit_scope, reversed.rate_limit_scope);
    assert!(
        resolve_risk_policy(
            std::slice::from_ref(&numeric),
            SPOT_ORDER,
            &[
                RiskScope::new("pair", "ETH-USDT"),
                RiskScope::new("pair", "43")
            ]
        )
        .rules
        .is_unrestricted()
    );
    assert_eq!(
        resolve_risk_policy(&[numeric], SPOT_ORDER, &scopes).rate_limit_scope,
        "pair:42"
    );
}

#[test]
fn risk_policy_keeps_numeric_symbol_legacy_namespace_unchanged() {
    // 既有模型不区分数字符号和数字主键；添加权威主键别名不得静默重写旧规则。
    let stored = StoredRiskRule {
        target_type: "pair".to_owned(),
        target_id: Some("42".to_owned()),
        config: json!({"max_requests": 0}),
    };
    let policy = resolve_risk_policy(
        &[stored],
        SPOT_ORDER,
        &[RiskScope::new("pair", "42"), RiskScope::new("pair", "99")],
    );
    assert_eq!(policy.rules.max_requests, Some(0));
    assert_eq!(policy.rate_limit_scope, "pair:42");
    assert_eq!(policy.rate_limit_window_seconds, 60);
}

#[test]
fn risk_config_validated_values_keep_runtime_policy_and_original_json() {
    use crate::modules::risk::service::validate_risk_rule_config;
    use std::str::FromStr;
    let config = json!({
        "operations": ["SPOT.ORDER.CREATE"],
        "blocked_operations": ["future.operation", "future.operation"],
        "max_amount": " 1.234567890123456789012345678901e20 ",
        "max_price_deviation_bps": " 0 ",
        "max_requests": "+0", "window_seconds": " 1 ",
        "extension": {"future": null}
    });
    let original = config.clone();
    assert_eq!(validate_risk_rule_config(&config), Ok(()));
    assert_eq!(config, original);
    let policy = resolve_risk_policy(
        &[StoredRiskRule {
            target_type: "global".to_owned(),
            target_id: None,
            config,
        }],
        SPOT_ORDER,
        &[],
    );
    assert_eq!(
        policy.rules.max_amount,
        Some(BigDecimal::from_str("1.234567890123456789012345678901e20").unwrap())
    );
    assert_eq!(policy.rules.max_price_deviation_bps, Some(0));
    assert_eq!(policy.rules.max_requests, Some(0));
    assert_eq!(policy.rate_limit_window_seconds, 1);
    assert_eq!(
        policy.rules.blocked_operations,
        Some(vec!["future.operation".to_owned()])
    );
    for config in [
        json!({}),
        json!({"operations": [], "max_amount": "10"}),
        json!({"operations": [SPOT_ORDER, WITHDRAWAL], "max_amount": "10"}),
        json!({"operations": ["future.operation"], "max_amount": "10"}),
    ] {
        assert_eq!(validate_risk_rule_config(&config), Ok(()));
        assert_eq!(
            resolve_risk_policy(
                &[StoredRiskRule {
                    target_type: "global".to_owned(),
                    target_id: None,
                    config
                }],
                SPOT_ORDER,
                &[]
            )
            .rules
            .max_amount,
            None
        );
    }
    // 写入校验不被移到历史读取路径：旧非法字段仍按既有运行时规则处理。
    let legacy = resolve_risk_policy(
        &[StoredRiskRule {
            target_type: "global".to_owned(),
            target_id: None,
            config: json!({"operations": null, "max_requests": 2, "window_seconds": 0, "max_price_deviation_bps": -1}),
        }],
        SPOT_ORDER,
        &[],
    );
    assert_eq!(legacy.rules.max_requests, Some(2));
    assert_eq!(legacy.rate_limit_window_seconds, 60);
    assert_eq!(legacy.rules.max_price_deviation_bps, None);
}
