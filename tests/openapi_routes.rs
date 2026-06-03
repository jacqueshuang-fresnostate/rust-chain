use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use exchange_api::{build_router, config::Settings, state::AppState};
use secrecy::SecretString;
use serde_json::Value;
use tower::ServiceExt;

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

async fn request_json(uri: &str) -> Value {
    let response = build_router(test_state())
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 256 * 1024).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn openapi_json() -> Value {
    request_json("/openapi.json").await
}

fn operation_has_bearer_security(openapi: &Value, path: &str, method: &str) -> bool {
    openapi["paths"][path][method]["security"]
        .as_array()
        .is_some_and(|entries| {
            entries
                .iter()
                .any(|entry| entry.get("bearerAuth").is_some())
        })
}

fn schema_is_unix_millis(value: &Value) -> bool {
    let has_integer_type = value.get("type").is_some_and(|schema_type| {
        schema_type == "integer"
            || schema_type
                .as_array()
                .is_some_and(|types| types.iter().any(|value| value == "integer"))
    });
    if has_integer_type && value.get("format") == Some(&Value::String("int64".to_owned())) {
        return true;
    }

    value
        .get("anyOf")
        .or_else(|| value.get("oneOf"))
        .and_then(Value::as_array)
        .is_some_and(|schemas| schemas.iter().any(schema_is_unix_millis))
}

#[tokio::test]
async fn openapi_json_exposes_first_batch_contract() {
    let openapi = openapi_json().await;

    assert_eq!(openapi["openapi"].as_str(), Some("3.1.0"));
    assert!(openapi["info"]["title"].as_str().is_some());
    assert_eq!(
        openapi["components"]["securitySchemes"]["bearerAuth"]["scheme"].as_str(),
        Some("bearer")
    );

    for path in [
        "/health",
        "/api/v1/auth/register",
        "/api/v1/auth/login",
        "/api/v1/auth/refresh",
        "/admin/api/v1/auth/register",
        "/admin/api/v1/auth/login",
        "/admin/api/v1/auth/refresh",
        "/agent/api/v1/auth/register",
        "/agent/api/v1/auth/login",
        "/agent/api/v1/auth/refresh",
        "/api/v1/user/profile",
        "/api/v1/user/email/bind-code",
        "/api/v1/user/email/bind",
        "/api/v1/user/password",
        "/api/v1/user/fund-password",
        "/admin/api/v1/smtp/config",
        "/admin/api/v1/smtp/test",
    ] {
        assert!(openapi["paths"].get(path).is_some(), "missing path {path}");
    }

    assert!(operation_has_bearer_security(
        &openapi,
        "/api/v1/user/profile",
        "get"
    ));
    assert!(operation_has_bearer_security(
        &openapi,
        "/api/v1/user/email/bind-code",
        "post"
    ));
    assert!(operation_has_bearer_security(
        &openapi,
        "/admin/api/v1/smtp/config",
        "get"
    ));

    let error_properties = &openapi["components"]["schemas"]["ErrorResponse"]["properties"];
    assert!(error_properties.get("code").is_some());
    assert!(error_properties.get("message").is_some());

    let profile_properties = &openapi["components"]["schemas"]["UserProfileResponse"]["properties"];
    assert!(schema_is_unix_millis(
        &profile_properties["email_verified_at"]
    ));

    let smtp_response_properties =
        &openapi["components"]["schemas"]["SmtpConfigResponse"]["properties"];
    assert!(smtp_response_properties.get("username_mask").is_some());
    assert!(smtp_response_properties.get("password_set").is_some());
    assert!(smtp_response_properties.get("password").is_none());
    assert!(
        smtp_response_properties
            .get("password_ciphertext")
            .is_none()
    );
    assert!(
        smtp_response_properties
            .get("username_ciphertext")
            .is_none()
    );
}

#[tokio::test]
async fn openapi_json_alias_is_registered() {
    let openapi = request_json("/api/openapi.json").await;

    assert_eq!(openapi["openapi"].as_str(), Some("3.1.0"));
    assert!(openapi["paths"].get("/api/v1/user/profile").is_some());
}

#[tokio::test]
async fn swagger_ui_route_is_registered() {
    for uri in ["/docs", "/api/docs"] {
        let response = build_router(test_state())
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert!(
            matches!(
                response.status(),
                StatusCode::OK
                    | StatusCode::MOVED_PERMANENTLY
                    | StatusCode::SEE_OTHER
                    | StatusCode::TEMPORARY_REDIRECT
            ),
            "unexpected Swagger UI status for {uri}: {}",
            response.status()
        );
    }
}
