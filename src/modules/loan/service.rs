//! loan bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。

use crate::{
    error::{AppError, AppResult},
    modules::{
        loan::domain::{
            INTEREST_MODE_ACTUAL_DAYS, INTEREST_MODE_FULL_TERM, LOAN_PRODUCT_NAME_TITLE_MAX_LEN,
            LOAN_TYPE_COLLATERALIZED, LOAN_TYPE_CREDIT,
        },
        wallet::truncate_amount_to_asset_precision,
    },
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};

/// 贷款业务中用于金额比较的零值基准，固定使用 18 位精度，避免尾差带来的边界歧义。
fn zero_amount() -> BigDecimal {
    BigDecimal::from(0).with_scale(18)
}

/// 计算还款利息：full_term 为 `本金*利率`，actual_days 为该值再乘计费天数/产品天数。
/// 实际天数按秒数向上取整，放款后至少计一天且最多 term_days；当前时刻早于放款时也按一天计算。
/// 结果按贷款资产 precision_scale 向零截断；未知计息模式返回校验错误，本函数不扣钱包或写流水。
pub(crate) fn calculate_interest_amount(
    principal: &BigDecimal,
    interest_rate: &BigDecimal,
    mode: &str,
    term_days: u32,
    disbursed_at: DateTime<Utc>,
    now: DateTime<Utc>,
    precision_scale: i32,
) -> AppResult<BigDecimal> {
    let raw_interest = match mode {
        INTEREST_MODE_FULL_TERM => principal.clone() * interest_rate.clone(),
        INTEREST_MODE_ACTUAL_DAYS => {
            let elapsed_seconds = (now - disbursed_at).num_seconds().max(0);
            let elapsed_days = ((elapsed_seconds + 86_399) / 86_400).max(1);
            let charged_days = elapsed_days.min(i64::from(term_days));
            principal.clone() * interest_rate.clone() * BigDecimal::from(charged_days)
                / BigDecimal::from(term_days)
        }
        _ => {
            return Err(AppError::Validation(
                "unsupported interest_calculation_mode".to_owned(),
            ));
        }
    };
    Ok(truncate_amount_to_asset_precision(
        &raw_interest,
        precision_scale,
    ))
}

/// 校验借款金额不低于产品最小额且不超过可选最大额。
/// 该纯规则不裁剪金额，精度校验由调用方按贷款资产另行执行。
pub(crate) fn ensure_amount_within_product_limits(
    amount: &BigDecimal,
    min_amount: &BigDecimal,
    max_amount: &Option<BigDecimal>,
) -> AppResult<()> {
    if amount < min_amount {
        return Err(AppError::Validation(
            "amount is below product minimum".to_owned(),
        ));
    }
    if let Some(max_amount) = max_amount.as_ref()
        && amount > max_amount
    {
        return Err(AppError::Validation(
            "amount exceeds product maximum".to_owned(),
        ));
    }
    Ok(())
}

/// 规范借贷类型，只接受信用贷或抵押贷。
pub(crate) fn validate_loan_type(value: &str) -> AppResult<String> {
    let value = optional_string(Some(value.to_owned()))
        .ok_or_else(|| AppError::Validation("loan_type is required".to_owned()))?;
    match value.as_str() {
        LOAN_TYPE_CREDIT | LOAN_TYPE_COLLATERALIZED => Ok(value),
        _ => Err(AppError::Validation("unsupported loan_type".to_owned())),
    }
}

/// 规范计息模式，只接受全期限或实际天数计息。
pub(crate) fn validate_interest_mode(value: &str) -> AppResult<String> {
    let value = optional_string(Some(value.to_owned()))
        .ok_or_else(|| AppError::Validation("interest_calculation_mode is required".to_owned()))?;
    match value.as_str() {
        INTEREST_MODE_FULL_TERM | INTEREST_MODE_ACTUAL_DAYS => Ok(value),
        _ => Err(AppError::Validation(
            "unsupported interest_calculation_mode".to_owned(),
        )),
    }
}

/// 规范借贷产品状态，只接受 active 或 disabled。
pub(crate) fn validate_product_status(value: &str) -> AppResult<String> {
    let value = optional_string(Some(value.to_owned()))
        .ok_or_else(|| AppError::Validation("status is required".to_owned()))?;
    if value == "active" || value == "disabled" {
        Ok(value)
    } else {
        Err(AppError::Validation(
            "unsupported loan product status".to_owned(),
        ))
    }
}

/// 裁剪并校验用户借贷幂等键，空值或超过 255 字节时拒绝创建订单。
/// 该函数不把键与产品/金额绑定；重复请求内容一致性当前由调用方保证。
pub(crate) fn validate_idempotency_key(value: String) -> AppResult<String> {
    let value = optional_string(Some(value))
        .ok_or_else(|| AppError::Validation("idempotency_key is required".to_owned()))?;
    if value.len() > 255 {
        return Err(AppError::Validation(
            "idempotency_key is too long".to_owned(),
        ));
    }
    Ok(value)
}

/// 校验指定金额字段严格大于零。
pub(crate) fn ensure_positive_amount(amount: &BigDecimal, field: &str) -> AppResult<()> {
    if amount <= &zero_amount() {
        return Err(AppError::Validation(format!("{field} must be positive")));
    }
    Ok(())
}

/// 校验指定金额字段不小于零。
pub(crate) fn ensure_non_negative_amount(amount: &BigDecimal, field: &str) -> AppResult<()> {
    if amount < &zero_amount() {
        return Err(AppError::Validation(format!(
            "{field} must be non-negative"
        )));
    }
    Ok(())
}

/// 确认金额有效小数位不超过资产 precision_scale，拒绝隐式舍入；尾随零不增加有效精度。
pub(crate) fn ensure_amount_precision(
    amount: &BigDecimal,
    precision_scale: i32,
    field: &str,
) -> AppResult<()> {
    if amount_scale_within_precision(amount, precision_scale) {
        return Ok(());
    }
    Err(AppError::Validation(format!(
        "{field} exceeds asset precision_scale {precision_scale}"
    )))
}

fn amount_scale_within_precision(amount: &BigDecimal, precision_scale: i32) -> bool {
    let (_, scale) = amount.normalized().as_bigint_and_exponent();
    scale.max(0) <= precision_scale.into()
}

/// 裁剪可选字符串，并把纯空白内容归一为空值。
pub(crate) fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 按产品名称构造兼容的默认中文多语言名称结构。
pub(crate) fn default_product_name_json(name: &str) -> Value {
    json!({
        "version": 1,
        "default_locale": "zh-CN",
        "items": [
            {
                "locale": "zh-CN",
                "country": "CN",
                "title": name
            }
        ]
    })
}

/// 补充默认名称结构并执行完整多语言格式校验。
pub(crate) fn normalized_product_name_json(
    value: Option<Value>,
    fallback_name: &str,
) -> AppResult<Value> {
    let name_json = value.unwrap_or_else(|| default_product_name_json(fallback_name));
    validate_product_name_json(&name_json)?;
    Ok(name_json)
}

/// 校验名称版本、默认语言、条目必填字段和标题长度。
/// 默认语言必须在条目中存在，失败时产品配置不得进入持久化。
pub(crate) fn validate_product_name_json(value: &Value) -> AppResult<()> {
    let object = value.as_object().ok_or_else(|| {
        AppError::Validation("loan product name_json must be an object".to_owned())
    })?;
    if object.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(AppError::Validation(
            "loan product name_json version must be 1".to_owned(),
        ));
    }
    let default_locale = object
        .get("default_locale")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::Validation("loan product name_json default_locale is required".to_owned())
        })?;
    let items = object
        .get("items")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or_else(|| {
            AppError::Validation("loan product name_json items are required".to_owned())
        })?;
    let mut has_default_locale = false;
    for item in items {
        let item_object = item.as_object().ok_or_else(|| {
            AppError::Validation("loan product name_json item must be an object".to_owned())
        })?;
        let locale = required_product_name_string(item_object.get("locale"), "locale")?;
        if locale == default_locale {
            has_default_locale = true;
        }
        required_product_name_string(item_object.get("country"), "country")?;
        let title = required_product_name_string(item_object.get("title"), "title")?;
        if title.chars().count() > LOAN_PRODUCT_NAME_TITLE_MAX_LEN {
            return Err(AppError::Validation(
                "loan product name_json title is too long".to_owned(),
            ));
        }
    }
    if !has_default_locale {
        return Err(AppError::Validation(
            "loan product name_json default_locale must exist in items".to_owned(),
        ));
    }
    Ok(())
}

fn required_product_name_string<'a>(value: Option<&'a Value>, field: &str) -> AppResult<&'a str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation(format!("loan product name_json {field} is required")))
}

/// 从贷款产品多语言名称中优先读取默认语言标题，缺失时回退到首个非空标题。
/// JSON 结构不完整或不存在可展示标题时返回 `None`，本函数不修改原始配置。
pub(crate) fn product_default_name(value: &Value) -> Option<String> {
    let object = value.as_object()?;
    let default_locale = object.get("default_locale")?.as_str()?.trim();
    let items = object.get("items")?.as_array()?;
    let default_title = items.iter().find_map(|item| {
        let item_object = item.as_object()?;
        if item_object.get("locale")?.as_str()?.trim() != default_locale {
            return None;
        }
        item_object
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(ToOwned::to_owned)
    });
    default_title
        .or_else(|| {
            items.iter().find_map(|item| {
                item.as_object()?
                    .get("title")?
                    .as_str()
                    .map(str::trim)
                    .filter(|title| !title.is_empty())
                    .map(ToOwned::to_owned)
            })
        })
        .filter(|title| !title.is_empty())
}

/// 将借贷列表数量规范为默认 50、最小 1、最大 200。
pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 200)
}

/// 偏移同样设上限：超大 offset 会让日志类大表退化为全表扫描加文件排序。
pub(crate) fn route_offset(offset: Option<u32>) -> u32 {
    offset.unwrap_or(0).min(100_000)
}

/// 从 subject 中提取 user id，形如 `user:123`。
pub(crate) fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 从 subject 中提取 admin id，形如 `admin:123`。
pub(crate) fn admin_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("admin:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}
