//! 代理佣金自动结算后台任务。
//!
//! 各业务线成交时只生成 pending 状态的返佣记录，真正把金额打入代理钱包由本 worker 定时完成。
//! 每轮按主键升序捞取账龄达标的待结算记录，逐条复用后台管理侧的权威结算用例，
//! 由该用例在单个事务内锁记录、给代理用户钱包可用余额入账、写流水并把状态改为 settled。
//! 幂等由数据库状态承担：仅 pending 可被结算，重放会被状态检查挡下而不会二次入账；
//! 进程内另有失败集合避免同一批坏记录在每个周期反复重试，重启后该集合清空并重新尝试。

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

/// 单轮结算的计数汇总，仅用于日志与测试断言，不落库也不参与幂等判定。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AgentCommissionSettlementSummary {
    /// 真正发起过结算尝试的笔数，被 guard 跳过的候选不计入。
    pub scanned: u32,
    pub settled: u32,
    /// 因业务冲突被拒的笔数，例如来源不支持打款或状态已非 pending。
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
    /// 清空是粗粒度的全量丢弃而非淘汰最旧项，因此越过容量后所有历史失败项都会在下一轮重新参与扫描。
    /// 该集合只影响重试节奏，不代表结算结论，佣金记录的真实状态始终以数据库为准。
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

/// 按主键升序捞取一批待结算佣金候选，只取记录 ID，实际字段由后续结算用例在事务内加锁重读。
/// 过滤条件为状态仍是 pending 且创建时间不晚于账龄截止点，让刚生成的佣金有观察窗口再进入自动打款。
/// 升序保证长期积压的旧记录优先出队，不会被新记录持续挤占配额；查询不加锁也不改状态，
/// 因此返回的候选可能在本轮处理前已被后台人工结算，这类冲突由结算用例的状态检查拒绝。
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

/// 计算本轮可结算佣金的创建时间上界，即当前时刻回退最小账龄秒数。
/// 该冷却窗口留给上游业务撤单、纠错和人工干预，避免刚落库的返佣立刻被打款而难以追回。
/// 秒数先压到 i64 上界再转换，杜绝超大配置值在转换时溢出得到反向偏移的时间点。
fn eligible_created_before(now: DateTime<Utc>, min_age_seconds: u64) -> DateTime<Utc> {
    now - chrono::Duration::seconds(min_age_seconds.min(i64::MAX as u64) as i64)
}

/// 把配置的每轮结算配额收敛到 1 到 200，作为本轮成功打款笔数的硬上限。
/// 下界为一保证配置写零时任务不至于空转，上界限制单轮持有的钱包事务总量，避免长时间占用连接与行锁。
fn agent_commission_settle_limit(limit: u32) -> u32 {
    limit.clamp(1, 200)
}

/// 计算候选扫描条数：在结算配额基础上放大十倍并压到 1,000 以内。
/// 放大是因为候选中可能混有已被 guard 标记的坏记录或已被人工处理的记录，若按结算配额等量捞取，
/// 这些记录会挤占名额导致每轮实际成功笔数远低于预期；乘法用饱和运算防止大配额溢出回绕。
fn agent_commission_scan_limit(limit: u32) -> u32 {
    agent_commission_settle_limit(limit)
        .saturating_mul(10)
        .clamp(1, 1000)
}

#[cfg(test)]
#[path = "../../tests/unit_src/src_workers_agent_commission_settlement_tests.rs"]
mod tests;
