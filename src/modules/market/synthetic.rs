//! 新币策略使用的纯 Rust 确定性行情生成器。

use std::str::FromStr;

use bigdecimal::{BigDecimal, RoundingMode};
use chrono::{DateTime, Duration, Timelike, Utc};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::domain::MarketKlineValues;

const ONE_MILLION: u64 = 1_000_000;

/// 只允许由权威 1m 窗口派生的公开周期。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntheticKlineInterval {
    FiveMinutes,
    FifteenMinutes,
    OneHour,
    FourHours,
    OneDay,
}

impl SyntheticKlineInterval {
    /// 返回对外 K 线合同使用的稳定周期代码。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FiveMinutes => "5m",
            Self::FifteenMinutes => "15m",
            Self::OneHour => "1h",
            Self::FourHours => "4h",
            Self::OneDay => "1d",
        }
    }

    /// 返回一个完整聚合窗口所需的 1m 根数。
    pub fn minute_count(self) -> usize {
        match self {
            Self::FiveMinutes => 5,
            Self::FifteenMinutes => 15,
            Self::OneHour => 60,
            Self::FourHours => 240,
            Self::OneDay => 1_440,
        }
    }
}

/// 目标值的计算基准：绝对价格、相对策略起点或相对前一节点。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntheticTargetType {
    AbsolutePrice,
    PercentFromStart,
    PercentFromPrevious,
}

/// 节点执行约束：hard 精确命中，soft/range 在容差带内确定性取值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntheticExecutionMode {
    Hard,
    Soft,
    Range,
}

/// 一个有序策略节点；该类型不读写数据库，由 [`SyntheticMarketConfig::new`] 统一校验。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticMarketNode {
    pub target_time: DateTime<Utc>,
    pub target_type: SyntheticTargetType,
    pub target_value: BigDecimal,
    pub execution_mode: SyntheticExecutionMode,
    pub tolerance: BigDecimal,
    pub volatility: BigDecimal,
    pub volume_min: Option<BigDecimal>,
    pub volume_max: Option<BigDecimal>,
}

/// 确定性生成所需的完整版本快照；构造成功后可在实时、预览和补偿路径重复使用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticMarketConfig {
    pub symbol: String,
    pub seed: String,
    pub version: u32,
    pub price_precision: u32,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub start_price: BigDecimal,
    pub target_price: BigDecimal,
    pub volatility: BigDecimal,
    pub volume_min: BigDecimal,
    pub volume_max: BigDecimal,
    pub nodes: Vec<SyntheticMarketNode>,
}

/// 生成失败只表示版本快照或时间槽不合法，函数本身无 I/O 和部分写入。
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SyntheticMarketError {
    #[error("synthetic symbol and seed must not be blank")]
    BlankIdentity,
    #[error("synthetic strategy times must align to UTC minutes")]
    MinuteAlignment,
    #[error("synthetic strategy end_time must be after start_time")]
    InvalidTimeRange,
    #[error("synthetic node times must be strictly increasing and inside strategy range")]
    InvalidNodeOrder,
    #[error("synthetic prices must be positive")]
    NonPositivePrice,
    #[error("synthetic volatility, tolerance, and volume must be non-negative")]
    NegativeParameter,
    #[error("synthetic volume range is invalid")]
    InvalidVolumeRange,
    #[error("synthetic open_time is outside the strategy range or not minute aligned")]
    InvalidOpenTime,
    #[error("synthetic 1m candles must be ordered, continuous, and minute aligned")]
    NonContinuousCandles,
    #[error("synthetic aggregate input must contain complete UTC-aligned windows")]
    IncompleteAggregateWindow,
    #[error("synthetic candle violates OHLCV invariants")]
    InvalidCandle,
    #[error("synthetic decimal calculation failed")]
    DecimalCalculation,
}

impl SyntheticMarketConfig {
    /// 校验分钟对齐、正数价格、非负参数、成交量区间及严格递增节点。
    /// 本函数不解析传输 DTO，也不修正非法时间，以保证版本快照可审计。
    pub fn new(mut config: Self) -> Result<Self, SyntheticMarketError> {
        config.symbol = config.symbol.trim().to_ascii_uppercase();
        if config.symbol.is_empty() || config.seed.trim().is_empty() {
            return Err(SyntheticMarketError::BlankIdentity);
        }
        if !is_minute_aligned(config.start_time) || !is_minute_aligned(config.end_time) {
            return Err(SyntheticMarketError::MinuteAlignment);
        }
        if config.end_time <= config.start_time {
            return Err(SyntheticMarketError::InvalidTimeRange);
        }
        if config.start_price <= 0 || config.target_price <= 0 {
            return Err(SyntheticMarketError::NonPositivePrice);
        }
        validate_non_negative(&config.volatility)?;
        validate_volume_range(&config.volume_min, &config.volume_max)?;

        let mut previous_time = None;
        for node in &config.nodes {
            if !is_minute_aligned(node.target_time)
                || node.target_time <= config.start_time
                || node.target_time >= config.end_time
                || previous_time.is_some_and(|previous| node.target_time <= previous)
            {
                return Err(SyntheticMarketError::InvalidNodeOrder);
            }
            if node.target_type == SyntheticTargetType::AbsolutePrice && node.target_value <= 0 {
                return Err(SyntheticMarketError::NonPositivePrice);
            }
            validate_non_negative(&node.tolerance)?;
            validate_non_negative(&node.volatility)?;
            match (&node.volume_min, &node.volume_max) {
                (None, None) => {}
                (Some(minimum), Some(maximum)) => validate_volume_range(minimum, maximum)?,
                _ => return Err(SyntheticMarketError::InvalidVolumeRange),
            }
            previous_time = Some(node.target_time);
        }
        config.resolve_anchors()?;
        Ok(config)
    }

    /// 使用 `seed + version + symbol + open_time` 独立派生每分钟随机性并生成权威 1m OHLCV。
    /// 调用不依赖前一次调用顺序，因而重启、重试和分批边界不会改变结果。
    pub fn generate_1m(
        &self,
        open_time: DateTime<Utc>,
    ) -> Result<SyntheticCandle, SyntheticMarketError> {
        if !is_minute_aligned(open_time)
            || open_time < self.start_time
            || open_time >= self.end_time
        {
            return Err(SyntheticMarketError::InvalidOpenTime);
        }
        let anchors = self.resolve_anchors()?;
        let open = self.price_at(&anchors, open_time)?;
        let close_time = open_time + Duration::minutes(1);
        let close = self.price_at(&anchors, close_time)?;
        let volatility = self.local_volatility(open_time);
        let upper = decimal_from_unit(slot_unit(self, open_time, b"wick-high"), 6)?;
        let lower = decimal_from_unit(slot_unit(self, open_time, b"wick-low"), 6)?;
        let body_high = open.clone().max(close.clone());
        let body_low = open.clone().min(close.clone());
        let wick_factor = decimal("0.75")?;
        let high = &body_high + (&body_high * &volatility * upper * &wick_factor);
        let low = (&body_low - (&body_low * &volatility * lower * wick_factor))
            .max(min_price(self.price_precision));
        let (volume_min, volume_max) = self.local_volume_range(open_time);
        let volume_unit = decimal_from_unit(slot_unit(self, open_time, b"volume"), 6)?;
        let volume = &volume_min + ((&volume_max - &volume_min) * volume_unit);

        let minimum_price = min_price(self.price_precision);
        let open = round_price(open, self.price_precision).max(minimum_price.clone());
        let close = round_price(close, self.price_precision).max(minimum_price.clone());
        let body_high = open.clone().max(close.clone());
        let body_low = open.clone().min(close.clone());
        let high = round_price(high, self.price_precision).max(body_high);
        let low = round_price(low, self.price_precision)
            .min(body_low)
            .max(minimum_price);

        Ok(SyntheticCandle {
            open_time,
            values: MarketKlineValues {
                open,
                high,
                low,
                close,
                volume: volume.with_scale_round(18, RoundingMode::HalfUp),
            },
        })
    }

    fn price_at(
        &self,
        anchors: &[ResolvedAnchor],
        time: DateTime<Utc>,
    ) -> Result<BigDecimal, SyntheticMarketError> {
        if let Some(anchor) = anchors.iter().find(|anchor| anchor.time == time) {
            return self.anchor_price(anchor, time);
        }
        let right_index = anchors.partition_point(|anchor| anchor.time < time);
        let right = &anchors[right_index.min(anchors.len() - 1)];
        let left = &anchors[right_index.saturating_sub(1)];
        let total = (right.time - left.time).num_minutes().max(1);
        let elapsed = (time - left.time).num_minutes().clamp(0, total);
        let fraction = BigDecimal::from(elapsed) / BigDecimal::from(total);
        let bridge = &left.price + ((&right.price - &left.price) * &fraction);
        let envelope = &fraction * (BigDecimal::from(1) - &fraction);
        let noise = decimal_from_signed_unit(slot_signed_unit(self, time, b"price"), 6)?;
        let mean_reversion = decimal("0.55")?;
        let adjustment = &bridge * self.local_volatility(time) * envelope * noise * mean_reversion;
        Ok((bridge + adjustment).max(min_price(self.price_precision)))
    }

    fn anchor_price(
        &self,
        anchor: &ResolvedAnchor,
        time: DateTime<Utc>,
    ) -> Result<BigDecimal, SyntheticMarketError> {
        match anchor.mode {
            SyntheticExecutionMode::Hard => Ok(anchor.price.clone()),
            SyntheticExecutionMode::Soft | SyntheticExecutionMode::Range => {
                let noise = decimal_from_signed_unit(slot_signed_unit(self, time, b"anchor"), 6)?;
                let tolerance_fraction = &anchor.tolerance / decimal("100")?;
                let mode_scale = match anchor.mode {
                    SyntheticExecutionMode::Soft => decimal("0.5")?,
                    SyntheticExecutionMode::Range => BigDecimal::from(1),
                    SyntheticExecutionMode::Hard => unreachable!("hard anchor returned above"),
                };
                let offset = &anchor.price * tolerance_fraction * noise * mode_scale;
                Ok((&anchor.price + offset).max(min_price(self.price_precision)))
            }
        }
    }

    fn resolve_anchors(&self) -> Result<Vec<ResolvedAnchor>, SyntheticMarketError> {
        let mut anchors = vec![ResolvedAnchor::hard(
            self.start_time,
            self.start_price.clone(),
        )];
        let mut previous = self.start_price.clone();
        for node in &self.nodes {
            let price = match node.target_type {
                SyntheticTargetType::AbsolutePrice => node.target_value.clone(),
                SyntheticTargetType::PercentFromStart => {
                    percent_price(&self.start_price, &node.target_value)?
                }
                SyntheticTargetType::PercentFromPrevious => {
                    percent_price(&previous, &node.target_value)?
                }
            };
            if price <= 0 {
                return Err(SyntheticMarketError::NonPositivePrice);
            }
            if node.target_time == self.start_time {
                anchors.clear();
            }
            anchors.push(ResolvedAnchor {
                time: node.target_time,
                price: round_price(price.clone(), self.price_precision),
                mode: node.execution_mode,
                tolerance: node.tolerance.clone(),
            });
            previous = price;
        }
        if anchors
            .last()
            .is_none_or(|anchor| anchor.time < self.end_time)
        {
            anchors.push(ResolvedAnchor::hard(
                self.end_time,
                round_price(self.target_price.clone(), self.price_precision),
            ));
        }
        Ok(anchors)
    }

    fn local_volatility(&self, time: DateTime<Utc>) -> BigDecimal {
        self.nodes
            .iter()
            .find(|node| node.target_time >= time)
            .map_or_else(|| self.volatility.clone(), |node| node.volatility.clone())
    }

    fn local_volume_range(&self, time: DateTime<Utc>) -> (BigDecimal, BigDecimal) {
        self.nodes
            .iter()
            .find(|node| node.target_time >= time)
            .and_then(|node| node.volume_min.clone().zip(node.volume_max.clone()))
            .unwrap_or_else(|| (self.volume_min.clone(), self.volume_max.clone()))
    }
}

/// 一根已通过价格精度与 OHLCV 不变量收敛的 K 线。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticCandle {
    pub open_time: DateTime<Utc>,
    pub values: MarketKlineValues,
}

/// 由连续 1m 窗口聚合的 K 线；周期代码与时间边界已通过验证。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticAggregateCandle {
    pub interval: SyntheticKlineInterval,
    pub open_time: DateTime<Utc>,
    pub values: MarketKlineValues,
}

/// 将恰好一个 UTC 对齐窗口的连续 1m K 线聚合成单根高周期 K 线。
/// 聚合只读内存；缺根、乱序、开收不连续或 OHLCV 非法时整体失败，不产生部分结果。
pub fn aggregate_1m_candles(
    candles: &[SyntheticCandle],
    interval: SyntheticKlineInterval,
) -> Result<SyntheticAggregateCandle, SyntheticMarketError> {
    let expected = interval.minute_count();
    if candles.len() != expected {
        return Err(SyntheticMarketError::IncompleteAggregateWindow);
    }
    let first = candles
        .first()
        .ok_or(SyntheticMarketError::IncompleteAggregateWindow)?;
    if !is_aggregate_boundary(first.open_time, interval) {
        return Err(SyntheticMarketError::IncompleteAggregateWindow);
    }

    let mut high = first.values.high.clone();
    let mut low = first.values.low.clone();
    let mut volume = BigDecimal::from(0);
    for (index, candle) in candles.iter().enumerate() {
        let expected_time = first.open_time + Duration::minutes(index as i64);
        if candle.open_time != expected_time || !is_valid_candle(&candle.values) {
            return Err(if candle.open_time == expected_time {
                SyntheticMarketError::InvalidCandle
            } else {
                SyntheticMarketError::NonContinuousCandles
            });
        }
        if index > 0 && candles[index - 1].values.close != candle.values.open {
            return Err(SyntheticMarketError::NonContinuousCandles);
        }
        high = high.max(candle.values.high.clone());
        low = low.min(candle.values.low.clone());
        volume += &candle.values.volume;
    }
    let last = candles
        .last()
        .ok_or(SyntheticMarketError::IncompleteAggregateWindow)?;
    Ok(SyntheticAggregateCandle {
        interval,
        open_time: first.open_time,
        values: MarketKlineValues {
            open: first.values.open.clone(),
            high,
            low,
            close: last.values.close.clone(),
            volume,
        },
    })
}

#[derive(Debug, Clone)]
struct ResolvedAnchor {
    time: DateTime<Utc>,
    price: BigDecimal,
    mode: SyntheticExecutionMode,
    tolerance: BigDecimal,
}

impl ResolvedAnchor {
    fn hard(time: DateTime<Utc>, price: BigDecimal) -> Self {
        Self {
            time,
            price,
            mode: SyntheticExecutionMode::Hard,
            tolerance: BigDecimal::from(0),
        }
    }
}

fn is_minute_aligned(time: DateTime<Utc>) -> bool {
    time.second() == 0 && time.nanosecond() == 0
}

fn is_aggregate_boundary(time: DateTime<Utc>, interval: SyntheticKlineInterval) -> bool {
    time.timestamp()
        .rem_euclid((interval.minute_count() as i64) * 60)
        == 0
}

fn is_valid_candle(values: &MarketKlineValues) -> bool {
    let zero = BigDecimal::from(0);
    values.open > zero
        && values.close > zero
        && values.high >= values.open
        && values.high >= values.close
        && values.low <= values.open
        && values.low <= values.close
        && values.low > zero
        && values.volume >= zero
}

fn validate_non_negative(value: &BigDecimal) -> Result<(), SyntheticMarketError> {
    if value < &BigDecimal::from(0) {
        Err(SyntheticMarketError::NegativeParameter)
    } else {
        Ok(())
    }
}

fn validate_volume_range(
    minimum: &BigDecimal,
    maximum: &BigDecimal,
) -> Result<(), SyntheticMarketError> {
    validate_non_negative(minimum)?;
    validate_non_negative(maximum)?;
    if maximum < minimum {
        Err(SyntheticMarketError::InvalidVolumeRange)
    } else {
        Ok(())
    }
}

fn percent_price(
    base: &BigDecimal,
    percentage: &BigDecimal,
) -> Result<BigDecimal, SyntheticMarketError> {
    Ok(base * (BigDecimal::from(1) + (percentage / decimal("100")?)))
}

fn slot_unit(config: &SyntheticMarketConfig, time: DateTime<Utc>, label: &[u8]) -> u64 {
    let digest = slot_digest(config, time, label);
    u64::from_be_bytes(digest[..8].try_into().expect("SHA-256 prefix length")) % ONE_MILLION
}

fn slot_signed_unit(config: &SyntheticMarketConfig, time: DateTime<Utc>, label: &[u8]) -> i64 {
    let digest = slot_digest(config, time, label);
    let unit = u64::from_be_bytes(digest[..8].try_into().expect("SHA-256 prefix length"))
        % (ONE_MILLION + 1);
    unit as i64 * 2 - ONE_MILLION as i64
}

fn slot_digest(config: &SyntheticMarketConfig, time: DateTime<Utc>, label: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(config.seed.as_bytes());
    hasher.update([0]);
    hasher.update(config.version.to_be_bytes());
    hasher.update([0]);
    hasher.update(config.symbol.as_bytes());
    hasher.update([0]);
    hasher.update(time.timestamp_millis().to_be_bytes());
    hasher.update(label);
    hasher.finalize().into()
}

fn decimal(value: &str) -> Result<BigDecimal, SyntheticMarketError> {
    BigDecimal::from_str(value).map_err(|_| SyntheticMarketError::DecimalCalculation)
}

fn decimal_from_unit(value: u64, scale: i64) -> Result<BigDecimal, SyntheticMarketError> {
    Ok(BigDecimal::new(value.into(), scale))
}

fn decimal_from_signed_unit(value: i64, scale: i64) -> Result<BigDecimal, SyntheticMarketError> {
    Ok(BigDecimal::new(value.into(), scale))
}

fn min_price(precision: u32) -> BigDecimal {
    BigDecimal::new(1.into(), i64::from(precision))
}

fn round_price(value: BigDecimal, precision: u32) -> BigDecimal {
    value.with_scale_round(i64::from(precision), RoundingMode::HalfUp)
}
