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
