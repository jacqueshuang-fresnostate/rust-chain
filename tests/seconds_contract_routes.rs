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
        market::market_ticker_redis_key,
        seconds_contract::routes::{admin_routes, user_routes},
    },
    state::AppState,
};
use redis::AsyncCommands;
use secrecy::SecretString;
use serde_json::Value;
use sqlx::{MySql, MySqlPool, Transaction, mysql::MySqlPoolOptions};
use std::{error::Error, str::FromStr, time::Duration};
use tokio::time::timeout;
use tower::ServiceExt;
use uuid::Uuid;

mod support;

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
            eprintln!("skipping MySQL seconds contract route test because DATABASE_URL is not set");
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

async fn redis_manager() -> Option<redis::aio::ConnectionManager> {
    let redis_url = match std::env::var("REDIS_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => return None,
    };
    let client = redis::Client::open(redis_url).unwrap();
    redis::aio::ConnectionManager::new(client).await.ok()
}

async fn seed_ticker(redis: &Option<redis::aio::ConnectionManager>, symbol: &str, price: &str) {
    let Some(redis) = redis else {
        return;
    };
    seed_ticker_at(redis, symbol, price, chrono::Utc::now().timestamp_millis()).await;
}

async fn seed_ticker_at(
    redis: &redis::aio::ConnectionManager,
    symbol: &str,
    price: &str,
    observed_at: i64,
) {
    let mut connection = redis.clone();
    let payload = serde_json::json!({
        "symbol": symbol.replace('-', ""),
        "last_price": price,
        "volume_24h": "1.000000000000000000",
        "observed_at": observed_at,
    })
    .to_string();
    let _: () = connection
        .set(market_ticker_redis_key(symbol), payload)
        .await
        .unwrap();
}

fn state_with_mysql_and_redis(
    settings: Settings,
    pool: MySqlPool,
    redis: Option<redis::aio::ConnectionManager>,
) -> AppState {
    let state = AppState::new(settings).with_mysql(pool);
    if let Some(redis) = redis {
        state.with_redis(redis)
    } else {
        state
    }
}

fn state_with_mysql_redis_and_events(
    settings: Settings,
    pool: MySqlPool,
    redis: Option<redis::aio::ConnectionManager>,
    hub: EventBroadcastHub,
) -> AppState {
    state_with_mysql_and_redis(settings, pool, redis).with_event_broadcast_hub(hub)
}

async fn create_user(tx: &mut Transaction<'_, MySql>) -> u64 {
    let email = format!("seconds-route-{}@example.test", Uuid::now_v7().simple());
    create_user_with_email(tx, email).await
}

async fn create_user_with_email(tx: &mut Transaction<'_, MySql>, email: String) -> u64 {
    sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(email)
        .bind("not-a-real-hash")
        .execute(&mut **tx)
        .await
        .unwrap()
        .last_insert_id()
}

async fn create_admin(pool: &MySqlPool) -> u64 {
    let suffix = Uuid::now_v7().simple().to_string();
    let role_id = sqlx::query("INSERT INTO admin_roles (name, permissions) VALUES (?, JSON_ARRAY('seconds_contract:read'))")
        .bind(format!("seconds-role-{}", &suffix[16..32]))
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id();
    sqlx::query("INSERT INTO admin_users (username, password_hash, role_id) VALUES (?, ?, ?)")
        .bind(format!("seconds-admin-{}", &suffix[16..32]))
        .bind("not-a-real-hash")
        .bind(role_id)
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id()
}

async fn create_asset(tx: &mut Transaction<'_, MySql>, prefix: &str) -> (u64, String) {
    let suffix = Uuid::now_v7().simple().to_string();
    let symbol = format!("{prefix}{}", &suffix[16..32]);
    let id = sqlx::query(
        "INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, 18, 'coin', 'active')",
    )
    .bind(&symbol)
    .bind(&symbol)
    .execute(&mut **tx)
    .await
    .unwrap()
    .last_insert_id();
    (id, symbol)
}

async fn create_pair(
    tx: &mut Transaction<'_, MySql>,
    base_asset: u64,
    quote_asset: u64,
    symbol: &str,
) -> u64 {
    sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, 18, 18, ?, 'active', 'external')"#,
    )
    .bind(base_asset)
    .bind(quote_asset)
    .bind(symbol)
    .bind(decimal("1.000000000000000000"))
    .execute(&mut **tx)
    .await
    .unwrap()
    .last_insert_id()
}

async fn seed_seconds_product(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
    stake_asset: u64,
) -> u64 {
    sqlx::query(
        r#"INSERT INTO seconds_contract_products
           (pair_id, stake_asset, duration_seconds, payout_rate, min_stake, max_stake, status)
           VALUES (?, ?, 60, ?, ?, ?, 'active')"#,
    )
    .bind(pair_id)
    .bind(stake_asset)
    .bind(decimal("0.80000000"))
    .bind(decimal("5.000000000000000000"))
    .bind(decimal("100.000000000000000000"))
    .execute(&mut **tx)
    .await
    .unwrap()
    .last_insert_id()
}

async fn seed_seconds_product_cycle(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    duration_seconds: u32,
    payout_rate: &str,
    min_stake: &str,
    max_stake: Option<&str>,
    sort_order: u32,
) {
    sqlx::query(
        r#"INSERT INTO seconds_contract_product_cycles
           (product_id, duration_seconds, payout_rate, min_stake, max_stake, sort_order)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(product_id)
    .bind(duration_seconds)
    .bind(decimal(payout_rate))
    .bind(decimal(min_stake))
    .bind(max_stake.map(decimal))
    .bind(sort_order)
    .execute(&mut **tx)
    .await
    .unwrap();
}

async fn body_json(response: axum::response::Response) -> Result<Value, Box<dyn Error>> {
    let body = axum::body::to_bytes(response.into_body(), 65_536).await?;
    Ok(serde_json::from_slice(&body)?)
}

#[tokio::test]
async fn seconds_contract_open_requires_fresh_positive_ticker_before_mutation()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut tx = pool.begin().await?;
    let user_id = create_user(&mut tx).await;
    let (base_asset, base_symbol) = create_asset(&mut tx, "NTB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut tx, "NTQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_seconds_product(&mut tx, pair_id, quote_asset).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("50.000000000000000000"))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();

    let no_redis = user_routes()
        .with_state(AppState::new(settings.clone()).with_mysql(pool.clone()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"product_id":{product_id},"direction":"up","stake_amount":"10.000000000000000000","idempotency_key":"seconds-no-redis-{}"}}"#,
                    Uuid::now_v7().simple()
                )))
                .unwrap(),
        )
        .await?;
    assert_eq!(no_redis.status(), StatusCode::BAD_REQUEST);

    if let Some(redis) = redis_manager().await {
        let mut connection = redis.clone();
        let _: usize = connection.del(market_ticker_redis_key(&symbol)).await?;
        let app = user_routes().with_state(
            AppState::new(settings)
                .with_mysql(pool.clone())
                .with_redis(redis.clone()),
        );

        let missing = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/seconds-contracts/orders")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"product_id":{product_id},"direction":"up","stake_amount":"10.000000000000000000","idempotency_key":"seconds-missing-ticker-{}"}}"#,
                        Uuid::now_v7().simple()
                    )))
                    .unwrap(),
            )
            .await?;
        assert_eq!(missing.status(), StatusCode::BAD_REQUEST);

        seed_ticker_at(
            &redis,
            &symbol,
            "100.000000000000000000",
            (chrono::Utc::now() - chrono::TimeDelta::seconds(61)).timestamp_millis(),
        )
        .await;
        let stale = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/seconds-contracts/orders")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"product_id":{product_id},"direction":"up","stake_amount":"10.000000000000000000","idempotency_key":"seconds-stale-ticker-{}"}}"#,
                        Uuid::now_v7().simple()
                    )))
                    .unwrap(),
            )
            .await?;
        assert_eq!(stale.status(), StatusCode::BAD_REQUEST);

        seed_ticker_at(
            &redis,
            &symbol,
            "0.000000000000000000",
            chrono::Utc::now().timestamp_millis(),
        )
        .await;
        let non_positive = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/seconds-contracts/orders")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"product_id":{product_id},"direction":"up","stake_amount":"10.000000000000000000","idempotency_key":"seconds-zero-ticker-{}"}}"#,
                        Uuid::now_v7().simple()
                    )))
                    .unwrap(),
            )
            .await?;
        assert_eq!(non_positive.status(), StatusCode::BAD_REQUEST);
    }

    let (order_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM seconds_contract_orders WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&pool)
            .await?;
    let (available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE user_id = ? AND ref_type = 'seconds_contract_order'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(order_count, 0);
    assert_eq!(available, decimal("50.000000000000000000"));
    assert_eq!(ledger_count, 0);
    Ok(())
}

#[tokio::test]
async fn seconds_contract_open_rejects_inactive_pair_or_assets_before_mutation()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut tx = pool.begin().await?;
    let user_id = create_user(&mut tx).await;
    let (base_asset, base_symbol) = create_asset(&mut tx, "ISB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut tx, "ISQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_seconds_product(&mut tx, pair_id, quote_asset).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("50.000000000000000000"))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = user_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    sqlx::query("UPDATE trading_pairs SET status = 'disabled' WHERE id = ?")
        .bind(pair_id)
        .execute(&pool)
        .await?;
    let inactive_pair = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"product_id":{product_id},"direction":"up","stake_amount":"10.000000000000000000","idempotency_key":"seconds-inactive-pair-{}"}}"#,
                    Uuid::now_v7().simple()
                )))
                .unwrap(),
        )
        .await?;
    assert_eq!(inactive_pair.status(), StatusCode::NOT_FOUND);

    sqlx::query("UPDATE trading_pairs SET status = 'active' WHERE id = ?")
        .bind(pair_id)
        .execute(&pool)
        .await?;
    sqlx::query("UPDATE assets SET status = 'disabled' WHERE id = ?")
        .bind(quote_asset)
        .execute(&pool)
        .await?;
    let inactive_stake_asset = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"product_id":{product_id},"direction":"up","stake_amount":"10.000000000000000000","idempotency_key":"seconds-inactive-stake-{}"}}"#,
                    Uuid::now_v7().simple()
                )))
                .unwrap(),
        )
        .await?;
    assert_eq!(inactive_stake_asset.status(), StatusCode::NOT_FOUND);

    sqlx::query("UPDATE assets SET status = 'active' WHERE id = ?")
        .bind(quote_asset)
        .execute(&pool)
        .await?;
    sqlx::query("UPDATE assets SET status = 'disabled' WHERE id = ?")
        .bind(base_asset)
        .execute(&pool)
        .await?;
    let inactive_pair_asset = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"product_id":{product_id},"direction":"up","stake_amount":"10.000000000000000000","idempotency_key":"seconds-inactive-base-{}"}}"#,
                    Uuid::now_v7().simple()
                )))
                .unwrap(),
        )
        .await?;
    assert_eq!(inactive_pair_asset.status(), StatusCode::NOT_FOUND);

    let (order_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM seconds_contract_orders WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&pool)
            .await?;
    let (available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(order_count, 0);
    assert_eq!(available, decimal("50.000000000000000000"));
    Ok(())
}

#[tokio::test]
async fn seconds_contract_stake_and_manual_payout_respect_asset_precision()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let Some(redis) = redis_manager().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut tx = pool.begin().await?;
    let user_id = create_user(&mut tx).await;
    let (base_asset, base_symbol) = create_asset(&mut tx, "PRB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut tx, "PRQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_seconds_product(&mut tx, pair_id, quote_asset).await;
    sqlx::query("UPDATE assets SET precision_scale = 2 WHERE id = ?")
        .bind(quote_asset)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE seconds_contract_products SET payout_rate = ? WHERE id = ?")
        .bind(decimal("0.33333333"))
        .bind(product_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("50.000000000000000000"))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let admin_id = create_admin(&pool).await;
    seed_ticker_at(
        &redis,
        &symbol,
        "100.000000000000000000",
        chrono::Utc::now().timestamp_millis(),
    )
    .await;

    let user_token =
        issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let user_app = user_routes().with_state(
        AppState::new(settings.clone())
            .with_mysql(pool.clone())
            .with_redis(redis),
    );

    let invalid = user_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/orders")
                .header("authorization", format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"product_id":{product_id},"direction":"up","stake_amount":"10.123","idempotency_key":"seconds-precision-invalid-{}"}}"#,
                    Uuid::now_v7().simple()
                )))
                .unwrap(),
        )
        .await?;
    let invalid_status = invalid.status();
    let invalid_payload = body_json(invalid).await?;
    assert_eq!(
        invalid_status,
        StatusCode::BAD_REQUEST,
        "payload: {invalid_payload}"
    );
    let (available_before_open,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(available_before_open, decimal("50.000000000000000000"));

    let opened = user_app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/orders")
                .header("authorization", format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"product_id":{product_id},"direction":"up","stake_amount":"10.12","idempotency_key":"seconds-precision-open-{}"}}"#,
                    Uuid::now_v7().simple()
                )))
                .unwrap(),
        )
        .await?;
    let opened_status = opened.status();
    let opened_payload = body_json(opened).await?;
    assert_eq!(opened_status, StatusCode::OK, "payload: {opened_payload}");
    let order_id = opened_payload["order"]["id"].as_u64().unwrap();

    let settled = admin_routes()
        .with_state(AppState::new(settings).with_mysql(pool.clone()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/seconds-contracts/orders/{order_id}/settle"))
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"result":"win","reason":"precision regression settlement"}"#,
                ))
                .unwrap(),
        )
        .await?;
    let settled_status = settled.status();
    let settled_payload = body_json(settled).await?;
    assert_eq!(settled_status, StatusCode::OK, "payload: {settled_payload}");
    assert_eq!(
        decimal(settled_payload["payout_amount"].as_str().unwrap()).normalized(),
        decimal("13.49").normalized()
    );

    let (available_after,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    let (payout_amount,): (BigDecimal,) = sqlx::query_as(
        "SELECT amount FROM wallet_ledger WHERE ref_type = 'seconds_contract_order' AND ref_id = ? AND change_type = 'seconds_contract_settle_win'",
    )
    .bind(order_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(available_after.normalized(), decimal("53.37").normalized());
    assert_eq!(payout_amount.normalized(), decimal("13.49").normalized());
    Ok(())
}

#[tokio::test]
async fn seconds_contract_routes_require_expected_scope() {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:42", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:7", TokenScope::Admin, 900).unwrap();

    let unauthenticated_open = user_routes()
        .with_state(AppState::new(settings.clone()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/orders")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"product_id":1,"direction":"up","stake_amount":"10.000000000000000000","idempotency_key":"scope-test"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthenticated_open.status(), StatusCode::UNAUTHORIZED);

    let user_on_admin = admin_routes()
        .with_state(AppState::new(settings.clone()))
        .oneshot(
            Request::builder()
                .uri("/seconds-contracts/products")
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(user_on_admin.status(), StatusCode::FORBIDDEN);

    let admin_on_user_open = user_routes()
        .with_state(AppState::new(settings.clone()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/orders")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"product_id":1,"direction":"up","stake_amount":"10.000000000000000000","idempotency_key":"scope-test"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(admin_on_user_open.status(), StatusCode::FORBIDDEN);

    let user_on_admin_settle = admin_routes()
        .with_state(AppState::new(settings.clone()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/orders/1/settle")
                .header("authorization", format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"result":"win","reason":"manual settle win"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(user_on_admin_settle.status(), StatusCode::FORBIDDEN);

    let user_on_admin_create = admin_routes()
        .with_state(AppState::new(settings))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/products")
                .header("authorization", format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"pair_id":1,"stake_asset":1,"duration_seconds":60,"payout_rate":"0.80000000","min_stake":"10.000000000000000000"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(user_on_admin_create.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn seconds_contract_routes_return_clear_error_without_mysql() {
    let settings = test_settings();
    let token = issue_token(&settings, "user:42", TokenScope::User, 900).unwrap();
    let response = user_routes()
        .with_state(AppState::new(settings))
        .oneshot(
            Request::builder()
                .uri("/seconds-contracts/products")
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
        "internal error: mysql pool is not configured for seconds contract routes"
    );
}

#[tokio::test]
async fn seconds_contract_lists_active_products_for_user_and_all_products_for_admin()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let admin_id = create_admin(&pool).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "SB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "SQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let active_product_id = seed_seconds_product(&mut fixture_tx, pair_id, quote_asset).await;
    let disabled_product_id = seed_seconds_product(&mut fixture_tx, pair_id, quote_asset).await;
    sqlx::query("UPDATE seconds_contract_products SET status = 'disabled' WHERE id = ?")
        .bind(disabled_product_id)
        .execute(&mut *fixture_tx)
        .await?;
    fixture_tx.commit().await?;

    let user_token =
        issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let state = state_with_mysql_and_redis(settings, pool.clone(), None);

    let user_response = user_routes()
        .with_state(state.clone())
        .oneshot(
            Request::builder()
                .uri("/seconds-contracts/products")
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let user_status = user_response.status();
    let user_body = axum::body::to_bytes(user_response.into_body(), 65_536).await?;
    assert_eq!(
        user_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&user_body)
    );
    let user_payload: Value = serde_json::from_slice(&user_body)?;
    assert!(
        user_payload["products"]
            .as_array()
            .unwrap()
            .iter()
            .any(|product| { product["id"] == active_product_id && product["symbol"] == symbol })
    );
    assert!(
        !user_payload["products"]
            .as_array()
            .unwrap()
            .iter()
            .any(|product| { product["id"] == disabled_product_id })
    );

    let admin_response = admin_routes()
        .with_state(state)
        .oneshot(
            Request::builder()
                .uri("/seconds-contracts/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let admin_status = admin_response.status();
    let admin_body = axum::body::to_bytes(admin_response.into_body(), 65_536).await?;
    let admin_payload: Value = serde_json::from_slice(&admin_body)?;
    assert_eq!(
        admin_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&admin_body)
    );
    assert!(
        admin_payload["products"]
            .as_array()
            .unwrap()
            .iter()
            .any(|product| { product["id"] == active_product_id })
    );
    assert!(
        admin_payload["products"]
            .as_array()
            .unwrap()
            .iter()
            .any(|product| { product["id"] == disabled_product_id })
    );

    Ok(())
}

#[tokio::test]
async fn admin_seconds_contract_product_routes_require_admin_scope_mysql_and_validation()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:42", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = admin_routes().with_state(AppState::new(settings));

    let unauthenticated_detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/seconds-contracts/products/1")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(unauthenticated_detail.status(), StatusCode::UNAUTHORIZED);

    let user_detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/seconds-contracts/products/1")
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(user_detail.status(), StatusCode::FORBIDDEN);

    let admin_detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/seconds-contracts/products/1")
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let admin_detail_status = admin_detail.status();
    let admin_detail_payload = body_json(admin_detail).await?;
    assert_eq!(admin_detail_status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(admin_detail_payload["code"], "INTERNAL_ERROR");
    assert_eq!(
        admin_detail_payload["message"],
        "internal error: mysql pool is not configured for seconds contract routes"
    );

    let blank_create_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"pair_id":1,"stake_asset":1,"duration_seconds":60,"payout_rate":"0.80000000","min_stake":"10.000000000000000000","reason":"   "}"#,
                ))
                .unwrap(),
        )
        .await?;
    let blank_create_reason_status = blank_create_reason.status();
    let blank_create_reason_payload = body_json(blank_create_reason).await?;
    assert_eq!(blank_create_reason_status, StatusCode::BAD_REQUEST);
    assert_eq!(blank_create_reason_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        blank_create_reason_payload["message"],
        "validation error: seconds contract reason is required"
    );

    let no_mysql = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"pair_id":1,"stake_asset":1,"duration_seconds":60,"payout_rate":"0.80000000","min_stake":"10.000000000000000000","reason":"create seconds product"}"#,
                ))
                .unwrap(),
        )
        .await?;
    let no_mysql_status = no_mysql.status();
    let no_mysql_payload = body_json(no_mysql).await?;
    assert_eq!(no_mysql_status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(no_mysql_payload["code"], "INTERNAL_ERROR");
    assert_eq!(
        no_mysql_payload["message"],
        "internal error: mysql pool is not configured for seconds contract routes"
    );

    let update_body = r#"{"pair_id":1,"stake_asset":1,"duration_seconds":60,"payout_rate":"0.80000000","min_stake":"10.000000000000000000","status":"active","reason":"update seconds product"}"#;
    let unauthenticated_update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/seconds-contracts/products/1")
                .header("content-type", "application/json")
                .body(Body::from(update_body))
                .unwrap(),
        )
        .await?;
    assert_eq!(unauthenticated_update.status(), StatusCode::UNAUTHORIZED);

    let user_update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/seconds-contracts/products/1")
                .header("authorization", format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(update_body))
                .unwrap(),
        )
        .await?;
    assert_eq!(user_update.status(), StatusCode::FORBIDDEN);

    let blank_update_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/seconds-contracts/products/1")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"pair_id":1,"stake_asset":1,"duration_seconds":60,"payout_rate":"0.80000000","min_stake":"10.000000000000000000","status":"active","reason":"   "}"#,
                ))
                .unwrap(),
        )
        .await?;
    let blank_update_reason_status = blank_update_reason.status();
    let blank_update_reason_payload = body_json(blank_update_reason).await?;
    assert_eq!(blank_update_reason_status, StatusCode::BAD_REQUEST);
    assert_eq!(blank_update_reason_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        blank_update_reason_payload["message"],
        "validation error: seconds contract reason is required"
    );

    let invalid_update_status = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/seconds-contracts/products/1")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"pair_id":1,"stake_asset":1,"duration_seconds":60,"payout_rate":"0.80000000","min_stake":"10.000000000000000000","status":"archived","reason":"invalid status test"}"#,
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid_update_status.status(), StatusCode::BAD_REQUEST);

    let no_mysql_update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/seconds-contracts/products/1")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(update_body))
                .unwrap(),
        )
        .await?;
    let no_mysql_update_status = no_mysql_update.status();
    let no_mysql_update_payload = body_json(no_mysql_update).await?;
    assert_eq!(no_mysql_update_status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(no_mysql_update_payload["code"], "INTERNAL_ERROR");
    assert_eq!(
        no_mysql_update_payload["message"],
        "internal error: mysql pool is not configured for seconds contract routes"
    );

    let blank_status_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/seconds-contracts/products/1/status")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"status":"active","reason":"   "}"#))
                .unwrap(),
        )
        .await?;
    let blank_status_reason_status = blank_status_reason.status();
    let blank_status_reason_payload = body_json(blank_status_reason).await?;
    assert_eq!(blank_status_reason_status, StatusCode::BAD_REQUEST);
    assert_eq!(blank_status_reason_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        blank_status_reason_payload["message"],
        "validation error: seconds contract reason is required"
    );

    let invalid_status = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/seconds-contracts/products/1/status")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"status":"archived","reason":"invalid status test"}"#,
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid_status.status(), StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn admin_seconds_contract_order_routes_require_admin_scope_mysql_and_validation()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:42", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = admin_routes().with_state(AppState::new(settings));

    let unauthenticated_detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/seconds-contracts/orders/1")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(unauthenticated_detail.status(), StatusCode::UNAUTHORIZED);

    let user_detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/seconds-contracts/orders/1")
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(user_detail.status(), StatusCode::FORBIDDEN);

    let admin_detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/seconds-contracts/orders/1")
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let admin_detail_status = admin_detail.status();
    let admin_detail_payload = body_json(admin_detail).await?;
    assert_eq!(admin_detail_status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(admin_detail_payload["code"], "INTERNAL_ERROR");
    assert_eq!(
        admin_detail_payload["message"],
        "internal error: mysql pool is not configured for seconds contract routes"
    );

    let blank_settle_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/orders/1/settle")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"result":"win","reason":"   "}"#))
                .unwrap(),
        )
        .await?;
    let blank_settle_reason_status = blank_settle_reason.status();
    let blank_settle_reason_payload = body_json(blank_settle_reason).await?;
    assert_eq!(blank_settle_reason_status, StatusCode::BAD_REQUEST);
    assert_eq!(blank_settle_reason_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        blank_settle_reason_payload["message"],
        "validation error: seconds contract reason is required"
    );

    Ok(())
}

#[tokio::test]
async fn admin_seconds_contract_product_rejects_unsafe_fields_before_mysql()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = admin_routes().with_state(AppState::new(settings));

    let unsafe_duration = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"pair_id":1,"stake_asset":1,"duration_seconds":0,"payout_rate":"0.80000000","min_stake":"10.000000000000000000"}"#,
                ))
                .unwrap(),
        )
        .await?;
    let unsafe_duration_status = unsafe_duration.status();
    let unsafe_duration_payload = body_json(unsafe_duration).await?;
    assert_eq!(unsafe_duration_status, StatusCode::BAD_REQUEST);
    assert_eq!(unsafe_duration_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        unsafe_duration_payload["message"],
        "validation error: seconds contract duration_seconds must be positive"
    );

    let overflow_payout = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"pair_id":1,"stake_asset":1,"duration_seconds":60,"payout_rate":"10000000000.00000000","min_stake":"10.000000000000000000"}"#,
                ))
                .unwrap(),
        )
        .await?;
    let overflow_payout_status = overflow_payout.status();
    let overflow_payout_payload = body_json(overflow_payout).await?;
    assert_eq!(overflow_payout_status, StatusCode::BAD_REQUEST);
    assert_eq!(overflow_payout_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        overflow_payout_payload["message"],
        "validation error: seconds contract payout_rate exceeds decimal storage precision"
    );

    let scale_payout = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"pair_id":1,"stake_asset":1,"duration_seconds":60,"payout_rate":"0.123456789","min_stake":"10.000000000000000000"}"#,
                ))
                .unwrap(),
        )
        .await?;
    let scale_payout_status = scale_payout.status();
    let scale_payout_payload = body_json(scale_payout).await?;
    assert_eq!(scale_payout_status, StatusCode::BAD_REQUEST);
    assert_eq!(scale_payout_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        scale_payout_payload["message"],
        "validation error: seconds contract payout_rate supports at most 8 decimal places"
    );

    let invalid_update_bounds = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/seconds-contracts/products/1")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"pair_id":1,"stake_asset":1,"duration_seconds":60,"payout_rate":"0.80000000","min_stake":"20.000000000000000000","max_stake":"10.000000000000000000","status":"active","reason":"invalid edit bounds"}"#,
                ))
                .unwrap(),
        )
        .await?;
    let invalid_update_bounds_status = invalid_update_bounds.status();
    let invalid_update_bounds_payload = body_json(invalid_update_bounds).await?;
    assert_eq!(invalid_update_bounds_status, StatusCode::BAD_REQUEST);
    assert_eq!(invalid_update_bounds_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        invalid_update_bounds_payload["message"],
        "validation error: seconds contract max_stake must be greater than or equal to min_stake"
    );

    let long_reason = "R".repeat(513);
    let long_reason_body = format!(
        r#"{{"pair_id":1,"stake_asset":1,"duration_seconds":60,"payout_rate":"0.80000000","min_stake":"10.000000000000000000","reason":"{long_reason}"}}"#
    );
    let long_reason_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(long_reason_body))
                .unwrap(),
        )
        .await?;
    let long_reason_status = long_reason_response.status();
    let long_reason_payload = body_json(long_reason_response).await?;
    assert_eq!(long_reason_status, StatusCode::BAD_REQUEST);
    assert_eq!(long_reason_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        long_reason_payload["message"],
        "validation error: seconds contract reason is too long"
    );

    Ok(())
}

#[tokio::test]
async fn admin_seconds_contract_product_create_update_status_and_audit()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let admin_id = create_admin(&pool).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "AB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "AQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    fixture_tx.commit().await?;

    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = admin_routes().with_state(state_with_mysql_and_redis(settings, pool.clone(), None));

    let missing_pair_body = format!(
        r#"{{"pair_id":999999999999,"stake_asset":{quote_asset},"duration_seconds":60,"payout_rate":"0.80000000","min_stake":"10.000000000000000000","reason":"missing pair test"}}"#
    );
    let missing_pair_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(missing_pair_body))
                .unwrap(),
        )
        .await?;
    let missing_pair_status = missing_pair_response.status();
    let missing_pair_payload = body_json(missing_pair_response).await?;
    assert_eq!(missing_pair_status, StatusCode::NOT_FOUND);
    assert_eq!(missing_pair_payload["code"], "NOT_FOUND");

    let create_body = format!(
        r#"{{"pair_id":{pair_id},"stake_asset":{quote_asset},"cycles":[{{"duration_seconds":90,"payout_rate":"0.75000000","min_stake":"15.000000000000000000","max_stake":"150.000000000000000000"}},{{"duration_seconds":120,"payout_rate":"0.82000000","min_stake":"20.000000000000000000","max_stake":null}}],"status":"active","reason":"launch seconds product"}}"#
    );
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(create_body))
                .unwrap(),
        )
        .await?;
    let create_status = create_response.status();
    let create_payload = body_json(create_response).await?;
    assert_eq!(create_status, StatusCode::OK, "payload: {create_payload}");
    let product_id = create_payload["id"].as_u64().unwrap();
    assert_eq!(create_payload["pair_id"], pair_id);
    assert_eq!(create_payload["symbol"], symbol);
    assert_eq!(create_payload["stake_asset"], quote_asset);
    assert_eq!(create_payload["stake_asset_symbol"], quote_symbol);
    assert_eq!(create_payload["duration_seconds"], 90);
    assert_eq!(create_payload["payout_rate"], "0.75000000");
    assert_eq!(create_payload["cycles"].as_array().unwrap().len(), 2);
    assert_eq!(create_payload["cycles"][0]["duration_seconds"], 90);
    assert_eq!(create_payload["cycles"][1]["duration_seconds"], 120);
    assert_eq!(create_payload["status"], "active");

    let detail_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/seconds-contracts/products/{product_id}"))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let detail_status = detail_response.status();
    let detail_payload = body_json(detail_response).await?;
    assert_eq!(detail_status, StatusCode::OK, "payload: {detail_payload}");
    assert_eq!(detail_payload["id"], product_id);
    assert_eq!(detail_payload["pair_id"], pair_id);
    assert_eq!(detail_payload["symbol"], symbol);
    assert_eq!(detail_payload["stake_asset"], quote_asset);
    assert_eq!(detail_payload["stake_asset_symbol"], quote_symbol);
    assert_eq!(detail_payload["cycles"].as_array().unwrap().len(), 2);
    assert_eq!(detail_payload["status"], "active");

    let unknown_detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/seconds-contracts/products/999999999999")
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(unknown_detail.status(), StatusCode::NOT_FOUND);

    let edit_body = format!(
        r#"{{"pair_id":{pair_id},"stake_asset":{quote_asset},"cycles":[{{"duration_seconds":120,"payout_rate":"0.82000000","min_stake":"20.000000000000000000","max_stake":null}},{{"duration_seconds":300,"payout_rate":"0.95000000","min_stake":"30.000000000000000000","max_stake":"300.000000000000000000"}}],"status":"active","reason":"adjust seconds product"}}"#
    );
    let edit_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/seconds-contracts/products/{product_id}"))
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(edit_body))
                .unwrap(),
        )
        .await?;
    let edit_status = edit_response.status();
    let edit_payload = body_json(edit_response).await?;
    assert_eq!(edit_status, StatusCode::OK, "payload: {edit_payload}");
    assert_eq!(edit_payload["id"], product_id);
    assert_eq!(edit_payload["pair_id"], pair_id);
    assert_eq!(edit_payload["stake_asset"], quote_asset);
    assert_eq!(edit_payload["duration_seconds"], 120);
    assert_eq!(edit_payload["payout_rate"], "0.82000000");
    assert_eq!(edit_payload["min_stake"], "20.000000000000000000");
    assert!(edit_payload["max_stake"].is_null());
    assert_eq!(edit_payload["cycles"].as_array().unwrap().len(), 2);
    assert_eq!(edit_payload["cycles"][0]["duration_seconds"], 120);
    assert_eq!(edit_payload["cycles"][1]["duration_seconds"], 300);
    assert_eq!(edit_payload["status"], "active");

    let long_update_reason = "R".repeat(513);
    let long_update_reason_body =
        format!(r#"{{"status":"disabled","reason":"{long_update_reason}"}}"#);
    let long_update_reason_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/seconds-contracts/products/{product_id}/status"))
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(long_update_reason_body))
                .unwrap(),
        )
        .await?;
    let long_update_reason_status = long_update_reason_response.status();
    let long_update_reason_payload = body_json(long_update_reason_response).await?;
    assert_eq!(long_update_reason_status, StatusCode::BAD_REQUEST);
    assert_eq!(long_update_reason_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        long_update_reason_payload["message"],
        "validation error: seconds contract reason is too long"
    );

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/seconds-contracts/products/{product_id}/status"))
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"status":"disabled","reason":"pause seconds product"}"#,
                ))
                .unwrap(),
        )
        .await?;
    let update_status = update_response.status();
    let update_payload = body_json(update_response).await?;
    assert_eq!(update_status, StatusCode::OK, "payload: {update_payload}");
    assert_eq!(update_payload["id"], product_id);
    assert_eq!(update_payload["status"], "disabled");

    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/seconds-contracts/products/{product_id}"))
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"reason":"remove disabled seconds product"}"#,
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    let product_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM seconds_contract_products WHERE id = ?")
            .bind(product_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(product_count, 0);

    let audit_rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT action, target_type, reason FROM admin_audit_logs WHERE admin_id = ? AND target_id = ? ORDER BY id",
    )
    .bind(admin_id)
    .bind(product_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audit_rows.len(), 4);
    assert_eq!(audit_rows[0].0, "seconds_contract_product.create");
    assert_eq!(audit_rows[0].1, "seconds_contract_product");
    assert_eq!(audit_rows[0].2, "launch seconds product");
    assert_eq!(audit_rows[1].0, "seconds_contract_product.update");
    assert_eq!(audit_rows[1].1, "seconds_contract_product");
    assert_eq!(audit_rows[1].2, "adjust seconds product");
    assert_eq!(audit_rows[2].0, "seconds_contract_product.update_status");
    assert_eq!(audit_rows[2].1, "seconds_contract_product");
    assert_eq!(audit_rows[2].2, "pause seconds product");
    assert_eq!(audit_rows[3].0, "seconds_contract_product.delete");
    assert_eq!(audit_rows[3].1, "seconds_contract_product");
    assert_eq!(audit_rows[3].2, "remove disabled seconds product");

    Ok(())
}

#[tokio::test]
async fn admin_seconds_contract_product_delete_requires_disabled_and_no_orders()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let admin_id = create_admin(&pool).await;
    let user_id = create_user(&mut fixture_tx).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "RD").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "RQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_seconds_product(&mut fixture_tx, pair_id, quote_asset).await;
    fixture_tx.commit().await?;

    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = admin_routes().with_state(state_with_mysql_and_redis(settings, pool.clone(), None));

    let active_delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/seconds-contracts/products/{product_id}"))
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"reason":"delete active product"}"#))
                .unwrap(),
        )
        .await?;
    let active_delete_status = active_delete_response.status();
    let active_delete_payload = body_json(active_delete_response).await?;
    assert_eq!(active_delete_status, StatusCode::BAD_REQUEST);
    assert_eq!(active_delete_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        active_delete_payload["message"],
        "validation error: seconds contract product must be disabled before deletion"
    );

    sqlx::query("UPDATE seconds_contract_products SET status = 'disabled' WHERE id = ?")
        .bind(product_id)
        .execute(&pool)
        .await?;
    sqlx::query(
        r#"INSERT INTO seconds_contract_orders
           (user_id, product_id, pair_id, stake_asset, direction, stake_amount,
            payout_rate, status, idempotency_key, expires_at)
           VALUES (?, ?, ?, ?, 'up', ?, ?, 'settled', ?, CURRENT_TIMESTAMP(6))"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(quote_asset)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("0.80000000"))
    .bind(format!("seconds-delete-guard-{}", Uuid::now_v7().simple()))
    .execute(&pool)
    .await?;

    let referenced_delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/seconds-contracts/products/{product_id}"))
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"reason":"delete referenced product"}"#))
                .unwrap(),
        )
        .await?;
    let referenced_delete_status = referenced_delete_response.status();
    let referenced_delete_payload = body_json(referenced_delete_response).await?;
    assert_eq!(referenced_delete_status, StatusCode::BAD_REQUEST);
    assert_eq!(referenced_delete_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        referenced_delete_payload["message"],
        "validation error: seconds contract product with orders cannot be deleted"
    );

    let product_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM seconds_contract_products WHERE id = ?")
            .bind(product_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(product_count, 1);

    Ok(())
}

#[tokio::test]
async fn admin_seconds_contract_product_create_rolls_back_when_audit_fails()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "XB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "XQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    fixture_tx.commit().await?;

    let admin_token = issue_token(&settings, "admin:999999999", TokenScope::Admin, 900).unwrap();
    let body = format!(
        r#"{{"pair_id":{pair_id},"stake_asset":{quote_asset},"duration_seconds":60,"payout_rate":"0.80000000","min_stake":"10.000000000000000000","reason":"audit should fail"}}"#
    );
    let response = admin_routes()
        .with_state(state_with_mysql_and_redis(settings, pool.clone(), None))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await?;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let (product_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM seconds_contract_products WHERE pair_id = ? AND stake_asset = ?",
    )
    .bind(pair_id)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(product_count, 0);

    Ok(())
}

#[tokio::test]
async fn seconds_contract_open_order_does_not_replay_foreign_key_failures()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let Some(redis) = redis_manager().await else {
        return Ok(());
    };
    let redis = Some(redis);
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "FB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "FQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_seconds_product(&mut fixture_tx, pair_id, quote_asset).await;
    fixture_tx.commit().await?;
    seed_ticker(&redis, &symbol, "100.000000000000000000").await;

    let token = issue_token(&settings, "user:999999999", TokenScope::User, 900).unwrap();
    let idempotency_key = format!("seconds-fk-{}", Uuid::now_v7().simple());
    let request_body = format!(
        r#"{{"product_id":{product_id},"direction":"up","stake_amount":"10.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );

    let response = user_routes()
        .with_state(state_with_mysql_and_redis(settings, pool.clone(), redis))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 65_536).await?;
    let payload: Value = serde_json::from_slice(&body)?;

    assert_eq!(
        status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );
    assert_eq!(payload["code"], "DATABASE_ERROR");
    assert_ne!(
        payload["message"],
        "conflict: seconds contract idempotency key is being committed"
    );

    Ok(())
}

#[tokio::test]
async fn seconds_contract_lists_current_user_orders_with_timestamp() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let other_user_id = create_user(&mut fixture_tx).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "LB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "LQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_seconds_product(&mut fixture_tx, pair_id, quote_asset).await;

    let first_order_id = sqlx::query(
        r#"INSERT INTO seconds_contract_orders
           (user_id, product_id, pair_id, stake_asset, direction, stake_amount,
            payout_rate, entry_price, status, idempotency_key, opened_at, expires_at, created_at)
           VALUES (?, ?, ?, ?, 'up', ?, ?, ?, 'opened', ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(quote_asset)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("0.80000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(format!("seconds-list-first-{}", Uuid::now_v7().simple()))
    .bind("2026-05-30 04:00:00.000000")
    .bind("2026-05-30 04:01:00.000000")
    .bind("2026-05-30 04:00:00.000000")
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    let second_order_id = sqlx::query(
        r#"INSERT INTO seconds_contract_orders
           (user_id, product_id, pair_id, stake_asset, direction, stake_amount,
            payout_rate, entry_price, status, idempotency_key, opened_at, expires_at, created_at)
           VALUES (?, ?, ?, ?, 'down', ?, ?, ?, 'opened', ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(quote_asset)
    .bind(decimal("20.000000000000000000"))
    .bind(decimal("0.80000000"))
    .bind(decimal("101.000000000000000000"))
    .bind(format!("seconds-list-second-{}", Uuid::now_v7().simple()))
    .bind("2026-05-30 05:00:00.000000")
    .bind("2026-05-30 05:01:00.000000")
    .bind("2026-05-30 05:00:00.000000")
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    let other_order_id = sqlx::query(
        r#"INSERT INTO seconds_contract_orders
           (user_id, product_id, pair_id, stake_asset, direction, stake_amount,
            payout_rate, entry_price, status, idempotency_key, opened_at, expires_at, created_at)
           VALUES (?, ?, ?, ?, 'up', ?, ?, ?, 'opened', ?, ?, ?, ?)"#,
    )
    .bind(other_user_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(quote_asset)
    .bind(decimal("30.000000000000000000"))
    .bind(decimal("0.80000000"))
    .bind(decimal("102.000000000000000000"))
    .bind(format!("seconds-list-other-{}", Uuid::now_v7().simple()))
    .bind("2026-05-30 06:00:00.000000")
    .bind("2026-05-30 06:01:00.000000")
    .bind("2026-05-30 06:00:00.000000")
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    fixture_tx.commit().await?;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let response = user_routes()
        .with_state(state_with_mysql_and_redis(settings, pool.clone(), None))
        .oneshot(
            Request::builder()
                .uri("/seconds-contracts/orders?limit=10")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
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
    let orders = payload["orders"].as_array().unwrap();
    let order_ids: Vec<u64> = orders
        .iter()
        .map(|order| order["id"].as_u64().unwrap())
        .collect();
    assert_eq!(order_ids, vec![second_order_id, first_order_id]);
    assert!(!order_ids.contains(&other_order_id));
    assert_eq!(orders[0]["direction"], "down");
    assert_eq!(orders[0]["symbol"], symbol);
    assert_eq!(orders[0]["stake_asset_symbol"], quote_symbol);
    assert_eq!(orders[0]["stake_amount"], "20.000000000000000000");
    assert!(orders[0]["expires_at"].is_number());
    assert!(orders[0]["created_at"].is_number());

    Ok(())
}

#[tokio::test]
async fn admin_seconds_contract_lists_orders_with_filters_and_timestamp()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let admin_id = create_admin(&pool).await;
    let mut fixture_tx = pool.begin().await?;
    sqlx::query(
        "DELETE FROM seconds_contract_orders WHERE idempotency_key LIKE 'seconds-admin-list-%'",
    )
    .execute(&mut *fixture_tx)
    .await?;
    let user_email = format!(
        "seconds-admin-filter-{}@example.test",
        Uuid::now_v7().simple()
    );
    let user_id = create_user_with_email(&mut fixture_tx, user_email.clone()).await;
    let other_user_id = create_user(&mut fixture_tx).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "AB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "AQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_seconds_product(&mut fixture_tx, pair_id, quote_asset).await;

    let first_open_order_id = sqlx::query(
        r#"INSERT INTO seconds_contract_orders
           (user_id, product_id, pair_id, stake_asset, direction, stake_amount,
            payout_rate, entry_price, status, idempotency_key, opened_at, expires_at, created_at)
           VALUES (?, ?, ?, ?, 'up', ?, ?, ?, 'opened', ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(quote_asset)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("0.80000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(format!(
        "seconds-admin-list-first-{}",
        Uuid::now_v7().simple()
    ))
    .bind("2037-01-01 04:00:00.000000")
    .bind("2037-01-01 04:01:00.000000")
    .bind("2037-01-01 04:00:00.000000")
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    let second_open_order_id = sqlx::query(
        r#"INSERT INTO seconds_contract_orders
           (user_id, product_id, pair_id, stake_asset, direction, stake_amount,
            payout_rate, entry_price, status, idempotency_key, opened_at, expires_at, created_at)
           VALUES (?, ?, ?, ?, 'down', ?, ?, ?, 'opened', ?, ?, ?, ?)"#,
    )
    .bind(other_user_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(quote_asset)
    .bind(decimal("20.000000000000000000"))
    .bind(decimal("0.80000000"))
    .bind(decimal("101.000000000000000000"))
    .bind(format!(
        "seconds-admin-list-second-{}",
        Uuid::now_v7().simple()
    ))
    .bind("2037-01-01 05:00:00.000000")
    .bind("2037-01-01 05:01:00.000000")
    .bind("2037-01-01 05:00:00.000000")
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    let other_settled_order_id = sqlx::query(
        r#"INSERT INTO seconds_contract_orders
           (user_id, product_id, pair_id, stake_asset, direction, stake_amount,
            payout_rate, entry_price, status, result, idempotency_key,
            opened_at, expires_at, settled_at, created_at)
           VALUES (?, ?, ?, ?, 'down', ?, ?, ?, 'settled', 'lose', ?, ?, ?, ?, ?)"#,
    )
    .bind(other_user_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(quote_asset)
    .bind(decimal("25.000000000000000000"))
    .bind(decimal("0.80000000"))
    .bind(decimal("101.500000000000000000"))
    .bind(format!(
        "seconds-admin-list-other-settled-{}",
        Uuid::now_v7().simple()
    ))
    .bind("2037-01-01 05:30:00.000000")
    .bind("2037-01-01 05:31:00.000000")
    .bind("2037-01-01 05:31:00.000000")
    .bind("2037-01-01 05:30:00.000000")
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    let settled_order_id = sqlx::query(
        r#"INSERT INTO seconds_contract_orders
           (user_id, product_id, pair_id, stake_asset, direction, stake_amount,
            payout_rate, entry_price, settlement_price, status, result, idempotency_key,
            opened_at, expires_at, settled_at, created_at)
           VALUES (?, ?, ?, ?, 'up', ?, ?, ?, ?, 'settled', 'win', ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(quote_asset)
    .bind(decimal("30.000000000000000000"))
    .bind(decimal("0.80000000"))
    .bind(decimal("102.000000000000000000"))
    .bind(decimal("108.000000000000000000"))
    .bind(format!(
        "seconds-admin-list-settled-{}",
        Uuid::now_v7().simple()
    ))
    .bind("2037-01-01 06:00:00.000000")
    .bind("2037-01-01 06:01:00.000000")
    .bind("2037-01-01 06:01:00.000000")
    .bind("2037-01-01 06:00:00.000000")
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    fixture_tx.commit().await?;

    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let user_token =
        issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = admin_routes().with_state(state_with_mysql_and_redis(settings, pool.clone(), None));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/seconds-contracts/orders?status=opened&limit=2")
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
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
    let orders = payload["orders"].as_array().unwrap();
    let order_ids: Vec<u64> = orders
        .iter()
        .map(|order| order["id"].as_u64().unwrap())
        .collect();
    assert_eq!(order_ids, vec![second_open_order_id, first_open_order_id]);
    assert_eq!(orders[0]["user_id"], other_user_id);
    assert!(orders[0]["email"].as_str().is_some());
    assert_eq!(orders[0]["symbol"], symbol);
    assert_eq!(orders[0]["settlement_price"], Value::Null);
    assert_eq!(orders[0]["direction"], "down");
    assert_eq!(orders[0]["stake_amount"], "20.000000000000000000");
    assert!(orders[0]["expires_at"].is_number());
    assert!(orders[0]["created_at"].is_number());
    assert!(!order_ids.contains(&settled_order_id));

    let filtered_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/seconds-contracts/orders?email={user_email}&status=settled&limit=10"
                ))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let filtered_status = filtered_response.status();
    let filtered_body = axum::body::to_bytes(filtered_response.into_body(), 65_536).await?;
    assert_eq!(
        filtered_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&filtered_body)
    );
    let filtered_payload: Value = serde_json::from_slice(&filtered_body)?;
    let filtered_orders = filtered_payload["orders"].as_array().unwrap();
    assert_eq!(filtered_orders.len(), 1);
    assert_eq!(filtered_orders[0]["id"], settled_order_id);
    assert!(
        !filtered_orders
            .iter()
            .any(|order| order["id"] == other_settled_order_id)
    );
    assert_eq!(filtered_orders[0]["user_id"], user_id);
    assert_eq!(filtered_orders[0]["email"], user_email);
    assert_eq!(filtered_orders[0]["symbol"], symbol);
    assert_eq!(
        filtered_orders[0]["settlement_price"],
        "108.000000000000000000"
    );
    assert_eq!(filtered_orders[0]["status"], "settled");
    assert_eq!(filtered_orders[0]["result"], "win");
    assert!(filtered_orders[0]["expires_at"].is_number());
    assert!(filtered_orders[0]["created_at"].is_number());

    let detail_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/seconds-contracts/orders/{settled_order_id}"))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let detail_status = detail_response.status();
    let detail_payload = body_json(detail_response).await?;
    assert_eq!(detail_status, StatusCode::OK, "payload: {detail_payload}");
    assert_eq!(detail_payload["id"], settled_order_id);
    assert_eq!(detail_payload["user_id"], user_id);
    assert_eq!(detail_payload["product_id"], product_id);
    assert_eq!(detail_payload["pair_id"], pair_id);
    assert_eq!(detail_payload["email"], user_email);
    assert_eq!(detail_payload["symbol"], symbol);
    assert_eq!(detail_payload["stake_asset"], quote_asset);
    assert_eq!(detail_payload["stake_asset_symbol"], quote_symbol);
    assert_eq!(detail_payload["settlement_price"], "108.000000000000000000");
    assert_eq!(detail_payload["status"], "settled");
    assert_eq!(detail_payload["result"], "win");
    assert!(detail_payload["expires_at"].is_number());
    assert!(detail_payload["created_at"].is_number());

    let unknown_detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/seconds-contracts/orders/999999999999")
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(unknown_detail.status(), StatusCode::NOT_FOUND);

    let forbidden_response = app
        .oneshot(
            Request::builder()
                .uri("/seconds-contracts/orders?limit=1")
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(forbidden_response.status(), StatusCode::FORBIDDEN);

    Ok(())
}

#[tokio::test]
async fn seconds_contract_open_order_debits_wallet_and_writes_ledger() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let Some(redis) = redis_manager().await else {
        return Ok(());
    };
    let redis = Some(redis);
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "OB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "OQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_seconds_product(&mut fixture_tx, pair_id, quote_asset).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("50.000000000000000000"))
        .execute(&mut *fixture_tx)
        .await?;
    fixture_tx.commit().await?;
    let commission_fixture =
        support::seed_direct_agent_commission(&pool, user_id, "seconds_contract", "0.05000000")
            .await?;
    seed_ticker(&redis, &symbol, "100.000000000000000000").await;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("seconds-open-{}", Uuid::now_v7().simple());
    let hub = EventBroadcastHub::new(16);
    let _keepalive_hub = hub.clone();
    let mut private_events = hub.subscribe(&WebSocketChannel::private_user(user_id));
    let app = user_routes().with_state(state_with_mysql_redis_and_events(
        settings,
        pool.clone(),
        redis,
        hub,
    ));
    let request_body = format!(
        r#"{{"product_id":{product_id},"direction":"up","stake_amount":"10.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
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
    let order_id = payload["order"]["id"].as_u64().unwrap();
    assert_eq!(payload["order"]["user_id"], user_id);
    assert_eq!(payload["order"]["product_id"], product_id);
    assert_eq!(payload["order"]["symbol"], symbol);
    assert_eq!(payload["order"]["stake_asset"], quote_asset);
    assert_eq!(payload["order"]["stake_asset_symbol"], quote_symbol);
    assert_eq!(payload["order"]["stake_amount"], "10.000000000000000000");
    assert_eq!(payload["order"]["status"], "opened");

    let event_message = timeout(Duration::from_millis(100), private_events.recv()).await??;
    let event: Value = serde_json::from_str(event_message.payload())?;
    assert_eq!(event["type"], "seconds_contract.order.opened");
    assert_eq!(event["order_id"], order_id);
    assert_eq!(event["product_id"], product_id);
    assert_eq!(event["pair_id"], pair_id);
    assert_eq!(event["symbol"], symbol);
    assert_eq!(event["stake_asset"], quote_asset);
    assert_eq!(event["stake_asset_symbol"], quote_symbol);
    assert_eq!(event["direction"], "up");
    assert_eq!(event["stake_amount"], "10.000000000000000000");
    assert_eq!(event["payout_rate"], "0.80000000");
    assert_eq!(event["status"], "opened");
    assert!(event["expires_at"].as_i64().is_some());

    let (available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(available, decimal("40.000000000000000000"));

    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'seconds_contract_order' AND ref_id = ? AND change_type = 'seconds_contract_open'",
    )
    .bind(order_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 1);

    let commission: (u64, BigDecimal, BigDecimal, BigDecimal, u64, String) = sqlx::query_as(
        r#"SELECT agent_id, source_amount, commission_rate, commission_amount,
                  payout_asset_id, status
           FROM agent_commission_records
           WHERE user_id = ? AND source_type = 'seconds_contract_order' AND source_id = ?"#,
    )
    .bind(user_id)
    .bind(order_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(commission.0, commission_fixture.agent_id);
    assert_eq!(commission.1, decimal("10.000000000000000000"));
    assert_eq!(commission.2, decimal("0.05000000"));
    assert_eq!(commission.3, decimal("0.500000000000000000"));
    assert_eq!(commission.4, quote_asset);
    assert_eq!(commission.5, "pending");
    support::cleanup_direct_agent_commission(&pool, user_id, commission_fixture).await?;

    Ok(())
}

#[tokio::test]
async fn seconds_contract_open_order_uses_requested_product_cycle() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let Some(redis) = redis_manager().await else {
        return Ok(());
    };
    let redis = Some(redis);
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "CB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "CQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_seconds_product(&mut fixture_tx, pair_id, quote_asset).await;
    seed_seconds_product_cycle(
        &mut fixture_tx,
        product_id,
        60,
        "0.70000000",
        "5.000000000000000000",
        Some("50.000000000000000000"),
        0,
    )
    .await;
    seed_seconds_product_cycle(
        &mut fixture_tx,
        product_id,
        120,
        "0.90000000",
        "20.000000000000000000",
        Some("200.000000000000000000"),
        1,
    )
    .await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&mut *fixture_tx)
        .await?;
    fixture_tx.commit().await?;
    seed_ticker(&redis, &symbol, "100.000000000000000000").await;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = user_routes().with_state(state_with_mysql_and_redis(
        settings,
        pool.clone(),
        redis.clone(),
    ));
    let too_small_body = format!(
        r#"{{"product_id":{product_id},"duration_seconds":120,"direction":"up","stake_amount":"10.000000000000000000","idempotency_key":"seconds-cycle-small-{}"}}"#,
        Uuid::now_v7().simple()
    );
    let too_small_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(too_small_body))
                .unwrap(),
        )
        .await?;
    let too_small_payload = body_json(too_small_response).await?;
    assert_eq!(
        too_small_payload["message"],
        "validation error: seconds contract stake is below product minimum"
    );

    let unsupported_body = format!(
        r#"{{"product_id":{product_id},"duration_seconds":180,"direction":"up","stake_amount":"20.000000000000000000","idempotency_key":"seconds-cycle-unsupported-{}"}}"#,
        Uuid::now_v7().simple()
    );
    let unsupported_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(unsupported_body))
                .unwrap(),
        )
        .await?;
    assert_eq!(unsupported_response.status(), StatusCode::NOT_FOUND);

    let idempotency_key = format!("seconds-cycle-open-{}", Uuid::now_v7().simple());
    let request_body = format!(
        r#"{{"product_id":{product_id},"duration_seconds":120,"direction":"down","stake_amount":"25.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let payload = body_json(response).await?;
    assert_eq!(status, StatusCode::OK, "payload: {payload}");
    assert_eq!(payload["order"]["product_id"], product_id);
    assert_eq!(payload["order"]["duration_seconds"], 120);
    assert_eq!(payload["order"]["payout_rate"], "0.90000000");
    assert_eq!(payload["order"]["stake_amount"], "25.000000000000000000");

    Ok(())
}

#[tokio::test]
async fn seconds_contract_settle_win_credits_payout_and_writes_ledger() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let Some(redis) = redis_manager().await else {
        return Ok(());
    };
    let redis = Some(redis);
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let admin_id = create_admin(&pool).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "WB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "WQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_seconds_product(&mut fixture_tx, pair_id, quote_asset).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("50.000000000000000000"))
        .execute(&mut *fixture_tx)
        .await?;
    fixture_tx.commit().await?;
    seed_ticker(&redis, &symbol, "100.000000000000000000").await;

    let user_token =
        issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let hub = EventBroadcastHub::new(16);
    let _keepalive_hub = hub.clone();
    let mut private_events = hub.subscribe(&WebSocketChannel::private_user(user_id));
    let app = user_routes().with_state(state_with_mysql_redis_and_events(
        settings.clone(),
        pool.clone(),
        redis,
        hub.clone(),
    ));
    let request_body = format!(
        r#"{{"product_id":{product_id},"direction":"up","stake_amount":"10.000000000000000000","idempotency_key":"seconds-settle-win-{}"}}"#,
        Uuid::now_v7().simple()
    );
    let open_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/orders")
                .header("authorization", format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await?;
    let open_body = axum::body::to_bytes(open_response.into_body(), 65_536).await?;
    let open_payload: Value = serde_json::from_slice(&open_body)?;
    let order_id = open_payload["order"]["id"].as_u64().unwrap();
    let opened_event_message = timeout(Duration::from_millis(100), private_events.recv()).await??;
    let opened_event: Value = serde_json::from_str(opened_event_message.payload())?;
    assert_eq!(opened_event["type"], "seconds_contract.order.opened");
    assert_eq!(opened_event["order_id"], order_id);

    let settle_app = admin_routes().with_state(
        state_with_mysql_and_redis(settings, pool.clone(), None).with_event_broadcast_hub(hub),
    );
    for attempt in 0..2 {
        let settle_response = settle_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/seconds-contracts/orders/{order_id}/settle"))
                    .header("authorization", format!("Bearer {admin_token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"result":"win","reason":"manual settle win"}"#,
                    ))
                    .unwrap(),
            )
            .await?;
        let settle_status = settle_response.status();
        let settle_body = axum::body::to_bytes(settle_response.into_body(), 65_536).await?;
        assert_eq!(
            settle_status,
            StatusCode::OK,
            "payload: {}",
            String::from_utf8_lossy(&settle_body)
        );
        let settle_payload: Value = serde_json::from_slice(&settle_body)?;
        assert_eq!(settle_payload["order"]["id"], order_id);
        assert_eq!(settle_payload["order"]["status"], "settled");
        assert_eq!(settle_payload["order"]["result"], "win");
        assert_eq!(settle_payload["payout_amount"], "18.000000000000000000");
        if attempt == 0 {
            let settled_event_message =
                timeout(Duration::from_millis(100), private_events.recv()).await??;
            let settled_event: Value = serde_json::from_str(settled_event_message.payload())?;
            assert_eq!(settled_event["type"], "seconds_contract.order.settled");
            assert_eq!(settled_event["order_id"], order_id);
            assert_eq!(settled_event["product_id"], product_id);
            assert_eq!(settled_event["pair_id"], pair_id);
            assert_eq!(settled_event["stake_asset"], quote_asset);
            assert_eq!(settled_event["direction"], "up");
            assert_eq!(settled_event["stake_amount"], "10.000000000000000000");
            assert_eq!(settled_event["payout_amount"], "18.000000000000000000");
            assert_eq!(settled_event["result"], "win");
            assert_eq!(settled_event["status"], "settled");
        } else {
            assert!(
                timeout(Duration::from_millis(25), private_events.recv())
                    .await
                    .is_err(),
                "idempotent seconds contract settlement replay must not publish duplicate private event"
            );
        }
    }

    let (available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(available, decimal("58.000000000000000000"));

    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'seconds_contract_order' AND ref_id = ? AND change_type LIKE 'seconds_contract_settle%'",
    )
    .bind(order_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 1);

    let audit_rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT action, target_type, reason FROM admin_audit_logs WHERE admin_id = ? AND target_id = ? ORDER BY id",
    )
    .bind(admin_id)
    .bind(order_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audit_rows.len(), 1);
    assert_eq!(audit_rows[0].0, "seconds_contract_order.settle");
    assert_eq!(audit_rows[0].1, "seconds_contract_order");
    assert_eq!(audit_rows[0].2, "manual settle win");

    Ok(())
}

#[tokio::test]
async fn seconds_contract_settle_loss_is_idempotent_without_credit() -> Result<(), Box<dyn Error>> {
    assert_seconds_contract_settlement_idempotent_loss().await
}

#[tokio::test]
async fn seconds_contract_settle_rejects_different_result_replay() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let Some(redis) = redis_manager().await else {
        return Ok(());
    };
    let redis = Some(redis);
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let admin_id = create_admin(&pool).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "RB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "RQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_seconds_product(&mut fixture_tx, pair_id, quote_asset).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("50.000000000000000000"))
        .execute(&mut *fixture_tx)
        .await?;
    fixture_tx.commit().await?;
    seed_ticker(&redis, &symbol, "100.000000000000000000").await;

    let user_token =
        issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let open_app = user_routes().with_state(state_with_mysql_and_redis(
        settings.clone(),
        pool.clone(),
        redis,
    ));
    let request_body = format!(
        r#"{{"product_id":{product_id},"direction":"up","stake_amount":"10.000000000000000000","idempotency_key":"seconds-settle-mismatch-{}"}}"#,
        Uuid::now_v7().simple()
    );
    let open_response = open_app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/orders")
                .header("authorization", format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await?;
    let open_body = axum::body::to_bytes(open_response.into_body(), 65_536).await?;
    let open_payload: Value = serde_json::from_slice(&open_body)?;
    let order_id = open_payload["order"]["id"].as_u64().unwrap();
    let settle_app =
        admin_routes().with_state(state_with_mysql_and_redis(settings, pool.clone(), None));

    let first_response = settle_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/seconds-contracts/orders/{order_id}/settle"))
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"result":"win","reason":"manual settle win"}"#,
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(first_response.status(), StatusCode::OK);

    let conflict_response = settle_app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/seconds-contracts/orders/{order_id}/settle"))
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"result":"loss","reason":"manual settle loss"}"#,
                ))
                .unwrap(),
        )
        .await?;
    let conflict_status = conflict_response.status();
    let conflict_body = axum::body::to_bytes(conflict_response.into_body(), 65_536).await?;
    let conflict_payload: Value = serde_json::from_slice(&conflict_body)?;
    assert_eq!(
        conflict_status,
        StatusCode::CONFLICT,
        "payload: {}",
        String::from_utf8_lossy(&conflict_body)
    );
    assert_eq!(conflict_payload["code"], "CONFLICT");

    let (available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(available, decimal("58.000000000000000000"));

    let (settlement_ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'seconds_contract_order' AND ref_id = ? AND change_type LIKE 'seconds_contract_settle%'",
    )
    .bind(order_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(settlement_ledger_count, 1);

    Ok(())
}

#[tokio::test]
async fn seconds_contract_open_order_is_idempotent_for_repeated_key() -> Result<(), Box<dyn Error>>
{
    assert_seconds_contract_idempotent_open(false).await
}

#[tokio::test]
async fn seconds_contract_open_order_concurrent_idempotency_key_debits_once()
-> Result<(), Box<dyn Error>> {
    assert_seconds_contract_idempotent_open(true).await
}

#[tokio::test]
async fn admin_disabling_seconds_contract_product_blocks_concurrent_open_after_commit()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "DB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "DQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_seconds_product(&mut fixture_tx, pair_id, quote_asset).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("50.000000000000000000"))
        .execute(&mut *fixture_tx)
        .await?;
    fixture_tx.commit().await?;

    let mut disable_tx = pool.begin().await?;
    sqlx::query("SELECT id FROM seconds_contract_products WHERE id = ? FOR UPDATE")
        .bind(product_id)
        .execute(&mut *disable_tx)
        .await?;
    sqlx::query("UPDATE seconds_contract_products SET status = 'disabled' WHERE id = ?")
        .bind(product_id)
        .execute(&mut *disable_tx)
        .await?;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = user_routes().with_state(state_with_mysql_and_redis(settings, pool.clone(), None));
    let idempotency_key = format!("seconds-disable-race-{}", Uuid::now_v7().simple());
    let request_body = format!(
        r#"{{"product_id":{product_id},"direction":"up","stake_amount":"10.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );
    let mut open_task = tokio::spawn(async move {
        app.oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
    });

    assert!(
        timeout(Duration::from_millis(200), &mut open_task)
            .await
            .is_err()
    );
    disable_tx.commit().await?;
    let response = open_task.await??;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 65_536).await?;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );

    let (order_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM seconds_contract_orders WHERE user_id = ? AND product_id = ?",
    )
    .bind(user_id)
    .bind(product_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(order_count, 0);

    let (available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(available, decimal("50.000000000000000000"));

    Ok(())
}

#[tokio::test]
async fn seconds_contract_open_replays_existing_key_after_product_disabled()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "ER").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "EQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_seconds_product(&mut fixture_tx, pair_id, quote_asset).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("50.000000000000000000"))
        .execute(&mut *fixture_tx)
        .await?;
    let idempotency_key = format!("seconds-disabled-replay-{}", Uuid::now_v7().simple());
    let order_id = sqlx::query(
        r#"INSERT INTO seconds_contract_orders
           (user_id, product_id, pair_id, stake_asset, direction, stake_amount,
            payout_rate, status, idempotency_key, expires_at)
           VALUES (?, ?, ?, ?, 'up', ?, ?, 'opened', ?, CURRENT_TIMESTAMP(6))"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(quote_asset)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("0.80000000"))
    .bind(&idempotency_key)
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(decimal("40.000000000000000000"))
        .bind(user_id)
        .bind(quote_asset)
        .execute(&mut *fixture_tx)
        .await?;
    sqlx::query("UPDATE seconds_contract_products SET status = 'disabled' WHERE id = ?")
        .bind(product_id)
        .execute(&mut *fixture_tx)
        .await?;
    fixture_tx.commit().await?;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = user_routes().with_state(state_with_mysql_and_redis(settings, pool.clone(), None));
    let request_body = format!(
        r#"{{"product_id":{product_id},"direction":"up","stake_amount":"10.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
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
    assert_eq!(payload["order"]["id"], order_id);

    let conflict_body = format!(
        r#"{{"product_id":{product_id},"direction":"up","stake_amount":"20.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );
    let conflict_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(conflict_body))
                .unwrap(),
        )
        .await?;
    let conflict_status = conflict_response.status();
    let conflict_payload = body_json(conflict_response).await?;
    assert_eq!(conflict_status, StatusCode::CONFLICT);
    assert_eq!(conflict_payload["code"], "CONFLICT");
    assert_eq!(
        conflict_payload["message"],
        "conflict: seconds contract idempotency key belongs to a different request"
    );

    let (available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(available, decimal("40.000000000000000000"));

    Ok(())
}

async fn assert_seconds_contract_settlement_idempotent_loss() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let Some(redis) = redis_manager().await else {
        return Ok(());
    };
    let redis = Some(redis);
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let admin_id = create_admin(&pool).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "LB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "LQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_seconds_product(&mut fixture_tx, pair_id, quote_asset).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("50.000000000000000000"))
        .execute(&mut *fixture_tx)
        .await?;
    fixture_tx.commit().await?;
    seed_ticker(&redis, &symbol, "100.000000000000000000").await;

    let user_token =
        issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = user_routes().with_state(state_with_mysql_and_redis(
        settings.clone(),
        pool.clone(),
        redis,
    ));
    let request_body = format!(
        r#"{{"product_id":{product_id},"direction":"down","stake_amount":"10.000000000000000000","idempotency_key":"seconds-settle-loss-{}"}}"#,
        Uuid::now_v7().simple()
    );
    let open_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/seconds-contracts/orders")
                .header("authorization", format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await?;
    let open_body = axum::body::to_bytes(open_response.into_body(), 65_536).await?;
    let open_payload: Value = serde_json::from_slice(&open_body)?;
    let order_id = open_payload["order"]["id"].as_u64().unwrap();
    let settle_app =
        admin_routes().with_state(state_with_mysql_and_redis(settings, pool.clone(), None));

    for _ in 0..2 {
        let response = settle_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/seconds-contracts/orders/{order_id}/settle"))
                    .header("authorization", format!("Bearer {admin_token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"result":"loss","reason":"manual settle loss"}"#,
                    ))
                    .unwrap(),
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
        assert_eq!(payload["order"]["id"], order_id);
        assert_eq!(payload["order"]["status"], "settled");
        assert_eq!(payload["order"]["result"], "loss");
        assert_eq!(payload["payout_amount"], "0");
    }

    let (available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(available, decimal("40.000000000000000000"));

    let (settlement_ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'seconds_contract_order' AND ref_id = ? AND change_type LIKE 'seconds_contract_settle%'",
    )
    .bind(order_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(settlement_ledger_count, 0);

    Ok(())
}

async fn assert_seconds_contract_idempotent_open(concurrent: bool) -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let Some(redis) = redis_manager().await else {
        return Ok(());
    };
    let redis = Some(redis);
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "IB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "IQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_seconds_product(&mut fixture_tx, pair_id, quote_asset).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("50.000000000000000000"))
        .execute(&mut *fixture_tx)
        .await?;
    fixture_tx.commit().await?;
    seed_ticker(&redis, &symbol, "100.000000000000000000").await;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("seconds-repeat-{}", Uuid::now_v7().simple());
    let app = user_routes().with_state(state_with_mysql_and_redis(settings, pool.clone(), redis));
    let request_body = format!(
        r#"{{"product_id":{product_id},"direction":"down","stake_amount":"10.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );

    let first_request = Request::builder()
        .method("POST")
        .uri("/seconds-contracts/orders")
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(request_body.clone()))
        .unwrap();
    let second_request = Request::builder()
        .method("POST")
        .uri("/seconds-contracts/orders")
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(request_body))
        .unwrap();

    let (first, second) = if concurrent {
        tokio::join!(
            app.clone().oneshot(first_request),
            app.oneshot(second_request)
        )
    } else {
        (
            app.clone().oneshot(first_request).await,
            app.oneshot(second_request).await,
        )
    };
    let first = first?;
    let second = second?;
    assert_eq!(first.status(), StatusCode::OK);
    let first_body = axum::body::to_bytes(first.into_body(), 65_536).await?;
    let first_payload: Value = serde_json::from_slice(&first_body)?;
    let first_order_id = first_payload["order"]["id"].as_u64().unwrap();

    let second_status = second.status();
    let second_body = axum::body::to_bytes(second.into_body(), 65_536).await?;
    let second_payload: Value = serde_json::from_slice(&second_body)?;
    assert_eq!(
        second_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&second_body)
    );
    assert_eq!(second_payload["order"]["id"], first_order_id);

    let (available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(available, decimal("40.000000000000000000"));

    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'seconds_contract_order' AND ref_id = ?",
    )
    .bind(first_order_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 1);

    Ok(())
}
