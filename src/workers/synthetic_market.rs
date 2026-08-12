//! 新币策略模拟行情实时 worker。
//!
//! 每轮只读取并生成“当前 UTC 分钟”这一槽位，不从 `strategy_runs` 检查点向前扫描；因此进程重启
//! 只恢复当前实时行情，停机历史缺口继续留给管理员显式预览、确认并调用 `kline_recovery` 复用入口。

use std::{collections::HashMap, str::FromStr, sync::OnceLock};

use bigdecimal::{BigDecimal, RoundingMode};
use chrono::{DateTime, TimeDelta, Utc};
use mongodb::{
    Database,
    bson::{DateTime as BsonDateTime, Document, doc},
    options::FindOptions,
};
use serde_json::Value;
use sqlx::{MySql, Pool, types::Json as SqlxJson};
use tokio::time::{Duration, interval};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    infra::mongo::kline_collection_name,
    modules::market::{
        MarketDataProvider, MarketKlineSnapshot, MarketKlineValues, MarketTickerSnapshot,
        MarketTickerValues, ValidatedMarketSymbol,
        adapters::{MarketIngestionService, SyntheticIngestionOutcome},
        synthetic::{
            SyntheticCandle, SyntheticExecutionMode, SyntheticKlineInterval, SyntheticMarketConfig,
            SyntheticMarketNode, SyntheticTargetType, aggregate_1m_candles,
        },
    },
    state::AppState,
};

const LEASE_SECONDS: i64 = 60;
const MAX_STRATEGIES_PER_ROUND: u32 = 100;
const TICKER_WINDOW_MINUTES: i64 = 24 * 60;
const AGGREGATE_INTERVALS: [SyntheticKlineInterval; 5] = [
    SyntheticKlineInterval::FiveMinutes,
    SyntheticKlineInterval::FifteenMinutes,
    SyntheticKlineInterval::OneHour,
    SyntheticKlineInterval::FourHours,
    SyntheticKlineInterval::OneDay,
];

/// 单轮模拟行情统计；`scanned` 包含租约竞争失败项，`published` 只计 ticker 与 K 线均落地并推进检查点的策略。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyntheticMarketSummary {
    pub scanned: u32,
    pub leased: u32,
    pub published: u32,
    pub skipped: u32,
    pub failed: u32,
}

/// 当前分钟的统一发布计划；K 线和 ticker 已使用同一根策略蜡烛计算，保证最新价等于该分钟收盘价。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticRealtimePlan {
    strategy_id: u64,
    version: u32,
    kline: MarketKlineSnapshot,
    ticker: MarketTickerSnapshot,
}

/// 本进程确认连续在线后形成的分钟闭合计划；1m 是确定性权威值，高周期只列出本次边界应重建的窗口。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticMinuteClosePlan {
    kline: MarketKlineSnapshot,
    aggregate_intervals: Vec<SyntheticKlineInterval>,
}

impl SyntheticMinuteClosePlan {
    /// 返回上一分钟的确定性闭合 1m；实时 worker 必须先持久化该值，再读取权威窗口做聚合。
    pub fn kline(&self) -> &MarketKlineSnapshot {
        &self.kline
    }

    /// 返回刚完成边界对应的高周期；仅完整窗口会出现在结果中，不会为停机缺口枚举中间槽。
    pub fn aggregate_intervals(&self) -> &[SyntheticKlineInterval] {
        &self.aggregate_intervals
    }
}

impl SyntheticRealtimePlan {
    /// 返回计划所属策略 ID；该标识只用于租约和检查点，不进入公共行情 payload。
    pub fn strategy_id(&self) -> u64 {
        self.strategy_id
    }

    /// 返回生成计划使用的不可变配置版本；检查点推进会再次比对该版本，避免旧计划覆盖新配置。
    pub fn version(&self) -> u32 {
        self.version
    }

    /// 返回当前分钟权威 1m K 线；调用方应先 ingestion 成功，再推进策略检查点。
    pub fn kline(&self) -> &MarketKlineSnapshot {
        &self.kline
    }

    /// 返回与 K 线收盘价一致的 ticker 及历史窗口统计；该值不得由恢复任务用于倒退实时缓存。
    pub fn ticker(&self) -> &MarketTickerSnapshot {
        &self.ticker
    }
}

#[derive(Debug, sqlx::FromRow)]
struct SyntheticStrategyRow {
    strategy_id: u64,
    symbol: String,
    price_precision: i32,
    start_price: BigDecimal,
    target_price: BigDecimal,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    volatility: BigDecimal,
    volume_min: BigDecimal,
    volume_max: BigDecimal,
    version: i32,
    seed: String,
    config_json: SqlxJson<Value>,
}

#[derive(Debug, sqlx::FromRow)]
struct SyntheticNodeRow {
    target_time: DateTime<Utc>,
    target_type: String,
    target_value: BigDecimal,
    execution_mode: String,
    tolerance: BigDecimal,
    volatility: BigDecimal,
    volume_min: Option<BigDecimal>,
    volume_max: Option<BigDecimal>,
}

/// 按固定短周期驱动实时策略；进程级 owner 在所有轮次保持稳定，活实例续租，崩溃实例最多占用一分钟租约。
/// 周期错误只记录并继续下一轮；本循环不调用旧 K 线恢复扫描，也不根据历史检查点生成任何中间槽位。
pub async fn run_loop(state: AppState, interval_seconds: u64, limit: u32) -> AppResult<()> {
    let owner = worker_owner().to_owned();
    let mut continuity = HashMap::new();
    let mut ticker = interval(Duration::from_secs(interval_seconds.max(1)));
    loop {
        ticker.tick().await;
        match run_once_for_owner(&state, Utc::now(), limit, &owner, &mut continuity).await {
            Ok(summary) => info!(
                scanned = summary.scanned,
                leased = summary.leased,
                published = summary.published,
                skipped = summary.skipped,
                failed = summary.failed,
                "模拟行情实时轮次完成"
            ),
            Err(error) => tracing::error!(%error, "模拟行情实时轮次失败"),
        }
    }
}

/// 从应用状态组装 MySQL、Mongo 与统一 ingestion 后执行一轮当前分钟发布；缺少任一权威依赖会在扫描前失败。
/// 该入口复用进程级租约 owner，重复调用只会幂等覆盖当前 1m 槽，不会补写检查点与当前分钟之间的历史。
pub async fn run_once(
    state: &AppState,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<SyntheticMarketSummary> {
    let mut continuity = HashMap::new();
    run_once_for_owner(state, now, limit, worker_owner(), &mut continuity).await
}

async fn run_once_for_owner(
    state: &AppState,
    now: DateTime<Utc>,
    limit: u32,
    owner: &str,
    continuity: &mut HashMap<u64, SyntheticRealtimePlan>,
) -> AppResult<SyntheticMarketSummary> {
    let pool = state.mysql.as_ref().ok_or_else(|| {
        AppError::Internal("mysql pool is required for synthetic market worker".to_owned())
    })?;
    let mongo = state.mongo.as_ref().ok_or_else(|| {
        AppError::Internal("mongo database is required for synthetic market worker".to_owned())
    })?;
    let ingestion = MarketIngestionService::from_state(state)?;
    run_once_with_runtime(pool, mongo, &ingestion, now, limit, owner, continuity).await
}

/// 读取 active strategy/internal 策略的最新版本快照，逐项竞争短租约并仅发布当前分钟 ticker 与 1m K 线。
/// 每项按“租约→版本/节点解析→历史窗口读取→K 线 ingestion/广播→ticker ingestion/广播→检查点”执行；
/// 单策略失败会写 `error_message` 后继续，跨 Redis/Mongo/MySQL 不伪造事务，重试依赖槽位 upsert 收敛。
pub async fn run_once_with_dependencies(
    pool: &Pool<MySql>,
    mongo: &Database,
    ingestion: &MarketIngestionService,
    now: DateTime<Utc>,
    limit: u32,
    owner: &str,
) -> AppResult<SyntheticMarketSummary> {
    let mut continuity = HashMap::new();
    run_once_with_runtime(pool, mongo, ingestion, now, limit, owner, &mut continuity).await
}

async fn run_once_with_runtime(
    pool: &Pool<MySql>,
    mongo: &Database,
    ingestion: &MarketIngestionService,
    now: DateTime<Utc>,
    limit: u32,
    owner: &str,
    continuity: &mut HashMap<u64, SyntheticRealtimePlan>,
) -> AppResult<SyntheticMarketSummary> {
    if owner.trim().is_empty() {
        return Err(AppError::Validation(
            "synthetic market lease owner must not be blank".to_owned(),
        ));
    }
    let open_time = current_minute_open_time(now)?;
    let rows = load_active_strategies(pool, now, limit, owner).await?;
    let mut summary = SyntheticMarketSummary {
        scanned: rows.len() as u32,
        ..SyntheticMarketSummary::default()
    };

    for row in rows {
        let strategy_id = row.strategy_id;
        let row_version = row.version;
        let lease_expires_at = now + TimeDelta::seconds(LEASE_SECONDS);
        if !acquire_lease(pool, strategy_id, row.version, owner, now, lease_expires_at).await? {
            summary.skipped += 1;
            continue;
        }
        summary.leased += 1;

        let result = process_leased_strategy(
            pool,
            mongo,
            ingestion,
            row,
            open_time,
            now,
            owner,
            lease_expires_at,
            continuity.get(&strategy_id),
        )
        .await;
        match result {
            Ok(plan) => {
                continuity.insert(strategy_id, plan);
                summary.published += 1;
            }
            Err(error) => {
                summary.failed += 1;
                warn!(strategy_id, %error, "模拟行情策略发布失败");
                mark_strategy_error(pool, strategy_id, row_version, owner, &error.to_string())
                    .await;
            }
        }
    }

    Ok(summary)
}

#[allow(clippy::too_many_arguments)]
async fn process_leased_strategy(
    pool: &Pool<MySql>,
    mongo: &Database,
    ingestion: &MarketIngestionService,
    row: SyntheticStrategyRow,
    open_time: DateTime<Utc>,
    observed_at: DateTime<Utc>,
    owner: &str,
    lease_expires_at: DateTime<Utc>,
    previous_plan: Option<&SyntheticRealtimePlan>,
) -> AppResult<SyntheticRealtimePlan> {
    let relation_nodes = load_strategy_nodes(pool, row.strategy_id).await?;
    let config = strategy_config(&row, relation_nodes)?;

    // ticker 是整轮提交门：其 CAS 拒写时不会发生订单触发/广播，也不会产生本轮 K 线或检查点副作用。
    let history = load_ticker_history(mongo, &config.symbol, open_time).await?;
    let plan = build_realtime_plan(row.strategy_id, &config, observed_at, &history)?;
    ensure_current_lease(pool, &plan, owner, observed_at).await?;
    if ingestion
        .ingest_and_publish_synthetic_ticker(plan.ticker())
        .await?
        == SyntheticIngestionOutcome::RejectedStale
    {
        return Err(stale_market_write_conflict("ticker"));
    }

    ensure_current_lease(pool, &plan, owner, Utc::now()).await?;
    if let Some(close_plan) =
        build_online_minute_close_plan(previous_plan, row.strategy_id, &config, observed_at)?
    {
        publish_minute_close(
            pool,
            mongo,
            ingestion,
            &config,
            &close_plan,
            &plan,
            owner,
            observed_at,
        )
        .await?;
    }
    ensure_current_lease(pool, &plan, owner, Utc::now()).await?;
    if ingestion
        .ingest_and_publish_synthetic_kline(plan.kline())
        .await?
        == SyntheticIngestionOutcome::RejectedStale
    {
        return Err(stale_market_write_conflict("kline"));
    }
    update_checkpoint(pool, &plan, owner, observed_at, lease_expires_at).await?;
    Ok(plan)
}

/// 使用确定性整分钟蜡烛和当前观测秒构造形成中 K 线，并把既有 1m 历史折叠为 24h ticker。
/// 第 59 秒发布完整确定性蜡烛，确保分钟最终 OHLCV 与补偿路径一致；本函数无 I/O，同秒重放稳定。
pub fn build_realtime_plan(
    strategy_id: u64,
    config: &SyntheticMarketConfig,
    observed_at: DateTime<Utc>,
    historical_1m: &[MarketKlineValues],
) -> AppResult<SyntheticRealtimePlan> {
    let open_time = current_minute_open_time(observed_at)?;
    let candle = config
        .generate_1m(open_time)
        .map_err(|error| AppError::Validation(error.to_string()))?;
    let values = forming_1m_values(
        &candle.values,
        candle.open_time,
        observed_at,
        config.price_precision,
    )?;
    let opening_price = historical_1m
        .first()
        .map_or_else(|| values.open.clone(), |item| item.open.clone());
    if opening_price <= 0 {
        return Err(AppError::Validation(
            "synthetic ticker opening price must be positive".to_owned(),
        ));
    }
    let mut high_24h = values.high.clone();
    let mut low_24h = values.low.clone();
    let mut volume_24h = values.volume.clone();
    for item in historical_1m {
        high_24h = high_24h.max(item.high.clone());
        low_24h = low_24h.min(item.low.clone());
        volume_24h += item.volume.clone();
    }
    let price_change_24h = &values.close - &opening_price;
    let price_change_percent_24h = (&price_change_24h / &opening_price) * BigDecimal::from(100);
    let ticker = MarketTickerSnapshot::with_24h(
        MarketDataProvider::Strategy,
        &config.symbol,
        MarketTickerValues::new(
            values.close.clone(),
            high_24h,
            low_24h,
            volume_24h,
            price_change_24h,
            price_change_percent_24h,
        ),
        observed_at,
    )
    .map_err(|error| AppError::Validation(error.to_string()))?;
    let kline = MarketKlineSnapshot::new(
        MarketDataProvider::Strategy,
        &config.symbol,
        "1m",
        candle.open_time,
        values,
        observed_at,
    )
    .map_err(|error| AppError::Validation(error.to_string()))?;

    Ok(SyntheticRealtimePlan {
        strategy_id,
        version: config.version,
        kline,
        ticker,
    })
}

/// 把确定性整分钟 OHLCV 映射为当前秒的形成中快照：价格依次经过两个确定性极值并回到最终 close，
/// 成交量按已观察秒数累计；第 59 秒直接返回整分钟值，避免实时闭合与手动补偿产生尾差。
fn forming_1m_values(
    closed: &MarketKlineValues,
    open_time: DateTime<Utc>,
    observed_at: DateTime<Utc>,
    price_precision: u32,
) -> AppResult<MarketKlineValues> {
    let elapsed_seconds = (observed_at - open_time).num_seconds();
    if !(0..60).contains(&elapsed_seconds) {
        return Err(AppError::Validation(
            "synthetic realtime observation must be inside current minute".to_owned(),
        ));
    }
    let observed_seconds = elapsed_seconds + 1;
    if observed_seconds == 60 {
        return Ok(closed.clone());
    }

    let (first_extreme, second_extreme) = if closed.close >= closed.open {
        (&closed.low, &closed.high)
    } else {
        (&closed.high, &closed.low)
    };
    let current_price = if observed_seconds <= 20 {
        interpolate_decimal(&closed.open, first_extreme, observed_seconds, 20)
    } else if observed_seconds <= 40 {
        interpolate_decimal(first_extreme, second_extreme, observed_seconds - 20, 20)
    } else {
        interpolate_decimal(second_extreme, &closed.close, observed_seconds - 40, 20)
    }
    .with_scale_round(i64::from(price_precision), RoundingMode::HalfUp);

    let mut high = closed.open.clone().max(current_price.clone());
    let mut low = closed.open.clone().min(current_price.clone());
    if observed_seconds >= 20 {
        high = high.max(first_extreme.clone());
        low = low.min(first_extreme.clone());
    }
    if observed_seconds >= 40 {
        high = high.max(second_extreme.clone());
        low = low.min(second_extreme.clone());
    }
    let volume = (&closed.volume * BigDecimal::from(observed_seconds) / BigDecimal::from(60))
        .with_scale_round(18, RoundingMode::HalfUp);

    Ok(MarketKlineValues {
        open: closed.open.clone(),
        high,
        low,
        close: current_price,
        volume,
    })
}

fn interpolate_decimal(
    start: &BigDecimal,
    end: &BigDecimal,
    elapsed: i64,
    total: i64,
) -> BigDecimal {
    start + ((end - start) * BigDecimal::from(elapsed) / BigDecimal::from(total))
}

/// 根据本次闭合时刻返回需要重建的完整高周期；策略尚未运行满一个窗口时不会声明该聚合。
pub fn completed_aggregate_intervals(
    strategy_start: DateTime<Utc>,
    closed_at: DateTime<Utc>,
) -> Vec<SyntheticKlineInterval> {
    AGGREGATE_INTERVALS
        .into_iter()
        .filter(|interval| {
            let interval_minutes = interval.minute_count() as i64;
            closed_at.timestamp().rem_euclid(interval_minutes * 60) == 0
                && closed_at - TimeDelta::minutes(interval_minutes) >= strategy_start
        })
        .collect()
}

/// 仅当前进程上一轮成功发布且当前槽恰好是下一分钟时，生成上一分钟闭合计划。
/// 连续在线只允许上下两次成功观测间隔不超过五秒；进程重启、版本切换或调度长暂停均不补历史。
pub fn build_online_minute_close_plan(
    previous: Option<&SyntheticRealtimePlan>,
    strategy_id: u64,
    config: &SyntheticMarketConfig,
    observed_at: DateTime<Utc>,
) -> AppResult<Option<SyntheticMinuteClosePlan>> {
    let Some(previous) = previous else {
        return Ok(None);
    };
    let current_open_time = current_minute_open_time(observed_at)?;
    let previous_open_time = previous.kline().open_time();
    if previous.strategy_id() != strategy_id
        || previous.version() != config.version
        || current_open_time != previous_open_time + TimeDelta::minutes(1)
        || observed_at <= previous.kline().observed_at()
        || observed_at - previous.kline().observed_at() > TimeDelta::seconds(5)
    {
        return Ok(None);
    }

    let closed = config
        .generate_1m(previous_open_time)
        .map_err(|error| AppError::Validation(error.to_string()))?;
    let kline = MarketKlineSnapshot::new(
        MarketDataProvider::Strategy,
        &config.symbol,
        "1m",
        closed.open_time,
        closed.values,
        observed_at,
    )
    .map_err(|error| AppError::Validation(error.to_string()))?;
    let closed_at = previous_open_time + TimeDelta::minutes(1);
    let aggregate_intervals = completed_aggregate_intervals(config.start_time, closed_at);

    Ok(Some(SyntheticMinuteClosePlan {
        kline,
        aggregate_intervals,
    }))
}

#[allow(clippy::too_many_arguments)]
async fn publish_minute_close(
    pool: &Pool<MySql>,
    mongo: &Database,
    ingestion: &MarketIngestionService,
    config: &SyntheticMarketConfig,
    close_plan: &SyntheticMinuteClosePlan,
    realtime_plan: &SyntheticRealtimePlan,
    owner: &str,
    observed_at: DateTime<Utc>,
) -> AppResult<()> {
    ensure_current_lease(pool, realtime_plan, owner, Utc::now()).await?;
    if ingestion
        .ingest_and_publish_synthetic_kline(close_plan.kline())
        .await?
        == SyntheticIngestionOutcome::RejectedStale
    {
        return Err(stale_market_write_conflict("closed 1m kline"));
    }
    for interval in close_plan.aggregate_intervals() {
        ensure_current_lease(pool, realtime_plan, owner, Utc::now()).await?;
        let window_end = close_plan.kline().open_time() + TimeDelta::minutes(1);
        if let Some(candles) = load_aggregate_window(mongo, config, *interval, window_end).await? {
            let snapshot =
                build_aggregate_kline_snapshot(&config.symbol, *interval, &candles, observed_at)?;
            if ingestion
                .ingest_and_publish_synthetic_kline(&snapshot)
                .await?
                == SyntheticIngestionOutcome::RejectedStale
            {
                return Err(stale_market_write_conflict("aggregate kline"));
            }
        }
    }
    Ok(())
}

async fn load_aggregate_window(
    mongo: &Database,
    config: &SyntheticMarketConfig,
    interval: SyntheticKlineInterval,
    window_end: DateTime<Utc>,
) -> AppResult<Option<Vec<SyntheticCandle>>> {
    let symbol = ValidatedMarketSymbol::from_raw(&config.symbol)
        .map_err(|error| AppError::Validation(error.to_string()))?;
    let collection = mongo.collection::<Document>(&kline_collection_name(&symbol));
    let window_start = window_end - TimeDelta::minutes(interval.minute_count() as i64);
    let mut cursor = collection
        .find(doc! {
            "interval": "1m",
            "open_time": {
                "$gte": BsonDateTime::from_millis(window_start.timestamp_millis()),
                "$lt": BsonDateTime::from_millis(window_end.timestamp_millis()),
            },
        })
        .sort(doc! { "open_time": 1 })
        .await?;
    let mut candles = Vec::with_capacity(interval.minute_count());
    while cursor.advance().await? {
        let document = cursor.deserialize_current()?;
        let open_time = document
            .get_datetime("open_time")
            .ok()
            .and_then(|value| DateTime::from_timestamp_millis(value.timestamp_millis()))
            .ok_or_else(|| {
                AppError::Validation("synthetic aggregate open_time is invalid".to_owned())
            })?;
        candles.push(SyntheticCandle {
            open_time,
            values: MarketKlineValues {
                open: document_decimal(&document, "open")?,
                high: document_decimal(&document, "high")?,
                low: document_decimal(&document, "low")?,
                close: document_decimal(&document, "close")?,
                volume: document_decimal(&document, "volume")?,
            },
        });
    }
    if candles.len() != interval.minute_count() {
        return Ok(None);
    }
    Ok(Some(candles))
}

/// 从已持久化并按开盘时间升序排列的权威 1m 窗口构造高周期快照；缺根、乱序或 OHLCV 非法均拒绝发布。
/// 本函数无 I/O，实时 worker 用它保证 Redis、Mongo 与 WebSocket 使用同一聚合结果。
pub fn build_aggregate_kline_snapshot(
    symbol: &str,
    interval: SyntheticKlineInterval,
    candles: &[SyntheticCandle],
    observed_at: DateTime<Utc>,
) -> AppResult<MarketKlineSnapshot> {
    let aggregate = aggregate_1m_candles(candles, interval)
        .map_err(|error| AppError::Validation(error.to_string()))?;
    MarketKlineSnapshot::new(
        MarketDataProvider::Strategy,
        symbol,
        interval.as_str(),
        aggregate.open_time,
        aggregate.values,
        observed_at,
    )
    .map_err(|error| AppError::Validation(error.to_string()))
}

async fn load_active_strategies(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    limit: u32,
    owner: &str,
) -> AppResult<Vec<SyntheticStrategyRow>> {
    sqlx::query_as::<_, SyntheticStrategyRow>(
        r#"SELECT strategies.id AS strategy_id,
                  pairs.symbol,
                  pairs.price_precision,
                  strategies.start_price,
                  strategies.target_price,
                  strategies.start_time,
                  strategies.end_time,
                  strategies.volatility,
                  strategies.volume_min,
                  strategies.volume_max,
                  versions.version,
                  versions.seed,
                  versions.config_json
           FROM market_strategies strategies
           INNER JOIN trading_pairs pairs ON pairs.id = strategies.pair_id
           INNER JOIN strategy_runs runs ON runs.strategy_id = strategies.id
           INNER JOIN strategy_versions versions
                   ON versions.strategy_id = strategies.id
                  AND versions.version = runs.active_version
           WHERE strategies.status = 'active'
             AND pairs.status = 'active'
             AND pairs.market_type IN ('strategy', 'internal')
             AND runs.run_status IN ('running', 'live')
             AND strategies.start_time <= ?
             AND strategies.end_time > ?
             AND (runs.lease_expires_at IS NULL OR runs.lease_expires_at <= ? OR runs.lease_owner = ?)
           ORDER BY strategies.id
           LIMIT ?"#,
    )
    .bind(now.naive_utc())
    .bind(now.naive_utc())
    .bind(now.naive_utc())
    .bind(owner)
    .bind(i64::from(limit.clamp(1, MAX_STRATEGIES_PER_ROUND)))
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

async fn acquire_lease(
    pool: &Pool<MySql>,
    strategy_id: u64,
    version: i32,
    owner: &str,
    now: DateTime<Utc>,
    expires_at: DateTime<Utc>,
) -> AppResult<bool> {
    let result = sqlx::query(
        r#"UPDATE strategy_runs runs
           INNER JOIN market_strategies strategies ON strategies.id = runs.strategy_id
           SET runs.active_version = ?, runs.lease_owner = ?, runs.lease_expires_at = ?
           WHERE runs.strategy_id = ?
             AND strategies.status = 'active'
             AND runs.run_status IN ('running', 'live')
             AND strategies.start_time <= ?
             AND strategies.end_time > ?
             AND (runs.lease_expires_at IS NULL OR runs.lease_expires_at <= ? OR runs.lease_owner = ?)
             AND runs.active_version = ?"#,
    )
    .bind(version)
    .bind(owner)
    .bind(expires_at.naive_utc())
    .bind(strategy_id)
    .bind(now.naive_utc())
    .bind(now.naive_utc())
    .bind(now.naive_utc())
    .bind(owner)
    .bind(version)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

async fn load_strategy_nodes(
    pool: &Pool<MySql>,
    strategy_id: u64,
) -> AppResult<Vec<SyntheticNodeRow>> {
    sqlx::query_as::<_, SyntheticNodeRow>(
        r#"SELECT target_time, target_type, target_value, execution_mode,
                  tolerance, volatility, volume_min, volume_max
           FROM market_strategy_nodes
           WHERE strategy_id = ?
           ORDER BY sequence_no"#,
    )
    .bind(strategy_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

fn strategy_config(
    row: &SyntheticStrategyRow,
    relation_nodes: Vec<SyntheticNodeRow>,
) -> AppResult<SyntheticMarketConfig> {
    let snapshot = &row.config_json.0;
    let nodes = match snapshot.get("nodes") {
        Some(Value::Array(nodes)) => nodes
            .iter()
            .map(config_node)
            .collect::<AppResult<Vec<_>>>()?,
        Some(_) => {
            return Err(AppError::Validation(
                "synthetic strategy version nodes must be an array".to_owned(),
            ));
        }
        None => relation_nodes
            .into_iter()
            .map(relation_node)
            .collect::<AppResult<Vec<_>>>()?,
    };
    let price_precision = u32::try_from(row.price_precision).map_err(|_| {
        AppError::Validation("synthetic strategy price precision must be non-negative".to_owned())
    })?;
    let version = u32::try_from(row.version).map_err(|_| {
        AppError::Validation("synthetic strategy version must be non-negative".to_owned())
    })?;
    SyntheticMarketConfig::new(SyntheticMarketConfig {
        symbol: row.symbol.clone(),
        seed: row.seed.clone(),
        version,
        price_precision,
        start_time: config_time(snapshot, "start_time", row.start_time)?,
        end_time: config_time(snapshot, "end_time", row.end_time)?,
        start_price: config_decimal(snapshot, "start_price", &row.start_price)?,
        target_price: config_decimal(snapshot, "target_price", &row.target_price)?,
        volatility: config_decimal(snapshot, "volatility", &row.volatility)?,
        volume_min: config_decimal(snapshot, "volume_min", &row.volume_min)?,
        volume_max: config_decimal(snapshot, "volume_max", &row.volume_max)?,
        nodes,
    })
    .map_err(|error| AppError::Validation(error.to_string()))
}

fn config_node(value: &Value) -> AppResult<SyntheticMarketNode> {
    Ok(SyntheticMarketNode {
        target_time: required_time(value, "target_time")?,
        target_type: parse_target_type(required_string(value, "target_type")?)?,
        target_value: required_decimal(value, "target_value")?,
        execution_mode: parse_execution_mode(required_string(value, "execution_mode")?)?,
        tolerance: required_decimal(value, "tolerance")?,
        volatility: required_decimal(value, "volatility")?,
        volume_min: optional_decimal(value, "volume_min")?,
        volume_max: optional_decimal(value, "volume_max")?,
    })
}

fn relation_node(row: SyntheticNodeRow) -> AppResult<SyntheticMarketNode> {
    Ok(SyntheticMarketNode {
        target_time: row.target_time,
        target_type: parse_target_type(&row.target_type)?,
        target_value: row.target_value,
        execution_mode: parse_execution_mode(&row.execution_mode)?,
        tolerance: row.tolerance,
        volatility: row.volatility,
        volume_min: row.volume_min,
        volume_max: row.volume_max,
    })
}

async fn load_ticker_history(
    mongo: &Database,
    symbol: &str,
    current_open_time: DateTime<Utc>,
) -> AppResult<Vec<MarketKlineValues>> {
    let symbol = ValidatedMarketSymbol::from_raw(symbol)
        .map_err(|error| AppError::Validation(error.to_string()))?;
    let collection = mongo.collection::<Document>(&kline_collection_name(&symbol));
    let options = FindOptions::builder()
        .sort(doc! { "open_time": 1 })
        .limit(TICKER_WINDOW_MINUTES)
        .build();
    let mut cursor = collection
        .find(doc! {
            "interval": "1m",
            "open_time": {
                "$gte": BsonDateTime::from_millis(
                    (current_open_time - TimeDelta::minutes(TICKER_WINDOW_MINUTES)).timestamp_millis()
                ),
                "$lt": BsonDateTime::from_millis(current_open_time.timestamp_millis()),
            },
        })
        .with_options(options)
        .await?;
    let mut history = Vec::new();
    while cursor.advance().await? {
        let document = cursor.deserialize_current()?;
        history.push(MarketKlineValues {
            open: document_decimal(&document, "open")?,
            high: document_decimal(&document, "high")?,
            low: document_decimal(&document, "low")?,
            close: document_decimal(&document, "close")?,
            volume: document_decimal(&document, "volume")?,
        });
    }
    Ok(history)
}

/// 在 Redis/Mongo/WS 副作用之间重新读取当前租约，确认 owner、active_version 与到期时间仍覆盖本次观察时刻。
/// 查询不续租；不存在匹配行即返回冲突，旧 owner 必须停止后续发布和检查点推进。
async fn ensure_current_lease(
    pool: &Pool<MySql>,
    plan: &SyntheticRealtimePlan,
    owner: &str,
    checked_at: DateTime<Utc>,
) -> AppResult<()> {
    let version = i32::try_from(plan.version()).map_err(|_| {
        AppError::Validation("synthetic strategy version exceeds database range".to_owned())
    })?;
    let current = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*)
           FROM strategy_runs
           WHERE strategy_id = ?
             AND lease_owner = ?
             AND lease_expires_at >= ?
             AND run_status IN ('running', 'live')
             AND active_version = ?"#,
    )
    .bind(plan.strategy_id())
    .bind(owner)
    .bind(checked_at.naive_utc())
    .bind(version)
    .fetch_one(pool)
    .await?;
    if current != 1 {
        return Err(AppError::Conflict(
            "synthetic strategy lease or version changed before publish".to_owned(),
        ));
    }
    Ok(())
}

fn stale_market_write_conflict(channel: &str) -> AppError {
    AppError::Conflict(format!(
        "synthetic {channel} rejected because a newer market snapshot already exists"
    ))
}

async fn update_checkpoint(
    pool: &Pool<MySql>,
    plan: &SyntheticRealtimePlan,
    owner: &str,
    observed_at: DateTime<Utc>,
    lease_expires_at: DateTime<Utc>,
) -> AppResult<()> {
    let result = sqlx::query(
        r#"UPDATE strategy_runs
           SET active_version = ?,
               current_price = ?,
               last_tick_at = ?,
               last_generated_at = ?,
               last_kline_open_time = ?,
               recovery_status = 'live',
               error_message = NULL,
               lease_expires_at = ?
           WHERE strategy_id = ?
             AND lease_owner = ?
             AND lease_expires_at >= ?
             AND run_status IN ('running', 'live')
             AND active_version = ?
             AND (last_tick_at IS NULL OR last_tick_at <= ?)
             AND (last_kline_open_time IS NULL OR last_kline_open_time <= ?)"#,
    )
    .bind(i32::try_from(plan.version()).map_err(|_| {
        AppError::Validation("synthetic strategy version exceeds database range".to_owned())
    })?)
    .bind(plan.ticker().last_price())
    .bind(observed_at.naive_utc())
    .bind(observed_at.naive_utc())
    .bind(plan.kline().open_time().naive_utc())
    .bind(lease_expires_at.naive_utc())
    .bind(plan.strategy_id())
    .bind(owner)
    .bind(observed_at.naive_utc())
    .bind(i32::try_from(plan.version()).map_err(|_| {
        AppError::Validation("synthetic strategy version exceeds database range".to_owned())
    })?)
    .bind(observed_at.naive_utc())
    .bind(plan.kline().open_time().naive_utc())
    .execute(pool)
    .await?;
    if result.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "synthetic strategy lease or version changed before checkpoint".to_owned(),
        ));
    }
    Ok(())
}

async fn mark_strategy_error(
    pool: &Pool<MySql>,
    strategy_id: u64,
    version: i32,
    owner: &str,
    message: &str,
) {
    let message = message.chars().take(1024).collect::<String>();
    if let Err(error) = sqlx::query(
        "UPDATE strategy_runs SET error_message = ? WHERE strategy_id = ? AND active_version = ? AND lease_owner = ?",
    )
    .bind(message)
    .bind(strategy_id)
    .bind(version)
    .bind(owner)
    .execute(pool)
    .await
    {
        warn!(strategy_id, %error, "记录模拟行情策略错误失败");
    }
}

fn current_minute_open_time(now: DateTime<Utc>) -> AppResult<DateTime<Utc>> {
    DateTime::<Utc>::from_timestamp(now.timestamp().div_euclid(60) * 60, 0).ok_or_else(|| {
        AppError::Validation("synthetic market timestamp is out of range".to_owned())
    })
}

fn config_time(value: &Value, key: &str, fallback: DateTime<Utc>) -> AppResult<DateTime<Utc>> {
    match value.get(key) {
        Some(value) => value_time(value, key),
        None => Ok(fallback),
    }
}

fn required_time(value: &Value, key: &str) -> AppResult<DateTime<Utc>> {
    value
        .get(key)
        .ok_or_else(|| AppError::Validation(format!("synthetic node {key} is required")))
        .and_then(|value| value_time(value, key))
}

fn value_time(value: &Value, key: &str) -> AppResult<DateTime<Utc>> {
    if let Some(millis) = value.as_i64() {
        return DateTime::<Utc>::from_timestamp_millis(millis).ok_or_else(|| {
            AppError::Validation(format!("synthetic strategy {key} is out of range"))
        });
    }
    if let Some(raw) = value.as_str() {
        return DateTime::parse_from_rfc3339(raw)
            .map(|time| time.with_timezone(&Utc))
            .map_err(|error| {
                AppError::Validation(format!("synthetic strategy {key} is invalid: {error}"))
            });
    }
    Err(AppError::Validation(format!(
        "synthetic strategy {key} must be milliseconds or RFC3339"
    )))
}

fn config_decimal(value: &Value, key: &str, fallback: &BigDecimal) -> AppResult<BigDecimal> {
    match value.get(key) {
        Some(value) => value_decimal(value, key),
        None => Ok(fallback.clone()),
    }
}

fn required_decimal(value: &Value, key: &str) -> AppResult<BigDecimal> {
    value
        .get(key)
        .ok_or_else(|| AppError::Validation(format!("synthetic node {key} is required")))
        .and_then(|value| value_decimal(value, key))
}

fn optional_decimal(value: &Value, key: &str) -> AppResult<Option<BigDecimal>> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value_decimal(value, key).map(Some),
    }
}

fn value_decimal(value: &Value, key: &str) -> AppResult<BigDecimal> {
    let raw = value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string());
    BigDecimal::from_str(&raw).map_err(|error| {
        AppError::Validation(format!("synthetic strategy {key} is invalid: {error}"))
    })
}

fn required_string<'a>(value: &'a Value, key: &str) -> AppResult<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Validation(format!("synthetic node {key} must be a string")))
}

fn parse_target_type(value: &str) -> AppResult<SyntheticTargetType> {
    match value {
        "absolute_price" => Ok(SyntheticTargetType::AbsolutePrice),
        "percent_from_start" => Ok(SyntheticTargetType::PercentFromStart),
        "percent_from_previous" => Ok(SyntheticTargetType::PercentFromPrevious),
        _ => Err(AppError::Validation(format!(
            "unsupported synthetic target type: {value}"
        ))),
    }
}

fn parse_execution_mode(value: &str) -> AppResult<SyntheticExecutionMode> {
    match value {
        "hard" => Ok(SyntheticExecutionMode::Hard),
        "soft" => Ok(SyntheticExecutionMode::Soft),
        "range" => Ok(SyntheticExecutionMode::Range),
        _ => Err(AppError::Validation(format!(
            "unsupported synthetic execution mode: {value}"
        ))),
    }
}

fn document_decimal(document: &Document, key: &str) -> AppResult<BigDecimal> {
    let raw = document.get_str(key).map_err(|error| {
        AppError::Validation(format!("synthetic history {key} is invalid: {error}"))
    })?;
    BigDecimal::from_str(raw).map_err(|error| {
        AppError::Validation(format!("synthetic history {key} is invalid: {error}"))
    })
}

fn worker_owner() -> &'static str {
    static OWNER: OnceLock<String> = OnceLock::new();
    OWNER.get_or_init(|| format!("synthetic:{}:{}", std::process::id(), Uuid::now_v7()))
}
