//! auth bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。

use crate::{
    architecture::DomainLayer,
    error::{AppError, AppResult},
};

#[derive(Debug)]
pub struct AuthValidationRules;

impl DomainLayer for AuthValidationRules {}

pub(crate) fn validate_email_code(value: &str) -> AppResult<String> {
    let code = value.trim();
    if code.len() != 6 || !code.chars().all(|char| char.is_ascii_digit()) {
        return Err(AppError::Validation("code is invalid".to_owned()));
    }
    Ok(code.to_owned())
}

pub(crate) fn validate_registration_email(value: Option<String>) -> AppResult<String> {
    let email = required_string(value, "email")?;
    if email.len() > 255 || !email.contains('@') {
        return Err(AppError::Validation("email format is invalid".to_owned()));
    }
    Ok(email.to_ascii_lowercase())
}

pub(crate) fn validate_reset_password(value: &str) -> AppResult<String> {
    let password = required_string(Some(value.to_owned()), "password")?;
    if !(6..=20).contains(&password.chars().count()) {
        return Err(AppError::Validation(
            "password must be 6-20 characters long".to_owned(),
        ));
    }
    Ok(password)
}

pub(crate) fn normalize_invite_code(code: &str) -> AppResult<String> {
    let code = code.trim();
    if code.is_empty() {
        return Err(AppError::Validation("invite_code is required".to_owned()));
    }
    Ok(code.to_owned())
}

pub(crate) fn required_string(value: Option<String>, field: &str) -> AppResult<String> {
    optional_string(value).ok_or_else(|| AppError::Validation(format!("{field} is required")))
}

pub(crate) fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
