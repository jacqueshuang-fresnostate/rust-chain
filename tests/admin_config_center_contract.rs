use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use exchange_api::{build_router, config::Settings, state::AppState};
use secrecy::SecretString;
use tower::ServiceExt;

const INFRASTRUCTURE_SOURCE: &str =
    include_str!("../src/modules/admin/infrastructure/config_center.rs");
const ROUTE_SOURCE: &str = include_str!("../src/modules/admin/routes/config_center.rs");
const PERMISSION_SOURCE: &str = include_str!("../src/modules/admin/service/access_control.rs");

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
        agent_commission_auto_settle_enabled: false,
        agent_commission_auto_settle_interval_seconds: 60,
        agent_commission_auto_settle_min_age_seconds: 3600,
        agent_commission_auto_settle_batch_limit: 100,
    })
}

#[tokio::test]
async fn config_center_route_is_registered_and_requires_admin_auth() {
    let response = build_router(test_state())
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/config-center")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(ROUTE_SOURCE.contains("AdminAuth"));
    assert!(ROUTE_SOURCE.contains("/config-center"));
}

#[test]
fn config_center_sql_contract_covers_exact_authoritative_sources_without_credentials() {
    let codes = [
        "prediction_settings",
        "market_feed",
        "market_strategy",
        "kyc_rules",
        "security_policy",
        "country_configs",
        "loan_products",
        "margin_products",
        "seconds_contract_products",
        "earn_products",
        "smtp",
        "upload_storage",
        "platform_brand",
    ];
    for code in codes {
        assert!(
            INFRASTRUCTURE_SOURCE.contains(&format!("SELECT '{code}' AS code")),
            "missing SQL branch for {code}"
        );
    }
    assert_eq!(INFRASTRUCTURE_SOURCE.matches("UNION ALL").count(), 12);

    for table in [
        "prediction_settings",
        "market_feed_configs",
        "market_strategies",
        "strategy_runs",
        "strategy_versions",
        "kyc_configs",
        "security_policy_configs",
        "country_configs",
        "loan_products",
        "margin_products",
        "seconds_contract_products",
        "earn_products",
        "smtp_configs",
        "admin_audit_logs",
        "upload_storage_configs",
        "platform_brand_configs",
    ] {
        assert!(
            INFRASTRUCTURE_SOURCE.contains(table),
            "missing authoritative source {table}"
        );
    }

    for forbidden in [
        "SELECT *",
        "password_ciphertext",
        "username_ciphertext",
        "bearer_token_ciphertext",
        "access_key_ciphertext",
        "secret_key_ciphertext",
        "api_key_ciphertext",
        "api_secret_ciphertext",
        "passphrase_ciphertext",
    ] {
        assert!(
            !INFRASTRUCTURE_SOURCE.contains(forbidden),
            "credential-bearing SQL is forbidden: {forbidden}"
        );
    }
    assert!(INFRASTRUCTURE_SOURCE.contains("LEFT(MAX(last_sync_error), 512)"));
    assert!(INFRASTRUCTURE_SOURCE.contains("LEFT(MAX(last_reload_error), 512)"));
}

#[test]
fn config_center_permission_contract_is_mapped_and_cataloged() {
    assert!(PERMISSION_SOURCE.contains("\"config_center\","));
    assert!(PERMISSION_SOURCE.contains("(\"/config-center\", \"config_center\")"));
}
