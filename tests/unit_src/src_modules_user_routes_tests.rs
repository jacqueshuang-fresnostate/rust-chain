use super::*;
use crate::{
    config::Settings,
    modules::{
        auth::TokenScope,
        user::service::{
            USER_INVITE_CODE_LENGTH, generate_user_invite_code, is_valid_user_invite_code,
        },
    },
};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use secrecy::SecretString;
use serde_json::Value;
use tower::ServiceExt;

#[test]
fn user_invite_code_is_six_uppercase_alphanumeric_chars() {
    for _ in 0..32 {
        let code = generate_user_invite_code().unwrap();
        assert_eq!(code.len(), USER_INVITE_CODE_LENGTH);
        assert!(is_valid_user_invite_code(&code));
    }
    assert!(is_valid_user_invite_code("A1B2C3"));
    assert!(!is_valid_user_invite_code("A1B2C"));
    assert!(!is_valid_user_invite_code("A1B2C3D"));
    assert!(!is_valid_user_invite_code("ABC-12"));
    assert!(!is_valid_user_invite_code("abc123"));
}

fn test_state() -> AppState {
    AppState::new(Settings {
        app_env: "test".to_owned(),
        app_host: "127.0.0.1".parse().unwrap(),
        app_port: 0,
        database_url: SecretString::new("mysql://test:test@localhost/test".to_owned()),
        mongodb_uri: SecretString::new("mongodb://localhost:27017".to_owned()),
        mongodb_database: "exchange_test".to_owned(),
        redis_url: SecretString::new("redis://localhost:6379".to_owned()),
        rabbitmq_url: SecretString::new("amqp://guest:guest@localhost:5672/%2f".to_owned()),
        jwt_secret: SecretString::new("test-secret".to_owned()),
        credential_encryption_key: Some(SecretString::new(
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
    })
}

#[tokio::test]
async fn profile_requires_mysql_after_user_auth() {
    assert_user_route_requires_mysql("GET", "/user/profile", Body::empty()).await;
}

#[tokio::test]
async fn avatar_requires_mysql_after_user_auth() {
    assert_user_route_requires_mysql_with_content_type(
        "POST",
        "/user/avatar",
        Body::empty(),
        "multipart/form-data; boundary=avatar",
    )
    .await;
}

#[tokio::test]
async fn two_factor_routes_require_mysql_after_user_auth() {
    for (method, path, body) in [
        ("GET", "/user/2fa", Body::empty()),
        ("POST", "/user/2fa/setup", Body::empty()),
        (
            "POST",
            "/user/2fa/confirm",
            Body::from(r#"{"totp_code":"123456"}"#),
        ),
        (
            "PATCH",
            "/user/2fa/login",
            Body::from(r#"{"enabled":true}"#),
        ),
        ("POST", "/user/2fa/reset-code", Body::empty()),
        (
            "POST",
            "/user/2fa/reset",
            Body::from(r#"{"code":"123456"}"#),
        ),
    ] {
        assert_user_route_requires_mysql(method, path, body).await;
    }
}

async fn assert_user_route_requires_mysql(method: &str, path: &str, body: Body) {
    assert_user_route_requires_mysql_with_content_type(method, path, body, "application/json")
        .await;
}

async fn assert_user_route_requires_mysql_with_content_type(
    method: &str,
    path: &str,
    body: Body,
    content_type: &str,
) {
    let state = test_state();
    let token =
        crate::modules::auth::issue_token(&state.settings, "user:42", TokenScope::User, 900)
            .unwrap();
    let response = routes()
        .with_state(state)
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", content_type)
                .body(body)
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::INTERNAL_SERVER_ERROR,
        "{path}"
    );
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for user routes"),
        "{path}"
    );
}
