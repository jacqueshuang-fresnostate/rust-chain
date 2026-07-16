use bigdecimal::BigDecimal;
use chrono::{TimeZone, Utc};
use exchange_api::{
    config::Settings,
    modules::events::{EventBroadcastHub, WebSocketChannel},
    state::AppState,
    workers::unlock_scanner::{UnlockScannerWorker, release_due_unlock_positions},
};
use secrecy::SecretString;
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::{error::Error, str::FromStr, time::Duration};
use tokio::{sync::Mutex, time::timeout};
use uuid::Uuid;

static TEST_LOCK: Mutex<()> = Mutex::const_new(());

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

fn scanner_now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2000, 1, 2, 0, 0, 0).unwrap()
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
            eprintln!("skipping MySQL unlock scanner test because DATABASE_URL is not set");
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
    let email = format!("unlock-scanner-{}@example.test", Uuid::now_v7().simple());
    sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(email)
        .bind("not-a-real-hash")
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id()
}

async fn create_asset(pool: &MySqlPool) -> u64 {
    let suffix = Uuid::now_v7().simple().to_string();
    let symbol = format!("US{}", &suffix[suffix.len() - 12..]);
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

async fn seed_due_unlock(
    pool: &MySqlPool,
    user_id: u64,
    asset_id: u64,
    fee_paid_status: &str,
) -> Result<(u64, String), sqlx::Error> {
    seed_unlock_record(
        pool,
        user_id,
        asset_id,
        decimal("10.000000000000000000"),
        true,
        fee_paid_status,
        "pending",
        user_id,
        asset_id,
        "active",
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn seed_unlock_record(
    pool: &MySqlPool,
    wallet_user_id: u64,
    wallet_asset_id: u64,
    unlock_quantity: BigDecimal,
    fee_enabled: bool,
    fee_paid_status: &str,
    unlock_status: &str,
    record_user_id: u64,
    record_asset_id: u64,
    position_status: &str,
) -> Result<(u64, String), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO wallet_accounts (user_id, asset_id, locked)
           VALUES (?, ?, ?)
           ON DUPLICATE KEY UPDATE locked = locked + VALUES(locked)"#,
    )
    .bind(wallet_user_id)
    .bind(wallet_asset_id)
    .bind(&unlock_quantity)
    .execute(pool)
    .await?;

    let merge_key = format!("unlock-scanner-lock-{}", Uuid::now_v7().simple());
    let lock_position_id = sqlx::query(
        r#"INSERT INTO asset_lock_positions
           (user_id, asset_id, unlock_type, unlock_at, locked_amount, released_amount,
            remaining_amount, merge_key, status)
           VALUES (?, ?, 'fixed_time', '2000-01-01 00:00:00.000000', ?, 0, ?, ?, ?)"#,
    )
    .bind(wallet_user_id)
    .bind(wallet_asset_id)
    .bind(&unlock_quantity)
    .bind(&unlock_quantity)
    .bind(merge_key)
    .bind(position_status)
    .execute(pool)
    .await?
    .last_insert_id();

    let unlock_key = format!("unlock-scanner-{}", Uuid::now_v7().simple());
    sqlx::query(
        r#"INSERT INTO asset_unlock_records
           (user_id, asset_id, lock_position_id, unlock_quantity, unlock_price,
            unlock_fee_enabled, unlock_fee_rate, unlock_fee_basis, unlock_fee_asset,
            unlock_fee_amount, fee_paid_status, status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, ?, 'market_value', ?, ?, ?, ?, ?)"#,
    )
    .bind(record_user_id)
    .bind(record_asset_id)
    .bind(lock_position_id)
    .bind(&unlock_quantity)
    .bind(decimal("5.000000000000000000"))
    .bind(fee_enabled)
    .bind(decimal("0.04000000"))
    .bind(record_asset_id)
    .bind(decimal("2.000000000000000000"))
    .bind(fee_paid_status)
    .bind(unlock_status)
    .bind(&unlock_key)
    .execute(pool)
    .await?;

    Ok((lock_position_id, unlock_key))
}

async fn cleanup_fixture(
    pool: &MySqlPool,
    user_id: u64,
    asset_id: u64,
    lock_position_id: u64,
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
    sqlx::query("DELETE FROM asset_lock_positions WHERE id = ?")
        .bind(lock_position_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
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

#[tokio::test]
async fn unlock_scanner_releases_due_paid_unlock_and_is_idempotent() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let _guard = TEST_LOCK.lock().await;
    let user_id = create_user(&pool).await;
    let asset_id = create_asset(&pool).await;
    let (lock_position_id, unlock_key) = seed_due_unlock(&pool, user_id, asset_id, "paid").await?;
    let hub = EventBroadcastHub::new(16);
    let _keepalive_hub = hub.clone();
    let mut private_events = hub.subscribe(&WebSocketChannel::private_user(user_id));
    let worker = UnlockScannerWorker;
    let state = AppState::new(test_settings())
        .with_mysql(pool.clone())
        .with_event_broadcast_hub(hub);

    let first = worker.run_once(&state, scanner_now(), 100).await?;

    assert!(first.released >= 1);
    assert_eq!(first.skipped, 0);
    let event_message = timeout(Duration::from_millis(100), private_events.recv()).await??;
    let event: serde_json::Value = serde_json::from_str(event_message.payload())?;
    assert_eq!(event["type"], "new_coin.unlock.released");
    assert_eq!(event["unlock_id"], unlock_key);
    assert_eq!(event["unlock_idempotency_key"], unlock_key);
    assert_eq!(event["lock_position_id"], lock_position_id);
    assert_eq!(event["asset_id"], asset_id);
    assert_eq!(event["released_amount"], "10.000000000000000000");
    assert_eq!(event["unlock_quantity"], "10.000000000000000000");
    assert_eq!(event["released"], true);
    assert_eq!(event["status"], "released");

    let (available, locked): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, locked FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(available, decimal("10.000000000000000000"));
    assert_eq!(locked, decimal("0.000000000000000000"));

    let (remaining, lock_status): (BigDecimal, String) =
        sqlx::query_as("SELECT remaining_amount, status FROM asset_lock_positions WHERE id = ?")
            .bind(lock_position_id)
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

    let second = worker.run_once(&state, scanner_now(), 100).await?;
    assert_eq!(second.released, 0);
    assert!(
        timeout(Duration::from_millis(25), private_events.recv())
            .await
            .is_err(),
        "idempotent unlock scanner replay must not publish duplicate private event"
    );

    let (ledger_count_after_second,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'new_coin_unlock' AND ref_id = ?",
    )
    .bind(&unlock_key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count_after_second, 2);

    cleanup_fixture(&pool, user_id, asset_id, lock_position_id, &unlock_key).await?;
    Ok(())
}

#[tokio::test]
async fn unlock_scanner_blocks_due_unlock_until_required_fee_is_paid() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let _guard = TEST_LOCK.lock().await;
    let user_id = create_user(&pool).await;
    let asset_id = create_asset(&pool).await;
    let (lock_position_id, unlock_key) =
        seed_due_unlock(&pool, user_id, asset_id, "pending").await?;

    let summary = release_due_unlock_positions(&pool, scanner_now(), 100).await?;

    assert!(summary.blocked_fee >= 1);
    assert_eq!(summary.skipped, 0);

    let (available, locked): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, locked FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(available, decimal("0.000000000000000000"));
    assert_eq!(locked, decimal("10.000000000000000000"));

    let (remaining, lock_status): (BigDecimal, String) =
        sqlx::query_as("SELECT remaining_amount, status FROM asset_lock_positions WHERE id = ?")
            .bind(lock_position_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(remaining, decimal("10.000000000000000000"));
    assert_eq!(lock_status, "active");

    let (unlock_status,): (String,) =
        sqlx::query_as("SELECT status FROM asset_unlock_records WHERE idempotency_key = ?")
            .bind(&unlock_key)
            .fetch_one(&pool)
            .await?;
    assert_eq!(unlock_status, "pending");

    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'new_coin_unlock' AND ref_id = ?",
    )
    .bind(&unlock_key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 0);

    cleanup_fixture(&pool, user_id, asset_id, lock_position_id, &unlock_key).await?;
    Ok(())
}

#[tokio::test]
async fn unlock_scanner_releases_due_not_required_unlock() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let _guard = TEST_LOCK.lock().await;
    let user_id = create_user(&pool).await;
    let asset_id = create_asset(&pool).await;
    let (lock_position_id, unlock_key) = seed_unlock_record(
        &pool,
        user_id,
        asset_id,
        decimal("10.000000000000000000"),
        false,
        "not_required",
        "pending",
        user_id,
        asset_id,
        "active",
    )
    .await?;

    let summary = release_due_unlock_positions(&pool, scanner_now(), 100).await?;

    assert!(summary.released >= 1);
    let (unlock_status,): (String,) =
        sqlx::query_as("SELECT status FROM asset_unlock_records WHERE idempotency_key = ?")
            .bind(&unlock_key)
            .fetch_one(&pool)
            .await?;
    assert_eq!(unlock_status, "released");
    let (available, locked): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, locked FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(available, decimal("10.000000000000000000"));
    assert_eq!(locked, decimal("0.000000000000000000"));

    cleanup_fixture(&pool, user_id, asset_id, lock_position_id, &unlock_key).await?;
    Ok(())
}

#[tokio::test]
async fn unlock_scanner_does_not_let_fee_blocked_rows_starve_releasable_rows()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let _guard = TEST_LOCK.lock().await;
    let blocked_user_id = create_user(&pool).await;
    let blocked_asset_id = create_asset(&pool).await;
    let (blocked_position_id, blocked_key) =
        seed_due_unlock(&pool, blocked_user_id, blocked_asset_id, "pending").await?;
    let paid_user_id = create_user(&pool).await;
    let paid_asset_id = create_asset(&pool).await;
    let (paid_position_id, paid_key) =
        seed_due_unlock(&pool, paid_user_id, paid_asset_id, "paid").await?;

    let summary = release_due_unlock_positions(&pool, scanner_now(), 1).await?;

    assert_eq!(summary.released, 1);
    assert!(summary.blocked_fee >= 1);
    let (paid_status,): (String,) =
        sqlx::query_as("SELECT status FROM asset_unlock_records WHERE idempotency_key = ?")
            .bind(&paid_key)
            .fetch_one(&pool)
            .await?;
    assert_eq!(paid_status, "released");
    let (blocked_status,): (String,) =
        sqlx::query_as("SELECT status FROM asset_unlock_records WHERE idempotency_key = ?")
            .bind(&blocked_key)
            .fetch_one(&pool)
            .await?;
    assert_eq!(blocked_status, "pending");

    cleanup_fixture(
        &pool,
        blocked_user_id,
        blocked_asset_id,
        blocked_position_id,
        &blocked_key,
    )
    .await?;
    cleanup_fixture(
        &pool,
        paid_user_id,
        paid_asset_id,
        paid_position_id,
        &paid_key,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn unlock_scanner_skips_cancelled_mismatched_and_non_positive_unlock_records()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let _guard = TEST_LOCK.lock().await;
    let cancelled_user_id = create_user(&pool).await;
    let cancelled_asset_id = create_asset(&pool).await;
    let (cancelled_position_id, cancelled_key) = seed_unlock_record(
        &pool,
        cancelled_user_id,
        cancelled_asset_id,
        decimal("10.000000000000000000"),
        false,
        "not_required",
        "cancelled",
        cancelled_user_id,
        cancelled_asset_id,
        "active",
    )
    .await?;

    let mismatch_user_id = create_user(&pool).await;
    let mismatch_asset_id = create_asset(&pool).await;
    let other_asset_id = create_asset(&pool).await;
    let (mismatch_position_id, mismatch_key) = seed_unlock_record(
        &pool,
        mismatch_user_id,
        mismatch_asset_id,
        decimal("10.000000000000000000"),
        false,
        "not_required",
        "pending",
        mismatch_user_id,
        other_asset_id,
        "active",
    )
    .await?;

    let zero_user_id = create_user(&pool).await;
    let zero_asset_id = create_asset(&pool).await;
    let (zero_position_id, zero_key) = seed_unlock_record(
        &pool,
        zero_user_id,
        zero_asset_id,
        decimal("0.000000000000000000"),
        false,
        "not_required",
        "pending",
        zero_user_id,
        zero_asset_id,
        "active",
    )
    .await?;

    let summary = release_due_unlock_positions(&pool, scanner_now(), 100).await?;

    assert_eq!(summary.released, 0);
    for unlock_key in [&cancelled_key, &mismatch_key, &zero_key] {
        let (unlock_status,): (String,) =
            sqlx::query_as("SELECT status FROM asset_unlock_records WHERE idempotency_key = ?")
                .bind(unlock_key)
                .fetch_one(&pool)
                .await?;
        assert_ne!(unlock_status, "released");
        let (ledger_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'new_coin_unlock' AND ref_id = ?",
        )
        .bind(unlock_key)
        .fetch_one(&pool)
        .await?;
        assert_eq!(ledger_count, 0);
    }

    cleanup_fixture(
        &pool,
        cancelled_user_id,
        cancelled_asset_id,
        cancelled_position_id,
        &cancelled_key,
    )
    .await?;
    cleanup_fixture(
        &pool,
        mismatch_user_id,
        mismatch_asset_id,
        mismatch_position_id,
        &mismatch_key,
    )
    .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(other_asset_id)
        .execute(&pool)
        .await?;
    cleanup_fixture(
        &pool,
        zero_user_id,
        zero_asset_id,
        zero_position_id,
        &zero_key,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn unlock_scanner_worker_run_once_uses_pool_and_limit() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let _guard = TEST_LOCK.lock().await;
    let user_id = create_user(&pool).await;
    let asset_id = create_asset(&pool).await;
    let (lock_position_id, unlock_key) = seed_due_unlock(&pool, user_id, asset_id, "paid").await?;

    let worker = UnlockScannerWorker;
    let state = AppState::new(test_settings()).with_mysql(pool.clone());
    let summary = worker.run_once(&state, scanner_now(), 1).await?;

    assert_eq!(summary.released, 1);
    cleanup_fixture(&pool, user_id, asset_id, lock_position_id, &unlock_key).await?;
    Ok(())
}
