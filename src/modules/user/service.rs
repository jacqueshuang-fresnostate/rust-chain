//! user bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务服务逐步迁入。

use crate::architecture::ServiceLayer;
use crate::{
    error::{AppError, AppResult},
    modules::{
        security::ThirdPartyBindingPolicy,
        user::domain::{optional_string, required_string},
    },
    state::AppState,
};
use ring::rand::{SecureRandom, SystemRandom};
use sqlx::{MySql, Pool};

#[derive(Debug)]
pub struct ServiceLayerMarker;

impl ServiceLayer for ServiceLayerMarker {}

pub(crate) const EMAIL_BIND_PURPOSE: &str = "bind";
pub(crate) const TWO_FACTOR_RESET_PURPOSE: &str = "two_factor_reset";
pub(crate) const FUND_PASSWORD_RESET_PURPOSE: &str = "fund_password_reset";
pub(crate) const EMAIL_VERIFICATION_CODE_TTL_MINUTES: u32 = 10;
pub(crate) const EMAIL_VERIFICATION_CODE_COOLDOWN_SECONDS: i64 = 60;
pub(crate) const USER_INVITE_CODE_LENGTH: usize = 6;
pub(crate) const USER_INVITE_CODE_CREATE_ATTEMPTS: usize = 12;
const USER_INVITE_CODE_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

/// 绑定邮箱前先做轻量格式校验；完整可达性由发信与后续验证码确认兜底。
pub(crate) fn validate_email(value: &str, field: &str) -> AppResult<String> {
    let email = required_string(Some(value.to_owned()), field)?;
    let mut parts = email.split('@');
    let local = parts.next().unwrap_or_default();
    let domain = parts.next().unwrap_or_default();
    if email.len() > 255
        || local.is_empty()
        || domain.is_empty()
        || parts.next().is_some()
        || email.chars().any(char::is_whitespace)
    {
        return Err(AppError::Validation(format!("{field} is invalid")));
    }
    Ok(email)
}

pub(crate) fn validate_email_code(value: &str) -> AppResult<String> {
    let code = required_string(Some(value.to_owned()), "code")?;
    if code.len() != 6 || !code.chars().all(|char| char.is_ascii_digit()) {
        return Err(AppError::Validation("code is invalid".to_owned()));
    }
    Ok(code)
}

pub(crate) fn generate_email_code() -> AppResult<String> {
    let rng = SystemRandom::new();
    let mut bytes = [0_u8; 4];
    rng.fill(&mut bytes)
        .map_err(|_| AppError::Internal("email verification code generation failed".to_owned()))?;
    let value = u32::from_be_bytes(bytes) % 1_000_000;
    Ok(format!("{value:06}"))
}

pub(crate) fn validate_login_password(value: &str, field: &str) -> AppResult<String> {
    let password = required_string(Some(value.to_owned()), field)?;
    if password.chars().count() < 8 {
        return Err(AppError::Validation(format!("{field} is too short")));
    }
    Ok(password)
}

pub(crate) fn validate_fund_password(value: &str, field: &str) -> AppResult<String> {
    let password = required_string(Some(value.to_owned()), field)?;
    if password.len() != 6 || !password.chars().all(|char| char.is_ascii_digit()) {
        return Err(AppError::Validation(format!("{field} must be 6 digits")));
    }
    Ok(password)
}

pub(crate) fn normalize_third_party_provider(value: &str) -> AppResult<&'static str> {
    match value.trim() {
        "coinbase_wallet" => Ok("coinbase_wallet"),
        "telegram_account" => Ok("telegram_account"),
        _ => Err(AppError::Validation("provider is invalid".to_owned())),
    }
}

pub(crate) fn is_third_party_binding_enabled(
    policy: &ThirdPartyBindingPolicy,
    provider: &str,
) -> bool {
    match provider {
        "coinbase_wallet" => policy.coinbase_wallet_enabled,
        "telegram_account" => policy.telegram_account_enabled,
        _ => false,
    }
}

pub(crate) fn validate_third_party_identifier(provider: &str, value: &str) -> AppResult<String> {
    let identifier = required_string(Some(value.to_owned()), "account_identifier")?;
    let max_len = if provider == "telegram_account" {
        64
    } else {
        255
    };
    if identifier.len() > max_len || identifier.chars().any(char::is_whitespace) {
        return Err(AppError::Validation(
            "account_identifier is invalid".to_owned(),
        ));
    }
    Ok(identifier)
}

pub(crate) fn normalize_third_party_display_name(
    value: Option<String>,
) -> AppResult<Option<String>> {
    let display_name = optional_string(value);
    if display_name
        .as_ref()
        .is_some_and(|value| value.chars().count() > 255)
    {
        return Err(AppError::Validation("display_name is too long".to_owned()));
    }
    Ok(display_name)
}

pub(crate) fn generate_user_invite_code() -> AppResult<String> {
    let rng = SystemRandom::new();
    let mut bytes = [0_u8; USER_INVITE_CODE_LENGTH];
    rng.fill(&mut bytes)
        .map_err(|_| AppError::Internal("invite code generation failed".to_owned()))?;

    Ok(bytes
        .iter()
        .map(|byte| {
            USER_INVITE_CODE_ALPHABET[*byte as usize % USER_INVITE_CODE_ALPHABET.len()] as char
        })
        .collect())
}

pub(crate) fn is_valid_user_invite_code(code: &str) -> bool {
    code.len() == USER_INVITE_CODE_LENGTH
        && code
            .chars()
            .all(|char| char.is_ascii_uppercase() || char.is_ascii_digit())
}

pub(crate) fn normalize_invite_code(code: &str) -> AppResult<String> {
    let code = code.trim();
    if code.is_empty() {
        return Err(AppError::Validation("code is required".to_owned()));
    }
    Ok(code.to_owned())
}

/// 统一从应用状态中获取数据库连接池，避免路由层重复拼接错误信息。
pub(crate) fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for user routes".to_owned())
    })
}

/// 从认证 subject 中提取 user ID。
pub(crate) fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}
