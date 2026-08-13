//! 新币锁仓解禁扫描后台任务。
//!
//! 新币认购形成的持仓按批次锁定在钱包 locked 桶中，每个批次对应一条解禁记录并带各自的解禁时刻。
//! 本 worker 定时扫描已到解禁时刻且仍待处理的批次，逐条在独立事务中把数量从 locked 转入 available，
//! 同时扣减锁仓剩余量、把解禁记录置为终态并写入成对的钱包账本流水。
//!
//! 解禁矿工费不在此计算与收取：费率规则与按解禁市值或解禁收益计费的口径属于新币上下文，
//! 本任务只检查费用开关与缴费状态这道闸门，开启收费但尚未缴清的批次不动资金，仅计入阻塞计数。
//!
//! 幂等与重入保护分三层：候选查询与加锁读取都要求解禁记录仍为待处理且锁仓仍为启用；
//! 锁仓、解禁记录、钱包三条更新语句各自带条件并逐一核对受影响行数，任一不匹配即整笔回滚跳过；
//! 账本引用使用解禁记录自带的幂等键，重复解禁不会产生第二组流水。
//! 事件在事务提交后才尽力广播，不持久化也不重放。

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
use tracing::{error, info, warn};

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

#[derive(Debug, sqlx::FromRow)]
struct DueUnlockCandidate {
    unlock_id: u64,
}

#[derive(Debug, sqlx::FromRow)]
struct ReleasableUnlockRow {
    unlock_id: u64,
    user_id: u64,
    asset_id: u64,
    lock_position_id: u64,
    unlock_quantity: BigDecimal,
    unlock_fee_enabled: bool,
    fee_paid_status: String,
    idempotency_key: String,
    remaining_amount: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct WalletBalanceRow {
    available: BigDecimal,
    frozen: BigDecimal,
    locked: BigDecimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockReleaseEvent {
    user_id: u64,
    unlock_id: String,
    lock_position_id: u64,
    asset_id: u64,
    released_amount: BigDecimal,
}

/// 单条解禁的处理结果，三种取值都表示事务已收敛，只有释放分支真正动过资金。
#[derive(Debug, Clone, PartialEq, Eq)]
enum UnlockReleaseOutcome {
    Released(UnlockReleaseEvent),
    /// 矿工费未缴清，事务已回滚且未改动任何余额。
    FeeBlocked,
    /// 条件在加锁复核时已不成立，按幂等跳过。
    Skipped,
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

/// 在显式 MySQL 依赖上释放同一有限批次但禁用进程内广播；解禁/锁仓/钱包锁序、资金守恒与终态幂等规则不变。
pub async fn release_due_unlock_positions(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<UnlockScannerSummary> {
    release_due_unlock_positions_with_broadcast(pool, None, now, limit).await
}

/// 按到期时间扫描至多 `limit` 收敛后的 100 条可释放记录，并逐项锁定解禁记录、锁仓和钱包，将 locked→available 与账本原子提交。
/// 未支付费用另行计数，状态变化或余额条件不符幂等跳过；本函数遇到单项数据库错误立即返回，已提交前项不回滚。
/// 私有解禁事件只在对应事务提交后尽力广播；广播不持久化、无订阅者时丢弃，断线客户端不会重放。
pub async fn release_due_unlock_positions_with_broadcast(
    pool: &Pool<MySql>,
    hub: Option<&EventBroadcastHub>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<UnlockScannerSummary> {
    let candidates = due_unlock_candidates(pool, now, limit).await?;
    let mut summary = UnlockScannerSummary::empty();
    summary.scanned = candidates.len() as u32;

    for candidate in candidates {
        match release_due_unlock_by_id(pool, candidate.unlock_id, now).await? {
            UnlockReleaseOutcome::Released(event) => {
                summary.released += 1;
                publish_unlock_release_event(hub, &event);
            }
            UnlockReleaseOutcome::FeeBlocked => summary.blocked_fee += 1,
            UnlockReleaseOutcome::Skipped => summary.skipped += 1,
        }
    }

    summary.blocked_fee += count_fee_blocked_due_unlocks(pool, now).await?;
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

/// 捞取本轮可尝试解禁的候选批次，只返回解禁记录主键，明细留待逐项加锁时重读。
/// 入选需同时满足六项：解禁记录仍待处理、与锁仓的用户和资产一致、解禁数量为正、
/// 锁仓仍处于启用状态且已过解禁时刻、锁仓剩余量足以覆盖本次解禁数量。
/// 第七项是费用闸门，未开启收费或已缴清、无需缴费的批次才进入候选，欠费批次在此被直接排除。
/// 排序先按解禁时刻升序再按记录主键，保证积压最久的批次优先出队且同刻批次次序稳定。
/// 查询不加锁也不改状态，因此候选在真正处理前可能已被其他路径改动，逐项事务会再次核验。
async fn due_unlock_candidates(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<Vec<DueUnlockCandidate>> {
    sqlx::query_as::<_, DueUnlockCandidate>(
        r#"SELECT unlocks.id AS unlock_id
           FROM asset_unlock_records unlocks
           INNER JOIN asset_lock_positions positions ON positions.id = unlocks.lock_position_id
           WHERE unlocks.status = 'pending'
             AND unlocks.user_id = positions.user_id
             AND unlocks.asset_id = positions.asset_id
             AND unlocks.unlock_quantity > 0
             AND positions.status = 'active'
             AND positions.unlock_at <= ?
             AND positions.remaining_amount >= unlocks.unlock_quantity
             AND (unlocks.unlock_fee_enabled = false OR unlocks.fee_paid_status IN ('paid', 'not_required'))
           ORDER BY positions.unlock_at ASC, unlocks.id ASC
           LIMIT ?"#,
    )
    .bind(now.naive_utc())
    .bind(unlock_scan_limit(limit) as i64)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

/// 统计当前已到期、其余条件均满足、仅因解禁矿工费未缴清而滞留的批次数量，用于运营侧观察欠费积压。
/// 过滤条件与候选查询完全对称，只把费用闸门反转成开启收费且缴费状态既非已付也非无需支付。
/// 这里刻意不受本轮批次上限约束，统计的是全量滞留数而非本轮扫描到的部分，因此可能远大于扫描条数。
/// 结果只并入汇总计数，不改任何状态；计数超出 u32 上界时饱和到最大值而非报错。
async fn count_fee_blocked_due_unlocks(pool: &Pool<MySql>, now: DateTime<Utc>) -> AppResult<u32> {
    let (blocked,): (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*)
           FROM asset_unlock_records unlocks
           INNER JOIN asset_lock_positions positions ON positions.id = unlocks.lock_position_id
           WHERE unlocks.status = 'pending'
             AND unlocks.user_id = positions.user_id
             AND unlocks.asset_id = positions.asset_id
             AND unlocks.unlock_quantity > 0
             AND positions.status = 'active'
             AND positions.unlock_at <= ?
             AND positions.remaining_amount >= unlocks.unlock_quantity
             AND unlocks.unlock_fee_enabled = true
             AND unlocks.fee_paid_status NOT IN ('paid', 'not_required')"#,
    )
    .bind(now.naive_utc())
    .fetch_one(pool)
    .await?;
    Ok(blocked.try_into().unwrap_or(u32::MAX))
}

/// 在独立事务中释放一条到期解禁：按解禁记录/锁仓条件锁行，再锁钱包并将数量从 locked 等额转入 available。
/// 费用未满足时只返回阻塞，不动资金；状态已变化时幂等跳过。钱包双桶、账本、剩余锁仓量和解禁终态必须原子提交。
/// 锁序固定为先联表锁定解禁记录与锁仓，再锁钱包账户行，全流程保持该顺序以免与其他资金路径互相死锁。
/// 加锁后重新核验全部前置条件，条件不再成立即回滚并按跳过处理，这构成重入保护的第二层。
/// 锁仓、解禁记录、钱包三条更新都带条件并逐一核对受影响行数必须为一，任一不匹配立即回滚，是第三层保护。
/// 钱包账户缺失或 locked 余额不足属于数据面异常，按校验错误上抛并终止整轮批次，而不是跳过这一条继续。
/// 剩余锁仓量减到零时批次状态转为已释放，否则保持启用等待后续批次。
/// 账本以解禁记录自带的幂等键为引用，一次写入 locked 扣减与 available 增加两条腿，重放不会产生第二组流水。
/// 事件对象在提交前构造但只在提交成功后返回，调用方据此保证不会为未落库的解禁发出通知。
async fn release_due_unlock_by_id(
    pool: &Pool<MySql>,
    unlock_id: u64,
    now: DateTime<Utc>,
) -> AppResult<UnlockReleaseOutcome> {
    let mut tx = pool.begin().await?;
    let Some(row) = sqlx::query_as::<_, ReleasableUnlockRow>(
        r#"SELECT unlocks.id AS unlock_id,
                  unlocks.user_id,
                  unlocks.asset_id,
                  unlocks.lock_position_id,
                  unlocks.unlock_quantity,
                  unlocks.unlock_fee_enabled,
                  unlocks.fee_paid_status,
                  unlocks.idempotency_key,
                  positions.remaining_amount
           FROM asset_unlock_records unlocks
           INNER JOIN asset_lock_positions positions ON positions.id = unlocks.lock_position_id
           WHERE unlocks.id = ?
             AND unlocks.status = 'pending'
             AND unlocks.user_id = positions.user_id
             AND unlocks.asset_id = positions.asset_id
             AND unlocks.unlock_quantity > 0
             AND positions.status = 'active'
             AND positions.unlock_at <= ?
             AND positions.remaining_amount >= unlocks.unlock_quantity
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(unlock_id)
    .bind(now.naive_utc())
    .fetch_optional(&mut *tx)
    .await?
    else {
        tx.rollback().await?;
        return Ok(UnlockReleaseOutcome::Skipped);
    };

    if requires_fee_payment(row.unlock_fee_enabled, &row.fee_paid_status) {
        tx.rollback().await?;
        return Ok(UnlockReleaseOutcome::FeeBlocked);
    }

    let Some(wallet) = sqlx::query_as::<_, WalletBalanceRow>(
        r#"SELECT available, frozen, locked
           FROM wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(row.user_id)
    .bind(row.asset_id)
    .fetch_optional(&mut *tx)
    .await?
    else {
        return Err(AppError::Validation(
            "wallet account is required before unlock scanner release".to_owned(),
        ));
    };

    if wallet.locked < row.unlock_quantity {
        return Err(AppError::Validation(
            "wallet locked balance is insufficient for unlock scanner release".to_owned(),
        ));
    }

    let available_after = wallet.available.clone() + row.unlock_quantity.clone();
    let locked_after = wallet.locked.clone() - row.unlock_quantity.clone();
    let remaining_after = row.remaining_amount - row.unlock_quantity.clone();
    let lock_status = if remaining_after == 0 {
        "released"
    } else {
        "active"
    };

    let lock_update = sqlx::query(
        r#"UPDATE asset_lock_positions
           SET released_amount = released_amount + ?,
               remaining_amount = ?,
               status = ?
           WHERE id = ? AND remaining_amount >= ?"#,
    )
    .bind(&row.unlock_quantity)
    .bind(&remaining_after)
    .bind(lock_status)
    .bind(row.lock_position_id)
    .bind(&row.unlock_quantity)
    .execute(&mut *tx)
    .await?;
    if lock_update.rows_affected() != 1 {
        tx.rollback().await?;
        warn!(unlock_id = row.unlock_id, "解禁扫描跳过过期锁仓更新");
        return Ok(UnlockReleaseOutcome::Skipped);
    }

    let unlock_update = sqlx::query(
        "UPDATE asset_unlock_records SET status = 'released' WHERE id = ? AND status = 'pending'",
    )
    .bind(row.unlock_id)
    .execute(&mut *tx)
    .await?;
    if unlock_update.rows_affected() != 1 {
        tx.rollback().await?;
        warn!(unlock_id = row.unlock_id, "解禁扫描跳过过期解禁记录更新");
        return Ok(UnlockReleaseOutcome::Skipped);
    }

    let wallet_update = sqlx::query(
        "UPDATE wallet_accounts SET available = ?, locked = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&available_after)
    .bind(&locked_after)
    .bind(row.user_id)
    .bind(row.asset_id)
    .execute(&mut *tx)
    .await?;
    if wallet_update.rows_affected() != 1 {
        tx.rollback().await?;
        warn!(unlock_id = row.unlock_id, "解禁扫描跳过缺失钱包更新");
        return Ok(UnlockReleaseOutcome::Skipped);
    }

    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, 'new_coin_unlock_release', ?, 'locked', ?, ?, ?, ?, 'new_coin_unlock', ?),
                  (?, ?, 'new_coin_unlock_release', ?, 'available', ?, ?, ?, ?, 'new_coin_unlock', ?)"#,
    )
    .bind(row.user_id)
    .bind(row.asset_id)
    .bind(-row.unlock_quantity.clone())
    .bind(&locked_after)
    .bind(&available_after)
    .bind(&wallet.frozen)
    .bind(&locked_after)
    .bind(&row.idempotency_key)
    .bind(row.user_id)
    .bind(row.asset_id)
    .bind(&row.unlock_quantity)
    .bind(&available_after)
    .bind(&available_after)
    .bind(&wallet.frozen)
    .bind(&locked_after)
    .bind(&row.idempotency_key)
    .execute(&mut *tx)
    .await?;

    let event = UnlockReleaseEvent {
        user_id: row.user_id,
        unlock_id: row.idempotency_key,
        lock_position_id: row.lock_position_id,
        asset_id: row.asset_id,
        released_amount: row.unlock_quantity,
    };
    tx.commit().await?;
    Ok(UnlockReleaseOutcome::Released(event))
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

/// 判定该批次是否还卡在解禁矿工费闸门上：仅当收费开关打开且缴费状态既非已付也非无需支付时成立。
/// 收费开关关闭时无论缴费状态为何都放行，这样历史遗留的状态值不会阻塞免费批次。
/// 本判定不计算费用金额也不发起扣款，费率与计费口径由新币上下文在解禁申请阶段处理。
fn requires_fee_payment(unlock_fee_enabled: bool, fee_paid_status: &str) -> bool {
    unlock_fee_enabled && !matches!(fee_paid_status, "paid" | "not_required")
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
