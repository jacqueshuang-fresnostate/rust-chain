use super::*;
use crate::modules::admin::{
    repository::AdminConfigCenterFactRecord,
    service::{admin_permission_catalog, required_admin_permission},
};

fn fact(code: &str) -> AdminConfigCenterFactRecord {
    AdminConfigCenterFactRecord {
        code: code.to_owned(),
        configured_count: 1,
        pending_apply_count: 0,
        published_version: None,
        applied_version: None,
        runtime_status: "not_applicable".to_owned(),
        last_modified_at: None,
        last_applied_at: None,
        last_tested_at: None,
        recent_error: None,
    }
}

fn complete_facts() -> Vec<AdminConfigCenterFactRecord> {
    ADMIN_CONFIG_CENTER_DEFINITIONS
        .iter()
        .map(|definition| fact(definition.code))
        .collect()
}

#[test]
fn config_center_catalog_covers_all_required_domains_and_paths() {
    let actual = ADMIN_CONFIG_CENTER_DEFINITIONS
        .iter()
        .map(|definition| definition.code)
        .collect::<BTreeSet<_>>();
    let expected = BTreeSet::from([
        "prediction_settings",
        "market_feed",
        "market_strategy",
        "kyc_rules",
        "loan_products",
        "margin_products",
        "seconds_contract_products",
        "earn_products",
        "smtp",
        "upload_storage",
        "platform_brand",
        "security_policy",
        "country_configs",
    ]);

    assert_eq!(actual, expected);
    assert_eq!(ADMIN_CONFIG_CENTER_DEFINITIONS.len(), 13);
    assert!(ADMIN_CONFIG_CENTER_DEFINITIONS.iter().all(|definition| {
        !definition.name.trim().is_empty()
            && !definition.group_name.trim().is_empty()
            && definition.config_path.starts_with("/admin/")
            && definition
                .operation_path
                .is_none_or(|path| path.starts_with("/admin/"))
    }));
    let kyc = ADMIN_CONFIG_CENTER_DEFINITIONS
        .iter()
        .find(|definition| definition.code == "kyc_rules")
        .unwrap();
    assert_eq!(kyc.config_path, "/admin/users/kyc/settings");
    assert_eq!(kyc.operation_path, Some("/admin/users/kyc/reviews"));
    let prediction = ADMIN_CONFIG_CENTER_DEFINITIONS
        .iter()
        .find(|definition| definition.code == "prediction_settings")
        .unwrap();
    assert_eq!(prediction.operation_path, Some("/admin/prediction/sync"));
}

#[test]
fn config_center_status_rule_has_explicit_fail_safe_priority() {
    let mut facts = complete_facts();

    let unconfigured = facts
        .iter_mut()
        .find(|fact| fact.code == "prediction_settings")
        .unwrap();
    unconfigured.configured_count = 0;
    unconfigured.pending_apply_count = 1;
    unconfigured.runtime_status = "error".to_owned();

    let runtime_error = facts
        .iter_mut()
        .find(|fact| fact.code == "market_feed")
        .unwrap();
    runtime_error.pending_apply_count = 1;
    runtime_error.runtime_status = "error".to_owned();
    runtime_error.recent_error = Some("provider timeout".to_owned());

    let pending = facts
        .iter_mut()
        .find(|fact| fact.code == "market_strategy")
        .unwrap();
    pending.pending_apply_count = 1;
    pending.runtime_status = "healthy".to_owned();

    let view = build_admin_config_center_view(facts, AdminConfigCenterFilter::default()).unwrap();
    let status = |code: &str| {
        view.items
            .iter()
            .find(|item| item.code == code)
            .unwrap()
            .config_status
    };

    assert_eq!(
        status("prediction_settings"),
        AdminConfigCenterStatus::Unconfigured
    );
    assert_eq!(status("market_feed"), AdminConfigCenterStatus::RuntimeError);
    assert_eq!(
        status("market_strategy"),
        AdminConfigCenterStatus::PendingApply
    );
    assert_eq!(status("kyc_rules"), AdminConfigCenterStatus::Normal);
}

#[test]
fn config_center_filters_search_group_and_status_with_stable_summary() {
    let mut facts = complete_facts();
    facts
        .iter_mut()
        .find(|fact| fact.code == "loan_products")
        .unwrap()
        .configured_count = 0;

    let filter = AdminConfigCenterFilter::new(
        Some(" 产品 ".to_owned()),
        Some(" PRODUCTS ".to_owned()),
        Some(" unconfigured ".to_owned()),
    )
    .unwrap();
    let view = build_admin_config_center_view(facts, filter).unwrap();

    assert_eq!(view.total, 1);
    assert_eq!(view.items[0].code, "loan_products");
    assert_eq!(view.summary.total, 4);
    assert_eq!(view.summary.unconfigured, 1);
    assert_eq!(view.summary.normal, 3);
    assert_eq!(view.summary.pending_apply, 0);
    assert_eq!(view.summary.runtime_error, 0);
}

#[test]
fn config_center_filter_rejects_unknown_values_and_oversized_search() {
    assert!(AdminConfigCenterFilter::new(None, Some("unknown".to_owned()), None).is_err());
    assert!(AdminConfigCenterFilter::new(None, None, Some("healthy".to_owned())).is_err());
    assert!(AdminConfigCenterFilter::new(Some("查".repeat(101)), None, None).is_err());
}

#[test]
fn config_center_requires_exactly_one_fact_per_catalog_code() {
    let mut missing = complete_facts();
    missing.pop();
    assert!(build_admin_config_center_view(missing, AdminConfigCenterFilter::default()).is_err());

    let mut duplicate = complete_facts();
    duplicate.push(fact("smtp"));
    assert!(build_admin_config_center_view(duplicate, AdminConfigCenterFilter::default()).is_err());

    let mut unexpected = complete_facts();
    unexpected[0].code = "unknown_config".to_owned();
    assert!(
        build_admin_config_center_view(unexpected, AdminConfigCenterFilter::default()).is_err()
    );
}

#[test]
fn config_center_error_summary_redacts_sensitive_values_and_truncates_unicode() {
    let sensitive = safe_admin_config_error_summary(Some(
        "provider failed Authorization: Bearer super-secret-token-value",
    ))
    .unwrap();
    assert_eq!(sensitive, "运行错误包含敏感信息，详细内容已隐藏");
    assert!(!sensitive.contains("super-secret-token-value"));

    let long_error = "错".repeat(200);
    let summary = safe_admin_config_error_summary(Some(&long_error)).unwrap();
    assert_eq!(summary.chars().count(), 161);
    assert!(summary.ends_with('…'));
    assert_eq!(safe_admin_config_error_summary(Some(" \n\t ")), None);
}

#[test]
fn config_center_permission_mapping_and_catalog_use_read_contract() {
    assert_eq!(
        required_admin_permission("GET", "/admin/api/v1/config-center").as_deref(),
        Some("config_center.read")
    );
    assert_eq!(
        required_admin_permission("POST", "/admin/api/v1/config-center").as_deref(),
        Some("config_center.write")
    );
    let catalog = admin_permission_catalog();
    assert!(
        catalog
            .iter()
            .any(|permission| permission == "config_center.read")
    );
}
