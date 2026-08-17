//! 新币策略使用的纯 Rust 确定性行情生成器。
//!
//! 全部计算只依赖版本快照与目标时刻：随机性由 seed、版本、交易对、开盘时间和用途标签经 SHA-256 派生，
//! 每个分钟槽独立取值且不依赖前一槽的结果，因此实时发布、后台预览与手动补偿对同一分钟必然得到同一根蜡烛，
//! 进程重启、失败重试和分批边界都不会改变输出。
//!
//! 权威周期只有 1m，5m 及以上一律由完整且连续的 1m 窗口聚合而成，不单独生成。
//! 本模块不访问 MySQL、Mongo 或 Redis，也不广播任何事件；返回错误只表示版本快照或时间槽不合法。

use std::str::FromStr;

use bigdecimal::{BigDecimal, RoundingMode};
use chrono::{DateTime, Duration, Timelike, Utc};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::domain::MarketKlineValues;

const ONE_MILLION: u64 = 1_000_000;

/// 模拟行情版本使用的场景标签；场景只描述后台预设来源，最终输出始终由快照中的显式节点与高级参数决定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntheticScenario {
    CustomPath,
    TrendUp,
    TrendDown,
    Range,
    HighVolatility,
    CrashRecovery,
    PumpThenDump,
}

impl SyntheticScenario {
    /// 返回版本快照与后台 API 使用的稳定场景代码，禁止把中文展示名写入持久化枚举。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CustomPath => "custom_path",
            Self::TrendUp => "trend_up",
            Self::TrendDown => "trend_down",
            Self::Range => "range",
            Self::HighVolatility => "high_volatility",
            Self::CrashRecovery => "crash_recovery",
            Self::PumpThenDump => "pump_then_dump",
        }
    }
}

/// 版本 seed 的管理方式；两种模式都必须在版本行中保存实际 seed，确保生成结果可重放。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntheticSeedMode {
    Auto,
    Fixed,
}

impl SyntheticSeedMode {
    /// 返回创建、编辑与版本详情共同使用的稳定 seed 模式代码。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Fixed => "fixed",
        }
    }
}

/// 成交量随策略全局进度变化的形态；基础随机刻度仍由版本 seed 确定性派生。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntheticVolumeShape {
    Uniform,
    Trend,
    Bell,
    EndSpike,
}

impl SyntheticVolumeShape {
    /// 返回版本快照与后台 API 使用的稳定成交量形态代码。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Uniform => "uniform",
            Self::Trend => "trend",
            Self::Bell => "bell",
            Self::EndSpike => "end_spike",
        }
    }
}

/// 单个不可变版本中的高级生成参数；默认值与 0102 版本上线时的固定算法常量完全一致。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticGeneratorSettings {
    pub scenario: SyntheticScenario,
    pub seed_mode: SyntheticSeedMode,
    pub mean_reversion_strength: BigDecimal,
    pub noise_scale: BigDecimal,
    pub wick_scale: BigDecimal,
    pub volume_shape: SyntheticVolumeShape,
}

impl Default for SyntheticGeneratorSettings {
    /// 为缺少 `generator` 对象的历史版本提供字节兼容默认值，不读取环境变量或当前后台配置。
    fn default() -> Self {
        Self {
            scenario: SyntheticScenario::CustomPath,
            seed_mode: SyntheticSeedMode::Auto,
            mean_reversion_strength: BigDecimal::new(55.into(), 2),
            noise_scale: BigDecimal::from(1),
            wick_scale: BigDecimal::new(75.into(), 2),
            volume_shape: SyntheticVolumeShape::Uniform,
        }
    }
}

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
    /// 取值固定为 5m、15m、1h、4h、1d，会原样写入 Mongo 文档的 `interval` 字段并进入实时缓存键。
    /// 枚举不含 1m，因为 1m 是确定性生成的权威值而非聚合产物；改动这些字面量会让历史蜡烛无法按周期检索。
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
    /// 5m 至 1d 依次为 5、15、60、240、1440 根，既用于校验聚合输入数量，也用于推算窗口起点和跨度。
    /// 聚合要求恰好这个根数，缺一根即视为窗口不完整，因此该值同时是补偿任务判断窗口是否可重建的口径。
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

impl SyntheticTargetType {
    /// 返回节点版本快照使用的稳定目标类型代码，关系表与 JSON 必须保持同一字面量。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AbsolutePrice => "absolute_price",
            Self::PercentFromStart => "percent_from_start",
            Self::PercentFromPrevious => "percent_from_previous",
        }
    }
}

/// 节点执行约束：hard 精确命中，soft/range 在容差带内确定性取值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntheticExecutionMode {
    Hard,
    Soft,
    Range,
}

impl SyntheticExecutionMode {
    /// 返回节点版本快照使用的稳定执行模式代码，禁止用展示文案替换持久化枚举。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hard => "hard",
            Self::Soft => "soft",
            Self::Range => "range",
        }
    }
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
    pub generator: SyntheticGeneratorSettings,
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
    #[error("synthetic generator parameters are outside their supported ranges")]
    InvalidGeneratorParameter,
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
    /// 交易对会被裁剪并转为大写，交易对或种子为空即拒绝；节点时间必须严格落在策略起止之间且逐个递增。
    /// 绝对价类型的节点目标值必须为正，成交量上下限只能同时给出或同时省略。
    /// 最后还会试算一次锚点序列，因此按涨跌幅算出非正价格的配置也在这里被拒绝，而不是等到生成蜡烛时才失败。
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
        validate_generator_settings(&config.generator)?;

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
    /// 开盘时间必须分钟对齐且落在策略区间内，否则返回 `InvalidOpenTime`；开收价分别取本分钟与下一分钟的路径价。
    /// 上下影线按各自方向的确定性刻度乘以局部波动率与版本影线系数展开，成交量按版本形态映射到局部区间。
    /// 取整后会再次收敛不变量：最高不低于开收较大者，最低不高于开收较小者且不低于精度对应的最小价。
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
        let wick_factor = self.generator.wick_scale.clone();
        let high = &body_high + (&body_high * &volatility * upper * &wick_factor);
        let low = (&body_low - (&body_low * &volatility * lower * wick_factor))
            .max(min_price(self.price_precision));
        let (volume_min, volume_max) = self.local_volume_range(open_time);
        let volume_unit = self.shaped_volume_unit(open_time)?;
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

    /// 计算任意分钟时刻的确定性价格：正好命中锚点时走锚点取值，否则在左右锚点之间按分钟数线性插值。
    /// 插值结果再叠加种子派生的有符号噪声，幅度由局部波动率、版本均值回归强度、噪声强度与 `f*(1-f)` 包络共同决定。
    /// 包络在区间两端为零，因此无论噪声取何值，锚点时刻的价格都不会被扰动，整条路径必然穿过既定节点。
    /// 结果不低于价格精度对应的最小价；本函数只做内存计算，同一版本与时刻可无限次复现同一结果。
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
        let adjustment = &bridge
            * self.local_volatility(time)
            * envelope
            * noise
            * &self.generator.mean_reversion_strength
            * &self.generator.noise_scale;
        Ok((bridge + adjustment).max(min_price(self.price_precision)))
    }

    /// 把版本 seed 派生的基础成交量刻度按策略全局进度塑形成均匀、递增、钟形或尾部放量。
    /// `uniform` 原样返回旧算法刻度以保持历史版本兼容；其余形态把确定性纹理与进度曲线混合，结果始终夹在 0～1。
    /// 本函数不改变节点的成交量上下界，只决定在当前局部区间中的相对位置，因此所有形态都继续满足非负与上下限约束。
    fn shaped_volume_unit(
        &self,
        open_time: DateTime<Utc>,
    ) -> Result<BigDecimal, SyntheticMarketError> {
        let random = decimal_from_unit(slot_unit(self, open_time, b"volume"), 6)?;
        if self.generator.volume_shape == SyntheticVolumeShape::Uniform {
            return Ok(random);
        }
        let total = (self.end_time - self.start_time).num_seconds().max(1);
        let elapsed = (open_time - self.start_time).num_seconds().clamp(0, total);
        let progress = BigDecimal::from(elapsed) / BigDecimal::from(total);
        let shaped = match self.generator.volume_shape {
            SyntheticVolumeShape::Uniform => random,
            SyntheticVolumeShape::Trend => {
                (&random * decimal("0.45")?) + (&progress * decimal("0.55")?)
            }
            SyntheticVolumeShape::Bell => {
                let bell = BigDecimal::from(4) * &progress * (BigDecimal::from(1) - &progress);
                (&random * decimal("0.35")?) + (bell * decimal("0.65")?)
            }
            SyntheticVolumeShape::EndSpike => {
                let tail = &progress * &progress * &progress;
                (&random * decimal("0.35")?) + (tail * decimal("0.65")?)
            }
        };
        Ok(shaped.max(BigDecimal::from(0)).min(BigDecimal::from(1)))
    }

    /// 给出锚点时刻的落地价格：hard 精确返回节点价，soft 与 range 在容差带内做确定性偏移。
    /// 偏移量等于节点价乘以容差百分比、种子派生的有符号噪声和模式系数，soft 取 0.5、range 取 1，
    /// 因此 soft 的实际波动带只有 range 的一半；容差配置为零时两种模式都退化为精确命中。
    /// 结果同样以最小价兜底；本函数不读取版本快照之外的数据，对同一锚点和时刻恒定。
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

    /// 把策略起点、有序节点与终点目标价展开成按时间递增的锚点序列，插值和锚点取值都以它为唯一依据。
    /// 节点价按类型换算：绝对价直接取用，相对起点以起始价为基数，相对前节点以上一节点换算出的未取整价为基数，
    /// 第一个节点无论哪种相对类型都以起始价为基数；换算结果非正立即报错，入列锚点按交易对精度四舍五入。
    /// 节点时间恰好等于策略起点时会清空已有锚点由该节点接管，末锚点早于结束时间时补一个终点硬锚。
    /// 本函数无 I/O，每次都从同一份版本快照重新推导，因此结果与调用次数和顺序无关。
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

    /// 取该时刻所在区段的波动率：命中第一个目标时间不早于它的节点，并采用该节点的波动率。
    /// 节点时间严格递增，因此选中的就是即将到达的那个节点，最后一个节点之后回落到策略级波动率。
    /// 波动率同时决定分钟内影线长度和插值噪声幅度，取零表示该区段严格沿锚点路径运行。
    fn local_volatility(&self, time: DateTime<Utc>) -> BigDecimal {
        self.nodes
            .iter()
            .find(|node| node.target_time >= time)
            .map_or_else(|| self.volatility.clone(), |node| node.volatility.clone())
    }

    /// 取该时刻所在区段的成交量区间，选节点的口径与局部波动率一致，同样取即将到达的那个节点。
    /// 节点必须同时配置上下限才生效，只配一侧的组合已在构造校验时被拒绝，这里只需处理成对存在的情况。
    /// 未命中节点或节点未配置区间时回落到策略级区间，最终成交量在该区间内按种子派生比例取值。
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
/// 输入根数必须恰好等于该周期所需数量，且首根开盘时间落在窗口边界上，否则返回窗口不完整。
/// 逐根要求开盘时间连续、OHLCV 合法，并且前一根收盘价等于后一根开盘价，任一条不满足即整体失败。
/// 结果取首根开盘价、末根收盘价、窗口内最高价与最低价，成交量为窗口内求和，不做精度归一。
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
    /// 构造起点与终点使用的硬锚：执行模式固定为 hard、容差固定为零，价格必须被精确命中。
    /// 策略起始价和终点目标价都经由它进入锚点序列，因此模拟行情的首尾价格不受任何噪声扰动。
    fn hard(time: DateTime<Utc>, price: BigDecimal) -> Self {
        Self {
            time,
            price,
            mode: SyntheticExecutionMode::Hard,
            tolerance: BigDecimal::from(0),
        }
    }
}

/// 判断时间是否落在整分钟边界，要求秒与纳秒同时为零。
/// 策略起止时间、节点时间和 1m 生成入口都用它做前置校验，未对齐的时间一律被拒绝而不是向下取整。
fn is_minute_aligned(time: DateTime<Utc>) -> bool {
    time.second() == 0 && time.nanosecond() == 0
}

/// 判断时间是否为该周期的窗口起点，按周期分钟数换算成秒后对时间戳取模。
/// 取模使用欧几里得余数，1970 之前的时间同样得到非负结果，只有余数为零才允许作为聚合窗口首根。
/// 边界一律按 UTC 切分，因此 4h 与 1d 窗口不受部署时区影响，与存储中的开盘时间槽位完全对齐。
fn is_aggregate_boundary(time: DateTime<Utc>, interval: SyntheticKlineInterval) -> bool {
    time.timestamp()
        .rem_euclid((interval.minute_count() as i64) * 60)
        == 0
}

/// 校验单根蜡烛的 OHLCV 不变量：开收必须为正，最高不低于开收，最低不高于开收且仍为正，成交量非负。
/// 聚合前逐根检查，任何一根违反都会让整个窗口失败，避免把脏数据折叠进高周期蜡烛。
/// 这里不检查时间连续性，槽位顺序与前后开收衔接由聚合函数单独判断。
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

/// 拒绝负数参数，用于波动率、容差这类只允许零或正值的配置项。
/// 零是合法取值，表示不引入波动或不允许偏移；违规统一返回 `NegativeParameter`，不区分具体字段。
fn validate_non_negative(value: &BigDecimal) -> Result<(), SyntheticMarketError> {
    if value < &BigDecimal::from(0) {
        Err(SyntheticMarketError::NegativeParameter)
    } else {
        Ok(())
    }
}

/// 校验成交量区间：上下限都不得为负，且上限不得小于下限。
/// 上下限相等表示固定成交量，属于允许的配置；区间倒置返回 `InvalidVolumeRange`。
/// 策略级区间与节点级区间共用本校验，两处的合法性口径因此完全一致。
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

/// 校验高级生成参数的稳定范围；场景、seed 模式与成交量形态由枚举类型保证，这里只验证高精度数值。
/// 均值回归强度允许 0～2，噪声与影线强度允许 0～5，边界值均合法；超界时拒绝整个版本而不做夹紧。
fn validate_generator_settings(
    settings: &SyntheticGeneratorSettings,
) -> Result<(), SyntheticMarketError> {
    let zero = BigDecimal::from(0);
    let two = BigDecimal::from(2);
    let five = BigDecimal::from(5);
    if settings.mean_reversion_strength < zero
        || settings.mean_reversion_strength > two
        || settings.noise_scale < zero
        || settings.noise_scale > five
        || settings.wick_scale < zero
        || settings.wick_scale > five
    {
        return Err(SyntheticMarketError::InvalidGeneratorParameter);
    }
    Ok(())
}

/// 按百分比在基准价上换算目标价，传入 25 表示相对基准上涨百分之二十五，传入负值表示下跌。
/// 计算保持 `BigDecimal` 全精度，不在此处按交易对精度取整，取整统一由锚点解析完成。
/// 百分比低于负一百会算出非正价格，该情形由调用方判定为错误，本函数不做夹紧。
fn percent_price(
    base: &BigDecimal,
    percentage: &BigDecimal,
) -> Result<BigDecimal, SyntheticMarketError> {
    Ok(base * (BigDecimal::from(1) + (percentage / decimal("100")?)))
}

/// 取槽位摘要前 8 字节的大端整数并对一百万取模，得到 0 至 999999 的确定性无符号刻度。
/// 影线长度和成交量在区间内的位置都由它决定；同一 seed、版本、交易对、时刻与标签必然得到同一刻度。
fn slot_unit(config: &SyntheticMarketConfig, time: DateTime<Utc>, label: &[u8]) -> u64 {
    let digest = slot_digest(config, time, label);
    u64::from_be_bytes(digest[..8].try_into().expect("SHA-256 prefix length")) % ONE_MILLION
}

/// 与无符号刻度同源，但先对一百万零一取模再线性映射到负一百万至正一百万的对称区间。
/// 价格噪声和锚点容差偏移用它同时决定方向与幅度，因此偏移在正负两侧对称且可完全复现。
fn slot_signed_unit(config: &SyntheticMarketConfig, time: DateTime<Utc>, label: &[u8]) -> i64 {
    let digest = slot_digest(config, time, label);
    let unit = u64::from_be_bytes(digest[..8].try_into().expect("SHA-256 prefix length"))
        % (ONE_MILLION + 1);
    unit as i64 * 2 - ONE_MILLION as i64
}

/// 用 SHA-256 派生单个时间槽的随机性，依次写入 seed、版本、交易对、毫秒时间戳和用途标签。
/// 前四段之间插入零字节分隔，避免不同字段拼接出相同输入；标签让同一时刻的不同用途得到互不相关的摘要。
/// 每个槽位独立派生而不依赖前一槽，这正是重启、重试和分批边界都不改变生成结果的根本原因。
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

/// 把源码内写死的十进制字面量解析为 `BigDecimal`，用于 0.75、0.55、100 这类算法常数。
/// 解析失败归类为 `DecimalCalculation`，实际只会在常数被改错时触发，运行期输入不会流入本函数。
fn decimal(value: &str) -> Result<BigDecimal, SyntheticMarketError> {
    BigDecimal::from_str(value).map_err(|_| SyntheticMarketError::DecimalCalculation)
}

/// 把无符号槽位刻度按给定小数位定标为 `BigDecimal`，调用处统一取 6 位，即映射到 0 至约 1 的比例。
/// 只做定标不做取整或夹紧；返回 `Result` 是为了与其他小数构造保持同一错误签名，当前实现不会失败。
fn decimal_from_unit(value: u64, scale: i64) -> Result<BigDecimal, SyntheticMarketError> {
    Ok(BigDecimal::new(value.into(), scale))
}

/// 把有符号槽位刻度按给定小数位定标为 `BigDecimal`，调用处取 6 位，即映射到负一至正一的对称比例。
/// 符号完全来自刻度本身，本函数不改变正负，也不做取整；返回 `Result` 仅为与其他小数构造签名对齐。
fn decimal_from_signed_unit(value: i64, scale: i64) -> Result<BigDecimal, SyntheticMarketError> {
    Ok(BigDecimal::new(value.into(), scale))
}

/// 返回该价格精度下的最小可表示正价，例如精度 8 对应 0.00000001。
/// 插值噪声、下影线和锚点偏移都以它兜底，确保任何蜡烛的价格严格为正，不会跌到零或负数。
fn min_price(precision: u32) -> BigDecimal {
    BigDecimal::new(1.into(), i64::from(precision))
}

/// 按交易对价格精度做四舍五入，统一采用 HalfUp 规则收敛小数位。
/// 锚点价与权威 1m 的开高低收都经它取整，聚合与手动补偿复用这批已取整的值，
/// 因此实时、预览和补偿三条路径对同一分钟得到完全一致的 OHLC。
fn round_price(value: BigDecimal, precision: u32) -> BigDecimal {
    value.with_scale_round(i64::from(precision), RoundingMode::HalfUp)
}
