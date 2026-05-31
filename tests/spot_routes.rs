use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use bigdecimal::BigDecimal;
use chrono::{TimeZone, Utc};
use exchange_api::{
    config::Settings,
    modules::{
        auth::{TokenScope, issue_token},
        events::{EventBroadcastHub, WebSocketChannel},
        spot::routes::{admin_routes, routes},
    },
    state::AppState,
};
use secrecy::SecretString;
use serde_json::Value;
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::{error::Error, str::FromStr, sync::Arc, time::Duration};
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
            eprintln!("skipping MySQL spot route test because DATABASE_URL is not set");
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
    let email = format!("spot-route-{}@example.test", Uuid::now_v7().simple());
    sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(email)
        .bind("not-a-real-hash")
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id()
}

async fn create_asset(pool: &MySqlPool, prefix: &str) -> (u64, String) {
    let suffix = Uuid::now_v7().simple().to_string();
    let symbol = format!("{prefix}{}", &suffix[16..32]);
    let asset_id = sqlx::query(
        "INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, 18, 'coin', 'active')",
    )
    .bind(&symbol)
    .bind(&symbol)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id();
    (asset_id, symbol)
}

async fn create_pair(
    pool: &MySqlPool,
    base_asset: u64,
    quote_asset: u64,
    base_symbol: &str,
    quote_symbol: &str,
) -> String {
    let symbol = format!("{base_symbol}-{quote_symbol}");
    sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, 2, 4, ?, 'active', 'spot')"#,
    )
    .bind(base_asset)
    .bind(quote_asset)
    .bind(&symbol)
    .bind(decimal("10.000000000000000000"))
    .execute(pool)
    .await
    .unwrap();
    symbol
}

async fn pair_id(pool: &MySqlPool, pair_symbol: &str) -> Result<u64, sqlx::Error> {
    sqlx::query_as("SELECT id FROM trading_pairs WHERE symbol = ?")
        .bind(pair_symbol)
        .fetch_one(pool)
        .await
        .map(|(id,)| id)
}

async fn seed_open_buy_order(
    pool: &MySqlPool,
    user_id: u64,
    pair_symbol: &str,
) -> Result<String, sqlx::Error> {
    Ok(sqlx::query(
        r#"INSERT INTO spot_orders
           (user_id, pair_id, side, order_type, price, quantity, filled_quantity, status)
           VALUES (?, ?, 'buy', 'limit', ?, ?, 0, 'open')"#,
    )
    .bind(user_id)
    .bind(pair_id(pool, pair_symbol).await?)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("2.0000"))
    .execute(pool)
    .await?
    .last_insert_id()
    .to_string())
}

async fn seed_open_order(
    pool: &MySqlPool,
    user_id: u64,
    pair_symbol: &str,
    side: &str,
    price: &str,
    quantity: &str,
) -> Result<String, sqlx::Error> {
    Ok(sqlx::query(
        r#"INSERT INTO spot_orders
           (user_id, pair_id, side, order_type, price, quantity, filled_quantity, status)
           VALUES (?, ?, ?, 'limit', ?, ?, 0, 'open')"#,
    )
    .bind(user_id)
    .bind(pair_id(pool, pair_symbol).await?)
    .bind(side)
    .bind(decimal(price))
    .bind(decimal(quantity))
    .execute(pool)
    .await?
    .last_insert_id()
    .to_string())
}

#[tokio::test]
async fn admin_spot_lists_orders_and_trades_with_filters() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let buyer_id = create_user(&pool).await;
    let seller_id = create_user(&pool).await;
    let other_user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "AB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "AQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    let pair_db_id = pair_id(&pool, &pair_symbol).await?;
    let first_created_at = Utc.with_ymd_and_hms(2026, 5, 30, 9, 0, 0).unwrap();
    let second_created_at = Utc.with_ymd_and_hms(2026, 5, 30, 9, 1, 0).unwrap();
    let third_created_at = Utc.with_ymd_and_hms(2026, 5, 30, 9, 2, 0).unwrap();
    let buy_order_id = sqlx::query(
        r#"INSERT INTO spot_orders
           (user_id, pair_id, side, order_type, price, quantity, filled_quantity, status, created_at)
           VALUES (?, ?, 'buy', 'limit', ?, ?, 0, 'open', ?)"#,
    )
    .bind(buyer_id)
    .bind(pair_db_id)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("2.0000"))
    .bind(first_created_at.naive_utc())
    .execute(&pool)
    .await?
    .last_insert_id()
    .to_string();
    let sell_order_id = sqlx::query(
        r#"INSERT INTO spot_orders
           (user_id, pair_id, side, order_type, price, quantity, filled_quantity, status, created_at)
           VALUES (?, ?, 'sell', 'limit', ?, ?, 0, 'filled', ?)"#,
    )
    .bind(seller_id)
    .bind(pair_db_id)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("2.0000"))
    .bind(second_created_at.naive_utc())
    .execute(&pool)
    .await?
    .last_insert_id()
    .to_string();
    let other_order_id = sqlx::query(
        r#"INSERT INTO spot_orders
           (user_id, pair_id, side, order_type, price, quantity, filled_quantity, status, created_at)
           VALUES (?, ?, 'buy', 'limit', ?, ?, 0, 'open', ?)"#,
    )
    .bind(other_user_id)
    .bind(pair_db_id)
    .bind(decimal("11.000000000000000000"))
    .bind(decimal("1.0000"))
    .bind(third_created_at.naive_utc())
    .execute(&pool)
    .await?
    .last_insert_id()
    .to_string();
    let first_trade_id = sqlx::query(
        r#"INSERT INTO spot_trades
           (pair_id, buy_order_id, sell_order_id, price, quantity, fee, idempotency_key, created_at)
           VALUES (?, ?, ?, ?, ?, 0, ?, ?)"#,
    )
    .bind(pair_db_id)
    .bind(&buy_order_id)
    .bind(&sell_order_id)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("1.000000000000000000"))
    .bind(format!("spot-admin-list-{}-1", Uuid::now_v7().simple()))
    .bind(second_created_at.naive_utc())
    .execute(&pool)
    .await?
    .last_insert_id()
    .to_string();
    let second_trade_id = sqlx::query(
        r#"INSERT INTO spot_trades
           (pair_id, buy_order_id, sell_order_id, price, quantity, fee, idempotency_key, created_at)
           VALUES (?, ?, ?, ?, ?, 0, ?, ?)"#,
    )
    .bind(pair_db_id)
    .bind(&other_order_id)
    .bind(&sell_order_id)
    .bind(decimal("11.000000000000000000"))
    .bind(decimal("1.000000000000000000"))
    .bind(format!("spot-admin-list-{}-2", Uuid::now_v7().simple()))
    .bind(third_created_at.naive_utc())
    .execute(&pool)
    .await?
    .last_insert_id()
    .to_string();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let user_token =
        issue_token(&settings, format!("user:{buyer_id}"), TokenScope::User, 900).unwrap();
    let app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let orders_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/spot/orders?pair_id={pair_symbol}&status=open&limit=2"
                ))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let orders_status = orders_response.status();
    let orders_body = axum::body::to_bytes(orders_response.into_body(), 8192).await?;
    assert_eq!(
        orders_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&orders_body)
    );
    let orders_payload: Value = serde_json::from_slice(&orders_body)?;
    let orders = orders_payload["orders"].as_array().unwrap();
    assert_eq!(orders.len(), 2);
    assert_eq!(orders[0]["id"], other_order_id);
    assert_eq!(orders[0]["user_id"], other_user_id.to_string());
    assert_eq!(orders[0]["status"], "open");
    assert_eq!(orders[1]["id"], buy_order_id);
    assert_eq!(orders[1]["user_id"], buyer_id.to_string());

    let filtered_orders_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/spot/orders?user_id={buyer_id}&status=open&limit=10"
                ))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let filtered_orders_status = filtered_orders_response.status();
    let filtered_orders_body =
        axum::body::to_bytes(filtered_orders_response.into_body(), 8192).await?;
    assert_eq!(
        filtered_orders_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&filtered_orders_body)
    );
    let filtered_orders_payload: Value = serde_json::from_slice(&filtered_orders_body)?;
    let filtered_orders = filtered_orders_payload["orders"].as_array().unwrap();
    assert_eq!(filtered_orders.len(), 1);
    assert_eq!(filtered_orders[0]["id"], buy_order_id);
    assert_eq!(filtered_orders[0]["user_id"], buyer_id.to_string());

    let trades_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/spot/trades?pair_id={pair_symbol}&limit=10"))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let trades_status = trades_response.status();
    let trades_body = axum::body::to_bytes(trades_response.into_body(), 8192).await?;
    assert_eq!(
        trades_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&trades_body)
    );
    let trades_payload: Value = serde_json::from_slice(&trades_body)?;
    let trades = trades_payload["trades"].as_array().unwrap();
    assert_eq!(trades.len(), 2);
    assert_eq!(trades[0]["id"], second_trade_id);
    assert_eq!(trades[0]["buy_order_id"], other_order_id);
    assert_eq!(trades[0]["sell_order_id"], sell_order_id);
    assert!(trades[0]["created_at"].is_number());
    assert_eq!(trades[1]["id"], first_trade_id);

    let filtered_trades_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/spot/trades?user_id={buyer_id}&limit=10"))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let filtered_trades_status = filtered_trades_response.status();
    let filtered_trades_body =
        axum::body::to_bytes(filtered_trades_response.into_body(), 8192).await?;
    assert_eq!(
        filtered_trades_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&filtered_trades_body)
    );
    let filtered_trades_payload: Value = serde_json::from_slice(&filtered_trades_body)?;
    let filtered_trades = filtered_trades_payload["trades"].as_array().unwrap();
    assert_eq!(filtered_trades.len(), 1);
    assert_eq!(filtered_trades[0]["id"], first_trade_id);
    assert_eq!(filtered_trades[0]["buy_order_id"], buy_order_id);

    let forbidden_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    sqlx::query("DELETE FROM spot_trades WHERE id IN (?, ?)")
        .bind(&first_trade_id)
        .bind(&second_trade_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM spot_orders WHERE id IN (?, ?, ?)")
        .bind(&buy_order_id)
        .bind(&sell_order_id)
        .bind(&other_order_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM trading_pairs WHERE symbol = ?")
        .bind(&pair_symbol)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id IN (?, ?)")
        .bind(base_asset)
        .bind(quote_asset)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id IN (?, ?, ?)")
        .bind(buyer_id)
        .bind(seller_id)
        .bind(other_user_id)
        .execute(&pool)
        .await?;

    assert_eq!(forbidden_response.status(), StatusCode::FORBIDDEN);
    Ok(())
}

#[tokio::test]
async fn spot_create_limit_buy_order_freezes_quote_wallet() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "SB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "SQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("spot-route-{}", Uuid::now_v7().simple());
    let hub = EventBroadcastHub::new(16);
    let mut private_events = hub.subscribe(&WebSocketChannel::private_user(user_id));
    let app = routes().with_state(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_event_broadcast_hub(hub),
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"limit","price":"10.000000000000000000","quantity":"2.0000","idempotency_key":"{idempotency_key}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 8192).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );
    let order: Value = serde_json::from_slice(&body)?;
    assert_eq!(order["status"], "pending");
    let order_id = order["id"].as_str().unwrap().to_owned();
    let event: Value = serde_json::from_str(private_events.recv().await?.payload())?;
    assert_eq!(event["type"], "spot.order.created");
    assert_eq!(event["order_id"], order_id);
    assert_eq!(event["pair_id"], pair_symbol);
    assert_eq!(event["side"], "buy");
    assert_eq!(event["order_type"], "limit");
    assert_eq!(event["status"], "pending");

    let (available, frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        available.normalized(),
        decimal("80.000000000000000000").normalized()
    );
    assert_eq!(
        frozen.normalized(),
        decimal("20.000000000000000000").normalized()
    );

    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'spot_order' AND ref_id = ?",
    )
    .bind(&order_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 2);

    cleanup_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &order_id,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn spot_create_order_is_idempotent_for_repeated_request_key() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "DB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "DQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("spot-route-{}", Uuid::now_v7().simple());
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let request_body = format!(
        r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"limit","price":"10.000000000000000000","quantity":"2.0000","idempotency_key":"{idempotency_key}"}}"#
    );

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let first_status = first_response.status();
    let first_body = axum::body::to_bytes(first_response.into_body(), 8192).await?;
    assert_eq!(
        first_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&first_body)
    );
    let first_order: Value = serde_json::from_slice(&first_body)?;
    let first_order_id = first_order["id"].as_str().unwrap().to_owned();

    let second_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_status = second_response.status();
    let second_body = axum::body::to_bytes(second_response.into_body(), 8192).await?;
    assert_eq!(
        second_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&second_body)
    );
    let second_order: Value = serde_json::from_slice(&second_body)?;
    assert_eq!(second_order["id"], first_order_id);

    let (order_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM spot_orders WHERE user_id = ? AND idempotency_key = ?",
    )
    .bind(user_id)
    .bind(&idempotency_key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(order_count, 1);

    let (available, frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        available.normalized(),
        decimal("80.000000000000000000").normalized()
    );
    assert_eq!(
        frozen.normalized(),
        decimal("20.000000000000000000").normalized()
    );

    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'spot_order' AND ref_id = ?",
    )
    .bind(&first_order_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 2);

    cleanup_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &first_order_id,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn spot_create_order_idempotency_key_accepts_numeric_pair_id_replay()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "NB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "NQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    let pair_db_id = pair_id(&pool, &pair_symbol).await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("spot-route-{}", Uuid::now_v7().simple());
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let request_body = format!(
        r#"{{"pair_id":"{pair_db_id}","side":"buy","order_type":"limit","price":"10.000000000000000000","quantity":"2.0000","idempotency_key":"{idempotency_key}"}}"#
    );

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let first_status = first_response.status();
    let first_payload = axum::body::to_bytes(first_response.into_body(), 8192).await?;
    assert_eq!(
        first_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&first_payload)
    );
    let first_order: Value = serde_json::from_slice(&first_payload)?;
    let first_order_id = first_order["id"].as_str().unwrap().to_owned();
    assert_eq!(first_order["pair_id"], pair_symbol);

    let second_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_status = second_response.status();
    let second_payload = axum::body::to_bytes(second_response.into_body(), 8192).await?;

    cleanup_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &first_order_id,
    )
    .await?;

    assert_eq!(
        second_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&second_payload)
    );
    let second_order: Value = serde_json::from_slice(&second_payload)?;
    assert_eq!(second_order["id"], first_order_id);
    assert_eq!(second_order["pair_id"], pair_symbol);
    Ok(())
}

#[tokio::test]
async fn spot_create_order_idempotency_key_accepts_case_alias_replay() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "AB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "AQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("spot-route-{}", Uuid::now_v7().simple());
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let pair_alias = pair_symbol.to_lowercase();
    assert_ne!(pair_alias, pair_symbol);
    let request_body = format!(
        r#"{{"pair_id":"{pair_alias}","side":"buy","order_type":"limit","price":"10.000000000000000000","quantity":"2.0000","idempotency_key":"{idempotency_key}"}}"#
    );

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let first_status = first_response.status();
    let first_payload = axum::body::to_bytes(first_response.into_body(), 8192).await?;
    assert_eq!(
        first_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&first_payload)
    );
    let first_order: Value = serde_json::from_slice(&first_payload)?;
    let first_order_id = first_order["id"].as_str().unwrap().to_owned();
    assert_eq!(first_order["pair_id"], pair_symbol);

    let second_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_status = second_response.status();
    let second_payload = axum::body::to_bytes(second_response.into_body(), 8192).await?;

    cleanup_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &first_order_id,
    )
    .await?;

    assert_eq!(
        second_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&second_payload)
    );
    let second_order: Value = serde_json::from_slice(&second_payload)?;
    assert_eq!(second_order["id"], first_order_id);
    assert_eq!(second_order["pair_id"], pair_symbol);
    Ok(())
}

#[tokio::test]
async fn spot_create_limit_order_idempotency_accepts_legacy_null_request_price()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "LB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "LQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("spot-route-{}", Uuid::now_v7().simple());
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let request_body = format!(
        r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"limit","price":"10.000000000000000000","quantity":"2.0000","idempotency_key":"{idempotency_key}"}}"#
    );

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let first_status = first_response.status();
    let first_payload = axum::body::to_bytes(first_response.into_body(), 8192).await?;
    assert_eq!(
        first_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&first_payload)
    );
    let first_order: Value = serde_json::from_slice(&first_payload)?;
    let first_order_id = first_order["id"].as_str().unwrap().to_owned();

    sqlx::query("UPDATE spot_orders SET request_price = NULL WHERE id = ?")
        .bind(&first_order_id)
        .execute(&pool)
        .await?;

    let second_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_status = second_response.status();
    let second_payload = axum::body::to_bytes(second_response.into_body(), 8192).await?;

    cleanup_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &first_order_id,
    )
    .await?;

    assert_eq!(
        second_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&second_payload)
    );
    let second_order: Value = serde_json::from_slice(&second_payload)?;
    assert_eq!(second_order["id"], first_order_id);
    Ok(())
}

#[tokio::test]
async fn spot_create_market_order_idempotency_accepts_legacy_null_reference_price()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "HB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "HQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("spot-route-{}", Uuid::now_v7().simple());
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let request_body = format!(
        r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"market","quantity":"2.0000","reference_price":"10.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let first_status = first_response.status();
    let first_payload = axum::body::to_bytes(first_response.into_body(), 8192).await?;
    assert_eq!(
        first_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&first_payload)
    );
    let first_order: Value = serde_json::from_slice(&first_payload)?;
    let first_order_id = first_order["id"].as_str().unwrap().to_owned();

    sqlx::query("UPDATE spot_orders SET request_reference_price = NULL WHERE id = ?")
        .bind(&first_order_id)
        .execute(&pool)
        .await?;

    let second_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_status = second_response.status();
    let second_payload = axum::body::to_bytes(second_response.into_body(), 8192).await?;

    cleanup_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &first_order_id,
    )
    .await?;

    assert_eq!(
        second_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&second_payload)
    );
    let second_order: Value = serde_json::from_slice(&second_payload)?;
    assert_eq!(second_order["id"], first_order_id);
    Ok(())
}

#[tokio::test]
async fn spot_create_legacy_market_sell_idempotency_rejects_changed_reference_price()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "SB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "SQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(base_asset)
        .bind(decimal("5.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("spot-route-{}", Uuid::now_v7().simple());
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let first_body = format!(
        r#"{{"pair_id":"{pair_symbol}","side":"sell","order_type":"market","quantity":"2.0000","reference_price":"10.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );
    let mismatched_body = format!(
        r#"{{"pair_id":"{pair_symbol}","side":"sell","order_type":"market","quantity":"2.0000","reference_price":"1.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(first_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let first_status = first_response.status();
    let first_payload = axum::body::to_bytes(first_response.into_body(), 8192).await?;
    assert_eq!(
        first_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&first_payload)
    );
    let first_order: Value = serde_json::from_slice(&first_payload)?;
    let first_order_id = first_order["id"].as_str().unwrap().to_owned();

    sqlx::query("UPDATE spot_orders SET request_reference_price = NULL WHERE id = ?")
        .bind(&first_order_id)
        .execute(&pool)
        .await?;

    let second_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(mismatched_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_status = second_response.status();
    let second_payload = axum::body::to_bytes(second_response.into_body(), 8192).await?;

    cleanup_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &first_order_id,
    )
    .await?;

    assert_eq!(
        second_status,
        StatusCode::CONFLICT,
        "payload: {}",
        String::from_utf8_lossy(&second_payload)
    );
    Ok(())
}

#[tokio::test]
async fn spot_create_legacy_market_order_idempotency_rejects_added_unused_price()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "PB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "PQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("spot-route-{}", Uuid::now_v7().simple());
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let first_body = format!(
        r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"market","quantity":"2.0000","reference_price":"10.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );
    let mismatched_body = format!(
        r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"market","price":"999.000000000000000000","quantity":"2.0000","reference_price":"10.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(first_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let first_status = first_response.status();
    let first_payload = axum::body::to_bytes(first_response.into_body(), 8192).await?;
    assert_eq!(
        first_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&first_payload)
    );
    let first_order: Value = serde_json::from_slice(&first_payload)?;
    let first_order_id = first_order["id"].as_str().unwrap().to_owned();

    sqlx::query(
        "UPDATE spot_orders SET request_reference_price = NULL, request_price = NULL WHERE id = ?",
    )
    .bind(&first_order_id)
    .execute(&pool)
    .await?;

    let second_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(mismatched_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_status = second_response.status();
    let second_payload = axum::body::to_bytes(second_response.into_body(), 8192).await?;

    cleanup_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &first_order_id,
    )
    .await?;

    assert_eq!(
        second_status,
        StatusCode::CONFLICT,
        "payload: {}",
        String::from_utf8_lossy(&second_payload)
    );
    Ok(())
}

#[tokio::test]
async fn spot_create_order_idempotency_key_rejects_mismatched_replay_request()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "RB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "RQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("spot-route-{}", Uuid::now_v7().simple());
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let first_body = format!(
        r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"limit","price":"10.000000000000000000","quantity":"2.0000","idempotency_key":"{idempotency_key}"}}"#
    );
    let mismatched_body = format!(
        r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"limit","price":"10.000000000000000000","quantity":"3.0000","idempotency_key":"{idempotency_key}"}}"#
    );

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(first_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let first_status = first_response.status();
    let first_payload = axum::body::to_bytes(first_response.into_body(), 8192).await?;
    assert_eq!(
        first_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&first_payload)
    );
    let first_order: Value = serde_json::from_slice(&first_payload)?;
    let first_order_id = first_order["id"].as_str().unwrap().to_owned();

    let second_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(mismatched_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_status = second_response.status();
    let second_payload = axum::body::to_bytes(second_response.into_body(), 8192).await?;

    cleanup_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &first_order_id,
    )
    .await?;

    assert_eq!(
        second_status,
        StatusCode::CONFLICT,
        "payload: {}",
        String::from_utf8_lossy(&second_payload)
    );
    Ok(())
}

#[tokio::test]
async fn spot_create_market_sell_idempotency_rejects_changed_reference_price()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "MS").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "MR").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(base_asset)
        .bind(decimal("5.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("spot-route-{}", Uuid::now_v7().simple());
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let first_body = format!(
        r#"{{"pair_id":"{pair_symbol}","side":"sell","order_type":"market","quantity":"2.0000","reference_price":"10.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );
    let mismatched_body = format!(
        r#"{{"pair_id":"{pair_symbol}","side":"sell","order_type":"market","quantity":"2.0000","reference_price":"1.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(first_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let first_status = first_response.status();
    let first_payload = axum::body::to_bytes(first_response.into_body(), 8192).await?;
    assert_eq!(
        first_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&first_payload)
    );
    let first_order: Value = serde_json::from_slice(&first_payload)?;
    let first_order_id = first_order["id"].as_str().unwrap().to_owned();

    let second_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(mismatched_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_status = second_response.status();
    let second_payload = axum::body::to_bytes(second_response.into_body(), 8192).await?;

    cleanup_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &first_order_id,
    )
    .await?;

    assert_eq!(
        second_status,
        StatusCode::CONFLICT,
        "payload: {}",
        String::from_utf8_lossy(&second_payload)
    );
    Ok(())
}

#[tokio::test]
async fn spot_create_market_order_idempotency_accepts_same_unused_price_replay()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "UP").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "UQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("spot-route-{}", Uuid::now_v7().simple());
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let request_body = format!(
        r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"market","price":"10.000000000000000000","quantity":"2.0000","reference_price":"10.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let first_status = first_response.status();
    let first_payload = axum::body::to_bytes(first_response.into_body(), 8192).await?;
    assert_eq!(
        first_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&first_payload)
    );
    let first_order: Value = serde_json::from_slice(&first_payload)?;
    let first_order_id = first_order["id"].as_str().unwrap().to_owned();

    let second_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_status = second_response.status();
    let second_payload = axum::body::to_bytes(second_response.into_body(), 8192).await?;

    cleanup_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &first_order_id,
    )
    .await?;

    assert_eq!(
        second_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&second_payload)
    );
    let second_order: Value = serde_json::from_slice(&second_payload)?;
    assert_eq!(second_order["id"], first_order_id);
    Ok(())
}

#[tokio::test]
async fn spot_create_market_order_idempotency_rejects_changed_unused_price()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "CP").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "CQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("spot-route-{}", Uuid::now_v7().simple());
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let first_body = format!(
        r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"market","price":"10.000000000000000000","quantity":"2.0000","reference_price":"10.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );
    let mismatched_body = format!(
        r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"market","price":"11.000000000000000000","quantity":"2.0000","reference_price":"10.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(first_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let first_status = first_response.status();
    let first_payload = axum::body::to_bytes(first_response.into_body(), 8192).await?;
    assert_eq!(
        first_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&first_payload)
    );
    let first_order: Value = serde_json::from_slice(&first_payload)?;
    let first_order_id = first_order["id"].as_str().unwrap().to_owned();

    let second_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(mismatched_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_status = second_response.status();
    let second_payload = axum::body::to_bytes(second_response.into_body(), 8192).await?;

    cleanup_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &first_order_id,
    )
    .await?;

    assert_eq!(
        second_status,
        StatusCode::CONFLICT,
        "payload: {}",
        String::from_utf8_lossy(&second_payload)
    );
    Ok(())
}

#[tokio::test]
async fn spot_create_order_concurrent_idempotency_key_freezes_once() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "CB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "CQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("spot-route-{}", Uuid::now_v7().simple());
    let hub = EventBroadcastHub::new(16);
    let _keepalive_hub = hub.clone();
    let mut private_events = hub.subscribe(&WebSocketChannel::private_user(user_id));
    let first_app = routes().with_state(
        AppState::new(settings.clone())
            .with_mysql(pool.clone())
            .with_event_broadcast_hub(hub.clone()),
    );
    let second_app = routes().with_state(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_event_broadcast_hub(hub),
    );
    let request_body = Arc::new(format!(
        r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"limit","price":"10.000000000000000000","quantity":"2.0000","idempotency_key":"{idempotency_key}"}}"#
    ));
    let first_token = token.clone();
    let second_token = token.clone();
    let first_body = Arc::clone(&request_body);
    let second_body = Arc::clone(&request_body);
    let first_task = tokio::spawn(async move {
        first_app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/spot/orders")
                    .header("authorization", format!("Bearer {first_token}"))
                    .header("content-type", "application/json")
                    .body(Body::from((*first_body).clone()))
                    .unwrap(),
            )
            .await
            .unwrap()
    });
    let second_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(1)).await;
        second_app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/spot/orders")
                    .header("authorization", format!("Bearer {second_token}"))
                    .header("content-type", "application/json")
                    .body(Body::from((*second_body).clone()))
                    .unwrap(),
            )
            .await
            .unwrap()
    });
    let (first_response, second_response) = tokio::join!(first_task, second_task);
    let first_response = first_response.unwrap();
    let second_response = second_response.unwrap();
    let first_status = first_response.status();
    let second_status = second_response.status();
    let first_payload = axum::body::to_bytes(first_response.into_body(), 8192).await?;
    let second_payload = axum::body::to_bytes(second_response.into_body(), 8192).await?;
    let (order_id,): (String,) = sqlx::query_as(
        "SELECT CAST(id AS CHAR) FROM spot_orders WHERE user_id = ? AND idempotency_key = ?",
    )
    .bind(user_id)
    .bind(&idempotency_key)
    .fetch_one(&pool)
    .await?;
    let (order_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM spot_orders WHERE user_id = ? AND idempotency_key = ?",
    )
    .bind(user_id)
    .bind(&idempotency_key)
    .fetch_one(&pool)
    .await?;
    let (available, frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'spot_order' AND ref_id = ?",
    )
    .bind(&order_id)
    .fetch_one(&pool)
    .await?;
    cleanup_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &order_id,
    )
    .await?;

    assert_eq!(
        first_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&first_payload)
    );
    assert_eq!(
        second_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&second_payload)
    );
    let first_order: Value = serde_json::from_slice(&first_payload)?;
    let second_order: Value = serde_json::from_slice(&second_payload)?;
    assert_eq!(first_order["id"], order_id);
    assert_eq!(second_order["id"], order_id);
    let event: Value = serde_json::from_str(private_events.recv().await?.payload())?;
    assert_eq!(event["type"], "spot.order.created");
    assert_eq!(event["order_id"], order_id);
    assert!(
        tokio::time::timeout(Duration::from_millis(25), private_events.recv())
            .await
            .is_err(),
        "idempotent replay must not publish a duplicate created event"
    );
    assert_eq!(order_count, 1);
    assert_eq!(
        available.normalized(),
        decimal("80.000000000000000000").normalized()
    );
    assert_eq!(
        frozen.normalized(),
        decimal("20.000000000000000000").normalized()
    );
    assert_eq!(ledger_count, 2);
    Ok(())
}

#[tokio::test]
async fn spot_create_order_idempotency_retry_skips_current_pair_rule_validation()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "VB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "VQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("spot-route-{}", Uuid::now_v7().simple());
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let request_body = format!(
        r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"limit","price":"10.000000000000000000","quantity":"2.0000","idempotency_key":"{idempotency_key}"}}"#
    );

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let first_status = first_response.status();
    let first_body = axum::body::to_bytes(first_response.into_body(), 8192).await?;
    assert_eq!(
        first_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&first_body)
    );
    let first_order: Value = serde_json::from_slice(&first_body)?;
    let first_order_id = first_order["id"].as_str().unwrap().to_owned();

    sqlx::query(
        "UPDATE trading_pairs SET status = 'disabled', min_order_value = ? WHERE symbol = ?",
    )
    .bind(decimal("1000.000000000000000000"))
    .bind(&pair_symbol)
    .execute(&pool)
    .await?;

    let second_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_status = second_response.status();
    let second_body = axum::body::to_bytes(second_response.into_body(), 8192).await?;
    assert_eq!(
        second_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&second_body)
    );
    let second_order: Value = serde_json::from_slice(&second_body)?;
    assert_eq!(second_order["id"], first_order_id);

    cleanup_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &first_order_id,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn spot_create_order_idempotency_retry_does_not_infer_new_order_from_missing_ledger()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "MB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "MQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("spot-route-{}", Uuid::now_v7().simple());
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let request_body = format!(
        r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"limit","price":"10.000000000000000000","quantity":"2.0000","idempotency_key":"{idempotency_key}"}}"#
    );

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let first_status = first_response.status();
    let first_body = axum::body::to_bytes(first_response.into_body(), 8192).await?;
    assert_eq!(
        first_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&first_body)
    );
    let first_order: Value = serde_json::from_slice(&first_body)?;
    let first_order_id = first_order["id"].as_str().unwrap().to_owned();

    sqlx::query("DELETE FROM wallet_ledger WHERE ref_type = 'spot_order' AND ref_id = ?")
        .bind(&first_order_id)
        .execute(&pool)
        .await?;

    let second_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_status = second_response.status();
    let second_body = axum::body::to_bytes(second_response.into_body(), 8192).await?;
    assert_eq!(
        second_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&second_body)
    );
    let second_order: Value = serde_json::from_slice(&second_body)?;
    assert_eq!(second_order["id"], first_order_id);

    let (available, frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        available.normalized(),
        decimal("80.000000000000000000").normalized()
    );
    assert_eq!(
        frozen.normalized(),
        decimal("20.000000000000000000").normalized()
    );
    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'spot_order' AND ref_id = ?",
    )
    .bind(&first_order_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 0);

    cleanup_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &first_order_id,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn spot_create_order_idempotency_key_is_scoped_to_same_user() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let first_user_id = create_user(&pool).await;
    let second_user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "XB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "XQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(first_user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(second_user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    let first_token = issue_token(
        &settings,
        format!("user:{first_user_id}"),
        TokenScope::User,
        900,
    )
    .unwrap();
    let second_token = issue_token(
        &settings,
        format!("user:{second_user_id}"),
        TokenScope::User,
        900,
    )
    .unwrap();
    let idempotency_key = format!("spot-route-{}", Uuid::now_v7().simple());
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let request_body = format!(
        r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"limit","price":"10.000000000000000000","quantity":"2.0000","idempotency_key":"{idempotency_key}"}}"#
    );

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {first_token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let first_status = first_response.status();
    let first_body = axum::body::to_bytes(first_response.into_body(), 8192).await?;
    assert_eq!(
        first_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&first_body)
    );
    let first_order: Value = serde_json::from_slice(&first_body)?;
    let first_order_id = first_order["id"].as_str().unwrap().to_owned();

    let second_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {second_token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_status = second_response.status();
    let second_body = axum::body::to_bytes(second_response.into_body(), 8192).await?;
    assert_eq!(
        second_status,
        StatusCode::CONFLICT,
        "payload: {}",
        String::from_utf8_lossy(&second_body)
    );

    let (first_available, first_frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(first_user_id)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        first_available.normalized(),
        decimal("80.000000000000000000").normalized()
    );
    assert_eq!(
        first_frozen.normalized(),
        decimal("20.000000000000000000").normalized()
    );
    let (second_available, second_frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(second_user_id)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        second_available.normalized(),
        decimal("100.000000000000000000").normalized()
    );
    assert_eq!(
        second_frozen.normalized(),
        decimal("0.000000000000000000").normalized()
    );

    let (order_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM spot_orders WHERE idempotency_key = ?")
            .bind(&idempotency_key)
            .fetch_one(&pool)
            .await?;
    assert_eq!(order_count, 1);

    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
        .bind(second_user_id)
        .bind(quote_asset)
        .execute(&pool)
        .await?;
    cleanup_fixture(
        &pool,
        first_user_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &first_order_id,
    )
    .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(second_user_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn spot_create_order_rolls_back_when_wallet_freeze_fails() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "RB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "RQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("spot-route-{}", Uuid::now_v7().simple());
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"limit","price":"10.000000000000000000","quantity":"2.0000","idempotency_key":"{idempotency_key}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 8192).await?;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );

    let (order_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM spot_orders WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(order_count, 0);

    cleanup_fixture(&pool, user_id, base_asset, quote_asset, &pair_symbol, "0").await?;
    Ok(())
}

#[tokio::test]
async fn spot_cancel_is_idempotent_without_repeating_unfreeze() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "CB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "CQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    let order_id = seed_open_buy_order(&pool, user_id, &pair_symbol).await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(quote_asset)
    .bind(decimal("80.000000000000000000"))
    .bind(decimal("20.000000000000000000"))
    .execute(&pool)
    .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let hub = EventBroadcastHub::new(16);
    let _keepalive_hub = hub.clone();
    let mut private_events = hub.subscribe(&WebSocketChannel::private_user(user_id));
    let app = routes().with_state(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_event_broadcast_hub(hub),
    );

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/spot/orders/{order_id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let first_status = first_response.status();
    let first_body = axum::body::to_bytes(first_response.into_body(), 8192).await?;
    assert_eq!(
        first_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&first_body)
    );
    let first_payload: Value = serde_json::from_slice(&first_body)?;
    assert_eq!(first_payload["cancelled"], true);

    let second_response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/spot/orders/{order_id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let second_status = second_response.status();
    let second_body = axum::body::to_bytes(second_response.into_body(), 8192).await?;
    assert_eq!(
        second_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&second_body)
    );
    let second_payload: Value = serde_json::from_slice(&second_body)?;
    assert_eq!(second_payload["cancelled"], false);
    assert_eq!(second_payload["order"]["status"], "cancelled");
    let event_message =
        tokio::time::timeout(Duration::from_millis(100), private_events.recv()).await??;
    let event: Value = serde_json::from_str(event_message.payload())?;
    assert_eq!(event["type"], "spot.order.cancelled");
    assert_eq!(event["order_id"], order_id);
    assert_eq!(event["status"], "cancelled");
    assert!(
        tokio::time::timeout(Duration::from_millis(25), private_events.recv())
            .await
            .is_err(),
        "idempotent cancel replay must not publish a duplicate cancelled event"
    );

    let (available, frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        available.normalized(),
        decimal("100.000000000000000000").normalized()
    );
    assert_eq!(
        frozen.normalized(),
        decimal("0.000000000000000000").normalized()
    );
    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'spot_order' AND ref_id = ?",
    )
    .bind(&order_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 2);

    cleanup_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &order_id,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn spot_fill_settles_buyer_and_seller_wallets() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let buyer_id = create_user(&pool).await;
    let seller_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "FB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "FQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    let buy_order_id = seed_open_order(
        &pool,
        buyer_id,
        &pair_symbol,
        "buy",
        "10.000000000000000000",
        "2.0000",
    )
    .await?;
    let sell_order_id = seed_open_order(
        &pool,
        seller_id,
        &pair_symbol,
        "sell",
        "10.000000000000000000",
        "2.0000",
    )
    .await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(buyer_id)
    .bind(quote_asset)
    .bind(decimal("80.000000000000000000"))
    .bind(decimal("20.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(buyer_id)
        .bind(base_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(seller_id)
    .bind(base_asset)
    .bind(decimal("0.000000000000000000"))
    .bind(decimal("2.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(seller_id)
        .bind(quote_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/fills")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"buy_order_id":"{buy_order_id}","sell_order_id":"{sell_order_id}","price":"10.000000000000000000","quantity":"2.000000000000000000","idempotency_key":"spot-fill-{buy_order_id}-{sell_order_id}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 8192).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );
    let payload: Value = serde_json::from_slice(&body)?;
    assert_eq!(payload["trade"]["buy_order_id"], buy_order_id);
    assert_eq!(payload["trade"]["sell_order_id"], sell_order_id);
    assert_eq!(payload["buy_order"]["status"], "filled");
    assert_eq!(payload["sell_order"]["status"], "filled");

    let (buyer_quote_available, buyer_quote_frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(buyer_id)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        buyer_quote_available.normalized(),
        decimal("80.000000000000000000").normalized()
    );
    assert_eq!(
        buyer_quote_frozen.normalized(),
        decimal("0.000000000000000000").normalized()
    );

    let (buyer_base_available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(buyer_id)
            .bind(base_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        buyer_base_available.normalized(),
        decimal("2.000000000000000000").normalized()
    );

    let (seller_base_available, seller_base_frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(seller_id)
    .bind(base_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        seller_base_available.normalized(),
        decimal("0.000000000000000000").normalized()
    );
    assert_eq!(
        seller_base_frozen.normalized(),
        decimal("0.000000000000000000").normalized()
    );

    let (seller_quote_available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(seller_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        seller_quote_available.normalized(),
        decimal("20.000000000000000000").normalized()
    );

    let (trade_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM spot_trades WHERE buy_order_id = ? AND sell_order_id = ?",
    )
    .bind(&buy_order_id)
    .bind(&sell_order_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(trade_count, 1);

    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'spot_trade' AND ref_id = ?",
    )
    .bind(format!("{buy_order_id}:{sell_order_id}"))
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 4);

    cleanup_fill_fixture(
        &pool,
        buyer_id,
        seller_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &buy_order_id,
        &sell_order_id,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn spot_fill_is_idempotent_for_repeated_request_key() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let buyer_id = create_user(&pool).await;
    let seller_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "IB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "IQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    let buy_order_id = seed_open_order(
        &pool,
        buyer_id,
        &pair_symbol,
        "buy",
        "10.000000000000000000",
        "2.0000",
    )
    .await?;
    let sell_order_id = seed_open_order(
        &pool,
        seller_id,
        &pair_symbol,
        "sell",
        "10.000000000000000000",
        "2.0000",
    )
    .await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(buyer_id)
    .bind(quote_asset)
    .bind(decimal("80.000000000000000000"))
    .bind(decimal("20.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(buyer_id)
        .bind(base_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(seller_id)
    .bind(base_asset)
    .bind(decimal("0.000000000000000000"))
    .bind(decimal("2.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(seller_id)
        .bind(quote_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let hub = EventBroadcastHub::new(16);
    let _keepalive_hub = hub.clone();
    let mut buyer_events = hub.subscribe(&WebSocketChannel::private_user(buyer_id));
    let mut seller_events = hub.subscribe(&WebSocketChannel::private_user(seller_id));
    let app = admin_routes().with_state(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_event_broadcast_hub(hub),
    );
    let idempotency_key = format!("spot-fill-{}", Uuid::now_v7().simple());
    let request_body = format!(
        r#"{{"buy_order_id":"{buy_order_id}","sell_order_id":"{sell_order_id}","price":"10.000000000000000000","quantity":"2.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/fills")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let first_status = first_response.status();
    let first_body = axum::body::to_bytes(first_response.into_body(), 8192).await?;
    assert_eq!(
        first_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&first_body)
    );
    let first_payload: Value = serde_json::from_slice(&first_body)?;
    let first_trade_id = first_payload["trade"]["id"].as_str().unwrap().to_owned();
    let buyer_event_message =
        tokio::time::timeout(Duration::from_millis(100), buyer_events.recv()).await??;
    let seller_event_message =
        tokio::time::timeout(Duration::from_millis(100), seller_events.recv()).await??;
    let buyer_event: Value = serde_json::from_str(buyer_event_message.payload())?;
    let seller_event: Value = serde_json::from_str(seller_event_message.payload())?;
    assert_eq!(buyer_event["type"], "spot.trade.filled");
    assert_eq!(buyer_event["trade_id"], first_trade_id);
    assert_eq!(buyer_event["order_id"], buy_order_id);
    assert_eq!(buyer_event["counterparty_order_id"], sell_order_id);
    assert_eq!(buyer_event["pair_id"], pair_symbol);
    assert_eq!(buyer_event["side"], "buy");
    assert_eq!(buyer_event["price"], "10.000000000000000000");
    assert_eq!(buyer_event["quantity"], "2.000000000000000000");
    assert_eq!(buyer_event["order_status"], "filled");
    assert_eq!(seller_event["type"], "spot.trade.filled");
    assert_eq!(seller_event["trade_id"], first_trade_id);
    assert_eq!(seller_event["order_id"], sell_order_id);
    assert_eq!(seller_event["counterparty_order_id"], buy_order_id);
    assert_eq!(seller_event["pair_id"], pair_symbol);
    assert_eq!(seller_event["side"], "sell");
    assert_eq!(seller_event["price"], "10.000000000000000000");
    assert_eq!(seller_event["quantity"], "2.000000000000000000");
    assert_eq!(seller_event["order_status"], "filled");

    let second_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/fills")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_status = second_response.status();
    let second_body = axum::body::to_bytes(second_response.into_body(), 8192).await?;
    assert_eq!(
        second_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&second_body)
    );
    let second_payload: Value = serde_json::from_slice(&second_body)?;
    assert_eq!(second_payload["trade"]["id"], first_trade_id);
    assert_eq!(second_payload["buy_order"]["status"], "filled");
    assert_eq!(second_payload["sell_order"]["status"], "filled");
    assert!(
        tokio::time::timeout(Duration::from_millis(25), buyer_events.recv())
            .await
            .is_err(),
        "idempotent fill replay must not publish duplicate buyer event"
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(25), seller_events.recv())
            .await
            .is_err(),
        "idempotent fill replay must not publish duplicate seller event"
    );

    let (trade_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM spot_trades WHERE buy_order_id = ? AND sell_order_id = ?",
    )
    .bind(&buy_order_id)
    .bind(&sell_order_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(trade_count, 1);

    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'spot_trade' AND ref_id = ?",
    )
    .bind(format!("{buy_order_id}:{sell_order_id}"))
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 4);

    let (buyer_quote_available, buyer_quote_frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(buyer_id)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        buyer_quote_available.normalized(),
        decimal("80.000000000000000000").normalized()
    );
    assert_eq!(
        buyer_quote_frozen.normalized(),
        decimal("0.000000000000000000").normalized()
    );

    let (seller_quote_available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(seller_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        seller_quote_available.normalized(),
        decimal("20.000000000000000000").normalized()
    );

    cleanup_fill_fixture(
        &pool,
        buyer_id,
        seller_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &buy_order_id,
        &sell_order_id,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn spot_fill_replays_leading_zero_order_ids_idempotently() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let buyer_id = create_user(&pool).await;
    let seller_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "IZ").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "JZ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    let buy_order_id = seed_open_order(
        &pool,
        buyer_id,
        &pair_symbol,
        "buy",
        "10.000000000000000000",
        "2.0000",
    )
    .await?;
    let sell_order_id = seed_open_order(
        &pool,
        seller_id,
        &pair_symbol,
        "sell",
        "10.000000000000000000",
        "2.0000",
    )
    .await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(buyer_id)
    .bind(quote_asset)
    .bind(decimal("80.000000000000000000"))
    .bind(decimal("20.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(buyer_id)
        .bind(base_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(seller_id)
    .bind(base_asset)
    .bind(decimal("0.000000000000000000"))
    .bind(decimal("2.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(seller_id)
        .bind(quote_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let idempotency_key = format!("spot-fill-zero-{}", Uuid::now_v7().simple());
    let padded_buy_order_id = format!("{:0>12}", buy_order_id.parse::<u64>()?);
    let padded_sell_order_id = format!("{:0>12}", sell_order_id.parse::<u64>()?);
    let request_body = format!(
        r#"{{"buy_order_id":"{padded_buy_order_id}","sell_order_id":"{padded_sell_order_id}","price":"10.000000000000000000","quantity":"2.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/fills")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let first_status = first_response.status();
    let first_body = axum::body::to_bytes(first_response.into_body(), 8192).await?;
    assert_eq!(
        first_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&first_body)
    );
    let first_payload: Value = serde_json::from_slice(&first_body)?;
    let first_trade_id = first_payload["trade"]["id"].as_str().unwrap().to_owned();

    let second_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/fills")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_status = second_response.status();
    let second_body = axum::body::to_bytes(second_response.into_body(), 8192).await?;
    assert_eq!(
        second_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&second_body)
    );
    let second_payload: Value = serde_json::from_slice(&second_body)?;
    assert_eq!(second_payload["trade"]["id"], first_trade_id);

    let (trade_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM spot_trades WHERE idempotency_key = ?")
            .bind(&idempotency_key)
            .fetch_one(&pool)
            .await?;
    assert_eq!(trade_count, 1);

    cleanup_fill_fixture(
        &pool,
        buyer_id,
        seller_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &buy_order_id,
        &sell_order_id,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn spot_fill_allows_multiple_partial_fills_for_same_order_pair() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let buyer_id = create_user(&pool).await;
    let seller_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "MB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "MQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    let buy_order_id = seed_open_order(
        &pool,
        buyer_id,
        &pair_symbol,
        "buy",
        "10.000000000000000000",
        "2.0000",
    )
    .await?;
    let sell_order_id = seed_open_order(
        &pool,
        seller_id,
        &pair_symbol,
        "sell",
        "10.000000000000000000",
        "2.0000",
    )
    .await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(buyer_id)
    .bind(quote_asset)
    .bind(decimal("80.000000000000000000"))
    .bind(decimal("20.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(buyer_id)
        .bind(base_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(seller_id)
    .bind(base_asset)
    .bind(decimal("0.000000000000000000"))
    .bind(decimal("2.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(seller_id)
        .bind(quote_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let first_key = format!("spot-fill-{}", Uuid::now_v7().simple());
    let second_key = format!("spot-fill-{}", Uuid::now_v7().simple());

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/fills")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"buy_order_id":"{buy_order_id}","sell_order_id":"{sell_order_id}","price":"10.000000000000000000","quantity":"1.000000000000000000","idempotency_key":"{first_key}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let first_status = first_response.status();
    let first_body = axum::body::to_bytes(first_response.into_body(), 8192).await?;
    assert_eq!(
        first_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&first_body)
    );
    let first_payload: Value = serde_json::from_slice(&first_body)?;
    assert_eq!(first_payload["buy_order"]["status"], "partially_filled");
    assert_eq!(first_payload["sell_order"]["status"], "partially_filled");

    let second_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/fills")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"buy_order_id":"{buy_order_id}","sell_order_id":"{sell_order_id}","price":"10.000000000000000000","quantity":"1.000000000000000000","idempotency_key":"{second_key}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_status = second_response.status();
    let second_body = axum::body::to_bytes(second_response.into_body(), 8192).await?;
    assert_eq!(
        second_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&second_body)
    );
    let second_payload: Value = serde_json::from_slice(&second_body)?;
    assert_ne!(second_payload["trade"]["id"], first_payload["trade"]["id"]);
    assert_eq!(second_payload["buy_order"]["status"], "filled");
    assert_eq!(second_payload["sell_order"]["status"], "filled");

    let (trade_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM spot_trades WHERE buy_order_id = ? AND sell_order_id = ?",
    )
    .bind(&buy_order_id)
    .bind(&sell_order_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(trade_count, 2);

    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'spot_trade' AND ref_id = ?",
    )
    .bind(format!("{buy_order_id}:{sell_order_id}"))
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 8);

    let (buyer_quote_available, buyer_quote_frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(buyer_id)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        buyer_quote_available.normalized(),
        decimal("80.000000000000000000").normalized()
    );
    assert_eq!(
        buyer_quote_frozen.normalized(),
        decimal("0.000000000000000000").normalized()
    );

    let (seller_quote_available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(seller_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        seller_quote_available.normalized(),
        decimal("20.000000000000000000").normalized()
    );

    cleanup_fill_fixture(
        &pool,
        buyer_id,
        seller_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &buy_order_id,
        &sell_order_id,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn spot_fill_rejects_user_scope_tokens() -> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let app = admin_routes().with_state(AppState::new(settings));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/fills")
                .header("authorization", format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"buy_order_id":"1","sell_order_id":"2","price":"9.000000000000000000","quantity":"2.000000000000000000"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    Ok(())
}

#[tokio::test]
async fn spot_fill_releases_limit_buy_price_improvement_reserve() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let buyer_id = create_user(&pool).await;
    let seller_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "PB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "PQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    let buy_order_id = seed_open_order(
        &pool,
        buyer_id,
        &pair_symbol,
        "buy",
        "10.000000000000000000",
        "2.0000",
    )
    .await?;
    let sell_order_id = seed_open_order(
        &pool,
        seller_id,
        &pair_symbol,
        "sell",
        "9.000000000000000000",
        "2.0000",
    )
    .await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(buyer_id)
    .bind(quote_asset)
    .bind(decimal("80.000000000000000000"))
    .bind(decimal("20.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(buyer_id)
        .bind(base_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(seller_id)
    .bind(base_asset)
    .bind(decimal("0.000000000000000000"))
    .bind(decimal("2.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(seller_id)
        .bind(quote_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/fills")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"buy_order_id":"{buy_order_id}","sell_order_id":"{sell_order_id}","price":"9.000000000000000000","quantity":"2.000000000000000000","idempotency_key":"spot-fill-{buy_order_id}-{sell_order_id}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 8192).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );

    let (buyer_quote_available, buyer_quote_frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(buyer_id)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        buyer_quote_available.normalized(),
        decimal("82.000000000000000000").normalized()
    );
    assert_eq!(
        buyer_quote_frozen.normalized(),
        decimal("0.000000000000000000").normalized()
    );

    let (seller_quote_available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(seller_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        seller_quote_available.normalized(),
        decimal("18.000000000000000000").normalized()
    );

    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'spot_trade' AND ref_id = ?",
    )
    .bind(format!("{buy_order_id}:{sell_order_id}"))
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 6);

    cleanup_fill_fixture(
        &pool,
        buyer_id,
        seller_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &buy_order_id,
        &sell_order_id,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn spot_fill_rejects_price_below_sell_limit() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let buyer_id = create_user(&pool).await;
    let seller_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "LB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "LQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    let buy_order_id = seed_open_order(
        &pool,
        buyer_id,
        &pair_symbol,
        "buy",
        "10.000000000000000000",
        "2.0000",
    )
    .await?;
    let sell_order_id = seed_open_order(
        &pool,
        seller_id,
        &pair_symbol,
        "sell",
        "9.000000000000000000",
        "2.0000",
    )
    .await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(buyer_id)
    .bind(quote_asset)
    .bind(decimal("80.000000000000000000"))
    .bind(decimal("20.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(buyer_id)
        .bind(base_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(seller_id)
    .bind(base_asset)
    .bind(decimal("0.000000000000000000"))
    .bind(decimal("2.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(seller_id)
        .bind(quote_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/fills")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"buy_order_id":"{buy_order_id}","sell_order_id":"{sell_order_id}","price":"8.000000000000000000","quantity":"2.000000000000000000","idempotency_key":"spot-fill-{buy_order_id}-{sell_order_id}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 8192).await?;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );

    let (trade_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM spot_trades WHERE buy_order_id = ? AND sell_order_id = ?",
    )
    .bind(&buy_order_id)
    .bind(&sell_order_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(trade_count, 0);

    cleanup_fill_fixture(
        &pool,
        buyer_id,
        seller_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &buy_order_id,
        &sell_order_id,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn spot_fill_rejects_price_above_buy_limit() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let buyer_id = create_user(&pool).await;
    let seller_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "HB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "HQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    let buy_order_id = seed_open_order(
        &pool,
        buyer_id,
        &pair_symbol,
        "buy",
        "10.000000000000000000",
        "2.0000",
    )
    .await?;
    let sell_order_id = seed_open_order(
        &pool,
        seller_id,
        &pair_symbol,
        "sell",
        "9.000000000000000000",
        "2.0000",
    )
    .await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(buyer_id)
    .bind(quote_asset)
    .bind(decimal("78.000000000000000000"))
    .bind(decimal("22.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(buyer_id)
        .bind(base_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(seller_id)
    .bind(base_asset)
    .bind(decimal("0.000000000000000000"))
    .bind(decimal("2.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(seller_id)
        .bind(quote_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/fills")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"buy_order_id":"{buy_order_id}","sell_order_id":"{sell_order_id}","price":"11.000000000000000000","quantity":"2.000000000000000000","idempotency_key":"spot-fill-{buy_order_id}-{sell_order_id}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 8192).await?;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );

    let (trade_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM spot_trades WHERE buy_order_id = ? AND sell_order_id = ?",
    )
    .bind(&buy_order_id)
    .bind(&sell_order_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(trade_count, 0);

    cleanup_fill_fixture(
        &pool,
        buyer_id,
        seller_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &buy_order_id,
        &sell_order_id,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn spot_cancel_one_of_two_reserved_orders_unfreezes_only_that_order()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "TB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "TQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let first_key = format!("spot-route-{}", Uuid::now_v7().simple());
    let second_key = format!("spot-route-{}", Uuid::now_v7().simple());

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"limit","price":"10.000000000000000000","quantity":"2.0000","idempotency_key":"{first_key}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let first_status = first_response.status();
    let first_body = axum::body::to_bytes(first_response.into_body(), 8192).await?;
    assert_eq!(
        first_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&first_body)
    );
    let first_order: Value = serde_json::from_slice(&first_body)?;
    let first_order_id = first_order["id"].as_str().unwrap().to_owned();

    let second_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"limit","price":"10.000000000000000000","quantity":"2.0000","idempotency_key":"{second_key}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_status = second_response.status();
    let second_body = axum::body::to_bytes(second_response.into_body(), 8192).await?;
    assert_eq!(
        second_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&second_body)
    );
    let second_order: Value = serde_json::from_slice(&second_body)?;
    let second_order_id = second_order["id"].as_str().unwrap().to_owned();

    let cancel_response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/spot/orders/{first_order_id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let cancel_status = cancel_response.status();
    let cancel_body = axum::body::to_bytes(cancel_response.into_body(), 8192).await?;
    assert_eq!(
        cancel_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&cancel_body)
    );

    let (available, frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        available.normalized(),
        decimal("80.000000000000000000").normalized()
    );
    assert_eq!(
        frozen.normalized(),
        decimal("20.000000000000000000").normalized()
    );

    sqlx::query("DELETE FROM wallet_ledger WHERE ref_type = 'spot_order' AND ref_id IN (?, ?)")
        .bind(&first_order_id)
        .bind(&second_order_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM spot_orders WHERE id IN (?, ?)")
        .bind(&first_order_id)
        .bind(&second_order_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ? AND asset_id IN (?, ?)")
        .bind(user_id)
        .bind(base_asset)
        .bind(quote_asset)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM trading_pairs WHERE symbol = ?")
        .bind(&pair_symbol)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id IN (?, ?)")
        .bind(base_asset)
        .bind(quote_asset)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn spot_cancel_market_buy_order_unfreezes_reference_price_reserve()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "MB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "MQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("spot-route-{}", Uuid::now_v7().simple());
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"market","quantity":"2.0000","reference_price":"10.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let create_status = create_response.status();
    let create_body = axum::body::to_bytes(create_response.into_body(), 8192).await?;
    assert_eq!(
        create_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&create_body)
    );
    let created: Value = serde_json::from_slice(&create_body)?;
    let order_id = created["id"].as_str().unwrap().to_owned();

    let cancel_response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/spot/orders/{order_id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let cancel_status = cancel_response.status();
    let cancel_body = axum::body::to_bytes(cancel_response.into_body(), 8192).await?;
    assert_eq!(
        cancel_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&cancel_body)
    );
    let cancelled: Value = serde_json::from_slice(&cancel_body)?;
    assert_eq!(cancelled["cancelled"], true);
    assert_eq!(cancelled["order"]["status"], "cancelled");

    let (available, frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        available.normalized(),
        decimal("100.000000000000000000").normalized()
    );
    assert_eq!(
        frozen.normalized(),
        decimal("0.000000000000000000").normalized()
    );
    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'spot_order' AND ref_id = ?",
    )
    .bind(&order_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 4);

    cleanup_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &order_id,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn spot_cancel_market_buy_after_below_reference_partial_fill_unfreezes_all_remaining_quote()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let buyer_id = create_user(&pool).await;
    let seller_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "MR").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "MQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(buyer_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(buyer_id)
        .bind(base_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(seller_id)
    .bind(base_asset)
    .bind(decimal("1.000000000000000000"))
    .bind(decimal("1.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(seller_id)
        .bind(quote_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    let user_token =
        issue_token(&settings, format!("user:{buyer_id}"), TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let user_app = routes().with_state(AppState::new(settings.clone()).with_mysql(pool.clone()));
    let admin_app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let idempotency_key = format!("spot-route-{}", Uuid::now_v7().simple());

    let create_response = user_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"market","quantity":"2.0000","reference_price":"10.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let create_status = create_response.status();
    let create_body = axum::body::to_bytes(create_response.into_body(), 8192).await?;
    assert_eq!(
        create_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&create_body)
    );
    let created: Value = serde_json::from_slice(&create_body)?;
    let buy_order_id = created["id"].as_str().unwrap().to_owned();
    sqlx::query("UPDATE spot_orders SET status = 'open' WHERE id = ?")
        .bind(&buy_order_id)
        .execute(&pool)
        .await?;
    let sell_order_id = seed_open_order(
        &pool,
        seller_id,
        &pair_symbol,
        "sell",
        "8.000000000000000000",
        "2.0000",
    )
    .await?;

    let fill_response = admin_app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/fills")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"buy_order_id":"{buy_order_id}","sell_order_id":"{sell_order_id}","price":"8.000000000000000000","quantity":"1.000000000000000000","idempotency_key":"spot-fill-{buy_order_id}-{sell_order_id}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let fill_status = fill_response.status();
    let fill_body = axum::body::to_bytes(fill_response.into_body(), 8192).await?;
    assert_eq!(
        fill_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&fill_body)
    );

    let cancel_response = user_app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/spot/orders/{buy_order_id}"))
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let cancel_status = cancel_response.status();
    let cancel_body = axum::body::to_bytes(cancel_response.into_body(), 8192).await?;
    assert_eq!(
        cancel_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&cancel_body)
    );
    let cancelled: Value = serde_json::from_slice(&cancel_body)?;
    assert_eq!(cancelled["cancelled"], true);
    assert_eq!(cancelled["order"]["status"], "cancelled");

    let (available, frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(buyer_id)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        available.normalized(),
        decimal("92.000000000000000000").normalized()
    );
    assert_eq!(
        frozen.normalized(),
        decimal("0.000000000000000000").normalized()
    );

    cleanup_fill_fixture(
        &pool,
        buyer_id,
        seller_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &buy_order_id,
        &sell_order_id,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn spot_cancel_market_buy_after_above_reference_partial_fill_unfreezes_remaining_quote()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let buyer_id = create_user(&pool).await;
    let seller_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "HR").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "HQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(buyer_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(buyer_id)
        .bind(base_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(seller_id)
    .bind(base_asset)
    .bind(decimal("1.000000000000000000"))
    .bind(decimal("1.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(seller_id)
        .bind(quote_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    let user_token =
        issue_token(&settings, format!("user:{buyer_id}"), TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let user_app = routes().with_state(AppState::new(settings.clone()).with_mysql(pool.clone()));
    let admin_app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let idempotency_key = format!("spot-route-{}", Uuid::now_v7().simple());

    let create_response = user_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"market","quantity":"2.0000","reference_price":"10.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let create_status = create_response.status();
    let create_body = axum::body::to_bytes(create_response.into_body(), 8192).await?;
    assert_eq!(
        create_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&create_body)
    );
    let created: Value = serde_json::from_slice(&create_body)?;
    let buy_order_id = created["id"].as_str().unwrap().to_owned();
    sqlx::query("UPDATE spot_orders SET status = 'open' WHERE id = ?")
        .bind(&buy_order_id)
        .execute(&pool)
        .await?;
    let sell_order_id = seed_open_order(
        &pool,
        seller_id,
        &pair_symbol,
        "sell",
        "12.000000000000000000",
        "2.0000",
    )
    .await?;

    let fill_response = admin_app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/fills")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"buy_order_id":"{buy_order_id}","sell_order_id":"{sell_order_id}","price":"12.000000000000000000","quantity":"1.000000000000000000","idempotency_key":"spot-fill-{buy_order_id}-{sell_order_id}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let fill_status = fill_response.status();
    let fill_body = axum::body::to_bytes(fill_response.into_body(), 8192).await?;
    assert_eq!(
        fill_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&fill_body)
    );

    let cancel_response = user_app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/spot/orders/{buy_order_id}"))
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let cancel_status = cancel_response.status();
    let cancel_body = axum::body::to_bytes(cancel_response.into_body(), 8192).await?;
    assert_eq!(
        cancel_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&cancel_body)
    );
    let cancelled: Value = serde_json::from_slice(&cancel_body)?;
    assert_eq!(cancelled["cancelled"], true);
    assert_eq!(cancelled["order"]["status"], "cancelled");

    let (available, frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(buyer_id)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        available.normalized(),
        decimal("88.000000000000000000").normalized()
    );
    assert_eq!(
        frozen.normalized(),
        decimal("0.000000000000000000").normalized()
    );

    cleanup_fill_fixture(
        &pool,
        buyer_id,
        seller_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &buy_order_id,
        &sell_order_id,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn spot_fill_rejects_market_buy_that_exceeds_order_reservation() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let buyer_id = create_user(&pool).await;
    let seller_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "OR").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "OQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(buyer_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(buyer_id)
        .bind(base_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(seller_id)
    .bind(base_asset)
    .bind(decimal("0.000000000000000000"))
    .bind(decimal("2.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(seller_id)
        .bind(quote_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    let user_token =
        issue_token(&settings, format!("user:{buyer_id}"), TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let user_app = routes().with_state(AppState::new(settings.clone()).with_mysql(pool.clone()));
    let admin_app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let market_key = format!("spot-route-{}", Uuid::now_v7().simple());
    let other_key = format!("spot-route-{}", Uuid::now_v7().simple());

    let market_response = user_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"market","quantity":"2.0000","reference_price":"10.000000000000000000","idempotency_key":"{market_key}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let market_status = market_response.status();
    let market_body = axum::body::to_bytes(market_response.into_body(), 8192).await?;
    assert_eq!(
        market_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&market_body)
    );
    let market_order: Value = serde_json::from_slice(&market_body)?;
    let market_order_id = market_order["id"].as_str().unwrap().to_owned();
    sqlx::query("UPDATE spot_orders SET status = 'open' WHERE id = ?")
        .bind(&market_order_id)
        .execute(&pool)
        .await?;

    let other_response = user_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"limit","price":"10.000000000000000000","quantity":"2.0000","idempotency_key":"{other_key}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let other_status = other_response.status();
    let other_body = axum::body::to_bytes(other_response.into_body(), 8192).await?;
    assert_eq!(
        other_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&other_body)
    );
    let other_order: Value = serde_json::from_slice(&other_body)?;
    let other_order_id = other_order["id"].as_str().unwrap().to_owned();
    let sell_order_id = seed_open_order(
        &pool,
        seller_id,
        &pair_symbol,
        "sell",
        "15.000000000000000000",
        "2.0000",
    )
    .await?;

    let fill_response = admin_app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/fills")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"buy_order_id":"{market_order_id}","sell_order_id":"{sell_order_id}","price":"15.000000000000000000","quantity":"2.000000000000000000","idempotency_key":"spot-fill-{market_order_id}-{sell_order_id}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let fill_status = fill_response.status();
    let fill_body = axum::body::to_bytes(fill_response.into_body(), 8192).await?;
    assert_eq!(
        fill_status,
        StatusCode::BAD_REQUEST,
        "payload: {}",
        String::from_utf8_lossy(&fill_body)
    );
    let (trade_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM spot_trades WHERE buy_order_id = ?")
            .bind(&market_order_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(trade_count, 0);

    sqlx::query("DELETE FROM wallet_ledger WHERE ref_type = 'spot_order' AND ref_id = ?")
        .bind(&other_order_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM spot_orders WHERE id = ?")
        .bind(&other_order_id)
        .execute(&pool)
        .await?;
    cleanup_fill_fixture(
        &pool,
        buyer_id,
        seller_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &market_order_id,
        &sell_order_id,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn spot_fill_full_market_buy_below_reference_releases_surplus_quote()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let buyer_id = create_user(&pool).await;
    let seller_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "FR").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "FQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(buyer_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(buyer_id)
        .bind(base_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(seller_id)
    .bind(base_asset)
    .bind(decimal("0.000000000000000000"))
    .bind(decimal("2.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(seller_id)
        .bind(quote_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    let user_token =
        issue_token(&settings, format!("user:{buyer_id}"), TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let user_app = routes().with_state(AppState::new(settings.clone()).with_mysql(pool.clone()));
    let admin_app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let idempotency_key = format!("spot-route-{}", Uuid::now_v7().simple());

    let buy_response = user_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"market","quantity":"2.0000","reference_price":"10.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let buy_status = buy_response.status();
    let buy_body = axum::body::to_bytes(buy_response.into_body(), 8192).await?;
    assert_eq!(
        buy_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&buy_body)
    );
    let buy_order: Value = serde_json::from_slice(&buy_body)?;
    let buy_order_id = buy_order["id"].as_str().unwrap().to_owned();
    sqlx::query("UPDATE spot_orders SET status = 'open' WHERE id = ?")
        .bind(&buy_order_id)
        .execute(&pool)
        .await?;
    let sell_order_id = seed_open_order(
        &pool,
        seller_id,
        &pair_symbol,
        "sell",
        "8.000000000000000000",
        "2.0000",
    )
    .await?;

    let fill_response = admin_app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/fills")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"buy_order_id":"{buy_order_id}","sell_order_id":"{sell_order_id}","price":"8.000000000000000000","quantity":"2.000000000000000000","idempotency_key":"spot-fill-{buy_order_id}-{sell_order_id}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let fill_status = fill_response.status();
    let fill_body = axum::body::to_bytes(fill_response.into_body(), 8192).await?;
    assert_eq!(
        fill_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&fill_body)
    );

    let (buyer_quote_available, buyer_quote_frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(buyer_id)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        buyer_quote_available.normalized(),
        decimal("84.000000000000000000").normalized()
    );
    assert_eq!(
        buyer_quote_frozen.normalized(),
        decimal("0.000000000000000000").normalized()
    );

    cleanup_fill_fixture(
        &pool,
        buyer_id,
        seller_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &buy_order_id,
        &sell_order_id,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn spot_cancel_limit_buy_after_price_improvement_fill_unfreezes_remaining_quote()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let buyer_id = create_user(&pool).await;
    let seller_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "CR").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "CQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(buyer_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(buyer_id)
        .bind(base_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(seller_id)
    .bind(base_asset)
    .bind(decimal("1.000000000000000000"))
    .bind(decimal("1.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(seller_id)
        .bind(quote_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    let user_token =
        issue_token(&settings, format!("user:{buyer_id}"), TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let user_app = routes().with_state(AppState::new(settings.clone()).with_mysql(pool.clone()));
    let admin_app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let idempotency_key = format!("spot-route-{}", Uuid::now_v7().simple());

    let buy_response = user_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"pair_id":"{pair_symbol}","side":"buy","order_type":"limit","price":"10.000000000000000000","quantity":"2.0000","idempotency_key":"{idempotency_key}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let buy_status = buy_response.status();
    let buy_body = axum::body::to_bytes(buy_response.into_body(), 8192).await?;
    assert_eq!(
        buy_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&buy_body)
    );
    let buy_order: Value = serde_json::from_slice(&buy_body)?;
    let buy_order_id = buy_order["id"].as_str().unwrap().to_owned();
    sqlx::query("UPDATE spot_orders SET status = 'open' WHERE id = ?")
        .bind(&buy_order_id)
        .execute(&pool)
        .await?;
    let sell_order_id = seed_open_order(
        &pool,
        seller_id,
        &pair_symbol,
        "sell",
        "8.000000000000000000",
        "2.0000",
    )
    .await?;

    let fill_response = admin_app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/fills")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"buy_order_id":"{buy_order_id}","sell_order_id":"{sell_order_id}","price":"8.000000000000000000","quantity":"1.000000000000000000","idempotency_key":"spot-fill-{buy_order_id}-{sell_order_id}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let fill_status = fill_response.status();
    let fill_body = axum::body::to_bytes(fill_response.into_body(), 8192).await?;
    assert_eq!(
        fill_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&fill_body)
    );

    let cancel_response = user_app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/spot/orders/{buy_order_id}"))
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let cancel_status = cancel_response.status();
    let cancel_body = axum::body::to_bytes(cancel_response.into_body(), 8192).await?;
    assert_eq!(
        cancel_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&cancel_body)
    );

    let (buyer_quote_available, buyer_quote_frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(buyer_id)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        buyer_quote_available.normalized(),
        decimal("92.000000000000000000").normalized()
    );
    assert_eq!(
        buyer_quote_frozen.normalized(),
        decimal("0.000000000000000000").normalized()
    );

    cleanup_fill_fixture(
        &pool,
        buyer_id,
        seller_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &buy_order_id,
        &sell_order_id,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn spot_cancel_sell_after_partial_fill_unfreezes_only_remaining_base()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let buyer_id = create_user(&pool).await;
    let seller_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "SR").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "SQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(seller_id)
        .bind(base_asset)
        .bind(decimal("2.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(seller_id)
        .bind(quote_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(buyer_id)
    .bind(quote_asset)
    .bind(decimal("80.000000000000000000"))
    .bind(decimal("20.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(buyer_id)
        .bind(base_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    let seller_token = issue_token(
        &settings,
        format!("user:{seller_id}"),
        TokenScope::User,
        900,
    )
    .unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let user_app = routes().with_state(AppState::new(settings.clone()).with_mysql(pool.clone()));
    let admin_app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let idempotency_key = format!("spot-route-{}", Uuid::now_v7().simple());

    let sell_response = user_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {seller_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"pair_id":"{pair_symbol}","side":"sell","order_type":"limit","price":"10.000000000000000000","quantity":"2.0000","idempotency_key":"{idempotency_key}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let sell_status = sell_response.status();
    let sell_body = axum::body::to_bytes(sell_response.into_body(), 8192).await?;
    assert_eq!(
        sell_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&sell_body)
    );
    let sell_order: Value = serde_json::from_slice(&sell_body)?;
    let sell_order_id = sell_order["id"].as_str().unwrap().to_owned();
    sqlx::query("UPDATE spot_orders SET status = 'open' WHERE id = ?")
        .bind(&sell_order_id)
        .execute(&pool)
        .await?;
    let buy_order_id = seed_open_order(
        &pool,
        buyer_id,
        &pair_symbol,
        "buy",
        "10.000000000000000000",
        "2.0000",
    )
    .await?;

    let fill_response = admin_app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/fills")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"buy_order_id":"{buy_order_id}","sell_order_id":"{sell_order_id}","price":"10.000000000000000000","quantity":"1.000000000000000000","idempotency_key":"spot-fill-{buy_order_id}-{sell_order_id}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let fill_status = fill_response.status();
    let fill_body = axum::body::to_bytes(fill_response.into_body(), 8192).await?;
    assert_eq!(
        fill_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&fill_body)
    );

    let cancel_response = user_app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/spot/orders/{sell_order_id}"))
                .header("authorization", format!("Bearer {seller_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let cancel_status = cancel_response.status();
    let cancel_body = axum::body::to_bytes(cancel_response.into_body(), 8192).await?;
    assert_eq!(
        cancel_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&cancel_body)
    );
    let cancelled: Value = serde_json::from_slice(&cancel_body)?;
    assert_eq!(cancelled["cancelled"], true);
    assert_eq!(cancelled["order"]["status"], "cancelled");

    let (seller_base_available, seller_base_frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(seller_id)
    .bind(base_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        seller_base_available.normalized(),
        decimal("1.000000000000000000").normalized()
    );
    assert_eq!(
        seller_base_frozen.normalized(),
        decimal("0.000000000000000000").normalized()
    );
    let (buyer_quote_available, buyer_quote_frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(buyer_id)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        buyer_quote_available.normalized(),
        decimal("80.000000000000000000").normalized()
    );
    assert_eq!(
        buyer_quote_frozen.normalized(),
        decimal("10.000000000000000000").normalized()
    );

    cleanup_fill_fixture(
        &pool,
        buyer_id,
        seller_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &buy_order_id,
        &sell_order_id,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn spot_cancel_order_unfreezes_remaining_quote_wallet() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "CB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "CQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    let order_id = seed_open_buy_order(&pool, user_id, &pair_symbol).await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(quote_asset)
    .bind(decimal("80.000000000000000000"))
    .bind(decimal("20.000000000000000000"))
    .execute(&pool)
    .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/spot/orders/{order_id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 8192).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );
    let payload: Value = serde_json::from_slice(&body)?;
    assert_eq!(payload["cancelled"], true);
    assert_eq!(payload["order"]["status"], "cancelled");

    let (available, frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        available.normalized(),
        decimal("100.000000000000000000").normalized()
    );
    assert_eq!(
        frozen.normalized(),
        decimal("0.000000000000000000").normalized()
    );

    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'spot_order' AND ref_id = ?",
    )
    .bind(&order_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 2);

    cleanup_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &order_id,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn spot_cancel_historical_market_buy_reservation_uses_freeze_ledger()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "HB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "HQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(quote_asset)
    .bind(decimal("80.000000000000000000"))
    .bind(decimal("20.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(base_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    let order_id = sqlx::query(
        r#"INSERT INTO spot_orders
           (user_id, pair_id, side, order_type, price, quantity, filled_quantity, status,
            reserved_asset, reserved_amount)
           VALUES (?, ?, 'buy', 'market', NULL, ?, 0, 'open', ?, ?)"#,
    )
    .bind(user_id)
    .bind(pair_id(&pool, &pair_symbol).await?)
    .bind(decimal("2.0000"))
    .bind(quote_asset)
    .bind(decimal("0.000000000000000000"))
    .execute(&pool)
    .await?
    .last_insert_id()
    .to_string();
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, 'spot_freeze', ?, 'frozen', ?, ?, ?, 0, 'spot_order', ?)"#,
    )
    .bind(user_id)
    .bind(quote_asset)
    .bind(decimal("20.000000000000000000"))
    .bind(decimal("20.000000000000000000"))
    .bind(decimal("80.000000000000000000"))
    .bind(decimal("20.000000000000000000"))
    .bind(&order_id)
    .execute(&pool)
    .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/spot/orders/{order_id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 8192).await?;
    let (available, frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;

    cleanup_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        &pair_symbol,
        &order_id,
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );
    assert_eq!(
        available.normalized(),
        decimal("100.000000000000000000").normalized()
    );
    assert_eq!(
        frozen.normalized(),
        decimal("0.000000000000000000").normalized()
    );
    Ok(())
}

#[tokio::test]
async fn spot_fill_rejects_sell_order_that_exceeds_order_reservation() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let buyer_id = create_user(&pool).await;
    let seller_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "VB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "VQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    let pair_db_id = pair_id(&pool, &pair_symbol).await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(buyer_id)
    .bind(quote_asset)
    .bind(decimal("80.000000000000000000"))
    .bind(decimal("20.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(buyer_id)
        .bind(base_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(seller_id)
    .bind(base_asset)
    .bind(decimal("0.000000000000000000"))
    .bind(decimal("2.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(seller_id)
        .bind(quote_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    let buy_order_id = sqlx::query(
        r#"INSERT INTO spot_orders
           (user_id, pair_id, side, order_type, price, quantity, filled_quantity, status,
            reserved_asset, reserved_amount)
           VALUES (?, ?, 'buy', 'limit', ?, ?, 0, 'open', ?, ?)"#,
    )
    .bind(buyer_id)
    .bind(pair_db_id)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("2.0000"))
    .bind(quote_asset)
    .bind(decimal("20.000000000000000000"))
    .execute(&pool)
    .await?
    .last_insert_id()
    .to_string();
    let sell_order_id = sqlx::query(
        r#"INSERT INTO spot_orders
           (user_id, pair_id, side, order_type, price, quantity, filled_quantity, status,
            reserved_asset, reserved_amount)
           VALUES (?, ?, 'sell', 'limit', ?, ?, 0, 'open', ?, ?)"#,
    )
    .bind(seller_id)
    .bind(pair_db_id)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("2.0000"))
    .bind(base_asset)
    .bind(decimal("0.000000000000000000"))
    .execute(&pool)
    .await?
    .last_insert_id()
    .to_string();
    let other_sell_order_id = sqlx::query(
        r#"INSERT INTO spot_orders
           (user_id, pair_id, side, order_type, price, quantity, filled_quantity, status,
            reserved_asset, reserved_amount)
           VALUES (?, ?, 'sell', 'limit', ?, ?, 0, 'open', ?, ?)"#,
    )
    .bind(seller_id)
    .bind(pair_db_id)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("2.0000"))
    .bind(base_asset)
    .bind(decimal("2.000000000000000000"))
    .execute(&pool)
    .await?
    .last_insert_id()
    .to_string();
    let token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/fills")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"buy_order_id":"{buy_order_id}","sell_order_id":"{sell_order_id}","price":"10.000000000000000000","quantity":"2.000000000000000000","idempotency_key":"spot-fill-{buy_order_id}-{sell_order_id}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 8192).await?;

    sqlx::query("DELETE FROM wallet_ledger WHERE ref_type = 'spot_trade' AND ref_id = ?")
        .bind(format!("{buy_order_id}:{sell_order_id}"))
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM spot_trades WHERE buy_order_id = ? OR sell_order_id = ?")
        .bind(&buy_order_id)
        .bind(&sell_order_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM spot_orders WHERE id IN (?, ?, ?)")
        .bind(&buy_order_id)
        .bind(&sell_order_id)
        .bind(&other_sell_order_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id IN (?, ?) AND asset_id IN (?, ?)")
        .bind(buyer_id)
        .bind(seller_id)
        .bind(base_asset)
        .bind(quote_asset)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM trading_pairs WHERE symbol = ?")
        .bind(&pair_symbol)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id IN (?, ?)")
        .bind(base_asset)
        .bind(quote_asset)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id IN (?, ?)")
        .bind(buyer_id)
        .bind(seller_id)
        .execute(&pool)
        .await?;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );
    Ok(())
}

#[tokio::test]
async fn spot_fill_idempotency_key_rejects_mismatched_replay() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let buyer_id = create_user(&pool).await;
    let seller_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "IB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "IQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(buyer_id)
    .bind(quote_asset)
    .bind(decimal("60.000000000000000000"))
    .bind(decimal("40.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(buyer_id)
        .bind(base_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
    )
    .bind(seller_id)
    .bind(base_asset)
    .bind(decimal("0.000000000000000000"))
    .bind(decimal("4.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(seller_id)
        .bind(quote_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    let buy_order_id = seed_open_order(
        &pool,
        buyer_id,
        &pair_symbol,
        "buy",
        "10.000000000000000000",
        "1.0000",
    )
    .await?;
    let sell_order_id = seed_open_order(
        &pool,
        seller_id,
        &pair_symbol,
        "sell",
        "10.000000000000000000",
        "1.0000",
    )
    .await?;
    let replay_buy_order_id = seed_open_order(
        &pool,
        buyer_id,
        &pair_symbol,
        "buy",
        "10.000000000000000000",
        "1.0000",
    )
    .await?;
    let replay_sell_order_id = seed_open_order(
        &pool,
        seller_id,
        &pair_symbol,
        "sell",
        "10.000000000000000000",
        "1.0000",
    )
    .await?;
    sqlx::query(
        r#"UPDATE spot_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           SET orders.reserved_asset = CASE WHEN orders.side = 'buy' THEN pairs.quote_asset ELSE pairs.base_asset END,
               orders.reserved_amount = CASE WHEN orders.side = 'buy' THEN orders.quantity * orders.price ELSE orders.quantity END
           WHERE orders.id IN (?, ?, ?, ?)"#,
    )
    .bind(&buy_order_id)
    .bind(&sell_order_id)
    .bind(&replay_buy_order_id)
    .bind(&replay_sell_order_id)
    .execute(&pool)
    .await?;
    let token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let idempotency_key = format!("spot-fill-replay-{}", Uuid::now_v7().simple());

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/fills")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"buy_order_id":"{buy_order_id}","sell_order_id":"{sell_order_id}","price":"10.000000000000000000","quantity":"1.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let first_status = first_response.status();
    let first_body = axum::body::to_bytes(first_response.into_body(), 8192).await?;
    assert_eq!(
        first_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&first_body)
    );

    let replay_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/fills")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"buy_order_id":"{replay_buy_order_id}","sell_order_id":"{replay_sell_order_id}","price":"10.000000000000000000","quantity":"1.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let replay_status = replay_response.status();
    let replay_body = axum::body::to_bytes(replay_response.into_body(), 8192).await?;

    sqlx::query("DELETE FROM wallet_ledger WHERE ref_type = 'spot_trade' AND ref_id IN (?, ?)")
        .bind(format!("{buy_order_id}:{sell_order_id}"))
        .bind(format!("{replay_buy_order_id}:{replay_sell_order_id}"))
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM spot_trades WHERE buy_order_id IN (?, ?) OR sell_order_id IN (?, ?)")
        .bind(&buy_order_id)
        .bind(&replay_buy_order_id)
        .bind(&sell_order_id)
        .bind(&replay_sell_order_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM spot_orders WHERE id IN (?, ?, ?, ?)")
        .bind(&buy_order_id)
        .bind(&sell_order_id)
        .bind(&replay_buy_order_id)
        .bind(&replay_sell_order_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id IN (?, ?) AND asset_id IN (?, ?)")
        .bind(buyer_id)
        .bind(seller_id)
        .bind(base_asset)
        .bind(quote_asset)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM trading_pairs WHERE symbol = ?")
        .bind(&pair_symbol)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id IN (?, ?)")
        .bind(base_asset)
        .bind(quote_asset)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id IN (?, ?)")
        .bind(buyer_id)
        .bind(seller_id)
        .execute(&pool)
        .await?;
    assert_eq!(
        replay_status,
        StatusCode::CONFLICT,
        "payload: {}",
        String::from_utf8_lossy(&replay_body)
    );
    Ok(())
}

#[tokio::test]
async fn spot_fill_concurrent_duplicate_key_rejects_mismatched_request_without_500()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let first_buyer_id = create_user(&pool).await;
    let first_seller_id = create_user(&pool).await;
    let second_buyer_id = create_user(&pool).await;
    let second_seller_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "DB").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "DQ").await;
    let pair_symbol =
        create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    let pair_db_id = pair_id(&pool, &pair_symbol).await?;

    for user_id in [first_buyer_id, second_buyer_id] {
        sqlx::query(
            "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("80.000000000000000000"))
        .bind(decimal("20.000000000000000000"))
        .execute(&pool)
        .await?;
        sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
            .bind(user_id)
            .bind(base_asset)
            .bind(decimal("0.000000000000000000"))
            .execute(&pool)
            .await?;
    }
    for user_id in [first_seller_id, second_seller_id] {
        sqlx::query(
            "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind(base_asset)
        .bind(decimal("0.000000000000000000"))
        .bind(decimal("2.000000000000000000"))
        .execute(&pool)
        .await?;
        sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
            .bind(user_id)
            .bind(quote_asset)
            .bind(decimal("0.000000000000000000"))
            .execute(&pool)
            .await?;
    }

    let first_buy_order_id = sqlx::query(
        r#"INSERT INTO spot_orders
           (user_id, pair_id, side, order_type, price, quantity, filled_quantity, status,
            reserved_asset, reserved_amount)
           VALUES (?, ?, 'buy', 'limit', ?, ?, 0, 'open', ?, ?)"#,
    )
    .bind(first_buyer_id)
    .bind(pair_db_id)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("2.0000"))
    .bind(quote_asset)
    .bind(decimal("20.000000000000000000"))
    .execute(&pool)
    .await?
    .last_insert_id()
    .to_string();
    let first_sell_order_id = sqlx::query(
        r#"INSERT INTO spot_orders
           (user_id, pair_id, side, order_type, price, quantity, filled_quantity, status,
            reserved_asset, reserved_amount)
           VALUES (?, ?, 'sell', 'limit', ?, ?, 0, 'open', ?, ?)"#,
    )
    .bind(first_seller_id)
    .bind(pair_db_id)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("2.0000"))
    .bind(base_asset)
    .bind(decimal("2.000000000000000000"))
    .execute(&pool)
    .await?
    .last_insert_id()
    .to_string();
    let second_buy_order_id = sqlx::query(
        r#"INSERT INTO spot_orders
           (user_id, pair_id, side, order_type, price, quantity, filled_quantity, status,
            reserved_asset, reserved_amount)
           VALUES (?, ?, 'buy', 'limit', ?, ?, 0, 'open', ?, ?)"#,
    )
    .bind(second_buyer_id)
    .bind(pair_db_id)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("2.0000"))
    .bind(quote_asset)
    .bind(decimal("20.000000000000000000"))
    .execute(&pool)
    .await?
    .last_insert_id()
    .to_string();
    let second_sell_order_id = sqlx::query(
        r#"INSERT INTO spot_orders
           (user_id, pair_id, side, order_type, price, quantity, filled_quantity, status,
            reserved_asset, reserved_amount)
           VALUES (?, ?, 'sell', 'limit', ?, ?, 0, 'open', ?, ?)"#,
    )
    .bind(second_seller_id)
    .bind(pair_db_id)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("2.0000"))
    .bind(base_asset)
    .bind(decimal("2.000000000000000000"))
    .execute(&pool)
    .await?
    .last_insert_id()
    .to_string();

    let token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let idempotency_key = format!("spot-fill-race-{}", Uuid::now_v7().simple());
    let mut blocker = pool.begin().await?;
    sqlx::query("SELECT id FROM wallet_accounts WHERE user_id = ? AND asset_id = ? FOR UPDATE")
        .bind(first_buyer_id)
        .bind(quote_asset)
        .fetch_one(&mut *blocker)
        .await?;

    let first_app = app.clone();
    let first_token = token.clone();
    let first_body = format!(
        r#"{{"buy_order_id":"{first_buy_order_id}","sell_order_id":"{first_sell_order_id}","price":"10.000000000000000000","quantity":"2.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );
    let first_fill = tokio::spawn(async move {
        first_app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/spot/fills")
                    .header("authorization", format!("Bearer {first_token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(first_body))
                    .unwrap(),
            )
            .await
            .unwrap()
    });
    tokio::time::sleep(Duration::from_millis(150)).await;

    let replay_app = app.clone();
    let replay_token = token.clone();
    let replay_body = format!(
        r#"{{"buy_order_id":"{second_buy_order_id}","sell_order_id":"{second_sell_order_id}","price":"10.000000000000000000","quantity":"2.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );
    let replay_fill = tokio::spawn(async move {
        replay_app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/spot/fills")
                    .header("authorization", format!("Bearer {replay_token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(replay_body))
                    .unwrap(),
            )
            .await
            .unwrap()
    });
    tokio::time::sleep(Duration::from_millis(150)).await;
    blocker.commit().await?;

    let first_response = first_fill.await?;
    let first_status = first_response.status();
    let first_body = axum::body::to_bytes(first_response.into_body(), 8192).await?;
    let replay_response = replay_fill.await?;
    let replay_status = replay_response.status();
    let replay_body = axum::body::to_bytes(replay_response.into_body(), 8192).await?;

    let (trade_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM spot_trades WHERE idempotency_key = ?")
            .bind(&idempotency_key)
            .fetch_one(&pool)
            .await?;
    let (second_trade_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM spot_trades WHERE buy_order_id = ? OR sell_order_id = ?",
    )
    .bind(&second_buy_order_id)
    .bind(&second_sell_order_id)
    .fetch_one(&pool)
    .await?;
    let (second_ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'spot_trade' AND ref_id = ?",
    )
    .bind(format!("{second_buy_order_id}:{second_sell_order_id}"))
    .fetch_one(&pool)
    .await?;
    let (second_buy_status, second_buy_filled): (String, BigDecimal) =
        sqlx::query_as("SELECT status, filled_quantity FROM spot_orders WHERE id = ?")
            .bind(&second_buy_order_id)
            .fetch_one(&pool)
            .await?;
    let (second_buyer_quote_available, second_buyer_quote_frozen): (BigDecimal, BigDecimal) =
        sqlx::query_as(
            "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
        )
        .bind(second_buyer_id)
        .bind(quote_asset)
        .fetch_one(&pool)
        .await?;

    sqlx::query("DELETE FROM wallet_ledger WHERE ref_type IN ('spot_trade', 'spot_order') AND ref_id IN (?, ?, ?, ?)")
        .bind(format!("{first_buy_order_id}:{first_sell_order_id}"))
        .bind(format!("{second_buy_order_id}:{second_sell_order_id}"))
        .bind(&first_buy_order_id)
        .bind(&second_buy_order_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM spot_trades WHERE idempotency_key = ?")
        .bind(&idempotency_key)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM spot_orders WHERE id IN (?, ?, ?, ?)")
        .bind(&first_buy_order_id)
        .bind(&first_sell_order_id)
        .bind(&second_buy_order_id)
        .bind(&second_sell_order_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id IN (?, ?, ?, ?) AND asset_id IN (?, ?)")
        .bind(first_buyer_id)
        .bind(first_seller_id)
        .bind(second_buyer_id)
        .bind(second_seller_id)
        .bind(base_asset)
        .bind(quote_asset)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM trading_pairs WHERE symbol = ?")
        .bind(&pair_symbol)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id IN (?, ?)")
        .bind(base_asset)
        .bind(quote_asset)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id IN (?, ?, ?, ?)")
        .bind(first_buyer_id)
        .bind(first_seller_id)
        .bind(second_buyer_id)
        .bind(second_seller_id)
        .execute(&pool)
        .await?;

    assert_eq!(
        first_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&first_body)
    );
    assert_eq!(
        replay_status,
        StatusCode::CONFLICT,
        "payload: {}",
        String::from_utf8_lossy(&replay_body)
    );
    assert_eq!(trade_count, 1);
    assert_eq!(second_trade_count, 0);
    assert_eq!(second_ledger_count, 0);
    assert_eq!(second_buy_status, "open");
    assert_eq!(
        second_buy_filled.normalized(),
        decimal("0.000000000000000000").normalized()
    );
    assert_eq!(
        second_buyer_quote_available.normalized(),
        decimal("80.000000000000000000").normalized()
    );
    assert_eq!(
        second_buyer_quote_frozen.normalized(),
        decimal("20.000000000000000000").normalized()
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn cleanup_fill_fixture(
    pool: &MySqlPool,
    buyer_id: u64,
    seller_id: u64,
    base_asset: u64,
    quote_asset: u64,
    pair_symbol: &str,
    buy_order_id: &str,
    sell_order_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM wallet_ledger WHERE ref_type = 'spot_trade' AND ref_id = ?")
        .bind(format!("{buy_order_id}:{sell_order_id}"))
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM wallet_ledger WHERE ref_type = 'spot_order' AND ref_id IN (?, ?)")
        .bind(buy_order_id)
        .bind(sell_order_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM spot_trades WHERE buy_order_id = ? OR sell_order_id = ?")
        .bind(buy_order_id)
        .bind(sell_order_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM spot_orders WHERE id IN (?, ?)")
        .bind(buy_order_id)
        .bind(sell_order_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id IN (?, ?) AND asset_id IN (?, ?)")
        .bind(buyer_id)
        .bind(seller_id)
        .bind(base_asset)
        .bind(quote_asset)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM trading_pairs WHERE symbol = ?")
        .bind(pair_symbol)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(base_asset)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(quote_asset)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id IN (?, ?)")
        .bind(buyer_id)
        .bind(seller_id)
        .execute(pool)
        .await?;
    Ok(())
}

async fn cleanup_fixture(
    pool: &MySqlPool,
    user_id: u64,
    base_asset: u64,
    quote_asset: u64,
    pair_symbol: &str,
    order_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM wallet_ledger WHERE ref_type = 'spot_order' AND ref_id = ?")
        .bind(order_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM spot_orders WHERE id = ?")
        .bind(order_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ? AND asset_id IN (?, ?)")
        .bind(user_id)
        .bind(base_asset)
        .bind(quote_asset)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM trading_pairs WHERE symbol = ?")
        .bind(pair_symbol)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(base_asset)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(quote_asset)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
