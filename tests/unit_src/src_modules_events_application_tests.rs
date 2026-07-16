use crate::{
    config::Settings,
    modules::{
        auth::{TokenScope, issue_token},
        events::{application::authorize_private_ws, presentation::PrivateWsQuery},
    },
    state::AppState,
};

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
    }
}
