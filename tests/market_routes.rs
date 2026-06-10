use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use bigdecimal::BigDecimal;
use chrono::{TimeZone, Utc};
use exchange_api::{
    config::Settings,
    modules::market::{
        KlineQuery, MarketDepthCacheEntry, MarketDepthLevel, MarketTickerCacheEntry, routes,
    },
    state::AppState,
};
use redis::AsyncCommands;
use secrecy::SecretString;
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::{error::Error, str::FromStr};
use tower::ServiceExt;
use uuid::Uuid;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

fn env_or_skip(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => {
            eprintln!("skipping integration test because {name} is not set");
            None
        }
    }
}

fn unique_symbol(prefix: &str) -> String {
    let uuid = Uuid::now_v7().simple().to_string();
    format!("{}{}", prefix, &uuid[16..32]).to_ascii_uppercase()
}

async fn mysql_pool_or_skip() -> Result<Option<MySqlPool>, Box<dyn Error>> {
    let Some(database_url) = env_or_skip("DATABASE_URL") else {
        return Ok(None);
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    Ok(Some(pool))
}

async fn create_market_asset(pool: &MySqlPool, symbol: &str) -> Result<u64, Box<dyn Error>> {
    let result = sqlx::query(
        "INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, 8, 'coin', 'active')",
    )
    .bind(symbol)
    .bind(format!("{symbol} asset"))
    .execute(pool)
    .await?;
    Ok(result.last_insert_id())
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

#[test]
fn kline_query_validates_interval_and_clamps_limit() {
    let query = KlineQuery::new("1m", None, None, Some(500)).unwrap();

    assert_eq!(query.interval, "1m");
    assert_eq!(query.limit, 100);
    assert!(KlineQuery::new("2m", None, None, Some(10)).is_err());
}

#[tokio::test]
async fn market_list_route_returns_active_pairs_from_mysql() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let base_symbol = unique_symbol("MLB");
    let quote_symbol = unique_symbol("MLQ");
    let disabled_base_symbol = unique_symbol("MLD");
    let base_asset_id = create_market_asset(&pool, &base_symbol).await?;
    let quote_asset_id = create_market_asset(&pool, &quote_symbol).await?;
    let disabled_base_asset_id = create_market_asset(&pool, &disabled_base_symbol).await?;
    let active_pair_symbol = format!("{base_symbol}-{quote_symbol}");
    let disabled_pair_symbol = format!("{disabled_base_symbol}-{quote_symbol}");
    let active_pair_id = sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, 8, 8, 1, 'active', 'external')"#,
    )
    .bind(base_asset_id)
    .bind(quote_asset_id)
    .bind(&active_pair_symbol)
    .execute(&pool)
    .await?
    .last_insert_id();
    let disabled_pair_id = sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, 8, 8, 1, 'disabled', 'external')"#,
    )
    .bind(disabled_base_asset_id)
    .bind(quote_asset_id)
    .bind(&disabled_pair_symbol)
    .execute(&pool)
    .await?
    .last_insert_id();
    let app = routes::routes().with_state(AppState::new(test_settings()).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/markets")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 1_048_576)
        .await
        .unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(status, StatusCode::OK, "payload: {payload}");
    assert!(payload["markets"].as_array().unwrap().iter().any(|market| {
        market["symbol"] == active_pair_symbol
            && market["base_asset"] == base_symbol
            && market["quote_asset"] == quote_symbol
            && market["status"] == "active"
            && market["market_type"] == "external"
    }));
    assert!(
        !payload["markets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|market| market["symbol"] == disabled_pair_symbol)
    );

    sqlx::query("DELETE FROM trading_pairs WHERE id IN (?, ?)")
        .bind(active_pair_id)
        .bind(disabled_pair_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id IN (?, ?, ?)")
        .bind(base_asset_id)
        .bind(quote_asset_id)
        .bind(disabled_base_asset_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn market_ticker_route_rejects_invalid_symbol_before_redis() {
    let app = routes::routes().with_state(AppState::new(test_settings()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/markets/BTC.USDT/ticker")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn market_ticker_route_returns_clear_error_without_redis() {
    let app = routes::routes().with_state(AppState::new(test_settings()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/markets/BTC-USDT/ticker")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("redis connection is not configured for market ticker routes")
    );
}

#[tokio::test]
async fn market_depth_route_returns_clear_error_without_redis() {
    let app = routes::routes().with_state(AppState::new(test_settings()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/markets/BTC-USDT/depth")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("redis connection is not configured for market depth routes")
    );
}

#[tokio::test]
async fn market_trades_route_returns_clear_error_without_mysql() {
    let app = routes::routes().with_state(AppState::new(test_settings()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/markets/BTC-USDT/trades?limit=5")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for market trade routes")
    );
}

#[tokio::test]
async fn market_ticker_route_reads_latest_cached_ticker() -> Result<(), Box<dyn Error>> {
    let Some(redis_url) = env_or_skip("REDIS_URL") else {
        return Ok(());
    };
    let client = redis::Client::open(redis_url)?;
    let manager = redis::aio::ConnectionManager::new(client).await?;
    let observed_at = Utc.with_ymd_and_hms(2026, 5, 28, 12, 50, 0).unwrap();
    let ticker = MarketTickerCacheEntry::new(
        "BTC-USDT",
        decimal("70001.120000000000000000"),
        decimal("245.500000000000000000"),
        observed_at,
    )?;
    let mut raw_connection = manager.clone();
    let _: usize = raw_connection.del(ticker.redis_key()).await?;
    let payload = serde_json::to_string(&ticker)?;
    let _: () = raw_connection.set(ticker.redis_key(), payload).await?;
    let app = routes::routes().with_state(AppState::new(test_settings()).with_redis(manager));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/markets/BTC-USDT/ticker")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(status, StatusCode::OK, "payload: {payload}");
    assert_eq!(payload["symbol"], "BTCUSDT");
    assert_eq!(payload["last_price"], "70001.120000000000000000");
    assert_eq!(payload["volume_24h"], "245.500000000000000000");
    assert_eq!(payload["observed_at"], observed_at.timestamp_millis());

    let _: usize = raw_connection.del(ticker.redis_key()).await?;
    Ok(())
}

#[tokio::test]
async fn market_depth_route_reads_latest_cached_depth() -> Result<(), Box<dyn Error>> {
    let Some(redis_url) = env_or_skip("REDIS_URL") else {
        return Ok(());
    };
    let client = redis::Client::open(redis_url)?;
    let manager = redis::aio::ConnectionManager::new(client).await?;
    let observed_at = Utc.with_ymd_and_hms(2026, 5, 28, 12, 55, 0).unwrap();
    let depth = MarketDepthCacheEntry::new(
        "BTC-USDT",
        vec![MarketDepthLevel::new(
            decimal("70000.000000000000000000"),
            decimal("1.250000000000000000"),
        )],
        vec![MarketDepthLevel::new(
            decimal("70002.000000000000000000"),
            decimal("0.750000000000000000"),
        )],
        observed_at,
    )?;
    let mut raw_connection = manager.clone();
    let _: usize = raw_connection.del(depth.redis_key()).await?;
    let payload = serde_json::to_string(&depth)?;
    let _: () = raw_connection.set(depth.redis_key(), payload).await?;
    let app = routes::routes().with_state(AppState::new(test_settings()).with_redis(manager));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/markets/BTC-USDT/depth")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(status, StatusCode::OK, "payload: {payload}");
    assert_eq!(payload["symbol"], "BTCUSDT");
    assert_eq!(payload["bids"][0]["price"], "70000.000000000000000000");
    assert_eq!(payload["bids"][0]["amount"], "1.250000000000000000");
    assert_eq!(payload["asks"][0]["price"], "70002.000000000000000000");
    assert_eq!(payload["asks"][0]["amount"], "0.750000000000000000");
    assert_eq!(payload["observed_at"], observed_at.timestamp_millis());

    let _: usize = raw_connection.del(depth.redis_key()).await?;
    Ok(())
}

#[tokio::test]
async fn market_trades_route_reads_recent_spot_trades_from_mysql() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let base_symbol = unique_symbol("MTB");
    let quote_symbol = unique_symbol("MTQ");
    let base_asset_id = create_market_asset(&pool, &base_symbol).await?;
    let quote_asset_id = create_market_asset(&pool, &quote_symbol).await?;
    let pair_symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, 8, 8, 1, 'active', 'external')"#,
    )
    .bind(base_asset_id)
    .bind(quote_asset_id)
    .bind(&pair_symbol)
    .execute(&pool)
    .await?
    .last_insert_id();
    let user_id = sqlx::query(
        "INSERT INTO users (email, password_hash, status) VALUES (?, 'hash', 'active')",
    )
    .bind(format!("{}@example.test", pair_symbol.to_ascii_lowercase()))
    .execute(&pool)
    .await?
    .last_insert_id();
    let buy_order_id = sqlx::query(
        r#"INSERT INTO spot_orders
           (user_id, pair_id, side, order_type, price, quantity, filled_quantity, status)
           VALUES (?, ?, 'buy', 'limit', 1, 1, 1, 'filled')"#,
    )
    .bind(user_id)
    .bind(pair_id)
    .execute(&pool)
    .await?
    .last_insert_id();
    let sell_order_id = sqlx::query(
        r#"INSERT INTO spot_orders
           (user_id, pair_id, side, order_type, price, quantity, filled_quantity, status)
           VALUES (?, ?, 'sell', 'limit', 1, 1, 1, 'filled')"#,
    )
    .bind(user_id)
    .bind(pair_id)
    .execute(&pool)
    .await?
    .last_insert_id();
    let trade_id = sqlx::query(
        r#"INSERT INTO spot_trades (pair_id, buy_order_id, sell_order_id, price, quantity, fee)
           VALUES (?, ?, ?, 3.25, 4.5, 0)"#,
    )
    .bind(pair_id)
    .bind(buy_order_id)
    .bind(sell_order_id)
    .execute(&pool)
    .await?
    .last_insert_id();
    let app = routes::routes().with_state(AppState::new(test_settings()).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/markets/{pair_symbol}/trades?limit=5"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(status, StatusCode::OK, "payload: {payload}");
    assert_eq!(payload["trades"][0]["id"], trade_id.to_string());
    assert_eq!(payload["trades"][0]["symbol"], pair_symbol.replace('-', ""));
    assert_eq!(payload["trades"][0]["price"], "3.250000000000000000");
    assert_eq!(payload["trades"][0]["amount"], "4.500000000000000000");
    assert_eq!(payload["trades"][0]["direction"], "BUY");
    assert!(payload["trades"][0]["time"].as_i64().unwrap() > 0);

    sqlx::query("DELETE FROM spot_trades WHERE id = ?")
        .bind(trade_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM spot_orders WHERE id IN (?, ?)")
        .bind(buy_order_id)
        .bind(sell_order_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM trading_pairs WHERE id = ?")
        .bind(pair_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id IN (?, ?)")
        .bind(base_asset_id)
        .bind(quote_asset_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn market_klines_route_rejects_invalid_symbol_before_mongo() {
    let app = routes::routes().with_state(AppState::new(test_settings()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/markets/BTC.USDT/klines?interval=1m")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn market_klines_route_rejects_unlisted_symbol_before_mongo() {
    let app = routes::routes().with_state(AppState::new(test_settings()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/markets/FAKE-USDT/klines?interval=1m")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn market_klines_route_returns_clear_error_without_mongo() {
    let app = routes::routes().with_state(AppState::new(test_settings()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/markets/BTC-USDT/klines?interval=1m")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mongo database is not configured")
    );
}
