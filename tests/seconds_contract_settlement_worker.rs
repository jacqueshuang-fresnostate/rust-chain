use bigdecimal::BigDecimal;
use chrono::{TimeZone, Utc};
use exchange_api::{
    config::Settings,
    modules::{
        events::{EventBroadcastHub, WebSocketChannel},
        market::market_ticker_redis_key,
    },
    state::AppState,
    workers::seconds_contract_settlement::{
        SecondsContractSettlementWorker, run_once_with_dependencies,
        seconds_contract_settlement_result,
    },
};
use redis::AsyncCommands;
use secrecy::SecretString;
use sqlx::{MySql, MySqlPool, Transaction, mysql::MySqlPoolOptions};
use std::{error::Error, str::FromStr, time::Duration};
use tokio::{sync::Mutex, time::timeout};
use uuid::Uuid;

static TEST_LOCK: Mutex<()> = Mutex::const_new(());

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

fn env_or_skip(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => {
            eprintln!("skipping seconds contract settlement worker test because {name} is not set");
            None
        }
    }
}

fn unique_symbol(prefix: &str) -> String {
    let uuid = Uuid::now_v7().simple().to_string();
    format!("{}{}", prefix, &uuid[22..32]).to_ascii_uppercase()
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

async fn mysql_pool_or_skip() -> Result<Option<MySqlPool>, Box<dyn Error>> {
    let Some(database_url) = env_or_skip("DATABASE_URL") else {
        return Ok(None);
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(Some(pool))
}

async fn redis_manager_or_skip() -> Result<Option<redis::aio::ConnectionManager>, Box<dyn Error>> {
    let Some(redis_url) = env_or_skip("REDIS_URL") else {
        return Ok(None);
    };
    let client = redis::Client::open(redis_url)?;
    Ok(Some(redis::aio::ConnectionManager::new(client).await?))
}

async fn close_previous_seconds_worker_orders(pool: &MySqlPool) -> Result<(), Box<dyn Error>> {
    sqlx::query(
        "UPDATE seconds_contract_orders SET status = 'settled' WHERE idempotency_key LIKE 'seconds-worker-%' AND status = 'opened'",
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn order_status(pool: &MySqlPool, order_id: u64) -> Result<String, Box<dyn Error>> {
    let (status,): (String,) =
        sqlx::query_as("SELECT status FROM seconds_contract_orders WHERE id = ?")
            .bind(order_id)
            .fetch_one(pool)
            .await?;
    Ok(status)
}

async fn create_user(tx: &mut Transaction<'_, MySql>) -> Result<u64, Box<dyn Error>> {
    let email = format!("seconds-worker-{}@example.test", Uuid::now_v7().simple());
    Ok(
        sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
            .bind(email)
            .bind("not-a-real-hash")
            .execute(&mut **tx)
            .await?
            .last_insert_id(),
    )
}

async fn create_asset(
    tx: &mut Transaction<'_, MySql>,
    symbol: &str,
) -> Result<u64, Box<dyn Error>> {
    Ok(sqlx::query(
        "INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, 18, 'coin', 'active')",
    )
    .bind(symbol)
    .bind(format!("{symbol} asset"))
    .execute(&mut **tx)
    .await?
    .last_insert_id())
}

async fn seed_due_seconds_order(
    pool: &MySqlPool,
    direction: &str,
    entry_price: &BigDecimal,
    now: chrono::DateTime<Utc>,
) -> Result<(u64, u64, u64, u64, String), Box<dyn Error>> {
    seed_due_seconds_order_with_optional_entry(pool, direction, Some(entry_price), now).await
}

async fn seed_due_seconds_order_without_entry_price(
    pool: &MySqlPool,
    direction: &str,
    now: chrono::DateTime<Utc>,
) -> Result<(u64, u64, u64, u64, String), Box<dyn Error>> {
    seed_due_seconds_order_with_optional_entry(pool, direction, None, now).await
}

async fn seed_due_seconds_order_with_missing_wallet(
    pool: &MySqlPool,
    direction: &str,
    entry_price: &BigDecimal,
    now: chrono::DateTime<Utc>,
) -> Result<(u64, u64, u64, u64, String), Box<dyn Error>> {
    let (order_id, user_id, product_id, stake_asset, pair_symbol) =
        seed_due_seconds_order(pool, direction, entry_price, now).await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok((order_id, user_id, product_id, stake_asset, pair_symbol))
}

async fn seed_due_seconds_order_with_optional_entry(
    pool: &MySqlPool,
    direction: &str,
    entry_price: Option<&BigDecimal>,
    now: chrono::DateTime<Utc>,
) -> Result<(u64, u64, u64, u64, String), Box<dyn Error>> {
    let mut tx = pool.begin().await?;
    let user_id = create_user(&mut tx).await?;
    let base_symbol = unique_symbol("SWB");
    let quote_symbol = unique_symbol("SWQ");
    let base_asset = create_asset(&mut tx, &base_symbol).await?;
    let quote_asset = create_asset(&mut tx, &quote_symbol).await?;
    let pair_symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, 18, 18, ?, 'active', 'external')"#,
    )
    .bind(base_asset)
    .bind(quote_asset)
    .bind(&pair_symbol)
    .bind(decimal("1.000000000000000000"))
    .execute(&mut *tx)
    .await?
    .last_insert_id();
    let product_id = sqlx::query(
        r#"INSERT INTO seconds_contract_products
           (pair_id, stake_asset, duration_seconds, payout_rate, min_stake, max_stake, status)
           VALUES (?, ?, 60, ?, ?, ?, 'active')"#,
    )
    .bind(pair_id)
    .bind(quote_asset)
    .bind(decimal("0.80000000"))
    .bind(decimal("5.000000000000000000"))
    .bind(decimal("100.000000000000000000"))
    .execute(&mut *tx)
    .await?
    .last_insert_id();
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("40.000000000000000000"))
        .execute(&mut *tx)
        .await?;
    let order_id = sqlx::query(
        r#"INSERT INTO seconds_contract_orders
           (user_id, product_id, pair_id, stake_asset, direction, stake_amount,
            payout_rate, entry_price, status, idempotency_key, expires_at, next_settlement_attempt_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'opened', ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(quote_asset)
    .bind(direction)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("0.80000000"))
    .bind(entry_price)
    .bind(format!("seconds-worker-{}", Uuid::now_v7().simple()))
    .bind((now - chrono::TimeDelta::seconds(1)).naive_utc())
    .bind(now.naive_utc())
    .execute(&mut *tx)
    .await?
    .last_insert_id();
    tx.commit().await?;
    Ok((order_id, user_id, product_id, quote_asset, pair_symbol))
}

#[test]
fn seconds_contract_settlement_result_uses_direction_and_prices() {
    assert_eq!(
        seconds_contract_settlement_result("up", &decimal("100"), &decimal("101")).unwrap(),
        "win"
    );
    assert_eq!(
        seconds_contract_settlement_result("up", &decimal("100"), &decimal("99")).unwrap(),
        "loss"
    );
    assert_eq!(
        seconds_contract_settlement_result("down", &decimal("100"), &decimal("99")).unwrap(),
        "win"
    );
    assert_eq!(
        seconds_contract_settlement_result("down", &decimal("100"), &decimal("101")).unwrap(),
        "loss"
    );
    assert_eq!(
        seconds_contract_settlement_result("up", &decimal("100"), &decimal("100")).unwrap(),
        "loss"
    );
}

#[tokio::test]
async fn seconds_contract_settlement_worker_settles_due_orders_from_cached_ticker_idempotently()
-> Result<(), Box<dyn Error>> {
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let Some(redis) = redis_manager_or_skip().await? else {
        return Ok(());
    };
    close_previous_seconds_worker_orders(&pool).await?;
    let now = Utc.with_ymd_and_hms(1980, 1, 1, 12, 0, 0).unwrap();
    let (order_id, user_id, product_id, stake_asset, pair_symbol) =
        seed_due_seconds_order(&pool, "up", &decimal("100.000000000000000000"), now).await?;
    let redis_key = market_ticker_redis_key(&pair_symbol);
    let mut redis_connection = redis.clone();
    let ticker_payload = serde_json::json!({
        "symbol": pair_symbol.replace('-', ""),
        "last_price": "105.000000000000000000",
        "volume_24h": "1.000000000000000000",
        "observed_at": now.timestamp_millis(),
    })
    .to_string();
    let _: () = redis_connection.set(&redis_key, ticker_payload).await?;
    let hub = EventBroadcastHub::new(16);
    let _keepalive_hub = hub.clone();
    let mut private_events = hub.subscribe(&WebSocketChannel::private_user(user_id));
    let worker = SecondsContractSettlementWorker;
    let state = AppState::new(test_settings())
        .with_mysql(pool.clone())
        .with_redis(redis.clone())
        .with_event_broadcast_hub(hub);

    let summary = worker.run_once(&state, now, 10).await?;

    assert_eq!(summary.scanned, 1);
    assert_eq!(summary.settled, 1);
    assert_eq!(summary.skipped, 0);
    assert_eq!(summary.failed, 0);
    let event_message = timeout(Duration::from_millis(100), private_events.recv()).await??;
    let event: serde_json::Value = serde_json::from_str(event_message.payload())?;
    assert_eq!(event["type"], "seconds_contract.order.settled");
    assert_eq!(event["order_id"], order_id);
    assert_eq!(event["product_id"], product_id);
    assert_eq!(event["stake_asset"], stake_asset);
    assert_eq!(event["direction"], "up");
    assert_eq!(event["stake_amount"], "10.000000000000000000");
    assert_eq!(event["payout_amount"], "18.00000000000000000000000000");
    assert_eq!(event["result"], "win");
    assert_eq!(event["status"], "settled");
    let (status, result): (String, Option<String>) =
        sqlx::query_as("SELECT status, result FROM seconds_contract_orders WHERE id = ?")
            .bind(order_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(status, "settled");
    assert_eq!(result.as_deref(), Some("win"));
    let (available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(available, decimal("58.000000000000000000"));
    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'seconds_contract_order' AND ref_id = ? AND change_type = 'seconds_contract_settle_win'",
    )
    .bind(order_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 1);

    let idempotent = worker.run_once(&state, now, 10).await?;

    assert_eq!(idempotent.scanned, 0);
    assert_eq!(idempotent.settled, 0);
    assert!(
        timeout(Duration::from_millis(25), private_events.recv())
            .await
            .is_err(),
        "idempotent seconds contract settlement replay must not publish duplicate private event"
    );
    let (ledger_count_after,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'seconds_contract_order' AND ref_id = ? AND change_type = 'seconds_contract_settle_win'",
    )
    .bind(order_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count_after, 1);
    let _: usize = redis_connection.del(&redis_key).await?;
    Ok(())
}

#[tokio::test]
async fn seconds_contract_settlement_worker_scans_past_missing_ticker_rows()
-> Result<(), Box<dyn Error>> {
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let Some(redis) = redis_manager_or_skip().await? else {
        return Ok(());
    };
    close_previous_seconds_worker_orders(&pool).await?;
    let now = Utc.with_ymd_and_hms(1980, 1, 2, 12, 0, 0).unwrap();
    let (missing_order_id, _, _, _, _) =
        seed_due_seconds_order(&pool, "up", &decimal("100.000000000000000000"), now).await?;
    let (settle_order_id, _, _, _, pair_symbol) =
        seed_due_seconds_order(&pool, "up", &decimal("100.000000000000000000"), now).await?;
    let redis_key = market_ticker_redis_key(&pair_symbol);
    let mut redis_connection = redis.clone();
    let ticker_payload = serde_json::json!({
        "symbol": pair_symbol.replace('-', ""),
        "last_price": "105.000000000000000000",
        "volume_24h": "1.000000000000000000",
        "observed_at": now.timestamp_millis(),
    })
    .to_string();
    let _: () = redis_connection.set(&redis_key, ticker_payload).await?;

    let skipped = run_once_with_dependencies(&pool, &redis, now, 1).await?;
    assert_eq!(skipped.scanned, 1);
    assert_eq!(skipped.settled, 0);
    assert_eq!(skipped.skipped, 1);

    let summary = run_once_with_dependencies(&pool, &redis, now, 1).await?;

    assert_eq!(summary.scanned, 1);
    assert_eq!(summary.settled, 1);
    assert_eq!(summary.skipped, 0);
    assert_eq!(summary.failed, 0);
    let (missing_status,): (String,) =
        sqlx::query_as("SELECT status FROM seconds_contract_orders WHERE id = ?")
            .bind(missing_order_id)
            .fetch_one(&pool)
            .await?;
    let (settled_status,): (String,) =
        sqlx::query_as("SELECT status FROM seconds_contract_orders WHERE id = ?")
            .bind(settle_order_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(missing_status, "opened");
    assert_eq!(settled_status, "settled");
    let (rescheduled_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM seconds_contract_orders WHERE id = ? AND next_settlement_attempt_at > ?",
    )
    .bind(missing_order_id)
    .bind(now.naive_utc())
    .fetch_one(&pool)
    .await?;
    assert_eq!(rescheduled_count, 1);
    let _: usize = redis_connection.del(&redis_key).await?;
    Ok(())
}

#[tokio::test]
async fn seconds_contract_settlement_worker_rejects_non_positive_exit_price()
-> Result<(), Box<dyn Error>> {
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let Some(redis) = redis_manager_or_skip().await? else {
        return Ok(());
    };
    close_previous_seconds_worker_orders(&pool).await?;
    let now = Utc.with_ymd_and_hms(1980, 1, 3, 12, 0, 0).unwrap();
    let (order_id, user_id, _, _, pair_symbol) =
        seed_due_seconds_order(&pool, "down", &decimal("100.000000000000000000"), now).await?;
    let redis_key = market_ticker_redis_key(&pair_symbol);
    let mut redis_connection = redis.clone();
    let ticker_payload = serde_json::json!({
        "symbol": pair_symbol.replace('-', ""),
        "last_price": "0.000000000000000000",
        "volume_24h": "1.000000000000000000",
        "observed_at": now.timestamp_millis(),
    })
    .to_string();
    let _: () = redis_connection.set(&redis_key, ticker_payload).await?;

    let summary = run_once_with_dependencies(&pool, &redis, now, 10).await?;

    assert_eq!(summary.scanned, 1);
    assert_eq!(summary.settled, 0);
    assert_eq!(summary.skipped, 0);
    assert_eq!(summary.failed, 1);
    let (status, result): (String, Option<String>) =
        sqlx::query_as("SELECT status, result FROM seconds_contract_orders WHERE id = ?")
            .bind(order_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(status, "opened");
    assert_eq!(result, None);
    let (available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(available, decimal("40.000000000000000000"));
    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'seconds_contract_order' AND ref_id = ? AND change_type = 'seconds_contract_settle_win'",
    )
    .bind(order_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 0);
    let (rescheduled_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM seconds_contract_orders WHERE id = ? AND next_settlement_attempt_at > ?",
    )
    .bind(order_id)
    .bind(now.naive_utc())
    .fetch_one(&pool)
    .await?;
    assert_eq!(rescheduled_count, 1);
    let _: usize = redis_connection.del(&redis_key).await?;
    Ok(())
}

#[tokio::test]
async fn seconds_contract_settlement_worker_reschedules_stale_tickers() -> Result<(), Box<dyn Error>>
{
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let Some(redis) = redis_manager_or_skip().await? else {
        return Ok(());
    };
    close_previous_seconds_worker_orders(&pool).await?;
    let now = Utc.with_ymd_and_hms(1980, 1, 4, 12, 0, 0).unwrap();
    let (order_id, _, _, _, pair_symbol) =
        seed_due_seconds_order(&pool, "up", &decimal("100.000000000000000000"), now).await?;
    let redis_key = market_ticker_redis_key(&pair_symbol);
    let mut redis_connection = redis.clone();
    let ticker_payload = serde_json::json!({
        "symbol": pair_symbol.replace('-', ""),
        "last_price": "105.000000000000000000",
        "volume_24h": "1.000000000000000000",
        "observed_at": (now - chrono::TimeDelta::seconds(61)).timestamp_millis(),
    })
    .to_string();
    let _: () = redis_connection.set(&redis_key, ticker_payload).await?;

    let summary = run_once_with_dependencies(&pool, &redis, now, 10).await?;

    assert_eq!(summary.scanned, 1);
    assert_eq!(summary.settled, 0);
    assert_eq!(summary.failed, 1);
    assert_eq!(order_status(&pool, order_id).await?, "opened");
    let (rescheduled_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM seconds_contract_orders WHERE id = ? AND next_settlement_attempt_at > ?",
    )
    .bind(order_id)
    .bind(now.naive_utc())
    .fetch_one(&pool)
    .await?;
    assert_eq!(rescheduled_count, 1);
    let _: usize = redis_connection.del(&redis_key).await?;
    Ok(())
}

#[tokio::test]
async fn seconds_contract_settlement_worker_reschedules_persistent_settlement_failures()
-> Result<(), Box<dyn Error>> {
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let Some(redis) = redis_manager_or_skip().await? else {
        return Ok(());
    };
    close_previous_seconds_worker_orders(&pool).await?;
    let now = Utc.with_ymd_and_hms(1980, 1, 5, 12, 0, 0).unwrap();
    let (broken_order_id, _, _, _, broken_pair_symbol) =
        seed_due_seconds_order_with_missing_wallet(
            &pool,
            "up",
            &decimal("100.000000000000000000"),
            now,
        )
        .await?;
    let (healthy_order_id, _, _, _, healthy_pair_symbol) =
        seed_due_seconds_order(&pool, "up", &decimal("100.000000000000000000"), now).await?;
    let mut redis_connection = redis.clone();
    for pair_symbol in [&broken_pair_symbol, &healthy_pair_symbol] {
        let payload = serde_json::json!({
            "symbol": pair_symbol.replace('-', ""),
            "last_price": "105.000000000000000000",
            "volume_24h": "1.000000000000000000",
            "observed_at": now.timestamp_millis(),
        })
        .to_string();
        let _: () = redis_connection
            .set(market_ticker_redis_key(pair_symbol), payload)
            .await?;
    }

    let failed = run_once_with_dependencies(&pool, &redis, now, 1).await?;
    assert_eq!(failed.scanned, 1);
    assert_eq!(failed.settled, 0);
    assert_eq!(failed.failed, 1);

    let summary = run_once_with_dependencies(&pool, &redis, now, 1).await?;

    assert_eq!(summary.scanned, 1);
    assert_eq!(summary.settled, 1);
    assert_eq!(summary.failed, 0);
    assert_eq!(order_status(&pool, broken_order_id).await?, "opened");
    assert_eq!(order_status(&pool, healthy_order_id).await?, "settled");
    let (rescheduled_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM seconds_contract_orders WHERE id = ? AND next_settlement_attempt_at > ?",
    )
    .bind(broken_order_id)
    .bind(now.naive_utc())
    .fetch_one(&pool)
    .await?;
    assert_eq!(rescheduled_count, 1);
    let _: usize = redis_connection
        .del(market_ticker_redis_key(&broken_pair_symbol))
        .await?;
    let _: usize = redis_connection
        .del(market_ticker_redis_key(&healthy_pair_symbol))
        .await?;
    Ok(())
}

#[tokio::test]
async fn seconds_contract_settlement_worker_reschedules_missing_entry_price_rows()
-> Result<(), Box<dyn Error>> {
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let Some(redis) = redis_manager_or_skip().await? else {
        return Ok(());
    };
    close_previous_seconds_worker_orders(&pool).await?;
    let now = Utc.with_ymd_and_hms(1980, 1, 6, 12, 0, 0).unwrap();
    let (missing_entry_order_id, _, _, _, _) =
        seed_due_seconds_order_without_entry_price(&pool, "up", now).await?;
    let (healthy_order_id, _, _, _, healthy_pair_symbol) =
        seed_due_seconds_order(&pool, "up", &decimal("100.000000000000000000"), now).await?;
    let mut redis_connection = redis.clone();
    let payload = serde_json::json!({
        "symbol": healthy_pair_symbol.replace('-', ""),
        "last_price": "105.000000000000000000",
        "volume_24h": "1.000000000000000000",
        "observed_at": now.timestamp_millis(),
    })
    .to_string();
    let _: () = redis_connection
        .set(market_ticker_redis_key(&healthy_pair_symbol), payload)
        .await?;

    let failed = run_once_with_dependencies(&pool, &redis, now, 1).await?;
    assert_eq!(failed.scanned, 1);
    assert_eq!(failed.settled, 0);
    assert_eq!(failed.failed, 1);

    let summary = run_once_with_dependencies(&pool, &redis, now, 1).await?;

    assert_eq!(summary.scanned, 1);
    assert_eq!(summary.settled, 1);
    assert_eq!(summary.failed, 0);
    assert_eq!(order_status(&pool, missing_entry_order_id).await?, "opened");
    assert_eq!(order_status(&pool, healthy_order_id).await?, "settled");
    let (rescheduled_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM seconds_contract_orders WHERE id = ? AND next_settlement_attempt_at > ?",
    )
    .bind(missing_entry_order_id)
    .bind(now.naive_utc())
    .fetch_one(&pool)
    .await?;
    assert_eq!(rescheduled_count, 1);
    let _: usize = redis_connection
        .del(market_ticker_redis_key(&healthy_pair_symbol))
        .await?;
    Ok(())
}
