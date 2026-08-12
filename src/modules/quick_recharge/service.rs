//! quick_recharge bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务服务逐步迁入。

use super::{
    presentation::{
        QuickRechargeReturnTarget, SaveQuickRechargeConfigRequest, TestQuickRechargeConfigResponse,
    },
    repository::QuickRechargeConfigRow,
};
use crate::{
    error::{AppError, AppResult},
    infra::secrets::{decrypt_secret, encrypt_secret_field},
};
use bigdecimal::BigDecimal;
use md5::{Digest, Md5};
use serde_json::{Map, Value, json};
use std::{collections::BTreeMap, str::FromStr};
use url::Url;

#[derive(Debug, Clone)]
pub(crate) struct QuickRechargeRuntimeConfig {
    pub(crate) api_base_url: String,
    pub(crate) merchant_pid: String,
    pub(crate) merchant_secret: String,
    pub(crate) currency: String,
    pub(crate) token: String,
    pub(crate) network: String,
    pub(crate) notify_url: String,
    pub(crate) redirect_url: Option<String>,
    pub(crate) pc_app_redirect_url: Option<String>,
    pub(crate) mac_app_redirect_url: Option<String>,
    pub(crate) ios_app_redirect_url: Option<String>,
    pub(crate) android_app_redirect_url: Option<String>,
    pub(crate) mobile_web_redirect_url: Option<String>,
    pub(crate) desktop_web_redirect_url: Option<String>,
    pub(crate) min_amount: BigDecimal,
    pub(crate) max_amount: Option<BigDecimal>,
}

#[derive(Debug)]
pub(crate) struct ValidatedQuickRechargeConfig {
    pub(crate) enabled: bool,
    pub(crate) api_base_url: Option<String>,
    pub(crate) merchant_pid: Option<String>,
    pub(crate) currency: String,
    pub(crate) token: String,
    pub(crate) network: String,
    pub(crate) notify_url: Option<String>,
    pub(crate) redirect_url: Option<String>,
    pub(crate) pc_app_redirect_url: Option<String>,
    pub(crate) mac_app_redirect_url: Option<String>,
    pub(crate) ios_app_redirect_url: Option<String>,
    pub(crate) android_app_redirect_url: Option<String>,
    pub(crate) mobile_web_redirect_url: Option<String>,
    pub(crate) desktop_web_redirect_url: Option<String>,
    pub(crate) min_amount: BigDecimal,
    pub(crate) max_amount: Option<BigDecimal>,
}

/// 把持久化配置解密并转换为支付方运行时配置，按需要求配置启用。
/// 密钥缺失、解密失败或启用字段不完整时在任何外部请求前返回错误。
pub(crate) fn runtime_config_from_row(
    row: QuickRechargeConfigRow,
    key: Option<&str>,
    require_enabled: bool,
) -> AppResult<QuickRechargeRuntimeConfig> {
    if require_enabled && !row.enabled {
        return Err(AppError::Validation(
            "quick recharge is not enabled".to_owned(),
        ));
    }
    let api_base_url = row.api_base_url.ok_or_else(|| {
        AppError::Validation("quick recharge api_base_url is not configured".to_owned())
    })?;
    let merchant_pid = row.merchant_pid.ok_or_else(|| {
        AppError::Validation("quick recharge merchant_pid is not configured".to_owned())
    })?;
    let secret_ciphertext = row.merchant_secret_ciphertext.ok_or_else(|| {
        AppError::Validation("quick recharge merchant_secret is not configured".to_owned())
    })?;
    let key = key.ok_or_else(|| {
        AppError::Internal("credential encryption key is not configured".to_owned())
    })?;
    let notify_url = row.notify_url.ok_or_else(|| {
        AppError::Validation("quick recharge notify_url is not configured".to_owned())
    })?;
    Ok(QuickRechargeRuntimeConfig {
        api_base_url,
        merchant_pid,
        merchant_secret: decrypt_secret(&secret_ciphertext, key)?,
        currency: row.currency,
        token: row.token,
        network: row.network,
        notify_url,
        redirect_url: row.redirect_url,
        pc_app_redirect_url: row.pc_app_redirect_url,
        mac_app_redirect_url: row.mac_app_redirect_url,
        ios_app_redirect_url: row.ios_app_redirect_url,
        android_app_redirect_url: row.android_app_redirect_url,
        mobile_web_redirect_url: row.mobile_web_redirect_url,
        desktop_web_redirect_url: row.desktop_web_redirect_url,
        min_amount: row.min_amount,
        max_amount: row.max_amount,
    })
}

/// 规范并校验快充地址、币种、网络和金额上下限，生成可保存配置。
/// min_amount 必须为正、max_amount 不得小于 min；当前规则不限制法币金额小数位或整数位。
/// 该纯校验不加密密钥、不访问支付方，也不修改当前配置。
pub(crate) fn validate_save_config_request(
    request: &SaveQuickRechargeConfigRequest,
) -> AppResult<ValidatedQuickRechargeConfig> {
    let api_base_url = validate_optional_url(request.api_base_url.clone(), "api_base_url")?;
    let notify_url = validate_optional_url(request.notify_url.clone(), "notify_url")?;
    let redirect_url = validate_optional_url(request.redirect_url.clone(), "redirect_url")?;
    let pc_app_redirect_url =
        validate_optional_return_url(request.pc_app_redirect_url.clone(), "pc_app_redirect_url")?;
    let mac_app_redirect_url =
        validate_optional_return_url(request.mac_app_redirect_url.clone(), "mac_app_redirect_url")?;
    let ios_app_redirect_url =
        validate_optional_return_url(request.ios_app_redirect_url.clone(), "ios_app_redirect_url")?;
    let android_app_redirect_url = validate_optional_return_url(
        request.android_app_redirect_url.clone(),
        "android_app_redirect_url",
    )?;
    let mobile_web_redirect_url = validate_optional_url(
        request.mobile_web_redirect_url.clone(),
        "mobile_web_redirect_url",
    )?;
    let desktop_web_redirect_url = validate_optional_url(
        request.desktop_web_redirect_url.clone(),
        "desktop_web_redirect_url",
    )?;
    let merchant_pid = validate_optional_ascii(
        request.merchant_pid.clone(),
        "merchant_pid",
        128,
        false,
        true,
    )?;
    let currency = validate_symbol_like(&request.currency, "currency", 16, false)?;
    let token = validate_symbol_like(&request.token, "token", 32, false)?;
    let network = validate_symbol_like(&request.network, "network", 32, true)?;
    let min_amount = request.min_amount.clone();
    if min_amount <= 0 {
        return Err(AppError::Validation(
            "quick recharge min_amount must be positive".to_owned(),
        ));
    }
    let max_amount = request.max_amount.clone();
    if let Some(max_amount) = max_amount.as_ref()
        && max_amount < &min_amount
    {
        return Err(AppError::Validation(
            "quick recharge max_amount must be greater than or equal to min_amount".to_owned(),
        ));
    }
    let config = ValidatedQuickRechargeConfig {
        enabled: request.enabled,
        api_base_url,
        merchant_pid,
        currency,
        token,
        network,
        notify_url,
        redirect_url,
        pc_app_redirect_url,
        mac_app_redirect_url,
        ios_app_redirect_url,
        android_app_redirect_url,
        mobile_web_redirect_url,
        desktop_web_redirect_url,
        min_amount,
        max_amount,
    };
    if config.enabled {
        require_config_field(config.api_base_url.as_deref(), "api_base_url")?;
        require_config_field(config.merchant_pid.as_deref(), "merchant_pid")?;
        require_config_field(config.notify_url.as_deref(), "notify_url")?;
    }
    Ok(config)
}

/// 启用配置必须已持有商户密钥密文；这里只检查存在性，不解密或验证密钥能被支付方接受。
pub(crate) fn validate_enabled_config_secrets(
    config: &ValidatedQuickRechargeConfig,
    secret_ciphertext: &Option<String>,
) -> AppResult<()> {
    if config.enabled && secret_ciphertext.is_none() {
        return Err(AppError::Validation(
            "quick recharge merchant_secret is required when enabled".to_owned(),
        ));
    }
    Ok(())
}

/// 校验快充金额为正且处于配置的最小值和可选最大值之间。
/// 由于配置 min_amount 必须为正，低于最小值即涵盖非正输入；当前规则不按法币或到账资产精度截断。
/// 校验在创建本地订单和支付方请求前完成，失败时不产生本地订单或外部副作用。
pub(crate) fn validate_recharge_amount(
    amount: &BigDecimal,
    config: &QuickRechargeRuntimeConfig,
) -> AppResult<()> {
    if amount < &config.min_amount {
        return Err(AppError::Validation(
            "quick recharge amount is below min_amount".to_owned(),
        ));
    }
    if let Some(max_amount) = config.max_amount.as_ref()
        && amount > max_amount
    {
        return Err(AppError::Validation(
            "quick recharge amount is above max_amount".to_owned(),
        ));
    }
    Ok(())
}

/// 按客户端目标选择专用回跳地址，缺失时回退到默认地址。
/// 选择只基于已验证配置，不拼接用户输入，也不修改订单或支付方状态。
pub(crate) fn redirect_url_for_target(
    config: &QuickRechargeRuntimeConfig,
    target: Option<QuickRechargeReturnTarget>,
) -> Option<String> {
    let target_url = target.and_then(|target| match target {
        QuickRechargeReturnTarget::PcApp => config.pc_app_redirect_url.clone(),
        QuickRechargeReturnTarget::MacApp => config.mac_app_redirect_url.clone(),
        QuickRechargeReturnTarget::IosApp => config.ios_app_redirect_url.clone(),
        QuickRechargeReturnTarget::AndroidApp => config.android_app_redirect_url.clone(),
        QuickRechargeReturnTarget::MobileWeb => config.mobile_web_redirect_url.clone(),
        QuickRechargeReturnTarget::DesktopWeb => config.desktop_web_redirect_url.clone(),
    });
    target_url.or_else(|| config.redirect_url.clone())
}

/// 新密钥非空时加密保存，空白时沿用旧密文；缺少加密键时返回配置错误。
/// 函数只返回待保存密文，不写数据库；配置事务失败时旧密钥仍保持生效。
pub(crate) fn prepare_secret_field(
    new_value: Option<&str>,
    existing_ciphertext: Option<String>,
    key: Option<&str>,
) -> AppResult<Option<String>> {
    if new_value.and_then(optional_str).is_some() {
        let key = key.ok_or_else(|| {
            AppError::Internal("credential encryption key is not configured".to_owned())
        })?;
        return encrypt_secret_field(key, new_value, existing_ciphertext);
    }
    Ok(existing_ciphertext)
}

/// 生成不含密钥明文的快充配置审计快照，仅保留掩码。
/// 前后快照用于配置事务审计，密文和解密值均不得进入审计 JSON。
pub(crate) fn config_audit_json(row: &QuickRechargeConfigRow) -> Value {
    json!({
        "id": row.id,
        "name": row.name,
        "provider": row.provider,
        "enabled": row.enabled,
        "api_base_url": row.api_base_url,
        "merchant_pid": row.merchant_pid,
        "merchant_secret_mask": row.merchant_secret_mask,
        "merchant_secret_set": row.merchant_secret_ciphertext.is_some(),
        "currency": row.currency,
        "token": row.token,
        "network": row.network,
        "notify_url": row.notify_url,
        "redirect_url": row.redirect_url,
        "pc_app_redirect_url": row.pc_app_redirect_url,
        "mac_app_redirect_url": row.mac_app_redirect_url,
        "ios_app_redirect_url": row.ios_app_redirect_url,
        "android_app_redirect_url": row.android_app_redirect_url,
        "mobile_web_redirect_url": row.mobile_web_redirect_url,
        "desktop_web_redirect_url": row.desktop_web_redirect_url,
        "min_amount": decimal_to_gmpay_string(&row.min_amount),
        "max_amount": row.max_amount.as_ref().map(decimal_to_gmpay_string),
        "updated_by": row.updated_by,
    })
}

/// 生成支付方连通性测试结果的管理员审计快照。
/// 快照只记录订单标识、币种和支付地址，不包含商户密钥或签名材料。
pub(crate) fn test_config_audit_json(response: &TestQuickRechargeConfigResponse) -> Value {
    json!({
        "order_id": response.order_id,
        "provider_trade_id": response.provider_trade_id,
        "currency": response.currency,
        "token": response.token,
        "network": response.network,
        "fiat_amount": decimal_to_gmpay_string(&response.fiat_amount),
        "actual_amount": decimal_to_gmpay_string(&response.actual_amount),
        "receive_address": response.receive_address,
        "payment_url": response.payment_url,
        "expiration_time": response.expiration_time,
        "tested_at": response.tested_at,
    })
}

/// 排除 `signature` 字段后按 GMPay 规则重算 MD5，并使用不区分 ASCII 大小写的普通字符串比较。
/// 该比较不是常量时间比较；验签失败发生在锁订单之前，不修改钱包、流水或支付状态。
pub(crate) fn verify_gmpay_notify_signature(
    object: &Map<String, Value>,
    secret: &str,
) -> AppResult<()> {
    let signature = required_json_string(object, "signature")?;
    let expected = gmpay_json_signature(object, secret);
    if !signature.eq_ignore_ascii_case(&expected) {
        return Err(AppError::Validation(
            "gmpay notify signature is invalid".to_owned(),
        ));
    }
    Ok(())
}

/// 按键名排序拼接非空字段与商户密钥，计算 GMPay MD5 签名。
/// 字段过滤和文本表示必须与通知验签共用，避免请求与回调规则漂移。
pub fn gmpay_signature(params: &BTreeMap<String, String>, secret: &str) -> String {
    let sign_source = params
        .iter()
        .filter(|(key, value)| key.as_str() != "signature" && !value.trim().is_empty())
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    md5_lower_hex(&format!("{sign_source}{secret}"))
}

/// 从回调对象读取并裁剪必填字符串，缺失或空白时返回验证错误。
pub(crate) fn required_json_string(object: &Map<String, Value>, field: &str) -> AppResult<String> {
    object
        .get(field)
        .and_then(json_value_to_sign_string)
        .ok_or_else(|| AppError::Validation(format!("gmpay notify {field} is required")))
}

/// 从回调对象读取可选字符串，并把空白内容归一为空值。
pub(crate) fn optional_json_string(object: &Map<String, Value>, field: &str) -> Option<String> {
    object.get(field).and_then(json_value_to_sign_string)
}

/// 从回调对象解析必填十进制金额，拒绝缺失或非法格式。
pub(crate) fn required_json_decimal(
    object: &Map<String, Value>,
    field: &str,
) -> AppResult<BigDecimal> {
    let value = required_json_string(object, field)?;
    BigDecimal::from_str(&value)
        .map_err(|_| AppError::Validation(format!("gmpay notify {field} is invalid")))
}

/// 把十进制金额标准化为 GMPay 签名和请求使用的无多余零字符串。
/// 请求与验签必须复用该表示，避免相同金额因文本差异产生签名不一致。
pub(crate) fn decimal_to_gmpay_string(value: &BigDecimal) -> String {
    let mut text = format!("{value:.18}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text == "-0" {
        return "0".to_owned();
    }
    text
}

/// 规范快充订单状态，只接受本地状态机定义的值。
/// 非法状态在查询执行前拒绝，不会被当作空结果或触发任何订单变更。
pub(crate) fn validate_order_status(value: &str) -> AppResult<String> {
    let status = value.trim();
    match status {
        "created" | "pending" | "paid" | "failed" | "expired" => Ok(status.to_owned()),
        _ => Err(AppError::Validation(
            "quick recharge status is invalid".to_owned(),
        )),
    }
}

/// 裁剪拥有所有权的可选字符串，并过滤空白值。
pub(crate) fn optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|value| optional_str(&value).map(str::to_owned))
}

/// 裁剪借用字符串，并把空白内容归一为空值。
pub(crate) fn optional_str(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

/// 裁剪管理员操作原因，空值时拒绝配置或删除操作。
pub(crate) fn required_reason(value: Option<String>) -> AppResult<String> {
    let Some(reason) = optional_string(value) else {
        return Err(AppError::Validation("reason is required".to_owned()));
    };
    if reason.len() > 512 {
        return Err(AppError::Validation("reason is too long".to_owned()));
    }
    Ok(reason)
}

/// 从用户 subject 提取可信编号，格式不符时返回未授权。
pub(crate) fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 从管理员 subject 提取可信编号，格式不符时返回未授权。
pub(crate) fn admin_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("admin:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 把快充列表数量限制在一到一百条。
pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 200)
}

/// 偏移同样设上限：超大 offset 会让日志类大表退化为全表扫描加文件排序。
pub(crate) fn route_offset(offset: Option<u32>) -> u32 {
    offset.unwrap_or(0).min(100_000)
}

/// 校验可选回跳地址必须是 HTTP 或 HTTPS，空白值归一为空。
/// 非法地址在配置保存前拒绝，避免支付方把用户重定向到非预期协议。
pub(crate) fn validate_optional_return_url(
    value: Option<String>,
    field: &str,
) -> AppResult<Option<String>> {
    let Some(url) = optional_string(value) else {
        return Ok(None);
    };
    let parsed = Url::parse(&url)
        .map_err(|_| AppError::Validation(format!("quick recharge {field} is invalid")))?;
    match parsed.scheme() {
        "http" | "https" => Ok(Some(url)),
        "javascript" | "data" | "file" | "about" => Err(AppError::Validation(format!(
            "quick recharge {field} uses an unsupported scheme"
        ))),
        scheme if !scheme.is_empty() => Ok(Some(url)),
        _ => Err(AppError::Validation(format!(
            "quick recharge {field} requires a url scheme"
        ))),
    }
}

fn validate_optional_url(value: Option<String>, field: &str) -> AppResult<Option<String>> {
    let Some(url) = optional_string(value) else {
        return Ok(None);
    };
    let parsed = Url::parse(&url)
        .map_err(|_| AppError::Validation(format!("quick recharge {field} is invalid")))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::Validation(format!(
            "quick recharge {field} must be http or https"
        )));
    }
    Ok(Some(url))
}

fn validate_optional_ascii(
    value: Option<String>,
    field: &str,
    max_len: usize,
    allow_dash: bool,
    allow_underscore: bool,
) -> AppResult<Option<String>> {
    optional_string(value)
        .map(|value| validate_ascii_token(&value, field, max_len, allow_dash, allow_underscore))
        .transpose()
}

fn validate_symbol_like(
    value: &str,
    field: &str,
    max_len: usize,
    allow_dash: bool,
) -> AppResult<String> {
    let normalized = validate_ascii_token(value, field, max_len, allow_dash, true)?;
    Ok(normalized.to_ascii_lowercase())
}

fn validate_ascii_token(
    value: &str,
    field: &str,
    max_len: usize,
    allow_dash: bool,
    allow_underscore: bool,
) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::Validation(format!(
            "quick recharge {field} is required"
        )));
    }
    if value.len() > max_len {
        return Err(AppError::Validation(format!(
            "quick recharge {field} is too long"
        )));
    }
    let valid = value.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || (allow_dash && ch == '-') || (allow_underscore && ch == '_')
    });
    if !valid {
        return Err(AppError::Validation(format!(
            "quick recharge {field} format is invalid"
        )));
    }
    Ok(value.to_owned())
}

fn require_config_field(value: Option<&str>, field: &str) -> AppResult<()> {
    if value.and_then(optional_str).is_none() {
        return Err(AppError::Validation(format!(
            "quick recharge {field} is required when enabled"
        )));
    }
    Ok(())
}

fn gmpay_json_signature(object: &Map<String, Value>, secret: &str) -> String {
    let mut params = BTreeMap::new();
    for (key, value) in object {
        if key == "signature" {
            continue;
        }
        if let Some(value) = json_value_to_sign_string(value) {
            params.insert(key.clone(), value);
        }
    }
    gmpay_signature(&params, secret)
}

fn md5_lower_hex(value: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())
}

fn json_value_to_sign_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(value) => optional_str(value).map(str::to_owned),
        Value::Number(value) => Some(value.to_string()).filter(|value| !value.is_empty()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Array(_) | Value::Object(_) => None,
    }
}
