use crate::{
    error::{AppError, AppResult},
    infra::mongo::{ensure_kline_indexes, kline_collection_name},
    modules::market::{KlineUpsertKey, ValidatedMarketSymbol},
    state::AppState,
};
use bigdecimal::{BigDecimal, ToPrimitive};
use chrono::{DateTime, TimeDelta, Timelike, Utc};
use mongodb::{
    Database,
    bson::{DateTime as BsonDateTime, Document, doc},
    options::UpdateOptions,
};
use sqlx::{MySql, Pool};
use std::str::FromStr;
use thiserror::Error;
use tokio::time::{Duration, interval};
use tracing::{error, info, warn};

const MAX_CANDLES_PER_STRATEGY_RUN: usize = 500;

pub struct KlineRecoveryWorker;

impl KlineRecoveryWorker {
    pub async fn run_once(
        &self,
        state: &AppState,
        now: DateTime<Utc>,
        limit: u32,
    ) -> AppResult<KlineRecoverySummary> {
        run_once(state, now, limit).await
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KlineRecoverySummary {
    pub scanned: u32,
    pub recovered_candles: u32,
    pub skipped: u32,
    pub failed: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KlineRecoveryPlanSummary {
    Recovered { candles: u32 },
    Skipped,
    Failed,
}

#[derive(Debug, Error)]
enum KlineRecoveryCheckpointError {
    #[error("K 线恢复检查点已被推进")]
    AlreadyAdvanced,
    #[error(transparent)]
    App(#[from] AppError),
}

pub fn summarize_recovery_plans(plans: &[KlineRecoveryPlanSummary]) -> KlineRecoverySummary {
    let mut summary = KlineRecoverySummary {
        scanned: plans.len() as u32,
        ..KlineRecoverySummary::default()
    };

    for plan in plans {
        match plan {
            KlineRecoveryPlanSummary::Recovered { candles } => {
                summary.recovered_candles += candles;
            }
            KlineRecoveryPlanSummary::Skipped => summary.skipped += 1,
            KlineRecoveryPlanSummary::Failed => summary.failed += 1,
        }
    }

    summary
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KlineRecoveryGap {
    missing_open_times: Vec<DateTime<Utc>>,
}

impl KlineRecoveryGap {
    pub fn missing_open_times(&self) -> &[DateTime<Utc>] {
        &self.missing_open_times
    }

    pub fn has_gap(&self) -> bool {
        !self.missing_open_times.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KlineRecoveryStrategyRun {
    strategy_id: u64,
    symbol: ValidatedMarketSymbol,
    checkpoint_open_time: DateTime<Utc>,
    current_price: BigDecimal,
    target_price: BigDecimal,
    volatility: BigDecimal,
    volume_min: BigDecimal,
    volume_max: BigDecimal,
}

impl KlineRecoveryStrategyRun {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        strategy_id: u64,
        symbol: &str,
        checkpoint_open_time: DateTime<Utc>,
        current_price: &str,
        target_price: &str,
        volatility: &str,
        volume_min: &str,
        volume_max: &str,
    ) -> AppResult<Self> {
        Self::from_values(
            strategy_id,
            symbol,
            checkpoint_open_time,
            parse_decimal(current_price)?,
            parse_decimal(target_price)?,
            parse_decimal(volatility)?,
            parse_decimal(volume_min)?,
            parse_decimal(volume_max)?,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_values(
        strategy_id: u64,
        symbol: &str,
        checkpoint_open_time: DateTime<Utc>,
        current_price: BigDecimal,
        target_price: BigDecimal,
        volatility: BigDecimal,
        volume_min: BigDecimal,
        volume_max: BigDecimal,
    ) -> AppResult<Self> {
        if current_price <= BigDecimal::default() || target_price <= BigDecimal::default() {
            return Err(AppError::Validation(
                "kline recovery prices must be positive".to_owned(),
            ));
        }
        if volatility < BigDecimal::default()
            || volume_min < BigDecimal::default()
            || volume_max < BigDecimal::default()
        {
            return Err(AppError::Validation(
                "kline recovery volatility and volume must be non-negative".to_owned(),
            ));
        }
        if volume_max < volume_min {
            return Err(AppError::Validation(
                "kline recovery volume_max must be greater than or equal to volume_min".to_owned(),
            ));
        }

        Ok(Self {
            strategy_id,
            symbol: ValidatedMarketSymbol::from_raw(symbol)
                .map_err(|error| AppError::Validation(error.to_string()))?,
            checkpoint_open_time,
            current_price,
            target_price,
            volatility,
            volume_min,
            volume_max,
        })
    }

    fn from_row(row: DueKlineRecoveryRun) -> AppResult<Self> {
        Self::from_values(
            row.strategy_id,
            &row.symbol,
            row.checkpoint_open_time,
            row.current_price,
            row.target_price,
            row.volatility,
            row.volume_min,
            row.volume_max,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KlineRecoveryPlan {
    strategy_id: u64,
    symbol: String,
    interval: String,
    candles: Vec<KlineRecoveryCandle>,
}

impl KlineRecoveryPlan {
    pub fn from_strategy(
        strategy: &KlineRecoveryStrategyRun,
        now: DateTime<Utc>,
        interval: TimeDelta,
    ) -> AppResult<Self> {
        let interval_name = recovery_interval_name(interval)?;
        let recovery_until = last_closed_open_time(now, interval)?;
        let gap = kline_recovery_gap(strategy.checkpoint_open_time, recovery_until, interval)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        let missing = gap.missing_open_times();
        if missing.is_empty() {
            return Ok(Self {
                strategy_id: strategy.strategy_id,
                symbol: strategy.symbol.as_str().to_owned(),
                interval: interval_name.to_owned(),
                candles: Vec::new(),
            });
        }

        let candle_count = missing.len() as i64;
        let divisor = BigDecimal::from(candle_count);
        let price_step = (strategy.target_price.clone() - strategy.current_price.clone()) / divisor;
        let volume_step = (strategy.volume_max.clone() - strategy.volume_min.clone())
            / BigDecimal::from(candle_count.max(1));
        let mut previous_close = strategy.current_price.clone();
        let mut candles = Vec::with_capacity(missing.len());

        for (index, open_time) in missing.iter().enumerate() {
            let ordinal = BigDecimal::from(index as i64 + 1);
            let close = if index + 1 == missing.len() {
                strategy.target_price.clone()
            } else {
                strategy.current_price.clone() + price_step.clone() * ordinal.clone()
            };
            let open = previous_close.clone();
            let high = decimal_max(&open, &close) + strategy.volatility.clone();
            let low = decimal_min(&open, &close) - strategy.volatility.clone();
            let volume = if index + 1 == missing.len() {
                strategy.volume_max.clone()
            } else {
                strategy.volume_min.clone() + volume_step.clone() * ordinal
            };

            candles.push(KlineRecoveryCandle::new(
                strategy.symbol.as_str(),
                interval_name,
                *open_time,
                open.to_string(),
                high.to_string(),
                low.to_string(),
                close.to_string(),
                volume.to_string(),
            )?);
            previous_close = close;
        }

        Ok(Self {
            strategy_id: strategy.strategy_id,
            symbol: strategy.symbol.as_str().to_owned(),
            interval: interval_name.to_owned(),
            candles,
        })
    }

    pub fn strategy_id(&self) -> u64 {
        self.strategy_id
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub fn interval(&self) -> &str {
        &self.interval
    }

    pub fn candles(&self) -> &[KlineRecoveryCandle] {
        &self.candles
    }

    fn last_candle(&self) -> Option<&KlineRecoveryCandle> {
        self.candles.last()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KlineRecoveryCandle {
    symbol: ValidatedMarketSymbol,
    interval: String,
    open_time: DateTime<Utc>,
    open: String,
    high: String,
    low: String,
    close: String,
    volume: String,
}

impl KlineRecoveryCandle {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        symbol: &str,
        interval: &str,
        open_time: DateTime<Utc>,
        open: impl Into<String>,
        high: impl Into<String>,
        low: impl Into<String>,
        close: impl Into<String>,
        volume: impl Into<String>,
    ) -> AppResult<Self> {
        let symbol = ValidatedMarketSymbol::from_raw(symbol)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        KlineUpsertKey::new(interval, open_time)
            .map_err(|error| AppError::Validation(error.to_string()))?;

        Ok(Self {
            symbol,
            interval: interval.to_owned(),
            open_time,
            open: open.into(),
            high: high.into(),
            low: low.into(),
            close: close.into(),
            volume: volume.into(),
        })
    }

    pub fn symbol(&self) -> &ValidatedMarketSymbol {
        &self.symbol
    }

    pub fn collection_name(&self) -> String {
        kline_collection_name(&self.symbol)
    }

    pub fn open_time(&self) -> DateTime<Utc> {
        self.open_time
    }

    pub fn close(&self) -> &str {
        &self.close
    }

    pub fn upsert_filter(&self) -> Document {
        doc! {
            "interval": &self.interval,
            "open_time": BsonDateTime::from_millis(self.open_time.timestamp_millis()),
        }
    }

    pub fn upsert_update(&self) -> Document {
        doc! {
            "$set": {
                "interval": &self.interval,
                "open_time": BsonDateTime::from_millis(self.open_time.timestamp_millis()),
                "open": &self.open,
                "high": &self.high,
                "low": &self.low,
                "close": &self.close,
                "volume": &self.volume,
            }
        }
    }
}

pub async fn run_once(
    state: &AppState,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<KlineRecoverySummary> {
    let pool = state.mysql.as_ref().ok_or_else(|| {
        AppError::Internal("mysql pool is required for kline recovery".to_owned())
    })?;
    let mongo = state.mongo.as_ref().ok_or_else(|| {
        AppError::Internal("mongo database is required for kline recovery".to_owned())
    })?;
    run_once_with_dependencies(pool, mongo, now, limit).await
}

pub async fn run_once_with_dependencies(
    pool: &Pool<MySql>,
    mongo: &Database,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<KlineRecoverySummary> {
    let rows = fetch_due_strategy_runs(pool, now, limit).await?;
    let mut outcomes = Vec::with_capacity(rows.len());

    for row in rows {
        let strategy_id = row.strategy_id;
        let outcome = match KlineRecoveryStrategyRun::from_row(row).and_then(|strategy| {
            KlineRecoveryPlan::from_strategy(&strategy, now, TimeDelta::minutes(1))
        }) {
            Ok(plan) if plan.candles().is_empty() => KlineRecoveryPlanSummary::Skipped,
            Ok(plan) => match recover_plan(pool, mongo, &plan).await {
                Ok(candles) => KlineRecoveryPlanSummary::Recovered { candles },
                Err(KlineRecoveryCheckpointError::AlreadyAdvanced) => {
                    warn!(strategy_id, "K 线恢复检查点已被推进");
                    KlineRecoveryPlanSummary::Skipped
                }
                Err(KlineRecoveryCheckpointError::App(error)) => {
                    warn!(strategy_id, %error, "K 线恢复计划执行失败");
                    mark_recovery_failed(pool, strategy_id, &error.to_string()).await;
                    KlineRecoveryPlanSummary::Failed
                }
            },
            Err(error) => {
                warn!(strategy_id, %error, "K 线恢复计划无效");
                mark_recovery_failed(pool, strategy_id, &error.to_string()).await;
                KlineRecoveryPlanSummary::Failed
            }
        };
        outcomes.push(outcome);
    }

    Ok(summarize_recovery_plans(&outcomes))
}

pub async fn run_loop(state: AppState, interval_seconds: u64, limit: u32) -> AppResult<()> {
    let mut ticker = interval(Duration::from_secs(interval_seconds.max(1)));

    loop {
        ticker.tick().await;
        match run_once(&state, Utc::now(), limit).await {
            Ok(summary) => info!(
                scanned = summary.scanned,
                recovered_candles = summary.recovered_candles,
                skipped = summary.skipped,
                failed = summary.failed,
                "K 线恢复周期完成"
            ),
            Err(error) => error!(%error, "K 线恢复周期失败"),
        }
    }
}

pub async fn upsert_recovered_kline(db: &Database, candle: &KlineRecoveryCandle) -> AppResult<()> {
    db.collection::<Document>(&candle.collection_name())
        .update_one(candle.upsert_filter(), candle.upsert_update())
        .with_options(UpdateOptions::builder().upsert(true).build())
        .await?;
    Ok(())
}

pub fn kline_recovery_gap(
    checkpoint_open_time: DateTime<Utc>,
    now: DateTime<Utc>,
    interval: TimeDelta,
) -> Result<KlineRecoveryGap, KlineRecoveryGapError> {
    if interval <= TimeDelta::zero() {
        return Err(KlineRecoveryGapError::InvalidInterval);
    }

    let checkpoint_open_time = align_open_time(checkpoint_open_time, interval)?;
    let now = align_open_time(now, interval)?;
    let mut missing_open_times = Vec::new();
    let mut open_time = checkpoint_open_time + interval;
    while open_time <= now && missing_open_times.len() < MAX_CANDLES_PER_STRATEGY_RUN {
        missing_open_times.push(open_time);
        open_time += interval;
    }

    Ok(KlineRecoveryGap { missing_open_times })
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KlineRecoveryGapError {
    #[error("kline interval must be positive")]
    InvalidInterval,
}

#[derive(Debug, sqlx::FromRow)]
struct DueKlineRecoveryRun {
    strategy_id: u64,
    symbol: String,
    checkpoint_open_time: DateTime<Utc>,
    current_price: BigDecimal,
    target_price: BigDecimal,
    volatility: BigDecimal,
    volume_min: BigDecimal,
    volume_max: BigDecimal,
}

async fn fetch_due_strategy_runs(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<Vec<DueKlineRecoveryRun>> {
    sqlx::query_as::<_, DueKlineRecoveryRun>(
        r#"SELECT strategies.id AS strategy_id,
                  pairs.symbol,
                  COALESCE(runs.last_kline_open_time, runs.last_generated_at, strategies.start_time) AS checkpoint_open_time,
                  COALESCE(runs.current_price, strategies.start_price) AS current_price,
                  strategies.target_price,
                  strategies.volatility,
                  strategies.volume_min,
                  strategies.volume_max
           FROM strategy_runs runs
           INNER JOIN market_strategies strategies ON strategies.id = runs.strategy_id
           INNER JOIN trading_pairs pairs ON pairs.id = strategies.pair_id
           WHERE strategies.status = 'active'
             AND pairs.status = 'active'
             AND runs.run_status IN ('running', 'live', 'catching_up')
             AND COALESCE(runs.recovery_status, 'idle') <> 'failed'
             AND COALESCE(runs.last_kline_open_time, runs.last_generated_at, strategies.start_time) < ?
           ORDER BY COALESCE(runs.last_kline_open_time, runs.last_generated_at, strategies.start_time) ASC,
                    strategies.id ASC
           LIMIT ?"#,
    )
    .bind(last_closed_open_time(now, TimeDelta::minutes(1))?.naive_utc())
    .bind(kline_recovery_limit(limit) as i64)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

async fn recover_plan(
    pool: &Pool<MySql>,
    mongo: &Database,
    plan: &KlineRecoveryPlan,
) -> Result<u32, KlineRecoveryCheckpointError> {
    let Some(last_candle) = plan.last_candle() else {
        return Ok(0);
    };

    // 先保证目标 collection 的唯一索引存在，再按 open_time 幂等补写缺口 K 线。
    ensure_kline_indexes(mongo, last_candle.symbol()).await?;
    for candle in plan.candles() {
        upsert_recovered_kline(mongo, candle).await?;
    }
    update_recovery_checkpoint(
        pool,
        plan.strategy_id(),
        last_candle.open_time(),
        last_candle.close(),
    )
    .await?;
    Ok(plan.candles().len() as u32)
}

async fn update_recovery_checkpoint(
    pool: &Pool<MySql>,
    strategy_id: u64,
    last_open_time: DateTime<Utc>,
    current_price: &str,
) -> Result<(), KlineRecoveryCheckpointError> {
    let current_price = parse_decimal(current_price)?;
    let result = sqlx::query(
        r#"UPDATE strategy_runs
           SET current_price = ?,
               last_generated_at = ?,
               last_kline_open_time = ?,
               recovery_status = 'live',
               error_message = NULL
           WHERE strategy_id = ?
             AND COALESCE(last_kline_open_time, last_generated_at, '1970-01-01 00:00:00') < ?"#,
    )
    .bind(current_price)
    .bind(last_open_time.naive_utc())
    .bind(last_open_time.naive_utc())
    .bind(strategy_id)
    .bind(last_open_time.naive_utc())
    .execute(pool)
    .await
    .map_err(AppError::from)?;
    if result.rows_affected() != 1 {
        return Err(KlineRecoveryCheckpointError::AlreadyAdvanced);
    }
    Ok(())
}

async fn mark_recovery_failed(pool: &Pool<MySql>, strategy_id: u64, error_message: &str) {
    let truncated = error_message.chars().take(1024).collect::<String>();
    if let Err(error) = sqlx::query(
        r#"UPDATE strategy_runs
           SET recovery_status = 'failed', error_message = ?
           WHERE strategy_id = ?"#,
    )
    .bind(truncated)
    .bind(strategy_id)
    .execute(pool)
    .await
    {
        warn!(strategy_id, %error, "标记 K 线恢复错误失败");
    }
}

fn recovery_interval_name(interval: TimeDelta) -> AppResult<&'static str> {
    match interval {
        value if value == TimeDelta::minutes(1) => Ok("1m"),
        value if value == TimeDelta::minutes(5) => Ok("5m"),
        value if value == TimeDelta::minutes(15) => Ok("15m"),
        value if value == TimeDelta::hours(1) => Ok("1h"),
        value if value == TimeDelta::days(1) => Ok("1d"),
        _ => Err(AppError::Validation(
            "unsupported kline recovery interval".to_owned(),
        )),
    }
}

fn last_closed_open_time(now: DateTime<Utc>, interval: TimeDelta) -> AppResult<DateTime<Utc>> {
    let aligned =
        align_open_time(now, interval).map_err(|error| AppError::Validation(error.to_string()))?;
    Ok(aligned - interval)
}

fn align_open_time(
    value: DateTime<Utc>,
    interval: TimeDelta,
) -> Result<DateTime<Utc>, KlineRecoveryGapError> {
    if interval <= TimeDelta::zero() {
        return Err(KlineRecoveryGapError::InvalidInterval);
    }
    let interval_seconds = interval
        .num_seconds()
        .to_f64()
        .ok_or(KlineRecoveryGapError::InvalidInterval)?;
    let timestamp = value.timestamp() as f64 + f64::from(value.nanosecond()) / 1_000_000_000.0;
    let aligned_seconds = (timestamp / interval_seconds).floor() * interval_seconds;
    let aligned_millis = (aligned_seconds * 1000.0).floor() as i64;
    DateTime::<Utc>::from_timestamp_millis(aligned_millis)
        .ok_or(KlineRecoveryGapError::InvalidInterval)
}

fn decimal_max(left: &BigDecimal, right: &BigDecimal) -> BigDecimal {
    if left >= right {
        left.clone()
    } else {
        right.clone()
    }
}

fn decimal_min(left: &BigDecimal, right: &BigDecimal) -> BigDecimal {
    if left <= right {
        left.clone()
    } else {
        right.clone()
    }
}

fn parse_decimal(value: &str) -> AppResult<BigDecimal> {
    BigDecimal::from_str(value)
        .map_err(|error| AppError::Validation(format!("invalid decimal value: {error}")))
}

fn kline_recovery_limit(limit: u32) -> u32 {
    limit.clamp(1, 100)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeDelta, TimeZone, Utc};

    #[test]
    fn recovery_gap_returns_missing_open_times_after_checkpoint_until_now() {
        let checkpoint = Utc.with_ymd_and_hms(2026, 5, 26, 10, 0, 0).unwrap();
        let now = checkpoint + TimeDelta::minutes(4);

        let gap = kline_recovery_gap(checkpoint, now, TimeDelta::minutes(1)).unwrap();

        assert_eq!(
            gap.missing_open_times(),
            &[
                checkpoint + TimeDelta::minutes(1),
                checkpoint + TimeDelta::minutes(2),
                checkpoint + TimeDelta::minutes(3),
                checkpoint + TimeDelta::minutes(4),
            ]
        );
        assert!(gap.has_gap());
    }

    #[test]
    fn recovery_gap_is_empty_without_elapsed_interval() {
        let checkpoint = Utc.with_ymd_and_hms(2026, 5, 26, 10, 0, 0).unwrap();

        let gap = kline_recovery_gap(
            checkpoint,
            checkpoint + TimeDelta::seconds(59),
            TimeDelta::minutes(1),
        )
        .unwrap();

        assert!(!gap.has_gap());
        assert!(gap.missing_open_times().is_empty());
        assert_eq!(
            kline_recovery_gap(checkpoint, checkpoint, TimeDelta::zero()).unwrap_err(),
            KlineRecoveryGapError::InvalidInterval
        );
    }

    #[test]
    fn recovered_kline_builds_symbol_scoped_upsert_documents() {
        use mongodb::bson::{DateTime as BsonDateTime, doc};

        let open_time = Utc.with_ymd_and_hms(2026, 5, 26, 10, 1, 0).unwrap();
        let candle = KlineRecoveryCandle::new(
            "NEW-USDT", "1m", open_time, "1.0", "2.0", "0.9", "1.5", "100.0",
        )
        .unwrap();

        assert_eq!(candle.collection_name(), "market_klines_NEWUSDT");
        assert_eq!(
            candle.upsert_filter(),
            doc! { "interval": "1m", "open_time": BsonDateTime::from_millis(open_time.timestamp_millis()) }
        );
        assert_eq!(
            candle.upsert_update(),
            doc! { "$set": {
                "interval": "1m",
                "open_time": BsonDateTime::from_millis(open_time.timestamp_millis()),
                "open": "1.0",
                "high": "2.0",
                "low": "0.9",
                "close": "1.5",
                "volume": "100.0",
            }}
        );
        assert!(
            KlineRecoveryCandle::new("NEW.USDT", "1m", open_time, "1", "1", "1", "1", "1").is_err()
        );
        assert!(
            KlineRecoveryCandle::new("NEW-USDT", "2m", open_time, "1", "1", "1", "1", "1").is_err()
        );
    }

    #[test]
    fn recovery_gap_aligns_open_times_and_caps_batch_size() {
        let checkpoint = Utc.with_ymd_and_hms(2026, 5, 29, 10, 0, 30).unwrap();
        let now = checkpoint + TimeDelta::minutes(800);

        let gap = kline_recovery_gap(checkpoint, now, TimeDelta::minutes(1)).unwrap();

        assert_eq!(gap.missing_open_times().len(), MAX_CANDLES_PER_STRATEGY_RUN);
        assert_eq!(
            gap.missing_open_times().first().copied().unwrap(),
            Utc.with_ymd_and_hms(2026, 5, 29, 10, 1, 0).unwrap()
        );
        assert_eq!(
            gap.missing_open_times().last().copied().unwrap(),
            Utc.with_ymd_and_hms(2026, 5, 29, 18, 20, 0).unwrap()
        );
    }

    #[test]
    fn recovery_plan_uses_only_last_closed_open_time() {
        let checkpoint = Utc.with_ymd_and_hms(2026, 5, 29, 10, 0, 0).unwrap();
        let now = Utc.with_ymd_and_hms(2026, 5, 29, 10, 3, 0).unwrap();
        let strategy = KlineRecoveryStrategyRun::new(
            9,
            "NEW-USDT",
            checkpoint,
            "1.000000000000000000",
            "1.060000000000000000",
            "0.01000000",
            "100.000000000000000000",
            "200.000000000000000000",
        )
        .unwrap();

        let plan = KlineRecoveryPlan::from_strategy(&strategy, now, TimeDelta::minutes(1)).unwrap();

        assert_eq!(plan.candles().len(), 2);
        assert_eq!(
            plan.candles().last().map(KlineRecoveryCandle::open_time),
            Some(checkpoint + TimeDelta::minutes(2))
        );
    }

    #[test]
    fn kline_recovery_plan_scans_running_strategies_until_now() {
        let checkpoint = Utc.with_ymd_and_hms(2026, 5, 29, 10, 0, 0).unwrap();
        let now = checkpoint + TimeDelta::minutes(4) + TimeDelta::seconds(30);
        let strategy = KlineRecoveryStrategyRun::new(
            7,
            "NEW-USDT",
            checkpoint,
            "1.000000000000000000",
            "1.060000000000000000",
            "0.01000000",
            "100.000000000000000000",
            "200.000000000000000000",
        )
        .unwrap();

        let plan = KlineRecoveryPlan::from_strategy(&strategy, now, TimeDelta::minutes(1)).unwrap();

        assert_eq!(plan.strategy_id(), 7);
        assert_eq!(plan.symbol(), "NEWUSDT");
        assert_eq!(plan.interval(), "1m");
        assert_eq!(plan.candles().len(), 3);
        assert_eq!(
            plan.candles()
                .iter()
                .map(KlineRecoveryCandle::open_time)
                .collect::<Vec<_>>(),
            vec![
                checkpoint + TimeDelta::minutes(1),
                checkpoint + TimeDelta::minutes(2),
                checkpoint + TimeDelta::minutes(3),
            ]
        );
        assert_eq!(
            plan.candles()
                .last()
                .map(KlineRecoveryCandle::close)
                .unwrap(),
            "1.060000000000000000"
        );
    }

    #[test]
    fn kline_recovery_summary_counts_scanned_recovered_and_skipped_runs() {
        let summary = summarize_recovery_plans(&[
            KlineRecoveryPlanSummary::Recovered { candles: 2 },
            KlineRecoveryPlanSummary::Skipped,
            KlineRecoveryPlanSummary::Failed,
        ]);

        assert_eq!(summary.scanned, 3);
        assert_eq!(summary.recovered_candles, 2);
        assert_eq!(summary.skipped, 1);
        assert_eq!(summary.failed, 1);
    }
}
