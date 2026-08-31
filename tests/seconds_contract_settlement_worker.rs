use bigdecimal::BigDecimal;
use chrono::{DateTime, TimeDelta, TimeZone, Utc};
use exchange_api::workers::seconds_contract_settlement::{
    run_once_with_pool, run_once_with_pool_and_max_wait, seconds_contract_settlement_result,
};
use sqlx::{MySql, MySqlPool, Transaction, mysql::MySqlPoolOptions};
use std::{error::Error, str::FromStr};
use tokio::sync::Mutex;
use uuid::Uuid;

static TEST_LOCK: Mutex<()> = Mutex::const_new(());

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

async fn mysql_pool_or_skip() -> Result<Option<MySqlPool>, Box<dyn Error>> {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping seconds event-price tests because DATABASE_URL is not set");
        return Ok(None);
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    sqlx::migrate!().run(&pool).await?;
    Ok(Some(pool))
}

async fn create_user(tx: &mut Transaction<'_, MySql>) -> Result<u64, Box<dyn Error>> {
    Ok(
        sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, 'test')")
            .bind(format!("event-price-{}@test.invalid", Uuid::now_v7()))
            .execute(&mut **tx)
            .await?
            .last_insert_id(),
    )
}

async fn create_asset(
    tx: &mut Transaction<'_, MySql>,
    prefix: &str,
) -> Result<(u64, String), Box<dyn Error>> {
    let suffix = Uuid::now_v7().simple().to_string();
    let symbol = format!("{}{}", prefix, &suffix[24..]).to_ascii_uppercase();
    let id = sqlx::query(
        "INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, 18, 'coin', 'active')",
    )
    .bind(&symbol)
    .bind(format!("{symbol} asset"))
    .execute(&mut **tx)
    .await?
    .last_insert_id();
    Ok((id, symbol))
}

struct SecondsFixture {
    pair_symbol: String,
    product_id: u64,
    pair_id: u64,
    stake_asset: u64,
}

async fn seed_fixture(pool: &MySqlPool) -> Result<SecondsFixture, Box<dyn Error>> {
    let mut tx = pool.begin().await?;
    let (base_asset, base_symbol) = create_asset(&mut tx, "EB").await?;
    let (quote_asset, quote_symbol) = create_asset(&mut tx, "EQ").await?;
    let pair_symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision,
            min_order_value, status, market_type)
           VALUES (?, ?, ?, 18, 18, 1, 'active', 'external')"#,
    )
    .bind(base_asset)
    .bind(quote_asset)
    .bind(&pair_symbol)
    .execute(&mut *tx)
    .await?
    .last_insert_id();
    let product_id = sqlx::query(
        r#"INSERT INTO seconds_contract_products
           (pair_id, stake_asset, duration_seconds, payout_rate, min_stake, max_stake, status)
           VALUES (?, ?, 60, 0.8, 1, 100, 'active')"#,
    )
    .bind(pair_id)
    .bind(quote_asset)
    .execute(&mut *tx)
    .await?
    .last_insert_id();
    tx.commit().await?;
    Ok(SecondsFixture {
        pair_symbol,
        product_id,
        pair_id,
        stake_asset: quote_asset,
    })
}

async fn seed_order(
    pool: &MySqlPool,
    fixture: &SecondsFixture,
    expires_at: DateTime<Utc>,
    direction: &str,
) -> Result<u64, Box<dyn Error>> {
    let mut tx = pool.begin().await?;
    let user_id = create_user(&mut tx).await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, 40)")
        .bind(user_id)
        .bind(fixture.stake_asset)
        .execute(&mut *tx)
        .await?;
    let order_id = sqlx::query(
        r#"INSERT INTO seconds_contract_orders
           (user_id, product_id, pair_id, stake_asset, direction, stake_amount,
            payout_rate, entry_price, status, idempotency_key, opened_at, expires_at)
           VALUES (?, ?, ?, ?, ?, 10, 0.8, 100, 'opened', ?, DATE_SUB(?, INTERVAL 60 SECOND), ?)"#,
    )
    .bind(user_id)
    .bind(fixture.product_id)
    .bind(fixture.pair_id)
    .bind(fixture.stake_asset)
    .bind(direction)
    .bind(format!("event-price-{}", Uuid::now_v7()))
    .bind(expires_at.naive_utc())
    .bind(expires_at.naive_utc())
    .execute(&mut *tx)
    .await?
    .last_insert_id();
    tx.commit().await?;
    Ok(order_id)
}

async fn insert_tick(
    pool: &MySqlPool,
    symbol: &str,
    price: &str,
    source: &str,
    observed_at: DateTime<Utc>,
    generation: u64,
) -> Result<u64, Box<dyn Error>> {
    let version = Uuid::now_v7().simple().to_string();
    Ok(sqlx::query(
        r#"INSERT INTO market_price_ticks
           (event_key, symbol, price, source, observed_at, generation, source_version)
           VALUES (?, REPLACE(UPPER(?), '-', ''), ?, ?, ?, ?, ?)"#,
    )
    .bind(format!("{version}{version}"))
    .bind(symbol)
    .bind(decimal(price))
    .bind(source)
    .bind(observed_at.naive_utc())
    .bind(generation)
    .bind(version)
    .execute(pool)
    .await?
    .last_insert_id())
}

type SnapshotRow = (
    String,
    Option<BigDecimal>,
    Option<u64>,
    Option<String>,
    Option<DateTime<Utc>>,
    Option<u64>,
    Option<String>,
);

async fn order_snapshot(pool: &MySqlPool, order_id: u64) -> Result<SnapshotRow, sqlx::Error> {
    sqlx::query_as(
        r#"SELECT status, settlement_price, settlement_price_tick_id,
                  settlement_price_source, settlement_price_observed_at,
                  settlement_price_generation, settlement_price_version
           FROM seconds_contract_orders WHERE id = ?"#,
    )
    .bind(order_id)
    .fetch_one(pool)
    .await
}

#[test]
fn settlement_result_uses_direction_and_exact_prices() {
    assert_eq!(
        seconds_contract_settlement_result("up", &decimal("100"), &decimal("101")).unwrap(),
        "win"
    );
    assert_eq!(
        seconds_contract_settlement_result("down", &decimal("100"), &decimal("100")).unwrap(),
        "loss"
    );
}

#[tokio::test]
async fn timely_delayed_and_replay_use_the_same_event_price_snapshot() -> Result<(), Box<dyn Error>>
{
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    sqlx::query("UPDATE seconds_contract_orders SET status = 'settled' WHERE idempotency_key LIKE 'event-price-%' AND status = 'opened'")
        .execute(&pool)
        .await?;
    let fixture = seed_fixture(&pool).await?;
    let expires_at = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
    let timely_order = seed_order(&pool, &fixture, expires_at, "up").await?;
    let delayed_order = seed_order(&pool, &fixture, expires_at, "up").await?;
    insert_tick(
        &pool,
        &fixture.pair_symbol,
        "105",
        "htx",
        expires_at + TimeDelta::milliseconds(100),
        8,
    )
    .await?;
    let expected_tick = insert_tick(
        &pool,
        &fixture.pair_symbol,
        "104",
        "bitget",
        expires_at + TimeDelta::milliseconds(100),
        7,
    )
    .await?;

    let timely = run_once_with_pool(&pool, expires_at + TimeDelta::seconds(5), 1).await?;
    assert_eq!(timely.settled, 1);
    let delayed = run_once_with_pool(&pool, expires_at + TimeDelta::minutes(5), 1).await?;
    assert_eq!(delayed.settled, 1);

    let timely_snapshot = order_snapshot(&pool, timely_order).await?;
    let delayed_snapshot = order_snapshot(&pool, delayed_order).await?;
    assert_eq!(timely_snapshot, delayed_snapshot);
    assert_eq!(timely_snapshot.0, "settled");
    assert_eq!(
        timely_snapshot.1.as_ref().unwrap().normalized(),
        decimal("104")
    );
    assert_eq!(timely_snapshot.2, Some(expected_tick));
    assert_eq!(timely_snapshot.3.as_deref(), Some("bitget"));
    assert_eq!(timely_snapshot.5, Some(7));

    let replay = run_once_with_pool(&pool, expires_at + TimeDelta::minutes(6), 100).await?;
    assert_eq!(replay.settled, 0);
    assert_eq!(order_snapshot(&pool, timely_order).await?, timely_snapshot);
    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'seconds_contract_order' AND ref_id IN (?, ?) AND change_type = 'seconds_contract_settle_win'",
    )
    .bind(timely_order.to_string())
    .bind(delayed_order.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 2);
    Ok(())
}

#[tokio::test]
async fn scan_before_window_close_does_not_postpone_first_eligible_settlement()
-> Result<(), Box<dyn Error>> {
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let fixture = seed_fixture(&pool).await?;
    let expires_at = Utc.with_ymd_and_hms(2020, 1, 1, 1, 0, 0).unwrap();
    let order_id = seed_order(&pool, &fixture, expires_at, "up").await?;
    let tick_id = insert_tick(&pool, &fixture.pair_symbol, "101", "bitget", expires_at, 1).await?;

    let early = run_once_with_pool(&pool, expires_at + TimeDelta::seconds(1), 1).await?;
    assert_eq!(early.scanned, 0);
    assert_eq!(order_snapshot(&pool, order_id).await?.0, "opened");

    let on_time = run_once_with_pool(&pool, expires_at + TimeDelta::seconds(5), 1).await?;
    assert_eq!(on_time.settled, 1);
    assert_eq!(order_snapshot(&pool, order_id).await?.2, Some(tick_id));
    Ok(())
}

#[tokio::test]
async fn missing_history_stays_pending_then_late_replay_uses_event_time_tick()
-> Result<(), Box<dyn Error>> {
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let fixture = seed_fixture(&pool).await?;
    let expires_at = Utc.with_ymd_and_hms(2020, 1, 2, 0, 0, 0).unwrap();
    let order_id = seed_order(&pool, &fixture, expires_at, "up").await?;
    let first_attempt = expires_at + TimeDelta::seconds(5);
    let pending = run_once_with_pool(&pool, first_attempt, 1).await?;
    assert_eq!(pending.skipped, 1);
    assert_eq!(pending.failed, 0);
    let pending_snapshot = order_snapshot(&pool, order_id).await?;
    assert_eq!(pending_snapshot.0, "opened");
    assert!(pending_snapshot.1.is_none());

    let tick_id = insert_tick(
        &pool,
        &fixture.pair_symbol,
        "103",
        "coinbase",
        expires_at + TimeDelta::seconds(1),
        9,
    )
    .await?;
    let delayed = run_once_with_pool(&pool, first_attempt + TimeDelta::seconds(60), 1).await?;
    assert_eq!(delayed.settled, 1);
    let snapshot = order_snapshot(&pool, order_id).await?;
    assert_eq!(snapshot.2, Some(tick_id));
    assert_eq!(snapshot.1.unwrap().normalized(), decimal("103"));
    Ok(())
}

#[tokio::test]
async fn settlement_window_is_left_closed_and_right_open() -> Result<(), Box<dyn Error>> {
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let fixture = seed_fixture(&pool).await?;
    let expires_at = Utc.with_ymd_and_hms(2020, 1, 3, 0, 0, 0).unwrap();
    let order_id = seed_order(&pool, &fixture, expires_at, "up").await?;
    insert_tick(
        &pool,
        &fixture.pair_symbol,
        "999",
        "bitget",
        expires_at - TimeDelta::microseconds(1),
        1,
    )
    .await?;
    insert_tick(
        &pool,
        &fixture.pair_symbol,
        "888",
        "bitget",
        expires_at + TimeDelta::seconds(5),
        1,
    )
    .await?;
    let pending = run_once_with_pool(&pool, expires_at + TimeDelta::seconds(5), 1).await?;
    assert_eq!(pending.skipped, 1);
    assert_eq!(order_snapshot(&pool, order_id).await?.0, "opened");

    let exact_tick =
        insert_tick(&pool, &fixture.pair_symbol, "101", "bitget", expires_at, 2).await?;
    let settled = run_once_with_pool(&pool, expires_at + TimeDelta::seconds(65), 1).await?;
    assert_eq!(settled.settled, 1);
    assert_eq!(order_snapshot(&pool, order_id).await?.2, Some(exact_tick));
    Ok(())
}

#[tokio::test]
async fn missing_snapshot_moves_once_to_manual_review_and_restart_does_not_mutate_wallet()
-> Result<(), Box<dyn Error>> {
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    sqlx::query(
        "UPDATE seconds_contract_orders SET status = 'settled' WHERE idempotency_key LIKE 'event-price-%' AND status = 'opened'",
    )
    .execute(&pool)
    .await?;
    let fixture = seed_fixture(&pool).await?;
    let expires_at = Utc.with_ymd_and_hms(2030, 1, 4, 0, 0, 0).unwrap();
    let order_id = seed_order(&pool, &fixture, expires_at, "up").await?;
    let processing_time = expires_at + TimeDelta::seconds(15);

    let first = run_once_with_pool_and_max_wait(&pool, processing_time, 1, 10).await?;
    assert_eq!(first.scanned, 1);
    assert_eq!(first.settled, 0);
    assert_eq!(first.manual_review, 1);
    assert_eq!(first.skipped, 0);
    assert_eq!(first.failed, 0);

    type FailureRow = (
        String,
        Option<String>,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        Option<BigDecimal>,
    );
    let failure: FailureRow = sqlx::query_as(
        r#"SELECT status, settlement_failure_code, settlement_failed_at,
                  settlement_window_start, settlement_window_end, settlement_price
           FROM seconds_contract_orders
           WHERE id = ?"#,
    )
    .bind(order_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(failure.0, "manual_review");
    assert_eq!(failure.1.as_deref(), Some("missing_settlement_snapshot"));
    assert_eq!(failure.2, Some(processing_time));
    assert_eq!(failure.3, Some(expires_at));
    assert_eq!(failure.4, Some(expires_at + TimeDelta::seconds(5)));
    assert!(failure.5.is_none());

    let exception: (i64, String, DateTime<Utc>, DateTime<Utc>, DateTime<Utc>) = sqlx::query_as(
        r#"SELECT COUNT(*), MAX(failure_code), MAX(detected_at),
                      MAX(window_start), MAX(window_end)
               FROM seconds_contract_settlement_exceptions
               WHERE order_id = ?"#,
    )
    .bind(order_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(exception.0, 1);
    assert_eq!(exception.1, "missing_settlement_snapshot");
    assert_eq!(exception.2, processing_time);
    assert_eq!(exception.3, expires_at);
    assert_eq!(exception.4, expires_at + TimeDelta::seconds(5));

    let wallet_before_restart: BigDecimal = sqlx::query_scalar(
        r#"SELECT wallets.available
           FROM seconds_contract_orders orders
           INNER JOIN wallet_accounts wallets
                   ON wallets.user_id = orders.user_id
                  AND wallets.asset_id = orders.stake_asset
           WHERE orders.id = ?"#,
    )
    .bind(order_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(wallet_before_restart.normalized(), decimal("40"));
    let ledger_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'seconds_contract_order' AND ref_id = ?",
    )
    .bind(order_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 0);

    let replay =
        run_once_with_pool_and_max_wait(&pool, processing_time + TimeDelta::minutes(1), 100, 10)
            .await?;
    assert_eq!(replay.scanned, 0);
    assert_eq!(replay.manual_review, 0);
    let exception_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM seconds_contract_settlement_exceptions WHERE order_id = ?",
    )
    .bind(order_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(exception_count, 1);
    let wallet_after_restart: BigDecimal = sqlx::query_scalar(
        r#"SELECT wallets.available
           FROM seconds_contract_orders orders
           INNER JOIN wallet_accounts wallets
                   ON wallets.user_id = orders.user_id
                  AND wallets.asset_id = orders.stake_asset
           WHERE orders.id = ?"#,
    )
    .bind(order_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(wallet_after_restart, wallet_before_restart);
    Ok(())
}
