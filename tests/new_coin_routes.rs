use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use bigdecimal::BigDecimal;
use exchange_api::{
    config::Settings,
    modules::{
        auth::{TokenScope, issue_token},
        events::{EventBroadcastHub, WebSocketChannel},
        new_coin::routes::user_routes,
    },
    state::AppState,
};
use secrecy::SecretString;
use serde_json::Value;
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::{error::Error, str::FromStr, time::Duration};
use tokio::time::timeout;
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
            eprintln!("skipping MySQL new coin route test because DATABASE_URL is not set");
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
    let email = format!("new-coin-route-{}@example.test", Uuid::now_v7().simple());
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
    let symbol = format!("{prefix}{}", &suffix[..12]);
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

async fn create_new_coin_project(pool: &MySqlPool, asset_id: u64, symbol: &str) -> u64 {
    create_new_coin_project_with_status(pool, asset_id, symbol, "listed").await
}

async fn create_new_coin_project_with_status(
    pool: &MySqlPool,
    asset_id: u64,
    symbol: &str,
    lifecycle_status: &str,
) -> u64 {
    sqlx::query(
        r#"INSERT INTO new_coin_projects
           (asset_id, symbol, lifecycle_status, total_supply, issue_price, listed_at,
            unlock_type, fixed_unlock_at, unlock_fee_enabled, unlock_fee_rate,
            unlock_fee_basis, unlock_fee_asset, status)
           VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP(6), 'fixed_time',
                   DATE_ADD(CURRENT_TIMESTAMP(6), INTERVAL 7 DAY), true, ?, 'market_value', ?, 'active')"#,
    )
    .bind(asset_id)
    .bind(symbol)
    .bind(lifecycle_status)
    .bind(decimal("1000000.000000000000000000"))
    .bind(decimal("1.000000000000000000"))
    .bind(decimal("0.04000000"))
    .bind(asset_id)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id()
}

async fn create_pair(
    pool: &MySqlPool,
    base_asset: u64,
    quote_asset: u64,
    base_symbol: &str,
    quote_symbol: &str,
) -> u64 {
    sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, 2, 4, ?, 'active', 'spot')"#,
    )
    .bind(base_asset)
    .bind(quote_asset)
    .bind(format!("{base_symbol}-{quote_symbol}"))
    .bind(decimal("1.000000000000000000"))
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id()
}

async fn seed_unlock_record(
    pool: &MySqlPool,
    user_id: u64,
    asset_id: u64,
    unlock_after_days: i64,
) -> String {
    let unlock_key = format!("unlock-route-{}", Uuid::now_v7().simple());
    let merge_key = format!("lock-route-{}", Uuid::now_v7().simple());
    let lock_position_id = sqlx::query(
        r#"INSERT INTO asset_lock_positions
           (user_id, asset_id, unlock_type, unlock_at, locked_amount, released_amount,
            remaining_amount, merge_key, status)
           VALUES (?, ?, 'fixed_time', DATE_ADD(CURRENT_TIMESTAMP(6), INTERVAL ? DAY), ?, 0, ?, ?, 'active')"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(unlock_after_days)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("10.000000000000000000"))
    .bind(merge_key)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id();

    sqlx::query(
        r#"INSERT INTO asset_unlock_records
           (user_id, asset_id, lock_position_id, unlock_quantity, unlock_price,
            unlock_fee_enabled, unlock_fee_rate, unlock_fee_basis, unlock_fee_asset,
            unlock_fee_amount, fee_paid_status, status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, true, ?, 'market_value', ?, ?, 'pending', 'pending', ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(lock_position_id)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("5.000000000000000000"))
    .bind(decimal("0.04000000"))
    .bind(asset_id)
    .bind(decimal("2.000000000000000000"))
    .bind(&unlock_key)
    .execute(pool)
    .await
    .unwrap();

    unlock_key
}

#[tokio::test]
async fn new_coin_routes_require_auth_for_user_unlocks() {
    let response = user_routes()
        .with_state(AppState::new(test_settings()))
        .oneshot(
            Request::builder()
                .uri("/new-coins/unlocks")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn new_coin_routes_return_clear_error_without_mysql() {
    let settings = test_settings();
    let token = issue_token(&settings, "user:42", TokenScope::User, 900).unwrap();
    let response = user_routes()
        .with_state(AppState::new(settings))
        .oneshot(
            Request::builder()
                .uri("/new-coins/unlocks")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert_eq!(
        payload["message"],
        "internal error: mysql pool is not configured for new coin routes"
    );
}

#[tokio::test]
async fn new_coin_routes_list_projects_and_allow_fee_payment() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (asset_id, symbol) = create_asset(&pool, "NC").await;
    let project_id = create_new_coin_project(&pool, asset_id, &symbol).await;
    let unlock_key = seed_unlock_record(&pool, user_id, asset_id, 0).await;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = user_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/new-coins?limit=100")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = axum::body::to_bytes(list_response.into_body(), 131072).await?;
    let projects: Value = serde_json::from_slice(&list_body)?;
    assert!(
        projects["projects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|project| {
                project["id"] == project_id
                    && project["symbol"] == symbol
                    && project["lifecycle_status"] == "listed"
                    && project["post_listing_purchase_enabled"] == false
                    && project["post_listing_pair_id"].is_null()
            }),
        "payload: {projects}"
    );

    let unlocks_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/new-coins/unlocks")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unlocks_response.status(), StatusCode::OK);
    let unlocks_body = axum::body::to_bytes(unlocks_response.into_body(), 8192).await?;
    let unlocks: Value = serde_json::from_slice(&unlocks_body)?;
    assert!(unlocks["unlocks"].as_array().unwrap().iter().any(|unlock| {
        unlock["idempotency_key"] == unlock_key && unlock["fee_paid_status"] == "pending"
    }));

    let pay_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/unlocks/{unlock_key}/pay-fee"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"payment_asset_id":{asset_id},"amount":"2.000000000000000000"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(pay_response.status(), StatusCode::OK);
    let pay_body = axum::body::to_bytes(pay_response.into_body(), 8192).await?;
    let paid: Value = serde_json::from_slice(&pay_body)?;
    assert_eq!(paid["paid"], true);
    assert_eq!(paid["unlock_idempotency_key"], unlock_key);

    cleanup_fixture(&pool, user_id, asset_id, project_id, &unlock_key).await?;
    Ok(())
}

#[tokio::test]
async fn new_coin_routes_reject_invalid_fee_payment_and_early_release() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (asset_id, symbol) = create_asset(&pool, "NF").await;
    let project_id = create_new_coin_project(&pool, asset_id, &symbol).await;
    let unlock_key = seed_unlock_record(&pool, user_id, asset_id, 7).await;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = user_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let invalid_fee_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/unlocks/{unlock_key}/pay-fee"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"payment_asset_id":{asset_id},"amount":"0.000000000000000000"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_fee_response.status(), StatusCode::BAD_REQUEST);

    let release_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/unlocks/{unlock_key}/release"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(release_response.status(), StatusCode::BAD_REQUEST);

    let (fee_status, status): (String, String) = sqlx::query_as(
        "SELECT fee_paid_status, status FROM asset_unlock_records WHERE idempotency_key = ?",
    )
    .bind(&unlock_key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(fee_status, "pending");
    assert_eq!(status, "pending");

    cleanup_fixture(&pool, user_id, asset_id, project_id, &unlock_key).await?;
    Ok(())
}

#[tokio::test]
async fn new_coin_subscription_debits_quote_wallet_and_locks_fixed_time_allocation()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "NS").await;
    let (quote_asset, _quote_symbol) = create_asset(&pool, "NQ").await;
    let project_id =
        create_new_coin_project_with_status(&pool, base_asset, &base_symbol, "subscription").await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, locked) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(base_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("new-sub-{}", Uuid::now_v7().simple());
    let hub = EventBroadcastHub::new(16);
    let _keepalive_hub = hub.clone();
    let mut private_events = hub.subscribe(&WebSocketChannel::private_user(user_id));
    let app = user_routes().with_state(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_event_broadcast_hub(hub),
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/{base_symbol}/subscriptions"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"quote_asset_id":{quote_asset},"quote_amount":"20.000000000000000000","quantity":"20.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
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
    let subscription: Value = serde_json::from_slice(&body)?;
    assert_eq!(subscription["idempotency_key"], idempotency_key);
    assert_eq!(subscription["status"], "allocated");
    let lock_position_id = subscription["lock_position_id"].as_u64().unwrap();
    let event_message = timeout(Duration::from_millis(100), private_events.recv()).await??;
    let event: Value = serde_json::from_str(event_message.payload())?;
    assert_eq!(event["type"], "new_coin.subscription.created");
    assert_eq!(event["idempotency_key"], idempotency_key);
    assert_eq!(event["project_id"], project_id);
    assert_eq!(event["asset_id"], base_asset);
    assert_eq!(event["quote_asset_id"], quote_asset);
    assert_eq!(event["quote_amount"], "20.000000000000000000");
    assert_eq!(event["quantity"], "20.000000000000000000");
    assert_eq!(event["status"], "allocated");
    assert_eq!(event["lock_position_id"], lock_position_id);

    let (quote_available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        quote_available.normalized(),
        decimal("80.000000000000000000").normalized()
    );

    let (base_locked,): (BigDecimal,) =
        sqlx::query_as("SELECT locked FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(base_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        base_locked.normalized(),
        decimal("20.000000000000000000").normalized()
    );

    let (remaining, lock_status): (BigDecimal, String) =
        sqlx::query_as("SELECT remaining_amount, status FROM asset_lock_positions WHERE id = ?")
            .bind(lock_position_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        remaining.normalized(),
        decimal("20.000000000000000000").normalized()
    );
    assert_eq!(lock_status, "active");

    let (source_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM asset_lock_position_sources WHERE source_type = 'new_coin_subscription' AND source_id = ?",
    )
    .bind(&idempotency_key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(source_count, 1);

    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'new_coin_subscription' AND ref_id = ?",
    )
    .bind(&idempotency_key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 2);

    let duplicate_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/{base_symbol}/subscriptions"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"quote_asset_id":{quote_asset},"quote_amount":"20.000000000000000000","quantity":"20.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let duplicate_status = duplicate_response.status();
    let duplicate_body = axum::body::to_bytes(duplicate_response.into_body(), 8192).await?;
    assert_eq!(
        duplicate_status,
        StatusCode::CONFLICT,
        "payload: {}",
        String::from_utf8_lossy(&duplicate_body)
    );
    let (quote_after_duplicate,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        quote_after_duplicate.normalized(),
        quote_available.normalized()
    );
    let (ledger_count_after_duplicate,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'new_coin_subscription' AND ref_id = ?",
    )
    .bind(&idempotency_key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count_after_duplicate, 2);

    cleanup_order_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        project_id,
        None,
        &idempotency_key,
        "new_coin_subscription",
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn new_coin_purchase_debits_quote_wallet_and_locks_fixed_time_allocation()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "NP").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "NT").await;
    let project_id = create_new_coin_project(&pool, base_asset, &base_symbol).await;
    let pair_id = create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query(
        "UPDATE new_coin_projects SET post_listing_purchase_enabled = TRUE, post_listing_pair_id = ? WHERE id = ?",
    )
    .bind(pair_id)
    .bind(project_id)
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, locked) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(base_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("new-pur-{}", Uuid::now_v7().simple());
    let hub = EventBroadcastHub::new(16);
    let _keepalive_hub = hub.clone();
    let mut private_events = hub.subscribe(&WebSocketChannel::private_user(user_id));
    let app = user_routes().with_state(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_event_broadcast_hub(hub),
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/{base_symbol}/purchase"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"pair_id":{pair_id},"price":"2.000000000000000000","quantity":"10.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
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
    let purchase: Value = serde_json::from_slice(&body)?;
    assert_eq!(purchase["idempotency_key"], idempotency_key);
    assert_eq!(purchase["status"], "locked");
    let lock_position_id = purchase["lock_position_id"].as_u64().unwrap();
    let event_message = timeout(Duration::from_millis(100), private_events.recv()).await??;
    let event: Value = serde_json::from_str(event_message.payload())?;
    assert_eq!(event["type"], "new_coin.purchase.created");
    assert_eq!(event["idempotency_key"], idempotency_key);
    assert_eq!(event["project_id"], project_id);
    assert_eq!(event["pair_id"], pair_id);
    assert_eq!(event["asset_id"], base_asset);
    assert_eq!(event["quote_asset_id"], quote_asset);
    assert_eq!(event["price"], "2.000000000000000000");
    assert_eq!(event["quantity"], "10.000000000000000000");
    assert_eq!(
        event["quote_amount"],
        "20.000000000000000000000000000000000000"
    );
    assert_eq!(event["status"], "locked");
    assert_eq!(event["lock_position_id"], lock_position_id);

    let (quote_available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        quote_available.normalized(),
        decimal("80.000000000000000000").normalized()
    );

    let (base_locked,): (BigDecimal,) =
        sqlx::query_as("SELECT locked FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(base_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        base_locked.normalized(),
        decimal("10.000000000000000000").normalized()
    );

    let (remaining,): (BigDecimal,) =
        sqlx::query_as("SELECT remaining_amount FROM asset_lock_positions WHERE id = ?")
            .bind(lock_position_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        remaining.normalized(),
        decimal("10.000000000000000000").normalized()
    );

    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'new_coin_purchase' AND ref_id = ?",
    )
    .bind(&idempotency_key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 2);

    let (unlock_count, fee_status): (i64, String) = sqlx::query_as(
        r#"SELECT COUNT(*), MIN(fee_paid_status)
           FROM asset_unlock_records
           WHERE user_id = ? AND asset_id = ? AND lock_position_id = ? AND status = 'pending'"#,
    )
    .bind(user_id)
    .bind(base_asset)
    .bind(lock_position_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(unlock_count, 1);
    assert_eq!(fee_status, "pending");

    let duplicate_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/{base_symbol}/purchase"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"pair_id":{pair_id},"price":"2.000000000000000000","quantity":"10.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let duplicate_status = duplicate_response.status();
    let duplicate_body = axum::body::to_bytes(duplicate_response.into_body(), 8192).await?;
    assert_eq!(
        duplicate_status,
        StatusCode::CONFLICT,
        "payload: {}",
        String::from_utf8_lossy(&duplicate_body)
    );
    let (quote_after_duplicate,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        quote_after_duplicate.normalized(),
        quote_available.normalized()
    );
    let (ledger_count_after_duplicate,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'new_coin_purchase' AND ref_id = ?",
    )
    .bind(&idempotency_key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count_after_duplicate, 2);

    cleanup_order_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        project_id,
        Some(pair_id),
        &idempotency_key,
        "new_coin_purchase",
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn new_coin_purchase_requires_enabled_post_listing_pair() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "NE").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "NU").await;
    let project_id = create_new_coin_project(&pool, base_asset, &base_symbol).await;
    let pair_id = create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = user_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let disabled_key = format!("new-pur-disabled-{}", Uuid::now_v7().simple());
    let disabled_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/{base_symbol}/purchase"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"pair_id":{pair_id},"price":"2.000000000000000000","quantity":"10.000000000000000000","idempotency_key":"{disabled_key}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let disabled_status = disabled_response.status();
    let disabled_body = axum::body::to_bytes(disabled_response.into_body(), 8192).await?;
    assert_eq!(
        disabled_status,
        StatusCode::BAD_REQUEST,
        "payload: {}",
        String::from_utf8_lossy(&disabled_body)
    );
    let disabled_payload: Value = serde_json::from_slice(&disabled_body)?;
    assert_eq!(disabled_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        disabled_payload["message"],
        "validation error: post-listing new coin purchase is not open for this project"
    );

    sqlx::query(
        "UPDATE new_coin_projects SET post_listing_purchase_enabled = TRUE, post_listing_pair_id = ? WHERE id = ?",
    )
    .bind(pair_id)
    .bind(project_id)
    .execute(&pool)
    .await?;

    let mismatched_pair_id = u64::MAX;
    let mismatched_key = format!("new-pur-mismatch-{}", Uuid::now_v7().simple());
    let mismatched_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/{base_symbol}/purchase"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"pair_id":{mismatched_pair_id},"price":"2.000000000000000000","quantity":"10.000000000000000000","idempotency_key":"{mismatched_key}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let mismatched_status = mismatched_response.status();
    let mismatched_body = axum::body::to_bytes(mismatched_response.into_body(), 8192).await?;
    assert_eq!(
        mismatched_status,
        StatusCode::BAD_REQUEST,
        "payload: {}",
        String::from_utf8_lossy(&mismatched_body)
    );

    cleanup_order_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        project_id,
        Some(pair_id),
        &mismatched_key,
        "new_coin_purchase",
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn new_coin_routes_release_due_paid_unlock_updates_wallet_and_lock_state()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (asset_id, symbol) = create_asset(&pool, "NR").await;
    let project_id = create_new_coin_project(&pool, asset_id, &symbol).await;
    let unlock_key = seed_unlock_record(&pool, user_id, asset_id, 0).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, locked) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(asset_id)
        .bind(decimal("10.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query(
        "UPDATE asset_unlock_records SET fee_paid_status = 'paid' WHERE idempotency_key = ?",
    )
    .bind(&unlock_key)
    .execute(&pool)
    .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let hub = EventBroadcastHub::new(16);
    let mut private_events = hub.subscribe(&WebSocketChannel::private_user(user_id));
    let _keepalive_hub = hub.clone();
    let app = user_routes().with_state(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_event_broadcast_hub(hub),
    );

    let release_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/unlocks/{unlock_key}/release"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(release_response.status(), StatusCode::OK);
    let release_body = axum::body::to_bytes(release_response.into_body(), 8192).await?;
    let released: Value = serde_json::from_slice(&release_body)?;
    assert_eq!(released["released"], true);

    let event_message = timeout(Duration::from_millis(100), private_events.recv()).await??;
    let event: Value = serde_json::from_str(event_message.payload())?;
    assert_eq!(event["type"], "new_coin.unlock.released");
    assert_eq!(event["unlock_idempotency_key"], unlock_key);
    assert_eq!(event["asset_id"], asset_id);
    assert_eq!(event["unlock_quantity"], "10.000000000000000000");
    assert_eq!(event["released"], true);

    let replay_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/unlocks/{unlock_key}/release"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replay_response.status(), StatusCode::OK);
    let replay_body = axum::body::to_bytes(replay_response.into_body(), 8192).await?;
    let replayed: Value = serde_json::from_slice(&replay_body)?;
    assert_eq!(replayed["released"], true);
    assert!(
        timeout(Duration::from_millis(25), private_events.recv())
            .await
            .is_err(),
        "idempotent new coin unlock replay must not publish duplicate private event"
    );

    let (available, locked): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, locked FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(available, decimal("10.000000000000000000"));
    assert_eq!(locked, decimal("0.000000000000000000"));

    let (remaining, lock_status): (BigDecimal, String) = sqlx::query_as(
        r#"SELECT remaining_amount, status
           FROM asset_lock_positions
           WHERE user_id = ? AND asset_id = ?"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(remaining, decimal("0.000000000000000000"));
    assert_eq!(lock_status, "released");

    let (unlock_status,): (String,) =
        sqlx::query_as("SELECT status FROM asset_unlock_records WHERE idempotency_key = ?")
            .bind(&unlock_key)
            .fetch_one(&pool)
            .await?;
    assert_eq!(unlock_status, "released");

    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'new_coin_unlock' AND ref_id = ?",
    )
    .bind(&unlock_key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 2);

    cleanup_fixture(&pool, user_id, asset_id, project_id, &unlock_key).await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn cleanup_order_fixture(
    pool: &MySqlPool,
    user_id: u64,
    base_asset: u64,
    quote_asset: u64,
    project_id: u64,
    pair_id: Option<u64>,
    idempotency_key: &str,
    ref_type: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM wallet_ledger WHERE ref_type = ? AND ref_id = ?")
        .bind(ref_type)
        .bind(idempotency_key)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM asset_unlock_records WHERE idempotency_key = ?")
        .bind(idempotency_key)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM asset_lock_position_sources WHERE source_id = ?")
        .bind(idempotency_key)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM new_coin_subscriptions WHERE idempotency_key = ?")
        .bind(idempotency_key)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM new_coin_purchase_orders WHERE idempotency_key = ?")
        .bind(idempotency_key)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM asset_lock_positions WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(base_asset)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ? AND asset_id IN (?, ?)")
        .bind(user_id)
        .bind(base_asset)
        .bind(quote_asset)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM new_coin_projects WHERE id = ?")
        .bind(project_id)
        .execute(pool)
        .await?;
    if let Some(pair_id) = pair_id {
        sqlx::query("DELETE FROM trading_pairs WHERE id = ?")
            .bind(pair_id)
            .execute(pool)
            .await?;
    }
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

async fn cleanup_fixture(
    pool: &MySqlPool,
    user_id: u64,
    asset_id: u64,
    project_id: u64,
    unlock_key: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM wallet_ledger WHERE ref_type = 'new_coin_unlock' AND ref_id = ?")
        .bind(unlock_key)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM asset_unlock_records WHERE idempotency_key = ?")
        .bind(unlock_key)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM asset_lock_positions WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM new_coin_projects WHERE id = ?")
        .bind(project_id)
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
