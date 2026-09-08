//! 跟随模式把本秒参考证据冻结进原交易对分钟状态，再复用已有持锁发布路径。

use super::*;
use crate::modules::market::{
    MarketDepthSnapshot, MarketTradeTick,
    synthetic_default::DefaultMarketMode,
    synthetic_follow::{FollowFrameInput, advance_follow_frame},
    synthetic_realtime::build_observed_market_details,
};

/// 新秒前以归档事实区分冻结待发帧与已接受帧；同秒失败仍原样重试，不重读参考。
/// ticker 已归档而后续缓存/聚合失败亦视为已接受，避免重启丢失已用于价格消费的证据。
pub(super) async fn reconcile_archived_frame(
    pool: &Pool<MySql>,
    pair_id: u64,
    state: &mut StoredMinute,
    now: DateTime<Utc>,
) -> AppResult<()> {
    let Some(frame) = &state.follow else {
        return Ok(());
    };
    if now.timestamp() <= frame.frame.observed_at().timestamp() {
        return Ok(());
    }
    let accepted: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM market_price_ticks WHERE symbol=? AND source='default' AND source_version=CONCAT('default:',?,':g',generation,':v',?) AND observed_at=? AND price=?)",
    ).bind(frame.frame.symbol()).bind(pair_id).bind(state.version)
        .bind(frame.frame.observed_at().naive_utc()).bind(frame.frame.close())
        .fetch_one(pool).await?;
    if accepted {
        state.accepted_follow = state.follow.clone();
    } else {
        state.follow = state.accepted_follow.clone();
    }
    Ok(())
}

/// 同秒直接重放已冻结帧；新秒只读可信参考归档，正常缺失切独立震荡，存储故障仍停止本轮。
pub(super) async fn refresh_frame(
    pool: &Pool<MySql>,
    pair_id: u64,
    symbol: &str,
    state: &mut StoredMinute,
    now: DateTime<Utc>,
) -> AppResult<DateTime<Utc>> {
    let parameters = state
        .parameters
        .as_ref()
        .ok_or_else(|| AppError::Internal("default parameters are missing".into()))?;
    if parameters.mode != DefaultMarketMode::Follow {
        state.follow = None;
        state.accepted_follow = None;
        return Ok(now);
    }
    let now = DateTime::from_timestamp(now.timestamp(), 0)
        .ok_or_else(|| AppError::Validation("跟随行情时间无效".into()))?;
    if state.follow.as_ref().is_some_and(|f| {
        f.frame.open_time() == state.closed.open_time() && f.frame.observed_at() == now
    }) {
        return Ok(now);
    }
    let config = parameters
        .follow
        .as_ref()
        .ok_or_else(|| AppError::Validation("跟随行情缺少参考配置".into()))?;
    let reference = default_runtime::load_follow_reference(
        pool,
        pair_id,
        config.reference_pair_id,
        now,
        config.stale_after_seconds,
        state
            .follow
            .as_ref()
            .and_then(|f| f.last_reference.as_ref().map(|r| r.source.as_str())),
    )
    .await?;
    let fallback = as_candle(&state.closed);
    let frame = advance_follow_frame(
        &FollowFrameInput {
            symbol,
            parameters,
            price_precision: state.price_precision,
            qty_precision: state.qty_precision,
            fallback: &fallback,
            now,
            reference_symbol: &reference.symbol,
            unavailable_reason: reference.reason.as_deref(),
        },
        state.follow.as_ref(),
        reference.observation,
    )?;
    state.follow = Some(frame);
    Ok(now)
}

/// 模拟量按固定分钟累计差分；盘口和成交价跟随本秒已冻结的本币收盘，不把参考真实成交拷贝到本币。
pub(super) fn build_details(
    pair_id: u64,
    state: &StoredMinute,
    followed: &FollowMinuteState,
    current: &MarketKlineSnapshot,
) -> AppResult<(MarketDepthSnapshot, Option<MarketTradeTick>)> {
    let config = state
        .parameters
        .as_ref()
        .ok_or_else(|| AppError::Internal("default parameters are missing".into()))?;
    let previous_volume = if current.observed_at() == current.open_time() {
        BigDecimal::from(0)
    } else {
        forming_default_1m_values(
            &as_candle(&state.closed).values,
            current.open_time(),
            current.observed_at() - TimeDelta::seconds(1),
            state.price_precision,
            state.qty_precision,
        )?
        .volume
    };
    build_observed_market_details(
        &format!("default:{pair_id}:v{}", state.version),
        &state.seed,
        state.version,
        state.price_precision,
        state.qty_precision,
        config.depth_levels,
        state.closed.volume(),
        current,
        &followed.previous_price,
        current.volume() - previous_volume,
    )
}
