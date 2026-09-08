//! 用真实隔离 MySQL 验证交易对锁与主池容量隔离；不创建交易对、不写行情或资金。

use std::{
    error::Error,
    time::{Duration, Instant},
};

use exchange_api::modules::market::infrastructure::default_runtime::PairGenerationLock;
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use tokio::time::timeout;
use uuid::Uuid;

async fn isolated_single_connection_pool() -> Result<Option<MySqlPool>, Box<dyn Error>> {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping default market ownership tests: isolated DATABASE_URL is required");
        return Ok(None);
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(2))
        .connect(&url)
        .await?;
    let database: String = sqlx::query_scalar("SELECT DATABASE()")
        .fetch_one(&pool)
        .await?;
    assert!(
        database.starts_with("codex_default_market_")
            || database.starts_with("codex_default_runtime_"),
        "ownership tests require an isolated default-market database"
    );
    Ok(Some(pool))
}

fn isolated_pair_id() -> u64 {
    u64::from_be_bytes(Uuid::now_v7().as_bytes()[8..16].try_into().unwrap())
}

#[tokio::test]
async fn single_connection_pool_allows_business_transactions_and_same_pair_contention()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = isolated_single_connection_pool().await? else {
        return Ok(());
    };
    let pair_id = isolated_pair_id();
    let mut owner = timeout(
        Duration::from_secs(3),
        PairGenerationLock::acquire(&pool, pair_id, 0),
    )
    .await??
    .expect("first pair owner");
    owner.ensure_owned().await?;
    // 命名锁所在连接已脱离业务池；上限为 1 的池仍能及时开启独立事务。
    let mut tx = timeout(Duration::from_secs(1), pool.begin()).await??;
    let business_connection: u64 = sqlx::query_scalar("SELECT CONNECTION_ID()")
        .fetch_one(&mut *tx)
        .await?;
    assert_ne!(business_connection, owner.connection_id());
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&mut *tx)
            .await?,
        1
    );
    tx.commit().await?;

    let skipped = timeout(
        Duration::from_secs(2),
        PairGenerationLock::acquire(&pool, pair_id, 0),
    )
    .await??;
    assert!(
        skipped.is_none(),
        "same-pair worker must skip rather than obtain a second owner"
    );
    let started = Instant::now();
    let timed_out = timeout(
        Duration::from_secs(3),
        PairGenerationLock::acquire(&pool, pair_id, 1),
    )
    .await??;
    assert!(
        timed_out.is_none(),
        "bounded contention must time out without stealing ownership"
    );
    assert!(started.elapsed() >= Duration::from_millis(800));
    owner.ensure_owned().await?;
    let original_connection = owner.connection_id();
    owner.release().await?;
    let mut released = timeout(
        Duration::from_secs(3),
        PairGenerationLock::acquire(&pool, pair_id, 1),
    )
    .await??
    .expect("released pair lock can be reacquired");
    assert_ne!(released.connection_id(), original_connection);
    released.ensure_owned().await?;
    drop(released);
    let mut after_drop = timeout(
        Duration::from_secs(3),
        PairGenerationLock::acquire(&pool, pair_id, 1),
    )
    .await??
    .expect("dropped dedicated connection releases pair lock");
    after_drop.ensure_owned().await?;
    after_drop.release().await?;
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn multiple_detached_pair_owners_leave_single_business_slot_available()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = isolated_single_connection_pool().await? else {
        return Ok(());
    };
    let mut owners = Vec::new();
    // 多个独立交易对持锁连接不参与主池 max_connections=1 的容量计数。
    for _ in 0..3 {
        owners.push(
            timeout(
                Duration::from_secs(3),
                PairGenerationLock::acquire(&pool, isolated_pair_id(), 1),
            )
            .await??
            .expect("independent pair obtains its own detached connection"),
        );
    }
    let mut tx = timeout(Duration::from_secs(1), pool.begin()).await??;
    let business_connection: u64 = sqlx::query_scalar("SELECT CONNECTION_ID()")
        .fetch_one(&mut *tx)
        .await?;
    assert!(
        owners
            .iter()
            .all(|owner| owner.connection_id() != business_connection)
    );
    tx.rollback().await?;
    for owner in &mut owners {
        owner.ensure_owned().await?;
    }
    for owner in owners {
        owner.release().await?;
    }
    pool.close().await;
    Ok(())
}
