use chrono::{Duration, Utc};
use exchange_api::workers::financial_retry::{RetryOutcome, claim, finish};
use sqlx::mysql::MySqlPoolOptions;

#[tokio::test]
async fn persistent_retry_lease_survives_restart_and_fences_late_finish()
-> Result<(), Box<dyn std::error::Error>> {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        return Ok(());
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    let id = uuid::Uuid::now_v7().as_u128() as u64;
    let now = Utc::now();
    let (a, b) = tokio::join!(claim(&pool, "test", id, now), claim(&pool, "test", id, now));
    let (a, b) = (a?, b?);
    assert_ne!(a.is_some(), b.is_some());
    let original = a.or(b).unwrap();
    let restarted = MySqlPoolOptions::new().connect(&url).await?;
    assert!(
        claim(&restarted, "test", id, now + Duration::seconds(299))
            .await?
            .is_none()
    );
    let replacement = claim(&restarted, "test", id, now + Duration::seconds(301))
        .await?
        .unwrap();
    finish(&pool, original, now, RetryOutcome::Complete).await?;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM financial_worker_retries WHERE task_kind = 'test' AND item_id = ?",
    )
    .bind(id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(count, 1, "late finish must not erase the replacement lease");
    finish(&restarted, replacement, now, RetryOutcome::Failed).await?;
    assert!(
        claim(&pool, "test", id, now + Duration::seconds(119))
            .await?
            .is_none()
    );
    let retry = claim(&pool, "test", id, now + Duration::seconds(121))
        .await?
        .unwrap();
    finish(&pool, retry, now, RetryOutcome::WaitingBalance).await?;
    let waiting: String = sqlx::query_scalar(
        "SELECT outcome FROM financial_worker_retries WHERE task_kind = 'test' AND item_id = ?",
    )
    .bind(id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(waiting, "waiting_balance");
    let final_lease = claim(&pool, "test", id, now + Duration::seconds(61))
        .await?
        .unwrap();
    finish(&pool, final_lease, now, RetryOutcome::Complete).await?;
    Ok(())
}
