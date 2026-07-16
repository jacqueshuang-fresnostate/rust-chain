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
    architecture::ServiceLayer,
    error::{AppError, AppResult},
    infra::secrets::{decrypt_secret, encrypt_secret_field},
};
use bigdecimal::BigDecimal;
use md5::{Digest, Md5};
use serde_json::{Map, Value, json};
use std::{collections::BTreeMap, str::FromStr};
use url::Url;

#[derive(Debug)]
pub struct ServiceLayerMarker;

impl ServiceLayer for ServiceLayerMarker {}

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
    if min_amount <= BigDecimal::from(0) {
        return Err(AppError::Validation(
            "quick recharge min_amount must be positive".to_owned(),
        ));
    }
    let max_amount = request.max_amount.clone();
    if let Some(max_amount) = max_amount.as_ref() {
        if max_amount < &min_amount {
            return Err(AppError::Validation(
                "quick recharge max_amount must be greater than or equal to min_amount".to_owned(),
            ));
        }
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

pub(crate) fn validate_recharge_amount(
    amount: &BigDecimal,
    config: &QuickRechargeRuntimeConfig,
) -> AppResult<()> {
    if amount < &config.min_amount {
        return Err(AppError::Validation(
            "quick recharge amount is below min_amount".to_owned(),
        ));
    }
    if let Some(max_amount) = config.max_amount.as_ref() {
        if amount > max_amount {
            return Err(AppError::Validation(
                "quick recharge amount is above max_amount".to_owned(),
            ));
        }
    }
    Ok(())
}

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

pub fn gmpay_signature(params: &BTreeMap<String, String>, secret: &str) -> String {
    let sign_source = params
        .iter()
        .filter(|(key, value)| key.as_str() != "signature" && !value.trim().is_empty())
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    md5_lower_hex(&format!("{sign_source}{secret}"))
}

pub(crate) fn required_json_string(object: &Map<String, Value>, field: &str) -> AppResult<String> {
    object
        .get(field)
        .and_then(json_value_to_sign_string)
        .ok_or_else(|| AppError::Validation(format!("gmpay notify {field} is required")))
}

pub(crate) fn optional_json_string(object: &Map<String, Value>, field: &str) -> Option<String> {
    object.get(field).and_then(json_value_to_sign_string)
}

pub(crate) fn required_json_decimal(
    object: &Map<String, Value>,
    field: &str,
) -> AppResult<BigDecimal> {
    let value = required_json_string(object, field)?;
    BigDecimal::from_str(&value)
        .map_err(|_| AppError::Validation(format!("gmpay notify {field} is invalid")))
}

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

pub(crate) fn validate_order_status(value: &str) -> AppResult<String> {
    let status = value.trim();
    match status {
        "created" | "pending" | "paid" | "failed" | "expired" => Ok(status.to_owned()),
        _ => Err(AppError::Validation(
            "quick recharge status is invalid".to_owned(),
        )),
    }
}

pub(crate) fn optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|value| optional_str(&value).map(str::to_owned))
}

pub(crate) fn optional_str(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

pub(crate) fn required_reason(value: Option<String>) -> AppResult<String> {
    let Some(reason) = optional_string(value) else {
        return Err(AppError::Validation("reason is required".to_owned()));
    };
    if reason.len() > 512 {
        return Err(AppError::Validation("reason is too long".to_owned()));
    }
    Ok(reason)
}

pub(crate) fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

pub(crate) fn admin_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("admin:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 200)
}

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
