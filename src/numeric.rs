//! 跨业务数值边界：输入保留十进制文本，存储校验不舍入，派生金额由所属业务按资产精度处理。

use crate::error::{AppError, AppResult};
use bigdecimal::BigDecimal;
use serde::{Deserialize, Deserializer, de::Error};
use std::str::FromStr;

pub const AMOUNT_PRECISION: u64 = 38;
pub const AMOUNT_SCALE: i64 = 18;
pub const MAX_DECIMAL_INPUT_LENGTH: usize = 256;
pub const MAX_DECIMAL_INPUT_EXPONENT: i64 = 256;

/// 验证值可被指定 DECIMAL 列原样保存；忽略尾随零，不展开指数，不依赖数据库隐式舍入。
pub fn ensure_decimal_storage(
    value: &BigDecimal,
    precision: u64,
    scale: i64,
    label: &str,
) -> AppResult<()> {
    if precision == 0 || precision > 65 || scale < 0 || scale as u64 > precision {
        return Err(AppError::Validation(format!(
            "{label} has invalid decimal bounds"
        )));
    }
    let (coefficient, exponent) = value.as_bigint_and_exponent();
    let text = coefficient.to_str_radix(10);
    let digits = text.trim_start_matches('-').trim_start_matches('0');
    if digits.is_empty() {
        return Ok(());
    }
    let significant = digits.trim_end_matches('0');
    let trailing = digits.len() - significant.len();
    // i128 防止极端 BigDecimal 指数减尾随零时发生 i64 溢出。
    let effective_scale = i128::from(exponent) - trailing as i128;
    let integer_digits = (significant.len() as i128 - effective_scale).max(0);
    if effective_scale.max(0) > i128::from(scale)
        || integer_digits > i128::from(precision) - i128::from(scale)
    {
        return Err(AppError::Validation(format!(
            "{label} exceeds DECIMAL({precision},{scale}) precision or range"
        )));
    }
    Ok(())
}

/// 金额写入前的数据库容量门槛；资产精度和正负业务规则仍由调用方单独校验。
pub fn ensure_amount_storage(value: &BigDecimal, label: &str) -> AppResult<()> {
    ensure_decimal_storage(value, AMOUNT_PRECISION, AMOUNT_SCALE, label)
}

/// 先限制文本与指数长度再构造 BigDecimal，拒绝异常指数及不可无损存储的输入，不把非法值当零。
pub fn parse_decimal_input(source: &str) -> Result<BigDecimal, String> {
    let source = source.trim();
    if source.is_empty() || source.len() > MAX_DECIMAL_INPUT_LENGTH {
        return Err("decimal input is empty or too long".to_owned());
    }
    if !source
        .bytes()
        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.' | b'e' | b'E'))
    {
        return Err("invalid decimal input".to_owned());
    }
    if let Some((_, exponent)) = source.split_once(['e', 'E']) {
        let exponent = exponent
            .parse::<i64>()
            .map_err(|_| "invalid decimal exponent".to_owned())?;
        if !(-MAX_DECIMAL_INPUT_EXPONENT..=MAX_DECIMAL_INPUT_EXPONENT).contains(&exponent) {
            return Err("decimal exponent is out of range".to_owned());
        }
    }
    let value = BigDecimal::from_str(source).map_err(|_| "invalid decimal input".to_owned())?;
    ensure_amount_storage(&value, "decimal input").map_err(|error| error.to_string())?;
    Ok(value)
}

fn decimal_value(value: serde_json::Value) -> Result<BigDecimal, String> {
    match value {
        serde_json::Value::String(source) => parse_decimal_input(&source),
        serde_json::Value::Number(number) => parse_decimal_input(&number.to_string()),
        _ => Err("decimal input must be a string or JSON number".to_owned()),
    }
}

/// 请求 DTO 的必填十进制入口；JSON 数字依赖 arbitrary_precision 保留字面值，绝不经过 f64。
pub fn deserialize_decimal<'de, D>(deserializer: D) -> Result<BigDecimal, D::Error>
where
    D: Deserializer<'de>,
{
    decimal_value(serde_json::Value::deserialize(deserializer)?).map_err(D::Error::custom)
}

/// 可空十进制请求入口；缺省由字段的 serde(default) 处理，null 保持 None。
pub fn deserialize_optional_decimal<'de, D>(deserializer: D) -> Result<Option<BigDecimal>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<serde_json::Value>::deserialize(deserializer)?
        .map(decimal_value)
        .transpose()
        .map_err(D::Error::custom)
}

/// 可选十进制列表逐项应用相同入口约束；缺省、null 与空数组保持原语义，不经浮点中转。
pub fn deserialize_optional_decimals<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<BigDecimal>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<Vec<serde_json::Value>>::deserialize(deserializer)?
        .map(|values| values.into_iter().map(decimal_value).collect())
        .transpose()
        .map_err(D::Error::custom)
}

/// PATCH 十进制字段保留缺省与显式清空的区别，同时对有值分支执行相同精度保护。
pub fn deserialize_patch_decimal<'de, D>(
    deserializer: D,
) -> Result<Option<Option<BigDecimal>>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_optional_decimal(deserializer).map(Some)
}

#[cfg(test)]
#[path = "../tests/unit_src/src_numeric_tests.rs"]
mod tests;
