//! 跟随来源只读现有外部采样归档；不请求交易所，不把旧缓存或平台自生成行情当作参考。

use super::*;
use crate::modules::market::{
    sanitize_symbol,
    synthetic_follow::{FollowObservation, select_follow_reference},
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, TimeDelta, Utc};

/// 精确交易对的可用参考证据；缺失/停用/过期作为正常降级原因，存储故障仍返回错误。
pub(crate) struct FollowReferenceRead {
    pub symbol: String,
    pub observation: Option<FollowObservation>,
    pub reason: Option<String>,
}

#[derive(sqlx::FromRow)]
struct Tick {
    event_key: String,
    source: String,
    price: BigDecimal,
    observed_at: DateTime<Utc>,
}

/// 读取不晚于生成秒的已归档正价格；优先保留已有健康供应商，换供应商由纯状态机重锚。
/// 拒绝自引用及非 active external 引用，未来/过期证据不推进跟随；归档时间是采集链语义而非交易所终局保证。
pub(crate) async fn load_follow_reference(
    pool: &Pool<MySql>,
    pair_id: u64,
    reference_pair_id: u64,
    now: DateTime<Utc>,
    stale_after_seconds: u32,
    preferred_source: Option<&str>,
) -> AppResult<FollowReferenceRead> {
    let row = sqlx::query_as::<_, (String, String, String)>(
        "SELECT symbol,status,market_type FROM trading_pairs WHERE id=?",
    )
    .bind(reference_pair_id)
    .fetch_optional(pool)
    .await?;
    let (symbol, valid) = row.map_or_else(
        || (String::new(), false),
        |(symbol, status, kind)| {
            (
                symbol,
                pair_id != reference_pair_id && status == "active" && kind == "external",
            )
        },
    );
    let unavailable = |reason: &str| FollowReferenceRead {
        symbol: symbol.clone(),
        observation: None,
        reason: Some(reason.into()),
    };
    if !valid {
        return Ok(unavailable(
            "参考交易对不存在、已停用或不是外部行情，正在独立震荡",
        ));
    }
    let normalized = sanitize_symbol(&symbol);
    let mut candidates = Vec::with_capacity(3);
    for source in ["bitget", "htx", "coinbase"] {
        if let Some(tick) = sqlx::query_as::<_, Tick>(
            "SELECT event_key,source,price,observed_at FROM market_price_ticks WHERE symbol=? AND source=? AND observed_at<=? AND observed_at>=? AND price>0 ORDER BY observed_at DESC,event_key DESC LIMIT 1",
        ).bind(&normalized).bind(source).bind(now.naive_utc()).bind((now-TimeDelta::seconds(i64::from(stale_after_seconds))).naive_utc()).fetch_optional(pool).await? {
            candidates.push(FollowObservation {
                event_key: tick.event_key, source: tick.source, symbol: normalized.clone(),
                price: tick.price, observed_at: tick.observed_at,
            });
        }
    }
    let selection = select_follow_reference(
        &symbol,
        &candidates,
        preferred_source,
        now,
        stale_after_seconds,
    );
    let Some(tick) = selection.observation else {
        return Ok(FollowReferenceRead {
            symbol,
            observation: None,
            reason: selection.reason,
        });
    };
    // SQL 检查全部同源同刻证据（包括非正价），等价于预览保留全部 ties 后调用共同选择器。
    let conflict: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM market_price_ticks WHERE symbol=? AND source=? AND observed_at=? AND price<>?)",
    ).bind(&normalized).bind(&tick.source).bind(tick.observed_at.naive_utc()).bind(&tick.price)
        .fetch_one(pool).await?;
    if conflict {
        return Ok(unavailable("参考行情同一时刻价格冲突，正在独立震荡"));
    }
    Ok(FollowReferenceRead {
        symbol,
        reason: None,
        observation: Some(tick),
    })
}
