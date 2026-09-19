//! 代理佣金自动结算后台任务。
//!
//! 各业务线成交时只生成 pending 状态的返佣记录，真正把金额打入代理钱包由本 worker 定时完成。
//! 每轮按主键升序捞取账龄达标的待结算记录，逐条复用后台管理侧的权威结算用例，
//! 由该用例在单个事务内锁记录、给代理用户钱包可用余额入账、写流水并把状态改为 settled。
//! 幂等由数据库状态承担：仅 pending 可被结算，重放会被状态检查挡下而不会二次入账；
//! 失败和等待来源的调度持久化，按上次尝试时间公平扫描；重启不丢失退避状态。

use crate::{
    error::{AppError, AppResult},
    modules::admin::{
        application::apply_admin_agent_commission_status,
        service::AGENT_COMMISSION_SOURCE_NOT_TERMINAL,
    },
    workers::financial_retry::{self, RetryOutcome},
};
use chrono::{DateTime, Utc};
use sqlx::{MySql, Pool};
use tokio::time::{Duration, interval};
use tracing::{error, info, warn};

/// 单轮结算的计数汇总，仅用于日志与测试断言，不落库也不参与幂等判定。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AgentCommissionSettlementSummary {
    /// 真正取得租约并发起过结算尝试的笔数。
    pub scanned: u32,
    pub settled: u32,
    /// 因业务冲突被拒的笔数，例如来源不支持打款或状态已非 pending。
    pub skipped: u32,
    pub failed: u32,
}

#[derive(Debug, sqlx::FromRow)]
struct PendingCommissionCandidate {
    id: u64,
}

/// 单轮按 ID 升序扫描账龄达标的 pending 佣金：成功上限为 `limit` 收敛到 1..=200，候选扫描最多放大十倍且不超过 1,000。
/// 每项独立调用权威结算用例，不持有跨项事务；冲突和失败写入持久化退避、计数后继续，已结算项不因后项失败回滚。
/// 数据库状态承担跨重启幂等，worker 本身不发布提交后事件；实际打款、状态锁定及审计副作用以应用用例合同为准。
pub async fn run_once_with_dependencies(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    min_age_seconds: u64,
    limit: u32,
) -> AppResult<AgentCommissionSettlementSummary> {
    let settle_limit = agent_commission_settle_limit(limit);
    let candidates = fetch_pending_commissions(
        pool,
        now,
        eligible_created_before(now, min_age_seconds),
        agent_commission_scan_limit(limit),
    )
    .await?;
    let mut summary = AgentCommissionSettlementSummary::default();

    for candidate in candidates {
        if summary.settled >= settle_limit {
            break;
        }
        let Some(lease) = financial_retry::claim(pool, "commission", candidate.id, now).await?
        else {
            continue;
        };
        summary.scanned += 1;
        let outcome =
            match apply_admin_agent_commission_status(pool, None, candidate.id, "settled", None)
                .await
            {
                Ok(_) => {
                    summary.settled += 1;
                    RetryOutcome::Complete
                }
                Err(AppError::Conflict(reason))
                    if reason == AGENT_COMMISSION_SOURCE_NOT_TERMINAL =>
                {
                    summary.skipped += 1;
                    warn!(commission_id = candidate.id, %reason, "代理佣金自动结算等待来源终态");
                    RetryOutcome::WaitingSource
                }
                // 无打款支持等永久冲突只记录并跳过，不做任何状态回写。
                Err(AppError::Conflict(reason)) => {
                    summary.skipped += 1;
                    warn!(commission_id = candidate.id, %reason, "代理佣金自动结算跳过");
                    RetryOutcome::Failed
                }
                Err(error) => {
                    summary.failed += 1;
                    warn!(commission_id = candidate.id, %error, "代理佣金自动结算失败");
                    RetryOutcome::Failed
                }
            };
        financial_retry::finish(pool, lease, now, outcome).await?;
    }

    Ok(summary)
}

/// 以至少 1 秒间隔持续运行代理佣金结算；候选查询等周期级错误只记录并进入下一轮，单项错误由单轮继续语义吸收。
/// 退避及租约保存在数据库；进程重启后沿用调度，临时错误不依赖人工重启恢复。
pub async fn run_loop(
    pool: Pool<MySql>,
    interval_seconds: u64,
    min_age_seconds: u64,
    limit: u32,
) -> AppResult<()> {
    let mut ticker = interval(Duration::from_secs(interval_seconds.max(1)));

    loop {
        ticker.tick().await;
        match run_once_with_dependencies(&pool, Utc::now(), min_age_seconds, limit).await {
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

/// 先按上次尝试时间、再按主键扫描到期重试，只取 ID，实际字段由结算用例加锁重读。
/// 过滤条件为状态仍是 pending 且创建时间不晚于账龄截止点，让刚生成的佣金有观察窗口再进入自动打款。
/// 未尝试项优先，已失败项按退避时间再进入队列；查询不加锁也不改状态，
/// 因此返回的候选可能在本轮处理前已被后台人工结算，这类冲突由结算用例的状态检查拒绝。
async fn fetch_pending_commissions(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    created_before: DateTime<Utc>,
    limit: u32,
) -> AppResult<Vec<PendingCommissionCandidate>> {
    sqlx::query_as::<_, PendingCommissionCandidate>(
        r#"SELECT c.id
           FROM agent_commission_records c
           LEFT JOIN financial_worker_retries r ON r.task_kind = 'commission' AND r.item_id = c.id
           WHERE c.status = 'pending' AND c.created_at <= ?
             AND (r.next_attempt_at IS NULL OR r.next_attempt_at <= ?)
           ORDER BY r.last_attempt_at ASC, c.id ASC
           LIMIT ?"#,
    )
    .bind(created_before.naive_utc())
    .bind(now.naive_utc())
    .bind(limit as i64)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

/// 计算本轮可结算佣金的创建时间上界，即当前时刻回退最小账龄秒数。
/// 该冷却窗口留给上游业务撤单、纠错和人工干预，避免刚落库的返佣立刻被打款而难以追回。
/// 无法表示的账龄返回最早时间，使超大配置不结算任何记录，也不触发日期运算 panic。
fn eligible_created_before(now: DateTime<Utc>, min_age_seconds: u64) -> DateTime<Utc> {
    i64::try_from(min_age_seconds)
        .ok()
        .and_then(chrono::Duration::try_seconds)
        .and_then(|age| now.checked_sub_signed(age))
        .unwrap_or(DateTime::<Utc>::MIN_UTC)
}

/// 把配置的每轮结算配额收敛到 1 到 200，作为本轮成功打款笔数的硬上限。
/// 下界为一保证配置写零时任务不至于空转，上界限制单轮持有的钱包事务总量，避免长时间占用连接与行锁。
fn agent_commission_settle_limit(limit: u32) -> u32 {
    limit.clamp(1, 200)
}

/// 计算候选扫描条数：在结算配额基础上放大十倍并压到 1,000 以内。
/// 放大是因为候选中可能混有业务拒绝或已被人工处理的记录，若按结算配额等量捞取，
/// 这些记录会挤占名额导致每轮实际成功笔数远低于预期；乘法用饱和运算防止大配额溢出回绕。
fn agent_commission_scan_limit(limit: u32) -> u32 {
    agent_commission_settle_limit(limit)
        .saturating_mul(10)
        .clamp(1, 1000)
}

#[cfg(test)]
#[path = "../../tests/unit_src/src_workers_agent_commission_settlement_tests.rs"]
mod tests;
