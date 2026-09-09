use super::*;

#[test]
fn build_scoped_new_coin_subscription_query_includes_project_id() {
    let query = AdminNewCoinScopedListQuery {
        user_id: Some(1001),
        status: Some("opened".to_owned()),
        email: Some("alpha@example.com".to_owned()),
        limit: Some(40),
        offset: Some(80),
    };

    let flat = build_new_coin_scoped_list_query(9001, query);

    assert_eq!(flat.project_id, Some(9001));
    assert_eq!(flat.user_id, Some(1001));
    assert_eq!(flat.status, Some("opened".to_owned()));
    assert_eq!(flat.email, Some("alpha@example.com".to_owned()));
    assert_eq!(flat.limit, Some(40));
    assert_eq!(flat.offset, Some(80));
}

#[test]
fn build_scoped_new_coin_list_query_allows_empty_filters() {
    let query = AdminNewCoinScopedListQuery {
        user_id: None,
        status: None,
        email: None,
        limit: None,
        offset: None,
    };

    let flat = build_new_coin_scoped_list_query(11, query);

    assert_eq!(flat.project_id, Some(11));
    assert!(flat.user_id.is_none());
    assert!(flat.status.is_none());
    assert!(flat.email.is_none());
    assert!(flat.limit.is_none());
    assert!(flat.offset.is_none());
}

#[test]
fn dashboard_environment_normalizes_aliases_to_a_stable_public_contract() {
    for (raw, expected) in [
        ("production", "production"),
        (" PROD ", "production"),
        ("staging", "staging"),
        ("pre_production", "staging"),
        ("test", "test"),
        ("CI", "test"),
        ("development", "development"),
        ("local", "development"),
        ("private-cluster-name", "development"),
    ] {
        assert_eq!(normalize_admin_dashboard_environment(raw), expected);
    }
}

#[test]
fn audit_log_time_range_accepts_open_and_inclusive_bounds() {
    let instant = DateTime::<Utc>::from_timestamp_millis(1_800_000_000_000).unwrap();

    assert!(validate_admin_audit_log_time_range(None, None).is_ok());
    assert!(validate_admin_audit_log_time_range(Some(instant), None).is_ok());
    assert!(validate_admin_audit_log_time_range(None, Some(instant)).is_ok());
    assert!(validate_admin_audit_log_time_range(Some(instant), Some(instant)).is_ok());
}

#[test]
fn audit_log_time_range_rejects_an_inverted_window() {
    let earlier = DateTime::<Utc>::from_timestamp_millis(1_800_000_000_000).unwrap();
    let later = DateTime::<Utc>::from_timestamp_millis(1_800_000_001_000).unwrap();

    let error = validate_admin_audit_log_time_range(Some(later), Some(earlier)).unwrap_err();
    assert!(matches!(error, AppError::Validation(_)));
    assert_eq!(
        error.to_string(),
        "validation error: created_from must not be later than created_to"
    );
}

fn financial_audit_record()
-> crate::modules::admin::infrastructure::AdminFinancialIdempotencyAuditRecord {
    crate::modules::admin::infrastructure::AdminFinancialIdempotencyAuditRecord {
        duplicate_idempotency_groups: 0,
        missing_idempotency_keys: 0,
        orphan_ledger_entries: 0,
        expired_seconds_orders: 0,
        expired_prediction_orders: 0,
        pending_loan_orders: 3,
        pending_new_coin_subscriptions: 2,
    }
}

#[test]
fn financial_idempotency_audit_treats_business_queues_as_informational() {
    let response = build_admin_financial_idempotency_audit(financial_audit_record(), Utc::now());

    assert_eq!(response.status, "balanced");
    assert_eq!(response.pending_loan_orders, 3);
    assert_eq!(response.pending_new_coin_subscriptions, 2);
    assert_eq!(response.anomaly_count, 0);
    assert!(response.anomalies.is_empty());
}

#[test]
fn financial_idempotency_audit_reports_every_integrity_failure() {
    let mut record = financial_audit_record();
    record.duplicate_idempotency_groups = 1;
    record.missing_idempotency_keys = 2;
    record.orphan_ledger_entries = 3;
    record.expired_seconds_orders = 4;
    record.expired_prediction_orders = 5;

    let response = build_admin_financial_idempotency_audit(record, Utc::now());

    assert_eq!(response.status, "attention");
    assert_eq!(response.anomaly_count, 5);
    assert!(
        response
            .anomalies
            .iter()
            .any(|value| value.contains("幂等键"))
    );
    assert!(
        response
            .anomalies
            .iter()
            .any(|value| value.contains("钱包流水"))
    );
    assert!(
        response
            .anomalies
            .iter()
            .any(|value| value.contains("秒合约"))
    );
    assert!(
        response
            .anomalies
            .iter()
            .any(|value| value.contains("竞猜"))
    );
}
