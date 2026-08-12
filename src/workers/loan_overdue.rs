use crate::{
    error::{AppError, AppResult},
    modules::loan::domain::STATUS_DISBURSED,
};
use chrono::{DateTime, Utc};
use sqlx::{MySql, Pool, Transaction};
use std::env;
use tokio::time::{Duration, interval};
use tracing::{error, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoanOverdueWorkerConfig {
    pub enabled: bool,
    pub interval_seconds: u64,
    pub batch_limit: u32,
}

impl LoanOverdueWorkerConfig {
    /// 从环境变量读取贷款逾期扫描配置；默认关闭，避免未显式启用时推进订单状态。
    pub fn from_env() -> Self {
        Self {
            enabled: env_bool("LOAN_OVERDUE_ENABLED", false),
            interval_seconds: env_u64("LOAN_OVERDUE_INTERVAL_SECONDS", 300),
            batch_limit: env_u32("LOAN_OVERDUE_BATCH_LIMIT", 100),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LoanOverdueSummary {
    pub scanned: u32,
    pub marked: u32,
    pub skipped: u32,
    pub failed: u32,
}

#[derive(Debug, sqlx::FromRow)]
struct LoanOverdueCandidate {
    order_id: u64,
}

#[derive(Debug, sqlx::FromRow)]
struct LockedLoanOrder {
    id: u64,
    status: String,
    due_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoanOverdueOutcome {
    Marked,
    Skipped,
}

/// 单轮扫描已放款且到期的贷款订单，并逐笔锁行推进为逾期状态。
/// 扫描数高于成功上限以越过并发失效候选；单笔失败只计数并继续，状态已变化的订单幂等跳过。
/// 当前入口不计提罚息、不修改钱包，也不发送消息，避免从未配置的费率生成资金结果。
pub async fn run_once_with_dependencies(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<LoanOverdueSummary> {
    let candidates = fetch_overdue_candidates(pool, now, loan_overdue_scan_limit(limit)).await?;
    let mut summary = LoanOverdueSummary::default();

    for candidate in candidates {
        if summary.marked >= loan_overdue_limit(limit) {
            break;
        }
        summary.scanned += 1;
        match mark_order_overdue(pool, candidate.order_id, now).await {
            Ok(LoanOverdueOutcome::Marked) => summary.marked += 1,
            Ok(LoanOverdueOutcome::Skipped) => summary.skipped += 1,
            Err(error) => {
                summary.failed += 1;
                warn!(order_id = candidate.order_id, %error, "贷款逾期标记失败");
            }
        }
    }

    Ok(summary)
}

/// 按固定间隔持续扫描贷款逾期；周期级错误记录后继续下一轮，数据库订单状态是重启后的恢复点。
pub async fn run_loop(pool: Pool<MySql>, interval_seconds: u64, limit: u32) -> AppResult<()> {
    let mut ticker = interval(Duration::from_secs(interval_seconds.max(1)));

    loop {
        ticker.tick().await;
        match run_once_with_dependencies(&pool, Utc::now(), limit).await {
            Ok(summary) => info!(
                scanned = summary.scanned,
                marked = summary.marked,
                skipped = summary.skipped,
                failed = summary.failed,
                "贷款逾期扫描周期完成"
            ),
            Err(error) => error!(%error, "贷款逾期扫描周期失败"),
        }
    }
}

async fn fetch_overdue_candidates(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<Vec<LoanOverdueCandidate>> {
    sqlx::query_as::<_, LoanOverdueCandidate>(
        r#"SELECT id AS order_id
           FROM loan_orders
           WHERE status = 'disbursed'
             AND due_at IS NOT NULL
             AND due_at <= ?
           ORDER BY due_at ASC, id ASC
           LIMIT ?"#,
    )
    .bind(now.naive_utc())
    .bind(limit as i64)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

/// 只做状态推进：产品未配置逾期罚息，这里不额外计息，避免凭空造出运营无法配置的费率。
async fn mark_order_overdue(
    pool: &Pool<MySql>,
    order_id: u64,
    now: DateTime<Utc>,
) -> AppResult<LoanOverdueOutcome> {
    let mut tx = pool.begin().await?;
    let Some(order) = lock_loan_order(&mut tx, order_id).await? else {
        tx.rollback().await?;
        return Ok(LoanOverdueOutcome::Skipped);
    };
    if order.status != STATUS_DISBURSED {
        tx.rollback().await?;
        return Ok(LoanOverdueOutcome::Skipped);
    }
    let Some(due_at) = order.due_at.filter(|due_at| *due_at <= now) else {
        tx.rollback().await?;
        return Ok(LoanOverdueOutcome::Skipped);
    };
    let update = sqlx::query(
        r#"UPDATE loan_orders
           SET status = 'overdue', overdue_at = ?
           WHERE id = ? AND status = 'disbursed'"#,
    )
    .bind(now.naive_utc())
    .bind(order.id)
    .execute(&mut *tx)
    .await?;
    if update.rows_affected() != 1 {
        tx.rollback().await?;
        return Ok(LoanOverdueOutcome::Skipped);
    }
    tx.commit().await?;
    info!(order_id = order.id, %due_at, "贷款订单已标记逾期");
    Ok(LoanOverdueOutcome::Marked)
}

async fn lock_loan_order(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
) -> AppResult<Option<LockedLoanOrder>> {
    sqlx::query_as::<_, LockedLoanOrder>(
        r#"SELECT id, status, due_at
           FROM loan_orders
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(order_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

fn loan_overdue_limit(limit: u32) -> u32 {
    limit.clamp(1, 200)
}

fn loan_overdue_scan_limit(limit: u32) -> u32 {
    loan_overdue_limit(limit).saturating_mul(10).clamp(1, 1000)
}

fn env_bool(key: &str, default: bool) -> bool {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<bool>().ok())
        .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

fn env_u32(key: &str, default: u32) -> u32 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(default)
}

#[cfg(test)]
#[path = "../../tests/unit_src/src_workers_loan_overdue_tests.rs"]
mod tests;
