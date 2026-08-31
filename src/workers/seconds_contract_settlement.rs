//! 秒合约事件时点结算 worker。
//!
//! 结算价格只从 MySQL 的 append-only `market_price_ticks` 历史读取。明确规则是选择
//! `[expires_at, expires_at + 5s)` 内按事件时间排序的第一条 ticker；窗口关闭前保持 `opened`，历史缺失则在可配置上限内重试。
//! 超过上限仍无快照时，订单与一条追加式异常证据原子转入 `manual_review`，绝不使用处理时 Redis 最新价、猜测价格或修改钱包。
//! 选中的行主键、来源、观察时间、generation 与源版本和订单终态在同一事务固化，处理时机与重启不改变选价。

use crate::{
    error::{AppError, AppResult},
    modules::{
        events::{EventBroadcastHub, EventBroadcastMessage},
        seconds_contract::{
            infrastructure,
            repository::{
                SecondsContractSettlementExceptionWrite, SecondsContractWalletLedgerWrite,
            },
            service::{
                SETTLEMENT_PRICE_WINDOW_SECONDS as EVENT_PRICE_WINDOW_SECONDS,
                seconds_contract_payout_amount, settlement_result_from_prices,
            },
        },
    },
    state::AppState,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use redis::aio::ConnectionManager;
use serde_json::json;
use sqlx::{MySql, Pool};
use std::env;
use tokio::time::{Duration, interval};
use tracing::{error, info, warn};

/// 事件时点价格窗口长度；右边界不包含在可选集合中。
pub const SETTLEMENT_PRICE_WINDOW_SECONDS: i64 = EVENT_PRICE_WINDOW_SECONDS;

/// 缺少事件时间快照时默认最多等待五分钟，超时后转人工审核。
pub const DEFAULT_MAX_SNAPSHOT_WAIT_SECONDS: u64 = 300;

const MAX_CONFIGURED_SNAPSHOT_WAIT_SECONDS: u64 = 86_400;
const MISSING_SETTLEMENT_SNAPSHOT_FAILURE_CODE: &str = "missing_settlement_snapshot";

/// 秒合约结算 worker 的无状态入口。
pub struct SecondsContractSettlementWorker;

/// 结算 worker 的启动参数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecondsContractSettlementWorkerConfig {
    /// 是否启用自动结算。
    pub enabled: bool,
    /// 扫描周期秒数。
    pub interval_seconds: u64,
    /// 单轮订单上限。
    pub batch_limit: u32,
    /// 事件窗口关闭后继续等待历史快照的最大秒数。
    pub max_snapshot_wait_seconds: u64,
}

impl SecondsContractSettlementWorkerConfig {
    /// 从环境变量读取配置，非法值回退到保守默认值。
    pub fn from_env() -> Self {
        Self {
            enabled: env_bool("SECONDS_CONTRACT_SETTLEMENT_ENABLED", true),
            interval_seconds: env_u64("SECONDS_CONTRACT_SETTLEMENT_INTERVAL_SECONDS", 5),
            batch_limit: env_u32("SECONDS_CONTRACT_SETTLEMENT_BATCH_LIMIT", 100),
            max_snapshot_wait_seconds: normalize_max_snapshot_wait_seconds(env_u64(
                "SECONDS_CONTRACT_SETTLEMENT_MAX_SNAPSHOT_WAIT_SECONDS",
                DEFAULT_MAX_SNAPSHOT_WAIT_SECONDS,
            )),
        }
    }
}

impl SecondsContractSettlementWorker {
    /// 执行一轮结算；唯一价格依赖是 MySQL 历史表，Redis 不参与结算判定。
    pub async fn run_once(
        &self,
        state: &AppState,
        now: DateTime<Utc>,
        limit: u32,
    ) -> AppResult<SecondsContractSettlementSummary> {
        run_once(state, now, limit).await
    }
}

/// 单轮结算统计。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SecondsContractSettlementSummary {
    /// 已扫描订单数。
    pub scanned: u32,
    /// 成功结算数。
    pub settled: u32,
    /// 因窗口未关闭、缺历史或并发终态而保持 pending 的数量。
    pub skipped: u32,
    /// 数据或资金异常数量。
    pub failed: u32,
    /// 因快照缺失超过最大等待时长而首次转入人工审核的数量。
    pub manual_review: u32,
}

#[derive(Debug, sqlx::FromRow)]
struct DueSecondsContractOrder {
    order_id: u64,
}

/// 提交后才可广播的结算事件快照。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecondsContractSettlementEvent {
    user_id: u64,
    order_id: u64,
    product_id: u64,
    pair_id: u64,
    stake_asset: u64,
    direction: String,
    stake_amount: BigDecimal,
    payout_amount: BigDecimal,
    settlement_price: BigDecimal,
    settlement_price_tick_id: u64,
    settlement_price_source: String,
    settlement_price_observed_at: DateTime<Utc>,
    settlement_price_generation: u64,
    settlement_price_version: String,
    result: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SettlementOutcome {
    Settled(Box<SecondsContractSettlementEvent>),
    ManualReview,
    Pending,
    Skipped,
}

/// 比较开仓价与结算价得出胜负；保留公开入口供既有调用方和测试复用。
pub fn seconds_contract_settlement_result(
    direction: &str,
    entry_price: &BigDecimal,
    exit_price: &BigDecimal,
) -> AppResult<&'static str> {
    settlement_result_from_prices(direction, entry_price, exit_price)
}

/// 从应用状态取得 MySQL 后执行一轮；未配置 Redis 不影响结算。
pub async fn run_once(
    state: &AppState,
    _now: DateTime<Utc>,
    limit: u32,
) -> AppResult<SecondsContractSettlementSummary> {
    let pool = state.mysql.as_ref().ok_or_else(|| {
        AppError::Internal("mysql pool is required for seconds contract settlement".to_owned())
    })?;
    let now = database_now(pool).await?;
    let max_snapshot_wait_seconds =
        SecondsContractSettlementWorkerConfig::from_env().max_snapshot_wait_seconds;
    run_once_with_broadcast_and_max_wait(
        pool,
        state.event_broadcast_hub.as_ref(),
        now,
        limit,
        max_snapshot_wait_seconds,
    )
    .await
}

/// 在显式 MySQL 池上执行一轮，供集成测试和独立调度复用。
pub async fn run_once_with_pool(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<SecondsContractSettlementSummary> {
    run_once_with_pool_and_max_wait(pool, now, limit, DEFAULT_MAX_SNAPSHOT_WAIT_SECONDS).await
}

/// 在显式 MySQL 池上使用调用方指定的快照最大等待时长执行一轮，供边界与重启集成测试使用。
/// 等待时长夹在 1 秒到 24 小时之间，避免误配为 0 导致窗口一关闭就大量转人工审核。
pub async fn run_once_with_pool_and_max_wait(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    limit: u32,
    max_snapshot_wait_seconds: u64,
) -> AppResult<SecondsContractSettlementSummary> {
    run_once_with_broadcast_and_max_wait(
        pool,
        None,
        now,
        limit,
        normalize_max_snapshot_wait_seconds(max_snapshot_wait_seconds),
    )
    .await
}

/// 兼容旧测试装配签名；Redis 参数被刻意忽略，不能成为结算价格来源。
#[deprecated(note = "use run_once_with_pool; Redis is not a settlement authority")]
pub async fn run_once_with_dependencies(
    pool: &Pool<MySql>,
    _redis: &ConnectionManager,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<SecondsContractSettlementSummary> {
    run_once_with_pool(pool, now, limit).await
}

/// 扫描并逐单结算；每单独立事务，缺历史在默认上限内退避，超龄则转人工审核且不改变资金。
pub async fn run_once_with_broadcast(
    pool: &Pool<MySql>,
    hub: Option<&EventBroadcastHub>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<SecondsContractSettlementSummary> {
    run_once_with_broadcast_and_max_wait(pool, hub, now, limit, DEFAULT_MAX_SNAPSHOT_WAIT_SECONDS)
        .await
}

/// 扫描并逐单结算；每单独立事务，缺历史在上限内退避，超龄则原子转人工审核。
async fn run_once_with_broadcast_and_max_wait(
    pool: &Pool<MySql>,
    hub: Option<&EventBroadcastHub>,
    now: DateTime<Utc>,
    limit: u32,
    max_snapshot_wait_seconds: u64,
) -> AppResult<SecondsContractSettlementSummary> {
    let settlement_limit = seconds_contract_settlement_limit(limit);
    let rows = fetch_due_orders(pool, now, seconds_contract_settlement_scan_limit(limit)).await?;
    let mut summary = SecondsContractSettlementSummary::default();

    for row in rows {
        if summary.settled >= settlement_limit {
            break;
        }
        summary.scanned += 1;
        match settle_order_by_id(pool, row.order_id, now, max_snapshot_wait_seconds).await {
            Ok(SettlementOutcome::Settled(event)) => {
                summary.settled += 1;
                publish_settlement_event(hub, &event);
            }
            Ok(SettlementOutcome::ManualReview) => summary.manual_review += 1,
            Ok(SettlementOutcome::Pending) => {
                summary.skipped += 1;
                reschedule_settlement_attempt(pool, row.order_id, now).await?;
            }
            Ok(SettlementOutcome::Skipped) => summary.skipped += 1,
            Err(error) => {
                summary.failed += 1;
                reschedule_settlement_attempt(pool, row.order_id, now).await?;
                warn!(order_id = row.order_id, %error, "秒合约事件时点结算失败");
            }
        }
    }
    Ok(summary)
}

/// 按固定周期持续执行结算；单轮错误只记录，下一轮继续恢复。
pub async fn run_loop(state: AppState, interval_seconds: u64, limit: u32) -> AppResult<()> {
    let mut ticker = interval(Duration::from_secs(interval_seconds.max(1)));
    loop {
        ticker.tick().await;
        match run_once(&state, Utc::now(), limit).await {
            Ok(summary) => info!(
                scanned = summary.scanned,
                settled = summary.settled,
                manual_review = summary.manual_review,
                skipped = summary.skipped,
                failed = summary.failed,
                "秒合约事件时点结算周期完成"
            ),
            Err(error) => error!(%error, "秒合约事件时点结算周期失败"),
        }
    }
}

/// 生产调度只使用 MySQL UTC 时钟；显式时间仅保留在 `run_once_with_pool` 测试入口。
async fn database_now(pool: &Pool<MySql>) -> AppResult<DateTime<Utc>> {
    let now = sqlx::query_scalar::<_, chrono::NaiveDateTime>("SELECT CURRENT_TIMESTAMP(6)")
        .fetch_one(pool)
        .await?;
    Ok(DateTime::<Utc>::from_naive_utc_and_offset(now, Utc))
}

async fn fetch_due_orders(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<Vec<DueSecondsContractOrder>> {
    sqlx::query_as::<_, DueSecondsContractOrder>(
        r#"SELECT id AS order_id
           FROM seconds_contract_orders
           WHERE status = 'opened'
             AND expires_at <= DATE_SUB(?, INTERVAL 5 SECOND)
             AND (next_settlement_attempt_at IS NULL OR next_settlement_attempt_at <= ?)
           ORDER BY expires_at ASC, id ASC
           LIMIT ?"#,
    )
    .bind(now.naive_utc())
    .bind(now.naive_utc())
    .bind(limit.clamp(1, 500) as i64)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

async fn reschedule_settlement_attempt(
    pool: &Pool<MySql>,
    order_id: u64,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE seconds_contract_orders SET next_settlement_attempt_at = ? WHERE id = ? AND status = 'opened'",
    )
    .bind((now + chrono::TimeDelta::seconds(60)).naive_utc())
    .bind(order_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 在订单锁之后选择事件价格，按订单→钱包固定锁序完成结算。
async fn settle_order_by_id(
    pool: &Pool<MySql>,
    order_id: u64,
    processing_time: DateTime<Utc>,
    max_snapshot_wait_seconds: u64,
) -> AppResult<SettlementOutcome> {
    let mut tx = pool.begin().await?;
    let order = match infrastructure::lock_order_by_id(&mut tx, order_id).await {
        Ok(order) => order,
        Err(AppError::NotFound) => {
            tx.rollback().await?;
            return Ok(SettlementOutcome::Skipped);
        }
        Err(error) => return Err(error),
    };
    if order.status == "settled" {
        tx.commit().await?;
        return Ok(SettlementOutcome::Skipped);
    }
    if order.status != "opened" {
        tx.rollback().await?;
        return Ok(SettlementOutcome::Skipped);
    }
    let window_closes_at =
        order.expires_at + chrono::TimeDelta::seconds(SETTLEMENT_PRICE_WINDOW_SECONDS);
    if processing_time < window_closes_at {
        tx.rollback().await?;
        return Ok(SettlementOutcome::Pending);
    }
    let Some(snapshot) =
        infrastructure::select_settlement_price_snapshot(&mut tx, &order.symbol, order.expires_at)
            .await?
    else {
        let max_wait =
            chrono::TimeDelta::seconds(i64::try_from(max_snapshot_wait_seconds).map_err(|_| {
                AppError::Validation(
                    "seconds contract snapshot wait exceeds supported range".to_owned(),
                )
            })?);
        let manual_review_at = window_closes_at
            .checked_add_signed(max_wait)
            .ok_or_else(|| {
                AppError::Validation(
                    "seconds contract manual review deadline is outside valid range".to_owned(),
                )
            })?;
        if processing_time >= manual_review_at {
            infrastructure::move_order_to_manual_review(
                &mut tx,
                &SecondsContractSettlementExceptionWrite {
                    order_id: order.id,
                    failure_code: MISSING_SETTLEMENT_SNAPSHOT_FAILURE_CODE,
                    detected_at: processing_time,
                    window_start: order.expires_at,
                    window_end: window_closes_at,
                },
            )
            .await?;
            tx.commit().await?;
            return Ok(SettlementOutcome::ManualReview);
        }
        tx.rollback().await?;
        return Ok(SettlementOutcome::Pending);
    };
    let entry_price = order.entry_price.as_ref().ok_or_else(|| {
        AppError::Validation("seconds contract entry price is required for settlement".to_owned())
    })?;
    let result = settlement_result_from_prices(&order.direction, entry_price, &snapshot.price)?;
    let precision = infrastructure::load_asset_precision_scale(&mut tx, order.stake_asset).await?;
    let payout_amount =
        seconds_contract_payout_amount(&order.stake_amount, &order.payout_rate, result, precision);
    if payout_amount > 0 {
        let wallet =
            infrastructure::lock_wallet_row(&mut tx, order.user_id, order.stake_asset).await?;
        let available_after = wallet.available.clone() + payout_amount.clone();
        infrastructure::update_wallet_available(
            &mut tx,
            order.user_id,
            order.stake_asset,
            &available_after,
        )
        .await?;
        infrastructure::insert_wallet_ledger(
            &mut tx,
            SecondsContractWalletLedgerWrite {
                user_id: order.user_id,
                asset_id: order.stake_asset,
                change_type: "seconds_contract_settle_win",
                amount: payout_amount.clone(),
                available_after,
                frozen_after: wallet.frozen,
                locked_after: wallet.locked,
                ref_id: order.id.to_string(),
            },
        )
        .await?;
    }
    infrastructure::mark_order_settled(&mut tx, order.id, result, &snapshot).await?;
    let event = SecondsContractSettlementEvent {
        user_id: order.user_id,
        order_id: order.id,
        product_id: order.product_id,
        pair_id: order.pair_id,
        stake_asset: order.stake_asset,
        direction: order.direction,
        stake_amount: order.stake_amount,
        payout_amount,
        settlement_price: snapshot.price,
        settlement_price_tick_id: snapshot.id,
        settlement_price_source: snapshot.source,
        settlement_price_observed_at: snapshot.observed_at,
        settlement_price_generation: snapshot.generation,
        settlement_price_version: snapshot.source_version,
        result: result.to_owned(),
    };
    tx.commit().await?;
    Ok(SettlementOutcome::Settled(Box::new(event)))
}

fn publish_settlement_event(
    hub: Option<&EventBroadcastHub>,
    event: &SecondsContractSettlementEvent,
) {
    if let Some(hub) = hub {
        hub.publish(EventBroadcastMessage::private_user(
            event.user_id,
            json!({
                "type": "seconds_contract.order.settled",
                "order_id": event.order_id,
                "product_id": event.product_id,
                "pair_id": event.pair_id,
                "stake_asset": event.stake_asset,
                "direction": event.direction,
                "stake_amount": event.stake_amount,
                "settlement_price": event.settlement_price,
                "settlement_price_tick_id": event.settlement_price_tick_id,
                "settlement_price_source": event.settlement_price_source,
                "settlement_price_observed_at": event.settlement_price_observed_at.timestamp_millis(),
                "settlement_price_generation": event.settlement_price_generation,
                "settlement_price_version": event.settlement_price_version,
                "payout_amount": event.payout_amount,
                "result": event.result,
                "status": "settled",
            })
            .to_string(),
        ));
    }
}

fn seconds_contract_settlement_limit(limit: u32) -> u32 {
    limit.clamp(1, 100)
}

fn seconds_contract_settlement_scan_limit(limit: u32) -> u32 {
    seconds_contract_settlement_limit(limit)
}

fn normalize_max_snapshot_wait_seconds(value: u64) -> u64 {
    value.clamp(1, MAX_CONFIGURED_SNAPSHOT_WAIT_SECONDS)
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
#[path = "../../tests/unit_src/src_workers_seconds_contract_settlement_tests.rs"]
mod tests;
