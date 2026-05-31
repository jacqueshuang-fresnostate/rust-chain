use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use bigdecimal::BigDecimal;
use chrono::{TimeDelta, Utc};
use exchange_api::{
    config::Settings,
    modules::{
        auth::{TokenScope, issue_token},
        convert::{ConvertQuoteCacheEntry, QuoteId, RedisConvertQuoteCache, routes::user_routes},
        events::{EventBroadcastHub, WebSocketChannel},
    },
    state::AppState,
};
use redis::AsyncCommands;
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
            eprintln!("skipping MySQL convert route test because DATABASE_URL is not set");
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
            eprintln!("skipping Redis convert route test because REDIS_URL is not set");
            return None;
        }
    };

    let client = redis::Client::open(redis_url).unwrap();
    Some(redis::aio::ConnectionManager::new(client).await.unwrap())
}

async fn create_user(pool: &MySqlPool) -> u64 {
    let email = format!("convert-route-{}@example.test", Uuid::now_v7().simple());
    sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(email)
        .bind("not-a-real-hash")
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id()
}

async fn create_agent(pool: &MySqlPool) -> (u64, u64) {
    let agent_user_id = create_user(pool).await;
    let agent_code = format!("convert-agent-{}", Uuid::now_v7().simple());
    let agent_id = sqlx::query("INSERT INTO agents (user_id, agent_code) VALUES (?, ?)")
        .bind(agent_user_id)
        .bind(agent_code)
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id();
    (agent_id, agent_user_id)
}

async fn refer_user_to_agent(pool: &MySqlPool, user_id: u64, agent_id: u64) {
    sqlx::query(
        r#"INSERT INTO user_referrals
           (user_id, direct_inviter_id, direct_inviter_type, root_agent_id, depth, path)
           VALUES (?, ?, 'agent', ?, 1, ?)"#,
    )
    .bind(user_id)
    .bind(agent_id)
    .bind(agent_id)
    .bind(format!("/{agent_id}/{user_id}"))
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_agent_commission_rule(pool: &MySqlPool, agent_id: u64) -> u64 {
    sqlx::query(
        r#"INSERT INTO agent_commission_rules (agent_id, product_type, commission_rate, status)
           VALUES (?, 'convert', ?, 'active')"#,
    )
    .bind(agent_id)
    .bind(decimal("0.05000000"))
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id()
}

async fn create_asset(pool: &MySqlPool, prefix: &str) -> u64 {
    let suffix = Uuid::now_v7().simple().to_string();
    let symbol = format!("{prefix}{}", &suffix[..12]);
    sqlx::query(
        "INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, 18, 'coin', 'active')",
    )
    .bind(&symbol)
    .bind(&symbol)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id()
}

async fn seed_convert_pair(pool: &MySqlPool, from_asset: u64, to_asset: u64) -> u64 {
    sqlx::query(
        r#"INSERT INTO convert_pairs
           (from_asset, to_asset, pricing_mode, spread_rate, min_amount, max_amount, enabled)
           VALUES (?, ?, 'fixed', ?, ?, NULL, true)"#,
    )
    .bind(from_asset)
    .bind(to_asset)
    .bind(decimal("0.00000000"))
    .bind(decimal("1.000000000000000000"))
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id()
}

async fn seed_convert_order(
    pool: &MySqlPool,
    user_id: u64,
    pair_id: u64,
    from_asset: u64,
    to_asset: u64,
) -> String {
    let quote_id = Uuid::now_v7().to_string();
    sqlx::query(
        r#"INSERT INTO convert_orders
           (quote_id, convert_pair_id, user_id, from_asset, to_asset, from_amount, to_amount, rate, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'pending')"#,
    )
    .bind(&quote_id)
    .bind(pair_id)
    .bind(user_id)
    .bind(from_asset)
    .bind(to_asset)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("20.000000000000000000"))
    .bind(decimal("2.000000000000000000"))
    .execute(pool)
    .await
    .unwrap();
    quote_id
}

async fn seed_convert_quote(
    pool: &MySqlPool,
    user_id: u64,
    pair_id: u64,
    from_asset: u64,
    to_asset: u64,
) -> String {
    let quote_id = Uuid::now_v7().to_string();
    sqlx::query(
        r#"INSERT INTO convert_quotes
           (quote_id, convert_pair_id, user_id, from_asset, to_asset, from_amount, to_amount, rate, spread_rate, expires_at, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, DATE_ADD(CURRENT_TIMESTAMP(6), INTERVAL 30 SECOND), 'quoted')"#,
    )
    .bind(&quote_id)
    .bind(pair_id)
    .bind(user_id)
    .bind(from_asset)
    .bind(to_asset)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("20.000000000000000000"))
    .bind(decimal("2.000000000000000000"))
    .bind(decimal("0.00000000"))
    .execute(pool)
    .await
    .unwrap();
    quote_id
}

#[tokio::test]
async fn convert_routes_require_auth_for_user_actions() {
    let app = user_routes().with_state(AppState::new(test_settings()));

    let orders_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/convert/orders")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(orders_response.status(), StatusCode::UNAUTHORIZED);

    let quote_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/convert/quote")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"from_asset_id":1,"to_asset_id":2,"from_amount":"10.000000000000000000"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(quote_response.status(), StatusCode::UNAUTHORIZED);

    let confirm_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/convert/confirm")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"quote_id":"00000000-0000-0000-0000-000000000000"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(confirm_response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn convert_routes_return_clear_error_without_mysql() {
    let settings = test_settings();
    let token = issue_token(&settings, "user:42", TokenScope::User, 900).unwrap();
    let response = user_routes()
        .with_state(AppState::new(settings))
        .oneshot(
            Request::builder()
                .uri("/convert/orders")
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
        "internal error: mysql pool is not configured for convert routes"
    );
}

#[tokio::test]
async fn convert_routes_list_pairs_and_user_orders() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let from_asset = create_asset(&pool, "CF").await;
    let to_asset = create_asset(&pool, "CT").await;
    let pair_id = seed_convert_pair(&pool, from_asset, to_asset).await;
    let quote_id = seed_convert_order(&pool, user_id, pair_id, from_asset, to_asset).await;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = user_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let pairs_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/convert/pairs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(pairs_response.status(), StatusCode::OK);
    let pairs_body = axum::body::to_bytes(pairs_response.into_body(), 8192).await?;
    let pairs: Value = serde_json::from_slice(&pairs_body)?;
    assert!(pairs["pairs"].as_array().unwrap().iter().any(|pair| {
        pair["id"] == pair_id
            && pair["from_asset_id"] == from_asset
            && pair["to_asset_id"] == to_asset
    }));

    let orders_response = app
        .oneshot(
            Request::builder()
                .uri("/convert/orders")
                .header("authorization", format!("Bearer {token}"))
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
    let orders: Value = serde_json::from_slice(&orders_body)?;
    assert!(
        orders["orders"]
            .as_array()
            .unwrap()
            .iter()
            .any(|order| { order["quote_id"] == quote_id && order["status"] == "pending" })
    );

    cleanup_fixture(&pool, &quote_id, pair_id, from_asset, to_asset, user_id).await?;
    Ok(())
}

#[tokio::test]
async fn convert_confirm_settles_wallet_balances_and_marks_order_completed()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let Some(redis) = redis_manager().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let from_asset = create_asset(&pool, "CS").await;
    let to_asset = create_asset(&pool, "CR").await;
    let pair_id = seed_convert_pair(&pool, from_asset, to_asset).await;
    let quote_id = seed_convert_quote(&pool, user_id, pair_id, from_asset, to_asset).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(from_asset)
        .bind(decimal("10.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(to_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;

    let parsed_quote_id = QuoteId(Uuid::parse_str(&quote_id)?);
    RedisConvertQuoteCache::new(redis.clone())
        .save_quote_ttl(ConvertQuoteCacheEntry {
            quote_id: parsed_quote_id.clone(),
            user_id: user_id.to_string(),
            from_asset: from_asset.to_string(),
            to_asset: to_asset.to_string(),
            from_amount: decimal("10.000000000000000000"),
            to_amount: decimal("20.000000000000000000"),
            expires_at: Utc::now() + TimeDelta::seconds(30),
            redis_key: format!("convert:quote:{}", parsed_quote_id.0),
            ttl_seconds: 30,
        })
        .await
        .unwrap();

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let hub = EventBroadcastHub::new(16);
    let mut private_events = hub.subscribe(&WebSocketChannel::private_user(user_id));
    let app = user_routes().with_state(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_redis(redis.clone())
            .with_event_broadcast_hub(hub),
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/convert/confirm")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(r#"{{"quote_id":"{quote_id}"}}"#)))
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
    let confirmed: Value = serde_json::from_slice(&body)?;
    assert_eq!(confirmed["confirmed"], true);
    assert_eq!(confirmed["quote_id"], quote_id);
    let event: Value = serde_json::from_str(private_events.recv().await?.payload())?;
    assert_eq!(event["type"], "convert.confirmed");
    assert_eq!(event["quote_id"], quote_id);
    assert_eq!(event["status"], "completed");

    let (from_available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(from_asset)
            .fetch_one(&pool)
            .await?;
    let (to_available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(to_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(from_available, decimal("0.000000000000000000"));
    assert_eq!(to_available, decimal("20.000000000000000000"));

    let (status,): (String,) =
        sqlx::query_as("SELECT status FROM convert_orders WHERE quote_id = ? AND user_id = ?")
            .bind(&quote_id)
            .bind(user_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(status, "completed");

    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'convert_order' AND ref_id = ?",
    )
    .bind(&quote_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 2);

    let mut raw_redis = redis.clone();
    let _: usize = raw_redis
        .del(format!("convert:quote:{}", parsed_quote_id.0))
        .await?;
    cleanup_fixture(&pool, &quote_id, pair_id, from_asset, to_asset, user_id).await?;
    Ok(())
}

#[tokio::test]
async fn convert_confirm_creates_pending_agent_commission_for_referred_user()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let Some(redis) = redis_manager().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (agent_id, agent_user_id) = create_agent(&pool).await;
    let user_id = create_user(&pool).await;
    refer_user_to_agent(&pool, user_id, agent_id).await;
    let rule_id = seed_agent_commission_rule(&pool, agent_id).await;
    let from_asset = create_asset(&pool, "CA").await;
    let to_asset = create_asset(&pool, "CG").await;
    let pair_id = seed_convert_pair(&pool, from_asset, to_asset).await;
    let quote_id = seed_convert_quote(&pool, user_id, pair_id, from_asset, to_asset).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(from_asset)
        .bind(decimal("10.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(to_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;

    let parsed_quote_id = QuoteId(Uuid::parse_str(&quote_id)?);
    RedisConvertQuoteCache::new(redis.clone())
        .save_quote_ttl(ConvertQuoteCacheEntry {
            quote_id: parsed_quote_id.clone(),
            user_id: user_id.to_string(),
            from_asset: from_asset.to_string(),
            to_asset: to_asset.to_string(),
            from_amount: decimal("10.000000000000000000"),
            to_amount: decimal("20.000000000000000000"),
            expires_at: Utc::now() + TimeDelta::seconds(30),
            redis_key: format!("convert:quote:{}", parsed_quote_id.0),
            ttl_seconds: 30,
        })
        .await
        .unwrap();

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = user_routes().with_state(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_redis(redis.clone()),
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/convert/confirm")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(r#"{{"quote_id":"{quote_id}"}}"#)))
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

    let (record_count, source_id, source_amount, commission_amount, status): (
        i64,
        String,
        BigDecimal,
        BigDecimal,
        String,
    ) = sqlx::query_as(
        r#"SELECT COUNT(*), COALESCE(MAX(source_id), ''), COALESCE(MAX(source_amount), 0),
                  COALESCE(MAX(commission_amount), 0), COALESCE(MAX(status), '')
           FROM agent_commission_records
           WHERE agent_id = ? AND user_id = ? AND source_type = 'convert_order'"#,
    )
    .bind(agent_id)
    .bind(user_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(record_count, 1);
    assert_eq!(source_id, quote_id);
    assert_eq!(source_amount, decimal("10.000000000000000000"));
    assert_eq!(commission_amount, decimal("0.500000000000000000"));
    assert_eq!(status, "pending");

    let mut raw_redis = redis.clone();
    let _: usize = raw_redis
        .del(format!("convert:quote:{}", parsed_quote_id.0))
        .await?;
    cleanup_agent_commission_fixture(&pool, rule_id, user_id, agent_id, agent_user_id).await?;
    cleanup_fixture(&pool, &quote_id, pair_id, from_asset, to_asset, user_id).await?;
    Ok(())
}

#[tokio::test]
async fn convert_confirm_rolls_back_order_when_settlement_fails_and_allows_retry()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let Some(redis) = redis_manager().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let from_asset = create_asset(&pool, "CB").await;
    let to_asset = create_asset(&pool, "CD").await;
    let pair_id = seed_convert_pair(&pool, from_asset, to_asset).await;
    let quote_id = seed_convert_quote(&pool, user_id, pair_id, from_asset, to_asset).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(from_asset)
        .bind(decimal("10.000000000000000000"))
        .execute(&pool)
        .await?;

    let parsed_quote_id = QuoteId(Uuid::parse_str(&quote_id)?);
    RedisConvertQuoteCache::new(redis.clone())
        .save_quote_ttl(ConvertQuoteCacheEntry {
            quote_id: parsed_quote_id.clone(),
            user_id: user_id.to_string(),
            from_asset: from_asset.to_string(),
            to_asset: to_asset.to_string(),
            from_amount: decimal("10.000000000000000000"),
            to_amount: decimal("20.000000000000000000"),
            expires_at: Utc::now() + TimeDelta::seconds(30),
            redis_key: format!("convert:quote:{}", parsed_quote_id.0),
            ttl_seconds: 30,
        })
        .await
        .unwrap();

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = user_routes().with_state(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_redis(redis.clone()),
    );

    let failed_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/convert/confirm")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(r#"{{"quote_id":"{quote_id}"}}"#)))
                .unwrap(),
        )
        .await
        .unwrap();
    let failed_status = failed_response.status();
    let failed_body = axum::body::to_bytes(failed_response.into_body(), 8192).await?;
    assert_eq!(
        failed_status,
        StatusCode::BAD_REQUEST,
        "payload: {}",
        String::from_utf8_lossy(&failed_body)
    );

    let (order_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM convert_orders WHERE quote_id = ?")
            .bind(&quote_id)
            .fetch_one(&pool)
            .await?;
    if order_count != 0 {
        let mut raw_redis = redis.clone();
        let _: usize = raw_redis
            .del(format!("convert:quote:{}", parsed_quote_id.0))
            .await?;
        cleanup_fixture(&pool, &quote_id, pair_id, from_asset, to_asset, user_id).await?;
    }
    assert_eq!(order_count, 0);

    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(to_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;

    let retry_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/convert/confirm")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(r#"{{"quote_id":"{quote_id}"}}"#)))
                .unwrap(),
        )
        .await
        .unwrap();
    let retry_status = retry_response.status();
    let retry_body = axum::body::to_bytes(retry_response.into_body(), 8192).await?;
    assert_eq!(
        retry_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&retry_body)
    );

    let (status,): (String,) =
        sqlx::query_as("SELECT status FROM convert_orders WHERE quote_id = ? AND user_id = ?")
            .bind(&quote_id)
            .bind(user_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(status, "completed");

    let mut raw_redis = redis.clone();
    let _: usize = raw_redis
        .del(format!("convert:quote:{}", parsed_quote_id.0))
        .await?;
    cleanup_fixture(&pool, &quote_id, pair_id, from_asset, to_asset, user_id).await?;
    Ok(())
}

async fn cleanup_agent_commission_fixture(
    pool: &MySqlPool,
    rule_id: u64,
    user_id: u64,
    agent_id: u64,
    agent_user_id: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM agent_commission_records WHERE agent_id = ? AND user_id = ?")
        .bind(agent_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM agent_commission_rules WHERE id = ?")
        .bind(rule_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM user_referrals WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM agents WHERE id = ?")
        .bind(agent_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(agent_user_id)
        .execute(pool)
        .await?;
    Ok(())
}

async fn cleanup_fixture(
    pool: &MySqlPool,
    quote_id: &str,
    pair_id: u64,
    from_asset: u64,
    to_asset: u64,
    user_id: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM wallet_ledger WHERE ref_type = 'convert_order' AND ref_id = ?")
        .bind(quote_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM convert_orders WHERE quote_id = ?")
        .bind(quote_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM convert_quotes WHERE quote_id = ?")
        .bind(quote_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ? AND asset_id IN (?, ?)")
        .bind(user_id)
        .bind(from_asset)
        .bind(to_asset)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM convert_pairs WHERE id = ?")
        .bind(pair_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(from_asset)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(to_asset)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
