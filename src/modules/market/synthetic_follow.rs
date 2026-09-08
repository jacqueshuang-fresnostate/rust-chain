//! 默认行情实时涨跌映射：冻结参考证据及本秒帧，恢复跟随时重新锚定，不预测参考未来价格。

mod reference_selection;
pub use reference_selection::{FollowReferenceSelection, select_follow_reference};

use bigdecimal::{BigDecimal, RoundingMode};
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use super::synthetic_default::{DefaultMarketParameters, forming_default_1m_values};
use super::{MarketDataProvider, MarketKlineSnapshot, MarketKlineValues, SyntheticCandle};
use crate::error::{AppError, AppResult};

/// 已归档外部采样证据；时间表示现有采集链的观察时间，不承诺交易所成交时间或终局收线。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FollowObservation {
    pub event_key: String,
    pub source: String,
    pub symbol: String,
    pub price: BigDecimal,
    pub observed_at: DateTime<Utc>,
}

/// 后台读取时的有效模式；降级是正常生成状态而非整体运行错误，独立参数仍受原边界约束。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FollowRuntimeStatus {
    pub mode: String,
    pub reference_pair_id: u64,
    pub reference_symbol: String,
    pub reference_price: Option<BigDecimal>,
    pub reference_observed_at: Option<i64>,
    pub fallback_reason: Option<String>,
    pub switched_at: i64,
}

/// 本分钟已确定帧和锚点；先持久化后发布，同秒重试不得重新读取参考价格改变既定帧。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FollowMinuteState {
    pub status: FollowRuntimeStatus,
    pub frame: MarketKlineSnapshot,
    pub last_reference: Option<FollowObservation>,
    pub target_anchor: BigDecimal,
    pub reference_anchor: BigDecimal,
    pub previous_price: BigDecimal,
}

/// 一次纯价格更新所需的本币参数与降级路径；参考数据可缺失，但不能省略显式初始价格。
pub struct FollowFrameInput<'a> {
    pub symbol: &'a str,
    pub parameters: &'a DefaultMarketParameters,
    pub price_precision: u32,
    pub qty_precision: u32,
    pub fallback: &'a SyntheticCandle,
    pub now: DateTime<Utc>,
    pub reference_symbol: &'a str,
    pub unavailable_reason: Option<&'a str>,
}

/// 从参考相对涨跌推进一个 UTC 秒，倍率只映射收益，不复制绝对价格或参考成交量。
/// 首次、过期恢复、供应商更换和长间隔都以本币上一价重锚；同分钟累计实际高低价并限制分钟涨跌。
/// 调用者负责验证引用 external 交易对、冻结结果与共享锁；此函数无 I/O、不补历史、不改旧状态。
pub fn advance_follow_frame(
    input: &FollowFrameInput<'_>,
    previous: Option<&FollowMinuteState>,
    observation: Option<FollowObservation>,
) -> AppResult<FollowMinuteState> {
    input
        .parameters
        .validate(input.price_precision, input.qty_precision)?;
    let config = input
        .parameters
        .follow
        .as_ref()
        .ok_or_else(|| invalid("跟随行情缺少参考配置"))?;
    let now = DateTime::from_timestamp(input.now.timestamp(), 0)
        .ok_or_else(|| invalid("跟随观察时间无效"))?;
    let previous =
        previous.filter(|state| state.status.reference_pair_id == config.reference_pair_id);
    if let Some(previous) = previous {
        if previous.frame.observed_at() > now {
            return Err(invalid("跟随行情观察时间倒退"));
        }
        if previous.frame.open_time() == input.fallback.open_time
            && previous.frame.observed_at() == now
        {
            return Ok(previous.clone());
        }
    }
    let base = forming_default_1m_values(
        &input.fallback.values,
        input.fallback.open_time,
        now,
        input.price_precision,
        input.qty_precision,
    )?;
    let same_minute = previous.filter(|p| p.frame.open_time() == input.fallback.open_time);
    let previous_price = same_minute
        .map(|p| p.frame.close().clone())
        .unwrap_or_else(|| base.open.clone());
    let valid = observation.filter(|r| {
        matches!(r.source.as_str(), "bitget" | "htx" | "coinbase")
            && !r.event_key.trim().is_empty()
            && r.price > 0
            && super::sanitize_symbol(&r.symbol) == super::sanitize_symbol(input.reference_symbol)
            && r.observed_at <= now
            && now - r.observed_at <= TimeDelta::seconds(i64::from(config.stale_after_seconds))
            && previous
                .and_then(|p| p.last_reference.as_ref())
                .is_none_or(|old| {
                    old.source != r.source
                        || (r.observed_at > old.observed_at
                            || (r.observed_at == old.observed_at && r.price == old.price))
                })
    });
    let is_following = valid.is_some();
    let mode = if is_following {
        "following"
    } else {
        "fallback"
    };
    let recent_previous = previous.filter(|p| {
        now - p.frame.observed_at() <= TimeDelta::seconds(i64::from(config.stale_after_seconds))
    });
    let continuous_reference = valid.as_ref().zip(recent_previous).is_some_and(|(r, p)| {
        p.status.mode == "following"
            && p.last_reference.as_ref().is_some_and(|old| {
                old.source == r.source
                    && old.symbol == r.symbol
                    && old.observed_at <= r.observed_at
                    && r.observed_at - old.observed_at
                        <= TimeDelta::seconds(i64::from(config.stale_after_seconds))
            })
    });
    let (target_anchor, reference_anchor, price) = if let Some(reference) = &valid {
        let anchors = if continuous_reference {
            if let Some(p) = same_minute {
                (p.target_anchor.clone(), p.reference_anchor.clone())
            } else {
                let p = recent_previous.expect("continuous reference requires previous state");
                (
                    base.open.clone(),
                    p.last_reference
                        .as_ref()
                        .expect("continuous reference evidence")
                        .price
                        .clone(),
                )
            }
        } else {
            (previous_price.clone(), reference.price.clone())
        };
        if anchors.1 <= 0 {
            return Err(invalid("已保存的参考锚定价无效"));
        }
        let ratio = (&reference.price / &anchors.1 - BigDecimal::from(1)) * &config.multiplier;
        let price = &anchors.0 * (BigDecimal::from(1) + ratio);
        (anchors.0, anchors.1, price)
    } else {
        let anchors = if let Some(p) = same_minute.filter(|p| p.status.mode == "fallback") {
            (p.target_anchor.clone(), p.reference_anchor.clone())
        } else if previous.is_some_and(|p| p.status.mode == "fallback") && same_minute.is_none() {
            (base.open.clone(), base.open.clone())
        } else {
            (previous_price.clone(), base.close.clone())
        };
        if anchors.1 <= 0 {
            return Err(invalid("已保存的独立震荡锚定价无效"));
        }
        let price = &anchors.0 * &base.close / &anchors.1;
        (anchors.0, anchors.1, price)
    };
    let tick = BigDecimal::new(1.into(), i64::from(input.price_precision));
    let min = (&base.open * (BigDecimal::from(1) - &config.max_move_ratio))
        .max(
            input
                .parameters
                .price_min
                .clone()
                .unwrap_or_else(|| tick.clone()),
        )
        .with_scale_round(i64::from(input.price_precision), RoundingMode::Ceiling);
    let max = (&base.open * (BigDecimal::from(1) + &config.max_move_ratio))
        .min(
            input
                .parameters
                .price_max
                .clone()
                .unwrap_or_else(|| BigDecimal::new(1.into(), -20) - &tick),
        )
        .with_scale_round(i64::from(input.price_precision), RoundingMode::Floor);
    let close = price
        .with_scale_round(i64::from(input.price_precision), RoundingMode::HalfUp)
        .max(min)
        .min(max);
    let high = same_minute
        .map_or_else(|| base.open.clone(), |p| p.frame.high().clone())
        .max(close.clone());
    let low = same_minute
        .map_or_else(|| base.open.clone(), |p| p.frame.low().clone())
        .min(close.clone());
    let switched = previous
        .filter(|p| p.status.mode == mode)
        .map_or(now.timestamp_millis(), |p| p.status.switched_at);
    let status = FollowRuntimeStatus {
        mode: mode.into(),
        reference_pair_id: config.reference_pair_id,
        reference_symbol: input.reference_symbol.into(),
        reference_price: valid.as_ref().map(|r| r.price.clone()),
        reference_observed_at: valid.as_ref().map(|r| r.observed_at.timestamp_millis()),
        fallback_reason: (!is_following).then(|| {
            input
                .unavailable_reason
                .unwrap_or("参考行情缺失、过期或无效，正在独立震荡")
                .to_owned()
        }),
        switched_at: switched,
    };
    Ok(FollowMinuteState {
        status,
        // 降级不能丢失参考水位，否则迟到证据会在恢复后重复累计已消费的涨跌。
        last_reference: valid.or_else(|| previous.and_then(|p| p.last_reference.clone())),
        target_anchor,
        reference_anchor,
        previous_price,
        frame: MarketKlineSnapshot::new(
            MarketDataProvider::Strategy,
            input.symbol,
            "1m",
            input.fallback.open_time,
            MarketKlineValues {
                open: base.open,
                high,
                low,
                close,
                volume: base.volume,
            },
            now,
        )
        .map_err(|e| invalid(&e.to_string()))?,
    })
}

fn invalid(message: &str) -> AppError {
    AppError::Validation(message.into())
}
