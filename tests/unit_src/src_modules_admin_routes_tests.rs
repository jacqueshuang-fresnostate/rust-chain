use super::*;
use crate::{
    config::Settings,
    modules::auth::{TokenScope, issue_token},
    state::AppState,
};
use axum::{
    body::Body,
    http::{Request, StatusCode, header::AUTHORIZATION},
};
use secrecy::SecretString;
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

async fn post_agents(app: Router, token: Option<&str>) -> StatusCode {
    let mut request = Request::builder().method("POST").uri("/agents");
    if let Some(token) = token {
        request = request.header(AUTHORIZATION, format!("Bearer {token}"));
    }

    app.oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn admin_routes_require_admin_scope() {
    let state = test_state();
    let user_token = issue_token(
        &state.settings,
        "user:1",
        TokenScope::User,
        state.settings.jwt_access_ttl_seconds,
    )
    .unwrap();
    let admin_token = issue_token(
        &state.settings,
        "admin:1",
        TokenScope::Admin,
        state.settings.jwt_access_ttl_seconds,
    )
    .unwrap();
    let app = routes().with_state(state);

    assert_eq!(
        post_agents(app.clone(), None).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post_agents(app.clone(), Some(&user_token)).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post_agents(app, Some(&admin_token)).await,
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
}
