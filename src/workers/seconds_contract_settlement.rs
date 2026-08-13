//! 秒合约到期结算 worker：周期性扫描已过期但仍处于 `opened` 的订单，判定胜负并完成派奖。
//!
//! 结算的两个价格来源不同且都由服务端掌握：开仓价是下单时写入订单行的快照，
//! 结算价则在本 worker 判定时从 Redis 行情缓存实时读取，要求价格为正且观测时间在 60 秒以内，
//! 因此结算价锚定的是「到期后首次成功处理的时刻」而非严格的到期毫秒，行情中断期间会推迟而不是取旧价。
//! 胜负规则为看涨时结算价高于开仓价判赢、看跌时结算价低于开仓价判赢，两价相等一律判输。
//!
//! 每笔订单独立开事务处理，互不影响：锁订单、按需锁钱包、派奖入账、写流水、置终态一次性提交。
//! 幂等由订单终态承担，`UPDATE ... WHERE status = 'opened'` 的影响行数是最后一道保险，
//! 与后台人工结算并发时只有一方能改到状态，另一方回滚并计入跳过，绝不会重复派奖。
//! 单笔失败不影响整批：把该订单的下次尝试时间推后 60 秒后继续处理下一笔。
//! 私有结算事件只在事务提交成功之后尽力广播，既不落库也不补发，客户端最终应以订单查询结果为准。

use crate::{
    error::{AppError, AppResult},
    modules::{
        events::{EventBroadcastHub, EventBroadcastMessage},
        market::market_ticker_redis_key,
        seconds_contract::service::seconds_contract_payout_amount,
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

/// 秒合约结算 worker 的无状态入口类型，仅用于把单轮结算暴露成方法形式供调度侧持有。
/// 本身不保存连接、游标或上一轮进度，重启后完全依赖订单终态与下次尝试时间恢复。
pub struct SecondsContractSettlementWorker;

/// 结算 worker 的运行参数，全部来自环境变量并在启动时读取一次，运行期间不热更新。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecondsContractSettlementWorkerConfig {
    /// 是否启用该 worker，关闭后到期订单只能靠后台人工结算。
    pub enabled: bool,
    /// 两轮扫描之间的间隔秒数，实际执行时会被抬到至少 1 秒。
    pub interval_seconds: u64,
    /// 单轮期望结算的订单数上限，最终会被收敛到 1 到 100 之间。
    pub batch_limit: u32,
}

impl SecondsContractSettlementWorkerConfig {
    /// 读取秒合约结算开关、周期与批量环境配置；默认启用、周期 5 秒、批量 100，缺失或不可解析值回落到默认值。
    pub fn from_env() -> Self {
        Self {
            enabled: env_bool("SECONDS_CONTRACT_SETTLEMENT_ENABLED", true),
            interval_seconds: env_u64("SECONDS_CONTRACT_SETTLEMENT_INTERVAL_SECONDS", 5),
            batch_limit: env_u32("SECONDS_CONTRACT_SETTLEMENT_BATCH_LIMIT", 100),
        }
    }
}

impl SecondsContractSettlementWorker {
    /// 执行一轮秒合约到期结算；成功和候选扫描都收敛到 1..=100，结算价只接受新鲜 Redis ticker。
    /// 方法体只是转发到同名自由函数，保留该包装是为了让调度侧持有一个可替换的对象而非直接依赖自由函数。
    /// 时间基准由调用方传入而非取当前时间，使单轮内所有订单共用同一时间线，判定可复现也便于测试。
    /// 每单独立锁定订单和钱包，失败或缺价时把该单推迟 60 秒后继续处理下一单，单笔异常不影响整批。
    /// 私有结算事件只在对应资金与订单事务提交成功之后才广播。
    pub async fn run_once(
        &self,
        state: &AppState,
        now: DateTime<Utc>,
        limit: u32,
    ) -> AppResult<SecondsContractSettlementSummary> {
        run_once(state, now, limit).await
    }
}

/// 单轮结算的计数汇总，仅用于日志与测试断言，不参与任何业务判定。
/// 四项之间不构成恒等式：扫描数含被上限提前打断前已处理的订单，其余三项按各自分支累加。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SecondsContractSettlementSummary {
    /// 本轮实际进入处理流程的候选订单数。
    pub scanned: u32,
    /// 真正完成结算并提交事务的订单数，达到上限即停止本轮。
    pub settled: u32,
    /// 因行情暂缺或订单状态已变化而跳过的订单数，属于预期内情况。
    pub skipped: u32,
    /// 因缺开仓价、读行情出错、结果计算失败或结算事务出错而未完成的订单数。
    pub failed: u32,
}

/// 候选订单扫描结果，只取判定胜负所需的最小字段集，避免批量扫描时拉取无用列。
/// 这里读到的值不带锁，仅用于筛选与取价，真正结算前会在事务内重新加锁读取权威快照。
#[derive(Debug, sqlx::FromRow)]
struct DueSecondsContractOrder {
    /// 订单主键。
    order_id: u64,
    /// 交易对符号，用于拼接行情缓存键取结算价。
    symbol: String,
    /// 下单方向，参与胜负判定。
    direction: String,
    /// 开仓价快照；理论上不应为空，为空说明数据异常，该单会被计为失败并推迟重试。
    entry_price: Option<BigDecimal>,
}

/// 结算事务内加锁读到的订单权威快照，字段覆盖派奖计算与事件推送所需的全部信息。
/// 与扫描结果相比额外带上资产精度、本金、赔率与当前状态，用于幂等判断和金额量化。
#[derive(Debug, sqlx::FromRow)]
struct LockedSecondsContractOrder {
    /// 订单主键。
    id: u64,
    /// 订单归属用户，也是派奖入账与事件推送的目标。
    user_id: u64,
    /// 产品编号，仅用于事件回传。
    product_id: u64,
    /// 交易对编号，仅用于事件回传。
    pair_id: u64,
    /// 质押资产编号，决定操作哪个钱包账户。
    stake_asset: u64,
    /// 质押资产小数位，用于把赔付额向零截断到可入账精度。
    stake_asset_precision: i32,
    /// 下单方向，随事件回传给客户端。
    direction: String,
    /// 投注本金，开仓时已扣除，此处只作为赔付计算的基数。
    stake_amount: BigDecimal,
    /// 下单时固化的赔率，结算按此值计算而非产品当前配置。
    payout_rate: BigDecimal,
    /// 当前订单状态，是幂等判断的依据。
    status: String,
    /// 既有结算结果，仅在状态已为 `settled` 时有值。
    result: Option<String>,
    /// 开仓价快照，缺失时拒绝结算。
    entry_price: Option<BigDecimal>,
}

/// 钱包账户在行锁保护下的三段余额快照，结算只增加可用余额，另两项原样写进流水。
#[derive(Debug, sqlx::FromRow)]
struct WalletBalanceRow {
    /// 可用余额，派奖直接加到这一项上。
    available: BigDecimal,
    /// 冻结余额，结算过程中不改动。
    frozen: BigDecimal,
    /// 锁定余额，结算过程中不改动。
    locked: BigDecimal,
}

/// 行情缓存 ticker 的最小反序列化视图，只取结算取价所需字段。
#[derive(Debug, Deserialize)]
struct CachedTickerPayload {
    /// 最新成交价，作为结算价与开仓价比对。
    last_price: BigDecimal,
    /// 该报价的观测时刻，以毫秒时间戳存储，用于判定行情是否过于陈旧而不可用于结算。
    #[serde(with = "unix_millis")]
    observed_at: DateTime<Utc>,
}

/// 一次成功结算的事件载荷，由结算事务在提交前组装、提交后才允许广播。
/// 载荷携带的是已落库的终态数据，因此广播失败不影响资金正确性，只影响推送时效。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecondsContractSettlementEvent {
    /// 事件收件用户，即订单归属人。
    user_id: u64,
    /// 已结算的订单主键。
    order_id: u64,
    /// 产品编号。
    product_id: u64,
    /// 交易对编号。
    pair_id: u64,
    /// 质押资产编号。
    stake_asset: u64,
    /// 下单方向。
    direction: String,
    /// 投注本金。
    stake_amount: BigDecimal,
    /// 实际入账赔付额，输单为零。
    payout_amount: BigDecimal,
    /// 本次采用的结算价。
    settlement_price: BigDecimal,
    /// 胜负结果。
    result: String,
}

/// 单笔订单结算事务的两种终局，用于把「已派奖」与「无需处理」区分开。
/// 真正的错误不走这个枚举，而是通过 `Err` 返回，由调用方安排重试。
#[derive(Debug, Clone, PartialEq, Eq)]
enum SettlementOutcome {
    /// 本次事务完成了结算并已提交，携带待广播的事件载荷。
    Settled(Box<SecondsContractSettlementEvent>),
    /// 订单已被他方结算、已不在 `opened`、或并发条件竞争失败，本次不做任何资金变更。
    Skipped,
}

/// 比较开仓价与结算价得出秒合约胜负，是整个模块唯一的判定口径。
/// 看涨方向要求结算价严格高于开仓价才算赢，看跌方向要求严格低于开仓价才算赢；
/// 两价完全相等时无论方向都判为 `loss`，即平价不退本金，这是平台侧的既定规则而非四舍五入误差所致。
/// 比较基于 `BigDecimal` 精确值，不做任何精度截断或容差处理，避免尾差改变胜负。
/// 方向不是 `up` 或 `down` 时返回 `AppError::Validation`，调用方据此把该单标记为失败并推迟重试。
/// 本函数只算结果，不读赔率、不动钱包、不改订单状态。
pub fn seconds_contract_settlement_result(
    direction: &str,
    entry_price: &BigDecimal,
    exit_price: &BigDecimal,
) -> AppResult<&'static str> {
    match direction {
        "up" if exit_price > entry_price => Ok("win"),
        "up" => Ok("loss"),
        "down" if exit_price < entry_price => Ok("win"),
        "down" => Ok("loss"),
        _ => Err(AppError::Validation(
            "seconds contract direction must be up or down".to_owned(),
        )),
    }
}

/// 从应用状态取得 MySQL 订单/钱包、Redis 权威 ticker 与可选事件 hub 后执行单轮结算；MySQL 或 Redis 缺失时在扫描前失败。
/// Redis 是硬依赖而非可选加速：结算价只能取自行情缓存，缺失时宁可整轮不执行也不按其他来源定价。
/// 事件广播 hub 则是可选的，缺失只影响推送时效，不影响资金正确性。
/// 两项依赖的检查都在扫描之前完成，因此配置缺失不会造成半轮处理。
/// 单轮成功与候选扫描上限均为 1 到 100；进程内事件仅在对应结算事务提交后尽力广播。
pub async fn run_once(
    state: &AppState,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<SecondsContractSettlementSummary> {
    let pool = state.mysql.as_ref().ok_or_else(|| {
        AppError::Internal("mysql pool is required for seconds contract settlement".to_owned())
    })?;
    let redis = state.redis.as_ref().ok_or_else(|| {
        AppError::Internal(
            "redis connection is required for seconds contract settlement".to_owned(),
        )
    })?;
    run_once_with_broadcast(pool, redis, state.event_broadcast_hub.as_ref(), now, limit).await
}

/// 在显式 MySQL/Redis 依赖上执行同一批次但禁用进程内广播；扫描上限、行情新鲜度、订单→钱包锁序、幂等及单项失败继续规则不变。
pub async fn run_once_with_dependencies(
    pool: &Pool<MySql>,
    redis: &ConnectionManager,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<SecondsContractSettlementSummary> {
    run_once_with_broadcast(pool, redis, None, now, limit).await
}

/// 执行单轮到期结算的核心实现，扫描候选、逐笔判定并派奖，返回本轮各项计数。
/// 成功结算数与候选扫描数都以 `limit` 收敛到 1 到 100，不额外放大扫描量；
/// 达到结算上限即立即跳出循环，剩余到期订单留给下一轮，保证单轮耗时可控。
/// 每笔订单在自己的事务内按先锁订单再锁钱包的固定顺序处理，与后台人工结算保持同序以避免死锁；
/// 已提交的前几笔不会因后续某笔失败而回滚，因此本轮天然是部分成功语义。
/// 四类异常各自分流：缺开仓价与结果计算失败计为失败，行情键缺失计为跳过，读行情或结算事务出错计为失败，
/// 四者都会把该订单的下次尝试时间推后 60 秒后继续处理下一笔，不中断整轮。
/// 私有结算事件只在对应事务提交成功后尽力广播，不持久化也不补发。
pub async fn run_once_with_broadcast(
    pool: &Pool<MySql>,
    redis: &ConnectionManager,
    hub: Option<&EventBroadcastHub>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<SecondsContractSettlementSummary> {
    let settlement_limit = seconds_contract_settlement_limit(limit);
    let rows = fetch_due_orders(pool, now, seconds_contract_settlement_scan_limit(limit)).await?;
    let mut summary = SecondsContractSettlementSummary::default();

    for row in rows {
        if summary.settled >= settlement_limit {
            break;
        }
        summary.scanned += 1;
        let Some(entry_price) = row.entry_price.as_ref() else {
            summary.failed += 1;
            reschedule_settlement_attempt(pool, row.order_id, now).await?;
            warn!(order_id = row.order_id, "秒合约结算跳过缺失开仓价订单");
            continue;
        };
        let exit_price = match cached_ticker_price(redis, &row.symbol, now).await {
            Ok(Some(price)) => price,
            Ok(None) => {
                summary.skipped += 1;
                reschedule_settlement_attempt(pool, row.order_id, now).await?;
                warn!(order_id = row.order_id, symbol = %row.symbol, "秒合约结算跳过缺失行情订单");
                continue;
            }
            Err(error) => {
                summary.failed += 1;
                reschedule_settlement_attempt(pool, row.order_id, now).await?;
                warn!(order_id = row.order_id, symbol = %row.symbol, %error, "秒合约结算读取行情失败");
                continue;
            }
        };
        let result =
            match seconds_contract_settlement_result(&row.direction, entry_price, &exit_price) {
                Ok(result) => result,
                Err(error) => {
                    summary.failed += 1;
                    reschedule_settlement_attempt(pool, row.order_id, now).await?;
                    warn!(order_id = row.order_id, %error, "秒合约结算结果计算失败");
                    continue;
                }
            };
        match settle_order_by_id(pool, row.order_id, result, &exit_price).await {
            Ok(SettlementOutcome::Settled(event)) => {
                summary.settled += 1;
                publish_settlement_event(hub, &event);
            }
            Ok(SettlementOutcome::Skipped) => summary.skipped += 1,
            Err(error) => {
                summary.failed += 1;
                reschedule_settlement_attempt(pool, row.order_id, now).await?;
                warn!(order_id = row.order_id, %error, "秒合约结算失败");
            }
        }
    }

    Ok(summary)
}

/// 以至少 1 秒间隔持续结算；周期级查询错误只记录并进入下一轮，单项错误由批次安排重试后继续。
/// 间隔为零时被抬到 1 秒，避免误配导致空转打满数据库。
/// 本函数是无限循环且永不正常返回，返回类型上的错误位仅为与调度接口保持一致。
/// 整轮失败只记错误日志不中断循环，因为常见原因是数据库或行情短暂不可用，下一轮即可自愈。
/// 每轮成功都会输出扫描、结算、跳过、失败四项计数，供观察积压是否在收敛。
/// 循环自身不保存任何进度：跨重启的恢复完全依赖订单终态与下次尝试时间，
/// 因此进程重启不会重复派奖，但提交后未发出的广播也不会补发。
pub async fn run_loop(state: AppState, interval_seconds: u64, limit: u32) -> AppResult<()> {
    let mut ticker = interval(Duration::from_secs(interval_seconds.max(1)));

    loop {
        ticker.tick().await;
        match run_once(&state, Utc::now(), limit).await {
            Ok(summary) => info!(
                scanned = summary.scanned,
                settled = summary.settled,
                skipped = summary.skipped,
                failed = summary.failed,
                "秒合约结算周期完成"
            ),
            Err(error) => error!(%error, "秒合约结算周期失败"),
        }
    }
}

/// 扫描本轮待结算的候选订单：状态仍为 `opened`、到期时刻已不晚于 `now`，
/// 且下次尝试时间为空或已到期，后一条件让上一轮失败的订单在 60 秒退避期内被自然跳过。
/// 按到期时间升序、主键升序排列，使最早到期的订单优先结算，主键作为唯一列保证顺序稳定。
/// 条数在这里另做一次 1 到 500 的夹取，属于防御性上限，正常路径传入的值已被收敛到 100 以内。
/// 时间参数按 `naive_utc` 绑定以匹配数据库列的存储形态；查询走连接池、不加锁，
/// 因此读到的候选可能在真正结算前被他方改动，最终一致性由结算事务内的加锁复查保证。
async fn fetch_due_orders(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<Vec<DueSecondsContractOrder>> {
    sqlx::query_as::<_, DueSecondsContractOrder>(
        r#"SELECT orders.id AS order_id,
                  pairs.symbol,
                  orders.direction,
                  orders.entry_price
           FROM seconds_contract_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           WHERE orders.status = 'opened'
             AND orders.expires_at <= ?
             AND (orders.next_settlement_attempt_at IS NULL OR orders.next_settlement_attempt_at <= ?)
           ORDER BY orders.expires_at ASC, orders.id ASC
           LIMIT ?"#,
    )
    .bind(now.naive_utc())
    .bind(now.naive_utc())
    .bind(limit.clamp(1, 500) as i64)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

/// 把一笔本轮未能结算的订单的下次尝试时间推后 60 秒，实现固定间隔的退避重试。
/// 退避是常量而非指数递增，因为最常见的失败原因是行情短暂缺失，属于很快能自愈的瞬时故障。
/// 更新附带 `status = 'opened'` 条件，若订单已被他方结算则不会误改终态订单的字段。
/// 该更新在自身连接上独立提交，不属于任何结算事务，因此即使随后结算失败退避也已生效。
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

/// 从行情缓存取该交易对的最新价作为结算价，返回值刻意区分三种情形。
/// 缓存键不存在返回 `Ok(None)`，表示行情暂时不可用，调用方计为跳过并推迟重试；
/// JSON 解析失败返回 `AppError::Internal`，因为那是行情写入方与本 worker 的数据契约破损；
/// 价格非正或观测时刻早于 `now` 之前 60 秒返回 `AppError::Validation`，两者都属于不可用于资金判定的行情。
/// 新鲜度以传入的 `now` 而非函数内部当前时间为基准，使单轮内所有订单共用同一时间基线，判定可复现。
/// 60 秒窗口意味着行情中断期间订单会被反复推迟而不会按陈旧价结算。
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
    let ticker = serde_json::from_str::<CachedTickerPayload>(&payload).map_err(|error| {
        AppError::Internal(format!(
            "invalid cached seconds contract ticker payload: {error}"
        ))
    })?;
    if ticker.last_price <= 0 {
        return Err(AppError::Validation(
            "seconds contract exit price must be positive".to_owned(),
        ));
    }
    if ticker.observed_at < now - chrono::TimeDelta::seconds(60) {
        return Err(AppError::Validation(
            "seconds contract ticker is stale".to_owned(),
        ));
    }
    Ok(Some(ticker.last_price))
}

/// 在独立事务中完成一笔订单的结算：锁订单、判幂等、按快照赔率算赔付、赢单派奖入账、置订单终态。
/// 本金在开仓时已从可用余额扣走，这里不再扣款，只在赔付额为正时把金额加回可用余额并写一条
/// `seconds_contract_settle_win` 流水；输单赔付为零，直接跳过整段钱包操作，不产生任何资金流水。
/// 赔付额由订单固化的本金与赔率算出，并按质押资产精度向零截断，与后台人工结算共用同一口径。
/// 幂等分三层：订单已是 `settled` 且结果相同则提交空事务并跳过，结果不同返回冲突以免覆盖他方结论；
/// 状态既非 `settled` 也非 `opened` 直接回滚跳过；最后的置终态语句带 `status = 'opened'` 条件，
/// 影响行数不为 1 说明并发中已被他人改动，整笔回滚并跳过，是防重复派奖的最后一道保险。
/// 钱包更新同样校验影响行数，异常时回滚，避免余额未变却记了流水。
/// 开仓价缺失返回校验错误。事件载荷在提交前组装、提交后才返回，本函数自身不广播任何消息。
async fn settle_order_by_id(
    pool: &Pool<MySql>,
    order_id: u64,
    result: &str,
    settlement_price: &BigDecimal,
) -> AppResult<SettlementOutcome> {
    let mut tx = pool.begin().await?;
    let Some(order) = lock_order_by_id(&mut tx, order_id).await? else {
        tx.rollback().await?;
        return Ok(SettlementOutcome::Skipped);
    };
    if order.status == "settled" {
        if order.result.as_deref() != Some(result) {
            return Err(AppError::Conflict(
                "seconds contract order was settled with a different result".to_owned(),
            ));
        }
        tx.commit().await?;
        return Ok(SettlementOutcome::Skipped);
    }
    if order.status != "opened" {
        tx.rollback().await?;
        return Ok(SettlementOutcome::Skipped);
    }
    if order.entry_price.is_none() {
        return Err(AppError::Validation(
            "seconds contract entry price is required for settlement".to_owned(),
        ));
    }

    let payout_amount = seconds_contract_payout_amount(
        &order.stake_amount,
        &order.payout_rate,
        result,
        order.stake_asset_precision,
    );
    if payout_amount > 0 {
        let wallet = lock_wallet_row(&mut tx, order.user_id, order.stake_asset).await?;
        let available_after = wallet.available.clone() + payout_amount.clone();
        let wallet_update = sqlx::query(
            "UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?",
        )
        .bind(&available_after)
        .bind(order.user_id)
        .bind(order.stake_asset)
        .execute(&mut *tx)
        .await?;
        if wallet_update.rows_affected() != 1 {
            tx.rollback().await?;
            return Ok(SettlementOutcome::Skipped);
        }
        sqlx::query(
            r#"INSERT INTO wallet_ledger
               (user_id, asset_id, change_type, amount, balance_type, balance_after,
                available_after, frozen_after, locked_after, ref_type, ref_id)
               VALUES (?, ?, 'seconds_contract_settle_win', ?, 'available', ?, ?, ?, ?, 'seconds_contract_order', ?)"#,
        )
        .bind(order.user_id)
        .bind(order.stake_asset)
        .bind(&payout_amount)
        .bind(&available_after)
        .bind(&available_after)
        .bind(&wallet.frozen)
        .bind(&wallet.locked)
        .bind(order.id.to_string())
        .execute(&mut *tx)
        .await?;
    }

    let update = sqlx::query(
        "UPDATE seconds_contract_orders SET status = 'settled', result = ?, settlement_price = ?, settled_at = CURRENT_TIMESTAMP(6) WHERE id = ? AND status = 'opened'",
    )
    .bind(result)
    .bind(settlement_price)
    .bind(order.id)
    .execute(&mut *tx)
    .await?;
    if update.rows_affected() != 1 {
        tx.rollback().await?;
        return Ok(SettlementOutcome::Skipped);
    }

    let event = SecondsContractSettlementEvent {
        user_id: order.user_id,
        order_id: order.id,
        product_id: order.product_id,
        pair_id: order.pair_id,
        stake_asset: order.stake_asset,
        direction: order.direction,
        stake_amount: order.stake_amount,
        payout_amount,
        settlement_price: settlement_price.clone(),
        result: result.to_owned(),
    };
    tx.commit().await?;
    Ok(SettlementOutcome::Settled(Box::new(event)))
}

/// 把已提交的结算结果推送到该用户的私有频道，事件类型与后台人工结算保持一致，
/// 使客户端无需区分订单是被 worker 自动结算还是被人工结算。
/// `status` 字段固定为 `settled`，因为只有成功提交的结算才会走到这里。
/// 未配置广播 hub 时静默跳过：事件属于时效性通知，缺失只影响推送而不影响已入账的资金。
/// 本函数为尽力投递，不重试也不落库，客户端最终应以订单查询结果为准。
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
                "payout_amount": event.payout_amount,
                "result": event.result,
                "status": "settled",
            })
            .to_string(),
        ));
    }
}

/// 在结算事务内以 `FOR UPDATE` 锁定订单并连带取出质押资产精度，是每笔结算的第一个加锁动作。
/// 锁定后读到的状态与结果才是可信的幂等判断依据，扫描阶段的非锁读取只能用于筛选。
/// 连接资产表取精度而不是另发一次查询，既减少往返也保证精度与订单在同一事务快照内一致。
/// 订单不存在返回 `Ok(None)` 而非错误，调用方按跳过处理，因为这通常意味着记录已被清理而非故障。
/// 锁在调用方事务提交或回滚时释放，期间后台人工结算同一订单会被阻塞。
async fn lock_order_by_id(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
) -> AppResult<Option<LockedSecondsContractOrder>> {
    sqlx::query_as::<_, LockedSecondsContractOrder>(
        r#"SELECT orders.id, orders.user_id, orders.product_id, orders.pair_id,
                  orders.stake_asset, assets.precision_scale AS stake_asset_precision,
                  orders.direction, orders.stake_amount, orders.payout_rate,
                  orders.status, orders.result, orders.entry_price
           FROM seconds_contract_orders orders
           INNER JOIN assets ON assets.id = orders.stake_asset
           WHERE orders.id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(order_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

/// 在结算事务内锁定收款钱包行并取回三段余额快照，只有赢单需要调用。
/// 加锁顺序固定排在订单锁之后，与开仓和后台人工结算保持一致，避免不同资金路径互相死锁。
/// 钱包账户不存在时返回校验错误而不是自动开户，此时整笔结算回滚，该单会被推迟到下一轮重试，
/// 因为在资金路径上隐式建账会掩盖账户数据本身的异常。
/// 返回的冻结与锁定余额不参与计算，只用于写流水时记录变更当时的完整余额分布。
async fn lock_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<WalletBalanceRow> {
    sqlx::query_as::<_, WalletBalanceRow>(
        r#"SELECT available, frozen, locked
           FROM wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| {
        AppError::Validation(
            "wallet account is required for seconds contract settlement".to_owned(),
        )
    })
}

/// 把配置传入的单轮结算上限夹到 1 到 100。
/// 下限防止误配为零导致每轮什么都不做，上限防止单轮处理过多订单而长时间占用连接并拉长整轮耗时。
/// 超限只静默截断不报错，因为这是防御性收敛而非配置校验。
fn seconds_contract_settlement_limit(limit: u32) -> u32 {
    limit.clamp(1, 100)
}

/// 计算候选订单的扫描条数上限，取值与结算上限完全相同，即不做超额预取。
/// 二者刻意保持一致：多扫出来的订单本轮也无法结算，只会白白读取并延后各自的重试时间。
fn seconds_contract_settlement_scan_limit(limit: u32) -> u32 {
    seconds_contract_settlement_limit(limit).clamp(1, 100)
}

/// 读取布尔型环境变量，只接受 `true` 与 `false` 字面量。
/// 变量缺失或无法解析都回落到默认值而不是报错，避免部署环境写错一个开关就让整个进程起不来。
fn env_bool(key: &str, default: bool) -> bool {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<bool>().ok())
        .unwrap_or(default)
}

/// 读取 64 位无符号整型环境变量，用于结算轮询间隔秒数。
/// 负数、小数或非数字文本都会解析失败并回落到默认值；此处不校验取值范围，
/// 间隔为 0 的情形由调用侧抬到至少 1 秒。
fn env_u64(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

/// 读取 32 位无符号整型环境变量，用于单轮结算批量上限。
/// 解析失败回落默认值；此处同样不夹取范围，上下限由 `seconds_contract_settlement_limit` 统一收敛。
fn env_u32(key: &str, default: u32) -> u32 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(default)
}

#[cfg(test)]
#[path = "../../tests/unit_src/src_workers_seconds_contract_settlement_tests.rs"]
mod tests;
