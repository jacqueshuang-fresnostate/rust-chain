//! countries bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。

use crate::{
    architecture::DomainLayer,
    error::{AppError, AppResult},
};

const ALLOWED_LOCALES: &[&str] = &["zh", "en"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicCountry {
    pub country_code: String,
    pub country_name: String,
    pub default_locale: String,
    pub supported_locales: Vec<String>,
}

impl DomainLayer for PublicCountry {}

pub fn normalize_country_code(value: &str) -> AppResult<String> {
    let country_code = value.trim().to_ascii_uppercase();
    if country_code.is_empty() {
        return Err(AppError::Validation("country_code is required".to_owned()));
    }
    if country_code.len() < 2
        || country_code.len() > 8
        || !country_code
            .chars()
            .all(|character| character.is_ascii_alphabetic())
    {
        return Err(AppError::Validation(
            "country_code format is invalid".to_owned(),
        ));
    }
    Ok(country_code)
}

pub fn normalize_locale(value: &str) -> AppResult<String> {
    let locale = value.trim().to_ascii_lowercase();
    if !ALLOWED_LOCALES.contains(&locale.as_str()) {
        return Err(AppError::Validation("unsupported locale".to_owned()));
    }
    Ok(locale)
}

pub fn normalize_supported_locales(values: Vec<String>) -> AppResult<Vec<String>> {
    let mut locales = Vec::new();
    for value in values {
        let locale = normalize_locale(&value)?;
        if !locales.contains(&locale) {
            locales.push(locale);
        }
    }
    if locales.is_empty() {
        return Err(AppError::Validation(
            "supported_locales is required".to_owned(),
        ));
    }
    Ok(locales)
}

pub fn ensure_default_locale_supported(
    default_locale: &str,
    supported_locales: &[String],
) -> AppResult<()> {
    if supported_locales
        .iter()
        .any(|locale| locale == default_locale)
    {
        Ok(())
    } else {
        Err(AppError::Validation(
            "default_locale must be included in supported_locales".to_owned(),
        ))
    }
}
