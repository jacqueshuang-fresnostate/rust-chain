use super::*;
use crate::modules::{
    margin::{
        application::close_margin_position_with_events,
        infrastructure::{
            claim_cross_margin_account_for_close, ensure_and_lock_cross_margin_account,
            lock_user_position_by_id,
        },
        presentation::CloseMarginPositionRequest,
    },
    market::market_ticker_redis_key,
};
use chrono::Utc;
use redis::AsyncCommands;
use sqlx::{Connection, MySqlConnection, MySqlPool, mysql::MySqlPoolOptions};
use std::{error::Error, sync::Arc, time::Duration};
use tokio::{sync::Barrier, time::timeout};
use uuid::Uuid;

type TestResult = Result<(), Box<dyn Error + Send + Sync>>;

async fn test_pool() -> Result<Option<MySqlPool>, Box<dyn Error + Send + Sync>> {
    let Ok(url) = std::env::var("MARGIN_CONCURRENCY_DATABASE_URL") else {
        eprintln!(
            "skipping margin close concurrency test: MARGIN_CONCURRENCY_DATABASE_URL is unset"
        );
        return Ok(None);
    };
    let parsed = url::Url::parse(&url)?;
    assert_eq!(parsed.scheme(), "mysql");
    assert!(matches!(
        parsed.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]")
    ));
    assert!(parsed.path().ends_with("_test") && parsed.path().trim_matches('/').len() > 5);
    let pool = MySqlPoolOptions::new()
        .max_connections(3)
        .after_connect(|connection, _| {
            Box::pin(async move {
                sqlx::query("SET time_zone = '+00:00'")
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(&url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(Some(pool))
}

struct Fixture {
    user: u64,
    base: u64,
    asset: u64,
    pair: u64,
    product: u64,
    position: u64,
    symbol: String,
    cross: bool,
}

impl Fixture {
    async fn new(pool: &MySqlPool, cross: bool) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let suffix = Uuid::now_v7().simple().to_string();
        let mut tx = pool.begin().await?;
        let user = sqlx::query("INSERT INTO users(email,password_hash) VALUES (?, 'unused')")
            .bind(format!("close-concurrency-{suffix}@example.test"))
            .execute(&mut *tx)
            .await?
            .last_insert_id();
        let mut assets = Vec::new();
        for prefix in ["MCB", "MCQ"] {
            let symbol = format!("{prefix}{}", &suffix[20..]);
            let id = sqlx::query("INSERT INTO assets(symbol,name,precision_scale,asset_type,status) VALUES (?, ?, 18, 'coin', 'active')")
                .bind(&symbol).bind(&symbol).execute(&mut *tx).await?.last_insert_id();
            assets.push((id, symbol));
        }
        let symbol = format!("{}-{}", assets[0].1, assets[1].1);
        let pair = sqlx::query("INSERT INTO trading_pairs(base_asset,quote_asset,symbol,price_precision,qty_precision,min_order_value,status,market_type) VALUES (?, ?, ?, 18, 18, 1, 'active', 'external')")
            .bind(assets[0].0).bind(assets[1].0).bind(&symbol).execute(&mut *tx).await?.last_insert_id();
        let mode = if cross { "cross" } else { "isolated" };
        let product = sqlx::query("INSERT INTO margin_products(pair_id,margin_asset,margin_mode,margin_modes,leverage_levels,max_leverage,min_margin,maintenance_margin_rate) VALUES (?, ?, ?, JSON_ARRAY(?), JSON_ARRAY('5'), 5, 1, 0.005)")
            .bind(pair).bind(assets[1].0).bind(mode).bind(mode)
            .execute(&mut *tx).await?.last_insert_id();
        let wallet_scope = if cross { "margin" } else { "spot" };
        let position = sqlx::query("INSERT INTO margin_positions(user_id,product_id,pair_id,margin_asset,wallet_scope,margin_mode,direction,margin_amount,leverage,notional_amount,borrowed_amount,interest_amount,entry_price,idempotency_key) VALUES (?, ?, ?, ?, ?, ?, 'long', 20, 5, 100, 80, 0, 100, ?)")
            .bind(user).bind(product).bind(pair).bind(assets[1].0)
            .bind(wallet_scope).bind(mode).bind(&suffix).execute(&mut *tx).await?.last_insert_id();
        let table = if cross {
            "margin_wallet_accounts"
        } else {
            "wallet_accounts"
        };
        sqlx::query(&format!(
            "INSERT INTO {table}(user_id,asset_id,available) VALUES (?, ?, 100)"
        ))
        .bind(user)
        .bind(assets[1].0)
        .execute(&mut *tx)
        .await?;
        if cross {
            ensure_and_lock_cross_margin_account(&mut tx, user, assets[1].0).await?;
        }
        tx.commit().await?;
        Ok(Self {
            user,
            base: assets[0].0,
            asset: assets[1].0,
            pair,
            product,
            position,
            symbol,
            cross,
        })
    }

    async fn lock(&self, tx: &mut Transaction<'_, MySql>) -> AppResult<()> {
        if self.cross {
            claim_cross_margin_account_for_close(tx, self.user, self.asset).await?;
        }
        assert!(
            lock_user_position_by_id(tx, self.user, self.position)
                .await?
                .is_some()
        );
        Ok(())
    }

    async fn cleanup(&self, pool: &MySqlPool) -> TestResult {
        for table in [
            "platform_financial_journal",
            "wallet_ledger",
            "margin_wallet_ledger",
        ] {
            sqlx::query(&format!("DELETE FROM {table} WHERE asset_id = ?"))
                .bind(self.asset)
                .execute(pool)
                .await?;
        }
        for table in [
            "margin_position_close_executions",
            "margin_positions",
            "margin_cross_accounts",
            "wallet_accounts",
            "margin_wallet_accounts",
        ] {
            sqlx::query(&format!("DELETE FROM {table} WHERE user_id = ?"))
                .bind(self.user)
                .execute(pool)
                .await?;
        }
        sqlx::query("DELETE FROM margin_products WHERE id = ?")
            .bind(self.product)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM trading_pairs WHERE id = ?")
            .bind(self.pair)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM assets WHERE id IN (?, ?)")
            .bind(self.base)
            .bind(self.asset)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(self.user)
            .execute(pool)
            .await?;
        Ok(())
    }
}

async fn insert_after_missing_lookup(
    pool: &MySqlPool,
    fixture: &Fixture,
    barrier: &Barrier,
) -> AppResult<u64> {
    let mut tx = pool.begin().await?;
    fixture.lock(&mut tx).await?;
    assert!(
        load_margin_close_execution_by_key_in_tx(&mut tx, fixture.user, "missing-key")
            .await?
            .is_none()
    );
    // Both absent-key reads must complete before either insert. FOR UPDATE here
    // deterministically creates the shared-supremum gap-lock deadlock.
    barrier.wait().await;
    let id = insert_margin_close_execution(
        &mut tx,
        MarginCloseExecutionWrite {
            user_id: fixture.user,
            position_id: fixture.position,
            idempotency_key: "missing-key",
            close_percentage: 50,
            close_margin_amount: &BigDecimal::from(10),
            close_notional_amount: &BigDecimal::from(50),
            close_borrowed_amount: &BigDecimal::from(40),
            close_interest_amount: &BigDecimal::from(0),
            exit_price: &BigDecimal::from(110),
            realized_pnl: &BigDecimal::from(5),
            settlement_amount: &BigDecimal::from(15),
            fully_closed: false,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(id)
}

#[tokio::test]
async fn margin_close_concurrency_missing_keys_insert_without_gap_deadlock() -> TestResult {
    let Some(pool) = test_pool().await? else {
        return Ok(());
    };
    let first = Fixture::new(&pool, false).await?;
    let second = Fixture::new(&pool, true).await?;
    let barrier = Barrier::new(2);
    let (a, b) = timeout(Duration::from_secs(10), async {
        tokio::try_join!(
            insert_after_missing_lookup(&pool, &first, &barrier),
            insert_after_missing_lookup(&pool, &second, &barrier),
        )
    })
    .await??;
    assert_ne!(a, b);
    first.cleanup(&pool).await?;
    second.cleanup(&pool).await?;
    Ok(())
}

fn intent(key: &str, percentage: i64) -> CloseMarginPositionRequest {
    CloseMarginPositionRequest {
        percentage: Some(percentage),
        idempotency_key: Some(key.into()),
    }
}

async fn wait_for_two_blocked_transactions(
    monitor: &mut MySqlConnection,
    fixture: &Fixture,
    blocker_connection_id: u64,
) -> TestResult {
    let table = if fixture.cross {
        "margin_cross_accounts"
    } else {
        "margin_positions"
    };
    let waited = timeout(Duration::from_secs(10), async {
        loop {
            // The no-op upsert can wait on PRIMARY supremum insert intention,
            // not the account's row key. Follow this fixture's actual blocker
            // and chained waiters, without counting unrelated table locks.
            let count: i64 = sqlx::query_scalar(
                "WITH RECURSIVE blocked (trx_id) AS (
                    SELECT w.REQUESTING_ENGINE_TRANSACTION_ID
                    FROM performance_schema.data_lock_waits w
                    JOIN performance_schema.data_locks r ON r.ENGINE_LOCK_ID = w.REQUESTING_ENGINE_LOCK_ID
                    JOIN performance_schema.threads t ON t.THREAD_ID = w.BLOCKING_THREAD_ID
                    WHERE t.PROCESSLIST_ID = ?
                      AND r.OBJECT_SCHEMA = DATABASE() AND r.OBJECT_NAME = ?
                    UNION DISTINCT
                    SELECT w.REQUESTING_ENGINE_TRANSACTION_ID
                    FROM performance_schema.data_lock_waits w
                    JOIN blocked b ON b.trx_id = w.BLOCKING_ENGINE_TRANSACTION_ID
                    JOIN performance_schema.data_locks r ON r.ENGINE_LOCK_ID = w.REQUESTING_ENGINE_LOCK_ID
                    WHERE r.OBJECT_SCHEMA = DATABASE() AND r.OBJECT_NAME = ?
                 ) SELECT COUNT(*) FROM blocked",
            )
            .bind(blocker_connection_id)
            .bind(table)
            .bind(table)
            .fetch_one(&mut *monitor)
            .await?;
            if count >= 2 {
                return Ok::<_, sqlx::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    if waited.is_err() {
        let locks: Vec<(String, Option<String>, Option<String>, String)> = sqlx::query_as(
            "SELECT OBJECT_NAME, INDEX_NAME, LOCK_DATA, LOCK_MODE FROM performance_schema.data_locks
             WHERE OBJECT_SCHEMA = DATABASE() AND LOCK_STATUS = 'WAITING'",
        ).fetch_all(&mut *monitor).await?;
        eprintln!(
            "wait detector cross={} blocker_connection={blocker_connection_id}: {locks:?}",
            fixture.cross
        );
    }
    waited??;
    Ok(())
}

async fn assert_close_account_claim_is_noop(pool: &MySqlPool, fixture: &Fixture) -> TestResult {
    sqlx::query("UPDATE margin_cross_accounts SET status = 'liquidating', updated_at = '2000-01-01 00:00:00' WHERE user_id = ? AND margin_asset = ?")
        .bind(fixture.user).bind(fixture.asset).execute(pool).await?;
    let snapshot_sql = "SELECT status, version, last_equity, updated_at FROM margin_cross_accounts WHERE user_id = ? AND margin_asset = ?";
    let before: (String, u64, BigDecimal, chrono::DateTime<Utc>) = sqlx::query_as(snapshot_sql)
        .bind(fixture.user)
        .bind(fixture.asset)
        .fetch_one(pool)
        .await?;
    let mut tx = pool.begin().await?;
    let account =
        claim_cross_margin_account_for_close(&mut tx, fixture.user, fixture.asset).await?;
    assert_eq!((&account.status, account.version), (&before.0, before.1));
    tx.commit().await?;
    let after: (String, u64, BigDecimal, chrono::DateTime<Utc>) = sqlx::query_as(snapshot_sql)
        .bind(fixture.user)
        .bind(fixture.asset)
        .fetch_one(pool)
        .await?;
    assert_eq!(
        before, after,
        "a lock-only claim must not touch state/version/risk/time"
    );
    let balance: BigDecimal = sqlx::query_scalar(
        "SELECT available FROM margin_wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(fixture.user)
    .bind(fixture.asset)
    .fetch_one(pool)
    .await?;
    assert_eq!(balance, BigDecimal::from(100));
    sqlx::query(
        "UPDATE margin_cross_accounts SET status = 'active' WHERE user_id = ? AND margin_asset = ?",
    )
    .bind(fixture.user)
    .bind(fixture.asset)
    .execute(pool)
    .await?;
    Ok(())
}

#[tokio::test]
async fn margin_close_concurrency_waited_isolated_and_cross_replay_see_committed_execution()
-> TestResult {
    let Some(pool) = test_pool().await? else {
        return Ok(());
    };
    let Ok(redis_url) = std::env::var("MARGIN_CONCURRENCY_REDIS_URL") else {
        eprintln!("skipping same-position concurrency test: MARGIN_CONCURRENCY_REDIS_URL is unset");
        return Ok(());
    };
    let redis = redis::aio::ConnectionManager::new(redis::Client::open(redis_url)?).await?;
    let mut monitor =
        MySqlConnection::connect(&std::env::var("MARGIN_CONCURRENCY_DATABASE_URL")?).await?;
    for cross in [false, true] {
        let fixture = Arc::new(Fixture::new(&pool, cross).await?);
        if cross {
            assert_close_account_claim_is_noop(&pool, &fixture).await?;
        }
        let key = Uuid::now_v7().to_string();
        let mut cache = redis.clone();
        let _: () = cache
            .set(
                market_ticker_redis_key(&fixture.symbol),
                serde_json::json!({
                    "symbol": fixture.symbol.replace('-', ""),
                    "last_price": "110", "volume_24h": "1",
                    "observed_at": Utc::now().timestamp_millis(),
                })
                .to_string(),
            )
            .await?;
        let mut blocker = pool.begin().await?;
        let blocker_connection_id: u64 = sqlx::query_scalar("SELECT CONNECTION_ID()")
            .fetch_one(&mut *blocker)
            .await?;
        fixture.lock(&mut blocker).await?;
        let mut calls = Vec::new();
        for _ in 0..2 {
            let pool = pool.clone();
            let redis = redis.clone();
            let fixture = fixture.clone();
            let key = key.clone();
            calls.push(tokio::spawn(async move {
                close_margin_position_with_events(
                    &pool,
                    Some(&redis),
                    None,
                    fixture.user,
                    fixture.position,
                    intent(&key, 100),
                )
                .await
            }));
        }
        // Both pass preflight before either commit. Full close makes a stale
        // snapshot fail at the closed-state guard instead of falling back to a
        // duplicate INSERT, proving visibility for in-tx replay.
        let waiting =
            wait_for_two_blocked_transactions(&mut monitor, &fixture, blocker_connection_id).await;
        blocker.rollback().await?;
        let mut results = Vec::new();
        for call in calls {
            results.push(timeout(Duration::from_secs(10), call).await???);
        }
        waiting?;
        assert_eq!(
            results.iter().filter(|response| !response.replayed).count(),
            1
        );
        let execution = results[0].execution.as_ref().unwrap();
        assert_eq!(results[1].execution.as_ref().unwrap().id, execution.id);
        assert_eq!(execution.close_margin_amount, BigDecimal::from(20));
        assert_eq!(execution.settlement_amount, BigDecimal::from(30));
        assert!(execution.fully_closed);
        assert!(
            results
                .iter()
                .all(|response| response.position.status == "closed")
        );
        let changed_intent = close_margin_position_with_events(
            &pool,
            Some(&redis),
            None,
            fixture.user,
            fixture.position,
            intent(&key, 25),
        )
        .await;
        assert!(matches!(changed_intent, Err(AppError::Conflict(_))));
        let other_position = sqlx::query(
            "INSERT INTO margin_positions
             (user_id,product_id,pair_id,margin_asset,wallet_scope,margin_mode,direction,
              margin_amount,leverage,notional_amount,borrowed_amount,interest_amount,
              entry_price,idempotency_key)
             SELECT user_id,product_id,pair_id,margin_asset,wallet_scope,margin_mode,direction,
                    margin_amount,leverage,notional_amount,borrowed_amount,interest_amount,
                    entry_price,?
             FROM margin_positions WHERE id = ?",
        )
        .bind(Uuid::now_v7().to_string())
        .bind(fixture.position)
        .execute(&pool)
        .await?
        .last_insert_id();
        let changed_position = close_margin_position_with_events(
            &pool,
            Some(&redis),
            None,
            fixture.user,
            other_position,
            intent(&key, 100),
        )
        .await;
        assert!(matches!(changed_position, Err(AppError::Conflict(_))));
        let table = if cross {
            "margin_wallet_accounts"
        } else {
            "wallet_accounts"
        };
        let available: BigDecimal = sqlx::query_scalar(&format!(
            "SELECT available FROM {table} WHERE user_id = ? AND asset_id = ?"
        ))
        .bind(fixture.user)
        .bind(fixture.asset)
        .fetch_one(&pool)
        .await?;
        assert_eq!(available, BigDecimal::from(130));
        let executions: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM margin_position_close_executions WHERE user_id = ?",
        )
        .bind(fixture.user)
        .fetch_one(&pool)
        .await?;
        assert_eq!(executions, 1);
        let journal: (i64, BigDecimal) = sqlx::query_as("SELECT COUNT(DISTINCT transaction_key), COALESCE(SUM(amount), 0) FROM platform_financial_journal WHERE asset_id = ?")
            .bind(fixture.asset).fetch_one(&pool).await?;
        assert_eq!(journal, (1, BigDecimal::from(0)));
        if cross {
            let account: (String, u64, BigDecimal) = sqlx::query_as(
                "SELECT status, version, last_equity FROM margin_cross_accounts WHERE user_id = ? AND margin_asset = ?",
            ).bind(fixture.user).bind(fixture.asset).fetch_one(&pool).await?;
            assert_eq!(account, ("active".into(), 1, BigDecimal::from(0)));
        }
        let _: () = cache.del(market_ticker_redis_key(&fixture.symbol)).await?;
        fixture.cleanup(&pool).await?;
    }
    Ok(())
}
