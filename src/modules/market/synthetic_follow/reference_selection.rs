//! 运行端与历史预览共用参考证据选择；只认已观察的外部报价，稳定供应商优先且冲突失败关闭。

use chrono::{DateTime, TimeDelta, Utc};

use super::FollowObservation;
use crate::modules::market::sanitize_symbol;

/// 已选择的外部采样或明确降级原因；缺失不是系统错误，不触发网络重试或历史补造。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FollowReferenceSelection {
    pub observation: Option<FollowObservation>,
    pub reason: Option<String>,
}

/// 从各供应商的候选历史样本选择当前证据；调用方须保留最新时间戳的全部同源记录以检测冲突。
/// 只用同符号、正价、非未来且未过期样本；已有健康供应商优先，切源时采用确定性排序且不回退掩盖冲突。
pub fn select_follow_reference(
    reference_symbol: &str,
    candidates: &[FollowObservation],
    preferred_source: Option<&str>,
    now: DateTime<Utc>,
    stale_after_seconds: u32,
) -> FollowReferenceSelection {
    let symbol = sanitize_symbol(reference_symbol);
    let eligible = |tick: &&FollowObservation| {
        matches!(tick.source.as_str(), "bitget" | "htx" | "coinbase")
            && !tick.event_key.trim().is_empty()
            && !symbol.is_empty()
            && sanitize_symbol(&tick.symbol) == symbol
            && tick.price > 0
            && tick.observed_at <= now
            && now - tick.observed_at <= TimeDelta::seconds(i64::from(stale_after_seconds))
    };
    let latest = |source: Option<&str>| {
        candidates
            .iter()
            .filter(eligible)
            .filter(|tick| source.is_none_or(|source| tick.source == source))
            .max_by(|a, b| {
                a.observed_at
                    .cmp(&b.observed_at)
                    .then_with(|| a.source.cmp(&b.source))
                    .then_with(|| a.event_key.cmp(&b.event_key))
            })
    };
    let selected = preferred_source
        .and_then(|source| latest(Some(source)))
        .or_else(|| latest(None));
    let Some(selected) = selected else {
        return unavailable("参考行情缺失或已过期，正在独立震荡");
    };
    if candidates.iter().any(|tick| {
        tick.source == selected.source
            && sanitize_symbol(&tick.symbol) == symbol
            && tick.observed_at == selected.observed_at
            && tick.price != selected.price
    }) {
        return unavailable("参考行情同一时刻价格冲突，正在独立震荡");
    }
    FollowReferenceSelection {
        observation: Some(selected.clone()),
        reason: None,
    }
}

fn unavailable(reason: &str) -> FollowReferenceSelection {
    FollowReferenceSelection {
        observation: None,
        reason: Some(reason.to_owned()),
    }
}
