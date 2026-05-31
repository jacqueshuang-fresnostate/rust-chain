use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use bigdecimal::BigDecimal;
use exchange_api::{
    config::Settings,
    modules::{
        auth::{TokenScope, issue_token},
        wallet::routes::routes,
    },
    state::AppState,
};
use secrecy::SecretString;
use serde_json::Value;
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::{error::Error, str::FromStr};
use tower::ServiceExt;
use uuid::Uuid;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

fn test_settings() -> Settings {
    Settings {
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
    }
}

async fn mysql_pool() -> Option<MySqlPool> {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!("skipping MySQL route integration test because DATABASE_URL is not set");
            return None;
        }
    };

    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    Some(pool)
}

async fn create_user(pool: &MySqlPool) -> u64 {
    let email = format!("wallet-route-{}@example.test", Uuid::now_v7().simple());
    sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(email)
        .bind("not-a-real-hash")
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id()
}

async fn create_asset(pool: &MySqlPool) -> u64 {
    let suffix = Uuid::now_v7().simple().to_string();
    let symbol = format!("WR{}", &suffix[..12]);
    sqlx::query("INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, 18, 'coin', 'active')")
        .bind(&symbol)
        .bind(&symbol)
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id()
}

async fn seed_wallet(pool: &MySqlPool, user_id: u64, asset_id: u64, ref_id: &str) {
    sqlx::query(
        r#"INSERT INTO wallet_accounts (user_id, asset_id, available, frozen, locked)
           VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(decimal("12.500000000000000000"))
    .bind(decimal("1.500000000000000000"))
    .bind(decimal("3.000000000000000000"))
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind("deposit_credit")
    .bind(decimal("12.500000000000000000"))
    .bind("available")
    .bind(decimal("12.500000000000000000"))
    .bind(decimal("12.500000000000000000"))
    .bind(decimal("1.500000000000000000"))
    .bind(decimal("3.000000000000000000"))
    .bind("deposit_record")
    .bind(ref_id)
    .execute(pool)
    .await
    .unwrap();
}

async fn cleanup_wallet_route_fixture(
    pool: &MySqlPool,
    user_id: u64,
    asset_id: u64,
    ref_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM wallet_ledger WHERE ref_type = ? AND ref_id = ?")
        .bind("deposit_record")
        .bind(ref_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(asset_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn wallet_routes_return_authenticated_user_accounts_and_ledger() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let asset_id = create_asset(&pool).await;
    let ref_id = format!("wallet-route-{}", Uuid::now_v7().simple());
    seed_wallet(&pool, user_id, asset_id, &ref_id).await;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let accounts_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/wallet/accounts")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(accounts_response.status(), StatusCode::OK);
    let accounts_body = axum::body::to_bytes(accounts_response.into_body(), 8192).await?;
    let accounts: Value = serde_json::from_slice(&accounts_body)?;
    assert_eq!(accounts["accounts"].as_array().unwrap().len(), 1);
    assert_eq!(accounts["accounts"][0]["user_id"], user_id);
    assert_eq!(accounts["accounts"][0]["asset_id"], asset_id);
    assert_eq!(
        accounts["accounts"][0]["available"],
        "12.500000000000000000"
    );

    let ledger_response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/wallet/ledger?asset_id={asset_id}&ref_type=deposit_record&ref_id={ref_id}&limit=10"
                ))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let ledger_status = ledger_response.status();
    let ledger_body = axum::body::to_bytes(ledger_response.into_body(), 8192).await?;
    assert_eq!(
        ledger_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&ledger_body)
    );
    let ledger: Value = serde_json::from_slice(&ledger_body)?;
    assert_eq!(ledger["entries"].as_array().unwrap().len(), 1);
    assert_eq!(ledger["entries"][0]["user_id"], user_id);
    assert_eq!(ledger["entries"][0]["ref_id"], ref_id);
    assert_eq!(ledger["entries"][0]["amount"], "12.500000000000000000");

    cleanup_wallet_route_fixture(&pool, user_id, asset_id, &ref_id).await?;
    Ok(())
}
