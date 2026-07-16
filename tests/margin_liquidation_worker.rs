use bigdecimal::BigDecimal;
use chrono::{TimeZone, Utc};
use exchange_api::{
    config::Settings,
    modules::{
        events::{EventBroadcastHub, WebSocketChannel},
        market::market_ticker_redis_key,
    },
    state::AppState,
    workers::{
        margin_interest::run_once_with_dependencies as run_margin_interest_once,
        margin_liquidation::{margin_liquidation_risk_state, run_once, run_once_with_dependencies},
    },
};
use redis::AsyncCommands;
use secrecy::SecretString;
use serde_json::Value;
use sqlx::{MySql, MySqlPool, Transaction, mysql::MySqlPoolOptions, types::Json as SqlxJson};
use std::{collections::HashSet, error::Error, str::FromStr, time::Duration};
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
            eprintln!("skipping margin liquidation worker test because {name} is not set");
            None
        }
    }
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

fn unique_symbol(prefix: &str) -> String {
    let uuid = Uuid::now_v7().simple().to_string();
    format!("{}{}", prefix, &uuid[22..32]).to_ascii_uppercase()
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

async fn close_previous_margin_worker_positions(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE margin_positions SET status = 'liquidated' WHERE idempotency_key LIKE 'margin-worker-%' AND status = 'opened'",
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn create_user(tx: &mut Transaction<'_, MySql>) -> Result<u64, sqlx::Error> {
    let email = format!("margin-worker-{}@example.test", Uuid::now_v7().simple());
    Ok(
        sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
            .bind(email)
            .bind("not-a-real-hash")
            .execute(&mut **tx)
            .await?
            .last_insert_id(),
    )
}

async fn create_asset(tx: &mut Transaction<'_, MySql>, symbol: &str) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, 18, 'coin', 'active')",
    )
    .bind(symbol)
    .bind(format!("{symbol} asset"))
    .execute(&mut **tx)
    .await?
    .last_insert_id())
}

#[derive(Debug)]
struct MarginFixture {
    user_id: u64,
    product_id: u64,
    pair_id: u64,
    margin_asset: u64,
    position_id: u64,
    pair_symbol: String,
}

#[derive(Debug, sqlx::FromRow)]
struct LiquidationRecordRow {
    user_id: u64,
    product_id: u64,
    pair_id: u64,
    margin_asset: u64,
    direction: String,
    margin_amount: BigDecimal,
    notional_amount: BigDecimal,
    interest_amount: BigDecimal,
    entry_price: BigDecimal,
    mark_price: BigDecimal,
    maintenance_margin_rate: BigDecimal,
    equity: BigDecimal,
    maintenance_margin: BigDecimal,
    realized_pnl: BigDecimal,
    payout_amount: BigDecimal,
    reason: String,
    liquidated_at: chrono::DateTime<Utc>,
}

async fn seed_margin_position(
    pool: &MySqlPool,
    direction: &str,
    entry_price: Option<&BigDecimal>,
) -> Result<MarginFixture, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let user_id = create_user(&mut tx).await?;
    let base_symbol = unique_symbol("MWB");
    let quote_symbol = unique_symbol("MWQ");
    let base_asset = create_asset(&mut tx, &base_symbol).await?;
    let margin_asset = create_asset(&mut tx, &quote_symbol).await?;
    let pair_symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, 18, 18, ?, 'active', 'external')"#,
    )
    .bind(base_asset)
    .bind(margin_asset)
    .bind(&pair_symbol)
    .bind(decimal("1.000000000000000000"))
    .execute(&mut *tx)
    .await?
    .last_insert_id();
    let product_id = sqlx::query(
        r#"INSERT INTO margin_products
           (pair_id, margin_asset, margin_mode, margin_modes, leverage_levels, max_leverage, min_margin, max_margin, maintenance_margin_rate, status)
           VALUES (?, ?, 'isolated', JSON_ARRAY('isolated'), ?, ?, ?, ?, ?, 'active')"#,
    )
    .bind(pair_id)
    .bind(margin_asset)
    .bind(SqlxJson(vec!["2".to_owned(), "5".to_owned()]))
    .bind(decimal("5.00000000"))
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("1000.000000000000000000"))
    .bind(decimal("0.05000000"))
    .execute(&mut *tx)
    .await?
    .last_insert_id();
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(margin_asset)
        .bind(decimal("80.000000000000000000"))
        .execute(&mut *tx)
        .await?;
    let position_id = sqlx::query(
        r#"INSERT INTO margin_positions
           (user_id, product_id, pair_id, margin_asset, margin_mode, direction, margin_amount,
            leverage, notional_amount, entry_price, status, idempotency_key)
           VALUES (?, ?, ?, ?, 'isolated', ?, ?, ?, ?, ?, 'opened', ?)"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(margin_asset)
    .bind(direction)
    .bind(decimal("20.000000000000000000"))
    .bind(decimal("5.00000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(entry_price)
    .bind(format!("margin-worker-{}", Uuid::now_v7().simple()))
    .execute(&mut *tx)
    .await?
    .last_insert_id();
    tx.commit().await?;
    Ok(MarginFixture {
        user_id,
        product_id,
        pair_id,
        margin_asset,
        position_id,
        pair_symbol,
    })
}

async fn cache_ticker(
    redis: &redis::aio::ConnectionManager,
    symbol: &str,
    price: &str,
    now: chrono::DateTime<Utc>,
) -> Result<(), Box<dyn Error>> {
    let mut connection = redis.clone();
    let payload = serde_json::json!({
        "symbol": symbol.replace('-', ""),
        "last_price": price,
        "volume_24h": "1.000000000000000000",
        "observed_at": now.timestamp_millis(),
    })
    .to_string();
    let _: () = connection
        .set(market_ticker_redis_key(symbol), payload)
        .await?;
    Ok(())
}

async fn close_other_open_positions(pool: &MySqlPool, keep_ids: &[u64]) -> Result<(), sqlx::Error> {
    let keep_ids = keep_ids.iter().copied().collect::<HashSet<_>>();
    let rows: Vec<(u64,)> =
        sqlx::query_as("SELECT id FROM margin_positions WHERE status = 'opened'")
            .fetch_all(pool)
            .await?;
    for (position_id,) in rows {
        if !keep_ids.contains(&position_id) {
            sqlx::query("UPDATE margin_positions SET status = 'liquidated' WHERE id = ?")
                .bind(position_id)
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

#[tokio::test]
async fn margin_interest_worker_accrues_elapsed_full_hours_idempotently()
-> Result<(), Box<dyn Error>> {
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    close_previous_margin_worker_positions(&pool).await?;
    let opened_at = Utc.with_ymd_and_hms(1991, 2, 1, 0, 0, 0).unwrap();
    let first_accrual_at = Utc.with_ymd_and_hms(1991, 2, 1, 3, 30, 0).unwrap();
    let second_accrual_at = Utc.with_ymd_and_hms(1991, 2, 1, 5, 45, 0).unwrap();
    let fixture =
        seed_margin_position(&pool, "long", Some(&decimal("100.000000000000000000"))).await?;
    sqlx::query("UPDATE margin_products SET hourly_interest_rate = ? WHERE id = ?")
        .bind(decimal("0.00100000"))
        .bind(fixture.product_id)
        .execute(&pool)
        .await?;
    sqlx::query(
        r#"UPDATE margin_positions
           SET opened_at = ?, borrowed_amount = ?, interest_amount = ?, interest_accrued_at = ?
           WHERE id = ?"#,
    )
    .bind(opened_at.naive_utc())
    .bind(decimal("80.000000000000000000"))
    .bind(decimal("0.000000000000000000"))
    .bind(opened_at.naive_utc())
    .bind(fixture.position_id)
    .execute(&pool)
    .await?;
    close_other_open_positions(&pool, &[fixture.position_id]).await?;

    let first = run_margin_interest_once(&pool, first_accrual_at, 10).await?;

    assert_eq!(first.scanned, 1);
    assert_eq!(first.accrued, 1);
    assert_eq!(first.skipped, 0);
    assert_eq!(first.failed, 0);
    let (interest_amount, interest_accrued_at): (BigDecimal, Option<chrono::DateTime<Utc>>) =
        sqlx::query_as(
            "SELECT interest_amount, interest_accrued_at FROM margin_positions WHERE id = ?",
        )
        .bind(fixture.position_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(interest_amount, decimal("0.240000000000000000"));
    assert_eq!(interest_accrued_at, Some(first_accrual_at));

    let replay = run_margin_interest_once(&pool, first_accrual_at, 10).await?;

    assert_eq!(replay.scanned, 1);
    assert_eq!(replay.accrued, 0);
    assert_eq!(replay.skipped, 1);
    let (interest_after_replay,): (BigDecimal,) =
        sqlx::query_as("SELECT interest_amount FROM margin_positions WHERE id = ?")
            .bind(fixture.position_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(interest_after_replay, decimal("0.240000000000000000"));

    let second = run_margin_interest_once(&pool, second_accrual_at, 10).await?;

    assert_eq!(second.scanned, 1);
    assert_eq!(second.accrued, 1);
    let (interest_after_second, interest_accrued_after_second): (
        BigDecimal,
        Option<chrono::DateTime<Utc>>,
    ) = sqlx::query_as(
        "SELECT interest_amount, interest_accrued_at FROM margin_positions WHERE id = ?",
    )
    .bind(fixture.position_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(interest_after_second, decimal("0.400000000000000000"));
    assert_eq!(interest_accrued_after_second, Some(second_accrual_at));

    Ok(())
}

#[test]
fn margin_liquidation_risk_state_uses_direction_and_maintenance_margin() {
    let unsafe_long = margin_liquidation_risk_state(
        "long",
        &decimal("20.000000000000000000"),
        &decimal("100.000000000000000000"),
        &decimal("1.250000000000000000"),
        &decimal("100.000000000000000000"),
        &decimal("84.000000000000000000"),
        &decimal("0.05000000"),
    )
    .unwrap();
    assert!(unsafe_long.should_liquidate);
    assert_eq!(unsafe_long.equity, decimal("2.750000000000000000"));

    let safe_short = margin_liquidation_risk_state(
        "short",
        &decimal("20.000000000000000000"),
        &decimal("100.000000000000000000"),
        &decimal("0.000000000000000000"),
        &decimal("100.000000000000000000"),
        &decimal("95.000000000000000000"),
        &decimal("0.05000000"),
    )
    .unwrap();
    assert!(!safe_short.should_liquidate);
    assert_eq!(safe_short.equity, decimal("25.000000000000000000"));
}

#[tokio::test]
async fn margin_liquidation_worker_liquidates_unsafe_position_idempotently()
-> Result<(), Box<dyn Error>> {
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let Some(redis) = redis_manager_or_skip().await? else {
        return Ok(());
    };
    close_previous_margin_worker_positions(&pool).await?;
    let now = Utc.with_ymd_and_hms(1991, 1, 1, 12, 0, 0).unwrap();
    let fixture =
        seed_margin_position(&pool, "long", Some(&decimal("100.000000000000000000"))).await?;
    sqlx::query("UPDATE margin_positions SET interest_amount = ? WHERE id = ?")
        .bind(decimal("1.250000000000000000"))
        .bind(fixture.position_id)
        .execute(&pool)
        .await?;
    cache_ticker(&redis, &fixture.pair_symbol, "84.000000000000000000", now).await?;
    close_other_open_positions(&pool, &[fixture.position_id]).await?;
    let hub = EventBroadcastHub::new(16);
    let _keepalive_hub = hub.clone();
    let mut private_events = hub.subscribe(&WebSocketChannel::private_user(fixture.user_id));
    let state = AppState::new(test_settings())
        .with_mysql(pool.clone())
        .with_redis(redis.clone())
        .with_event_broadcast_hub(hub);

    let summary = run_once(&state, now, 10).await?;

    assert_eq!(summary.scanned, 1);
    assert_eq!(summary.liquidated, 1);
    assert_eq!(summary.skipped, 0);
    assert_eq!(summary.failed, 0);
    let event_message = timeout(Duration::from_millis(100), private_events.recv()).await??;
    let event: Value = serde_json::from_str(event_message.payload())?;
    assert_eq!(event["type"], "margin.position.liquidated");
    assert_eq!(event["position_id"], fixture.position_id);
    assert_eq!(event["product_id"], fixture.product_id);
    assert_eq!(event["pair_id"], fixture.pair_id);
    assert_eq!(event["margin_asset"], fixture.margin_asset);
    assert_eq!(event["direction"], "long");
    assert_eq!(event["margin_amount"], "20.000000000000000000");
    assert_eq!(event["notional_amount"], "100.000000000000000000");
    assert_eq!(event["entry_price"], "100.000000000000000000");
    assert_eq!(event["mark_price"], "84.000000000000000000");
    assert_eq!(event["realized_pnl"], "-16.000000000000000000");
    assert_eq!(event["interest_amount"], "1.250000000000000000");
    assert_eq!(event["payout_amount"], "2.750000000000000000");
    assert_eq!(event["reason"], "maintenance_margin");
    assert_eq!(event["liquidated_at"], now.timestamp_millis());
    let (status, exit_price, realized_pnl): (String, Option<BigDecimal>, Option<BigDecimal>) =
        sqlx::query_as(
            "SELECT status, exit_price, realized_pnl FROM margin_positions WHERE id = ?",
        )
        .bind(fixture.position_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(status, "liquidated");
    assert_eq!(exit_price, Some(decimal("84.000000000000000000")));
    assert_eq!(realized_pnl, Some(decimal("-16.000000000000000000")));
    let (available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(fixture.user_id)
            .bind(fixture.margin_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(available, decimal("82.750000000000000000"));
    let (ledger_amount, ledger_count): (BigDecimal, i64) = sqlx::query_as(
        r#"SELECT COALESCE(SUM(amount), 0), COUNT(*)
           FROM wallet_ledger
           WHERE ref_type = 'margin_position'
             AND ref_id = ?
             AND change_type = 'margin_position_liquidate'"#,
    )
    .bind(fixture.position_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_amount, decimal("2.750000000000000000"));
    assert_eq!(ledger_count, 1);

    let record: LiquidationRecordRow = sqlx::query_as(
        r#"SELECT user_id, product_id, pair_id, margin_asset, direction, margin_amount,
                  notional_amount, interest_amount, entry_price, mark_price, maintenance_margin_rate,
                  equity, maintenance_margin, realized_pnl, payout_amount, reason, liquidated_at
           FROM margin_liquidation_records
           WHERE position_id = ?"#,
    )
    .bind(fixture.position_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(record.user_id, fixture.user_id);
    assert_eq!(record.product_id, fixture.product_id);
    assert_eq!(record.pair_id, fixture.pair_id);
    assert_eq!(record.margin_asset, fixture.margin_asset);
    assert_eq!(record.direction, "long");
    assert_eq!(record.margin_amount, decimal("20.000000000000000000"));
    assert_eq!(record.notional_amount, decimal("100.000000000000000000"));
    assert_eq!(record.interest_amount, decimal("1.250000000000000000"));
    assert_eq!(record.entry_price, decimal("100.000000000000000000"));
    assert_eq!(record.mark_price, decimal("84.000000000000000000"));
    assert_eq!(record.maintenance_margin_rate, decimal("0.05000000"));
    assert_eq!(record.equity, decimal("2.750000000000000000"));
    assert_eq!(record.maintenance_margin, decimal("5.000000000000000000"));
    assert_eq!(record.realized_pnl, decimal("-16.000000000000000000"));
    assert_eq!(record.payout_amount, decimal("2.750000000000000000"));
    assert_eq!(record.reason, "maintenance_margin");
    assert_eq!(record.liquidated_at, now);

    let idempotent = run_once(&state, now, 10).await?;

    assert_eq!(idempotent.scanned, 0);
    assert_eq!(idempotent.liquidated, 0);
    assert!(
        timeout(Duration::from_millis(25), private_events.recv())
            .await
            .is_err(),
        "idempotent margin liquidation replay must not publish duplicate private event"
    );
    let (ledger_count_after,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'margin_position' AND ref_id = ? AND change_type = 'margin_position_liquidate'",
    )
    .bind(fixture.position_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count_after, 1);
    let (record_count_after,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM margin_liquidation_records WHERE position_id = ?")
            .bind(fixture.position_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(record_count_after, 1);
    Ok(())
}

#[tokio::test]
async fn margin_liquidation_worker_credits_recorded_margin_wallet_scope()
-> Result<(), Box<dyn Error>> {
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let Some(redis) = redis_manager_or_skip().await? else {
        return Ok(());
    };
    close_previous_margin_worker_positions(&pool).await?;
    let now = Utc.with_ymd_and_hms(1991, 1, 7, 12, 0, 0).unwrap();
    let fixture =
        seed_margin_position(&pool, "long", Some(&decimal("100.000000000000000000"))).await?;
    sqlx::query(
        "INSERT INTO margin_wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)",
    )
    .bind(fixture.user_id)
    .bind(fixture.margin_asset)
    .bind(decimal("80.000000000000000000"))
    .execute(&pool)
    .await?;
    sqlx::query(
        "UPDATE margin_positions SET wallet_scope = 'margin', interest_amount = ? WHERE id = ?",
    )
    .bind(decimal("1.250000000000000000"))
    .bind(fixture.position_id)
    .execute(&pool)
    .await?;
    cache_ticker(&redis, &fixture.pair_symbol, "84.000000000000000000", now).await?;
    close_other_open_positions(&pool, &[fixture.position_id]).await?;

    let summary = run_once_with_dependencies(&pool, &redis, now, 10).await?;
    assert_eq!(summary.liquidated, 1);
    assert_eq!(summary.failed, 0);

    let (spot_available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(fixture.user_id)
            .bind(fixture.margin_asset)
            .fetch_one(&pool)
            .await?;
    let (margin_available,): (BigDecimal,) = sqlx::query_as(
        "SELECT available FROM margin_wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(fixture.user_id)
    .bind(fixture.margin_asset)
    .fetch_one(&pool)
    .await?;
    let (spot_ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'margin_position' AND ref_id = ? AND change_type = 'margin_position_liquidate'",
    )
    .bind(fixture.position_id.to_string())
    .fetch_one(&pool)
    .await?;
    let (margin_ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM margin_wallet_ledger WHERE ref_type = 'margin_position' AND ref_id = ? AND change_type = 'margin_position_liquidate'",
    )
    .bind(fixture.position_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(spot_available, decimal("80.000000000000000000"));
    assert_eq!(margin_available, decimal("82.750000000000000000"));
    assert_eq!(spot_ledger_count, 0);
    assert_eq!(margin_ledger_count, 1);
    Ok(())
}

#[tokio::test]
async fn margin_liquidation_worker_skips_safe_position() -> Result<(), Box<dyn Error>> {
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let Some(redis) = redis_manager_or_skip().await? else {
        return Ok(());
    };
    close_previous_margin_worker_positions(&pool).await?;
    let now = Utc.with_ymd_and_hms(1991, 1, 2, 12, 0, 0).unwrap();
    let fixture =
        seed_margin_position(&pool, "long", Some(&decimal("100.000000000000000000"))).await?;
    cache_ticker(&redis, &fixture.pair_symbol, "99.000000000000000000", now).await?;
    close_other_open_positions(&pool, &[fixture.position_id]).await?;

    let summary = run_once_with_dependencies(&pool, &redis, now, 10).await?;

    assert_eq!(summary.scanned, 1);
    assert_eq!(summary.liquidated, 0);
    assert_eq!(summary.skipped, 1);
    let (status, next_attempt): (String, Option<chrono::DateTime<Utc>>) = sqlx::query_as(
        "SELECT status, next_liquidation_attempt_at FROM margin_positions WHERE id = ?",
    )
    .bind(fixture.position_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(status, "opened");
    assert_eq!(next_attempt, Some(now + chrono::TimeDelta::seconds(5)));
    Ok(())
}

#[tokio::test]
async fn margin_liquidation_worker_rotates_past_safe_positions() -> Result<(), Box<dyn Error>> {
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let Some(redis) = redis_manager_or_skip().await? else {
        return Ok(());
    };
    close_previous_margin_worker_positions(&pool).await?;
    let now = Utc.with_ymd_and_hms(1991, 1, 4, 12, 0, 0).unwrap();
    let mut safe_positions = Vec::new();
    for _ in 0..10 {
        let safe =
            seed_margin_position(&pool, "long", Some(&decimal("100.000000000000000000"))).await?;
        cache_ticker(&redis, &safe.pair_symbol, "99.000000000000000000", now).await?;
        safe_positions.push(safe);
    }
    let unsafe_position =
        seed_margin_position(&pool, "long", Some(&decimal("100.000000000000000000"))).await?;
    cache_ticker(
        &redis,
        &unsafe_position.pair_symbol,
        "84.000000000000000000",
        now,
    )
    .await?;
    let mut keep_ids = safe_positions
        .iter()
        .map(|fixture| fixture.position_id)
        .collect::<Vec<_>>();
    keep_ids.push(unsafe_position.position_id);
    close_other_open_positions(&pool, &keep_ids).await?;

    let first = run_once_with_dependencies(&pool, &redis, now, 1).await?;

    assert_eq!(first.scanned, 10);
    assert_eq!(first.liquidated, 0);
    assert_eq!(first.skipped, 10);

    let second =
        run_once_with_dependencies(&pool, &redis, now + chrono::TimeDelta::seconds(5), 1).await?;

    assert_eq!(second.scanned, 1);
    assert_eq!(second.liquidated, 1);
    let safe_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM margin_positions WHERE id IN (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) AND status = 'opened'",
    )
    .bind(safe_positions[0].position_id)
    .bind(safe_positions[1].position_id)
    .bind(safe_positions[2].position_id)
    .bind(safe_positions[3].position_id)
    .bind(safe_positions[4].position_id)
    .bind(safe_positions[5].position_id)
    .bind(safe_positions[6].position_id)
    .bind(safe_positions[7].position_id)
    .bind(safe_positions[8].position_id)
    .bind(safe_positions[9].position_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(safe_count, 10);
    let (unsafe_status,): (String,) =
        sqlx::query_as("SELECT status FROM margin_positions WHERE id = ?")
            .bind(unsafe_position.position_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(unsafe_status, "liquidated");
    Ok(())
}

#[tokio::test]
async fn margin_liquidation_worker_scans_past_broken_rows() -> Result<(), Box<dyn Error>> {
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let Some(redis) = redis_manager_or_skip().await? else {
        return Ok(());
    };
    close_previous_margin_worker_positions(&pool).await?;
    let now = Utc.with_ymd_and_hms(1991, 1, 3, 12, 0, 0).unwrap();
    let broken = seed_margin_position(&pool, "long", None).await?;
    cache_ticker(&redis, &broken.pair_symbol, "84.000000000000000000", now).await?;
    let healthy =
        seed_margin_position(&pool, "long", Some(&decimal("100.000000000000000000"))).await?;
    cache_ticker(&redis, &healthy.pair_symbol, "84.000000000000000000", now).await?;
    close_other_open_positions(&pool, &[broken.position_id, healthy.position_id]).await?;

    let summary = run_once_with_dependencies(&pool, &redis, now, 1).await?;

    assert_eq!(summary.scanned, 2);
    assert_eq!(summary.liquidated, 1);
    assert_eq!(summary.failed, 1);
    let (broken_status,): (String,) =
        sqlx::query_as("SELECT status FROM margin_positions WHERE id = ?")
            .bind(broken.position_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(broken_status, "opened");
    let (healthy_status,): (String,) =
        sqlx::query_as("SELECT status FROM margin_positions WHERE id = ?")
            .bind(healthy.position_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(healthy_status, "liquidated");
    Ok(())
}
