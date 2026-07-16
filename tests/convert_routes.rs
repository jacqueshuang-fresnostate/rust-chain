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
        market::market_ticker_redis_key,
    },
    state::AppState,
};
use redis::AsyncCommands;
use secrecy::SecretString;
use serde_json::{Value, json};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::{error::Error, str::FromStr};
use tower::ServiceExt;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct AgentCommissionRecordAssertion {
    agent_id: u64,
    source_id: String,
    source_amount: BigDecimal,
    commission_rate: BigDecimal,
    commission_amount: BigDecimal,
    status: String,
    payout_asset_id: Option<u64>,
}

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
    let agent_id = sqlx::query("INSERT INTO agents (user_id, agent_code, path) VALUES (?, ?, '')")
        .bind(agent_user_id)
        .bind(agent_code)
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id();
    sqlx::query("UPDATE agents SET root_agent_id = ?, path = ? WHERE id = ?")
        .bind(agent_id)
        .bind(format!("/agent:{agent_id}"))
        .bind(agent_id)
        .execute(pool)
        .await
        .unwrap();
    (agent_id, agent_user_id)
}

async fn create_child_agent(pool: &MySqlPool, parent_agent_id: u64, level: i32) -> (u64, u64) {
    let (agent_id, agent_user_id) = create_agent(pool).await;
    let (root_agent_id, parent_path): (u64, String) =
        sqlx::query_as("SELECT root_agent_id, path FROM agents WHERE id = ?")
            .bind(parent_agent_id)
            .fetch_one(pool)
            .await
            .unwrap();
    sqlx::query(
        "UPDATE agents SET parent_agent_id = ?, root_agent_id = ?, level = ?, path = ? WHERE id = ?",
    )
    .bind(parent_agent_id)
    .bind(root_agent_id)
    .bind(level)
    .bind(format!("{parent_path}/agent:{agent_id}"))
    .bind(agent_id)
    .execute(pool)
    .await
    .unwrap();
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

async fn seed_agent_commission_rule_with(
    pool: &MySqlPool,
    agent_id: u64,
    commission_rate: &str,
    status: &str,
) -> u64 {
    sqlx::query(
        r#"INSERT INTO agent_commission_rules (agent_id, product_type, commission_rate, status)
           VALUES (?, 'convert', ?, ?)"#,
    )
    .bind(agent_id)
    .bind(decimal(commission_rate))
    .bind(status)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id()
}

async fn create_asset(pool: &MySqlPool, prefix: &str) -> u64 {
    create_asset_with_precision(pool, prefix, 18).await
}

async fn create_asset_with_precision(pool: &MySqlPool, prefix: &str, precision_scale: i32) -> u64 {
    let suffix = Uuid::now_v7().simple().to_string();
    let symbol = format!("{prefix}{}", &suffix[..12]);
    sqlx::query(
        "INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, ?, 'coin', 'active')",
    )
    .bind(&symbol)
    .bind(&symbol)
    .bind(precision_scale)
    .execute(pool)
    .await
    .unwrap()
        .last_insert_id()
}

async fn asset_symbol(pool: &MySqlPool, asset_id: u64) -> String {
    let (symbol,): (String,) = sqlx::query_as("SELECT symbol FROM assets WHERE id = ?")
        .bind(asset_id)
        .fetch_one(pool)
        .await
        .unwrap();
    symbol
}

async fn seed_convert_pair(pool: &MySqlPool, from_asset: u64, to_asset: u64) -> u64 {
    seed_convert_pair_with_pricing(pool, from_asset, to_asset, "fixed").await
}

async fn seed_convert_pair_with_pricing(
    pool: &MySqlPool,
    from_asset: u64,
    to_asset: u64,
    pricing_mode: &str,
) -> u64 {
    sqlx::query(
        r#"INSERT INTO convert_pairs
           (from_asset, to_asset, pricing_mode, spread_rate, min_amount, max_amount, enabled)
           VALUES (?, ?, ?, ?, ?, NULL, true)"#,
    )
    .bind(from_asset)
    .bind(to_asset)
    .bind(pricing_mode)
    .bind(decimal("0.00000000"))
    .bind(decimal("1.000000000000000000"))
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id()
}

async fn seed_trading_pair(pool: &MySqlPool, base_asset: u64, quote_asset: u64) -> String {
    let base_symbol = asset_symbol(pool, base_asset).await;
    let quote_symbol = asset_symbol(pool, quote_asset).await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, 18, 18, ?, 'active', 'external')"#,
    )
    .bind(base_asset)
    .bind(quote_asset)
    .bind(&symbol)
    .bind(decimal("1.000000000000000000"))
    .execute(pool)
    .await
    .unwrap();
    symbol
}

async fn cache_market_ticker(
    redis: &redis::aio::ConnectionManager,
    symbol: &str,
    last_price: &str,
) -> Result<(), redis::RedisError> {
    let mut connection = redis.clone();
    let payload = json!({
        "symbol": symbol,
        "last_price": last_price,
        "observed_at": 1_717_171_000_000_i64,
    })
    .to_string();
    connection
        .set(market_ticker_redis_key(symbol), payload)
        .await
}

async fn seed_convert_rule(pool: &MySqlPool, pair_id: u64, fixed_rate: &str) -> u64 {
    sqlx::query(
        r#"INSERT INTO new_coin_convert_rules
           (convert_pair_id, rate_source, fixed_rate, status)
           VALUES (?, 'fixed', ?, 'active')"#,
    )
    .bind(pair_id)
    .bind(decimal(fixed_rate))
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
    let from_symbol = asset_symbol(&pool, from_asset).await;
    let to_symbol = asset_symbol(&pool, to_asset).await;
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
            && pair["from_asset_symbol"] == from_symbol
            && pair["to_asset_id"] == to_asset
            && pair["to_asset_symbol"] == to_symbol
            && pair["target_min_amount"] == "0.000000000000000000"
            && pair["target_max_amount"].is_null()
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
async fn convert_quote_supports_reverse_direction_from_single_pair() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let Some(redis) = redis_manager().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let from_asset = create_asset(&pool, "CRF").await;
    let to_asset = create_asset(&pool, "CRT").await;
    let pair_id = seed_convert_pair(&pool, from_asset, to_asset).await;
    seed_convert_rule(&pool, pair_id, "2.000000000000000000").await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(to_asset)
        .bind(decimal("10.000000000000000000"))
        .execute(&pool)
        .await?;

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
                .uri("/convert/quote")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "from_asset_id": to_asset,
                        "to_asset_id": from_asset,
                        "from_amount": "10.000000000000000000"
                    })
                    .to_string(),
                ))
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
    let quote: Value = serde_json::from_slice(&body)?;
    assert_eq!(quote["convert_pair_id"], pair_id);
    assert_eq!(quote["from_asset_id"], to_asset);
    assert_eq!(quote["to_asset_id"], from_asset);
    assert_eq!(
        BigDecimal::from_str(quote["rate"].as_str().unwrap()).unwrap(),
        decimal("0.500000000000000000")
    );
    assert_eq!(
        BigDecimal::from_str(quote["to_amount"].as_str().unwrap()).unwrap(),
        decimal("5.000000000000000000")
    );

    let quote_id = quote["quote_id"].as_str().unwrap().to_owned();
    let mut raw_redis = redis.clone();
    let _: usize = raw_redis.del(format!("convert:quote:{quote_id}")).await?;
    cleanup_fixture(&pool, &quote_id, pair_id, from_asset, to_asset, user_id).await?;
    Ok(())
}

#[tokio::test]
async fn convert_quote_applies_pair_fee_rate_and_settles_net_amount() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let Some(redis) = redis_manager().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let from_asset = create_asset(&pool, "CFF").await;
    let to_asset = create_asset(&pool, "CFT").await;
    let pair_id = seed_convert_pair(&pool, from_asset, to_asset).await;
    sqlx::query("UPDATE convert_pairs SET fee_rate = ? WHERE id = ?")
        .bind(decimal("0.01000000"))
        .bind(pair_id)
        .execute(&pool)
        .await?;
    seed_convert_rule(&pool, pair_id, "2.000000000000000000").await;
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
                .method("POST")
                .uri("/convert/quote")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "from_asset_id": from_asset,
                        "to_asset_id": to_asset,
                        "from_amount": "10.000000000000000000"
                    })
                    .to_string(),
                ))
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
    let quote: Value = serde_json::from_slice(&body)?;
    assert_eq!(quote["fee_rate"], "0.01000000");
    assert_eq!(
        BigDecimal::from_str(quote["fee_amount"].as_str().unwrap()).unwrap(),
        decimal("0.100000000000000000")
    );
    assert_eq!(
        BigDecimal::from_str(quote["to_amount"].as_str().unwrap()).unwrap(),
        decimal("19.800000000000000000")
    );

    let quote_id = quote["quote_id"].as_str().unwrap().to_owned();
    let confirm = app
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
    let confirm_status = confirm.status();
    let confirm_body = axum::body::to_bytes(confirm.into_body(), 8192).await?;
    assert_eq!(
        confirm_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&confirm_body)
    );

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
    assert_eq!(to_available, decimal("19.800000000000000000"));

    let (order_fee_rate, order_fee_amount): (BigDecimal, BigDecimal) =
        sqlx::query_as("SELECT fee_rate, fee_amount FROM convert_orders WHERE quote_id = ?")
            .bind(&quote_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(order_fee_rate, decimal("0.01000000"));
    assert_eq!(order_fee_amount, decimal("0.100000000000000000"));

    let mut raw_redis = redis.clone();
    let _: usize = raw_redis.del(format!("convert:quote:{quote_id}")).await?;
    cleanup_fixture(&pool, &quote_id, pair_id, from_asset, to_asset, user_id).await?;
    Ok(())
}

#[tokio::test]
async fn convert_quote_uses_target_asset_limits_for_reverse_direction() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let Some(redis) = redis_manager().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let from_asset = create_asset(&pool, "CLF").await;
    let to_asset = create_asset(&pool, "CLT").await;
    let pair_id = sqlx::query(
        r#"INSERT INTO convert_pairs
           (from_asset, to_asset, pricing_mode, spread_rate, min_amount, max_amount,
            target_min_amount, target_max_amount, enabled)
           VALUES (?, ?, 'fixed', ?, ?, ?, ?, ?, true)"#,
    )
    .bind(from_asset)
    .bind(to_asset)
    .bind(decimal("0.00000000"))
    .bind(decimal("1.000000000000000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(decimal("20.000000000000000000"))
    .bind(decimal("30.000000000000000000"))
    .execute(&pool)
    .await?
    .last_insert_id();
    seed_convert_rule(&pool, pair_id, "2.000000000000000000").await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(to_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = user_routes().with_state(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_redis(redis),
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/convert/quote")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "from_asset_id": to_asset,
                        "to_asset_id": from_asset,
                        "from_amount": "10.000000000000000000"
                    })
                    .to_string(),
                ))
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
    let payload: Value = serde_json::from_slice(&body)?;
    assert_eq!(
        payload["message"],
        "validation error: convert amount is below pair minimum"
    );

    cleanup_fixture(
        &pool,
        "directional-limit-no-quote",
        pair_id,
        from_asset,
        to_asset,
        user_id,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn convert_quote_supports_market_pricing_from_cached_ticker() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let Some(redis) = redis_manager().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let base_asset = create_asset(&pool, "CMB").await;
    let quote_asset = create_asset(&pool, "CMQ").await;
    let market_symbol = seed_trading_pair(&pool, base_asset, quote_asset).await;
    let pair_id = seed_convert_pair_with_pricing(&pool, quote_asset, base_asset, "market").await;
    cache_market_ticker(&redis, &market_symbol, "2.000000000000000000").await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("10.000000000000000000"))
        .execute(&pool)
        .await?;

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
                .uri("/convert/quote")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "from_asset_id": quote_asset,
                        "to_asset_id": base_asset,
                        "from_amount": "10.000000000000000000"
                    })
                    .to_string(),
                ))
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
    let quote: Value = serde_json::from_slice(&body)?;
    assert_eq!(quote["convert_pair_id"], pair_id);
    assert_eq!(quote["from_asset_id"], quote_asset);
    assert_eq!(quote["to_asset_id"], base_asset);
    assert_eq!(
        BigDecimal::from_str(quote["rate"].as_str().unwrap()).unwrap(),
        decimal("0.500000000000000000")
    );
    assert_eq!(
        BigDecimal::from_str(quote["to_amount"].as_str().unwrap()).unwrap(),
        decimal("5.000000000000000000")
    );

    let quote_id = quote["quote_id"].as_str().unwrap().to_owned();
    let mut raw_redis = redis.clone();
    let _: usize = raw_redis.del(format!("convert:quote:{quote_id}")).await?;
    let _: usize = raw_redis
        .del(market_ticker_redis_key(&market_symbol))
        .await?;
    sqlx::query("DELETE FROM trading_pairs WHERE symbol = ?")
        .bind(&market_symbol)
        .execute(&pool)
        .await?;
    cleanup_fixture(&pool, &quote_id, pair_id, quote_asset, base_asset, user_id).await?;
    Ok(())
}

#[tokio::test]
async fn convert_market_quote_truncates_target_amount_to_asset_precision()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let Some(redis) = redis_manager().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let btc_asset = create_asset_with_precision(&pool, "CBTC", 8).await;
    let usdt_asset = create_asset_with_precision(&pool, "CUSDT", 18).await;
    let market_symbol = seed_trading_pair(&pool, btc_asset, usdt_asset).await;
    let pair_id = seed_convert_pair_with_pricing(&pool, usdt_asset, btc_asset, "market").await;
    cache_market_ticker(&redis, &market_symbol, "3.000000000000000000").await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(usdt_asset)
        .bind(decimal("1.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(btc_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = user_routes().with_state(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_redis(redis.clone()),
    );

    let quote_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/convert/quote")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "from_asset_id": usdt_asset,
                        "to_asset_id": btc_asset,
                        "from_amount": "1.000000000000000000"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let quote_status = quote_response.status();
    let quote_body = axum::body::to_bytes(quote_response.into_body(), 8192).await?;
    assert_eq!(
        quote_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&quote_body)
    );
    let quote: Value = serde_json::from_slice(&quote_body)?;
    assert_eq!(quote["convert_pair_id"], pair_id);
    assert_eq!(
        BigDecimal::from_str(quote["to_amount"].as_str().unwrap()).unwrap(),
        decimal("0.33333333")
    );

    let quote_id = quote["quote_id"].as_str().unwrap().to_owned();
    let confirm_response = app
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
    let confirm_status = confirm_response.status();
    let confirm_body = axum::body::to_bytes(confirm_response.into_body(), 8192).await?;
    assert_eq!(
        confirm_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&confirm_body)
    );

    let (btc_available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(btc_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        btc_available.normalized(),
        decimal("0.33333333").normalized()
    );

    let (ledger_after,): (BigDecimal,) = sqlx::query_as(
        r#"SELECT available_after
           FROM wallet_ledger
           WHERE ref_type = 'convert_order' AND ref_id = ? AND asset_id = ?
           ORDER BY id DESC
           LIMIT 1"#,
    )
    .bind(&quote_id)
    .bind(btc_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        ledger_after.normalized(),
        decimal("0.33333333").normalized()
    );

    let mut raw_redis = redis.clone();
    let _: usize = raw_redis.del(format!("convert:quote:{quote_id}")).await?;
    let _: usize = raw_redis
        .del(market_ticker_redis_key(&market_symbol))
        .await?;
    sqlx::query("DELETE FROM trading_pairs WHERE symbol = ?")
        .bind(&market_symbol)
        .execute(&pool)
        .await?;
    cleanup_fixture(&pool, &quote_id, pair_id, usdt_asset, btc_asset, user_id).await?;
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
            fee_rate: decimal("0.00000000"),
            fee_amount: decimal("0.000000000000000000"),
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
    let (root_agent_id, root_agent_user_id) = create_agent(&pool).await;
    let (level_two_agent_id, level_two_agent_user_id) =
        create_child_agent(&pool, root_agent_id, 2).await;
    let (owner_agent_id, owner_agent_user_id) =
        create_child_agent(&pool, level_two_agent_id, 3).await;
    let user_id = create_user(&pool).await;
    refer_user_to_agent(&pool, user_id, owner_agent_id).await;
    let root_rule_id =
        seed_agent_commission_rule_with(&pool, root_agent_id, "0.10000000", "active").await;
    let level_two_rule_id =
        seed_agent_commission_rule_with(&pool, level_two_agent_id, "0.08000000", "active").await;
    let owner_rule_id =
        seed_agent_commission_rule_with(&pool, owner_agent_id, "0.05000000", "active").await;
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
            fee_rate: decimal("0.00000000"),
            fee_amount: decimal("0.000000000000000000"),
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

    let records = sqlx::query_as::<_, AgentCommissionRecordAssertion>(
        r#"SELECT agent_id, source_id, source_amount, commission_rate,
                  commission_amount, status, payout_asset_id
           FROM agent_commission_records
           WHERE user_id = ? AND source_type = 'convert_order' AND source_id = ?
           ORDER BY commission_rate DESC"#,
    )
    .bind(user_id)
    .bind(&quote_id)
    .fetch_all(&pool)
    .await?;
    assert_eq!(records.len(), 3);
    assert_eq!(records[0].agent_id, owner_agent_id);
    assert_eq!(records[0].source_id, quote_id);
    assert_eq!(records[0].source_amount, decimal("10.000000000000000000"));
    assert_eq!(records[0].commission_rate, decimal("0.05000000"));
    assert_eq!(
        records[0].commission_amount,
        decimal("0.500000000000000000")
    );
    assert_eq!(records[1].agent_id, level_two_agent_id);
    assert_eq!(records[1].commission_rate, decimal("0.03000000"));
    assert_eq!(
        records[1].commission_amount,
        decimal("0.300000000000000000")
    );
    assert_eq!(records[2].agent_id, root_agent_id);
    assert_eq!(records[2].commission_rate, decimal("0.02000000"));
    assert_eq!(
        records[2].commission_amount,
        decimal("0.200000000000000000")
    );
    assert!(records.iter().all(|record| record.status == "pending"));
    assert!(
        records
            .iter()
            .all(|record| record.payout_asset_id == Some(from_asset))
    );

    let mut raw_redis = redis.clone();
    let _: usize = raw_redis
        .del(format!("convert:quote:{}", parsed_quote_id.0))
        .await?;
    sqlx::query("DELETE FROM agent_commission_records WHERE user_id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM agent_commission_rules WHERE id IN (?, ?, ?)")
        .bind(root_rule_id)
        .bind(level_two_rule_id)
        .bind(owner_rule_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM user_referrals WHERE user_id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM agents WHERE id = ?")
        .bind(owner_agent_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM agents WHERE id = ?")
        .bind(level_two_agent_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM agents WHERE id = ?")
        .bind(root_agent_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id IN (?, ?, ?)")
        .bind(owner_agent_user_id)
        .bind(level_two_agent_user_id)
        .bind(root_agent_user_id)
        .execute(&pool)
        .await?;
    cleanup_fixture(&pool, &quote_id, pair_id, from_asset, to_asset, user_id).await?;
    Ok(())
}

#[tokio::test]
async fn convert_confirm_skips_disabled_agent_commission_rule() -> Result<(), Box<dyn Error>> {
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
    let rule_id = seed_agent_commission_rule_with(&pool, agent_id, "0.05000000", "disabled").await;
    let from_asset = create_asset(&pool, "CDR").await;
    let to_asset = create_asset(&pool, "CDT").await;
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
            fee_rate: decimal("0.00000000"),
            fee_amount: decimal("0.000000000000000000"),
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

    let (record_count,): (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*)
           FROM agent_commission_records
           WHERE agent_id = ? AND user_id = ? AND source_type = 'convert_order'"#,
    )
    .bind(agent_id)
    .bind(user_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(record_count, 0);

    let mut raw_redis = redis.clone();
    let _: usize = raw_redis
        .del(format!("convert:quote:{}", parsed_quote_id.0))
        .await?;
    cleanup_agent_commission_fixture(&pool, rule_id, user_id, agent_id, agent_user_id).await?;
    cleanup_fixture(&pool, &quote_id, pair_id, from_asset, to_asset, user_id).await?;
    Ok(())
}

#[tokio::test]
async fn convert_confirm_uses_latest_active_agent_commission_rule() -> Result<(), Box<dyn Error>> {
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
    let old_rule_id =
        seed_agent_commission_rule_with(&pool, agent_id, "0.05000000", "active").await;
    let latest_rule_id =
        seed_agent_commission_rule_with(&pool, agent_id, "0.08000000", "active").await;
    let from_asset = create_asset(&pool, "CLR").await;
    let to_asset = create_asset(&pool, "CLT").await;
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
            fee_rate: decimal("0.00000000"),
            fee_amount: decimal("0.000000000000000000"),
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

    let (commission_amount,): (BigDecimal,) = sqlx::query_as(
        r#"SELECT commission_amount
           FROM agent_commission_records
           WHERE agent_id = ? AND user_id = ? AND source_type = 'convert_order'
           LIMIT 1"#,
    )
    .bind(agent_id)
    .bind(user_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(commission_amount, decimal("0.800000000000000000"));

    let mut raw_redis = redis.clone();
    let _: usize = raw_redis
        .del(format!("convert:quote:{}", parsed_quote_id.0))
        .await?;
    sqlx::query("DELETE FROM agent_commission_records WHERE agent_id = ? AND user_id = ?")
        .bind(agent_id)
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM agent_commission_rules WHERE id IN (?, ?)")
        .bind(old_rule_id)
        .bind(latest_rule_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM user_referrals WHERE user_id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM agents WHERE id = ?")
        .bind(agent_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(agent_user_id)
        .execute(&pool)
        .await?;
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
            fee_rate: decimal("0.00000000"),
            fee_amount: decimal("0.000000000000000000"),
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
    sqlx::query("DELETE FROM new_coin_convert_rules WHERE convert_pair_id = ?")
        .bind(pair_id)
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
