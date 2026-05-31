use crate::error::{AppError, AppResult};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use sqlx::{MySql, Pool, Transaction};
use std::env;
use tokio::time::{Duration, interval};
use tracing::{error, info, warn};

pub struct MarginInterestWorker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarginInterestWorkerConfig {
    pub enabled: bool,
    pub interval_seconds: u64,
    pub batch_limit: u32,
}

impl MarginInterestWorkerConfig {
    pub fn from_env() -> Self {
        Self {
            enabled: env_bool("MARGIN_INTEREST_ENABLED", true),
            interval_seconds: env_u64("MARGIN_INTEREST_INTERVAL_SECONDS", 60),
            batch_limit: env_u32("MARGIN_INTEREST_BATCH_LIMIT", 100),
        }
    }
}

impl MarginInterestWorker {
    pub async fn run_once(
        &self,
        pool: &Pool<MySql>,
        now: DateTime<Utc>,
        limit: u32,
    ) -> AppResult<MarginInterestSummary> {
        run_once_with_dependencies(pool, now, limit).await
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MarginInterestSummary {
    pub scanned: u32,
    pub accrued: u32,
    pub skipped: u32,
    pub failed: u32,
}

#[derive(Debug, sqlx::FromRow)]
struct MarginInterestCandidate {
    position_id: u64,
}

#[derive(Debug, sqlx::FromRow)]
struct LockedMarginPosition {
    id: u64,
    borrowed_amount: BigDecimal,
    interest_amount: BigDecimal,
    interest_accrued_at: Option<DateTime<Utc>>,
    opened_at: DateTime<Utc>,
    status: String,
    hourly_interest_rate: BigDecimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarginInterestOutcome {
    Accrued,
    Skipped,
}

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
        match accrue_position_interest(pool, candidate.position_id, now).await {
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

pub async fn run_loop(pool: Pool<MySql>, interval_seconds: u64, limit: u32) -> AppResult<()> {
    let mut ticker = interval(Duration::from_secs(interval_seconds.max(1)));

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

async fn fetch_interest_candidates(
    pool: &Pool<MySql>,
    limit: u32,
) -> AppResult<Vec<MarginInterestCandidate>> {
    sqlx::query_as::<_, MarginInterestCandidate>(
        r#"SELECT positions.id AS position_id
           FROM margin_positions positions
           INNER JOIN margin_products products ON products.id = positions.product_id
           WHERE positions.status = 'opened'
             AND positions.borrowed_amount > 0
             AND products.hourly_interest_rate > 0
           ORDER BY positions.interest_accrued_at ASC, positions.opened_at ASC, positions.id ASC
           LIMIT ?"#,
    )
    .bind(limit.clamp(1, 500) as i64)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

async fn accrue_position_interest(
    pool: &Pool<MySql>,
    position_id: u64,
    now: DateTime<Utc>,
) -> AppResult<MarginInterestOutcome> {
    let mut tx = pool.begin().await?;
    let Some(position) = lock_position(&mut tx, position_id).await? else {
        tx.rollback().await?;
        return Ok(MarginInterestOutcome::Skipped);
    };
    if position.status != "opened" || position.borrowed_amount <= 0 {
        tx.rollback().await?;
        return Ok(MarginInterestOutcome::Skipped);
    }
    let accrued_from = position.interest_accrued_at.unwrap_or(position.opened_at);
    let elapsed_hours = full_elapsed_hours(accrued_from, now);
    if elapsed_hours == 0 || position.hourly_interest_rate <= 0 {
        tx.rollback().await?;
        return Ok(MarginInterestOutcome::Skipped);
    }
    let interest_delta = margin_interest_delta(
        &position.borrowed_amount,
        &position.hourly_interest_rate,
        elapsed_hours,
    );
    if interest_delta <= 0 {
        tx.rollback().await?;
        return Ok(MarginInterestOutcome::Skipped);
    }
    let interest_after = (position.interest_amount + interest_delta).with_scale(18);
    let update = sqlx::query(
        r#"UPDATE margin_positions
           SET interest_amount = ?, interest_accrued_at = ?
           WHERE id = ? AND status = 'opened'"#,
    )
    .bind(&interest_after)
    .bind(now.naive_utc())
    .bind(position.id)
    .execute(&mut *tx)
    .await?;
    if update.rows_affected() != 1 {
        tx.rollback().await?;
        return Ok(MarginInterestOutcome::Skipped);
    }
    tx.commit().await?;
    Ok(MarginInterestOutcome::Accrued)
}

async fn lock_position(
    tx: &mut Transaction<'_, MySql>,
    position_id: u64,
) -> AppResult<Option<LockedMarginPosition>> {
    sqlx::query_as::<_, LockedMarginPosition>(
        r#"SELECT positions.id, positions.borrowed_amount, positions.interest_amount,
                  positions.interest_accrued_at, positions.opened_at, positions.status,
                  products.hourly_interest_rate
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

fn margin_interest_delta(
    borrowed_amount: &BigDecimal,
    hourly_interest_rate: &BigDecimal,
    elapsed_hours: u64,
) -> BigDecimal {
    (borrowed_amount.clone() * hourly_interest_rate.clone() * BigDecimal::from(elapsed_hours))
        .with_scale(18)
}

fn full_elapsed_hours(from: DateTime<Utc>, now: DateTime<Utc>) -> u64 {
    if now <= from {
        return 0;
    }
    (now - from).num_hours().max(0) as u64
}

fn margin_interest_limit(limit: u32) -> u32 {
    limit.clamp(1, 100)
}

fn margin_interest_scan_limit(limit: u32) -> u32 {
    margin_interest_limit(limit)
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
