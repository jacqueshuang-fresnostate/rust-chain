//! 常驻默认行情运行：固定分钟状态先持久化，再经共同所有权归档和现有量价发布链落地。

use serde::{Deserialize, Serialize};

mod follow;

use super::*;
use crate::modules::market::{
    DefaultMarketParameters,
    adapters::{DefaultTickerProvenance, PairTickerFence},
    generate_default_1m,
    infrastructure::default_runtime::{
        self, DefaultPairRow, PairGenerationLock, PairGenerationRun,
    },
    synthetic_default::forming_default_1m_values,
    synthetic_follow::FollowMinuteState,
    synthetic_realtime::build_synthetic_market_details_from_candle,
};

/// 固定分钟的确定性完整蜡烛与配置；逐秒观察价变化不会改变本分钟开盘锚点。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct StoredMinute {
    closed: MarketKlineSnapshot,
    seed: String,
    version: u32,
    anchor: BigDecimal,
    parameters: Option<DefaultMarketParameters>,
    price_precision: u32,
    qty_precision: u32,
    #[serde(default)]
    pending_close: Option<MarketKlineSnapshot>,
    #[serde(default)]
    pending_owner: Option<String>,
    #[serde(default)]
    follow: Option<FollowMinuteState>,
    #[serde(default)]
    accepted_follow: Option<FollowMinuteState>,
}

/// 默认候选单独受有界扫描限制；单交易对失败隔离，未抢到共同锁或等待交接边界计为跳过。
pub(super) async fn run_defaults(
    pool: &Pool<MySql>,
    mongo: &Database,
    ingestion: &MarketIngestionService,
    now: DateTime<Utc>,
    limit: u32,
    owner: &str,
) -> AppResult<SyntheticMarketSummary> {
    let rows = default_runtime::load_default_pairs(pool, now, limit).await?;
    let mut summary = SyntheticMarketSummary {
        scanned: rows.len() as u32,
        ..Default::default()
    };
    for row in rows {
        let pair_id = row.pair_id;
        let Some(mut lock) = PairGenerationLock::acquire(pool, pair_id, 0).await? else {
            summary.skipped += 1;
            continue;
        };
        let row = default_runtime::reload_default_pair(pool, pair_id).await?;
        let previous = default_runtime::load_pair_run(pool, pair_id).await?;
        match must_wait_for_boundary(previous.as_ref(), "default", None, None, now) {
            Ok(true) => {
                summary.skipped += 1;
                continue;
            }
            Err(error) => {
                summary.leased += 1;
                summary.failed += 1;
                warn!(pair_id,%error,"默认行情分钟状态无效");
                default_runtime::mark_pair_error(pool, pair_id, &error.to_string()).await;
                lock.release().await?;
                continue;
            }
            Ok(false) => {}
        }
        summary.leased += 1;
        match process_default(pool, mongo, ingestion, row, previous, now, owner, &mut lock).await {
            Ok(()) => summary.published += 1,
            Err(error) => {
                summary.failed += 1;
                warn!(pair_id,%error,"默认行情发布失败");
                default_runtime::mark_pair_error(pool, pair_id, &error.to_string()).await;
            }
        }
        lock.release().await?;
    }
    Ok(summary)
}

/// 交接时若当前分钟已被别的来源观察过，等待下一分钟，不重写半根旧蜡烛。
/// 暂停整体生成保留的来源信息以 state_json 为准，因此恢复后同源可以继续原固定分钟。
pub(super) fn must_wait_for_boundary(
    previous: Option<&PairGenerationRun>,
    source: &str,
    strategy_id: Option<u64>,
    strategy_version: Option<i32>,
    now: DateTime<Utc>,
) -> AppResult<bool> {
    let Some(previous) = previous else {
        return Ok(false);
    };
    let same = previous.active_source == source
        && previous.strategy_id == strategy_id
        && (source != "strategy" || previous.strategy_version == strategy_version);
    if same {
        return Ok(false);
    }
    let open = current_minute_open_time(now)?;
    let reserved_minute = stored_minute(previous)?;
    if previous.last_tick_at.is_none_or(|tick| tick < open)
        && reserved_minute
            .as_ref()
            .is_none_or(|state| state.closed.open_time() != open)
    {
        return Ok(false);
    }
    if source == "default"
        && previous.active_source == "none"
        && let Some(state) = stored_minute(previous)?
        && state.parameters.is_some()
        && state.closed.open_time() == open
    {
        return Ok(false);
    }
    Ok(true)
}

fn stored_minute(run: &PairGenerationRun) -> AppResult<Option<StoredMinute>> {
    if run.active_source != "none" && run.generation == 0 {
        return Err(AppError::Validation(
            "invalid synthetic source generation".into(),
        ));
    }
    run.state_json
        .as_ref()
        .map(|s| {
            serde_json::from_value(s.0.clone()).map_err(|e| {
                AppError::Validation(format!("invalid persisted synthetic minute: {e}"))
            })
        })
        .transpose()
}

/// 在默认推进及人工接管前核对上一跟随帧的归档事实，阻止待发价格混入闭合根。
/// 调用方持有交易对锁；这里只修正本轮读取快照，由既有分钟预留写入负责持久化。
pub(super) async fn reconcile_follow_run(
    pool: &Pool<MySql>,
    pair_id: u64,
    previous: Option<&mut PairGenerationRun>,
    now: DateTime<Utc>,
) -> AppResult<()> {
    if let Some(run) = previous
        && let Some(mut state) = stored_minute(run)?
    {
        follow::reconcile_archived_frame(pool, pair_id, &mut state, now).await?;
        run.state_json = Some(SqlxJson(
            serde_json::to_value(state).map_err(|e| AppError::Internal(e.to_string()))?,
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn process_default(
    pool: &Pool<MySql>,
    mongo: &Database,
    ingestion: &MarketIngestionService,
    row: DefaultPairRow,
    mut previous: Option<PairGenerationRun>,
    now: DateTime<Utc>,
    owner: &str,
    lock: &mut PairGenerationLock,
) -> AppResult<()> {
    let open = current_minute_open_time(now)?;
    reconcile_follow_run(pool, row.pair_id, previous.as_mut(), now).await?;
    let old = previous.as_ref().map(stored_minute).transpose()?.flatten();
    if let (Some(run), Some(state)) = (previous.as_ref(), old.as_ref())
        && run.active_source == "default"
        && run.default_version != Some(state.version)
    {
        return Err(AppError::Validation(
            "default minute configuration version mismatch".into(),
        ));
    }
    let close = source_transition_close(previous.as_ref(), now, owner)?;
    let mut state = if let Some(state) = old
        .as_ref()
        .filter(|s| s.closed.open_time() == open && s.parameters.is_some())
    {
        state.clone()
    } else {
        let p = u32::try_from(row.price_precision)
            .map_err(|_| AppError::Validation("invalid default price precision".into()))?;
        let q = u32::try_from(row.qty_precision)
            .map_err(|_| AppError::Validation("invalid default quantity precision".into()))?;
        let parameters: DefaultMarketParameters = serde_json::from_value(row.config_json.0)
            .map_err(|e| {
                AppError::Validation(format!("invalid default market configuration: {e}"))
            })?;
        let accepted = default_runtime::latest_archived_pair_price(pool, &row.symbol, now).await?;
        let price = close
            .as_ref()
            .map(|s| s.close().clone())
            .or(accepted)
            .or_else(|| previous.as_ref().and_then(|r| r.last_price.clone()))
            .or(row.initial_price)
            .ok_or_else(|| {
                AppError::Validation(
                    "default market initial price is required without existing history".into(),
                )
            })?;
        let anchor = if previous
            .as_ref()
            .is_some_and(|r| r.active_source == "default")
        {
            old.as_ref()
                .map(|s| s.anchor.clone())
                .unwrap_or_else(|| price.clone())
        } else {
            price.clone()
        };
        let candle = generate_default_1m(
            &row.symbol,
            &row.seed,
            row.version,
            p,
            q,
            open,
            &price,
            &anchor,
            &parameters,
        )?;
        StoredMinute {
            closed: MarketKlineSnapshot::new(
                MarketDataProvider::Strategy,
                &row.symbol,
                "1m",
                open,
                candle.values,
                open + TimeDelta::seconds(59),
            )
            .map_err(|e| AppError::Validation(e.to_string()))?,
            seed: row.seed,
            version: row.version,
            anchor,
            parameters: Some(parameters),
            price_precision: p,
            qty_precision: q,
            pending_close: close.clone(),
            pending_owner: close.as_ref().map(|_| owner.to_owned()),
            follow: old.as_ref().and_then(|s| s.follow.clone()),
            accepted_follow: old.as_ref().and_then(|s| s.accepted_follow.clone()),
        }
    };
    let now = follow::refresh_frame(pool, row.pair_id, &row.symbol, &mut state, now).await?;
    let state_json = serde_json::to_value(&state).map_err(|e| AppError::Internal(e.to_string()))?;
    lock.ensure_owned().await?;
    let generation = default_runtime::activate_pair_source(
        pool,
        row.pair_id,
        "default",
        None,
        state.version,
        owner,
        now,
        Some(&state_json),
    )
    .await?;
    let fence = pair_fence(row.pair_id, generation, owner, lock);
    let mut history = load_ticker_history(mongo, &row.symbol, open).await?;
    if let Some(close) = &close {
        replace_history_close(&mut history, close);
    }
    let closed = as_candle(&state.closed);
    let values = if let Some(follow) = &state.follow {
        as_candle(&follow.frame).values
    } else {
        forming_default_1m_values(
            &closed.values,
            open,
            now,
            state.price_precision,
            state.qty_precision,
        )?
    };
    let kline = MarketKlineSnapshot::new(
        MarketDataProvider::Strategy,
        &row.symbol,
        "1m",
        open,
        values.clone(),
        now,
    )
    .map_err(|e| AppError::Validation(e.to_string()))?;
    let ticker = build_ticker_snapshot(
        &row.symbol,
        values,
        now,
        &history.iter().map(|c| c.values.clone()).collect::<Vec<_>>(),
    )?;
    let parameters = state
        .parameters
        .as_ref()
        .ok_or_else(|| AppError::Internal("default parameters are missing".into()))?;
    let (depth, trade) = if let Some(follow) = &state.follow {
        follow::build_details(row.pair_id, &state, follow, &kline)?
    } else {
        build_synthetic_market_details_from_candle(
            &format!("default:{}:v{}", row.pair_id, state.version),
            &state.seed,
            state.version,
            state.price_precision,
            state.qty_precision,
            parameters.depth_levels,
            &closed,
            &kline,
        )?
    };
    lock.ensure_owned().await?;
    if !ingestion
        .ingest_and_publish_default_ticker(
            &ticker,
            &DefaultTickerProvenance {
                fence,
                config_version: state.version,
            },
        )
        .await?
        .is_accepted()
    {
        return Err(stale_market_write_conflict("default ticker"));
    }
    if let Some(close) = close {
        publish_cross_source_close(mongo, ingestion, &close, now, lock).await?;
        clear_pending_close(pool, row.pair_id, generation, owner).await?;
    }
    lock.ensure_owned().await?;
    if !ingestion
        .ingest_and_publish_synthetic_kline(&kline)
        .await?
        .is_accepted()
    {
        return Err(stale_market_write_conflict("default 1m"));
    }
    lock.ensure_owned().await?;
    if !ingestion
        .ingest_and_publish_synthetic_details(&ticker, &depth, trade.as_ref())
        .await?
        .is_accepted()
    {
        return Err(stale_market_write_conflict("default details"));
    }
    for interval in AGGREGATE_INTERVALS {
        let aggregate = build_forming_aggregate(&kline, interval, &history)?;
        lock.ensure_owned().await?;
        if !ingestion
            .ingest_and_publish_forming_aggregate(&aggregate)
            .await?
            .is_accepted()
        {
            return Err(stale_market_write_conflict("default forming aggregate"));
        }
    }
    lock.ensure_owned().await?;
    default_runtime::checkpoint_pair(
        pool,
        row.pair_id,
        generation,
        owner,
        ticker.last_price(),
        now,
    )
    .await
}

/// 创建归档可验证的共同锁证据；实际所有权随后由归档事务重新读取数据库核验。
pub(super) fn pair_fence(
    pair_id: u64,
    generation: u64,
    owner: &str,
    lock: &PairGenerationLock,
) -> PairTickerFence {
    PairTickerFence {
        pair_id,
        generation,
        owner: owner.into(),
        lock_name: lock.name().into(),
        connection_id: lock.connection_id(),
    }
}

/// 在本进程短间隔连续发布且恰好跨过一根分钟时闭合旧来源；重启或停机不补历史。
pub(super) fn online_previous_close(
    previous: Option<&PairGenerationRun>,
    state: Option<&StoredMinute>,
    now: DateTime<Utc>,
    owner: &str,
) -> AppResult<Option<MarketKlineSnapshot>> {
    let (Some(previous), Some(state)) = (previous, state) else {
        return Ok(None);
    };
    let Some(last) = previous.last_tick_at else {
        return Ok(None);
    };
    if state.parameters.as_ref().is_some_and(|p| {
        p.mode == crate::modules::market::synthetic_default::DefaultMarketMode::Follow
    }) && state.follow.is_none()
    {
        return Ok(None);
    }
    if previous.lease_owner.as_deref() != Some(owner)
        || now <= last
        || now - last > TimeDelta::seconds(5)
        || current_minute_open_time(now)? != state.closed.open_time() + TimeDelta::minutes(1)
    {
        return Ok(None);
    }
    Ok(Some(
        MarketKlineSnapshot::new(
            MarketDataProvider::Strategy,
            state.closed.symbol(),
            "1m",
            state.closed.open_time(),
            completed_values(state),
            now,
        )
        .map_err(|e| AppError::Validation(e.to_string()))?,
    ))
}

/// 人工策略也持久化固定分钟完整形态，使排期结束交给默认来源时可以完成同一根旧分钟。
pub(super) async fn store_manual_minute(
    pool: &Pool<MySql>,
    fence: &PairTickerFence,
    config: &SyntheticMarketConfig,
    qty_precision: u32,
    open: DateTime<Utc>,
    pending_close: Option<&MarketKlineSnapshot>,
) -> AppResult<()> {
    let candle = config
        .generate_1m(open)
        .map_err(|e| AppError::Validation(e.to_string()))?;
    let state = StoredMinute {
        closed: MarketKlineSnapshot::new(
            MarketDataProvider::Strategy,
            &config.symbol,
            "1m",
            open,
            candle.values.clone(),
            open + TimeDelta::seconds(59),
        )
        .map_err(|e| AppError::Validation(e.to_string()))?,
        seed: config.seed.clone(),
        version: config.version,
        anchor: candle.values.open,
        parameters: None,
        price_precision: config.price_precision,
        qty_precision,
        pending_close: pending_close.cloned(),
        pending_owner: pending_close.map(|_| fence.owner.clone()),
        follow: None,
        accepted_follow: None,
    };
    let json = serde_json::to_value(state).map_err(|e| AppError::Internal(e.to_string()))?;
    let updated = sqlx::query(
        "UPDATE market_pair_generation_runs SET state_json=? WHERE pair_id=? AND generation=?",
    )
    .bind(SqlxJson(json))
    .bind(fence.pair_id)
    .bind(fence.generation)
    .execute(pool)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(stale_market_write_conflict("manual minute state"));
    }
    Ok(())
}

/// 接管或重试时读取已持久化的完整分钟，不重新运行新来源算法覆盖旧来源。
/// 已登记且紧邻当前分钟的 pending 可由接管者完成；仅重新推导旧分钟仍受同进程五秒连续性限制。
pub(super) fn source_transition_close(
    previous: Option<&PairGenerationRun>,
    now: DateTime<Utc>,
    owner: &str,
) -> AppResult<Option<MarketKlineSnapshot>> {
    let state = previous.map(stored_minute).transpose()?.flatten();
    if let Some(state) = &state
        && state.closed.open_time() == current_minute_open_time(now)?
        && let Some(close) = &state.pending_close
        && close.open_time() + TimeDelta::minutes(1) == state.closed.open_time()
    {
        return Ok(Some(close.clone()));
    }
    online_previous_close(previous, state.as_ref(), now, owner)
}

/// 闭合旧分钟及其高周期成功后才清除待发布状态；失败重试仍保留原来源的完整分钟，代际与 owner 必须一致。
pub(super) async fn clear_pending_close(
    pool: &Pool<MySql>,
    pair_id: u64,
    generation: u64,
    owner: &str,
) -> AppResult<()> {
    let changed = sqlx::query(
        "UPDATE market_pair_generation_runs SET state_json=JSON_SET(state_json,'$.pending_close',NULL,'$.pending_owner',NULL) WHERE pair_id=? AND generation=? AND lease_owner=?",
    ).bind(pair_id).bind(generation).bind(owner).execute(pool).await?;
    if changed.rows_affected() != 1 {
        return Err(stale_market_write_conflict("pending minute close"));
    }
    Ok(())
}

fn as_candle(snapshot: &MarketKlineSnapshot) -> SyntheticCandle {
    SyntheticCandle {
        open_time: snapshot.open_time(),
        values: MarketKlineValues {
            open: snapshot.open().clone(),
            high: snapshot.high().clone(),
            low: snapshot.low().clone(),
            close: snapshot.close().clone(),
            volume: snapshot.volume().clone(),
        },
    }
}

fn completed_values(state: &StoredMinute) -> MarketKlineValues {
    let Some(follow) = &state.follow else {
        return as_candle(&state.closed).values;
    };
    let mut values = as_candle(&follow.frame).values;
    // 量由已固定的模拟分钟计划派生，价格高低仅来自实际观察；不预测缺失的参考价格。
    values.volume = state.closed.volume().clone();
    values
}

/// 计划内的闭合根替换旧 forming 根，ticker 与高周期基于同一份输入，不重复累计成交量。
pub(super) fn replace_history_close(
    history: &mut Vec<SyntheticCandle>,
    close: &MarketKlineSnapshot,
) {
    history.retain(|c| c.open_time != close.open_time());
    history.push(as_candle(close));
    history.sort_by_key(|c| c.open_time);
}

/// 共用闭合发布只处理当前连续在线的上一分钟，并在 UTC 边界从真实持久化根聚合高周期。
pub(super) async fn publish_cross_source_close(
    mongo: &Database,
    ingestion: &MarketIngestionService,
    close: &MarketKlineSnapshot,
    now: DateTime<Utc>,
    lock: &mut PairGenerationLock,
) -> AppResult<()> {
    lock.ensure_owned().await?;
    if !ingestion
        .ingest_and_publish_synthetic_kline(close)
        .await?
        .is_accepted()
    {
        return Err(stale_market_write_conflict("cross-source closed 1m"));
    }
    let end = close.open_time() + TimeDelta::minutes(1);
    for interval in AGGREGATE_INTERVALS {
        if end
            .timestamp()
            .rem_euclid(interval.minute_count() as i64 * 60)
            != 0
        {
            continue;
        }
        if let Some(candles) =
            load_aggregate_window_for_symbol(mongo, close.symbol(), interval, end).await?
        {
            let aggregate =
                build_aggregate_kline_snapshot(close.symbol(), interval, &candles, now)?;
            lock.ensure_owned().await?;
            if !ingestion
                .ingest_and_publish_synthetic_kline(&aggregate)
                .await?
                .is_accepted()
            {
                return Err(stale_market_write_conflict("cross-source aggregate"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_workers_synthetic_market_default_market_tests.rs"]
mod tests;
