use crate::{
    error::{AppError, AppResult},
    modules::{
        events::{EventBroadcastHub, EventBroadcastMessage},
        margin::infrastructure::credit_margin_position_amount,
        market::market_ticker_redis_key,
    },
    state::AppState,
    time::unix_millis,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use redis::{AsyncCommands, aio::ConnectionManager};
use serde::Deserialize;
use serde_json::json;
use sqlx::{MySql, Pool, Transaction};
use std::env;
use tokio::time::{Duration, interval};
use tracing::{error, info, warn};

pub struct MarginLiquidationWorker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarginLiquidationWorkerConfig {
    pub enabled: bool,
    pub interval_seconds: u64,
    pub batch_limit: u32,
}

impl MarginLiquidationWorkerConfig {
    pub fn from_env() -> Self {
        Self {
            enabled: env_bool("MARGIN_LIQUIDATION_ENABLED", true),
            interval_seconds: env_u64("MARGIN_LIQUIDATION_INTERVAL_SECONDS", 5),
            batch_limit: env_u32("MARGIN_LIQUIDATION_BATCH_LIMIT", 100),
        }
    }
}

impl MarginLiquidationWorker {
    pub async fn run_once(
        &self,
        state: &AppState,
        now: DateTime<Utc>,
        limit: u32,
    ) -> AppResult<MarginLiquidationSummary> {
        run_once(state, now, limit).await
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MarginLiquidationSummary {
    pub scanned: u32,
    pub liquidated: u32,
    pub skipped: u32,
    pub failed: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarginLiquidationRiskState {
    pub should_liquidate: bool,
    pub equity: BigDecimal,
    pub maintenance_margin: BigDecimal,
    pub realized_pnl: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct MarginLiquidationCandidate {
    position_id: u64,
    symbol: String,
}

#[derive(Debug, sqlx::FromRow)]
struct LockedMarginPosition {
    id: u64,
    user_id: u64,
    product_id: u64,
    pair_id: u64,
    margin_asset: u64,
    wallet_scope: String,
    direction: String,
    margin_amount: BigDecimal,
    notional_amount: BigDecimal,
    interest_amount: BigDecimal,
    status: String,
    entry_price: Option<BigDecimal>,
    maintenance_margin_rate: BigDecimal,
}

#[derive(Debug, Deserialize)]
struct CachedTickerPayload {
    last_price: BigDecimal,
    #[serde(with = "unix_millis")]
    observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct MarginLiquidationEvent {
    user_id: u64,
    position_id: u64,
    product_id: u64,
    pair_id: u64,
    margin_asset: u64,
    direction: String,
    margin_amount: BigDecimal,
    notional_amount: BigDecimal,
    interest_amount: BigDecimal,
    entry_price: BigDecimal,
    mark_price: BigDecimal,
    realized_pnl: BigDecimal,
    payout_amount: BigDecimal,
    reason: &'static str,
    liquidated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
enum LiquidationOutcome {
    Liquidated(Box<MarginLiquidationEvent>),
    Skipped,
}

pub fn margin_liquidation_risk_state(
    direction: &str,
    margin_amount: &BigDecimal,
    notional_amount: &BigDecimal,
    interest_amount: &BigDecimal,
    entry_price: &BigDecimal,
    mark_price: &BigDecimal,
    maintenance_margin_rate: &BigDecimal,
) -> AppResult<MarginLiquidationRiskState> {
    validate_positive_decimal(entry_price, "margin entry price")?;
    validate_positive_decimal(mark_price, "margin mark price")?;
    let price_delta = match direction {
        "long" => mark_price.clone() - entry_price.clone(),
        "short" => entry_price.clone() - mark_price.clone(),
        _ => {
            return Err(AppError::Validation(
                "margin direction must be long or short".to_owned(),
            ));
        }
    };
    let realized_pnl = (notional_amount.clone() * price_delta / entry_price.clone()).with_scale(18);
    let equity =
        (margin_amount.clone() + realized_pnl.clone() - interest_amount.clone()).with_scale(18);
    let maintenance_margin =
        (notional_amount.clone() * maintenance_margin_rate.clone()).with_scale(18);
    Ok(MarginLiquidationRiskState {
        should_liquidate: equity <= maintenance_margin,
        equity,
        maintenance_margin,
        realized_pnl,
    })
}

pub async fn run_once(
    state: &AppState,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<MarginLiquidationSummary> {
    let pool = state.mysql.as_ref().ok_or_else(|| {
        AppError::Internal("mysql pool is required for margin liquidation".to_owned())
    })?;
    let redis = state.redis.as_ref().ok_or_else(|| {
        AppError::Internal("redis connection is required for margin liquidation".to_owned())
    })?;
    run_once_with_dependencies_and_events(
        pool,
        redis,
        state.event_broadcast_hub.as_ref(),
        now,
        limit,
    )
    .await
}

pub async fn run_once_with_dependencies(
    pool: &Pool<MySql>,
    redis: &ConnectionManager,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<MarginLiquidationSummary> {
    run_once_with_dependencies_and_events(pool, redis, None, now, limit).await
}

async fn run_once_with_dependencies_and_events(
    pool: &Pool<MySql>,
    redis: &ConnectionManager,
    event_hub: Option<&EventBroadcastHub>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<MarginLiquidationSummary> {
    let liquidation_limit = margin_liquidation_limit(limit);
    let candidates = fetch_open_positions(pool, now, margin_liquidation_scan_limit(limit)).await?;
    let mut summary = MarginLiquidationSummary::default();

    for candidate in candidates {
        if summary.liquidated >= liquidation_limit {
            break;
        }
        summary.scanned += 1;
        let mark_price = match cached_ticker_price(redis, &candidate.symbol, now).await {
            Ok(Some(price)) => price,
            Ok(None) => {
                summary.skipped += 1;
                reschedule_liquidation_attempt(pool, candidate.position_id, now).await?;
                warn!(position_id = candidate.position_id, symbol = %candidate.symbol, "杠杆强平跳过缺失行情仓位");
                continue;
            }
            Err(error) => {
                summary.failed += 1;
                reschedule_liquidation_attempt(pool, candidate.position_id, now).await?;
                warn!(position_id = candidate.position_id, symbol = %candidate.symbol, %error, "杠杆强平读取行情失败");
                continue;
            }
        };

        match liquidate_position_by_id(pool, candidate.position_id, &mark_price, now).await {
            Ok(LiquidationOutcome::Liquidated(event)) => {
                summary.liquidated += 1;
                if let Some(hub) = event_hub {
                    publish_liquidation_event(hub, &event);
                }
            }
            Ok(LiquidationOutcome::Skipped) => {
                summary.skipped += 1;
                reschedule_safe_liquidation_check(pool, candidate.position_id, now).await?;
            }
            Err(error) => {
                summary.failed += 1;
                reschedule_liquidation_attempt(pool, candidate.position_id, now).await?;
                warn!(position_id = candidate.position_id, %error, "杠杆强平失败");
            }
        }
    }

    Ok(summary)
}

pub async fn run_loop(state: AppState, interval_seconds: u64, limit: u32) -> AppResult<()> {
    let mut ticker = interval(Duration::from_secs(interval_seconds.max(1)));

    loop {
        ticker.tick().await;
        match run_once(&state, Utc::now(), limit).await {
            Ok(summary) => info!(
                scanned = summary.scanned,
                liquidated = summary.liquidated,
                skipped = summary.skipped,
                failed = summary.failed,
                "杠杆强平周期完成"
            ),
            Err(error) => error!(%error, "杠杆强平周期失败"),
        }
    }
}

async fn fetch_open_positions(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<Vec<MarginLiquidationCandidate>> {
    sqlx::query_as::<_, MarginLiquidationCandidate>(
        r#"SELECT positions.id AS position_id,
                  pairs.symbol
           FROM margin_positions positions
           INNER JOIN trading_pairs pairs ON pairs.id = positions.pair_id
           WHERE positions.status = 'opened'
             AND (positions.next_liquidation_attempt_at IS NULL OR positions.next_liquidation_attempt_at <= ?)
           ORDER BY positions.next_liquidation_attempt_at ASC, positions.opened_at ASC, positions.id ASC
           LIMIT ?"#,
    )
    .bind(now.naive_utc())
    .bind(limit.clamp(1, 500) as i64)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

async fn cached_ticker_price(
    redis: &ConnectionManager,
    symbol: &str,
    now: DateTime<Utc>,
) -> AppResult<Option<BigDecimal>> {
    let mut connection = redis.clone();
    let payload: Option<String> = connection.get(market_ticker_redis_key(symbol)).await?;
    let Some(payload) = payload else {
        return Ok(None);
    };
    let ticker = serde_json::from_str::<CachedTickerPayload>(&payload)
        .map_err(|error| AppError::Internal(format!("invalid margin ticker payload: {error}")))?;
    validate_positive_decimal(&ticker.last_price, "margin mark price")?;
    if ticker.observed_at < now - chrono::TimeDelta::seconds(60) {
        return Err(AppError::Validation("margin ticker is stale".to_owned()));
    }
    Ok(Some(ticker.last_price))
}

async fn liquidate_position_by_id(
    pool: &Pool<MySql>,
    position_id: u64,
    mark_price: &BigDecimal,
    now: DateTime<Utc>,
) -> AppResult<LiquidationOutcome> {
    let mut tx = pool.begin().await?;
    let Some(position) = lock_position_by_id(&mut tx, position_id).await? else {
        tx.rollback().await?;
        return Ok(LiquidationOutcome::Skipped);
    };
    if position.status != "opened" {
        tx.rollback().await?;
        return Ok(LiquidationOutcome::Skipped);
    }
    let Some(entry_price) = position.entry_price.as_ref() else {
        return Err(AppError::Validation(
            "margin entry price is required for liquidation".to_owned(),
        ));
    };
    let risk_state = margin_liquidation_risk_state(
        &position.direction,
        &position.margin_amount,
        &position.notional_amount,
        &position.interest_amount,
        entry_price,
        mark_price,
        &position.maintenance_margin_rate,
    )?;
    if !risk_state.should_liquidate {
        tx.rollback().await?;
        return Ok(LiquidationOutcome::Skipped);
    }

    let payout_amount = non_negative_amount(&risk_state.equity);
    credit_margin_position_amount(
        &mut tx,
        position.user_id,
        position.margin_asset,
        &position.wallet_scope,
        &payout_amount,
        "margin_position_liquidate",
        position.id,
    )
    .await?;

    insert_liquidation_record(
        &mut tx,
        &position,
        entry_price,
        mark_price,
        &risk_state,
        &payout_amount,
        now,
    )
    .await?;

    let update_position = sqlx::query(
        r#"UPDATE margin_positions
           SET status = 'liquidated', closed_at = ?, liquidated_at = ?, exit_price = ?,
               realized_pnl = ?, liquidation_reason = 'maintenance_margin', next_liquidation_attempt_at = NULL
           WHERE id = ? AND status = 'opened'"#,
    )
    .bind(now.naive_utc())
    .bind(now.naive_utc())
    .bind(mark_price)
    .bind(&risk_state.realized_pnl)
    .bind(position.id)
    .execute(&mut *tx)
    .await?;
    if update_position.rows_affected() != 1 {
        tx.rollback().await?;
        return Ok(LiquidationOutcome::Skipped);
    }

    let event = MarginLiquidationEvent {
        user_id: position.user_id,
        position_id: position.id,
        product_id: position.product_id,
        pair_id: position.pair_id,
        margin_asset: position.margin_asset,
        direction: position.direction,
        margin_amount: position.margin_amount,
        notional_amount: position.notional_amount,
        interest_amount: position.interest_amount,
        entry_price: entry_price.clone(),
        mark_price: mark_price.clone(),
        realized_pnl: risk_state.realized_pnl,
        payout_amount,
        reason: "maintenance_margin",
        liquidated_at: now,
    };
    tx.commit().await?;
    Ok(LiquidationOutcome::Liquidated(Box::new(event)))
}

fn publish_liquidation_event(hub: &EventBroadcastHub, event: &MarginLiquidationEvent) {
    hub.publish(EventBroadcastMessage::private_user(
        event.user_id,
        json!({
            "type": "margin.position.liquidated",
            "position_id": event.position_id,
            "product_id": event.product_id,
            "pair_id": event.pair_id,
            "margin_asset": event.margin_asset,
            "direction": event.direction,
            "margin_amount": event.margin_amount,
            "notional_amount": event.notional_amount,
            "interest_amount": decimal_amount_string(&event.interest_amount),
            "entry_price": event.entry_price,
            "mark_price": event.mark_price,
            "realized_pnl": event.realized_pnl,
            "payout_amount": event.payout_amount,
            "reason": event.reason,
            "liquidated_at": event.liquidated_at.timestamp_millis(),
        })
        .to_string(),
    ));
}

async fn lock_position_by_id(
    tx: &mut Transaction<'_, MySql>,
    position_id: u64,
) -> AppResult<Option<LockedMarginPosition>> {
    sqlx::query_as::<_, LockedMarginPosition>(
        r#"SELECT positions.id, positions.user_id, positions.product_id, positions.pair_id,
                  positions.margin_asset, positions.wallet_scope, positions.direction, positions.margin_amount,
                  positions.notional_amount, positions.interest_amount, positions.status,
                  positions.entry_price, products.maintenance_margin_rate
           FROM margin_positions positions
           INNER JOIN margin_products products ON products.id = positions.product_id
           WHERE positions.id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(position_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

async fn insert_liquidation_record(
    tx: &mut Transaction<'_, MySql>,
    position: &LockedMarginPosition,
    entry_price: &BigDecimal,
    mark_price: &BigDecimal,
    risk_state: &MarginLiquidationRiskState,
    payout_amount: &BigDecimal,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO margin_liquidation_records
           (position_id, user_id, product_id, pair_id, margin_asset, direction, margin_amount,
            notional_amount, interest_amount, entry_price, mark_price, maintenance_margin_rate, equity,
            maintenance_margin, realized_pnl, payout_amount, reason, liquidated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'maintenance_margin', ?)"#,
    )
    .bind(position.id)
    .bind(position.user_id)
    .bind(position.product_id)
    .bind(position.pair_id)
    .bind(position.margin_asset)
    .bind(&position.direction)
    .bind(&position.margin_amount)
    .bind(&position.notional_amount)
    .bind(&position.interest_amount)
    .bind(entry_price)
    .bind(mark_price)
    .bind(&position.maintenance_margin_rate)
    .bind(&risk_state.equity)
    .bind(&risk_state.maintenance_margin)
    .bind(&risk_state.realized_pnl)
    .bind(payout_amount)
    .bind(now.naive_utc())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn reschedule_liquidation_attempt(
    pool: &Pool<MySql>,
    position_id: u64,
    now: DateTime<Utc>,
) -> AppResult<()> {
    schedule_next_liquidation_attempt(pool, position_id, now + chrono::TimeDelta::seconds(60)).await
}

async fn reschedule_safe_liquidation_check(
    pool: &Pool<MySql>,
    position_id: u64,
    now: DateTime<Utc>,
) -> AppResult<()> {
    schedule_next_liquidation_attempt(pool, position_id, now + chrono::TimeDelta::seconds(5)).await
}

async fn schedule_next_liquidation_attempt(
    pool: &Pool<MySql>,
    position_id: u64,
    next_attempt_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE margin_positions SET next_liquidation_attempt_at = ? WHERE id = ? AND status = 'opened'",
    )
    .bind(next_attempt_at.naive_utc())
    .bind(position_id)
    .execute(pool)
    .await?;
    Ok(())
}

fn non_negative_amount(amount: &BigDecimal) -> BigDecimal {
    if amount > &BigDecimal::from(0) {
        amount.clone().with_scale(18)
    } else {
        BigDecimal::from(0).with_scale(18)
    }
}

fn decimal_amount_string(amount: &BigDecimal) -> String {
    format!("{amount:.18}")
}

fn validate_positive_decimal(amount: &BigDecimal, label: &str) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        return Err(AppError::Validation(format!("{label} must be positive")));
    }
    Ok(())
}

fn margin_liquidation_limit(limit: u32) -> u32 {
    limit.clamp(1, 100)
}

fn margin_liquidation_scan_limit(limit: u32) -> u32 {
    margin_liquidation_limit(limit)
        .saturating_mul(10)
        .clamp(1, 500)
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
#[path = "../../tests/unit_src/src_workers_margin_liquidation_tests.rs"]
mod tests;
