use super::*;

#[test]
fn build_scoped_new_coin_subscription_query_includes_project_id() {
    let query = AdminNewCoinScopedListQuery {
        user_id: Some(1001),
        status: Some("opened".to_owned()),
        email: Some("alpha@example.com".to_owned()),
        limit: Some(40),
    };

    let flat = build_new_coin_scoped_list_query(9001, query);

    assert_eq!(flat.project_id, Some(9001));
    assert_eq!(flat.user_id, Some(1001));
    assert_eq!(flat.status, Some("opened".to_owned()));
    assert_eq!(flat.email, Some("alpha@example.com".to_owned()));
    assert_eq!(flat.limit, Some(40));
}

#[test]
fn build_scoped_new_coin_list_query_allows_empty_filters() {
    let query = AdminNewCoinScopedListQuery {
        user_id: None,
        status: None,
        email: None,
        limit: None,
    };

    let flat = build_new_coin_scoped_list_query(11, query);

    assert_eq!(flat.project_id, Some(11));
    assert!(flat.user_id.is_none());
    assert!(flat.status.is_none());
    assert!(flat.email.is_none());
    assert!(flat.limit.is_none());
}
