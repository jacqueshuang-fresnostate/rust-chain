use crate::{
    error::{AppError, AppResult},
    modules::admin::application::apply_admin_agent_commission_status,
};
use chrono::{DateTime, Utc};
use sqlx::{MySql, Pool};
use std::collections::HashSet;
use tokio::time::{Duration, interval};
use tracing::{error, info, warn};

const GUARD_CAPACITY: usize = 10_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AgentCommissionSettlementSummary {
    pub scanned: u32,
    pub settled: u32,
    pub skipped: u32,
    pub failed: u32,
}

/// 记录本进程内已失败的佣金 ID，避免每个周期重复重试同一批坏记录。
#[derive(Debug, Default)]
pub struct AgentCommissionSettlementGuard {
    failed_ids: HashSet<u64>,
}

impl AgentCommissionSettlementGuard {
    /// 判断佣金 ID 是否尚未进入当前进程的失败集合；仅用于跨周期避开已知坏记录，数据库 `pending` 状态仍是重启后的幂等依据。
    pub fn should_attempt(&self, commission_id: u64) -> bool {
        !self.failed_ids.contains(&commission_id)
    }

    /// 把失败佣金 ID 加入进程内 guard；集合达到 10,000 项时整体清空，允许旧失败项以后重新尝试并限制常驻内存。
    pub fn record_failure(&mut self, commission_id: u64) {
        if self.failed_ids.len() >= GUARD_CAPACITY {
            self.failed_ids.clear();
        }
        self.failed_ids.insert(commission_id);
    }
}

#[derive(Debug, sqlx::FromRow)]
struct PendingCommissionCandidate {
    id: u64,
}

/// 单轮按 ID 升序扫描账龄达标的 pending 佣金：成功上限为 `limit` 收敛到 1..=200，候选扫描最多放大十倍且不超过 1,000。
/// 每项独立调用权威结算用例，不持有跨项事务；冲突和失败写入进程 guard、计数后继续，已结算项不因后项失败回滚。
/// 数据库状态承担跨重启幂等，worker 本身不发布提交后事件；实际打款、状态锁定及审计副作用以应用用例合同为准。
pub async fn run_once_with_dependencies(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    min_age_seconds: u64,
    limit: u32,
    guard: &mut AgentCommissionSettlementGuard,
) -> AppResult<AgentCommissionSettlementSummary> {
    let settle_limit = agent_commission_settle_limit(limit);
    let candidates = fetch_pending_commissions(
        pool,
        eligible_created_before(now, min_age_seconds),
        agent_commission_scan_limit(limit),
    )
    .await?;
    let mut summary = AgentCommissionSettlementSummary::default();

    for candidate in candidates {
        if summary.settled >= settle_limit {
            break;
        }
        if !guard.should_attempt(candidate.id) {
            continue;
        }
        summary.scanned += 1;
        match apply_admin_agent_commission_status(pool, None, candidate.id, "settled", None).await {
            Ok(_) => summary.settled += 1,
            // 无打款支持等冲突只记录并跳过，不做任何状态回写。
            Err(AppError::Conflict(reason)) => {
                summary.skipped += 1;
                guard.record_failure(candidate.id);
                warn!(commission_id = candidate.id, %reason, "代理佣金自动结算跳过");
            }
            Err(error) => {
                summary.failed += 1;
                guard.record_failure(candidate.id);
                warn!(commission_id = candidate.id, %error, "代理佣金自动结算失败");
            }
        }
    }

    Ok(summary)
}

/// 以至少 1 秒间隔持续运行代理佣金结算；候选查询等周期级错误只记录并进入下一轮，单项错误由单轮继续语义吸收。
/// 同一进程复用最多 10,000 项的失败 guard；重启后 guard 清空，数据库中仍为 pending 的记录会重新进入幂等结算。
pub async fn run_loop(
    pool: Pool<MySql>,
    interval_seconds: u64,
    min_age_seconds: u64,
    limit: u32,
) -> AppResult<()> {
    let mut ticker = interval(Duration::from_secs(interval_seconds.max(1)));
    let mut guard = AgentCommissionSettlementGuard::default();

    loop {
        ticker.tick().await;
        match run_once_with_dependencies(&pool, Utc::now(), min_age_seconds, limit, &mut guard)
            .await
        {
            Ok(summary) => info!(
                scanned = summary.scanned,
                settled = summary.settled,
                skipped = summary.skipped,
                failed = summary.failed,
                "代理佣金自动结算周期完成"
            ),
            Err(error) => error!(%error, "代理佣金自动结算周期失败"),
        }
    }
}

async fn fetch_pending_commissions(
    pool: &Pool<MySql>,
    created_before: DateTime<Utc>,
    limit: u32,
) -> AppResult<Vec<PendingCommissionCandidate>> {
    sqlx::query_as::<_, PendingCommissionCandidate>(
        r#"SELECT id
           FROM agent_commission_records
           WHERE status = 'pending'
             AND created_at <= ?
           ORDER BY id ASC
           LIMIT ?"#,
    )
    .bind(created_before.naive_utc())
    .bind(limit as i64)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

fn eligible_created_before(now: DateTime<Utc>, min_age_seconds: u64) -> DateTime<Utc> {
    now - chrono::Duration::seconds(min_age_seconds.min(i64::MAX as u64) as i64)
}

fn agent_commission_settle_limit(limit: u32) -> u32 {
    limit.clamp(1, 200)
}

fn agent_commission_scan_limit(limit: u32) -> u32 {
    agent_commission_settle_limit(limit)
        .saturating_mul(10)
        .clamp(1, 1000)
}

#[cfg(test)]
#[path = "../../tests/unit_src/src_workers_agent_commission_settlement_tests.rs"]
mod tests;
