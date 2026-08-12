//! user bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务服务逐步迁入。

use crate::{
    error::{AppError, AppResult},
    modules::{
        security::ThirdPartyBindingPolicy,
        user::domain::{optional_string, required_string},
    },
};
use ring::rand::{SecureRandom, SystemRandom};

pub(crate) const EMAIL_BIND_PURPOSE: &str = "bind";
pub(crate) const TWO_FACTOR_RESET_PURPOSE: &str = "two_factor_reset";
pub(crate) const FUND_PASSWORD_RESET_PURPOSE: &str = "fund_password_reset";
pub(crate) const EMAIL_VERIFICATION_CODE_TTL_MINUTES: u32 = 10;
pub(crate) const EMAIL_VERIFICATION_CODE_COOLDOWN_SECONDS: i64 = 60;
pub(crate) const USER_INVITE_CODE_LENGTH: usize = 6;
pub(crate) const USER_INVITE_CODE_CREATE_ATTEMPTS: usize = 12;
const USER_INVITE_CODE_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

/// 绑定邮箱前校验单一 `@`、非空两段、无空白且总长度不超过 255。
/// 只保留规范字符串；地址可达性由发信和后续验证码确认，本函数无 I/O 副作用。
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

/// 校验六位纯数字邮件验证码并返回规范值，格式不符时不尝试数据库比对。
pub(crate) fn validate_email_code(value: &str) -> AppResult<String> {
    let code = required_string(Some(value.to_owned()), "code")?;
    if code.len() != 6 || !code.chars().all(|char| char.is_ascii_digit()) {
        return Err(AppError::Validation("code is invalid".to_owned()));
    }
    Ok(code)
}

/// 使用系统安全随机源生成补零后的六位验证码。
/// 随机源失败时返回内部错误，调用方不得发送或持久化占位验证码。
pub(crate) fn generate_email_code() -> AppResult<String> {
    let rng = SystemRandom::new();
    let mut bytes = [0_u8; 4];
    rng.fill(&mut bytes)
        .map_err(|_| AppError::Internal("email verification code generation failed".to_owned()))?;
    let value = u32::from_be_bytes(bytes) % 1_000_000;
    Ok(format!("{value:06}"))
}

/// 校验登录密码至少包含八个字符，并保留原始字符内容供散列。
pub(crate) fn validate_login_password(value: &str, field: &str) -> AppResult<String> {
    let password = required_string(Some(value.to_owned()), field)?;
    if password.chars().count() < 8 {
        return Err(AppError::Validation(format!("{field} is too short")));
    }
    Ok(password)
}

/// 校验资金密码为恰好六位数字，拒绝空白及其他字符。
pub(crate) fn validate_fund_password(value: &str, field: &str) -> AppResult<String> {
    let password = required_string(Some(value.to_owned()), field)?;
    if password.len() != 6 || !password.chars().all(|char| char.is_ascii_digit()) {
        return Err(AppError::Validation(format!("{field} must be 6 digits")));
    }
    Ok(password)
}

/// 将第三方绑定提供方限制为平台已实现的稳定存储枚举。
pub(crate) fn normalize_third_party_provider(value: &str) -> AppResult<&'static str> {
    match value.trim() {
        "coinbase_wallet" => Ok("coinbase_wallet"),
        "telegram_account" => Ok("telegram_account"),
        _ => Err(AppError::Validation("provider is invalid".to_owned())),
    }
}

/// 按安全策略判断指定第三方绑定入口是否启用，未知提供方始终关闭。
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

/// 校验第三方账号标识长度和空白字符，Telegram 使用更严格的 64 字节上限。
/// 本函数不验证远端账号是否存在，该可达性由具体第三方授权流程负责。
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

/// 规范化可选第三方显示名，并限制最多 255 个字符。
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

/// 使用系统安全随机源生成六位大写字母数字邀请码。
/// 随机源失败时返回内部错误；唯一性冲突由应用层在限定次数内重新生成。
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

/// 判断邀请码是否满足固定长度和大写字母数字字符集约束。
pub(crate) fn is_valid_user_invite_code(code: &str) -> bool {
    code.len() == USER_INVITE_CODE_LENGTH
        && code
            .chars()
            .all(|char| char.is_ascii_uppercase() || char.is_ascii_digit())
}

/// 去除邀请码首尾空白并拒绝空值；具体存在性与使用次数由仓储校验。
pub(crate) fn normalize_invite_code(code: &str) -> AppResult<String> {
    let code = code.trim();
    if code.is_empty() {
        return Err(AppError::Validation("code is required".to_owned()));
    }
    Ok(code.to_owned())
}

/// 从 `user:<id>` 认证 subject 中提取用户 ID，主体类型或数值非法时返回未授权。
pub(crate) fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}
