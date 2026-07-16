use super::*;
use crate::{
    config::Settings,
    modules::auth::{TokenScope, issue_token},
    modules::wallet::application::{
        route_limit as wallet_route_limit, route_offset as wallet_route_offset,
    },
};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
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

fn bearer_token(state: &AppState) -> String {
    issue_token(&state.settings, "user:42", TokenScope::User, 900).unwrap()
}

#[tokio::test]
async fn wallet_accounts_route_requires_user_auth() {
    let app = routes().with_state(test_state());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/wallet/accounts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn wallet_withdrawal_route_requires_user_auth() {
    let app = routes().with_state(test_state());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallet/withdrawals")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"asset_symbol":"USDT","address":"TTest","amount":"1.000000000000000000","fee":"0.100000000000000000"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn wallet_withdrawal_route_returns_clear_error_without_mysql_after_auth() {
    let state = test_state();
    let token = bearer_token(&state);
    let app = routes().with_state(state);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallet/withdrawals")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"asset_symbol":"USDT","address":"TTest","amount":"1.000000000000000000","fee":"0.100000000000000000","fund_password":"123456"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert_eq!(
        payload["message"],
        "internal error: mysql pool is not configured for wallet routes"
    );
}

#[tokio::test]
async fn wallet_accounts_route_returns_clear_error_without_mysql() {
    let state = test_state();
    let token = bearer_token(&state);
    let app = routes().with_state(state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/wallet/accounts")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert_eq!(
        payload["message"],
        "internal error: mysql pool is not configured for wallet routes"
    );
}

#[tokio::test]
async fn wallet_deposit_networks_route_rejects_invalid_asset_symbol() {
    let state = test_state();
    let token = bearer_token(&state);
    let app = routes().with_state(state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/wallet/deposit-networks?asset_symbol=BTC-USDT")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["code"], "VALIDATION_ERROR");
}

#[test]
fn wallet_ledger_limit_is_clamped() {
    assert_eq!(wallet_route_limit(None), 50);
    assert_eq!(wallet_route_limit(Some(0)), 1);
    assert_eq!(wallet_route_limit(Some(500)), 100);
}

#[test]
fn wallet_ledger_offset_is_clamped() {
    assert_eq!(wallet_route_offset(None), 0);
    assert_eq!(wallet_route_offset(Some(20)), 20);
    assert_eq!(wallet_route_offset(Some(500_000)), 100_000);
}
