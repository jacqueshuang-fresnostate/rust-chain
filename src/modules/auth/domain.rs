//! auth bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。

use crate::{
    architecture::DomainLayer,
    error::{AppError, AppResult},
};

/// 连续密码失败次数达到该阈值即触发临时锁定，与邮件验证码的尝试上限保持一致。
pub(crate) const LOGIN_FAILURE_LIMIT: u32 = 5;
/// 失败计数窗口：窗口过期后计数自动归零，无需人工干预。
pub(crate) const LOGIN_FAILURE_WINDOW_SECONDS: i64 = 900;
/// 锁定时长：到期后自动解锁，避免计数卡死造成永久拒绝登录。
pub(crate) const LOGIN_LOCKOUT_SECONDS: i64 = 900;

const LOGIN_FAILURE_KEY_MAX_CHARS: usize = 191;

#[derive(Debug)]
pub struct AuthValidationRules;

impl DomainLayer for AuthValidationRules {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoginTurnstilePolicy {
    enabled: bool,
    enforce_token: bool,
}

impl LoginTurnstilePolicy {
    /// 仅在密钥和站点公钥齐备时启用 Turnstile，强制标志不单独开启校验。
    pub(crate) fn new(has_secret: bool, has_site_key: bool, enforce_token: bool) -> Self {
        Self {
            enabled: has_secret && has_site_key,
            enforce_token,
        }
    }

    /// 返回运行时配置是否完整开启 Turnstile。
    pub(crate) fn enabled(&self) -> bool {
        self.enabled
    }

    /// 根据强制策略和 `cf_clearance` 决定本次登录是否必须调用站点校验。
    pub(crate) fn requires_verification(&self, has_cf_clearance: bool) -> bool {
        self.enabled && (self.enforce_token || !has_cf_clearance)
    }
}

/// 失败计数键统一小写并截断，避免通过大小写或超长输入绕过锁定。
pub fn login_failure_key(identifier: &str) -> String {
    identifier
        .trim()
        .to_lowercase()
        .chars()
        .take(LOGIN_FAILURE_KEY_MAX_CHARS)
        .collect()
}

/// 锁定提示不区分账号是否存在，只暴露可重试时间。
pub(crate) fn login_locked_error(retry_after_seconds: i64) -> AppError {
    let minutes = (retry_after_seconds.max(0) as u64).div_ceil(60).max(1);
    AppError::security_validation(
        "login_temporarily_locked",
        format!("登录失败次数过多，请在 {minutes} 分钟后重试"),
    )
}

/// 校验邮件码为六位 ASCII 数字并返回去除空白后的值，格式错误不进入仓储。
pub(crate) fn validate_email_code(value: &str) -> AppResult<String> {
    let code = value.trim();
    if code.len() != 6 || !code.chars().all(|char| char.is_ascii_digit()) {
        return Err(AppError::Validation("code is invalid".to_owned()));
    }
    Ok(code.to_owned())
}

/// 校验注册邮箱非空、长度不超过 255 且包含 `@`，存储前统一转小写。
pub(crate) fn validate_registration_email(value: Option<String>) -> AppResult<String> {
    let email = required_string(value, "email")?;
    if email.len() > 255 || !email.contains('@') {
        return Err(AppError::Validation("email format is invalid".to_owned()));
    }
    Ok(email.to_ascii_lowercase())
}

/// 校验重置口令去除首尾空白后为 6–20 个字符，返回值仅供后续安全散列。
pub(crate) fn validate_reset_password(value: &str) -> AppResult<String> {
    let password = required_string(Some(value.to_owned()), "password")?;
    if !(6..=20).contains(&password.chars().count()) {
        return Err(AppError::Validation(
            "password must be 6-20 characters long".to_owned(),
        ));
    }
    Ok(password)
}

/// 去除邀请码首尾空白并拒绝空值，具体存在性和可用次数留给事务仓储校验。
pub(crate) fn normalize_invite_code(code: &str) -> AppResult<String> {
    let code = code.trim();
    if code.is_empty() {
        return Err(AppError::Validation("invite_code is required".to_owned()));
    }
    Ok(code.to_owned())
}

/// 规范化必填字符串并在缺失时保留调用方的字段名用于错误语义。
pub(crate) fn required_string(value: Option<String>, field: &str) -> AppResult<String> {
    optional_string(value).ok_or_else(|| AppError::Validation(format!("{field} is required")))
}

/// 去除可选字符串首尾空白，并将空结果统一折叠为 `None`。
pub(crate) fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_auth_domain_tests.rs"]
mod tests;
