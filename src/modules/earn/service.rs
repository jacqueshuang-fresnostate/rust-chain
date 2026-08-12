//! earn bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务服务逐步迁入。

use crate::{
    error::{AppError, AppResult},
    modules::earn::{
        presentation::{
            CreateEarnProductRequest, EarnCategoryResponse, EarnProductResponse,
            EarnSubscriptionResponse, UpdateEarnProductRequest,
        },
        redemption::{
            EARLY_REDEEM_FEE_BASIS_NONE, EARLY_REDEEM_FEE_BASIS_PRINCIPAL,
            EARLY_REDEEM_FEE_BASIS_PROFIT, EarnRedemptionAmounts, EarnRedemptionTerms,
            calculate_earn_redemption_amounts,
        },
        repository::{EarnProductFeeConfig, EarnProductRuleRow},
    },
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};

/// 从管理员 subject 提取可信编号，格式不符时返回未授权。
pub(crate) fn admin_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("admin:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 从用户 subject 提取可信编号，格式不符时返回未授权。
pub(crate) fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 把理财列表数量限制在一到一百条。
pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

/// 偏移同样设上限：超大 offset 会让日志类大表退化为全表扫描加文件排序。
pub(crate) fn route_offset(offset: Option<u32>) -> u32 {
    offset.unwrap_or(0).min(100_000)
}

/// 裁剪可选字符串，并把空白内容归一为空值。
pub(crate) fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 把产品当前 APR、期限、额度、分类及全部费用配置映射为管理员审计快照。
/// 该快照只记录配置变更；既有订阅仍使用申购时复制的费用字段，不会被审计映射改写。
pub(crate) fn product_audit_json(product: &EarnProductResponse) -> Value {
    json!({
        "id": product.id,
        "asset_id": product.asset_id,
        "asset_symbol": product.asset_symbol,
        "name": product.name,
        "banner_url": product.banner_url,
        "small_logo_url": product.small_logo_url,
        "category": product.category,
        "category_name": product.category_name,
        "category_name_json": product.category_name_json.as_ref().map(|value| value.0.clone()),
        "introduction_json": product.introduction_json.0.clone(),
        "term_days": product.term_days,
        "apr_rate": product.apr_rate,
        "redemption_fee_rate": product.redemption_fee_rate,
        "maturity_profit_fee_rate": product.maturity_profit_fee_rate,
        "early_redeem_fee_basis": product.early_redeem_fee_basis,
        "early_redeem_fee_rate": product.early_redeem_fee_rate,
        "min_subscribe": product.min_subscribe,
        "max_subscribe": product.max_subscribe,
        "status": product.status,
    })
}

/// 把分类代码、多语言名称、排序和状态映射为审计快照。
pub(crate) fn category_audit_json(category: &EarnCategoryResponse) -> Value {
    json!({
        "id": category.id,
        "code": category.code,
        "name_json": category.name_json.0.clone(),
        "default_name": category.default_name,
        "sort_order": category.sort_order,
        "status": category.status,
    })
}

#[allow(clippy::too_many_arguments)] // 纯函数校验完整产品快照，显式参数防止局部更新绕过字段约束。
fn validate_product_request_fields(
    asset_id: u64,
    name: &str,
    term_days: u32,
    apr_rate: &BigDecimal,
    min_subscribe: &BigDecimal,
    max_subscribe: Option<&BigDecimal>,
    status: Option<&str>,
    category: Option<&str>,
    introduction_json: Option<Value>,
    reason: Option<&str>,
) -> AppResult<()> {
    if asset_id == 0 {
        return Err(AppError::Validation("asset_id is required".to_owned()));
    }
    let Some(name) = optional_string(Some(name.to_owned())) else {
        return Err(AppError::Validation(
            "earn product name is required".to_owned(),
        ));
    };
    if name.chars().count() > EARN_PRODUCT_NAME_MAX_LEN {
        return Err(AppError::Validation(
            "earn product name is too long".to_owned(),
        ));
    }
    validate_term_days(term_days)?;
    validate_apr_rate(apr_rate)?;
    validate_amount(min_subscribe)?;
    if let Some(max_subscribe) = max_subscribe {
        validate_amount(max_subscribe)?;
        if max_subscribe < min_subscribe {
            return Err(AppError::Validation(
                "earn product max_subscribe must be greater than or equal to min_subscribe"
                    .to_owned(),
            ));
        }
    }
    if let Some(status) = status {
        normalized_product_status(status)?;
    }
    normalized_product_category(category)?;
    normalized_introduction_json(introduction_json, &name)?;
    validate_optional_reason(reason)?;
    Ok(())
}

/// 校验新产品的资产、名称、期限、收益率、额度、分类、介绍及审计原因。
/// 该纯规则不访问数据库；失败时应用层不得创建产品或写入审计。
pub(crate) fn validate_create_product_request(request: &CreateEarnProductRequest) -> AppResult<()> {
    validate_product_request_fields(
        request.asset_id,
        &request.name,
        request.term_days,
        &request.apr_rate,
        &request.min_subscribe,
        request.max_subscribe.as_ref(),
        request.status.as_deref(),
        request.category.as_deref(),
        request.introduction_json.clone(),
        request.reason.as_deref(),
    )
}

/// 按完整快照校验更新产品字段，防止局部更新绕过额度或内容规则。
/// 校验不修改既有产品、订阅费用快照或用户钱包。
pub(crate) fn validate_update_product_request(request: &UpdateEarnProductRequest) -> AppResult<()> {
    validate_product_request_fields(
        request.asset_id,
        &request.name,
        request.term_days,
        &request.apr_rate,
        &request.min_subscribe,
        request.max_subscribe.as_ref(),
        Some(request.status.as_str()),
        request.category.as_deref(),
        request.introduction_json.clone(),
        request.reason.as_deref(),
    )?;
    Ok(())
}

/// 为新产品补齐赎回费、到期收益费和提前赎回费规则，并校验各费率位于 0..=1、最多 8 位小数。
/// early basis 为 none 时强制提前赎回费率归零；该纯规则不写产品或历史订阅。
pub(crate) fn product_fee_config_from_create_request(
    request: &CreateEarnProductRequest,
) -> AppResult<EarnProductFeeConfig> {
    normalized_product_fee_config(
        request.redemption_fee_rate.as_ref(),
        request.maturity_profit_fee_rate.as_ref(),
        request.early_redeem_fee_basis.as_deref(),
        request.early_redeem_fee_rate.as_ref(),
    )
}

/// 规范更新产品的全部费用字段；费率位于 0..=1 且最多 8 位小数，none 基准强制提前赎回费率归零。
/// 返回值供新配置和未来订阅快照使用，不重算既有订阅费用。
pub(crate) fn product_fee_config_from_update_request(
    request: &UpdateEarnProductRequest,
) -> AppResult<EarnProductFeeConfig> {
    normalized_product_fee_config(
        request.redemption_fee_rate.as_ref(),
        request.maturity_profit_fee_rate.as_ref(),
        request.early_redeem_fee_basis.as_deref(),
        request.early_redeem_fee_rate.as_ref(),
    )
}

fn normalized_product_fee_config(
    redemption_fee_rate: Option<&BigDecimal>,
    maturity_profit_fee_rate: Option<&BigDecimal>,
    early_redeem_fee_basis: Option<&str>,
    early_redeem_fee_rate: Option<&BigDecimal>,
) -> AppResult<EarnProductFeeConfig> {
    let redemption_fee_rate = redemption_fee_rate
        .cloned()
        .unwrap_or_else(|| BigDecimal::from(0));
    let maturity_profit_fee_rate = maturity_profit_fee_rate
        .cloned()
        .unwrap_or_else(|| BigDecimal::from(0));
    let early_redeem_fee_basis = normalized_early_redeem_fee_basis(early_redeem_fee_basis)?;
    let early_redeem_fee_rate = if early_redeem_fee_basis == EARLY_REDEEM_FEE_BASIS_NONE {
        BigDecimal::from(0)
    } else {
        early_redeem_fee_rate
            .cloned()
            .unwrap_or_else(|| BigDecimal::from(0))
    };

    validate_fee_rate(&redemption_fee_rate, "earn product redemption_fee_rate")?;
    validate_fee_rate(
        &maturity_profit_fee_rate,
        "earn product maturity_profit_fee_rate",
    )?;
    validate_fee_rate(&early_redeem_fee_rate, "earn product early_redeem_fee_rate")?;

    Ok(EarnProductFeeConfig {
        redemption_fee_rate,
        maturity_profit_fee_rate,
        early_redeem_fee_basis,
        early_redeem_fee_rate,
    })
}

fn normalized_early_redeem_fee_basis(value: Option<&str>) -> AppResult<String> {
    let basis = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(EARLY_REDEEM_FEE_BASIS_NONE);
    match basis {
        EARLY_REDEEM_FEE_BASIS_NONE
        | EARLY_REDEEM_FEE_BASIS_PRINCIPAL
        | EARLY_REDEEM_FEE_BASIS_PROFIT => Ok(basis.to_owned()),
        _ => Err(AppError::Validation(
            "earn product early_redeem_fee_basis must be none, principal, or profit".to_owned(),
        )),
    }
}

fn validate_optional_reason(reason: Option<&str>) -> AppResult<()> {
    if let Some(reason) = reason
        && reason.trim().chars().count() > EARN_AUDIT_REASON_MAX_LEN
    {
        return Err(AppError::Validation(
            "earn product reason is too long".to_owned(),
        ));
    }
    Ok(())
}

/// 裁剪并校验管理员操作原因，空值或超长内容返回验证错误。
pub(crate) fn required_reason(reason: Option<String>) -> AppResult<String> {
    let Some(reason) = optional_string(reason) else {
        return Err(AppError::Validation(
            "earn product reason is required".to_owned(),
        ));
    };
    validate_optional_reason(Some(reason.as_str()))?;
    Ok(reason)
}

/// 将产品状态规范为 active 或 disabled，拒绝其他值。
pub(crate) fn normalized_product_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "earn product status is required".to_owned(),
        ));
    };
    match status.as_str() {
        "active" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "earn product status must be active or disabled".to_owned(),
        )),
    }
}

/// 将理财分类状态规范为 active 或 disabled。
pub(crate) fn normalized_category_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "earn product category status is required".to_owned(),
        ));
    };
    match status.as_str() {
        "active" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "earn product category status must be active or disabled".to_owned(),
        )),
    }
}

/// 裁剪并校验必填分类代码，仅允许受控字符和长度。
pub(crate) fn normalized_required_category_code(value: &str) -> AppResult<String> {
    let Some(code) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "earn product category code is required".to_owned(),
        ));
    };
    validate_category_code(&code, "earn product category code")?;
    Ok(code)
}

/// 规范产品分类代码，未填写时兼容回退到 fixed_term。
pub(crate) fn normalized_product_category(value: Option<&str>) -> AppResult<String> {
    let Some(category) = value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .or_else(|| Some("fixed_term".to_owned()))
    else {
        unreachable!("default earn product category is always present");
    };
    validate_category_code(&category, "earn product category")?;
    Ok(category)
}

fn validate_category_code(value: &str, label: &str) -> AppResult<()> {
    if value.chars().count() > EARN_PRODUCT_CATEGORY_MAX_LEN {
        return Err(AppError::Validation(format!("{label} is too long")));
    }
    if !value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_' || character == '-')
    {
        return Err(AppError::Validation(format!(
            "{label} supports only letters, numbers, underscore, and hyphen"
        )));
    }
    Ok(())
}

fn default_category_name_json(code: &str) -> Value {
    json!({
        "version": 1,
        "default_locale": "zh-CN",
        "items": [
            {
                "locale": "zh-CN",
                "country": "CN",
                "title": code
            }
        ]
    })
}

/// 补充默认分类多语言结构，并校验版本、默认语言和标题。
pub(crate) fn normalized_category_name_json(value: Option<Value>, code: &str) -> AppResult<Value> {
    let name_json = value.unwrap_or_else(|| default_category_name_json(code));
    validate_category_name_json(&name_json)?;
    Ok(name_json)
}

fn validate_category_name_json(value: &Value) -> AppResult<()> {
    let object = value.as_object().ok_or_else(|| {
        AppError::Validation("earn product category name_json must be an object".to_owned())
    })?;
    if object.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(AppError::Validation(
            "earn product category name_json version must be 1".to_owned(),
        ));
    }
    let default_locale = object
        .get("default_locale")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::Validation(
                "earn product category name_json default_locale is required".to_owned(),
            )
        })?;
    let items = object
        .get("items")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or_else(|| {
            AppError::Validation("earn product category name_json items are required".to_owned())
        })?;
    let mut has_default_locale = false;
    for item in items {
        let item_object = item.as_object().ok_or_else(|| {
            AppError::Validation(
                "earn product category name_json item must be an object".to_owned(),
            )
        })?;
        let locale = required_category_name_string(item_object.get("locale"), "locale")?;
        if locale == default_locale {
            has_default_locale = true;
        }
        required_category_name_string(item_object.get("country"), "country")?;
        let title = required_category_name_string(item_object.get("title"), "title")?;
        if title.chars().count() > EARN_CATEGORY_TITLE_MAX_LEN {
            return Err(AppError::Validation(
                "earn product category name_json title is too long".to_owned(),
            ));
        }
    }
    if !has_default_locale {
        return Err(AppError::Validation(
            "earn product category name_json default_locale must exist in items".to_owned(),
        ));
    }
    Ok(())
}

fn required_category_name_string<'a>(value: Option<&'a Value>, field: &str) -> AppResult<&'a str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::Validation(format!(
                "earn product category name_json {field} is required"
            ))
        })
}

fn default_introduction_json(product_name: &str) -> Value {
    json!({
        "version": 1,
        "default_locale": "zh-CN",
        "items": [
            {
                "locale": "zh-CN",
                "country": "CN",
                "title": product_name,
                "content": [
                    { "type": "p", "children": [{ "text": product_name }] }
                ]
            }
        ]
    })
}

/// 补充默认产品介绍结构，并校验受控富文本节点与多语言条目。
/// 无效节点整体拒绝，避免未支持结构进入存储和前台渲染。
pub(crate) fn normalized_introduction_json(
    value: Option<Value>,
    product_name: &str,
) -> AppResult<Value> {
    let introduction = value.unwrap_or_else(|| default_introduction_json(product_name));
    validate_introduction_json(&introduction)?;
    Ok(introduction)
}

fn validate_introduction_json(value: &Value) -> AppResult<()> {
    let object = value.as_object().ok_or_else(|| {
        AppError::Validation("earn product introduction must be an object".to_owned())
    })?;
    if object.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(AppError::Validation(
            "earn product introduction version must be 1".to_owned(),
        ));
    }
    let default_locale = object
        .get("default_locale")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::Validation("earn product introduction default_locale is required".to_owned())
        })?;
    let items = object
        .get("items")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or_else(|| {
            AppError::Validation("earn product introduction items are required".to_owned())
        })?;
    let mut has_default_locale = false;
    for item in items {
        let item_object = item.as_object().ok_or_else(|| {
            AppError::Validation("earn product introduction item must be an object".to_owned())
        })?;
        let locale = required_intro_string(item_object.get("locale"), "locale")?;
        if locale == default_locale {
            has_default_locale = true;
        }
        required_intro_string(item_object.get("country"), "country")?;
        let title = required_intro_string(item_object.get("title"), "title")?;
        if title.chars().count() > EARN_INTRO_TITLE_MAX_LEN {
            return Err(AppError::Validation(
                "earn product introduction title is too long".to_owned(),
            ));
        }
        let content = item_object
            .get("content")
            .and_then(Value::as_array)
            .filter(|content| !content.is_empty())
            .ok_or_else(|| {
                AppError::Validation("earn product introduction content is required".to_owned())
            })?;
        validate_plate_content(content)?;
    }
    if !has_default_locale {
        return Err(AppError::Validation(
            "earn product introduction default_locale must exist in items".to_owned(),
        ));
    }
    Ok(())
}

fn validate_plate_content(content: &[Value]) -> AppResult<()> {
    for node in content {
        validate_plate_block_node(node)?;
    }
    Ok(())
}

fn validate_plate_block_node(node: &Value) -> AppResult<()> {
    let object = node.as_object().ok_or_else(invalid_plate_content)?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "type" | "children"))
    {
        return Err(invalid_plate_content());
    }
    let node_type = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(invalid_plate_content)?;
    if !matches!(node_type, "p" | "h1" | "h2" | "h3" | "blockquote") {
        return Err(invalid_plate_content());
    }
    let children = object
        .get("children")
        .and_then(Value::as_array)
        .filter(|children| !children.is_empty())
        .ok_or_else(invalid_plate_content)?;
    for child in children {
        validate_plate_child_node(child)?;
    }
    Ok(())
}

fn validate_plate_child_node(node: &Value) -> AppResult<()> {
    let object = node.as_object().ok_or_else(invalid_plate_content)?;
    if !object.get("text").is_some_and(Value::is_string) {
        return Err(invalid_plate_content());
    }
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "text" | "bold" | "italic" | "underline"))
    {
        return Err(invalid_plate_content());
    }
    for mark in ["bold", "italic", "underline"] {
        if let Some(value) = object.get(mark)
            && !value.is_boolean()
        {
            return Err(invalid_plate_content());
        }
    }
    Ok(())
}

fn invalid_plate_content() -> AppError {
    AppError::Validation("earn product introduction content node is invalid".to_owned())
}

fn required_intro_string<'a>(value: Option<&'a Value>, field: &str) -> AppResult<&'a str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::Validation(format!("earn product introduction {field} is required"))
        })
}

/// 校验申购金额处于已锁产品的最小值和可选最大值范围。
/// 该规则在订阅插入和 available 扣款前执行；不做资产 precision_scale 截断。
pub(crate) fn validate_product_amount(
    amount: &BigDecimal,
    product: &EarnProductRuleRow,
) -> AppResult<()> {
    if amount < &product.min_subscribe {
        return Err(AppError::Validation(
            "earn subscription amount is below product minimum".to_owned(),
        ));
    }
    if let Some(max_subscribe) = &product.max_subscribe
        && amount > max_subscribe
    {
        return Err(AppError::Validation(
            "earn subscription amount exceeds product maximum".to_owned(),
        ));
    }
    Ok(())
}

const EARN_PRODUCT_MAX_TERM_DAYS: u32 = 3_650;
const EARN_PRODUCT_NAME_MAX_LEN: usize = 128;
const EARN_PRODUCT_CATEGORY_MAX_LEN: usize = 64;
const EARN_CATEGORY_TITLE_MAX_LEN: usize = 128;
const EARN_INTRO_TITLE_MAX_LEN: usize = 128;
const EARN_AUDIT_REASON_MAX_LEN: usize = 512;
const EARN_APR_MAX_SCALE: i64 = 8;
const EARN_APR_MAX_INTEGER_DIGITS: usize = 10;
const EARN_FEE_RATE_MAX_SCALE: i64 = 8;
const EARN_FEE_RATE_MAX_INTEGER_DIGITS: usize = 1;
const EARN_AMOUNT_MAX_SCALE: i64 = 18;
const EARN_AMOUNT_MAX_INTEGER_DIGITS: usize = 20;

/// 以调用时 UTC 当前时间加 term_days 计算订阅到期时刻；日期溢出时拒绝创建订阅。
pub(crate) fn earn_matures_at(term_days: u32) -> AppResult<DateTime<Utc>> {
    Utc::now()
        .checked_add_signed(chrono::TimeDelta::days(term_days as i64))
        .ok_or_else(|| {
            AppError::Validation("earn product term_days exceeds supported maximum".to_owned())
        })
}

fn validate_term_days(term_days: u32) -> AppResult<()> {
    if term_days == 0 {
        return Err(AppError::Validation(
            "earn product term_days must be positive".to_owned(),
        ));
    }
    if term_days > EARN_PRODUCT_MAX_TERM_DAYS {
        return Err(AppError::Validation(
            "earn product term_days exceeds supported maximum".to_owned(),
        ));
    }
    Ok(())
}

fn validate_apr_rate(apr_rate: &BigDecimal) -> AppResult<()> {
    if apr_rate < &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "earn product apr_rate must be non-negative".to_owned(),
        ));
    }
    validate_decimal_storage(
        apr_rate,
        EARN_APR_MAX_SCALE,
        EARN_APR_MAX_INTEGER_DIGITS,
        "earn product apr_rate",
    )
}

fn validate_fee_rate(fee_rate: &BigDecimal, label: &str) -> AppResult<()> {
    if fee_rate < &BigDecimal::from(0) {
        return Err(AppError::Validation(format!(
            "{label} must be non-negative"
        )));
    }
    if fee_rate > &BigDecimal::from(1) {
        return Err(AppError::Validation(format!(
            "{label} must be less than or equal to 1"
        )));
    }
    validate_decimal_storage(
        fee_rate,
        EARN_FEE_RATE_MAX_SCALE,
        EARN_FEE_RATE_MAX_INTEGER_DIGITS,
        label,
    )
}

/// 校验理财申购金额为正、最多 18 位小数且整数部分不超过 20 位。
/// 这是数据库存储口径，不读取资产 precision_scale，也不对输入金额隐式截断。
pub(crate) fn validate_amount(amount: &BigDecimal) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "earn subscription amount must be positive".to_owned(),
        ));
    }

    validate_decimal_storage(
        amount,
        EARN_AMOUNT_MAX_SCALE,
        EARN_AMOUNT_MAX_INTEGER_DIGITS,
        "earn subscription amount",
    )
}

fn validate_decimal_storage(
    value: &BigDecimal,
    max_scale: i64,
    max_integer_digits: usize,
    label: &str,
) -> AppResult<()> {
    let (digits, scale) = value.as_bigint_and_exponent();
    if scale > max_scale {
        return Err(AppError::Validation(format!(
            "{label} supports at most {max_scale} decimal places"
        )));
    }

    let significant_digits = digits
        .to_str_radix(10)
        .trim_start_matches('-')
        .trim_start_matches('0')
        .len();
    let integer_digits = if scale >= 0 {
        significant_digits.saturating_sub(scale as usize)
    } else {
        significant_digits.saturating_add(scale.unsigned_abs() as usize)
    };
    if integer_digits > max_integer_digits {
        return Err(AppError::Validation(format!(
            "{label} exceeds decimal storage precision"
        )));
    }

    Ok(())
}

/// 裁剪并校验理财申购幂等键，空值或超过存储长度时拒绝请求。
/// 规范后的键用于用户级唯一约束，重放时必须继续核对产品和申购金额。
pub(crate) fn normalize_idempotency_key(value: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "idempotency_key is required for earn subscriptions".to_owned(),
        ));
    }
    if trimmed.len() > 255 {
        return Err(AppError::Validation(
            "idempotency_key is too long for earn subscriptions".to_owned(),
        ));
    }
    Ok(trimmed.to_owned())
}

/// 裁剪可选图片地址并限制长度，空白地址归一为空值。
pub(crate) fn optional_image_url(value: Option<String>, field: &str) -> AppResult<Option<String>> {
    let Some(url) = optional_string(value) else {
        return Ok(None);
    };
    if url.chars().count() > 2048 {
        return Err(AppError::Validation(format!("{field} is too long")));
    }
    Ok(Some(url))
}

/// 仅使用订阅快照计算赎回：到期按 `本金*APR*term_days/365`，提前赎回按实际秒数计毛收益。
/// 通用赎回费按本金+毛收益计；到期收益费只在到期后按毛收益计；提前费按配置对本金或毛收益计。
/// 各中间费用与收益统一保留 18 位，净到账最低为零；产品后续修改不影响结果，本函数不写钱包。
pub(crate) fn redemption_amounts_for_subscription(
    subscription: &EarnSubscriptionResponse,
    now: DateTime<Utc>,
) -> EarnRedemptionAmounts {
    calculate_earn_redemption_amounts(
        EarnRedemptionTerms {
            amount: &subscription.amount,
            apr_rate: &subscription.apr_rate,
            term_days: subscription.term_days,
            subscribed_at: subscription.subscribed_at,
            matures_at: subscription.matures_at,
            redemption_fee_rate: &subscription.redemption_fee_rate,
            maturity_profit_fee_rate: &subscription.maturity_profit_fee_rate,
            early_redeem_fee_basis: &subscription.early_redeem_fee_basis,
            early_redeem_fee_rate: &subscription.early_redeem_fee_rate,
        },
        now,
    )
}

/// 验证幂等键对应订阅的产品和金额与本次请求完全一致。
/// 不一致时返回冲突，防止复用同一键绕过申购资金语义。
pub(crate) fn ensure_existing_subscription_matches_request(
    existing: &EarnSubscriptionResponse,
    product_id: u64,
    amount: &BigDecimal,
) -> AppResult<()> {
    if existing.product_id != product_id || existing.amount != *amount {
        return Err(AppError::Conflict(
            "earn idempotency key belongs to a different request".to_owned(),
        ));
    }
    Ok(())
}
