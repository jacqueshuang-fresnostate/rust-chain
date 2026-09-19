//! 杠杆借款利息计提后台任务。
//!
//! 周期性扫描已成交且有借款的持仓，按「上次计息点到当前时刻的完整小时数」累加利息债务。
//! 计息口径是单利：借款额乘小时利率再乘完整小时数，新增利息按保证金资产精度向零量化。
//! 26 位前向余数与检查点同事务保存，即使本批新增资产单位为零也推进，不足一小时不计不预扣。
//! 计息时间戳只推进到「上次计息点 + 已计费整小时数」，因此不足一小时的零头会顺延到下个窗口而不是被丢掉。
//! `interest_accrued_at` 既是计息起点也是跨重启检查点，与利息增量在同一事务内提交，
//! 因此进程崩溃重启后只会补上尚未计提的完整小时，不会重复收费也不会漏收。
//! 本 worker 只增加仓位上的债务数字，不扣任何钱包余额、不写资金流水、不发布 WebSocket 事件；
//! 这笔债务要到平仓或强平时才被真正消费，从权益里扣除。
//! 每个仓位独立开事务并加行锁，单笔失败只计数并继续，不影响同批其他仓位。

use crate::{
    error::{AppError, AppResult},
    modules::margin::infrastructure::{
        ensure_and_lock_cross_margin_account, require_active_cross_margin_account,
    },
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use sqlx::{MySql, Pool, Transaction};
use std::env;
use tokio::time::{Duration, interval};
use tracing::{error, info, warn};

/// 利息计提任务的入口句柄，本身无状态，仅用于把 `run_once` 暴露成方法形式。
pub struct MarginInterestWorker;

/// 利息计提任务的运行参数，全部来自环境变量并在启动时读取一次。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarginInterestWorkerConfig {
    /// 是否启用该后台任务，关闭后持仓将不再累积利息债务。
    pub enabled: bool,
    /// 两轮计提之间的间隔秒数，实际执行时至少为一秒。
    pub interval_seconds: u64,
    /// 单轮成功计提的仓位数上限，运行时会被夹到 1 到 100。
    pub batch_limit: u32,
}

impl MarginInterestWorkerConfig {
    /// 读取杠杆计息开关、周期与批量环境配置；默认启用、周期 60 秒、批量 100，缺失或不可解析值回落到默认值。
    pub fn from_env() -> Self {
        Self {
            enabled: env_bool("MARGIN_INTEREST_ENABLED", true),
            interval_seconds: env_u64("MARGIN_INTEREST_INTERVAL_SECONDS", 60),
            batch_limit: env_u32("MARGIN_INTEREST_BATCH_LIMIT", 100),
        }
    }
}

impl MarginInterestWorker {
    /// 执行一轮杠杆利息计提；成功上限收敛到 1..=100、候选最多 500，每个仓位独立锁定并仅计完整小时。
    /// 状态变化或不足一小时幂等跳过，单项失败继续；本入口不扣钱包、不广播事件，债务与计息时间戳在单项事务内共同提交。
    pub async fn run_once(
        &self,
        pool: &Pool<MySql>,
        now: DateTime<Utc>,
        limit: u32,
    ) -> AppResult<MarginInterestSummary> {
        run_once_with_dependencies(pool, now, limit).await
    }
}

/// 单轮计提的结果统计，四个计数之和不一定等于候选总数，因为达到上限会提前退出循环。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MarginInterestSummary {
    /// 本轮实际尝试处理的仓位数。
    pub scanned: u32,
    /// 写入计息检查点的仓位数，包含只积累余数的零单位增量，达到上限停止本轮。
    pub accrued: u32,
    /// 因状态变化、无借款或不足一小时而幂等跳过的仓位数。
    pub skipped: u32,
    /// 处理过程中抛错的仓位数，已记录告警日志且不影响其他仓位。
    pub failed: u32,
}

/// 候选查询只取仓位主键，实际字段在逐笔加锁时重新读取，避免使用可能已过期的快照。
#[derive(Debug, sqlx::FromRow)]
struct MarginInterestCandidate {
    position_id: u64,
    user_id: u64,
    margin_asset: u64,
    margin_mode: String,
}

/// 计提事务内加锁读到的仓位与产品联表快照，计息判定与写入全部基于这份数据。
#[derive(Debug, sqlx::FromRow)]
struct LockedMarginPosition {
    /// 仓位主键，用于带状态条件的原子更新。
    id: u64,
    /// 所属用户，全仓模式下用于重算账户级利息聚合。
    user_id: u64,
    /// 保证金币种，与用户一起定位全仓账户。
    margin_asset: u64,
    /// 保证金模式，只有 cross 才需要额外刷新账户级聚合。
    margin_mode: String,
    /// 借款额，是计息基数，非正时直接跳过。
    borrowed_amount: BigDecimal,
    /// 服务端权威入场价，为 NULL 表示限价挂单尚未成交，严禁计息。
    entry_price: Option<BigDecimal>,
    /// 加锁瞬间的仓位状态，非 opened 则放弃本次计提。
    status: String,
}

/// 单个仓位的计提结果，只区分「写入了利息」和「幂等跳过」，错误走 Err 分支不在此枚举内。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarginInterestOutcome {
    Accrued,
    Skipped,
}

/// 扫描 opened 仓位，成功计提上限为 `limit` 收敛到 1..=100，候选最多放大十倍且不超过 500，以越过并发失效项。
/// 每项独立事务锁定仓位，按截至 `now` 的完整小时原子提交利息增量与 `interest_accrued_at`；状态变化或不足一小时幂等跳过，失败计数后继续。
/// 该 worker 只增加持久化债务，不直接扣钱包、写资金账本或发布事件；平仓/强平在后续事务中消费该利息快照。
pub async fn run_once_with_dependencies(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<MarginInterestSummary> {
    let candidates = fetch_interest_candidates(pool, margin_interest_scan_limit(limit)).await?;
    let mut summary = MarginInterestSummary::default();

    for candidate in candidates {
        if summary.accrued >= margin_interest_limit(limit) {
            break;
        }
        summary.scanned += 1;
        match accrue_position_interest(pool, &candidate, now).await {
            Ok(MarginInterestOutcome::Accrued) => summary.accrued += 1,
            Ok(MarginInterestOutcome::Skipped) => summary.skipped += 1,
            Err(error) => {
                summary.failed += 1;
                warn!(position_id = candidate.position_id, %error, "杠杆利息计提失败");
            }
        }
    }

    Ok(summary)
}

/// 以至少 1 秒间隔持续计提利息；候选查询等周期故障只记录并进入下一轮，单项失败不会终止循环。
/// `interest_accrued_at` 是跨重启检查点，确保恢复后只补尚未计提的完整小时；循环不发布提交后事件。
pub async fn run_loop(pool: Pool<MySql>, interval_seconds: u64, limit: u32) -> AppResult<()> {
    let duration = Duration::from_secs(interval_seconds.max(1));
    if std::time::Instant::now().checked_add(duration).is_none() {
        return Err(AppError::Validation(
            "margin interest interval overflow".to_owned(),
        ));
    }
    let mut ticker = interval(duration);

    loop {
        ticker.tick().await;
        match run_once_with_dependencies(&pool, Utc::now(), limit).await {
            Ok(summary) => info!(
                scanned = summary.scanned,
                accrued = summary.accrued,
                skipped = summary.skipped,
                failed = summary.failed,
                "杠杆利息周期完成"
            ),
            Err(error) => error!(%error, "杠杆利息周期失败"),
        }
    }
}

/// 挑选本轮待计息的候选仓位主键，条件是已有入场价、状态 opened、借款额为正且快照小时利率为正。
/// `entry_price IS NOT NULL` 是挂单与真实持仓的资金边界；其余条件把免息产品和一倍杠杆仓位挡在事务之外。
/// 利率取自仓位自身在借款开始时固化的快照，不再联产品表，因此管理员改配产品不会改变存量仓位的计费口径。
/// 排序以 `interest_accrued_at` 升序打头，让最久未计息的仓位优先处理，形成天然的公平轮转；
/// 再以开仓时间和主键兜底，保证排序完全确定，不会因并列值导致某些仓位长期排在后面被饿死。
/// 上限在这里再夹一次到 1 到 500，即便调用方传入异常值也不会拉出超大结果集。
/// 只读查询不加锁，读到的候选随后可能已被平仓，逐笔加锁时会重新判定并跳过。
async fn fetch_interest_candidates(
    pool: &Pool<MySql>,
    limit: u32,
) -> AppResult<Vec<MarginInterestCandidate>> {
    sqlx::query_as::<_, MarginInterestCandidate>(
        r#"SELECT positions.id AS position_id, positions.user_id,
                  positions.margin_asset, positions.margin_mode
           FROM margin_positions positions
           WHERE positions.status = 'opened'
             AND positions.entry_price IS NOT NULL
             AND positions.borrowed_amount > 0
             AND positions.hourly_interest_rate > 0
           ORDER BY positions.interest_accrued_at ASC, positions.opened_at ASC, positions.id ASC
           LIMIT ?"#,
    )
    .bind(i64::from(limit.clamp(1, 500)))
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

/// 在独立事务中锁定一个 opened 仓位，按上次计息点到 `now` 的完整小时数累加借款利息。
/// 不足完整小时、状态变化或无借款仓位幂等跳过；利息余额与计息时间戳同事务提交，避免崩溃重放重复收费。
///
/// 仓位不存在、未成交、无借款或未满完整小时都会回滚并跳过；新增资产单位为零时仍提交余数。
/// 状态和利息检查点写入由共享适配器再次在锁内核对，容量失败回滚且不推进时间，不能隐式舍入。
/// `interest_accrued_at` 写成「上次计息点加已计费整小时数」而不是 now，因此不足一小时的零头会顺延到下个窗口，
/// 多轮累计下来计费小时数与真实经过时长一致；若写成 now 则每轮都会丢掉零头，长期系统性少收利息。
/// 全仓仓位在同一事务内额外重算账户级利息聚合并递增版本号，让风险快照和强平读到一致的账户视图。
async fn accrue_position_interest(
    pool: &Pool<MySql>,
    candidate: &MarginInterestCandidate,
    now: DateTime<Utc>,
) -> AppResult<MarginInterestOutcome> {
    let mut tx = pool.begin().await?;
    let cross_account = if candidate.margin_mode == "cross" {
        let account = ensure_and_lock_cross_margin_account(
            &mut tx,
            candidate.user_id,
            candidate.margin_asset,
        )
        .await?;
        require_active_cross_margin_account(&account)?;
        Some(account)
    } else {
        None
    };
    let Some(position) = lock_position(&mut tx, candidate.position_id).await? else {
        tx.rollback().await?;
        return Ok(MarginInterestOutcome::Skipped);
    };
    if position.user_id != candidate.user_id
        || position.margin_asset != candidate.margin_asset
        || position.margin_mode != candidate.margin_mode
    {
        return Err(AppError::Conflict(
            "margin interest account scope changed concurrently".to_owned(),
        ));
    }
    if position.status != "opened"
        || position.entry_price.is_none()
        || position.borrowed_amount <= 0
    {
        tx.rollback().await?;
        return Ok(MarginInterestOutcome::Skipped);
    }
    if crate::modules::margin::infrastructure::accrue_locked_position_interest(
        &mut tx,
        position.id,
        now,
    )
    .await?
    .is_none()
    {
        tx.rollback().await?;
        return Ok(MarginInterestOutcome::Skipped);
    }
    if position.margin_mode == "cross" {
        // 全仓利息按账户聚合，风险快照和统一强平都读取这个聚合值。
        let account = cross_account.as_ref().ok_or_else(|| {
            AppError::Conflict("cross margin account lock is required for interest".to_owned())
        })?;
        account.version.checked_add(1).ok_or_else(|| {
            AppError::Conflict("cross margin interest version exhausted".to_owned())
        })?;
        let total_interest = sqlx::query_scalar::<_, BigDecimal>(
            r#"SELECT COALESCE(SUM(interest_amount), 0) FROM margin_positions
               WHERE user_id = ? AND margin_asset = ? AND margin_mode = 'cross'
                 AND status = 'opened' AND entry_price IS NOT NULL"#,
        )
        .bind(position.user_id)
        .bind(position.margin_asset)
        .fetch_one(&mut *tx)
        .await?;
        crate::numeric::ensure_amount_storage(&total_interest, "cross margin accrued interest")?;
        let account_update = sqlx::query(
            r#"UPDATE margin_cross_accounts
               SET last_interest_amount = ?,
                   version = version + 1
               WHERE user_id = ? AND margin_asset = ? AND version = ?"#,
        )
        .bind(&total_interest)
        .bind(position.user_id)
        .bind(position.margin_asset)
        .bind(account.version)
        .execute(&mut *tx)
        .await?;
        if account_update.rows_affected() != 1 {
            return Err(AppError::Conflict(
                "cross margin account changed during interest accrual".to_owned(),
            ));
        }
    }
    tx.commit().await?;
    Ok(MarginInterestOutcome::Accrued)
}

/// 对目标仓位加 FOR UPDATE 行锁并复核候选账户归属；利率和余数由后续共享适配器在同一锁下读取。
/// 只按主键定位、不带状态条件，因此已平仓的仓位也能被读到，状态判定交给调用方处理。
/// 加锁把余额、上次计息点和状态固定在同一版本上，防止与并发的平仓或强平交叉写入。
/// 利率取仓位在借款开始时写入的快照，不再联产品表：管理员改配只影响之后新开的仓位，
/// 不会追溯改写存量仓位已经发生或尚未计提的窗口，与借贷、理财的费率快照口径一致。
/// 仓位不存在时返回 None，调用方回滚后按跳过计数。
async fn lock_position(
    tx: &mut Transaction<'_, MySql>,
    position_id: u64,
) -> AppResult<Option<LockedMarginPosition>> {
    sqlx::query_as::<_, LockedMarginPosition>(
        r#"SELECT positions.id, positions.user_id, positions.margin_asset, positions.margin_mode,
                  positions.borrowed_amount, positions.entry_price, positions.status
           FROM margin_positions positions
           WHERE positions.id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(position_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

/// 把单轮成功计提数上限夹到 1 到 100，即便配置传入零或极大值也保证每轮至少推进一笔、至多一百笔。
/// 该上限限制的是成功写入数而非扫描数，跳过和失败的仓位不占用配额。
fn margin_interest_limit(limit: u32) -> u32 {
    limit.clamp(1, 100)
}

/// 推导候选查询的抓取上限，在成功上限基础上放大十倍再夹到 1 到 500。
/// 放大是因为候选里必然混有不足一小时或已被平仓的仓位，只抓等量候选会让每轮实际计提数远低于配额。
/// 用 `saturating_mul` 避免极大配置值相乘溢出，最终仍被 500 的硬上限兜住，防止单轮拉出过大结果集。
fn margin_interest_scan_limit(limit: u32) -> u32 {
    margin_interest_limit(limit)
        .saturating_mul(10)
        .clamp(1, 500)
}

/// 读取布尔型环境变量，只接受 Rust 的 `true` 与 `false` 字面量，其余写法按无法解析处理。
/// 变量缺失和解析失败都静默回落到默认值，因此配置写错不会阻止服务启动，只会以默认行为运行。
fn env_bool(key: &str, default: bool) -> bool {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<bool>().ok())
        .unwrap_or(default)
}

/// 读取无符号 64 位整型环境变量，用于计提周期秒数，负值和非数字都会解析失败并回落默认。
/// 超出有符号时长或单调时钟范围的配置回落默认；零值仍由运行循环兼容为至少一秒。
fn env_u64(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| {
            i64::try_from(*seconds)
                .ok()
                .and_then(chrono::TimeDelta::try_seconds)
                .is_some()
                && std::time::Instant::now()
                    .checked_add(Duration::from_secs(*seconds))
                    .is_some()
        })
        .unwrap_or(default)
}

/// 读取无符号 32 位整型环境变量，用于单轮批量上限，解析失败或缺失时回落默认值。
/// 同样不在此处夹范围，真正的 1 到 100 收敛发生在每轮执行时，配置错误不会导致越界扫描。
fn env_u32(key: &str, default: u32) -> u32 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(default)
}

#[cfg(test)]
#[path = "../../tests/unit_src/src_workers_margin_interest_tests.rs"]
mod tests;
