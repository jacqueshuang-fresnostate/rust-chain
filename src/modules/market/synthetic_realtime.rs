//! 策略实时展示派生：只从确定性 1m 构造盘口、逐笔和形成中高周期，不创建真实订单或补写历史。

use bigdecimal::{BigDecimal, RoundingMode};
use chrono::{DateTime, TimeDelta, Utc};
use sha2::{Digest, Sha256};

use super::{
    MarketDataProvider, MarketDepthLevel, MarketDepthSnapshot, MarketKlineSnapshot,
    MarketKlineValues, MarketTradeSide, MarketTradeTick, SyntheticCandle, SyntheticKlineInterval,
    SyntheticMarketConfig,
};
use crate::error::{AppError, AppResult};

/// 以当前权威 1m 收盘价为中心生成最多 20 档非交叉盘口，以及当前秒的模拟成交。
/// 价格精度沿用策略，挂单量按交易对数量精度取整；逐笔量取分钟累计量之差以避免每秒重复整分钟成交量。
/// 逐笔 ID 绑定策略、版本和 UTC 秒，重启重放稳定；零成交量不制造成交，所有结果只供行情展示。
pub fn build_synthetic_market_details(
    strategy_id: u64,
    config: &SyntheticMarketConfig,
    qty_precision: u32,
    current: &MarketKlineSnapshot,
) -> AppResult<(MarketDepthSnapshot, Option<MarketTradeTick>)> {
    let closed = config
        .generate_1m(current.open_time())
        .map_err(|error| AppError::Validation(error.to_string()))?;
    build_market_details_from_candle(
        &format!("strategy:{strategy_id}:v{}", config.version),
        &config.seed,
        config.version,
        config.price_precision,
        qty_precision,
        20,
        &closed,
        current,
        false,
    )
}

/// 从已经固定的完整分钟蜡烛派生展示盘口和逐笔，不依赖人工排期配置，也不生成或保存新的根蜡烛。
/// identifier 是来源特有的逐笔 ID 前缀；沿用原 seed/version/symbol/秒摘要，旧策略输出保持一致。
/// 输入须是该分钟该秒的确定性形成中值，累计量与逐笔差量均遵守数量精度；拒绝错槽、错来源及非一致快照。
#[allow(clippy::too_many_arguments)]
pub fn build_synthetic_market_details_from_candle(
    identifier: &str,
    seed: &str,
    version: u32,
    price_precision: u32,
    qty_precision: u32,
    depth_levels: u32,
    closed: &SyntheticCandle,
    current: &MarketKlineSnapshot,
) -> AppResult<(MarketDepthSnapshot, Option<MarketTradeTick>)> {
    build_market_details_from_candle(
        identifier,
        seed,
        version,
        price_precision,
        qty_precision,
        depth_levels,
        closed,
        current,
        true,
    )
}

/// 默认路径先量化前后累计量再求差，旧人工策略路径保持十八位累计量以兼容既有逐笔身份和结果。
#[allow(clippy::too_many_arguments)]
fn build_market_details_from_candle(
    identifier: &str,
    seed: &str,
    version: u32,
    price_precision: u32,
    qty_precision: u32,
    depth_levels: u32,
    closed: &SyntheticCandle,
    current: &MarketKlineSnapshot,
    quantize_volume: bool,
) -> AppResult<(MarketDepthSnapshot, Option<MarketTradeTick>)> {
    if price_precision > 18
        || qty_precision > 18
        || !(1..=20).contains(&depth_levels)
        || identifier.trim().is_empty()
        || seed.trim().is_empty()
        || current.interval() != "1m"
        || current.provider() != MarketDataProvider::Strategy
        || closed.open_time != current.open_time()
        || closed.values.open <= 0
        || closed.values.close <= 0
        || closed.values.low <= 0
        || closed.values.volume < 0
        || closed.values.low > closed.values.open.clone().min(closed.values.close.clone())
        || closed.values.high < closed.values.open.clone().max(closed.values.close.clone())
    {
        return Err(AppError::Validation(
            "invalid synthetic market detail input".into(),
        ));
    }
    let second = DateTime::from_timestamp(current.observed_at().timestamp(), 0)
        .ok_or_else(|| AppError::Validation("invalid synthetic trade time".into()))?;
    let forming_values = |at| {
        let mut values = forming_1m_values(&closed.values, closed.open_time, at, price_precision)?;
        if quantize_volume {
            values.volume = values
                .volume
                .with_scale_round(i64::from(qty_precision), RoundingMode::HalfUp);
        }
        Ok::<_, AppError>(values)
    };
    let expected = forming_values(second)?;
    if current.open() != &expected.open
        || current.high() != &expected.high
        || current.low() != &expected.low
        || current.close() != &expected.close
        || current.volume() != &expected.volume
    {
        return Err(AppError::Validation(
            "synthetic market details require the matching forming candle".into(),
        ));
    }
    let elapsed = (second - current.open_time()).num_seconds();
    let previous = if elapsed == 0 {
        MarketKlineValues {
            open: closed.values.open.clone(),
            high: closed.values.open.clone(),
            low: closed.values.open.clone(),
            close: closed.values.open.clone(),
            volume: BigDecimal::from(0),
        }
    } else {
        forming_values(second - TimeDelta::seconds(1))?
    };
    let quantity = current.volume() - &previous.volume;
    build_observed_market_details(
        identifier,
        seed,
        version,
        price_precision,
        qty_precision,
        depth_levels,
        &closed.values.volume,
        current,
        &previous.close,
        quantity,
    )
}

/// 从已持久化实际观察帧派生模拟盘口与本秒逐笔；跟随模式只换价格路径，不复制参考量、不创建真实订单。
/// 调用方传入分钟总模拟量、已量化差量与上一价；验证 OHLC 包络、数量和精度后复用旧摘要及盘口算法。
#[allow(clippy::too_many_arguments)]
pub fn build_observed_market_details(
    identifier: &str,
    seed: &str,
    version: u32,
    price_precision: u32,
    qty_precision: u32,
    depth_levels: u32,
    minute_volume: &BigDecimal,
    current: &MarketKlineSnapshot,
    previous_price: &BigDecimal,
    quantity: BigDecimal,
) -> AppResult<(MarketDepthSnapshot, Option<MarketTradeTick>)> {
    if price_precision > 18
        || qty_precision > 18
        || !(1..=20).contains(&depth_levels)
        || identifier.trim().is_empty()
        || seed.trim().is_empty()
        || version == 0
        || current.provider() != MarketDataProvider::Strategy
        || current.interval() != "1m"
        || current.open() <= &BigDecimal::from(0)
        || current.low() <= &BigDecimal::from(0)
        || current.low() > &current.open().clone().min(current.close().clone())
        || current.high() < &current.open().clone().max(current.close().clone())
        || quantity < 0
        || minute_volume < &BigDecimal::from(0)
        || &quantity > minute_volume
        || current.volume() > minute_volume
        || previous_price <= &BigDecimal::from(0)
    {
        return Err(AppError::Validation(
            "invalid observed synthetic market detail input".into(),
        ));
    }
    let second = DateTime::from_timestamp(current.observed_at().timestamp(), 0)
        .ok_or_else(|| AppError::Validation("invalid synthetic trade time".into()))?;
    let price_unit = BigDecimal::new(1.into(), i64::from(price_precision));
    let quantity_unit = BigDecimal::new(1.into(), i64::from(qty_precision));
    let step = (current.close() / BigDecimal::from(10_000))
        .with_scale_round(i64::from(price_precision), RoundingMode::HalfUp)
        .max(price_unit);
    let base_quantity = (minute_volume / BigDecimal::from(60)).max(quantity_unit.clone());
    let mut bids = Vec::with_capacity(depth_levels as usize);
    let mut asks = Vec::with_capacity(depth_levels as usize);
    let digest = Sha256::digest(format!(
        "{}:{}:{}:{}",
        seed,
        version,
        current.symbol(),
        second.timestamp()
    ));
    for level in 1..=depth_levels {
        let offset = &step * BigDecimal::from(level);
        let amount = |salt: usize| {
            (&base_quantity
                * BigDecimal::from(1 + u32::from(digest[(level as usize + salt) % 32]) % 20))
            .with_scale_round(i64::from(qty_precision), RoundingMode::HalfUp)
            .max(quantity_unit.clone())
        };
        let bid = current.close() - &offset;
        if bid > 0 {
            bids.push(MarketDepthLevel::new(bid, amount(0)));
        }
        asks.push(MarketDepthLevel::new(current.close() + offset, amount(11)));
    }
    let depth = MarketDepthSnapshot::new(
        MarketDataProvider::Strategy,
        current.symbol(),
        bids,
        asks,
        current.observed_at(),
    )
    .map_err(|error| AppError::Validation(error.to_string()))?;
    let trade = if quantity > 0 {
        let side = if current.close() > previous_price
            || (current.close() == previous_price && digest[0] % 2 == 0)
        {
            MarketTradeSide::Buy
        } else {
            MarketTradeSide::Sell
        };
        Some(
            MarketTradeTick::new(
                MarketDataProvider::Strategy,
                current.symbol(),
                format!("{identifier}:{}", second.timestamp()),
                side,
                current.close().clone(),
                quantity,
                second,
            )
            .map_err(|error| AppError::Validation(error.to_string()))?,
        )
    } else {
        None
    };
    Ok((depth, trade))
}

/// 聚合当前 UTC 高周期内实际存在的 1m 加当前形成中 1m；不生成缺失根，不使用未来根或其他周期。
/// 返回值仅用于当前 Redis/WS 快照，绝不当作已完成历史入 Mongo；完整闭合仍走严格权威窗口校验。
/// 输入根先按槽去重排序，当前根覆盖同槽旧值，因而多次观测不会重复累计成交量。
pub fn build_forming_aggregate(
    current: &MarketKlineSnapshot,
    interval: SyntheticKlineInterval,
    history: &[SyntheticCandle],
) -> AppResult<MarketKlineSnapshot> {
    let seconds = interval.minute_count() as i64 * 60;
    let start = DateTime::from_timestamp(
        current.open_time().timestamp().div_euclid(seconds) * seconds,
        0,
    )
    .ok_or_else(|| AppError::Validation("invalid aggregate boundary".into()))?;
    let mut roots = std::collections::BTreeMap::new();
    for candle in history {
        if candle.open_time >= start && candle.open_time < current.open_time() {
            roots.insert(candle.open_time, candle.values.clone());
        }
    }
    roots.insert(
        current.open_time(),
        MarketKlineValues {
            open: current.open().clone(),
            high: current.high().clone(),
            low: current.low().clone(),
            close: current.close().clone(),
            volume: current.volume().clone(),
        },
    );
    let mut values = roots
        .first_key_value()
        .expect("current root exists")
        .1
        .clone();
    for root in roots.values().skip(1) {
        values.high = values.high.max(root.high.clone());
        values.low = values.low.min(root.low.clone());
        values.close = root.close.clone();
        values.volume += &root.volume;
    }
    MarketKlineSnapshot::new(
        MarketDataProvider::Strategy,
        current.symbol(),
        interval.as_str(),
        start,
        values,
        current.observed_at(),
    )
    .map_err(|error| AppError::Validation(error.to_string()))
}

/// 把确定性整分钟 OHLCV 映射为当前秒的形成中快照：价格依次经过两个确定性极值并回到最终 close，
/// 成交量按已观察秒数累计；第 59 秒直接返回整分钟值，避免实时闭合与手动补偿产生尾差。
/// 观察时刻必须落在该分钟之内，否则返回校验错误；路径分三段各 20 秒，先走首个极值再走另一极值，最后回到收盘价。
/// 收涨时先探低后探高，收跌时相反；高低价只在对应极值被越过后才纳入，成交量按已观察秒数占比线性摊分。
pub(crate) fn forming_1m_values(
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
