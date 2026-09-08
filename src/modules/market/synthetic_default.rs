//! 默认合成行情的纯分钟生成规则；只消费持久化分钟开盘锚点，不读取时钟、不补历史或发布行情。

use bigdecimal::{BigDecimal, RoundingMode};
use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{MarketKlineValues, SyntheticCandle, ValidatedMarketSymbol};
use crate::error::{AppError, AppResult};

/// 默认行情的配置模式；旧版本未保存此字段时保持原来的独立震荡算法。
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DefaultMarketMode {
    #[default]
    Independent,
    Follow,
}

/// 跟随只消费所选外部交易对的百分比变化；独立量价参数仍作为缺价时的明确兜底。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DefaultMarketFollowParameters {
    pub reference_pair_id: u64,
    #[serde(default = "default_follow_multiplier")]
    pub multiplier: BigDecimal,
    #[serde(default = "default_follow_max_move_ratio")]
    pub max_move_ratio: BigDecimal,
    #[serde(default = "default_follow_stale_after_seconds")]
    pub stale_after_seconds: u32,
}

impl DefaultMarketFollowParameters {
    /// 仅校验显式跟随参数；引用是否为启用的外部交易对由有目录上下文的应用层验证。
    pub fn validate(&self) -> AppResult<()> {
        if self.reference_pair_id == 0 {
            return Err(invalid("请选择有效的跟随交易对"));
        }
        for (label, ratio, maximum) in [
            ("跟随倍率", &self.multiplier, BigDecimal::from(3)),
            (
                "每分钟最大跟随幅度",
                &self.max_move_ratio,
                BigDecimal::from(1),
            ),
        ] {
            exact_decimal(ratio, 18, label)?;
            if ratio <= &BigDecimal::from(0) || ratio > &maximum {
                return Err(invalid(&format!("{label}须大于零且不超过{maximum}")));
            }
        }
        if !(15..=300).contains(&self.stale_after_seconds) {
            return Err(invalid("参考行情过期时间须在 15 至 300 秒之间"));
        }
        Ok(())
    }
}

fn default_follow_multiplier() -> BigDecimal {
    BigDecimal::from(1)
}
fn default_follow_max_move_ratio() -> BigDecimal {
    BigDecimal::new(5.into(), 2)
}
fn default_follow_stale_after_seconds() -> u32 {
    60
}

/// 默认生成参数作为不可变配置版本持久化；所有比例使用小数而非百分数字面量。
/// 初始价由运行层显式选择并持久化，不属于系统默认参数，缺价时不得在此编造价格。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct DefaultMarketParameters {
    pub mode: DefaultMarketMode,
    pub follow: Option<DefaultMarketFollowParameters>,
    pub volatility: BigDecimal,
    pub mean_reversion: BigDecimal,
    pub price_min: Option<BigDecimal>,
    pub price_max: Option<BigDecimal>,
    pub volume_min: BigDecimal,
    pub volume_max: BigDecimal,
    pub wick_strength: BigDecimal,
    pub depth_levels: u32,
}

impl Default for DefaultMarketParameters {
    /// 系统缺省采用每分钟最多 0.15% 收盘偏移、5% 回归系数和四分之一强度影线；不设置初始价格。
    fn default() -> Self {
        Self {
            mode: DefaultMarketMode::Independent,
            follow: None,
            volatility: BigDecimal::new(15.into(), 4),
            mean_reversion: BigDecimal::new(5.into(), 2),
            price_min: None,
            price_max: None,
            volume_min: BigDecimal::from(10),
            volume_max: BigDecimal::from(20),
            wick_strength: BigDecimal::new(25.into(), 2),
            depth_levels: 20,
        }
    }
}

impl DefaultMarketParameters {
    /// 按交易对精度拒绝越界参数，不自动舍入管理员配置；数值须符合 DECIMAL(38,18) 的整数容量。
    /// 比例限定 0..=1，量价范围须有序，盘口档位限定 1..=20；校验不读取或修改运行检查点。
    pub fn validate(&self, price_precision: u32, qty_precision: u32) -> AppResult<()> {
        match (self.mode, &self.follow) {
            (DefaultMarketMode::Independent, None) => {}
            (DefaultMarketMode::Follow, Some(follow)) => follow.validate()?,
            (DefaultMarketMode::Independent, Some(_)) => {
                return Err(invalid("独立震荡模式不应包含跟随参数"));
            }
            (DefaultMarketMode::Follow, None) => {
                return Err(invalid("跟随模式必须配置参考交易对及跟随参数"));
            }
        }
        if price_precision > 18 || qty_precision > 18 {
            return Err(invalid("价格和数量精度须在 0 至 18 位之间"));
        }
        for (label, ratio) in [
            ("波动率", &self.volatility),
            ("均值回归强度", &self.mean_reversion),
            ("影线强度", &self.wick_strength),
        ] {
            exact_decimal(ratio, 18, label)?;
            if ratio < &BigDecimal::from(0) || ratio > &BigDecimal::from(1) {
                return Err(invalid(&format!("{label}须在 0 至 1 之间")));
            }
        }
        for (label, price) in [("最低价格", &self.price_min), ("最高价格", &self.price_max)]
        {
            if let Some(price) = price {
                positive_price(price, price_precision, label)?;
            }
        }
        if self
            .price_min
            .as_ref()
            .zip(self.price_max.as_ref())
            .is_some_and(|(min, max)| min > max)
        {
            return Err(invalid("最低价格不得高于最高价格"));
        }
        for (label, volume) in [
            ("最小成交量", &self.volume_min),
            ("最大成交量", &self.volume_max),
        ] {
            exact_decimal(volume, qty_precision, label)?;
            if volume < &BigDecimal::from(0) {
                return Err(invalid(&format!("{label}不得为负数")));
            }
        }
        if self.volume_min > self.volume_max {
            return Err(invalid("最小成交量不得大于最大成交量"));
        }
        if !(1..=20).contains(&self.depth_levels) {
            return Err(invalid("盘口档数须在 1 至 20 档之间"));
        }
        Ok(())
    }
}

/// 从固定 seed、配置版本、UTC 分钟和持久化开盘价生成一根确定性 1m；不依赖上次调用或当前时钟。
/// 开盘与回归锚点必须为精度内正数，开盘不得越过显式价格范围；不静默修正续接价格。
/// 收盘先受单分钟波幅限制再受总价范围限制，量化边界后舍入确保不越界；影线与成交量独立派生。
/// 运行层必须先保存本分钟 open_price 再发布首帧；不能把逐秒变动的 last_price 当作同分钟的新开盘价。
#[allow(clippy::too_many_arguments)]
pub fn generate_default_1m(
    symbol: &str,
    seed: &str,
    version: u32,
    price_precision: u32,
    qty_precision: u32,
    open_time: DateTime<Utc>,
    open_price: &BigDecimal,
    anchor_price: &BigDecimal,
    parameters: &DefaultMarketParameters,
) -> AppResult<SyntheticCandle> {
    parameters.validate(price_precision, qty_precision)?;
    let symbol = ValidatedMarketSymbol::from_raw(symbol).map_err(|_| invalid("交易对符号无效"))?;
    if seed.trim().is_empty() || seed.len() > 128 || version == 0 {
        return Err(invalid(
            "随机种子须为 128 字节内的非空文本，配置版本须为正整数",
        ));
    }
    if open_time.second() != 0 || open_time.nanosecond() != 0 {
        return Err(invalid("K 线开盘时间须对齐 UTC 整分钟"));
    }
    positive_price(open_price, price_precision, "开盘价格")?;
    positive_price(anchor_price, price_precision, "回归锚定价格")?;
    if parameters
        .price_min
        .as_ref()
        .is_some_and(|min| open_price < min)
        || parameters
            .price_max
            .as_ref()
            .is_some_and(|max| open_price > max)
    {
        return Err(invalid("开盘价格须在配置的最低价和最高价之间"));
    }
    let unit = |label: &[u8]| slot_unit(symbol.as_str(), seed, version, open_time, label);
    let zero = BigDecimal::from(0);
    let one = BigDecimal::from(1);
    let tick = BigDecimal::new(1.into(), i64::from(price_precision));
    let movement = open_price * &parameters.volatility;
    let price_floor = parameters.price_min.clone().unwrap_or_else(|| tick.clone());
    // Storage keeps twenty integer digits, even for a pair with fewer fractional digits.
    let storage_ceiling = BigDecimal::new(1.into(), -20) - &tick;
    let price_ceiling = parameters.price_max.clone().unwrap_or(storage_ceiling);
    let minimum = (open_price - &movement)
        .max(price_floor.clone())
        .with_scale_round(i64::from(price_precision), RoundingMode::Ceiling);
    let maximum = (open_price + &movement)
        .min(price_ceiling.clone())
        .with_scale_round(i64::from(price_precision), RoundingMode::Floor);
    let noise = unit(b"close") * BigDecimal::from(2) - &one;
    let drift = (anchor_price - open_price) * &parameters.mean_reversion;
    let close = (open_price + drift + &movement * noise)
        .with_scale_round(i64::from(price_precision), RoundingMode::HalfUp)
        .max(minimum)
        .min(maximum);
    let body_high = open_price.clone().max(close.clone());
    let body_low = open_price.clone().min(close.clone());
    let wick = &movement * &parameters.wick_strength;
    let high = (&body_high + &wick * unit(b"high"))
        .with_scale_round(i64::from(price_precision), RoundingMode::HalfUp)
        .max(body_high)
        .min(price_ceiling);
    let low = (&body_low - &wick * unit(b"low"))
        .with_scale_round(i64::from(price_precision), RoundingMode::HalfUp)
        .min(body_low)
        .max(price_floor);
    let volume = (&parameters.volume_min
        + (&parameters.volume_max - &parameters.volume_min) * unit(b"volume"))
    .with_scale_round(i64::from(qty_precision), RoundingMode::HalfUp)
    .max(zero);
    Ok(SyntheticCandle {
        open_time,
        values: MarketKlineValues {
            open: open_price.clone(),
            high,
            low,
            close,
            volume,
        },
    })
}

/// 默认模式把既有秒级价格路径的累计量量化到交易对数量精度；相邻累计量相减保证逐笔可表示且分钟总量守恒。
/// 不修改旧人工策略的十八位成交量路径；完整分钟量必须已通过默认生成器的数量精度校验。
pub fn forming_default_1m_values(
    closed: &MarketKlineValues,
    open_time: DateTime<Utc>,
    observed_at: DateTime<Utc>,
    price_precision: u32,
    qty_precision: u32,
) -> AppResult<MarketKlineValues> {
    if price_precision > 18 || qty_precision > 18 {
        return Err(invalid("价格和数量精度须在 0 至 18 位之间"));
    }
    exact_decimal(&closed.volume, qty_precision, "分钟成交量")?;
    if closed.volume < 0 {
        return Err(invalid("分钟成交量不得为负数"));
    }
    let mut values = super::synthetic_realtime::forming_1m_values(
        closed,
        open_time,
        observed_at,
        price_precision,
    )?;
    values.volume = values
        .volume
        .with_scale_round(i64::from(qty_precision), RoundingMode::HalfUp);
    Ok(values)
}

/// 在分配 BigDecimal 舍入缓冲前限制指数、整数容量和有效小数位，避免极端指数配置触发巨大内存计算。
fn exact_decimal(value: &BigDecimal, precision: u32, label: &str) -> AppResult<()> {
    let normalized = value.normalized();
    let scale = normalized.fractional_digit_count();
    let integer_digits = i128::from(normalized.digits()) - i128::from(scale);
    if scale > i64::from(precision) || integer_digits > 20 {
        return Err(invalid(&format!(
            "{label}超出十进制存储容量或交易对允许精度"
        )));
    }
    Ok(())
}

/// 拒绝零价、负价和超精度输入，保持分钟续接锚点的真实配置值而非在生成时悄悄抬价。
fn positive_price(value: &BigDecimal, precision: u32, label: &str) -> AppResult<()> {
    exact_decimal(value, precision, label)?;
    if value <= &BigDecimal::from(0) {
        return Err(invalid(&format!("{label}须大于零")));
    }
    Ok(())
}

/// 独立的默认生成命名空间隔离旧策略摘要；字段用零字节分隔，返回 0..1 的六位确定性比例。
fn slot_unit(
    symbol: &str,
    seed: &str,
    version: u32,
    time: DateTime<Utc>,
    label: &[u8],
) -> BigDecimal {
    let mut digest = Sha256::new();
    digest.update(b"default-market-v1\0");
    digest.update(seed.as_bytes());
    digest.update([0]);
    digest.update(version.to_be_bytes());
    digest.update([0]);
    digest.update(symbol.as_bytes());
    digest.update([0]);
    digest.update(time.timestamp_millis().to_be_bytes());
    digest.update(label);
    let hash = digest.finalize();
    let value = u64::from_be_bytes(hash[..8].try_into().expect("SHA-256 prefix")) % 1_000_001;
    BigDecimal::new(value.into(), 6)
}

/// 返回默认参数/输入的可展示校验错误；本模块没有 I/O，因此失败前后都不会留下部分行情。
fn invalid(message: &str) -> AppError {
    AppError::Validation(format!("默认行情生成参数无效：{message}"))
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_market_synthetic_default_tests.rs"]
mod tests;
