//! 常驻行情配置纯校验；不得静默截断起始价或为无历史交易对编造启动价格。

use crate::error::{AppError, AppResult};
use bigdecimal::BigDecimal;

/// 起始价必须为 DECIMAL(38,18) 内的正数并精确满足交易对价格精度；空值留给历史价格启动规则。
pub(crate) fn validate_default_market_initial_price(
    price: Option<&BigDecimal>,
    price_precision: i32,
) -> AppResult<()> {
    if !(0..=18).contains(&price_precision) {
        return Err(AppError::Validation(
            "交易对价格精度必须在 0～18 之间".to_owned(),
        ));
    }
    if let Some(price) = price {
        let upper = BigDecimal::from(10u64).powi(20);
        if price <= &BigDecimal::from(0) || price >= &upper {
            return Err(AppError::Validation(
                "默认行情起始价必须为有效的正数且小于 10 的 20 次方".to_owned(),
            ));
        }
        if price.with_scale(i64::from(price_precision)) != *price {
            return Err(AppError::Validation(
                "默认行情起始价超出交易对价格精度".to_owned(),
            ));
        }
    }
    Ok(())
}

/// 乐观并发校验要求调用方刚刚锁后读取，首次创建使用 0；过期编辑返回冲突而不是覆盖他人配置。
pub(crate) fn next_default_market_version(current: u32, expected: u32) -> AppResult<u32> {
    if current != expected {
        return Err(AppError::Conflict(
            "默认行情配置已更新，请刷新后重试".to_owned(),
        ));
    }
    current
        .checked_add(1)
        .ok_or_else(|| AppError::Conflict("默认行情配置版本已达到上限".to_owned()))
}

/// 无 I/O 的历史预览上下文：起价来自本币已提交行情，外部证据仅用于涨跌映射，不复制参考绝对价。
pub(crate) struct DefaultMarketFollowPreviewInput<'a> {
    pub(crate) symbol: &'a str,
    pub(crate) seed: &'a str,
    pub(crate) version: u32,
    pub(crate) price_precision: u32,
    pub(crate) qty_precision: u32,
    pub(crate) start: chrono::DateTime<chrono::Utc>,
    pub(crate) start_price: &'a BigDecimal,
    pub(crate) parameters: &'a crate::modules::market::synthetic_default::DefaultMarketParameters,
    pub(crate) reference_symbol: &'a str,
}

/// 将六十个完整历史分钟的真实采样按秒推进同一跟随状态机，缺失时自动使用连续独立震荡路径。
/// 只保留各源最新时间戳的全部冲突候选，限制选择开销；无写入、无预测，样本量与降级范围明确返回。
pub(crate) fn replay_default_market_follow_preview(
    input: &DefaultMarketFollowPreviewInput<'_>,
    observations: &[crate::modules::market::synthetic_follow::FollowObservation],
    truncated: bool,
) -> AppResult<(
    Vec<crate::modules::admin::presentation::MarketStrategyRecoverySampleResponse>,
    crate::modules::admin::presentation::DefaultMarketFollowPreviewResponse,
)> {
    use crate::modules::{
        admin::presentation::{
            DefaultMarketFollowPreviewResponse, MarketStrategyRecoverySampleResponse,
        },
        market::{
            synthetic_default::generate_default_1m,
            synthetic_follow::{
                FollowFrameInput, FollowMinuteState, advance_follow_frame, select_follow_reference,
            },
        },
    };
    use chrono::Duration;
    use std::collections::HashSet;

    let follow = input
        .parameters
        .follow
        .as_ref()
        .ok_or_else(|| AppError::Validation("跟随预览缺少参考配置".to_owned()))?;
    let mut price = input.start_price.clone();
    let mut samples = Vec::with_capacity(60);
    let mut previous: Option<FollowMinuteState> = None;
    let mut candidates = Vec::new();
    let mut cursor = 0;
    let mut used = HashSet::new();
    let mut fallback_seconds = 0;
    for minute in 0..60 {
        let fallback = generate_default_1m(
            input.symbol,
            input.seed,
            input.version,
            input.price_precision,
            input.qty_precision,
            input.start + Duration::minutes(minute),
            &price,
            input.start_price,
            input.parameters,
        )?;
        for second in 0..60 {
            let now = fallback.open_time + Duration::seconds(second);
            while !truncated
                && cursor < observations.len()
                && observations[cursor].observed_at <= now
            {
                let observation = &observations[cursor];
                candidates.retain(
                    |old: &crate::modules::market::synthetic_follow::FollowObservation| {
                        old.source != observation.source
                            || old.observed_at == observation.observed_at
                    },
                );
                candidates.push(observation.clone());
                cursor += 1;
            }
            let selected = select_follow_reference(
                input.reference_symbol,
                &candidates,
                previous
                    .as_ref()
                    .and_then(|state| state.last_reference.as_ref())
                    .map(|r| r.source.as_str()),
                now,
                follow.stale_after_seconds,
            );
            if let Some(observation) = &selected.observation {
                used.insert(observation.event_key.clone());
            } else {
                fallback_seconds += 1;
            }
            let frame = advance_follow_frame(
                &FollowFrameInput {
                    symbol: input.symbol,
                    parameters: input.parameters,
                    price_precision: input.price_precision,
                    qty_precision: input.qty_precision,
                    fallback: &fallback,
                    now,
                    reference_symbol: input.reference_symbol,
                    unavailable_reason: selected.reason.as_deref(),
                },
                previous.as_ref(),
                selected.observation,
            )?;
            previous = Some(frame);
        }
        let frame = &previous.as_ref().expect("minute has sixty frames").frame;
        price = frame.close().clone();
        samples.push(MarketStrategyRecoverySampleResponse {
            open_time: frame.open_time(),
            open: frame.open().clone(),
            high: frame.high().clone(),
            low: frame.low().clone(),
            close: frame.close().clone(),
            volume: frame.volume().clone(),
        });
    }
    let warning = if truncated {
        "参考采样超过 20000 条预览上限，未使用截断数据；当前显示独立震荡样本，不是历史参考完整回放或未来预测。".to_owned()
    } else if used.is_empty() {
        "最近 60 个完整分钟没有可用参考证据，当前显示独立震荡样本，不是参考行情预测。".to_owned()
    } else {
        format!(
            "以当前本币起价映射最近 60 个完整分钟的已归档参考采样并按秒回放，不是实时逐笔记录或未来预测；其中 {fallback_seconds} 秒因参考缺失、过期或冲突使用独立震荡。"
        )
    };
    Ok((
        samples,
        DefaultMarketFollowPreviewResponse {
            kind: if used.is_empty() {
                "independent_fallback"
            } else {
                "historical_replay"
            },
            reference_pair_id: follow.reference_pair_id,
            reference_symbol: input.reference_symbol.to_owned(),
            range_start: input.start,
            range_end: input.start + Duration::minutes(60),
            reference_sample_count: used.len(),
            warning: Some(warning),
        },
    ))
}
