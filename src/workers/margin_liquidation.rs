//! 杠杆强平后台任务。
//!
//! 每轮先按账户处理全仓，再逐笔处理逐仓，两类候选都由 `next_liquidation_attempt_at` 调度节流。
//! 标记价只认 Redis 中六十秒内的正价格，缺价或过期一律推迟重试，绝不用入场价或客户端价格兜底。
//! 逐仓判定基于单仓权益：保证金加浮盈减利息，权益不高于名义价值乘维持保证金率即触发，
//! 结算时按非负权益返还，亏损截零，穿仓部分由平台隐性承担且不单独登记。
//! 全仓判定基于账户共享权益：钱包可用余额加全部仓位保证金加浮盈减利息，与维持保证金总额比较，
//! 触发后账户内所有全仓仓位在同一事务中一起关闭，共享钱包只按组合权益变更一次，
//! 扣穿部分钳零后作为坏账写入账户行；各仓位的 payout 只是按正权益占比的展示性分摊，之和不超过组合正权益。
//! 每个账户或仓位独立开事务，单项失败只计数并继续，不会回滚本轮已提交的其他强平。
//! 私有强平事件严格在对应事务提交之后才广播，且广播失败不影响已落地的资金结果，进程重启也不补发。

use crate::{
    error::{AppError, AppResult},
    modules::{
        events::{EventBroadcastHub, EventBroadcastMessage},
        margin::domain::{
            CrossMarginPositionRisk, CrossMarginRiskState, allocate_cross_margin_payouts,
            evaluate_cross_margin,
        },
        margin::infrastructure::{
            apply_cross_margin_account_settlement, credit_margin_position_amount,
        },
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
use std::collections::HashMap;
use std::env;
use tokio::time::{Duration, interval};
use tracing::{error, info, warn};

/// 强平任务的入口句柄，本身无状态，仅用于把 `run_once` 暴露成方法形式。
pub struct MarginLiquidationWorker;

/// 强平任务的运行参数，全部来自环境变量并在启动时读取一次。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarginLiquidationWorkerConfig {
    /// 是否启用强平任务，关闭后触及强平线的仓位不会被自动处置。
    pub enabled: bool,
    /// 两轮扫描之间的间隔秒数，默认五秒，实际执行时至少为一秒。
    pub interval_seconds: u64,
    /// 单轮成功强平的账户与仓位总数上限，运行时会被夹到 1 到 100。
    pub batch_limit: u32,
}

impl MarginLiquidationWorkerConfig {
    /// 读取强平开关、周期与批量环境配置；默认启用、周期 5 秒、批量 100，缺失或不可解析值回落到默认值。
    pub fn from_env() -> Self {
        Self {
            enabled: env_bool("MARGIN_LIQUIDATION_ENABLED", true),
            interval_seconds: env_u64("MARGIN_LIQUIDATION_INTERVAL_SECONDS", 5),
            batch_limit: env_u32("MARGIN_LIQUIDATION_BATCH_LIMIT", 100),
        }
    }
}

impl MarginLiquidationWorker {
    /// 执行一轮全仓优先、逐仓随后强平；成功上限收敛到 1..=100，逐仓候选最多 500，全仓账户最多 100。
    /// 仅接受新鲜 Redis 标记价，逐账户/逐仓独立资金事务，单项失败继续，私有事件在对应事务提交后广播。
    pub async fn run_once(
        &self,
        state: &AppState,
        now: DateTime<Utc>,
        limit: u32,
    ) -> AppResult<MarginLiquidationSummary> {
        run_once(state, now, limit).await
    }
}

/// 单轮强平的结果统计，全仓账户与逐仓仓位共用同一组计数器，一个账户整体算作一次。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MarginLiquidationSummary {
    /// 本轮实际尝试处理的账户数加仓位数。
    pub scanned: u32,
    /// 成功完成强平的次数，达到上限即停止本轮扫描。
    pub liquidated: u32,
    /// 因风险已恢复、状态已变化或行情缺失而跳过的次数。
    pub skipped: u32,
    /// 处理过程中抛错的次数，已记录告警并推迟重试。
    pub failed: u32,
}

/// 单个仓位的强平判定结果，逐仓直接使用，全仓则为每个仓位单独构造用于写强平记录。
#[derive(Debug, Clone, PartialEq)]
pub struct MarginLiquidationRiskState {
    /// 是否触及强平线，判定条件是权益不高于维持保证金。
    pub should_liquidate: bool,
    /// 仓位权益，等于保证金加已实现口径盈亏再减累计利息，可为负。
    pub equity: BigDecimal,
    /// 维持保证金要求，等于名义价值乘产品维持保证金率。
    pub maintenance_margin: BigDecimal,
    /// 按标记价折算的盈亏，做多做空取号相反。
    pub realized_pnl: BigDecimal,
}

/// 逐仓强平候选，只带主键和交易对符号，后者用于取行情缓存。
#[derive(Debug, sqlx::FromRow)]
struct MarginLiquidationCandidate {
    position_id: u64,
    symbol: String,
}

/// 全仓强平候选，粒度是「用户加保证金币种」这一对键，即一个共享权益账户。
#[derive(Debug, sqlx::FromRow)]
struct CrossMarginAccountCandidate {
    user_id: u64,
    margin_asset: u64,
}

/// 全仓账户下的单个仓位标识，用于在开事务前逐个取标记价。
#[derive(Debug, sqlx::FromRow)]
struct CrossMarginPositionCandidate {
    id: u64,
    symbol: String,
}

/// 全仓强平事务内加锁读到的仓位快照，与产品表联表取维持保证金率。
/// 没有 status 列是因为查询条件已固定 `status = 'opened'`，读到即代表仍在持仓。
#[derive(Debug, sqlx::FromRow)]
struct LockedCrossMarginPosition {
    id: u64,
    user_id: u64,
    product_id: u64,
    pair_id: u64,
    margin_asset: u64,
    /// 全仓抵押必须锁在杠杆钱包，非 margin 值会中止整笔账户事务。
    wallet_scope: String,
    direction: String,
    margin_amount: BigDecimal,
    notional_amount: BigDecimal,
    interest_amount: BigDecimal,
    /// 为 NULL 表示未成交，全仓强平遇到这种数据视为异常并中止整个账户。
    entry_price: Option<BigDecimal>,
    maintenance_margin_rate: BigDecimal,
}

/// 逐仓强平事务内加锁读到的仓位快照，比全仓版本多一个 status 列。
/// 因为逐仓按主键加锁不带状态条件，必须在锁定后自行判断仓位是否仍为 opened。
#[derive(Debug, sqlx::FromRow)]
struct LockedMarginPosition {
    id: u64,
    user_id: u64,
    product_id: u64,
    pair_id: u64,
    margin_asset: u64,
    /// 开仓时实际扣款的资金域，强平返还按它原路退回 spot 或 margin。
    wallet_scope: String,
    direction: String,
    margin_amount: BigDecimal,
    notional_amount: BigDecimal,
    interest_amount: BigDecimal,
    /// 加锁瞬间的状态，非 opened 说明已被平仓或撤销，回滚跳过。
    status: String,
    entry_price: Option<BigDecimal>,
    maintenance_margin_rate: BigDecimal,
}

/// Redis ticker 缓存中强平关心的两个字段，其余字段反序列化时忽略。
#[derive(Debug, Deserialize)]
struct CachedTickerPayload {
    last_price: BigDecimal,
    #[serde(with = "unix_millis")]
    observed_at: DateTime<Utc>,
}

/// 强平事件载荷，在事务内构造但只在提交成功后才被广播出去。
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
    /// 触发强平时采用的标记价，同时被写为仓位的退出价。
    mark_price: BigDecimal,
    realized_pnl: BigDecimal,
    /// 逐仓是真实入账的非负返还额；全仓是按正权益占比的分摊值，仅供展示。
    payout_amount: BigDecimal,
    /// 强平原因，逐仓为 `maintenance_margin`，全仓为 `cross_maintenance_margin`。
    reason: &'static str,
    liquidated_at: DateTime<Utc>,
}

/// 单次强平尝试的结果；触发时携带待广播事件列表，全仓可能一次产出多条。
#[derive(Debug, Clone)]
enum LiquidationOutcome {
    Liquidated(Vec<MarginLiquidationEvent>),
    Skipped,
}

/// 按方向、名义本金、保证金、累计利息和维持保证金率计算逐仓强平状态。
/// 入场价与标记价必须为正；权益等于保证金加已实现口径盈亏减利息，权益不高于维持保证金即触发。
/// 本函数不读写存储，返回值必须由持锁事务重新核对仓位状态后才能用于资金结算。
///
/// 盈亏口径为名义价值乘价差再除以入场价，做多取标记价减入场价、做空取反；
/// 维持保证金为名义价值乘产品维持保证金率，三个中间量都归一到十八位小数。
/// 判定用的是不高于而非严格小于，因此权益恰好等于维持保证金也会触发强平。
/// 用户端的风险快照接口也复用它，保证页面展示的强平判定与后台实际执行完全同源。
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

/// 只计算按标记价折算的盈亏，不产出权益和维持保证金，供全仓路径逐仓位取浮盈使用。
/// 与 `margin_liquidation_risk_state` 内部同名口径一致：名义价值乘价差再除入场价，
/// 做多取标记价减入场价、做空取反，结果归一到十八位小数并可正可负。
/// 全仓不能直接用完整风险函数，因为它的判定基于账户共享权益而不是单仓权益，
/// 所以这里只取盈亏这一项，再交给领域层的账户级评估汇总。
/// 入场价与标记价必须为正，方向必须是 long 或 short，否则返回参数错误中止整笔账户事务。
fn margin_realized_pnl(
    direction: &str,
    notional_amount: &BigDecimal,
    entry_price: &BigDecimal,
    mark_price: &BigDecimal,
) -> AppResult<BigDecimal> {
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
    Ok((notional_amount.clone() * price_delta / entry_price.clone()).with_scale(18))
}

/// 从应用状态取得 MySQL 仓位/钱包、Redis 权威 ticker 与可选事件 hub 后执行单轮强平；MySQL 或 Redis 缺失时在扫描前失败。
/// 单轮成功上限为 1..=100，逐账户/逐仓独立提交资金与终态；私有强平事件仅在对应事务提交后尽力广播。
///
/// MySQL 与 Redis 都是硬依赖，任一缺失立即报内部错误并整轮放弃，绝不降级到不看行情就强平。
/// 事件广播中心是可选依赖，缺失时强平照常执行，只是不推送私有通知。
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

/// 在显式 MySQL/Redis 依赖上执行同一全仓优先、逐仓随后批次，但禁用进程内广播；扫描上限、价格新鲜度、锁序、资金守恒与幂等不变。
/// 存在的意义是让集成测试和运维脚本能绕开 `AppState` 直接注入连接，并避免测试触发真实的用户推送。
/// 由于事件被丢弃而资金写入照常发生，调用方必须自行确认这种静默行为是可接受的。
pub async fn run_once_with_dependencies(
    pool: &Pool<MySql>,
    redis: &ConnectionManager,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<MarginLiquidationSummary> {
    run_once_with_dependencies_and_events(pool, redis, None, now, limit).await
}

/// 强平单轮核心：先按账户处理全仓，再处理逐仓；每个账户或仓位独立事务，单项失败不会回滚已提交强平。
/// 缺失/过期行情只推迟下一次检查，不以入场价或客户端价格兜底；成功达到上限后停止扫描。
/// 只有新强平事务提交后才广播事件，安全仓位与状态已变化仓位均幂等跳过。
///
/// 两类候选在进入循环前一次性查好，因此本轮不会看到扫描期间新触发强平线的仓位，留待下一轮。
/// 全仓先于逐仓处理，且共用同一个 `liquidated` 配额，账户数占满配额时逐仓本轮完全不执行。
/// 全仓有一条重要的全有全无规则：账户内任意一个仓位取不到新鲜标记价，就把该仓位推迟六十秒并
/// 整个账户跳过，因为共享权益必须用同一时点的完整估值计算，缺任何一腿都会算出偏乐观的结果。
/// 逐仓的失败处理更细分：缺行情、读行情出错和强平抛错都推迟六十秒，
/// 而风险已恢复的安全仓位只推迟五秒，让接近强平线的仓位保持高频复查。
/// 事件在每次成功后立即逐条广播，因此前序账户的通知不会被后续失败吞掉。
async fn run_once_with_dependencies_and_events(
    pool: &Pool<MySql>,
    redis: &ConnectionManager,
    event_hub: Option<&EventBroadcastHub>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<MarginLiquidationSummary> {
    let liquidation_limit = margin_liquidation_limit(limit);
    let candidates = fetch_open_positions(pool, now, margin_liquidation_scan_limit(limit)).await?;
    let cross_accounts =
        fetch_open_cross_accounts(pool, now, margin_liquidation_limit(limit)).await?;
    let mut summary = MarginLiquidationSummary::default();

    // 全仓按账户一次性评估；一旦触发，账户内所有全仓仓位在同一事务中统一处理。
    for account in cross_accounts {
        if summary.liquidated >= liquidation_limit {
            break;
        }
        summary.scanned += 1;
        let positions =
            fetch_cross_account_positions(pool, account.user_id, account.margin_asset).await?;
        let mut marks = HashMap::new();
        let mut missing_mark = false;
        for position in &positions {
            match cached_ticker_price(redis, &position.symbol, now).await {
                Ok(Some(price)) => {
                    marks.insert(position.id, price);
                }
                Ok(None) | Err(_) => {
                    missing_mark = true;
                    schedule_next_liquidation_attempt(
                        pool,
                        position.id,
                        now + chrono::TimeDelta::seconds(60),
                    )
                    .await?;
                }
            }
        }
        if missing_mark {
            summary.skipped += 1;
            continue;
        }
        match liquidate_cross_account(pool, account.user_id, account.margin_asset, &marks, now)
            .await
        {
            Ok(LiquidationOutcome::Liquidated(events)) => {
                summary.liquidated += 1;
                if let Some(hub) = event_hub {
                    for event in &events {
                        publish_liquidation_event(hub, event);
                    }
                }
            }
            Ok(LiquidationOutcome::Skipped) => summary.skipped += 1,
            Err(error) => {
                summary.failed += 1;
                warn!(user_id = account.user_id, margin_asset = account.margin_asset, %error, "全仓账户强平失败");
            }
        }
    }

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
            Ok(LiquidationOutcome::Liquidated(events)) => {
                summary.liquidated += 1;
                if let Some(hub) = event_hub {
                    for event in &events {
                        publish_liquidation_event(hub, event);
                    }
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

/// 以至少 1 秒间隔持续强平；周期级候选查询错误只记录并进入下一轮，单项缺价/失败由核心批次延期后继续。
/// next-attempt、仓位终态、强平记录与账本承担跨重启恢复；提交后进程内私有事件不会补发。
/// 循环永不返回 Ok，只会在被外部取消时结束；轮次级错误只写日志，不会让任务退出。
/// 使用 tokio 的 `interval`，若某轮执行超过间隔时长，下一次 tick 会立即触发而不是累积补齐。
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

/// 挑选本轮待检查的逐仓候选，条件是已成交、状态 opened、模式 isolated 且已到重试时间。
/// `entry_price IS NOT NULL` 显式把已冻结抵押但尚未成交的限价挂单隔离在强平风险集合之外。
/// `next_liquidation_attempt_at` 为 NULL 视为立即可查，新开仓位因此在下一轮就被纳入检查。
/// 该列同时充当节流器与跨重启检查点：安全仓位推迟五秒、异常仓位推迟六十秒，
/// 进程重启后无需任何内存状态即可从数据库恢复原有的检查节奏。
/// 排序以重试时间升序打头，让最该被检查的仓位优先，开仓时间与主键兜底保证顺序完全确定。
/// 联表取交易对符号以便调用方直接查行情；上限在此夹到 1 到 500。
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
           WHERE positions.status = 'opened' AND positions.margin_mode = 'isolated'
             AND positions.entry_price IS NOT NULL
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

/// 挑选本轮待检查的全仓账户，用 DISTINCT 把已成交持仓按「用户加保证金币种」去重成账户粒度。
/// 之所以从仓位表推导而不是直接扫账户表，是因为只有还持有 opened 全仓仓位的账户才需要评估风险。
/// 只要账户内任意一个仓位到了重试时间，整个账户就会被选中，随后统一重新估值。
/// 上限比逐仓更严，夹到 1 到 100 而非 500，因为一个账户可能展开成多笔仓位的重活。
/// 按用户和币种升序排列，使多实例或多轮扫描的处理顺序稳定，减少互相阻塞。
async fn fetch_open_cross_accounts(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<Vec<CrossMarginAccountCandidate>> {
    sqlx::query_as::<_, CrossMarginAccountCandidate>(
        r#"SELECT DISTINCT positions.user_id, positions.margin_asset
           FROM margin_positions positions
           WHERE positions.status = 'opened'
             AND positions.margin_mode = 'cross'
             AND positions.entry_price IS NOT NULL
             AND (positions.next_liquidation_attempt_at IS NULL OR positions.next_liquidation_attempt_at <= ?)
           ORDER BY positions.user_id ASC, positions.margin_asset ASC
           LIMIT ?"#,
    )
    .bind(now.naive_utc())
    .bind(limit.clamp(1, 100) as i64)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

/// 列出某个全仓账户下全部已成交 opened 仓位的主键与交易对符号，用于在开事务前逐个取标记价。
/// 不带 `next_liquidation_attempt_at` 条件，因为账户级评估必须覆盖全部仓位，
/// 漏掉任何一笔都会让共享权益算高，从而错过本应触发的强平。
/// 只读不加锁，读到的集合可能与随后加锁读到的不完全一致；那次带 FOR UPDATE 的查询才是权威依据，
/// 若届时某个仓位缺少对应标记价，整笔账户事务会中止而不是带着残缺估值继续。
/// 按主键升序返回，与事务内加锁查询的排序一致，保持取锁顺序稳定。
async fn fetch_cross_account_positions(
    pool: &Pool<MySql>,
    user_id: u64,
    margin_asset: u64,
) -> AppResult<Vec<CrossMarginPositionCandidate>> {
    sqlx::query_as::<_, CrossMarginPositionCandidate>(
        r#"SELECT positions.id, pairs.symbol
           FROM margin_positions positions
           INNER JOIN trading_pairs pairs ON pairs.id = positions.pair_id
           WHERE positions.user_id = ? AND positions.margin_asset = ?
             AND positions.margin_mode = 'cross' AND positions.status = 'opened'
             AND positions.entry_price IS NOT NULL
           ORDER BY positions.id ASC"#,
    )
    .bind(user_id)
    .bind(margin_asset)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

/// 读取指定交易对的服务端标记价，并施加强平专用的有效性判定。
/// 三种结果语义不同：缓存键不存在返回 Ok(None) 表示行情缺失，价格非正或超过六十秒未更新返回 Err，
/// JSON 解析失败同样返回 Err 但归为内部错误，因为那意味着写入端与读取端契约不一致。
/// 新鲜度以传入的 `now` 为基准而非函数内部再取时间，使同一轮扫描对所有仓位使用统一的时间尺度。
/// 拒绝陈旧价格是强平安全的关键：宁可推迟处置，也不用过期价格误判某个仓位安全或误砍某个正常仓位。
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

/// 在独立事务中强平一个逐仓仓位：先锁仓位并重算风险，再结算非负剩余权益、写清算记录并关闭仓位。
/// 非 opened 或风险恢复的仓位回滚后跳过；钱包资金、清算审计与仓位状态必须同事务提交。
/// 本函数只在提交后返回事件载荷，不直接广播；重复扫描最终状态不会产生第二笔结算。
///
/// 加锁后先重算风险而不是沿用扫描时的判断，因为利息可能刚被计提、产品维持保证金率可能刚被调整。
/// 风险已恢复时回滚并跳过，调用方随后把该仓位的下次检查推迟五秒。
/// 缺入场价直接返回错误而非跳过，因为未成交仓位本不应进入强平候选，属于数据异常需要暴露。
/// 返还额取权益的非负截断，亏损吃穿保证金时只退零，穿仓缺口在逐仓路径下不单独登记为坏账。
/// 关闭仓位的 UPDATE 带 `status = 'opened'` 条件，影响行数不为一即回滚跳过，
/// 这道兜底确保与用户主动平仓并发时不会重复结算，同时清空重试时间把仓位移出调度。
/// 强平记录、钱包入账和仓位终态同事务提交，事件在提交后才由调用方发布。
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
           WHERE id = ? AND status = 'opened' AND entry_price IS NOT NULL"#,
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
    Ok(LiquidationOutcome::Liquidated(vec![event]))
}

/// 统一处理一个全仓账户：按仓位 ID 锁定全部 opened 仓位，再锁保证金钱包并用组合权益决定是否清算。
/// 调用方必须为每个仓位提供同一扫描时点的新鲜标记价；缺价、非 margin 钱包或缺入场价均中止整笔账户事务。
/// 触发时按组合权益分配各仓位 payout，账户钱包结算、坏账、清算记录、仓位终态和风险快照必须原子提交。
/// 未触发只提交最新风险快照；成功事件在事务提交后由上层发布，重复扫描已关闭仓位会跳过。
///
/// 锁序固定为「先按主键升序锁全部全仓仓位，再锁杠杆钱包」，与开仓、平仓、划转路径的
/// 「先仓位后钱包」方向一致，因此账户级强平不会与用户主动操作形成交叉等待。
/// 仓位集合为空说明账户已被清空，回滚跳过；钱包行缺失时按十八位精度的零权益参与计算而不报错。
/// 逐仓位校验三件事：资金域必须是 margin、入场价必须存在、必须能在 `marks` 中找到对应标记价，
/// 任一不满足立即返回错误中止整笔账户事务，绝不带着残缺估值继续结算。
/// 账户风险由领域层统一评估：钱包可用余额加全部仓位保证金加浮盈减利息为账户权益，
/// 与维持保证金总额比较决定是否强平；无论是否触发都会先写回风险快照并递增版本号。
/// 未触发时提交快照即返回，因此本函数在安全路径下仍有写入，不是纯只读。
/// 触发后按各仓位正权益占比分摊组合权益，分摊结果只写进强平记录与事件用于展示，
/// 真正的资金变更只有一次：共享钱包按组合权益增减，扣穿部分钳零并作为坏账记入账户行。
/// 逐仓位写强平记录并以 `status = 'opened'` 为条件迁移到 liquidated，同时清空重试时间。
/// 最后把账户状态置为 liquidated 并落盘结算后余额与坏债，全部写入随同一事务原子提交。
async fn liquidate_cross_account(
    pool: &Pool<MySql>,
    user_id: u64,
    margin_asset: u64,
    marks: &HashMap<u64, BigDecimal>,
    now: DateTime<Utc>,
) -> AppResult<LiquidationOutcome> {
    let mut tx = pool.begin().await?;
    let positions = sqlx::query_as::<_, LockedCrossMarginPosition>(
        r#"SELECT positions.id, positions.user_id, positions.product_id, positions.pair_id,
                  positions.margin_asset, positions.wallet_scope, positions.direction,
                  positions.margin_amount, positions.notional_amount, positions.interest_amount,
                  positions.entry_price, products.maintenance_margin_rate
           FROM margin_positions positions
           INNER JOIN margin_products products ON products.id = positions.product_id
           WHERE positions.user_id = ? AND positions.margin_asset = ?
             AND positions.margin_mode = 'cross' AND positions.status = 'opened'
             AND positions.entry_price IS NOT NULL
           ORDER BY positions.id ASC
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(margin_asset)
    .fetch_all(&mut *tx)
    .await?;
    if positions.is_empty() {
        tx.rollback().await?;
        return Ok(LiquidationOutcome::Skipped);
    }

    let wallet_equity = sqlx::query_scalar::<_, BigDecimal>(
        r#"SELECT COALESCE(available, 0)
           FROM margin_wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(margin_asset)
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or_else(|| BigDecimal::from(0).with_scale(18));
    let mut position_margin = BigDecimal::from(0);
    let mut risks = Vec::with_capacity(positions.len());
    let mut per_position_states = Vec::with_capacity(positions.len());
    for position in &positions {
        if position.wallet_scope != "margin" {
            return Err(AppError::Validation(
                "cross margin position must use margin wallet scope".to_owned(),
            ));
        }
        let Some(entry_price) = position.entry_price.as_ref() else {
            return Err(AppError::Validation(
                "cross margin entry price is required for liquidation".to_owned(),
            ));
        };
        let mark_price = marks.get(&position.id).ok_or_else(|| {
            AppError::Validation("cross margin mark price is required for liquidation".to_owned())
        })?;
        let realized_pnl = margin_realized_pnl(
            &position.direction,
            &position.notional_amount,
            entry_price,
            mark_price,
        )?;
        let maintenance_margin = (position.notional_amount.clone()
            * position.maintenance_margin_rate.clone())
        .with_scale(18);
        position_margin += position.margin_amount.clone();
        risks.push(CrossMarginPositionRisk {
            unrealized_pnl: realized_pnl.clone(),
            interest_amount: position.interest_amount.clone(),
            maintenance_margin: maintenance_margin.clone(),
        });
        per_position_states.push((
            realized_pnl,
            maintenance_margin,
            entry_price.clone(),
            mark_price.clone(),
        ));
    }
    let account_risk = evaluate_cross_margin(&wallet_equity, &position_margin, &risks);
    update_cross_account_snapshot(&mut tx, user_id, margin_asset, &account_risk, now).await?;
    if !account_risk.should_liquidate {
        tx.commit().await?;
        return Ok(LiquidationOutcome::Skipped);
    }

    let position_equities = positions
        .iter()
        .zip(&per_position_states)
        .map(|(position, (realized_pnl, _, _, _))| {
            (position.margin_amount.clone() + realized_pnl.clone()
                - position.interest_amount.clone())
            .with_scale(18)
        })
        .collect::<Vec<_>>();
    let payouts = allocate_cross_margin_payouts(&position_equities, &account_risk.portfolio_equity);
    let reference_id = format!(
        "{}:{}:{}",
        user_id,
        margin_asset,
        positions.first().map(|position| position.id).unwrap_or(0)
    );
    let settlement = apply_cross_margin_account_settlement(
        &mut tx,
        user_id,
        margin_asset,
        &account_risk.portfolio_equity,
        &reference_id,
    )
    .await?;

    let mut events = Vec::with_capacity(positions.len());
    for ((position, (realized_pnl, maintenance_margin, entry_price, mark_price)), payout_amount) in
        positions.iter().zip(per_position_states).zip(payouts)
    {
        let position_equity = (position.margin_amount.clone() + realized_pnl.clone()
            - position.interest_amount.clone())
        .with_scale(18);
        let position_risk = MarginLiquidationRiskState {
            should_liquidate: true,
            equity: position_equity,
            maintenance_margin,
            realized_pnl: realized_pnl.clone(),
        };
        insert_cross_liquidation_record(
            &mut tx,
            position,
            &entry_price,
            &mark_price,
            &position_risk,
            &payout_amount,
            now,
        )
        .await?;
        sqlx::query(
            r#"UPDATE margin_positions
               SET status = 'liquidated', closed_at = ?, liquidated_at = ?, exit_price = ?,
                   realized_pnl = ?, liquidation_reason = 'cross_maintenance_margin',
                   next_liquidation_attempt_at = NULL
               WHERE id = ? AND status = 'opened' AND entry_price IS NOT NULL"#,
        )
        .bind(now.naive_utc())
        .bind(now.naive_utc())
        .bind(&mark_price)
        .bind(&realized_pnl)
        .bind(position.id)
        .execute(&mut *tx)
        .await?;
        events.push(MarginLiquidationEvent {
            user_id: position.user_id,
            position_id: position.id,
            product_id: position.product_id,
            pair_id: position.pair_id,
            margin_asset: position.margin_asset,
            direction: position.direction.clone(),
            margin_amount: position.margin_amount.clone(),
            notional_amount: position.notional_amount.clone(),
            interest_amount: position.interest_amount.clone(),
            entry_price,
            mark_price,
            realized_pnl,
            payout_amount,
            reason: "cross_maintenance_margin",
            liquidated_at: now,
        });
    }
    sqlx::query(
        r#"UPDATE margin_cross_accounts
           SET status = 'liquidated', last_equity = ?, last_bad_debt = ?,
               version = version + 1
           WHERE user_id = ? AND margin_asset = ?"#,
    )
    .bind(&settlement.available_after)
    .bind(&settlement.bad_debt)
    .bind(user_id)
    .bind(margin_asset)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(LiquidationOutcome::Liquidated(events))
}

/// 在强平事务内写回全仓账户的最新风险快照，账户行不存在时顺带以 active 状态创建。
/// 用 INSERT ... ON DUPLICATE KEY UPDATE 实现存在即更新，因此首次评估的账户无需预建。
/// 每次写入都把 `version` 递增，供读取方判断快照新旧；插入分支从 1 起算。
/// 状态被重置为 active，所以本次评估若判定安全，此前被标记为 liquidated 的账户会恢复可用；
/// 若判定触发强平，调用方会在同一事务的最后再把状态改写为 liquidated，以后者为准。
/// 保证金率为 None 时该列落成 NULL，表示维持保证金为零、比率无意义，而不是记作零。
async fn update_cross_account_snapshot(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    margin_asset: u64,
    risk: &CrossMarginRiskState,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO margin_cross_accounts
             (user_id, margin_asset, status, last_equity, last_unrealized_pnl,
              last_interest_amount, last_maintenance_margin, last_margin_ratio, last_risk_at, version)
           VALUES (?, ?, 'active', ?, ?, ?, ?, ?, ?, 1)
           ON DUPLICATE KEY UPDATE
             status = 'active', last_equity = VALUES(last_equity),
             last_unrealized_pnl = VALUES(last_unrealized_pnl),
             last_interest_amount = VALUES(last_interest_amount),
             last_maintenance_margin = VALUES(last_maintenance_margin),
             last_margin_ratio = VALUES(last_margin_ratio), last_risk_at = VALUES(last_risk_at),
             version = version + 1"#,
    )
    .bind(user_id)
    .bind(margin_asset)
    .bind(&risk.equity)
    .bind(&risk.unrealized_pnl)
    .bind(&risk.interest_amount)
    .bind(&risk.maintenance_margin)
    .bind(&risk.margin_ratio)
    .bind(now.naive_utc())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 向用户私有频道推送单笔强平通知，载荷含入场价、标记价、盈亏、返还额和强平原因。
/// `reason` 让客户端能区分逐仓维持保证金触发与全仓账户级触发，两者的资金含义不同。
/// 只有累计利息用十八位小数字符串输出，其余十进制沿用默认序列化，与开平仓事件的处理保持一致。
/// 调用方必须在对应事务提交成功后才调用；本函数不校验仓位状态，也不产生任何资金写入。
/// 广播是尽力而为的，失败不会回滚已落地的强平，进程重启后也不补发历史事件。
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

/// 在逐仓强平事务内按主键对仓位加 FOR UPDATE，并联表取出产品当前的维持保证金率。
/// 不带状态和模式条件，因此可能锁到已平仓或全仓的记录，状态判定由调用方在锁定后完成。
/// 加锁把方向、名义价值、利息、入场价和资金域固定在同一版本上，避免与用户主动平仓交叉写入。
/// 维持保证金率实时从产品表联出而非用开仓时的历史值，后台调整强平线会立即影响本次判定。
/// 仓位不存在返回 None，调用方回滚后按跳过计数。
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

/// 在逐仓强平事务内写入一条强平审计记录，完整固化触发时刻的全部输入与计算结果。
/// 同时保存判定输入（入场价、标记价、维持保证金率、名义价值、利息）和判定输出
/// （权益、维持保证金、已实现盈亏、返还额），使事后无需依赖行情历史即可离线复算这次强平是否正确。
/// `reason` 硬编码为 `maintenance_margin`，与全仓路径的记录在同一张表里靠该列区分。
/// 与钱包入账、仓位终态处于同一事务，回滚时一并消失，不会留下无对应资金变动的孤立审计。
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

/// 在全仓账户强平事务内为其中一个仓位写强平审计，一次账户强平会逐笔调用多次。
/// 列结构与逐仓版本完全一致，仅 `reason` 固定为 `cross_maintenance_margin`，
/// 两条路径共用 `margin_liquidation_records` 一张表，靠该列区分处置类型。
/// 需要特别注意 `payout_amount` 的含义与逐仓不同：这里是按正权益占比分摊出的展示值，
/// 并非该仓位实际收到的入账，全仓真正的资金变更只在共享钱包上发生一次。
/// `equity` 记的是单仓权益而非账户权益，账户级的结算后余额与坏账另行写入账户行。
async fn insert_cross_liquidation_record(
    tx: &mut Transaction<'_, MySql>,
    position: &LockedCrossMarginPosition,
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
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'cross_maintenance_margin', ?)"#,
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

/// 把仓位的下次强平检查推迟六十秒，用于行情缺失、行情读取出错和强平执行失败三种异常路径。
/// 退避较长是为了避免行情链路中断时整轮扫描被同一批取不到价的仓位反复占满配额。
/// 全仓账户里取不到标记价的仓位也走同样的六十秒退避，由调用方直接调度。
async fn reschedule_liquidation_attempt(
    pool: &Pool<MySql>,
    position_id: u64,
    now: DateTime<Utc>,
) -> AppResult<()> {
    schedule_next_liquidation_attempt(pool, position_id, now + chrono::TimeDelta::seconds(60)).await
}

/// 把风险已恢复的安全仓位推迟五秒后再查，退避远短于异常路径的六十秒。
/// 差异化退避是这套调度的核心权衡：正常持仓保持接近实时的复查频率以免错过突然的行情跳空，
/// 而异常仓位拉长间隔避免拖慢整批扫描。五秒也与默认的扫描周期一致，形成稳定轮转。
async fn reschedule_safe_liquidation_check(
    pool: &Pool<MySql>,
    position_id: u64,
    now: DateTime<Utc>,
) -> AppResult<()> {
    schedule_next_liquidation_attempt(pool, position_id, now + chrono::TimeDelta::seconds(5)).await
}

/// 直接在连接池上更新仓位的下次强平检查时间，不参与任何强平事务。
/// 独立于事务是刻意设计：即便强平事务回滚，退避时间也必须留下来，否则失败仓位会被立刻重试并再次失败。
/// WHERE 带 `status = 'opened' AND entry_price IS NOT NULL` 条件，已终结仓位和未成交挂单都不会被排入风险调度。
/// 该列同时是跨重启检查点，进程重启后无需内存状态即可从数据库恢复原有的检查节奏。
async fn schedule_next_liquidation_attempt(
    pool: &Pool<MySql>,
    position_id: u64,
    next_attempt_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE margin_positions SET next_liquidation_attempt_at = ? WHERE id = ? AND status = 'opened' AND entry_price IS NOT NULL",
    )
    .bind(next_attempt_at.naive_utc())
    .bind(position_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 把逐仓强平的权益截断为非负并归一到十八位小数，得到实际返还给用户的金额。
/// 权益为负说明亏损已吃穿保证金，此时只退零，缺口在逐仓路径下不单独登记为坏账；
/// 与全仓账户结算把穿仓部分显式记入 `bad_debt` 的做法不同，这是两种模式的既定差异。
fn non_negative_amount(amount: &BigDecimal) -> BigDecimal {
    if amount > &BigDecimal::from(0) {
        amount.clone().with_scale(18)
    } else {
        BigDecimal::from(0).with_scale(18)
    }
}

/// 把金额固定格式化为十八位小数字符串，与钱包和仓位列的存储精度一致。
/// 强平事件中的累计利息走这里，避免 JSON 数值序列化把大额十进制转成浮点后丢精度。
fn decimal_amount_string(amount: &BigDecimal) -> String {
    format!("{amount:.18}")
}

/// 校验入场价或标记价严格大于零，`label` 只用于拼出可定位的错误文案。
/// 零价会让盈亏公式除零，负价会算出方向相反的结果，两者都必须在参与结算前拦下。
fn validate_positive_decimal(amount: &BigDecimal, label: &str) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        return Err(AppError::Validation(format!("{label} must be positive")));
    }
    Ok(())
}

/// 把单轮成功强平数上限夹到 1 到 100，同时也被复用为全仓账户候选的抓取上限。
/// 该配额由全仓账户与逐仓仓位共享，一个账户整体算作一次，无论它展开成多少笔仓位。
fn margin_liquidation_limit(limit: u32) -> u32 {
    limit.clamp(1, 100)
}

/// 推导逐仓候选的抓取上限，在成功配额基础上放大十倍再夹到 1 到 500。
/// 放大是因为候选里多数仓位风险正常会被跳过，只抓等量候选会让每轮实际处置数远低于配额。
/// 用 `saturating_mul` 防止极大配置值相乘溢出，最终仍被 500 的硬上限兜住。
fn margin_liquidation_scan_limit(limit: u32) -> u32 {
    margin_liquidation_limit(limit)
        .saturating_mul(10)
        .clamp(1, 500)
}

/// 读取布尔型环境变量，只接受 Rust 的 `true` 与 `false` 字面量，其余写法视为无法解析。
/// 缺失和解析失败都静默回落默认值，因此把开关拼错会得到「默认启用强平」而不是启动失败。
fn env_bool(key: &str, default: bool) -> bool {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<bool>().ok())
        .unwrap_or(default)
}

/// 读取无符号 64 位整型环境变量，用于强平扫描周期秒数，负值和非数字都回落默认。
/// 这里不夹范围，配置成零时由运行循环用 `max(1)` 兜底为至少一秒，不会变成忙轮询。
fn env_u64(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

/// 读取无符号 32 位整型环境变量，用于单轮强平配额，解析失败或缺失时回落默认值。
/// 同样不在此处夹范围，真正的 1 到 100 收敛发生在每轮执行时，配置越界不会导致超量处置。
fn env_u32(key: &str, default: u32) -> u32 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(default)
}

#[cfg(test)]
#[path = "../../tests/unit_src/src_workers_margin_liquidation_tests.rs"]
mod tests;
