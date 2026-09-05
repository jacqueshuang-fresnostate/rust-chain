//! 新币锁仓解禁扫描后台任务。
//!
//! 新币认购形成的持仓按批次锁定在钱包 locked 桶中，每个批次对应一条解禁记录并带各自的解禁时刻。
//! 本 worker 定时扫描已到解禁时刻且仍待处理的批次，逐条在独立事务中把数量从 locked 转入 available，
//! 同时扣减锁仓剩余量、把解禁记录置为终态并写入成对的钱包账本流水。
//!
//! 解禁矿工费不在此计算与收取：费率规则与按解禁市值或解禁收益计费的口径属于新币上下文，
//! 本任务与用户手动释放共用完整的缴费凭证、身份、精度和到期校验，paid 标记本身不构成支付证据。
//! 每项释放按资产→钱包→解禁/锁仓锁序提交；异常整体回滚，重放不产生第二组流水。
//! 事件仅在事务成功提交后尽力广播，不持久化也不重放。

use crate::modules::new_coin::infrastructure::{
    MySqlNewCoinReadRepository, UNLOCK_NOT_READY, count_fee_blocked_unlocks, scan_due_unlocks,
};
use crate::{
    error::{AppError, AppResult},
    modules::events::{EventBroadcastHub, EventBroadcastMessage},
    state::AppState,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::{MySql, Pool};
use tokio::time::{Duration, interval};
use tracing::{error, info};

pub struct UnlockScannerWorker;

impl UnlockScannerWorker {
    /// 执行一轮到期解禁；候选查询上限收敛到 1..=100，只有费用条件满足且仍 active 的锁仓进入逐项资金事务。
    /// 终态重扫幂等跳过，单项数据库错误会终止本批；释放事件只在对应事务提交后广播。
    pub async fn run_once(
        &self,
        state: &AppState,
        now: DateTime<Utc>,
        limit: u32,
    ) -> AppResult<UnlockScannerSummary> {
        run_once(state, now, limit).await
    }
}

/// 单轮解禁扫描的计数汇总，仅用于日志与测试断言。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnlockScannerSummary {
    /// 本轮捞到的候选条数，受批次上限约束。
    pub scanned: u32,
    pub released: u32,
    /// 因矿工费未缴清而滞留的批次数，主要来自末尾的全量统计而非本轮候选。
    pub blocked_fee: u32,
    /// 因状态在两次读取之间发生变化而幂等跳过的条数。
    pub skipped: u32,
}

impl UnlockScannerSummary {
    /// 构造四项计数全为零的初始汇总，每轮扫描开始时使用。
    /// 该结构只用于日志与测试断言，不落库，因此计数丢失不影响解禁本身的正确性。
    fn empty() -> Self {
        Self {
            scanned: 0,
            released: 0,
            blocked_fee: 0,
            skipped: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockPositionStatus {
    Active,
    Released,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockScanPosition {
    pub id: String,
    pub unlock_at: DateTime<Utc>,
    pub status: LockPositionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockReleaseEvent {
    user_id: u64,
    unlock_id: String,
    lock_position_id: u64,
    asset_id: u64,
    released_amount: BigDecimal,
}

/// 从内存快照中过滤已到期且仍 active 的锁仓位置；不检查费用或数据库并发状态。
/// 到期判定用小于等于，解禁时刻恰好等于当前时间的批次即视为可解禁，不再等下一轮。
/// 已释放与已取消的批次被排除在外，这是内存侧的纯筛选，返回的是对输入切片的借用而非拷贝。
/// 真正的解禁仍须经数据库加锁复核，本函数的结论不能替代那次校验。
pub fn due_unlock_positions(
    positions: &[UnlockScanPosition],
    now: DateTime<Utc>,
) -> Vec<&UnlockScanPosition> {
    positions
        .iter()
        .filter(|position| {
            position.status == LockPositionStatus::Active && position.unlock_at <= now
        })
        .collect()
}

/// 从应用状态取得 MySQL 与可选进程内事件 hub 后释放至多 1..=100 条到期记录；缺少数据库时在扫描前失败。
/// 资金和账本由逐项事务提交，私有事件仅在提交后尽力广播，hub 缺失不影响解禁结果。
/// 事件通道是可选依赖，未配置时解禁照常完成，只是没有实时推送，前端需靠查询接口感知到账。
pub async fn run_once(
    state: &AppState,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<UnlockScannerSummary> {
    let pool = state.mysql.as_ref().ok_or_else(|| {
        AppError::Internal("mysql pool is required for unlock scanner".to_owned())
    })?;
    release_due_unlock_positions_with_broadcast(
        pool,
        state.event_broadcast_hub.as_ref(),
        now,
        limit,
    )
    .await
}

/// 在显式 MySQL 依赖上释放同一有限批次但禁用进程内广播；资产/钱包/解禁/锁仓锁序、资金守恒与终态幂等规则不变。
pub async fn release_due_unlock_positions(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<UnlockScannerSummary> {
    release_due_unlock_positions_with_broadcast(pool, None, now, limit).await
}

/// 按到期时间扫描至多 `limit` 收敛后的 100 条可释放记录，并复用共享事务按资产、钱包、解禁和锁仓取锁，将 locked→available 与账本原子提交。
/// 未支付费用另行计数，状态变化或余额条件不符幂等跳过；本函数遇到单项数据库错误立即返回，已提交前项不回滚。
/// 私有解禁事件只在对应事务提交后尽力广播；广播不持久化、无订阅者时丢弃，断线客户端不会重放。
pub async fn release_due_unlock_positions_with_broadcast(
    pool: &Pool<MySql>,
    hub: Option<&EventBroadcastHub>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<UnlockScannerSummary> {
    let candidates = scan_due_unlocks(pool, now, unlock_scan_limit(limit)).await?;
    let repository = MySqlNewCoinReadRepository::new(pool.clone());
    let mut summary = UnlockScannerSummary::empty();
    summary.scanned = candidates.len() as u32;
    for candidate in candidates {
        match repository
            .release_due_paid_unlock_at(&candidate.idempotency_key, candidate.user_id, now)
            .await
        {
            Ok(outcome) if outcome.released => {
                summary.released += 1;
                publish_unlock_release_event(
                    hub,
                    &UnlockReleaseEvent {
                        user_id: candidate.user_id,
                        unlock_id: candidate.idempotency_key,
                        lock_position_id: candidate.lock_position_id,
                        asset_id: outcome.asset_id,
                        released_amount: outcome.unlock_quantity,
                    },
                );
            }
            Ok(_) => summary.skipped += 1,
            Err(AppError::Validation(reason)) if reason == UNLOCK_NOT_READY => summary.skipped += 1,
            Err(AppError::NotFound) => summary.skipped += 1,
            Err(error) => return Err(error),
        }
    }
    summary.blocked_fee = count_fee_blocked_unlocks(pool, now).await?;
    Ok(summary)
}

/// 以至少 1 秒间隔持续解禁；周期或单项导致的批次错误只记录并进入下一轮。
/// 解禁终态、剩余锁仓量和账本引用承担跨重启幂等；提交后进程内事件不会因循环重启补发。
/// 间隔取配置值与一秒的较大者，防止配置为零时空转打满数据库连接；本循环不会主动退出。
pub async fn run_loop(state: AppState, interval_seconds: u64, limit: u32) -> AppResult<()> {
    let mut ticker = interval(Duration::from_secs(interval_seconds.max(1)));

    loop {
        ticker.tick().await;
        match run_once(&state, Utc::now(), limit).await {
            Ok(summary) => info!(
                scanned = summary.scanned,
                released = summary.released,
                blocked_fee = summary.blocked_fee,
                skipped = summary.skipped,
                "解禁扫描周期完成"
            ),
            Err(error) => error!(%error, "解禁扫描周期失败"),
        }
    }
}

/// 向该用户的私有频道广播一条解禁到账通知，只能在对应事务提交成功之后调用。
/// hub 缺省时静默跳过，因此纯数据库入口下解禁照常完成，只是没有实时推送。
/// 消息为尽力投递：不落库、无订阅者时直接丢弃、断线客户端重连后也不会补发，
/// 前端必须以查询接口为准，不能把这条推送当作解禁是否成功的唯一依据。
/// 载荷中同时给出解禁标识与幂等键、数量与释放量两组同值字段，是为兼容不同版本客户端的取值习惯。
fn publish_unlock_release_event(hub: Option<&EventBroadcastHub>, event: &UnlockReleaseEvent) {
    if let Some(hub) = hub {
        hub.publish(EventBroadcastMessage::private_user(
            event.user_id,
            json!({
                "type": "new_coin.unlock.released",
                "unlock_id": event.unlock_id,
                "unlock_idempotency_key": event.unlock_id,
                "lock_position_id": event.lock_position_id,
                "asset_id": event.asset_id,
                "released_amount": event.released_amount,
                "unlock_quantity": event.released_amount,
                "released": true,
                "status": "released",
            })
            .to_string(),
        ));
    }
}

/// 把配置的每轮扫描条数收敛到 1 到 100，作为候选查询的硬上限。
/// 每个候选都要独占一次钱包行锁事务，上限过大会让单轮长时间持锁并拖慢用户侧的资金操作；
/// 下界为一保证配置写零时任务不至于空转，超出部分只夹断而不报错。
fn unlock_scan_limit(limit: u32) -> u32 {
    limit.clamp(1, 100)
}

#[cfg(test)]
#[path = "../../tests/unit_src/src_workers_unlock_scanner_tests.rs"]
mod tests;
