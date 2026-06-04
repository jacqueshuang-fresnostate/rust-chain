use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use bigdecimal::BigDecimal;
use chrono::Utc;
use exchange_api::{
    config::Settings,
    modules::{
        auth::{TokenScope, issue_token},
        events::{EventBroadcastHub, WebSocketChannel},
        margin::routes::{admin_routes, user_routes},
        market::market_ticker_redis_key,
    },
    state::AppState,
};
use redis::AsyncCommands;
use secrecy::SecretString;
use serde_json::Value;
use sqlx::{MySql, MySqlPool, Transaction, mysql::MySqlPoolOptions, types::Json as SqlxJson};
use std::{error::Error, str::FromStr, time::Duration};
use tokio::time::timeout;
use tower::ServiceExt;
use uuid::Uuid;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

async fn body_json(response: axum::response::Response) -> Result<Value, Box<dyn Error>> {
    let body = axum::body::to_bytes(response.into_body(), 65_536).await?;
    Ok(serde_json::from_slice(&body)?)
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
            eprintln!("skipping MySQL margin route test because DATABASE_URL is not set");
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
        _ => {
            eprintln!("skipping Redis-backed margin route test because REDIS_URL is not set");
            return None;
        }
    };
    let client = redis::Client::open(redis_url).unwrap();
    Some(redis::aio::ConnectionManager::new(client).await.unwrap())
}

async fn cache_margin_ticker(
    redis: &redis::aio::ConnectionManager,
    symbol: &str,
    price: &str,
) -> Result<(), Box<dyn Error>> {
    cache_margin_ticker_at(redis, symbol, price, Utc::now().timestamp_millis()).await
}

async fn cache_margin_ticker_at(
    redis: &redis::aio::ConnectionManager,
    symbol: &str,
    price: &str,
    observed_at: i64,
) -> Result<(), Box<dyn Error>> {
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
        .await?;
    Ok(())
}

async fn create_user(tx: &mut Transaction<'_, MySql>) -> u64 {
    let email = format!("margin-route-{}@example.test", Uuid::now_v7().simple());
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
    let role_id = sqlx::query(
        "INSERT INTO admin_roles (name, permissions) VALUES (?, JSON_ARRAY('margin:read'))",
    )
    .bind(format!("margin-role-{}", &suffix[16..32]))
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id();
    sqlx::query("INSERT INTO admin_users (username, password_hash, role_id) VALUES (?, ?, ?)")
        .bind(format!("margin-admin-{}", &suffix[16..32]))
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

async fn seed_margin_product(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
    margin_asset: u64,
) -> u64 {
    seed_margin_product_with_mode(
        tx,
        pair_id,
        margin_asset,
        "isolated",
        vec!["2", "3", "4", "5"],
    )
    .await
}

async fn seed_margin_product_with_mode(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
    margin_asset: u64,
    margin_mode: &str,
    leverage_levels: Vec<&str>,
) -> u64 {
    let max_leverage = leverage_levels
        .last()
        .map(|level| decimal(level))
        .unwrap_or_else(|| decimal("5.00000000"));
    let leverage_levels: Vec<String> = leverage_levels
        .into_iter()
        .map(std::string::ToString::to_string)
        .collect();

    sqlx::query(
        r#"INSERT INTO margin_products
           (pair_id, margin_asset, margin_mode, leverage_levels, max_leverage, min_margin, max_margin, maintenance_margin_rate, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'active')"#,
    )
    .bind(pair_id)
    .bind(margin_asset)
    .bind(margin_mode)
    .bind(SqlxJson(leverage_levels))
    .bind(max_leverage)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("1000.000000000000000000"))
    .bind(decimal("0.05000000"))
    .execute(&mut **tx)
    .await
    .unwrap()
    .last_insert_id()
}

#[tokio::test]
async fn margin_routes_require_expected_scope() {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:42", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:7", TokenScope::Admin, 900).unwrap();

    let unauthenticated_open = user_routes()
        .with_state(AppState::new(settings.clone()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/margin/positions")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"product_id":1,"direction":"long","margin_amount":"20.000000000000000000","leverage":"3.00000000","idempotency_key":"scope-test"}"#,
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
                .uri("/margin/products")
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(user_on_admin.status(), StatusCode::FORBIDDEN);

    let admin_on_user_open = user_routes()
        .with_state(AppState::new(settings))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/margin/positions")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"product_id":1,"direction":"long","margin_amount":"20.000000000000000000","leverage":"3.00000000","idempotency_key":"scope-test"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(admin_on_user_open.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn margin_routes_return_clear_error_without_mysql() {
    let settings = test_settings();
    let token = issue_token(&settings, "user:42", TokenScope::User, 900).unwrap();
    let response = user_routes()
        .with_state(AppState::new(settings))
        .oneshot(
            Request::builder()
                .uri("/margin/products")
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
        "internal error: mysql pool is not configured for margin routes"
    );
}

#[tokio::test]
async fn margin_lists_active_products_for_user_and_all_products_for_admin()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let admin_id = create_admin(&pool).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "MB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "MQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let active_product_id = seed_margin_product(&mut fixture_tx, pair_id, quote_asset).await;
    let disabled_product_id = seed_margin_product(&mut fixture_tx, pair_id, quote_asset).await;
    sqlx::query("UPDATE margin_products SET status = 'disabled' WHERE id = ?")
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
    let state = AppState::new(settings).with_mysql(pool.clone());

    let user_response = user_routes()
        .with_state(state.clone())
        .oneshot(
            Request::builder()
                .uri("/margin/products")
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
                .uri("/margin/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let admin_status = admin_response.status();
    let admin_body = axum::body::to_bytes(admin_response.into_body(), 65_536).await?;
    assert_eq!(
        admin_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&admin_body)
    );
    let admin_payload: Value = serde_json::from_slice(&admin_body)?;
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
async fn admin_margin_product_routes_require_admin_scope_mysql_and_validation()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:42", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = admin_routes().with_state(AppState::new(settings));

    let unauthenticated_detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/margin/products/1")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(unauthenticated_detail.status(), StatusCode::UNAUTHORIZED);

    let user_detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/margin/products/1")
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
                .uri("/margin/products/1")
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
        "internal error: mysql pool is not configured for margin routes"
    );

    let blank_create_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/margin/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"pair_id":1,"margin_asset":1,"max_leverage":"5.00000000","min_margin":"10.000000000000000000","maintenance_margin_rate":"0.05000000","reason":"   "}"#,
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
        "validation error: margin product reason is required"
    );

    let no_mysql = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/margin/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"pair_id":1,"margin_asset":1,"max_leverage":"5.00000000","min_margin":"10.000000000000000000","maintenance_margin_rate":"0.05000000","reason":"create test product"}"#,
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
        "internal error: mysql pool is not configured for margin routes"
    );

    let blank_status_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/margin/products/1/status")
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
        "validation error: margin product reason is required"
    );

    let invalid_status = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/margin/products/1/status")
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
async fn admin_margin_product_rejects_unsafe_fields_before_mysql() -> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = admin_routes().with_state(AppState::new(settings));

    let unsafe_leverage = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/margin/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"pair_id":1,"margin_asset":1,"max_leverage":"1.00000000","min_margin":"10.000000000000000000","maintenance_margin_rate":"0.05000000"}"#,
                ))
                .unwrap(),
        )
        .await?;
    let unsafe_leverage_status = unsafe_leverage.status();
    let unsafe_leverage_payload = body_json(unsafe_leverage).await?;
    assert_eq!(unsafe_leverage_status, StatusCode::BAD_REQUEST);
    assert_eq!(unsafe_leverage_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        unsafe_leverage_payload["message"],
        "validation error: margin product max_leverage must be greater than 1"
    );

    let overflow_leverage = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/margin/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"pair_id":1,"margin_asset":1,"max_leverage":"10000000000.00000000","min_margin":"10.000000000000000000","maintenance_margin_rate":"0.05000000"}"#,
                ))
                .unwrap(),
        )
        .await?;
    let overflow_leverage_status = overflow_leverage.status();
    let overflow_leverage_payload = body_json(overflow_leverage).await?;
    assert_eq!(overflow_leverage_status, StatusCode::BAD_REQUEST);
    assert_eq!(overflow_leverage_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        overflow_leverage_payload["message"],
        "validation error: margin product max_leverage exceeds decimal storage precision"
    );

    let scale_rate = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/margin/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"pair_id":1,"margin_asset":1,"max_leverage":"5.000000001","min_margin":"10.000000000000000000","maintenance_margin_rate":"0.05000000"}"#,
                ))
                .unwrap(),
        )
        .await?;
    let scale_rate_status = scale_rate.status();
    let scale_rate_payload = body_json(scale_rate).await?;
    assert_eq!(scale_rate_status, StatusCode::BAD_REQUEST);
    assert_eq!(scale_rate_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        scale_rate_payload["message"],
        "validation error: margin product max_leverage supports at most 8 decimal places"
    );

    let long_reason = "R".repeat(513);
    let long_reason_body = format!(
        r#"{{"pair_id":1,"margin_asset":1,"max_leverage":"5.00000000","min_margin":"10.000000000000000000","maintenance_margin_rate":"0.05000000","reason":"{long_reason}"}}"#
    );
    let long_reason_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/margin/products")
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
        "validation error: margin product reason is too long"
    );

    Ok(())
}

#[tokio::test]
async fn admin_margin_product_create_update_status_and_audit() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let admin_id = create_admin(&pool).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "AM").await;
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
    let app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let missing_pair_body = format!(
        r#"{{"pair_id":999999999999,"margin_asset":{quote_asset},"max_leverage":"5.00000000","min_margin":"10.000000000000000000","maintenance_margin_rate":"0.05000000","reason":"missing pair test"}}"#
    );
    let missing_pair_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/margin/products")
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
        r#"{{"pair_id":{pair_id},"margin_asset":{quote_asset},"max_leverage":"8.00000000","min_margin":"15.000000000000000000","max_margin":"150.000000000000000000","maintenance_margin_rate":"0.04000000","status":"active","reason":"launch margin product"}}"#
    );
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/margin/products")
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
    assert_eq!(create_payload["margin_asset"], quote_asset);
    assert_eq!(create_payload["margin_asset_symbol"], quote_symbol);
    assert_eq!(create_payload["max_leverage"], "8.00000000");
    assert_eq!(create_payload["maintenance_margin_rate"], "0.04000000");
    assert_eq!(create_payload["status"], "active");

    let detail_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/margin/products/{product_id}"))
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
    assert_eq!(detail_payload["margin_asset"], quote_asset);
    assert_eq!(detail_payload["margin_asset_symbol"], quote_symbol);
    assert_eq!(detail_payload["status"], "active");

    let unknown_detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/margin/products/999999999999")
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(unknown_detail.status(), StatusCode::NOT_FOUND);

    let long_update_reason = "R".repeat(513);
    let long_update_reason_body =
        format!(r#"{{"status":"disabled","reason":"{long_update_reason}"}}"#);
    let long_update_reason_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/margin/products/{product_id}/status"))
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
        "validation error: margin product reason is too long"
    );

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/margin/products/{product_id}/status"))
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"status":"disabled","reason":"pause margin product"}"#,
                ))
                .unwrap(),
        )
        .await?;
    let update_status = update_response.status();
    let update_payload = body_json(update_response).await?;
    assert_eq!(update_status, StatusCode::OK, "payload: {update_payload}");
    assert_eq!(update_payload["id"], product_id);
    assert_eq!(update_payload["status"], "disabled");

    let audit_rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT action, target_type, reason FROM admin_audit_logs WHERE admin_id = ? AND target_id = ? ORDER BY id",
    )
    .bind(admin_id)
    .bind(product_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audit_rows.len(), 2);
    assert_eq!(audit_rows[0].0, "margin_product.create");
    assert_eq!(audit_rows[0].1, "margin_product");
    assert_eq!(audit_rows[0].2, "launch margin product");
    assert_eq!(audit_rows[1].0, "margin_product.update_status");
    assert_eq!(audit_rows[1].1, "margin_product");
    assert_eq!(audit_rows[1].2, "pause margin product");

    Ok(())
}

#[tokio::test]
async fn admin_margin_product_create_rolls_back_when_audit_fails() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "XM").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "XQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    fixture_tx.commit().await?;

    let admin_token = issue_token(&settings, "admin:999999999", TokenScope::Admin, 900).unwrap();
    let body = format!(
        r#"{{"pair_id":{pair_id},"margin_asset":{quote_asset},"max_leverage":"5.00000000","min_margin":"10.000000000000000000","maintenance_margin_rate":"0.05000000","reason":"audit should fail"}}"#
    );
    let response = admin_routes()
        .with_state(AppState::new(settings).with_mysql(pool.clone()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/margin/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await?;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let (product_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM margin_products WHERE pair_id = ? AND margin_asset = ?",
    )
    .bind(pair_id)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(product_count, 0);

    Ok(())
}

#[tokio::test]
async fn admin_margin_product_persists_mode_and_leverage_levels() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let admin_id = create_admin(&pool).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "LM").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "LQ").await;
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
    let app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let create_body = format!(
        r#"{{"pair_id":{pair_id},"margin_asset":{quote_asset},"margin_mode":"isolated","leverage_levels":["2","5","10"],"max_leverage":"10.00000000","min_margin":"15.000000000000000000","max_margin":"150.000000000000000000","maintenance_margin_rate":"0.04000000","status":"active","reason":"launch leverage levels"}}"#
    );

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/margin/products")
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
    assert_eq!(create_payload["margin_mode"], "isolated");
    assert_eq!(
        create_payload["leverage_levels"],
        serde_json::json!(["2", "5", "10"])
    );
    assert_eq!(create_payload["max_leverage"], "10.00000000");

    let (stored_mode, stored_levels): (String, SqlxJson<Vec<String>>) =
        sqlx::query_as("SELECT margin_mode, leverage_levels FROM margin_products WHERE id = ?")
            .bind(product_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(stored_mode, "isolated");
    assert_eq!(stored_levels.0, vec!["2", "5", "10"]);

    let detail_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/margin/products/{product_id}"))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let detail_status = detail_response.status();
    let detail_payload = body_json(detail_response).await?;
    assert_eq!(detail_status, StatusCode::OK, "payload: {detail_payload}");
    assert_eq!(detail_payload["margin_mode"], "isolated");
    assert_eq!(
        detail_payload["leverage_levels"],
        serde_json::json!(["2", "5", "10"])
    );

    let list_response = app
        .oneshot(
            Request::builder()
                .uri("/margin/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let list_status = list_response.status();
    let list_payload = body_json(list_response).await?;
    assert_eq!(list_status, StatusCode::OK, "payload: {list_payload}");
    assert!(
        list_payload["products"]
            .as_array()
            .unwrap()
            .iter()
            .any(|product| {
                product["id"] == product_id
                    && product["margin_mode"] == "isolated"
                    && product["leverage_levels"] == serde_json::json!(["2", "5", "10"])
            })
    );

    Ok(())
}

#[tokio::test]
async fn admin_margin_product_rejects_invalid_mode_and_leverage_levels_before_mysql()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = admin_routes().with_state(AppState::new(settings));

    for body in [
        r#"{"pair_id":1,"margin_asset":1,"margin_mode":"portfolio","leverage_levels":["2","5"],"max_leverage":"5.00000000","min_margin":"10.000000000000000000","maintenance_margin_rate":"0.05000000","reason":"invalid mode"}"#,
        r#"{"pair_id":1,"margin_asset":1,"margin_mode":"isolated","leverage_levels":[],"max_leverage":"5.00000000","min_margin":"10.000000000000000000","maintenance_margin_rate":"0.05000000","reason":"empty levels"}"#,
        r#"{"pair_id":1,"margin_asset":1,"margin_mode":"isolated","leverage_levels":["1","5"],"max_leverage":"5.00000000","min_margin":"10.000000000000000000","maintenance_margin_rate":"0.05000000","reason":"low level"}"#,
        r#"{"pair_id":1,"margin_asset":1,"margin_mode":"isolated","leverage_levels":["2","2","5"],"max_leverage":"5.00000000","min_margin":"10.000000000000000000","maintenance_margin_rate":"0.05000000","reason":"duplicate level"}"#,
        r#"{"pair_id":1,"margin_asset":1,"margin_mode":"isolated","leverage_levels":["2","5","10"],"max_leverage":"5.00000000","min_margin":"10.000000000000000000","maintenance_margin_rate":"0.05000000","reason":"mismatch max"}"#,
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/margin/products")
                    .header("authorization", format!("Bearer {admin_token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await?;
        let status = response.status();
        let payload = body_json(response).await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "payload: {payload}");
        assert_eq!(payload["code"], "VALIDATION_ERROR");
    }

    Ok(())
}

#[tokio::test]
async fn margin_open_position_requires_configured_leverage_level_and_persists_mode()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "LB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "LQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_margin_product_with_mode(
        &mut fixture_tx,
        pair_id,
        quote_asset,
        "isolated",
        vec!["2", "5", "10"],
    )
    .await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&mut *fixture_tx)
        .await?;
    fixture_tx.commit().await?;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = user_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let rejected_body = format!(
        r#"{{"product_id":{product_id},"direction":"long","margin_amount":"20.000000000000000000","leverage":"7.00000000","idempotency_key":"margin-level-reject-{}"}}"#,
        Uuid::now_v7().simple()
    );
    let rejected_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/margin/positions")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(rejected_body))
                .unwrap(),
        )
        .await?;
    let rejected_status = rejected_response.status();
    let rejected_payload = body_json(rejected_response).await?;
    assert_eq!(
        rejected_status,
        StatusCode::BAD_REQUEST,
        "payload: {rejected_payload}"
    );
    assert_eq!(rejected_payload["code"], "VALIDATION_ERROR");

    let accepted_body = format!(
        r#"{{"product_id":{product_id},"direction":"long","margin_amount":"20.000000000000000000","leverage":"5.00000000","idempotency_key":"margin-level-open-{}"}}"#,
        Uuid::now_v7().simple()
    );
    let accepted_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/margin/positions")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(accepted_body))
                .unwrap(),
        )
        .await?;
    let accepted_status = accepted_response.status();
    let accepted_payload = body_json(accepted_response).await?;
    assert_eq!(
        accepted_status,
        StatusCode::OK,
        "payload: {accepted_payload}"
    );
    let position_id = accepted_payload["position"]["id"].as_u64().unwrap();
    assert_eq!(accepted_payload["position"]["margin_mode"], "isolated");

    let (stored_mode,): (String,) =
        sqlx::query_as("SELECT margin_mode FROM margin_positions WHERE id = ?")
            .bind(position_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(stored_mode, "isolated");

    Ok(())
}

#[tokio::test]
async fn margin_open_position_debits_wallet_and_writes_ledger() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "PB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "PQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_margin_product(&mut fixture_tx, pair_id, quote_asset).await;
    sqlx::query("UPDATE margin_products SET hourly_interest_rate = ? WHERE id = ?")
        .bind(decimal("0.00100000"))
        .bind(product_id)
        .execute(&mut *fixture_tx)
        .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&mut *fixture_tx)
        .await?;
    fixture_tx.commit().await?;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("margin-open-{}", Uuid::now_v7().simple());
    let hub = EventBroadcastHub::new(16);
    let _keepalive_hub = hub.clone();
    let mut private_events = hub.subscribe(&WebSocketChannel::private_user(user_id));
    let app = user_routes().with_state(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_event_broadcast_hub(hub),
    );
    let request_body = format!(
        r#"{{"product_id":{product_id},"direction":"long","margin_amount":"20.000000000000000000","leverage":"3.00000000","idempotency_key":"{idempotency_key}"}}"#
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/margin/positions")
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
    let position_id = payload["position"]["id"].as_u64().unwrap();
    assert_eq!(payload["position"]["user_id"], user_id);
    assert_eq!(payload["position"]["product_id"], product_id);
    assert_eq!(payload["position"]["margin_asset"], quote_asset);
    assert_eq!(
        payload["position"]["margin_amount"],
        "20.000000000000000000"
    );
    assert_eq!(
        payload["position"]["notional_amount"],
        "60.000000000000000000"
    );
    assert_eq!(payload["position"]["status"], "opened");
    assert_eq!(
        payload["position"]["borrowed_amount"],
        "40.000000000000000000"
    );
    assert_eq!(
        payload["position"]["interest_amount"],
        "0.000000000000000000"
    );

    let event_message = timeout(Duration::from_millis(100), private_events.recv()).await??;
    let event: Value = serde_json::from_str(event_message.payload())?;
    assert_eq!(event["type"], "margin.position.opened");
    assert_eq!(event["position_id"], position_id);
    assert_eq!(event["product_id"], product_id);
    assert_eq!(event["pair_id"], pair_id);
    assert_eq!(event["margin_asset"], quote_asset);
    assert_eq!(event["direction"], "long");
    assert_eq!(event["margin_amount"], "20.000000000000000000");
    assert_eq!(event["leverage"], "3.00000000");
    assert_eq!(event["notional_amount"], "60.000000000000000000");
    assert_eq!(event["borrowed_amount"], "40.000000000000000000");
    assert_eq!(event["interest_amount"], "0.000000000000000000");
    assert_eq!(event["status"], "opened");

    let (available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(available, decimal("80.000000000000000000"));

    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'margin_position' AND ref_id = ? AND change_type = 'margin_position_open'",
    )
    .bind(position_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 1);

    Ok(())
}

#[tokio::test]
async fn margin_position_queries_return_only_authenticated_user_positions()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let other_user_id = create_user(&mut fixture_tx).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "QB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "QQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_margin_product(&mut fixture_tx, pair_id, quote_asset).await;
    let opened_id = sqlx::query(
        r#"INSERT INTO margin_positions
           (user_id, product_id, pair_id, margin_asset, direction, margin_amount,
            leverage, notional_amount, entry_price, status, idempotency_key)
           VALUES (?, ?, ?, ?, 'long', ?, ?, ?, ?, 'opened', ?)"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(quote_asset)
    .bind(decimal("20.000000000000000000"))
    .bind(decimal("5.00000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(format!("margin-query-open-{}", Uuid::now_v7().simple()))
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    let closed_id = sqlx::query(
        r#"INSERT INTO margin_positions
           (user_id, product_id, pair_id, margin_asset, direction, margin_amount,
            leverage, notional_amount, entry_price, status, idempotency_key, closed_at,
            exit_price, realized_pnl)
           VALUES (?, ?, ?, ?, 'short', ?, ?, ?, ?, 'closed', ?, CURRENT_TIMESTAMP(6), ?, ?)"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(quote_asset)
    .bind(decimal("30.000000000000000000"))
    .bind(decimal("4.00000000"))
    .bind(decimal("120.000000000000000000"))
    .bind(decimal("200.000000000000000000"))
    .bind(format!("margin-query-closed-{}", Uuid::now_v7().simple()))
    .bind(decimal("180.000000000000000000"))
    .bind(decimal("12.000000000000000000"))
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    let other_position_id = sqlx::query(
        r#"INSERT INTO margin_positions
           (user_id, product_id, pair_id, margin_asset, direction, margin_amount,
            leverage, notional_amount, entry_price, status, idempotency_key)
           VALUES (?, ?, ?, ?, 'long', ?, ?, ?, ?, 'opened', ?)"#,
    )
    .bind(other_user_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(quote_asset)
    .bind(decimal("40.000000000000000000"))
    .bind(decimal("2.00000000"))
    .bind(decimal("80.000000000000000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(format!("margin-query-other-{}", Uuid::now_v7().simple()))
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    fixture_tx.commit().await?;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = user_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/margin/positions?status=opened&limit=10")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let list_status = list_response.status();
    let list_payload = body_json(list_response).await?;
    assert_eq!(list_status, StatusCode::OK, "payload: {list_payload}");
    let positions = list_payload["positions"].as_array().unwrap();
    assert!(positions.iter().any(|position| position["id"] == opened_id));
    assert!(!positions.iter().any(|position| position["id"] == closed_id));
    assert!(
        !positions
            .iter()
            .any(|position| position["id"] == other_position_id)
    );
    assert_eq!(positions[0]["entry_price"], "100.000000000000000000");

    let detail_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/margin/positions/{closed_id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let detail_status = detail_response.status();
    let detail_payload = body_json(detail_response).await?;
    assert_eq!(detail_status, StatusCode::OK, "payload: {detail_payload}");
    assert_eq!(detail_payload["position"]["id"], closed_id);
    assert_eq!(detail_payload["position"]["status"], "closed");
    assert_eq!(
        detail_payload["position"]["exit_price"],
        "180.000000000000000000"
    );
    assert_eq!(
        detail_payload["position"]["realized_pnl"],
        "12.000000000000000000"
    );
    assert!(detail_payload["position"]["closed_at"].as_i64().is_some());

    let other_detail = app
        .oneshot(
            Request::builder()
                .uri(format!("/margin/positions/{other_position_id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(other_detail.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn admin_margin_positions_require_admin_scope_and_mysql() -> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:42", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:7", TokenScope::Admin, 900).unwrap();
    let app = admin_routes().with_state(AppState::new(settings));

    let user_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/margin/positions")
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(user_response.status(), StatusCode::FORBIDDEN);

    let admin_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/margin/positions")
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let admin_status = admin_response.status();
    let admin_payload = body_json(admin_response).await?;
    assert_eq!(admin_status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(admin_payload["code"], "INTERNAL_ERROR");
    assert_eq!(
        admin_payload["message"],
        "internal error: mysql pool is not configured for margin routes"
    );

    let unauthenticated_detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/margin/positions/1")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(unauthenticated_detail.status(), StatusCode::UNAUTHORIZED);

    let user_detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/margin/positions/1")
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(user_detail.status(), StatusCode::FORBIDDEN);

    let admin_detail = app
        .oneshot(
            Request::builder()
                .uri("/margin/positions/1")
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
        "internal error: mysql pool is not configured for margin routes"
    );

    Ok(())
}

#[tokio::test]
async fn admin_margin_positions_filter_history_and_return_interest_fields()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let admin_id = create_admin(&pool).await;
    let user_email = format!(
        "margin-admin-filter-{}@example.test",
        Uuid::now_v7().simple()
    );
    let user_id = create_user_with_email(&mut fixture_tx, user_email.clone()).await;
    let other_user_id = create_user(&mut fixture_tx).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "HB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "HQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let other_pair_id = create_pair(
        &mut fixture_tx,
        base_asset,
        quote_asset,
        &format!("{base_symbol}{quote_symbol}-ALT"),
    )
    .await;
    let product_id = seed_margin_product(&mut fixture_tx, pair_id, quote_asset).await;
    let other_product_id = seed_margin_product(&mut fixture_tx, other_pair_id, quote_asset).await;
    let liquidated_id = sqlx::query(
        r#"INSERT INTO margin_positions
           (user_id, product_id, pair_id, margin_asset, direction, margin_amount,
            leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
            status, idempotency_key, liquidated_at, liquidation_reason)
           VALUES (?, ?, ?, ?, 'long', ?, ?, ?, ?, ?, ?, 'liquidated', ?, CURRENT_TIMESTAMP(6), 'maintenance_margin')"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(quote_asset)
    .bind(decimal("25.000000000000000000"))
    .bind(decimal("4.00000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(decimal("75.000000000000000000"))
    .bind(decimal("2.500000000000000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(format!("admin-margin-history-{}", Uuid::now_v7().simple()))
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    let closed_id = sqlx::query(
        r#"INSERT INTO margin_positions
           (user_id, product_id, pair_id, margin_asset, direction, margin_amount,
            leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
            exit_price, realized_pnl, status, idempotency_key, closed_at)
           VALUES (?, ?, ?, ?, 'short', ?, ?, ?, ?, ?, ?, ?, ?, 'closed', ?, CURRENT_TIMESTAMP(6))"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(quote_asset)
    .bind(decimal("30.000000000000000000"))
    .bind(decimal("3.00000000"))
    .bind(decimal("90.000000000000000000"))
    .bind(decimal("60.000000000000000000"))
    .bind(decimal("1.250000000000000000"))
    .bind(decimal("120.000000000000000000"))
    .bind(decimal("110.000000000000000000"))
    .bind(decimal("7.500000000000000000"))
    .bind(format!("admin-margin-closed-{}", Uuid::now_v7().simple()))
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    let other_user_position_id = sqlx::query(
        r#"INSERT INTO margin_positions
           (user_id, product_id, pair_id, margin_asset, direction, margin_amount,
            leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
            status, idempotency_key, liquidated_at, liquidation_reason)
           VALUES (?, ?, ?, ?, 'long', ?, ?, ?, ?, ?, ?, 'liquidated', ?, CURRENT_TIMESTAMP(6), 'maintenance_margin')"#,
    )
    .bind(other_user_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(quote_asset)
    .bind(decimal("25.000000000000000000"))
    .bind(decimal("4.00000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(decimal("75.000000000000000000"))
    .bind(decimal("2.500000000000000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(format!("admin-margin-other-user-{}", Uuid::now_v7().simple()))
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    let other_pair_position_id = sqlx::query(
        r#"INSERT INTO margin_positions
           (user_id, product_id, pair_id, margin_asset, direction, margin_amount,
            leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
            status, idempotency_key, liquidated_at, liquidation_reason)
           VALUES (?, ?, ?, ?, 'long', ?, ?, ?, ?, ?, ?, 'liquidated', ?, CURRENT_TIMESTAMP(6), 'maintenance_margin')"#,
    )
    .bind(user_id)
    .bind(other_product_id)
    .bind(other_pair_id)
    .bind(quote_asset)
    .bind(decimal("25.000000000000000000"))
    .bind(decimal("4.00000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(decimal("75.000000000000000000"))
    .bind(decimal("2.500000000000000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(format!("admin-margin-other-pair-{}", Uuid::now_v7().simple()))
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
    let app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/margin/positions?email={user_email}&pair_id={pair_id}&status=liquidated&limit=10"
                ))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let payload = body_json(response).await?;
    assert_eq!(status, StatusCode::OK, "payload: {payload}");
    let positions = payload["positions"].as_array().unwrap();
    assert_eq!(positions.len(), 1);
    let position = &positions[0];
    assert_eq!(position["id"], liquidated_id);
    assert_eq!(position["user_id"], user_id);
    assert_eq!(position["product_id"], product_id);
    assert_eq!(position["pair_id"], pair_id);
    assert_eq!(position["margin_asset"], quote_asset);
    assert_eq!(position["status"], "liquidated");
    assert_eq!(position["borrowed_amount"], "75.000000000000000000");
    assert_eq!(position["interest_amount"], "2.500000000000000000");
    assert_eq!(position["closed_at"], Value::Null);
    assert!(position["liquidated_at"].as_i64().is_some());
    assert_eq!(position["liquidation_reason"], "maintenance_margin");
    assert!(!positions.iter().any(|position| position["id"] == closed_id));
    assert!(
        !positions
            .iter()
            .any(|position| position["id"] == other_user_position_id)
    );
    assert!(
        !positions
            .iter()
            .any(|position| position["id"] == other_pair_position_id)
    );

    let closed_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/margin/positions?email={user_email}&status=closed&limit=10"
                ))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let closed_status = closed_response.status();
    let closed_payload = body_json(closed_response).await?;
    assert_eq!(closed_status, StatusCode::OK, "payload: {closed_payload}");
    let closed_positions = closed_payload["positions"].as_array().unwrap();
    assert!(closed_positions.iter().any(|position| {
        position["id"] == closed_id && position["closed_at"].as_i64().is_some()
    }));

    let detail_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/margin/positions/{liquidated_id}"))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let detail_status = detail_response.status();
    let detail_payload = body_json(detail_response).await?;
    assert_eq!(detail_status, StatusCode::OK, "payload: {detail_payload}");
    assert_eq!(detail_payload["id"], liquidated_id);
    assert_eq!(detail_payload["user_id"], user_id);
    assert_eq!(detail_payload["product_id"], product_id);
    assert_eq!(detail_payload["pair_id"], pair_id);
    assert_eq!(detail_payload["margin_asset"], quote_asset);
    assert_eq!(detail_payload["status"], "liquidated");
    assert_eq!(detail_payload["borrowed_amount"], "75.000000000000000000");
    assert_eq!(detail_payload["interest_amount"], "2.500000000000000000");
    assert!(detail_payload["liquidated_at"].as_i64().is_some());
    assert_eq!(detail_payload["liquidation_reason"], "maintenance_margin");

    let unknown_detail = app
        .oneshot(
            Request::builder()
                .uri("/margin/positions/999999999999")
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(unknown_detail.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn admin_margin_interest_summary_requires_admin_scope_and_mysql() -> Result<(), Box<dyn Error>>
{
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:42", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:7", TokenScope::Admin, 900).unwrap();
    let app = admin_routes().with_state(AppState::new(settings));

    let user_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/margin/interest/summary")
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(user_response.status(), StatusCode::FORBIDDEN);

    let admin_response = app
        .oneshot(
            Request::builder()
                .uri("/margin/interest/summary")
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let admin_status = admin_response.status();
    let admin_payload = body_json(admin_response).await?;
    assert_eq!(admin_status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(admin_payload["code"], "INTERNAL_ERROR");
    assert_eq!(
        admin_payload["message"],
        "internal error: mysql pool is not configured for margin routes"
    );

    Ok(())
}

#[tokio::test]
async fn admin_margin_interest_summary_groups_by_status_and_filters() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let admin_id = create_admin(&pool).await;
    let user_email = format!(
        "margin-interest-filter-{}@example.test",
        Uuid::now_v7().simple()
    );
    let user_id = create_user_with_email(&mut fixture_tx, user_email.clone()).await;
    let other_user_id = create_user(&mut fixture_tx).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "IB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "IQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let other_pair_id = create_pair(
        &mut fixture_tx,
        base_asset,
        quote_asset,
        &format!("{base_symbol}{quote_symbol}-SUM"),
    )
    .await;
    let product_id = seed_margin_product(&mut fixture_tx, pair_id, quote_asset).await;
    let other_product_id = seed_margin_product(&mut fixture_tx, other_pair_id, quote_asset).await;

    for (status, borrowed_amount, interest_amount) in [
        ("opened", "40.000000000000000000", "1.000000000000000000"),
        ("closed", "60.000000000000000000", "2.000000000000000000"),
        (
            "liquidated",
            "75.000000000000000000",
            "2.500000000000000000",
        ),
    ] {
        sqlx::query(
            r#"INSERT INTO margin_positions
               (user_id, product_id, pair_id, margin_asset, direction, margin_amount,
                leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
                status, idempotency_key)
               VALUES (?, ?, ?, ?, 'long', ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(user_id)
        .bind(product_id)
        .bind(pair_id)
        .bind(quote_asset)
        .bind(decimal("20.000000000000000000"))
        .bind(decimal("3.00000000"))
        .bind(decimal("60.000000000000000000"))
        .bind(decimal(borrowed_amount))
        .bind(decimal(interest_amount))
        .bind(decimal("100.000000000000000000"))
        .bind(status)
        .bind(format!(
            "admin-margin-interest-{status}-{}",
            Uuid::now_v7().simple()
        ))
        .execute(&mut *fixture_tx)
        .await?;
    }
    sqlx::query(
        r#"INSERT INTO margin_positions
           (user_id, product_id, pair_id, margin_asset, direction, margin_amount,
            leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
            status, idempotency_key)
           VALUES (?, ?, ?, ?, 'long', ?, ?, ?, ?, ?, ?, 'opened', ?)"#,
    )
    .bind(other_user_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(quote_asset)
    .bind(decimal("20.000000000000000000"))
    .bind(decimal("3.00000000"))
    .bind(decimal("60.000000000000000000"))
    .bind(decimal("99.000000000000000000"))
    .bind(decimal("9.000000000000000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(format!(
        "admin-margin-interest-other-user-{}",
        Uuid::now_v7().simple()
    ))
    .execute(&mut *fixture_tx)
    .await?;
    sqlx::query(
        r#"INSERT INTO margin_positions
           (user_id, product_id, pair_id, margin_asset, direction, margin_amount,
            leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
            status, idempotency_key)
           VALUES (?, ?, ?, ?, 'long', ?, ?, ?, ?, ?, ?, 'opened', ?)"#,
    )
    .bind(user_id)
    .bind(other_product_id)
    .bind(other_pair_id)
    .bind(quote_asset)
    .bind(decimal("20.000000000000000000"))
    .bind(decimal("3.00000000"))
    .bind(decimal("60.000000000000000000"))
    .bind(decimal("88.000000000000000000"))
    .bind(decimal("8.000000000000000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(format!(
        "admin-margin-interest-other-pair-{}",
        Uuid::now_v7().simple()
    ))
    .execute(&mut *fixture_tx)
    .await?;
    fixture_tx.commit().await?;

    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/margin/interest/summary?email={user_email}&pair_id={pair_id}&limit=10"
                ))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let payload = body_json(response).await?;
    assert_eq!(status, StatusCode::OK, "payload: {payload}");
    let summaries = payload["summaries"].as_array().unwrap();
    assert_eq!(summaries.len(), 3);
    let opened = summaries
        .iter()
        .find(|summary| summary["status"] == "opened")
        .unwrap();
    assert_eq!(opened["margin_asset"], quote_asset);
    assert_eq!(opened["position_count"], 1);
    assert_eq!(opened["borrowed_amount"], "40.000000000000000000");
    assert_eq!(opened["interest_amount"], "1.000000000000000000");
    let closed = summaries
        .iter()
        .find(|summary| summary["status"] == "closed")
        .unwrap();
    assert_eq!(closed["borrowed_amount"], "60.000000000000000000");
    assert_eq!(closed["interest_amount"], "2.000000000000000000");
    let liquidated = summaries
        .iter()
        .find(|summary| summary["status"] == "liquidated")
        .unwrap();
    assert_eq!(liquidated["borrowed_amount"], "75.000000000000000000");
    assert_eq!(liquidated["interest_amount"], "2.500000000000000000");

    let opened_response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/margin/interest/summary?email={user_email}&pair_id={pair_id}&status=opened&limit=10"
                ))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let opened_status = opened_response.status();
    let opened_payload = body_json(opened_response).await?;
    assert_eq!(opened_status, StatusCode::OK, "payload: {opened_payload}");
    let filtered = opened_payload["summaries"].as_array().unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0]["status"], "opened");
    assert_eq!(filtered[0]["borrowed_amount"], "40.000000000000000000");
    assert_eq!(filtered[0]["interest_amount"], "1.000000000000000000");

    Ok(())
}

#[tokio::test]
async fn margin_position_risk_snapshot_returns_owned_position_metrics() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let Some(redis) = redis_manager().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let other_user_id = create_user(&mut fixture_tx).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "RK").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "RQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_margin_product(&mut fixture_tx, pair_id, quote_asset).await;
    let position_id = sqlx::query(
        r#"INSERT INTO margin_positions
           (user_id, product_id, pair_id, margin_asset, direction, margin_amount,
            leverage, notional_amount, entry_price, status, idempotency_key)
           VALUES (?, ?, ?, ?, 'long', ?, ?, ?, ?, 'opened', ?)"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(quote_asset)
    .bind(decimal("20.000000000000000000"))
    .bind(decimal("5.00000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(format!("margin-risk-owned-{}", Uuid::now_v7().simple()))
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    sqlx::query("UPDATE margin_positions SET interest_amount = ? WHERE id = ?")
        .bind(decimal("1.500000000000000000"))
        .bind(position_id)
        .execute(&mut *fixture_tx)
        .await?;
    let other_position_id = sqlx::query(
        r#"INSERT INTO margin_positions
           (user_id, product_id, pair_id, margin_asset, direction, margin_amount,
            leverage, notional_amount, entry_price, status, idempotency_key)
           VALUES (?, ?, ?, ?, 'short', ?, ?, ?, ?, 'opened', ?)"#,
    )
    .bind(other_user_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(quote_asset)
    .bind(decimal("20.000000000000000000"))
    .bind(decimal("5.00000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(format!("margin-risk-other-{}", Uuid::now_v7().simple()))
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    fixture_tx.commit().await?;

    let observed_at_millis = Utc::now().timestamp_millis();
    cache_margin_ticker_at(&redis, &symbol, "84.000000000000000000", observed_at_millis).await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = user_routes().with_state(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_redis(redis.clone()),
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/margin/positions/{position_id}/risk"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let payload = body_json(response).await?;
    assert_eq!(status, StatusCode::OK, "payload: {payload}");
    assert_eq!(payload["risk"]["position_id"], position_id);
    assert_eq!(payload["risk"]["pair_id"], pair_id);
    assert_eq!(payload["risk"]["symbol"], symbol);
    assert_eq!(payload["risk"]["direction"], "long");
    assert_eq!(payload["risk"]["margin_amount"], "20.000000000000000000");
    assert_eq!(payload["risk"]["notional_amount"], "100.000000000000000000");
    assert_eq!(payload["risk"]["entry_price"], "100.000000000000000000");
    assert_eq!(payload["risk"]["mark_price"], "84.000000000000000000");
    assert_eq!(payload["risk"]["maintenance_margin_rate"], "0.05000000");
    assert_eq!(payload["risk"]["realized_pnl"], "-16.000000000000000000");
    assert_eq!(payload["risk"]["interest_amount"], "1.500000000000000000");
    assert_eq!(payload["risk"]["equity"], "2.500000000000000000");
    assert_eq!(
        payload["risk"]["maintenance_margin"],
        "5.000000000000000000"
    );
    assert_eq!(payload["risk"]["should_liquidate"], true);
    assert_eq!(payload["risk"]["observed_at"], observed_at_millis);

    let other_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/margin/positions/{other_position_id}/risk"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(other_response.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn margin_position_risk_snapshot_requires_user_scope_and_dependencies()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:42", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:7", TokenScope::Admin, 900).unwrap();
    let app = user_routes().with_state(AppState::new(settings));

    let missing_auth = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/margin/positions/1/risk")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(missing_auth.status(), StatusCode::UNAUTHORIZED);

    let admin = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/margin/positions/1/risk")
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(admin.status(), StatusCode::FORBIDDEN);

    let no_mysql = app
        .oneshot(
            Request::builder()
                .uri("/margin/positions/1/risk")
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let no_mysql_status = no_mysql.status();
    let no_mysql_payload = body_json(no_mysql).await?;
    assert_eq!(no_mysql_status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(no_mysql_payload["code"], "INTERNAL_ERROR");
    assert_eq!(
        no_mysql_payload["message"],
        "internal error: mysql pool is not configured for margin routes"
    );

    Ok(())
}

#[tokio::test]
async fn margin_position_risk_snapshot_rejects_closed_and_stale_positions()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let Some(redis) = redis_manager().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "SK").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "SQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_margin_product(&mut fixture_tx, pair_id, quote_asset).await;
    let closed_position_id = sqlx::query(
        r#"INSERT INTO margin_positions
           (user_id, product_id, pair_id, margin_asset, direction, margin_amount,
            leverage, notional_amount, entry_price, status, idempotency_key, closed_at)
           VALUES (?, ?, ?, ?, 'long', ?, ?, ?, ?, 'closed', ?, CURRENT_TIMESTAMP(6))"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(quote_asset)
    .bind(decimal("20.000000000000000000"))
    .bind(decimal("5.00000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(format!("margin-risk-closed-{}", Uuid::now_v7().simple()))
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    let opened_position_id = sqlx::query(
        r#"INSERT INTO margin_positions
           (user_id, product_id, pair_id, margin_asset, direction, margin_amount,
            leverage, notional_amount, entry_price, status, idempotency_key)
           VALUES (?, ?, ?, ?, 'long', ?, ?, ?, ?, 'opened', ?)"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(quote_asset)
    .bind(decimal("20.000000000000000000"))
    .bind(decimal("5.00000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(format!("margin-risk-stale-{}", Uuid::now_v7().simple()))
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    fixture_tx.commit().await?;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = user_routes().with_state(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_redis(redis.clone()),
    );

    cache_margin_ticker(&redis, &symbol, "90.000000000000000000").await?;
    let closed_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/margin/positions/{closed_position_id}/risk"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(closed_response.status(), StatusCode::BAD_REQUEST);

    cache_margin_ticker_at(
        &redis,
        &symbol,
        "90.000000000000000000",
        (Utc::now() - chrono::TimeDelta::seconds(120)).timestamp_millis(),
    )
    .await?;
    let stale_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/margin/positions/{opened_position_id}/risk"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let stale_status = stale_response.status();
    let stale_payload = body_json(stale_response).await?;
    assert_eq!(stale_status, StatusCode::BAD_REQUEST);
    assert_eq!(stale_payload["code"], "VALIDATION_ERROR");
    assert!(stale_payload["message"].as_str().unwrap().contains("stale"));

    Ok(())
}

#[tokio::test]
async fn margin_close_position_settles_realized_pnl_and_is_idempotent() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let Some(redis) = redis_manager().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "CB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "CQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_margin_product(&mut fixture_tx, pair_id, quote_asset).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&mut *fixture_tx)
        .await?;
    fixture_tx.commit().await?;

    cache_margin_ticker(&redis, &symbol, "100.000000000000000000").await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let hub = EventBroadcastHub::new(16);
    let _keepalive_hub = hub.clone();
    let mut private_events = hub.subscribe(&WebSocketChannel::private_user(user_id));
    let app = user_routes().with_state(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_redis(redis.clone())
            .with_event_broadcast_hub(hub),
    );
    let idempotency_key = format!("margin-close-{}", Uuid::now_v7().simple());
    let open_body = format!(
        r#"{{"product_id":{product_id},"direction":"long","margin_amount":"20.000000000000000000","leverage":"5.00000000","idempotency_key":"{idempotency_key}"}}"#
    );
    let open_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/margin/positions")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(open_body))
                .unwrap(),
        )
        .await?;
    let open_status = open_response.status();
    let open_payload = body_json(open_response).await?;
    assert_eq!(open_status, StatusCode::OK, "payload: {open_payload}");
    let position_id = open_payload["position"]["id"].as_u64().unwrap();
    let opened_event_message = timeout(Duration::from_millis(100), private_events.recv()).await??;
    let opened_event: Value = serde_json::from_str(opened_event_message.payload())?;
    assert_eq!(opened_event["type"], "margin.position.opened");
    assert_eq!(opened_event["position_id"], position_id);
    sqlx::query("UPDATE margin_positions SET interest_amount = ? WHERE id = ?")
        .bind(decimal("1.250000000000000000"))
        .bind(position_id)
        .execute(&pool)
        .await?;

    cache_margin_ticker(&redis, &symbol, "110.000000000000000000").await?;
    let close_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/margin/positions/{position_id}/close"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let close_status = close_response.status();
    let close_payload = body_json(close_response).await?;
    assert_eq!(close_status, StatusCode::OK, "payload: {close_payload}");
    assert_eq!(close_payload["position"]["id"], position_id);
    assert_eq!(close_payload["position"]["status"], "closed");
    assert_eq!(
        close_payload["position"]["exit_price"],
        "110.000000000000000000"
    );
    assert_eq!(
        close_payload["position"]["realized_pnl"],
        "10.000000000000000000"
    );
    assert_eq!(
        close_payload["position"]["interest_amount"],
        "1.250000000000000000"
    );
    assert!(close_payload["position"]["closed_at"].as_i64().is_some());

    let closed_event_message = timeout(Duration::from_millis(100), private_events.recv()).await??;
    let closed_event: Value = serde_json::from_str(closed_event_message.payload())?;
    assert_eq!(closed_event["type"], "margin.position.closed");
    assert_eq!(closed_event["position_id"], position_id);
    assert_eq!(closed_event["product_id"], product_id);
    assert_eq!(closed_event["pair_id"], pair_id);
    assert_eq!(closed_event["margin_asset"], quote_asset);
    assert_eq!(closed_event["direction"], "long");
    assert_eq!(closed_event["margin_amount"], "20.000000000000000000");
    assert_eq!(closed_event["exit_price"], "110.000000000000000000");
    assert_eq!(closed_event["realized_pnl"], "10.000000000000000000");
    assert_eq!(closed_event["interest_amount"], "1.250000000000000000");
    assert_eq!(closed_event["payout_amount"], "28.750000000000000000");
    assert_eq!(closed_event["status"], "closed");
    assert!(closed_event["closed_at"].as_i64().is_some());

    let (status, exit_price, realized_pnl): (String, Option<BigDecimal>, Option<BigDecimal>) =
        sqlx::query_as(
            "SELECT status, exit_price, realized_pnl FROM margin_positions WHERE id = ?",
        )
        .bind(position_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(status, "closed");
    assert_eq!(exit_price, Some(decimal("110.000000000000000000")));
    assert_eq!(realized_pnl, Some(decimal("10.000000000000000000")));
    let (available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(available, decimal("108.750000000000000000"));
    let (close_ledger_amount, close_ledger_count): (BigDecimal, i64) = sqlx::query_as(
        r#"SELECT COALESCE(SUM(amount), 0), COUNT(*)
           FROM wallet_ledger
           WHERE ref_type = 'margin_position'
             AND ref_id = ?
             AND change_type = 'margin_position_close'"#,
    )
    .bind(position_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(close_ledger_amount, decimal("28.750000000000000000"));
    assert_eq!(close_ledger_count, 1);

    cache_margin_ticker(&redis, &symbol, "120.000000000000000000").await?;
    let replay_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/margin/positions/{position_id}/close"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let replay_status = replay_response.status();
    let replay_payload = body_json(replay_response).await?;
    assert_eq!(replay_status, StatusCode::OK, "payload: {replay_payload}");
    assert_eq!(replay_payload["position"]["id"], position_id);
    assert_eq!(
        replay_payload["position"]["exit_price"],
        "110.000000000000000000"
    );
    assert!(
        timeout(Duration::from_millis(25), private_events.recv())
            .await
            .is_err(),
        "idempotent margin close replay must not publish duplicate private event"
    );

    let (available_after_replay,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(available_after_replay, decimal("108.750000000000000000"));
    let (close_ledger_count_after_replay,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'margin_position' AND ref_id = ? AND change_type = 'margin_position_close'",
    )
    .bind(position_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(close_ledger_count_after_replay, 1);

    Ok(())
}

#[tokio::test]
async fn margin_close_position_hides_other_users_position() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let owner_id = create_user(&mut fixture_tx).await;
    let other_user_id = create_user(&mut fixture_tx).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "OB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "OQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_margin_product(&mut fixture_tx, pair_id, quote_asset).await;
    let position_id = sqlx::query(
        r#"INSERT INTO margin_positions
           (user_id, product_id, pair_id, margin_asset, direction, margin_amount,
            leverage, notional_amount, entry_price, status, idempotency_key)
           VALUES (?, ?, ?, ?, 'long', ?, ?, ?, ?, 'opened', ?)"#,
    )
    .bind(owner_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(quote_asset)
    .bind(decimal("20.000000000000000000"))
    .bind(decimal("5.00000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(format!("margin-other-{}", Uuid::now_v7().simple()))
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    fixture_tx.commit().await?;

    let other_token = issue_token(
        &settings,
        format!("user:{other_user_id}"),
        TokenScope::User,
        900,
    )
    .unwrap();
    let response = user_routes()
        .with_state(AppState::new(settings).with_mysql(pool.clone()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/margin/positions/{position_id}/close"))
                .header("authorization", format!("Bearer {other_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let (status,): (String,) = sqlx::query_as("SELECT status FROM margin_positions WHERE id = ?")
        .bind(position_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(status, "opened");
    let (close_ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'margin_position' AND ref_id = ? AND change_type = 'margin_position_close'",
    )
    .bind(position_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(close_ledger_count, 0);

    Ok(())
}

#[tokio::test]
async fn margin_open_position_replays_existing_key_after_product_is_disabled()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "RB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "RQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_margin_product(&mut fixture_tx, pair_id, quote_asset).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&mut *fixture_tx)
        .await?;
    fixture_tx.commit().await?;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("margin-replay-disabled-{}", Uuid::now_v7().simple());
    let app = user_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let request_body = format!(
        r#"{{"product_id":{product_id},"direction":"long","margin_amount":"20.000000000000000000","leverage":"2.00000000","idempotency_key":"{idempotency_key}"}}"#
    );

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/margin/positions")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(first.status(), StatusCode::OK);
    let first_body = axum::body::to_bytes(first.into_body(), 65_536).await?;
    let first_payload: Value = serde_json::from_slice(&first_body)?;
    let first_position_id = first_payload["position"]["id"].as_u64().unwrap();

    sqlx::query("UPDATE margin_products SET status = 'disabled' WHERE id = ?")
        .bind(product_id)
        .execute(&pool)
        .await?;

    let second = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/margin/positions")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await?;
    let second_status = second.status();
    let second_body = axum::body::to_bytes(second.into_body(), 65_536).await?;
    let second_payload: Value = serde_json::from_slice(&second_body)?;
    assert_eq!(
        second_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&second_body)
    );
    assert_eq!(second_payload["position"]["id"], first_position_id);

    let conflict_body = format!(
        r#"{{"product_id":{product_id},"direction":"long","margin_amount":"30.000000000000000000","leverage":"2.00000000","idempotency_key":"{idempotency_key}"}}"#
    );
    let conflict_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/margin/positions")
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
        "conflict: margin idempotency key belongs to a different request"
    );

    let (available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(available, decimal("80.000000000000000000"));

    Ok(())
}

#[tokio::test]
async fn admin_disabling_margin_product_blocks_concurrent_open_after_commit()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "DM").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "DQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_margin_product(&mut fixture_tx, pair_id, quote_asset).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&mut *fixture_tx)
        .await?;
    fixture_tx.commit().await?;

    let mut disable_tx = pool.begin().await?;
    sqlx::query("SELECT id FROM margin_products WHERE id = ? FOR UPDATE")
        .bind(product_id)
        .execute(&mut *disable_tx)
        .await?;
    sqlx::query("UPDATE margin_products SET status = 'disabled' WHERE id = ?")
        .bind(product_id)
        .execute(&mut *disable_tx)
        .await?;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = user_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let idempotency_key = format!("margin-disable-race-{}", Uuid::now_v7().simple());
    let request_body = format!(
        r#"{{"product_id":{product_id},"direction":"long","margin_amount":"20.000000000000000000","leverage":"2.00000000","idempotency_key":"{idempotency_key}"}}"#
    );
    let mut open_task = tokio::spawn(async move {
        app.oneshot(
            Request::builder()
                .method("POST")
                .uri("/margin/positions")
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

    let (position_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM margin_positions WHERE user_id = ? AND product_id = ?",
    )
    .bind(user_id)
    .bind(product_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(position_count, 0);

    let (available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(available, decimal("100.000000000000000000"));

    Ok(())
}

#[tokio::test]
async fn margin_open_position_is_idempotent_for_repeated_key() -> Result<(), Box<dyn Error>> {
    assert_margin_open_position_idempotent(false).await
}

#[tokio::test]
async fn margin_open_position_concurrent_idempotency_key_debits_once() -> Result<(), Box<dyn Error>>
{
    assert_margin_open_position_idempotent(true).await
}

async fn assert_margin_open_position_idempotent(concurrent: bool) -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let (base_asset, base_symbol) = create_asset(&mut fixture_tx, "IB").await;
    let (quote_asset, quote_symbol) = create_asset(&mut fixture_tx, "IQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = create_pair(&mut fixture_tx, base_asset, quote_asset, &symbol).await;
    let product_id = seed_margin_product(&mut fixture_tx, pair_id, quote_asset).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&mut *fixture_tx)
        .await?;
    fixture_tx.commit().await?;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("margin-repeat-{}", Uuid::now_v7().simple());
    let app = user_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let request_body = format!(
        r#"{{"product_id":{product_id},"direction":"short","margin_amount":"20.000000000000000000","leverage":"2.00000000","idempotency_key":"{idempotency_key}"}}"#
    );

    let first_request = Request::builder()
        .method("POST")
        .uri("/margin/positions")
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(request_body.clone()))
        .unwrap();
    let second_request = Request::builder()
        .method("POST")
        .uri("/margin/positions")
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
    let first_position_id = first_payload["position"]["id"].as_u64().unwrap();

    let second_status = second.status();
    let second_body = axum::body::to_bytes(second.into_body(), 65_536).await?;
    let second_payload: Value = serde_json::from_slice(&second_body)?;
    assert_eq!(
        second_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&second_body)
    );
    assert_eq!(second_payload["position"]["id"], first_position_id);

    let (available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(available, decimal("80.000000000000000000"));

    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'margin_position' AND ref_id = ?",
    )
    .bind(first_position_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 1);

    Ok(())
}
