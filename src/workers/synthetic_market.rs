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
        MarketTickerValues, SyntheticCandle, SyntheticKlineInterval, SyntheticMarketConfig,
        SyntheticMarketNode, SyntheticStrategySnapshot, ValidatedMarketSymbol,
        adapters::{MarketIngestionService, SyntheticIngestionOutcome},
        aggregate_1m_candles, synthetic_config_from_snapshot, synthetic_execution_mode_from_code,
        synthetic_target_type_from_code,
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
    /// 这里是整分钟的最终 OHLCV 而非形成中快照，与手动补偿对同一槽位生成的结果逐字段相同。
    pub fn kline(&self) -> &MarketKlineSnapshot {
        &self.kline
    }

    /// 返回刚完成边界对应的高周期；仅完整窗口会出现在结果中，不会为停机缺口枚举中间槽。
    /// 列表可能为空，表示本次闭合没有跨过任何高周期边界，此时调用方只需写入这一根 1m。
    pub fn aggregate_intervals(&self) -> &[SyntheticKlineInterval] {
        &self.aggregate_intervals
    }
}

impl SyntheticRealtimePlan {
    /// 返回计划所属策略 ID；该标识只用于租约和检查点，不进入公共行情 payload。
    /// 每次产生副作用之前都要用它重新核对租约，确认本进程仍是该策略的当前持有者。
    pub fn strategy_id(&self) -> u64 {
        self.strategy_id
    }

    /// 返回生成计划使用的不可变配置版本；检查点推进会再次比对该版本，避免旧计划覆盖新配置。
    /// 版本变更意味着价格路径被重新定义，跨版本的计划一律作废，不允许继续沿用旧计划发布。
    pub fn version(&self) -> u32 {
        self.version
    }

    /// 返回当前分钟权威 1m K 线；调用方应先 ingestion 成功，再推进策略检查点。
    /// 该快照按观察秒截断为形成中蜡烛，只有落在整分钟第 59 秒时才等于最终闭合值。
    pub fn kline(&self) -> &MarketKlineSnapshot {
        &self.kline
    }

    /// 返回与 K 线收盘价一致的 ticker 及历史窗口统计；该值不得由恢复任务用于倒退实时缓存。
    /// 最新价直接取自同一根形成中蜡烛的收盘价，24 小时高低价与成交量已叠加历史 1m 窗口。
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

/// 从应用状态取出 MySQL、Mongo 与统一 ingestion 后，用调用方指定的租约 owner 执行一轮当前分钟发布。
/// 三个依赖缺一不可，缺失时在扫描策略之前就返回内部错误，不会留下任何行情写入。
/// `continuity` 由调用方跨轮持有，用来判断本进程是否连续在线，进而决定是否补写上一分钟的闭合蜡烛。
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

/// 单轮调度主体：先拒绝空白 owner，再把当前时刻对齐到 UTC 整分钟，然后按策略 ID 顺序处理本轮可见的活跃策略。
/// 每个策略先做租约 CAS，抢不到计入 skipped 并跳过，抢到计入 leased 后才进入发布流程。
/// 发布成功的计划写回 `continuity` 供下一轮判断连续性并计入 published；失败只写该策略的 `error_message`
/// 并计入 failed，既不中断本轮其余策略，也不回滚这一项已经落地的行情。
/// 计数关系固定为 leased 加 skipped 等于 scanned、published 加 failed 等于 leased，可据此核对本轮是否有漏项。
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

/// 处理一个已抢到租约的策略：解析节点与版本快照、读取 24 小时 1m 历史，再生成本分钟的统一发布计划。
/// ticker 是整轮的提交门，它的 Redis 时序 CAS 一旦拒写就直接返回冲突，本轮不会写任何 K 线、
/// 不触发现货限价单，也不推进检查点；只有 ticker 落地后才依次补写上一分钟闭合蜡烛、高周期聚合与当前形成中 1m。
/// 每个副作用之前都重新核对租约的 owner、运行状态与 `active_version`，被抢走租约的旧实例会在这里被拦下。
/// 跨 MySQL、Mongo、Redis 没有事务，中途失败会留下部分写入，依靠同槽 upsert 与时序 CAS 在后续轮次收敛。
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
/// 开盘价取历史窗口首根的开盘价，窗口为空时退回本分钟开盘价；该值非正直接返回校验错误，避免涨跌幅除零。
/// 24 小时高低价与成交量由形成中蜡烛叠加全部历史 1m 得到，涨跌额和涨跌幅按最新价对该开盘价计算。
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
/// 观察时刻必须落在该分钟之内，否则返回校验错误；路径分三段各 20 秒，先走首个极值再走另一极值，最后回到收盘价。
/// 收涨时先探低后探高，收跌时相反；高低价只在对应极值被越过后才纳入，成交量按已观察秒数占比线性摊分。
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

/// 在起点与终点之间按已过份额做线性插值，用于把整分钟极值拆成逐秒推进的形成中价格。
/// 计算保持 `BigDecimal` 全精度并不在此取整；`total` 由调用方固定为 20 秒一段，份额不会越界。
fn interpolate_decimal(
    start: &BigDecimal,
    end: &BigDecimal,
    elapsed: i64,
    total: i64,
) -> BigDecimal {
    start + ((end - start) * BigDecimal::from(elapsed) / BigDecimal::from(total))
}

/// 根据本次闭合时刻返回需要重建的完整高周期；策略尚未运行满一个窗口时不会声明该聚合。
/// 判定有两条：闭合时刻正好落在该周期的 UTC 边界上，且往前推一个完整窗口不早于策略起始时间。
/// 返回顺序固定为 5m 到 1d 且可能为空；本函数只做时间判断，不读存储，也不检查窗口内是否真有蜡烛。
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
/// 策略 ID 与上一轮不符、版本已变或观测时间没有前进时同样返回空计划，宁可少补也不写出可能属于旧版本的历史。
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

/// 补齐上一分钟的最终形态：先写确定性闭合 1m，再按需重建本次跨过边界的高周期蜡烛。
/// 每次写入之前都重新核对租约，任一步被时序 CAS 判定为陈旧就立即返回冲突，不再继续写后续周期。
/// 高周期从 Mongo 重读权威 1m 窗口，缺根时静默跳过该周期而不报错，留给手动补偿稍后重建。
/// 每次成功写入都会经统一 ingestion 广播 WebSocket；本函数不推进检查点，那一步由调用方在最后完成。
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

/// 从该交易对的 Mongo 集合读取指定高周期窗口内的全部 1m，按开盘时间升序返回。
/// 窗口是左闭右开区间，起点由窗口结束时间往前推该周期的分钟数得到，与 UTC 边界严格对齐。
/// 根数不足视为窗口不完整并返回 `None`，调用方据此跳过该周期，而不是写出一根残缺聚合。
/// 文档中的开盘时间或 OHLCV 字段无法解析时返回校验错误；本函数只读 Mongo，不写入也不广播。
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
/// 聚合失败一律转成校验错误；`observed_at` 由调用方传入而不取本机时间，以便与同轮其他快照共用同一时序判定。
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

/// 扫描本轮可处理的策略：要求策略与交易对均为 active、市场类型属于 strategy 或 internal、
/// 运行状态为 running 或 live 且当前时刻落在起止区间内，并按 `active_version` 关联到对应版本快照。
/// 租约为空、已过期或本就属于自己三者之一才会返回，因此被其他实例持有中的策略不会进入本轮。
/// 结果按策略 ID 升序且条数夹紧到 1 至 100，使并发实例的处理顺序一致；本查询只读，不占用租约。
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

/// 用单条 UPDATE 完成租约 CAS：只有策略仍活跃、运行状态合法、当前落在起止区间内，
/// 且租约为空、已过期或属于自己，同时 `active_version` 仍等于扫描时读到的版本，才写入新的 owner 与到期时间。
/// 影响行数恰为 1 表示本进程取得租约，否则说明版本已切换或已被其他实例抢占，调用方必须跳过该策略。
/// 活实例每轮续租，崩溃实例最多让策略停摆一个租约周期；本语句只改租约字段，不推进任何检查点。
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

/// 按 `sequence_no` 升序读取策略的关系表节点，作为版本快照未内嵌节点时的回退来源。
/// 只取生成所需的目标时间、类型、目标值、执行模式、容差、波动率与成交量上下限，不含审计字段。
/// 顺序直接决定相对前节点类型的换算基准，因此排序必须稳定；本查询只读，也不校验节点合法性。
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

/// 把数据库行与版本快照 JSON 合成一份可用于确定性生成的配置，并交由领域构造器统一校验。
/// 节点优先取版本快照中的 `nodes` 数组，该字段存在但不是数组直接报错，只有字段缺失才回退到关系表节点。
/// 起止时间、起始价、目标价、波动率与成交量区间同样以快照为准，快照未覆盖的键才回退到策略主表当前值。
/// 价格精度或版本号转换为无符号失败时返回校验错误；本函数不访问存储，所需数据由调用方一次性读齐。
fn strategy_config(
    row: &SyntheticStrategyRow,
    relation_nodes: Vec<SyntheticNodeRow>,
) -> AppResult<SyntheticMarketConfig> {
    let fallback_nodes = relation_nodes
        .into_iter()
        .map(relation_node)
        .collect::<AppResult<Vec<_>>>()?;
    synthetic_config_from_snapshot(SyntheticStrategySnapshot {
        symbol: row.symbol.clone(),
        seed: row.seed.clone(),
        version: row.version,
        price_precision: row.price_precision,
        config_json: row.config_json.0.clone(),
        fallback_start_time: row.start_time,
        fallback_end_time: row.end_time,
        fallback_start_price: row.start_price.clone(),
        fallback_target_price: row.target_price.clone(),
        fallback_volatility: row.volatility.clone(),
        fallback_volume_min: row.volume_min.clone(),
        fallback_volume_max: row.volume_max.clone(),
        fallback_nodes,
    })
    .map_err(|error| AppError::Validation(error.to_string()))
}

/// 把关系表节点行转为领域节点，字段形态已由列类型保证，只需再解析目标类型和执行模式两个枚举文本。
/// 成交量上下限保持数据库中的可空语义原样传递；未知枚举文本返回校验错误，不会静默降级成默认模式。
fn relation_node(row: SyntheticNodeRow) -> AppResult<SyntheticMarketNode> {
    Ok(SyntheticMarketNode {
        target_time: row.target_time,
        target_type: synthetic_target_type_from_code(&row.target_type)
            .map_err(|error| AppError::Validation(error.to_string()))?,
        target_value: row.target_value,
        execution_mode: synthetic_execution_mode_from_code(&row.execution_mode)
            .map_err(|error| AppError::Validation(error.to_string()))?,
        tolerance: row.tolerance,
        volatility: row.volatility,
        volume_min: row.volume_min,
        volume_max: row.volume_max,
    })
}

/// 读取当前分钟之前最多 24 小时的权威 1m，用于折算 ticker 的开盘价、24 小时高低价与成交量。
/// 查询区间左闭右开且不含当前分钟，按开盘时间升序并限制在 1440 条内，因此首条即窗口起始蜡烛。
/// 窗口内缺根不会报错，统计只基于已存在的蜡烛；OHLCV 字段无法解析为十进制时返回校验错误。
/// 本函数只读 Mongo，既不补写缺口，也不改动任何缓存或检查点。
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
/// 版本号超出数据库列范围时先返回校验错误；命中行数不等于 1 一律按冲突处理，不区分被抢占还是状态变更。
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

/// 为被时序 CAS 拒写的通道生成统一的冲突错误，消息里带上 ticker、kline 等通道名便于定位。
/// 出现该错误说明存储中已有更新的快照，本轮必须放弃后续副作用与检查点推进，而不是重试覆盖。
fn stale_market_write_conflict(channel: &str) -> AppError {
    AppError::Conflict(format!(
        "synthetic {channel} rejected because a newer market snapshot already exists"
    ))
}

/// 在本轮全部行情副作用成功后推进策略检查点：写入最新价、最后 tick 时间和最后 K 线开盘时间，
/// 把恢复状态标回 live、清空 `error_message`，同时把租约续期到本轮计算出的到期时刻。
/// WHERE 条件同时约束 owner、租约未过期、运行状态与 `active_version`，并要求既有的 `last_tick_at`
/// 和 `last_kline_open_time` 都不晚于本次值，因此迟到的旧计划无法让检查点倒退。
/// 影响行数不为 1 一律按冲突返回；此时行情可能已经写入并广播，检查点留待后续轮次重新推进。
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

/// 把单个策略的失败原因写入 `error_message`，供后台排查本轮为何没有产出行情。
/// 消息按字符截断到 1024，避免超长错误撑爆列宽；更新同时限定策略、版本与 owner，不会覆盖他人写下的错误。
/// 本函数吞掉自身的数据库错误并只记录告警，因为它处在失败处理路径上，不能让记录失败再次中断本轮扫描。
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

/// 把任意时刻向下取整到所属的 UTC 整分钟，作为本轮生成与写入使用的槽位起点。
/// 取整使用欧几里得除法，1970 之前的时间同样向下对齐；时间戳超出可表示范围时返回校验错误。
fn current_minute_open_time(now: DateTime<Utc>) -> AppResult<DateTime<Utc>> {
    DateTime::<Utc>::from_timestamp(now.timestamp().div_euclid(60) * 60, 0).ok_or_else(|| {
        AppError::Validation("synthetic market timestamp is out of range".to_owned())
    })
}

/// 从 Mongo 文档按字段名读出十进制字符串并解析，字段类型不是字符串或内容非法都返回校验错误。
/// 历史 1m 的 OHLCV 一律以字符串存储，本函数是聚合窗口与 ticker 统计读取这些数值的唯一入口。
fn document_decimal(document: &Document, key: &str) -> AppResult<BigDecimal> {
    let raw = document.get_str(key).map_err(|error| {
        AppError::Validation(format!("synthetic history {key} is invalid: {error}"))
    })?;
    BigDecimal::from_str(raw).map_err(|error| {
        AppError::Validation(format!("synthetic history {key} is invalid: {error}"))
    })
}

/// 返回本进程稳定不变的租约 owner，由 synthetic 前缀、进程号和一次性 UUID v7 拼成。
/// 进程内只初始化一次，保证所有轮次用同一标识续租；换进程必然换标识，旧租约只能等待到期释放。
fn worker_owner() -> &'static str {
    static OWNER: OnceLock<String> = OnceLock::new();
    OWNER.get_or_init(|| format!("synthetic:{}:{}", std::process::id(), Uuid::now_v7()))
}
