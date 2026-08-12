use crate::{
    config::Settings,
    error::AppError,
    modules::{
        auth::{AdminAuth, TokenScope, issue_token},
        events::{
            application::{
                authorize_private_ws, list_inbox_records, list_outbox_records,
                requeue_outbox_dead_letter,
            },
            presentation::{
                EventRecordsQuery, EventRecordsResponse, OutboxRecordResponse, PrivateWsQuery,
                RequeueOutboxRequest,
            },
        },
    },
    state::AppState,
};
use chrono::{TimeZone, Utc};

#[tokio::test]
async fn authorize_private_ws_requires_token_input() {
    let state = AppState::new(test_settings());

    let result = authorize_private_ws(&state, PrivateWsQuery { token: None }).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn authorize_private_ws_accepts_user_token_from_query() {
    let state = AppState::new(test_settings());
    let token = issue_token(&state.settings, "user:42", TokenScope::User, 900).unwrap();

    let auth = authorize_private_ws(&state, PrivateWsQuery { token: Some(token) })
        .await
        .unwrap();

    assert_eq!(auth.user_id, 42);
}

#[tokio::test]
async fn authorize_private_ws_rejects_non_user_token() {
    let state = AppState::new(test_settings());
    let token = issue_token(&state.settings, "admin:7", TokenScope::Admin, 900).unwrap();

    let result = authorize_private_ws(&state, PrivateWsQuery { token: Some(token) }).await;

    assert!(result.is_err());
}

#[test]
fn event_record_query_normalizes_status_and_pagination_bounds() {
    let normalized = EventRecordsQuery {
        status: Some("  dead_letter  ".to_owned()),
        limit: Some(1_000),
        offset: Some(200_000),
    }
    .normalize();

    assert_eq!(normalized.status.as_deref(), Some("dead_letter"));
    assert_eq!(normalized.limit, 100);
    assert_eq!(normalized.offset, 100_000);

    let defaults = EventRecordsQuery {
        status: Some("   ".to_owned()),
        limit: None,
        offset: None,
    }
    .normalize();
    assert_eq!(defaults.status, None);
    assert_eq!(defaults.limit, 50);
    assert_eq!(defaults.offset, 0);
}

#[test]
fn outbox_response_dto_preserves_list_and_requeue_json_contracts() {
    let created_at = Utc.timestamp_millis_opt(1_700_000_000_123).unwrap();
    let record = || OutboxRecordResponse {
        id: 42,
        aggregate_type: "wallet".to_owned(),
        aggregate_id: "7".to_owned(),
        event_type: "updated".to_owned(),
        routing_key: "wallet.updated".to_owned(),
        status: "pending".to_owned(),
        retry_count: 0,
        next_retry_at: None,
        published_at: None,
        created_at,
    };
    let response = EventRecordsResponse::new(vec![record()], 1);

    let payload = serde_json::to_value(response).unwrap();
    assert_eq!(
        payload,
        serde_json::json!({
            "records": [{
                "id": 42,
                "aggregate_type": "wallet",
                "aggregate_id": "7",
                "event_type": "updated",
                "routing_key": "wallet.updated",
                "status": "pending",
                "retry_count": 0,
                "next_retry_at": null,
                "published_at": null,
                "created_at": 1_700_000_000_123_i64,
            }],
            "total": 1,
        })
    );
    assert_eq!(payload["records"][0]["status"], "pending");

    let requeue_payload = serde_json::to_value(record()).unwrap();
    assert_eq!(requeue_payload, payload["records"][0]);
}

#[tokio::test]
async fn event_record_use_cases_keep_missing_pool_error_semantics() {
    let state = AppState::new(test_settings());
    let query = || EventRecordsQuery {
        status: None,
        limit: None,
        offset: None,
    };

    for error in [
        list_outbox_records(&state, query()).await.unwrap_err(),
        list_inbox_records(&state, query()).await.unwrap_err(),
    ] {
        assert!(matches!(error, AppError::Internal(_)));
        assert_eq!(
            error.to_string(),
            "internal error: mysql pool is not configured for event routes"
        );
    }
}

#[tokio::test]
async fn requeue_validates_reason_then_admin_identity_before_pool_lookup() {
    let state = AppState::new(test_settings());

    let missing_reason = requeue_outbox_dead_letter(
        &state,
        admin_auth("admin:7"),
        42,
        RequeueOutboxRequest { reason: None },
    )
    .await
    .unwrap_err();
    assert!(matches!(missing_reason, AppError::Validation(_)));
    assert_eq!(
        missing_reason.to_string(),
        "validation error: reason is required"
    );

    let invalid_admin = requeue_outbox_dead_letter(
        &state,
        admin_auth("user:7"),
        42,
        RequeueOutboxRequest {
            reason: Some("重排死信".to_owned()),
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(invalid_admin, AppError::Unauthorized));

    let missing_pool = requeue_outbox_dead_letter(
        &state,
        admin_auth("admin:7"),
        42,
        RequeueOutboxRequest {
            reason: Some("重排死信".to_owned()),
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(missing_pool, AppError::Internal(_)));
    assert_eq!(
        missing_pool.to_string(),
        "internal error: mysql pool is not configured for event routes"
    );
}

fn admin_auth(subject: &str) -> AdminAuth {
    AdminAuth(crate::modules::auth::Claims {
        sub: subject.to_owned(),
        scope: TokenScope::Admin,
        exp: usize::MAX,
        token_id: "events-application-test".to_owned(),
    })
}

fn test_settings() -> Settings {
    Settings {
        app_env: "test".to_owned(),
        app_host: "127.0.0.1".parse().unwrap(),
        app_port: 0,
        database_url: secrecy::SecretString::new("mysql://test:test@localhost/test".to_owned()),
        mongodb_uri: secrecy::SecretString::new("mongodb://localhost:27017".to_owned()),
        mongodb_database: "exchange_test".to_owned(),
        redis_url: secrecy::SecretString::new("redis://localhost:6379".to_owned()),
        rabbitmq_url: secrecy::SecretString::new(
            "amqp://guest:guest@localhost:5672/%2f".to_owned(),
        ),
        jwt_secret: secrecy::SecretString::new("test-secret".to_owned()),
        credential_encryption_key: Some(secrecy::SecretString::new(
            "0123456789abcdef0123456789abcdef".to_owned(),
        )),
        jwt_access_ttl_seconds: 900,
        jwt_refresh_ttl_seconds: 2_592_000,
        bitget_rest_base_url: "https://bitget.test".to_owned(),
        bitget_ws_url: "wss://bitget.test/ws".to_owned(),
        htx_rest_base_url: "https://htx.test".to_owned(),
        htx_ws_url: "wss://htx.test/ws".to_owned(),
        coinbase_rest_base_url: "https://coinbase.test".to_owned(),
        coinbase_ws_url: "wss://coinbase.test/ws".to_owned(),
        market_feed_symbols: Vec::new(),
        market_feed_intervals: Vec::new(),
        market_feed_providers: Vec::new(),
        market_feed_reconnect_seconds: 5,
        market_feed_rest_fallback_timeout_seconds: 3,
        event_inbox_retry_scan_seconds: 10,
        event_outbox_publisher_enabled: true,
        event_outbox_publisher_interval_seconds: 5,
        unlock_scanner_enabled: true,
        unlock_scanner_interval_seconds: 10,
        unlock_scanner_batch_limit: 100,
        kline_recovery_enabled: true,
        kline_recovery_interval_seconds: 30,
        kline_recovery_batch_limit: 100,
        seconds_contract_settlement_enabled: true,
        seconds_contract_settlement_interval_seconds: 5,
        seconds_contract_settlement_batch_limit: 100,
        earn_auto_redemption_enabled: true,
        earn_auto_redemption_interval_seconds: 60,
        earn_auto_redemption_batch_limit: 100,
        margin_liquidation_enabled: true,
        margin_liquidation_interval_seconds: 5,
        margin_liquidation_batch_limit: 100,
        margin_interest_enabled: true,
        margin_interest_interval_seconds: 60,
        margin_interest_batch_limit: 100,
        agent_commission_auto_settle_enabled: false,
        agent_commission_auto_settle_interval_seconds: 60,
        agent_commission_auto_settle_min_age_seconds: 3600,
        agent_commission_auto_settle_batch_limit: 100,
    }
}
