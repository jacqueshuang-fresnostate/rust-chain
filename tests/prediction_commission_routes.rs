use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use bigdecimal::BigDecimal;
use exchange_api::{
    config::Settings,
    modules::{
        auth::{TokenScope, issue_token},
        prediction::routes::user_routes,
    },
    state::AppState,
};
use secrecy::SecretString;
use serde_json::Value;
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::{error::Error, str::FromStr};
use tower::ServiceExt;
use uuid::Uuid;

mod support;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).expect("valid decimal")
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

async fn mysql_pool() -> Option<MySqlPool> {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!("skipping prediction commission test because DATABASE_URL is not set");
            return None;
        }
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("connect test mysql");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("run migrations");
    Some(pool)
}

#[tokio::test]
async fn prediction_order_creates_precise_idempotent_agent_commission() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let suffix = Uuid::now_v7().simple().to_string();
    let user_id = sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(format!("prediction-commission-{suffix}@example.test"))
        .bind("not-a-real-hash")
        .execute(&pool)
        .await?
        .last_insert_id();
    let commission_fixture =
        support::seed_direct_agent_commission(&pool, user_id, "prediction", "0.05000000").await?;
    let asset_id = sqlx::query(
        "INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, 8, 'coin', 'active')",
    )
    .bind(format!("PC{}", &suffix[..12]))
    .bind(format!("Prediction commission {suffix}"))
    .execute(&pool)
    .await?
    .last_insert_id();
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(asset_id)
        .bind(decimal("20.00000000"))
        .execute(&pool)
        .await?;
    let market_id = sqlx::query(
        r#"INSERT INTO prediction_markets
           (external_market_id, title, tags_json, yes_price, no_price)
           VALUES (?, ?, JSON_ARRAY(), 0.50000000, 0.50000000)"#,
    )
    .bind(format!("prediction-commission-{suffix}"))
    .bind("Prediction commission market")
    .execute(&pool)
    .await?
    .last_insert_id();
    let quote_id = format!("prediction-quote-{suffix}");
    sqlx::query(
        r#"INSERT INTO prediction_quotes
           (quote_id, user_id, market_id, outcome, asset_id, stake_amount, fee_amount,
            accepted_price, shares, theoretical_payout, effective_payout_cap, expires_at)
           VALUES (?, ?, ?, 'yes', ?, ?, ?, ?, ?, ?, ?, DATE_ADD(NOW(6), INTERVAL 1 HOUR))"#,
    )
    .bind(&quote_id)
    .bind(user_id)
    .bind(market_id)
    .bind(asset_id)
    .bind(decimal("10.12345678"))
    .bind(decimal("0.10000000"))
    .bind(decimal("0.50000000"))
    .bind(decimal("20.24691356"))
    .bind(decimal("20.24691356"))
    .bind(decimal("100.00000000"))
    .execute(&pool)
    .await?;

    let token = issue_token(
        &test_settings(),
        format!("user:{user_id}"),
        TokenScope::User,
        900,
    )?;
    let app = user_routes().with_state(AppState::new(test_settings()).with_mysql(pool.clone()));
    let idempotency_key = format!("prediction-order-{suffix}");
    let request_body = serde_json::json!({
        "quote_id": quote_id,
        "idempotency_key": idempotency_key,
    })
    .to_string();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/prediction/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body.clone()))?,
        )
        .await?;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 65_536).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );
    let payload: Value = serde_json::from_slice(&body)?;
    let order_id = payload["order"]["id"]
        .as_u64()
        .expect("prediction order id");

    let replay = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/prediction/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))?,
        )
        .await?;
    assert_eq!(replay.status(), StatusCode::OK);

    let records: Vec<(u64, BigDecimal, BigDecimal, BigDecimal, u64, String)> = sqlx::query_as(
        r#"SELECT agent_id, source_amount, commission_rate, commission_amount,
                  payout_asset_id, status
           FROM agent_commission_records
           WHERE user_id = ? AND source_type = 'prediction_order' AND source_id = ?"#,
    )
    .bind(user_id)
    .bind(order_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].0, commission_fixture.agent_id);
    assert_eq!(records[0].1, decimal("10.123456780000000000"));
    assert_eq!(records[0].2, decimal("0.05000000"));
    assert_eq!(records[0].3, decimal("0.50617283"));
    assert_eq!(records[0].4, asset_id);
    assert_eq!(records[0].5, "pending");

    support::cleanup_direct_agent_commission(&pool, user_id, commission_fixture).await?;
    sqlx::query("DELETE FROM wallet_ledger WHERE ref_type = 'prediction_order' AND ref_id = ?")
        .bind(order_id.to_string())
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM prediction_orders WHERE id = ?")
        .bind(order_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM prediction_quotes WHERE quote_id = ?")
        .bind(&quote_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM prediction_markets WHERE id = ?")
        .bind(market_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    Ok(())
}
