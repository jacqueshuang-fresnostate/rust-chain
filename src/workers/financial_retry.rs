//! 资金 worker 的持久化调度；租约只减少重复执行，资金幂等仍由业务事务保证。
use crate::error::AppResult;
use chrono::{DateTime, Duration, Utc};
use sqlx::MySqlPool;
use uuid::Uuid;

/// 一次独占调度尝试，完成时只能回写自己的租约。
pub struct RetryLease {
    kind: &'static str,
    item_id: u64,
    token: String,
    attempt: u64,
}

/// 可观测的重试分类；只保存稳定分类，不持久化可能含敏感信息的原始错误。
#[derive(Clone, Copy)]
pub enum RetryOutcome {
    Complete,
    WaitingBalance,
    WaitingSource,
    Failed,
}

/// 在短事务中领取到期任务并提交五分钟租约；并发争抢失败返回 None，进程崩溃后租约到期自动恢复。
pub async fn claim(
    pool: &MySqlPool,
    kind: &'static str,
    item_id: u64,
    now: DateTime<Utc>,
) -> AppResult<Option<RetryLease>> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO financial_worker_retries (task_kind, item_id, next_attempt_at) VALUES (?, ?, ?) ON DUPLICATE KEY UPDATE item_id = item_id",
    )
    .bind(kind).bind(item_id).bind(now.naive_utc()).execute(&mut *tx).await?;
    let token = Uuid::now_v7().to_string();
    let claimed = sqlx::query(
        r#"UPDATE financial_worker_retries SET lease_token = ?, outcome = 'running',
           attempt_count = attempt_count + 1, last_attempt_at = ?, next_attempt_at = ?
           WHERE task_kind = ? AND item_id = ? AND next_attempt_at <= ?"#,
    )
    .bind(&token)
    .bind(now.naive_utc())
    .bind((now + Duration::minutes(5)).naive_utc())
    .bind(kind)
    .bind(item_id)
    .bind(now.naive_utc())
    .execute(&mut *tx)
    .await?;
    if claimed.rows_affected() == 0 {
        tx.rollback().await?;
        return Ok(None);
    }
    let attempt = sqlx::query_scalar(
        "SELECT attempt_count FROM financial_worker_retries WHERE task_kind = ? AND item_id = ?",
    )
    .bind(kind)
    .bind(item_id)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Some(RetryLease {
        kind,
        item_id,
        token,
        attempt,
    }))
}

/// 完成只清理当前租约；等待项一分钟后再查，失败项指数退避到一小时，不设永久黑名单。
/// 旧进程晚到的结果不能覆盖新租约；业务提交后调度回写失败可重试，原资金事务仍负责幂等。
pub async fn finish(
    pool: &MySqlPool,
    lease: RetryLease,
    now: DateTime<Utc>,
    outcome: RetryOutcome,
) -> AppResult<()> {
    let (label, delay) = match outcome {
        RetryOutcome::Complete => {
            sqlx::query("DELETE FROM financial_worker_retries WHERE task_kind = ? AND item_id = ? AND lease_token = ?")
                .bind(lease.kind).bind(lease.item_id).bind(lease.token).execute(pool).await?;
            return Ok(());
        }
        RetryOutcome::WaitingBalance => ("waiting_balance", 60),
        RetryOutcome::WaitingSource => ("waiting_source", 60),
        RetryOutcome::Failed => ("failed", retry_delay(lease.attempt)),
    };
    sqlx::query(
        r#"UPDATE financial_worker_retries SET outcome = ?, next_attempt_at = ?, lease_token = NULL
           WHERE task_kind = ? AND item_id = ? AND lease_token = ?"#,
    )
    .bind(label)
    .bind((now + Duration::seconds(delay)).naive_utc())
    .bind(lease.kind)
    .bind(lease.item_id)
    .bind(lease.token)
    .execute(pool)
    .await?;
    Ok(())
}

fn retry_delay(attempt: u64) -> i64 {
    (60_i64 * (1_i64 << attempt.saturating_sub(1).min(6))).min(3600)
}

#[cfg(test)]
#[path = "../../tests/unit_src/src_workers_financial_retry_tests.rs"]
mod tests;
