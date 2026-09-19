use crate::{
    error::{AppError, AppResult},
    modules::loan::{
        application::settle_locked_loan_order_repayment_in_tx,
        domain::{STATUS_DISBURSED, STATUS_OVERDUE, STATUS_REPAID},
        infrastructure::lock_loan_order,
    },
    workers::{
        financial_retry::{self, RetryOutcome},
        loan_health,
    },
};
use chrono::{DateTime, Utc};
use redis::{Client, aio::ConnectionManager};
use sqlx::{MySql, Pool};
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
    /// 读取贷款逾期与健康扫描共用开关、周期和批量配置；默认启用以保证风险状态会被持续推进。
    pub fn from_env() -> Self {
        Self {
            enabled: env_bool("LOAN_OVERDUE_ENABLED", true),
            interval_seconds: env_u64("LOAN_OVERDUE_INTERVAL_SECONDS", 300),
            batch_limit: env_u32("LOAN_OVERDUE_BATCH_LIMIT", 100),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LoanOverdueSummary {
    pub scanned: u32,
    pub marked: u32,
    pub collected: u32,
    pub skipped: u32,
    pub failed: u32,
    pub waiting_balance: u32,
}

#[derive(Debug, sqlx::FromRow)]
struct LoanOverdueCandidate {
    order_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoanOverdueOutcome {
    Marked,
    Collected,
    Skipped,
    WaitingBalance,
}

/// 单轮按到期时间和 ID 扫描已放款订单：成功上限为 `limit` 收敛到 1..=200，候选最多放大十倍且不超过 1,000。
/// 每个订单在独立事务内 `FOR UPDATE` 后由 `disbursed` 推进为 `overdue`；可用余额覆盖本息时再走共用还款路径结清。
/// 余额不足保持 overdue，不造罚息、不做部分扣款；状态已变化时幂等跳过，单项失败计数后继续，已提交项不回滚。
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
        let Some(lease) = financial_retry::claim(pool, "loan", candidate.order_id, now).await?
        else {
            continue;
        };
        summary.scanned += 1;
        let outcome = match mark_order_overdue(pool, candidate.order_id, now).await {
            Ok(LoanOverdueOutcome::Marked) => {
                summary.marked += 1;
                summary.waiting_balance += 1;
                RetryOutcome::WaitingBalance
            }
            Ok(LoanOverdueOutcome::Collected) => {
                summary.marked += 1;
                summary.collected += 1;
                RetryOutcome::Complete
            }
            Ok(LoanOverdueOutcome::WaitingBalance) => {
                summary.waiting_balance += 1;
                RetryOutcome::WaitingBalance
            }
            Ok(LoanOverdueOutcome::Skipped) => {
                summary.skipped += 1;
                RetryOutcome::Complete
            }
            Err(error) => {
                summary.failed += 1;
                warn!(order_id = candidate.order_id, %error, "贷款逾期标记失败");
                RetryOutcome::Failed
            }
        };
        financial_retry::finish(pool, lease, now, outcome).await?;
    }

    Ok(summary)
}

/// 以至少 1 秒间隔持续扫描贷款逾期；候选查询等周期级错误只记录并继续，单项失败已由单轮隔离。
/// 业务状态承担资金幂等，持久化重试记录承担跨重启公平调度；不补发提交后事件。
pub async fn run_loop(pool: Pool<MySql>, interval_seconds: u64, limit: u32) -> AppResult<()> {
    let mut ticker = interval(Duration::from_secs(interval_seconds.max(1)));
    let mut health_redis = match connect_loan_health_redis().await {
        Ok(redis) => Some(redis),
        Err(error) => {
            error!(%error, "贷款健康扫描 Redis 初始化失败，将在周期内重试");
            None
        }
    };

    loop {
        ticker.tick().await;
        let now = Utc::now();
        match run_once_with_dependencies(&pool, now, limit).await {
            Ok(summary) => info!(
                scanned = summary.scanned,
                marked = summary.marked,
                collected = summary.collected,
                skipped = summary.skipped,
                failed = summary.failed,
                waiting_balance = summary.waiting_balance,
                "贷款逾期扫描周期完成"
            ),
            Err(error) => error!(%error, "贷款逾期扫描周期失败"),
        }
        if health_redis.is_none() {
            health_redis = match connect_loan_health_redis().await {
                Ok(redis) => Some(redis),
                Err(error) => {
                    error!(%error, "贷款健康扫描 Redis 重连失败");
                    None
                }
            };
        }
        if let Some(redis) = health_redis.as_ref() {
            // 逾期扫描可能耗时，健康检查必须重新取时钟，避免把已过期 ticker 当成仍然新鲜。
            let health_now = Utc::now();
            match loan_health::run_once_with_dependencies(&pool, redis, health_now, limit).await {
                Ok(summary) => info!(
                    scanned = summary.scanned,
                    liquidated = summary.liquidated,
                    healthy = summary.healthy,
                    skipped = summary.skipped,
                    failed = summary.failed,
                    "贷款健康与清算扫描周期完成"
                ),
                Err(error) => error!(%error, "贷款健康与清算扫描周期失败"),
            }
        }
    }
}

/// 复用进程的 REDIS_URL 建立可重连管理器；URL 不进入错误文案和日志字段。
async fn connect_loan_health_redis() -> AppResult<ConnectionManager> {
    let redis_url = env::var("REDIS_URL")
        .map_err(|_| AppError::Internal("REDIS_URL is required for loan health scan".to_owned()))?;
    let client = Client::open(redis_url)?;
    ConnectionManager::new(client).await.map_err(AppError::from)
}

async fn fetch_overdue_candidates(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<Vec<LoanOverdueCandidate>> {
    sqlx::query_as::<_, LoanOverdueCandidate>(
        r#"SELECT o.id AS order_id
           FROM loan_orders o
           LEFT JOIN financial_worker_retries r ON r.task_kind = 'loan' AND r.item_id = o.id
           WHERE o.status IN ('disbursed', 'overdue') AND o.due_at <= ?
             AND (r.next_attempt_at IS NULL OR r.next_attempt_at <= ?)
           ORDER BY r.last_attempt_at ASC, o.due_at ASC, o.id ASC
           LIMIT ?"#,
    )
    .bind(now.naive_utc())
    .bind(now.naive_utc())
    .bind(limit as i64)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

/// 先把到期已放款单标为 overdue；余额足够时再按用户还款同一口径扣本息结清。
/// 产品未配置逾期罚息，这里不额外计息。余额不足保持 overdue，本轮不写部分流水。
async fn mark_order_overdue(
    pool: &Pool<MySql>,
    order_id: u64,
    now: DateTime<Utc>,
) -> AppResult<LoanOverdueOutcome> {
    let mut tx = pool.begin().await?;
    let order = match lock_loan_order(&mut tx, order_id).await {
        Ok(order) => order,
        Err(AppError::NotFound) => {
            tx.rollback().await?;
            return Ok(LoanOverdueOutcome::Skipped);
        }
        Err(error) => return Err(error),
    };
    if order.status == STATUS_REPAID {
        tx.rollback().await?;
        return Ok(LoanOverdueOutcome::Skipped);
    }
    if order.status != STATUS_DISBURSED && order.status != STATUS_OVERDUE {
        tx.rollback().await?;
        return Ok(LoanOverdueOutcome::Skipped);
    }
    let Some(due_at) = order.due_at.filter(|due_at| *due_at <= now) else {
        tx.rollback().await?;
        return Ok(LoanOverdueOutcome::Skipped);
    };
    let newly_marked = order.status == STATUS_DISBURSED;
    if newly_marked {
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
    }
    match settle_locked_loan_order_repayment_in_tx(&mut tx, &order, now).await {
        Ok(()) => {
            tx.commit().await?;
            info!(order_id = order.id, %due_at, "贷款订单已到期回收");
            Ok(LoanOverdueOutcome::Collected)
        }
        Err(AppError::Validation(message))
            if message.contains("insufficient available balance for loan repayment") =>
        {
            tx.commit().await?;
            if newly_marked {
                info!(order_id = order.id, %due_at, "贷款订单已标记逾期，可用余额不足尚未回收");
                Ok(LoanOverdueOutcome::Marked)
            } else {
                Ok(LoanOverdueOutcome::WaitingBalance)
            }
        }
        Err(error) => {
            tx.rollback().await?;
            Err(error)
        }
    }
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
