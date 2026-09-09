//! 后台 K 线补偿任务，包含自动缺口扫描与管理员手动补偿两条彼此独立的路径。
//!
//! 自动路径按 `strategy_runs` 的连续检查点扫描已闭合的 1m 槽位，先与 Mongo 权威历史做差集，
//! 再复用实时发布和后台预览的 active-version 确定性生成器补齐实际缺根；写入成功后
//! 只以旧检查点和版本为乐观条件推进 MySQL 历史位置。手动路径由后台显式触发，
//! 补写指定槽位并重建受影响的完整聚合窗口，全程不写 MySQL。
//!
//! 两条路径都只写 Mongo K 线集合，一律以 `interval` 加 `open_time` 幂等 upsert，同键重放只覆盖不新增。
//! 本文件不写 Redis 行情缓存、不推进实时 checkpoint、不触发现货限价单，也不广播任何 WebSocket 事件。

use crate::{
    error::{AppError, AppResult},
    infra::mongo::{ensure_kline_indexes, kline_collection_name},
    modules::market::{
        KlineUpsertKey, SyntheticCandle, SyntheticKlineInterval, SyntheticMarketConfig,
        ValidatedMarketSymbol, aggregate_1m_candles,
        infrastructure::list_compatible_one_minute_kline_open_times,
    },
    state::AppState,
};
use bigdecimal::{BigDecimal, ToPrimitive};
use chrono::{DateTime, Duration, TimeDelta, Timelike, Utc};
use mongodb::{
    Database,
    bson::{DateTime as BsonDateTime, Document, doc},
    options::{FindOptions, UpdateOptions},
};
use sqlx::{MySql, Pool};
use std::str::FromStr;
use thiserror::Error;
use tracing::warn;

const MAX_CANDLES_PER_STRATEGY_RUN: usize = 500;
pub const MAX_MANUAL_RECOVERY_1M_CANDLES: usize = 10_080;
const MANUAL_RECOVERY_INTERVALS: [SyntheticKlineInterval; 5] = [
    SyntheticKlineInterval::FiveMinutes,
    SyntheticKlineInterval::FifteenMinutes,
    SyntheticKlineInterval::OneHour,
    SyntheticKlineInterval::FourHours,
    SyntheticKlineInterval::OneDay,
];

/// 自动补偿写入 Mongo 的版本证据。Mongo 整数是有符号类型，因此在任何 I/O 前验证策略 ID。
#[derive(Debug, Clone, Copy)]
struct AutomaticRecoveryProvenance {
    strategy_id: i64,
    strategy_version: i32,
}

impl AutomaticRecoveryProvenance {
    fn new(strategy_id: u64, strategy_version: i32) -> AppResult<Self> {
        Ok(Self {
            strategy_id: i64::try_from(strategy_id).map_err(|_| {
                AppError::Validation("strategy id exceeds Mongo signed integer range".to_owned())
            })?,
            strategy_version,
        })
    }
}

/// 自动 K 线缺口恢复任务的调用入口，自身不持有状态，所需依赖在每次执行时由调用方传入。
pub struct KlineRecoveryWorker;

/// 手动补偿返回的实际 Mongo 写入进度；根数按已成功执行的幂等 upsert 次数计算。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ManualKlineRecoveryCounts {
    pub actual_1m_count: u32,
    pub actual_aggregate_count: u32,
    pub skipped_aggregate_count: u32,
}

/// 手动补偿执行错误同时携带失败前已落地的实际进度，供任务终态审计。
#[derive(Debug)]
pub struct ManualKlineRecoveryError {
    counts: ManualKlineRecoveryCounts,
    source: Box<AppError>,
}

impl ManualKlineRecoveryError {
    /// 返回错误发生前已成功写入的 1m 与聚合根数，不把未确认的写入算入进度。
    /// 手动补偿没有事务，这些已落地的蜡烛不会回滚，任务终态必须据此如实记录部分完成的进度。
    pub fn counts(&self) -> ManualKlineRecoveryCounts {
        self.counts
    }
}

impl std::fmt::Display for ManualKlineRecoveryError {
    /// 直接透传内部 `AppError` 的展示文本，不追加进度信息，使日志与既有错误消息保持一致。
    /// 需要已写入根数时应改用 `counts`，两者刻意分开，避免错误文本被进度数字污染。
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.source.fmt(formatter)
    }
}

impl std::error::Error for ManualKlineRecoveryError {
    /// 保留被装箱 `AppError` 的错误链，便于日志和诊断工具继续读取底层错误类别。
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

impl KlineRecoveryWorker {
    /// 执行一轮 K 线缺口恢复；策略扫描上限收敛到 1..=100，每个策略最多生成 500 根已闭合 K 线。
    /// Mongo 以 interval+open_time 幂等 upsert，成功写入后才以旧值乐观推进 MySQL 检查点；单策略失败继续后项。
    pub async fn run_once(
        &self,
        state: &AppState,
        now: DateTime<Utc>,
        limit: u32,
    ) -> AppResult<KlineRecoverySummary> {
        run_once(state, now, limit).await
    }
}

/// 单轮自动恢复的统计口径：`recovered_candles` 按蜡烛根数累计，其余三项都按策略计数。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KlineRecoverySummary {
    pub scanned: u32,
    pub recovered_candles: u32,
    pub skipped: u32,
    pub failed: u32,
}

/// 单个策略在本轮的结局：补写了多少根、因无缺口或检查点已被推进而跳过，或者执行失败。
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

/// 汇总一轮恢复计划结果；只聚合计数，不改变检查点或持久化 K 线。
/// scanned 等于本轮处理的策略数，recovered_candles 累加的是蜡烛根数，skipped 与 failed 才按策略计数，
/// 因此三项之和不应直接拿去和 scanned 比较。检查点被并发推进的情形归入 skipped，不计为失败。
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
    /// 暴露检查点之后至最近完整周期之间按时间升序排列的缺失开盘时间，恢复器据此生成确定顺序的蜡烛并推进检查点。
    pub fn missing_open_times(&self) -> &[DateTime<Utc>] {
        &self.missing_open_times
    }

    /// 判断当前策略是否存在需要恢复的完整周期；空缺口应跳过 Mongo 写入和 MySQL 检查点竞争，而不是记作失败。
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
    /// 构造一次可执行的恢复策略快照；交易对和小数参数在扫描后、写入前完成校验，拒绝非正价格、负波动/成交量及倒置的成交量区间。
    /// 检查点定义恢复起点，当前价与目标价定义缺口区间的价格轨迹；本步骤不访问存储，也不推进策略状态。
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

    /// 用已解析的十进制参数构造恢复策略快照，并在此集中完成全部数值与交易对校验。
    /// 当前价与目标价必须为正，波动率和成交量上下限不得为负，上限不得小于下限，任一条不满足都返回校验错误。
    /// 交易对经规范化处理，格式非法同样在这里被拒绝，避免脏 symbol 影响后续 Mongo 集合名。
    /// 本函数不访问任何存储，也不改变策略状态，只把一行数据收敛成可安全执行的恢复输入。
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KlineRecoveryPlan {
    strategy_id: u64,
    symbol: String,
    interval: String,
    candles: Vec<KlineRecoveryCandle>,
}

impl KlineRecoveryPlan {
    /// 从策略检查点生成截至最近完整周期的缺失 K 线，最多生成受控数量并保证末根收盘价命中目标价。
    /// 输入价格必须为正、波动和成交量非负；仅构造恢复计划，不写 Mongo 或推进 MySQL 检查点。
    /// 缺口为空时返回空批次而不是报错，调用方据此跳过该策略；单批根数由缺口枚举上限收敛到 500。
    /// 每根开盘价取前一根收盘价，收盘价按当前价到目标价均分推进，末根强制等于目标价并使用成交量上限。
    /// 最高最低价由开收极值加减波动率得到，因此波动率为零时生成的蜡烛没有上下影线。
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

    /// 标识本恢复计划所属的策略行，用于乐观推进同一策略检查点并归属单策略失败记录。
    /// 该 ID 只在 MySQL 侧使用，不会写入 Mongo 蜡烛文档，也不出现在任何对外行情字段中。
    pub fn strategy_id(&self) -> u64 {
        self.strategy_id
    }

    /// 标识整批恢复蜡烛的规范化交易对，决定 Mongo 集合分区并用于恢复日志关联。
    /// 一个计划只服务一个交易对，因此批内所有蜡烛必然落到同一个集合，不存在跨市场混写。
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// 标识本计划所有蜡烛共享的周期；它与开盘时间共同构成 Mongo 幂等 upsert 键。
    /// 自动扫描路径固定按一分钟间隔构造计划，因此该值实际恒为 1m，高周期只能由手动补偿重建。
    pub fn interval(&self) -> &str {
        &self.interval
    }

    /// 提供按开盘时间排序的完整恢复批次；空批次表示检查点后没有已闭合缺口，worker 应跳过写入和检查点推进。
    pub fn candles(&self) -> &[KlineRecoveryCandle] {
        &self.candles
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
    /// 构造一根待恢复 K 线并校验交易对及 interval+open_time 幂等键；价格与成交量文本保持策略计算结果，供 Mongo 原样写入。
    /// 此阶段只形成持久化命令，不访问 Mongo；同键重放由后续 upsert 覆盖而不新增重复蜡烛。
    /// 周期不在支持白名单内会在构造时失败；本函数不检查开盘时间是否对齐该周期，对齐由缺口枚举负责保证。
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

    /// 提供已校验的交易对分区标识，确保恢复写入只落到该市场对应的 K 线集合。
    /// 该值在构造蜡烛时就已完成规范化，写入前无需再次裁剪、去分隔符或转换大小写。
    pub fn symbol(&self) -> &ValidatedMarketSymbol {
        &self.symbol
    }

    /// 依据规范化交易对生成 Mongo 集合名，使恢复写入与实时行情使用相同的市场分区规则。
    /// 这里复用共享的命名入口而不是本地拼接字符串，因此补偿写入与实时摄取永远落在同一个集合。
    pub fn collection_name(&self) -> String {
        kline_collection_name(&self.symbol)
    }

    /// 标识这根蜡烛所属的 UTC 开盘槽位；恢复完成后最后一个槽位用于乐观推进策略检查点。
    /// 槽位由缺口计算给出且已按周期对齐，写入时原样进入幂等键，不会再做一次取整。
    pub fn open_time(&self) -> DateTime<Utc> {
        self.open_time
    }

    /// 提供恢复后的收盘价，作为策略检查点推进后的最新价格，保证下一轮从本轮末价继续而非重新插值。
    pub fn close(&self) -> &str {
        &self.close
    }

    /// 构造 Mongo 幂等选择条件；周期与 UTC 开盘时间共同锁定同一根逻辑蜡烛，重放不得插入第二条记录。
    /// 时间按毫秒转成 BSON 时间，与集合上的唯一索引口径一致，因此并发重放最终收敛到同一条文档。
    pub fn upsert_filter(&self) -> Document {
        doc! {
            "interval": &self.interval,
            "open_time": BsonDateTime::from_millis(self.open_time.timestamp_millis()),
        }
    }

    /// 构造恢复蜡烛的完整 `$set` 更新；同键重试会用同一计划数据收敛覆盖，且不会提前推进 MySQL 策略检查点。
    /// OHLCV 按字符串原样写入以保留计划算出的十进制精度，一次性覆盖全部字段，不做逐字段比较或部分更新。
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

/// 从应用状态取得 MySQL 策略检查点与 Mongo K 线存储后执行单轮恢复；任一权威存储缺失时在扫描前失败。
/// 单轮最多处理 1..=100 个策略、每个策略最多 500 根 K 线；本入口不广播实时行情或写 outbox。
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

/// 按策略 ID 扫描至多 `limit` 收敛后的 100 个到期策略，每项检查最多 500 个截至最近闭合分钟的槽位。
/// 实际缺根先与 Mongo 权威 1m 做差集，再使用实时 worker 的 active version 确定性配置补写并重建完整高周期；
/// 成功后只乐观推进连续恢复检查点，不覆盖实时 `current_price/last_tick_at/last_generated_at`。
/// 单策略校验、Mongo 或检查点失败记录后继续后项；本 worker 不发布 WebSocket、Redis 或 outbox 事件。
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
        let outcome = match recover_detected_strategy_gap(pool, mongo, &row, now).await {
            Ok(0) => KlineRecoveryPlanSummary::Skipped,
            Ok(candles) => KlineRecoveryPlanSummary::Recovered { candles },
            Err(KlineRecoveryCheckpointError::AlreadyAdvanced) => {
                warn!(strategy_id, "K 线恢复检查点或配置版本已被推进");
                KlineRecoveryPlanSummary::Skipped
            }
            Err(KlineRecoveryCheckpointError::App(error)) => {
                warn!(strategy_id, %error, "K 线恢复计划无效");
                mark_recovery_failed(pool, &row, &error.to_string()).await;
                KlineRecoveryPlanSummary::Failed
            }
        };
        outcomes.push(outcome);
    }

    Ok(summarize_recovery_plans(&outcomes))
}

/// 对单策略检查连续的有界分钟窗口，只补 Mongo 中实际缺少的槽位。
///
/// 候选窗口包含旧检查点自身，因为新建策略会先把起始时刻写入检查点、随后才可能产生首根 K 线；
/// 已存在的检查点根会被 Mongo 差集排除。补写复用实时 active version 的确定性生成器，成功后
/// 检查点推进到本轮最后检查的槽位；即使本轮没有缺根也要推进，从而收敛已由人工补好的历史。
async fn recover_detected_strategy_gap(
    pool: &Pool<MySql>,
    mongo: &Database,
    row: &DueKlineRecoveryRun,
    now: DateTime<Utc>,
) -> Result<u32, KlineRecoveryCheckpointError> {
    let recovery_until = last_closed_open_time(now, TimeDelta::minutes(1))?.min(
        align_open_time(
            row.strategy_end_time - TimeDelta::microseconds(1),
            TimeDelta::minutes(1),
        )
        .map_err(|error| AppError::Validation(error.to_string()))?,
    );
    let scan_times = recovery_scan_open_times(row.checkpoint_open_time, recovery_until);
    let Some(first_open_time) = scan_times.first().copied() else {
        return Ok(0);
    };
    let last_open_time = scan_times.last().copied().expect("non-empty scan times");
    let range_end = last_open_time + TimeDelta::minutes(1);
    let provenance = AutomaticRecoveryProvenance::new(row.strategy_id, row.active_version)?;
    let existing = list_compatible_one_minute_kline_open_times(
        mongo,
        &row.symbol,
        first_open_time,
        range_end,
        row.strategy_id,
        row.active_version,
    )
    .await?
    .into_iter()
    .collect::<std::collections::HashSet<_>>();
    let missing = scan_times
        .iter()
        .copied()
        .filter(|open_time| !existing.contains(open_time))
        .collect::<Vec<_>>();

    let recovered_candles = if missing.is_empty() {
        0
    } else {
        let (version, config) =
            super::synthetic_market::load_strategy_config_for_recovery(pool, row.strategy_id)
                .await?;
        if version != row.active_version {
            return Err(KlineRecoveryCheckpointError::AlreadyAdvanced);
        }
        execute_synthetic_recovery(mongo, &config, &missing, now, Some(provenance))
            .await
            .map_err(|error| AppError::Internal(format!("automatic kline recovery: {error}")))?
            .actual_1m_count
    };

    update_recovery_checkpoint(
        pool,
        row.strategy_id,
        row.active_version,
        row.checkpoint_open_time,
        last_open_time,
        last_open_time >= recovery_until,
    )
    .await?;
    Ok(recovered_candles)
}

/// 形成包含旧检查点自身的连续分钟扫描窗口，并把单轮工作量限制为 500 个槽位。
/// 检查点正好等于恢复终点时仍扫描该槽，供从未生成或失败的短策略完成最后一次核验；
/// 检查点已经越过终点才返回空。函数不访问存储，也不推断 Mongo 中是否已存在对应 K 线。
fn recovery_scan_open_times(
    checkpoint_open_time: DateTime<Utc>,
    recovery_until: DateTime<Utc>,
) -> Vec<DateTime<Utc>> {
    if checkpoint_open_time > recovery_until {
        return Vec::new();
    }
    let mut open_time =
        DateTime::from_timestamp(checkpoint_open_time.timestamp().div_euclid(60) * 60, 0)
            .unwrap_or(checkpoint_open_time);
    let mut times = Vec::new();
    while open_time <= recovery_until && times.len() < MAX_CANDLES_PER_STRATEGY_RUN {
        times.push(open_time);
        open_time += TimeDelta::minutes(1);
    }
    times
}

/// 以交易对集合及 interval+open_time 唯一键 upsert 一根恢复 K 线，重放只覆盖同一根蜡烛而不新增重复记录。
/// 写入不比较既有文档的新旧，恢复值会无条件覆盖同槽历史，因此调用方必须先确认该槽确实属于待补缺口。
/// 每根蜡烛自成一次写操作，批次中途失败会留下部分已补数据，靠后续同键重写而不是回滚来收敛。
pub async fn upsert_recovered_kline(db: &Database, candle: &KlineRecoveryCandle) -> AppResult<()> {
    db.collection::<Document>(&candle.collection_name())
        .update_one(candle.upsert_filter(), candle.upsert_update())
        .with_options(UpdateOptions::builder().upsert(true).build())
        .await?;
    Ok(())
}

/// 使用与实时/预览相同的 [`SyntheticMarketConfig`] 生成任务原始范围的 1m，并重建所有受影响完整聚合窗口。
/// 本入口只读写 Mongo K 线集合：每根以 `interval + open_time` 幂等 upsert，不接触 Redis ticker/Kline、WebSocket 或 MySQL 检查点。
/// 管理员手动补偿会清除自动补偿版本标记，使人工确认后的结果成为权威历史；自动调用由内部入口写入策略/version 证据。
/// 聚合前会从 Mongo 重读完整 1m 窗口；窗口仅因缺根不完整时跳过该高周期，已存根的非法数值或不连续仍使任务失败。
/// 入参必须非空、严格递增、单次不超过 10080 根，且每个槽位都是落在策略区间内且已经闭合的整分钟，任一条不满足在写入前失败。
pub async fn execute_manual_synthetic_recovery(
    db: &Database,
    config: &SyntheticMarketConfig,
    missing_open_times: &[DateTime<Utc>],
    observed_at: DateTime<Utc>,
) -> Result<ManualKlineRecoveryCounts, ManualKlineRecoveryError> {
    execute_synthetic_recovery(db, config, missing_open_times, observed_at, None).await
}

async fn execute_synthetic_recovery(
    db: &Database,
    config: &SyntheticMarketConfig,
    missing_open_times: &[DateTime<Utc>],
    observed_at: DateTime<Utc>,
    automatic_recovery: Option<AutomaticRecoveryProvenance>,
) -> Result<ManualKlineRecoveryCounts, ManualKlineRecoveryError> {
    let mut counts = ManualKlineRecoveryCounts::default();
    if missing_open_times.is_empty() {
        return Err(manual_recovery_error(
            counts,
            AppError::Validation(
                "manual recovery requires at least one missing 1m candle".to_owned(),
            ),
        ));
    }
    if missing_open_times.len() > MAX_MANUAL_RECOVERY_1M_CANDLES {
        return Err(manual_recovery_error(
            counts,
            AppError::Validation(format!(
                "manual recovery is limited to {MAX_MANUAL_RECOVERY_1M_CANDLES} 1m candles per execution"
            )),
        ));
    }
    if missing_open_times
        .windows(2)
        .any(|times| times[1] <= times[0])
    {
        return Err(manual_recovery_error(
            counts,
            AppError::Validation(
                "manual recovery open times must be strictly increasing".to_owned(),
            ),
        ));
    }
    if missing_open_times.iter().any(|open_time| {
        open_time.timestamp_subsec_nanos() != 0
            || open_time
                .timestamp()
                .rem_euclid(TimeDelta::minutes(1).num_seconds())
                != 0
            || *open_time < config.start_time
            || *open_time >= config.end_time
            || *open_time >= observed_at
    }) {
        return Err(manual_recovery_error(
            counts,
            AppError::Validation(
                "manual recovery open times must be closed UTC-minute slots inside the strategy range"
                    .to_owned(),
            ),
        ));
    }

    let symbol = ValidatedMarketSymbol::from_raw(&config.symbol)
        .map_err(|error| manual_recovery_error(counts, AppError::Validation(error.to_string())))?;
    ensure_kline_indexes(db, &symbol)
        .await
        .map_err(|error| manual_recovery_error(counts, error))?;
    let collection = db.collection::<Document>(&kline_collection_name(&symbol));

    for open_time in missing_open_times {
        let candle = config.generate_1m(*open_time).map_err(|error| {
            manual_recovery_error(counts, AppError::Validation(error.to_string()))
        })?;
        upsert_manual_candle(
            &collection,
            "1m",
            candle.open_time,
            &candle,
            observed_at,
            automatic_recovery,
        )
        .await
        .map_err(|error| manual_recovery_error(counts, error))?;
        counts.actual_1m_count = counts.actual_1m_count.saturating_add(1);
    }

    for interval in MANUAL_RECOVERY_INTERVALS {
        for window_start in affected_aggregate_window_starts(missing_open_times, interval) {
            let Some(candles) =
                load_complete_one_minute_window(&collection, window_start, interval)
                    .await
                    .map_err(|error| manual_recovery_error(counts, error))?
            else {
                counts.skipped_aggregate_count = counts.skipped_aggregate_count.saturating_add(1);
                continue;
            };
            let aggregate = aggregate_1m_candles(&candles, interval).map_err(|error| {
                manual_recovery_error(counts, AppError::Validation(error.to_string()))
            })?;
            let candle = SyntheticCandle {
                open_time: aggregate.open_time,
                values: aggregate.values,
            };
            upsert_manual_candle(
                &collection,
                interval.as_str(),
                candle.open_time,
                &candle,
                observed_at,
                automatic_recovery,
            )
            .await
            .map_err(|error| manual_recovery_error(counts, error))?;
            counts.actual_aggregate_count = counts.actual_aggregate_count.saturating_add(1);
        }
    }

    Ok(counts)
}

/// 把底层错误与当前已完成的写入进度打包成手动补偿错误，让任务终态同时保留失败原因和部分进度。
/// 计数按值拷贝，因此错误一经生成就固定在失败发生的那一刻，之后不会再被累加。
fn manual_recovery_error(
    counts: ManualKlineRecoveryCounts,
    source: AppError,
) -> ManualKlineRecoveryError {
    ManualKlineRecoveryError {
        counts,
        source: Box::new(source),
    }
}

/// 补偿的单根写入：先用领域键校验周期与开盘时间，再按周期加开盘时间幂等 upsert。
/// 除 OHLCV 外还写入固定的 `source` 标记与本次执行的 `updated_at`，便于区分补偿产物和实时摄取结果。
/// 自动路径附带策略/version 证据，手动路径显式清除这两个字段；实时摄取也会清除，避免旧标记污染新快照。
/// 自动路径只允许匹配已有自动补偿文档（且不能晚于本轮观察时间）；普通实时/人工文档不会被
/// 覆盖。若普通文档在读差集之后抢先写入，唯一键会把 upsert 转成冲突，调用方下一轮会重新
/// 读取权威历史。这样既保留旧自动版本可被新版本替换的能力，也避免恢复任务覆盖人工结果。
/// 本函数只写 Mongo，不触碰 Redis 缓存、不推进任何检查点，也不广播实时事件。
async fn upsert_manual_candle(
    collection: &mongodb::Collection<Document>,
    interval: &str,
    open_time: DateTime<Utc>,
    candle: &SyntheticCandle,
    observed_at: DateTime<Utc>,
    automatic_recovery: Option<AutomaticRecoveryProvenance>,
) -> AppResult<()> {
    KlineUpsertKey::new(interval, open_time)
        .map_err(|error| AppError::Validation(error.to_string()))?;
    let mut fields = doc! {
        "interval": interval,
        "open_time": BsonDateTime::from_millis(open_time.timestamp_millis()),
        "open": candle.values.open.to_string(),
        "high": candle.values.high.to_string(),
        "low": candle.values.low.to_string(),
        "close": candle.values.close.to_string(),
        "volume": candle.values.volume.to_string(),
        "source": "strategy",
        "updated_at": BsonDateTime::from_millis(observed_at.timestamp_millis()),
    };
    let mut update = Document::new();
    if let Some(provenance) = automatic_recovery {
        fields.insert("automatic_recovery_strategy_id", provenance.strategy_id);
        fields.insert(
            "automatic_recovery_strategy_version",
            provenance.strategy_version,
        );
    } else {
        update.insert(
            "$unset",
            doc! {
                "automatic_recovery_strategy_id": "",
                "automatic_recovery_strategy_version": "",
            },
        );
    }
    update.insert("$set", fields);
    let filter =
        automatic_recovery_write_filter(interval, open_time, observed_at, automatic_recovery);
    match collection
        .update_one(filter, update)
        .with_options(UpdateOptions::builder().upsert(true).build())
        .await
    {
        Ok(_) => Ok(()),
        Err(error) if automatic_recovery.is_some() && error.to_string().contains("E11000") => Err(
            AppError::Conflict("automatic kline recovery lost a newer Mongo write race".to_owned()),
        ),
        Err(error) => Err(AppError::Mongo(error)),
    }
}

/// 构造补偿写入过滤器。普通/人工文档只要已经占据唯一键，就不能被自动补偿匹配；
/// 没有同槽文档时过滤器自然触发 upsert，唯一索引负责把并发首写收敛为一次。
/// 已有自动文档必须带完整来源标记，且 `updated_at` 不晚于本轮观察时间，避免旧任务覆盖
/// 实时或另一轮补偿刚写入的较新值。手动路径不加这些条件，仍保持原有的确定性幂等覆盖。
fn automatic_recovery_write_filter(
    interval: &str,
    open_time: DateTime<Utc>,
    observed_at: DateTime<Utc>,
    automatic_recovery: Option<AutomaticRecoveryProvenance>,
) -> Document {
    let mut filter = doc! {
        "interval": interval,
        "open_time": BsonDateTime::from_millis(open_time.timestamp_millis()),
    };
    if automatic_recovery.is_some() {
        let observed_at = BsonDateTime::from_millis(observed_at.timestamp_millis());
        // 过滤器刻意要求已有记录带完整自动来源标记。没有匹配记录时 upsert 仍会插入新
        // 自动文档；若普通/人工记录已占据该唯一键，Mongo 会返回 E11000 而不会覆盖它。
        filter.insert(
            "$or",
            vec![
                doc! {
                    "automatic_recovery_strategy_id": { "$exists": true },
                    "automatic_recovery_strategy_version": { "$exists": true },
                    "updated_at": { "$exists": false },
                },
                doc! {
                    "automatic_recovery_strategy_id": { "$exists": true },
                    "automatic_recovery_strategy_version": { "$exists": true },
                    "updated_at": { "$lte": observed_at },
                },
            ],
        );
    }
    filter
}

/// 由本次补写的 1m 槽位反推需要重建的高周期窗口起点，按周期秒数向下取整后排序去重。
/// 取整使用欧几里得除法以保证 UTC 边界一致；同一窗口内补写多根只会产生一个起点。
/// 返回的只是候选窗口，是否完整由后续读取 Mongo 时判断，本函数不访问任何存储。
fn affected_aggregate_window_starts(
    missing_open_times: &[DateTime<Utc>],
    interval: SyntheticKlineInterval,
) -> Vec<DateTime<Utc>> {
    let window_seconds = interval.minute_count() as i64 * 60;
    let mut starts = missing_open_times
        .iter()
        .filter_map(|open_time| {
            DateTime::from_timestamp(
                open_time.timestamp().div_euclid(window_seconds) * window_seconds,
                0,
            )
        })
        .collect::<Vec<_>>();
    starts.sort_unstable();
    starts.dedup();
    starts
}

/// 读取指定高周期窗口内已落库的全部 1m，只投影聚合所需字段并按开盘时间升序返回。
/// 区间左闭右开，起点即窗口起点，终点由该周期的分钟数推出，因此不会把相邻窗口的蜡烛读进来。
/// 根数不足该周期要求时返回 `None`，调用方据此把该窗口记为跳过而不是失败。
/// 已存记录的开盘时间或 OHLCV 无法解析时返回校验错误，脏数据不会被静默跳过。
async fn load_complete_one_minute_window(
    collection: &mongodb::Collection<Document>,
    window_start: DateTime<Utc>,
    interval: SyntheticKlineInterval,
) -> AppResult<Option<Vec<SyntheticCandle>>> {
    let window_end = window_start + Duration::minutes(interval.minute_count() as i64);
    let options = FindOptions::builder()
        .sort(doc! { "open_time": 1 })
        .projection(doc! {
            "_id": 0, "open_time": 1, "open": 1, "high": 1,
            "low": 1, "close": 1, "volume": 1,
        })
        .build();
    let mut cursor = collection
        .find(doc! {
            "interval": "1m",
            "open_time": {
                "$gte": BsonDateTime::from_millis(window_start.timestamp_millis()),
                "$lt": BsonDateTime::from_millis(window_end.timestamp_millis()),
            },
        })
        .with_options(options)
        .await?;
    let mut candles = Vec::with_capacity(interval.minute_count());
    while cursor.advance().await? {
        let document = cursor.deserialize_current()?;
        let bson_time = document.get_datetime("open_time").map_err(|_| {
            AppError::Validation("stored 1m candle open_time is invalid".to_owned())
        })?;
        let open_time =
            DateTime::from_timestamp_millis(bson_time.timestamp_millis()).ok_or_else(|| {
                AppError::Validation("stored 1m candle open_time is out of range".to_owned())
            })?;
        candles.push(SyntheticCandle {
            open_time,
            values: crate::modules::market::MarketKlineValues {
                open: manual_document_decimal(&document, "open")?,
                high: manual_document_decimal(&document, "high")?,
                low: manual_document_decimal(&document, "low")?,
                close: manual_document_decimal(&document, "close")?,
                volume: manual_document_decimal(&document, "volume")?,
            },
        });
    }
    Ok(complete_one_minute_window(candles, interval))
}

/// 仅当根数恰好等于该周期所需的 1m 数量时才认为窗口完整并返回蜡烛，否则返回 `None`。
/// 这里只比较数量，时间连续性与前后开收衔接留给聚合函数判断，两层校验共同拦住残缺窗口。
fn complete_one_minute_window(
    candles: Vec<SyntheticCandle>,
    interval: SyntheticKlineInterval,
) -> Option<Vec<SyntheticCandle>> {
    (candles.len() == interval.minute_count()).then_some(candles)
}

/// 从已存 1m 文档按字段名读出十进制字符串并解析，字段类型不对时返回明确指向该字段的校验错误。
/// 聚合重建完全依赖这些历史数值，因此解析失败必须让整个补偿任务失败，而不能用零值凑出一根聚合蜡烛。
fn manual_document_decimal(document: &Document, field: &str) -> AppResult<BigDecimal> {
    let value = document.get_str(field).map_err(|_| {
        AppError::Validation(format!("stored 1m candle {field} must be a decimal string"))
    })?;
    parse_decimal(value)
}

/// 计算检查点之后、恢复终点之前按固定周期缺失的开盘时间；周期必须为正且结果受单计划最大根数限制。
/// 检查点与终点都会先向下对齐到周期边界，枚举从检查点后一个槽开始并包含终点槽，因此检查点本身不会被重写。
/// 达到 500 根上限即截断，剩余缺口留给后续轮次继续补；本函数不访问存储，也不判断这些槽位是否已经存在。
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
    active_version: i32,
    checkpoint_open_time: DateTime<Utc>,
    strategy_end_time: DateTime<Utc>,
}

/// 扫描存在 1m 缺口的策略：要求策略与交易对均为 active、运行状态属于 running、live 或 catching_up，
/// 且检查点早于最近一根已闭合的分钟。失败状态仍会在后续轮次重试，避免短暂存储故障将恢复永久冻结。
/// 检查点按最后 K 线开盘时间、最后生成时间、策略起始时间的顺序取首个非空值；
/// 同时读取 active version 与策略结束时间，后续补偿不会越过半开策略区间。
/// 已到最后可生成分钟且状态为 live 的已结束策略不再重复扫描；idle/failed 的最后单槽仍保留一次恢复机会。
/// 结果按检查点与策略 ID 升序，优先处理落后最多的策略，条数夹紧到 1 至 100；本查询只读，不加锁也不占用租约。
async fn fetch_due_strategy_runs(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<Vec<DueKlineRecoveryRun>> {
    sqlx::query_as::<_, DueKlineRecoveryRun>(
        r#"SELECT strategies.id AS strategy_id,
                  pairs.symbol,
                  runs.active_version,
                  COALESCE(runs.last_kline_open_time, runs.last_generated_at, strategies.start_time) AS checkpoint_open_time,
                  strategies.end_time AS strategy_end_time
           FROM strategy_runs runs
           INNER JOIN market_strategies strategies ON strategies.id = runs.strategy_id
           INNER JOIN trading_pairs pairs ON pairs.id = strategies.pair_id
           WHERE strategies.status = 'active'
             AND pairs.status = 'active'
             AND pairs.market_type IN ('strategy', 'internal')
             AND runs.run_status IN ('running', 'live', 'catching_up')
             AND COALESCE(runs.last_kline_open_time, runs.last_generated_at, strategies.start_time) < ?
             AND (
               DATE_ADD(COALESCE(runs.last_kline_open_time, runs.last_generated_at, strategies.start_time), INTERVAL 1 MINUTE) < strategies.end_time
               OR (
                 COALESCE(runs.recovery_status, 'idle') <> 'live'
                 AND COALESCE(runs.last_kline_open_time, runs.last_generated_at, strategies.start_time) < strategies.end_time
               )
             )
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

/// 以旧检查点和 active version 为乐观条件推进连续历史扫描位置。
///
/// 该更新刻意不写 `current_price`、`last_tick_at` 或 `last_generated_at`，避免历史补偿回退实时行情；
/// 完全追到最近闭合分钟时标记 live，否则保留 catching_up 供后续轮次继续。更新再次约束策略仍启用、
/// 运行状态仍可恢复以及 active version/旧检查点均未变化；暂停、改版或并发推进时返回
/// `AlreadyAdvanced`，Mongo 已写根可由同键与后续差集安全收敛。
async fn update_recovery_checkpoint(
    pool: &Pool<MySql>,
    strategy_id: u64,
    active_version: i32,
    previous_open_time: DateTime<Utc>,
    last_open_time: DateTime<Utc>,
    fully_caught_up: bool,
) -> Result<(), KlineRecoveryCheckpointError> {
    let result = sqlx::query(
        r#"UPDATE strategy_runs runs
           INNER JOIN market_strategies strategies ON strategies.id = runs.strategy_id
           SET runs.last_kline_open_time = ?,
               runs.recovery_status = ?,
               runs.error_message = NULL
           WHERE runs.strategy_id = ?
             AND strategies.status = 'active'
             AND runs.run_status IN ('running', 'live', 'catching_up')
             AND runs.active_version = ?
             AND COALESCE(runs.last_kline_open_time, runs.last_generated_at, strategies.start_time) = ?"#,
    )
    .bind(last_open_time.naive_utc())
    .bind(if fully_caught_up {
        "live"
    } else {
        "catching_up"
    })
    .bind(strategy_id)
    .bind(active_version)
    .bind(previous_open_time.naive_utc())
    .execute(pool)
    .await
    .map_err(AppError::from)?;
    if result.rows_affected() != 1 {
        return Err(KlineRecoveryCheckpointError::AlreadyAdvanced);
    }
    Ok(())
}

/// 把仍指向本轮版本与检查点的策略恢复状态置为 failed，并记录截断到 1024 字符的原因。
/// 扫描查询不排除 failed，因此短暂故障在下一轮仍会重试；乐观条件保证并发成功、暂停或改版后，
/// 迟到的失败不会覆盖更新状态。记录失败本身的数据库错误只写告警，不中断整轮扫描。
async fn mark_recovery_failed(pool: &Pool<MySql>, row: &DueKlineRecoveryRun, error_message: &str) {
    let truncated = error_message.chars().take(1024).collect::<String>();
    if let Err(error) = sqlx::query(
        r#"UPDATE strategy_runs runs
           INNER JOIN market_strategies strategies ON strategies.id = runs.strategy_id
           SET runs.recovery_status = 'failed', runs.error_message = ?
           WHERE runs.strategy_id = ?
             AND strategies.status = 'active'
             AND runs.run_status IN ('running', 'live', 'catching_up')
             AND runs.active_version = ?
             AND COALESCE(runs.last_kline_open_time, runs.last_generated_at, strategies.start_time) = ?"#,
    )
    .bind(truncated)
    .bind(row.strategy_id)
    .bind(row.active_version)
    .bind(row.checkpoint_open_time.naive_utc())
    .execute(pool)
    .await
    {
        warn!(strategy_id = row.strategy_id, %error, "标记 K 线恢复错误失败");
    }
}

/// 把时间间隔映射为存储使用的周期代码，只支持 1m、5m、15m、1h、1d 五种。
/// 4h 不在本映射内，因此自动恢复路径无法直接产出该周期；未匹配的间隔返回校验错误，不会猜测最接近的周期。
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

/// 求最近一根已闭合蜡烛的开盘时间：先把当前时刻按周期向下对齐，再回退一个完整周期。
/// 恢复终点与到期扫描都用它，从而保证任何路径都不会去补写仍在形成中的那一根蜡烛。
fn last_closed_open_time(now: DateTime<Utc>, interval: TimeDelta) -> AppResult<DateTime<Utc>> {
    let aligned =
        align_open_time(now, interval).map_err(|error| AppError::Validation(error.to_string()))?;
    Ok(aligned - interval)
}

/// 把时间向下对齐到周期边界：非正周期直接返回 `InvalidInterval`，其余按秒数取整后回落到毫秒精度。
/// 换算经过浮点运算，适用于当前使用的分钟到天级周期；结果超出可表示范围时同样返回 `InvalidInterval`。
/// 对齐只做向下取整，绝不会把时间推进到下一个槽位，缺口枚举与恢复终点推导都依赖这一点。
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

/// 返回两个十进制中较大的一个，用于取开盘价与收盘价的高者作为恢复蜡烛的最高价基准。
/// 相等时返回左值，语义上没有差别；比较按数值进行，不受尾随零等文本差异影响。
fn decimal_max(left: &BigDecimal, right: &BigDecimal) -> BigDecimal {
    if left >= right {
        left.clone()
    } else {
        right.clone()
    }
}

/// 返回两个十进制中较小的一个，与取大值配对，共同确定恢复蜡烛的最低价基准。
/// 相等时同样返回左值；比较基于数值而非文本，标度不同但数值相等的两个数不会被误判。
fn decimal_min(left: &BigDecimal, right: &BigDecimal) -> BigDecimal {
    if left <= right {
        left.clone()
    } else {
        right.clone()
    }
}

/// 把十进制文本解析为 `BigDecimal`，失败时统一转成带原始原因的校验错误。
/// 策略参数与已存蜡烛数值都经由它进入计算，因此非法文本会在任何写入发生之前被拦下。
fn parse_decimal(value: &str) -> AppResult<BigDecimal> {
    BigDecimal::from_str(value)
        .map_err(|error| AppError::Validation(format!("invalid decimal value: {error}")))
}

/// 把单轮扫描的策略数夹紧到 1 至 100，传入 0 或超大值都会收敛到边界而不是报错。
/// 该上限与每策略最多 500 根的限制共同约束一轮工作量，避免单轮压垮 MySQL 与 Mongo。
fn kline_recovery_limit(limit: u32) -> u32 {
    limit.clamp(1, 100)
}

#[cfg(test)]
#[path = "../../tests/unit_src/src_workers_kline_recovery_tests.rs"]
mod tests;
