//! news bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。

use crate::{
    architecture::DomainLayer,
    error::{AppError, AppResult},
};

#[derive(Debug, Clone, Default)]
pub struct PublicNewsFilter {
    pub category: Option<String>,
    pub country_code: Option<String>,
    pub locale: Option<String>,
    pub keyword: Option<String>,
    pub limit: u32,
    pub offset: u32,
}

impl DomainLayer for PublicNewsFilter {}

pub fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

pub fn route_offset(offset: Option<u32>) -> u32 {
    offset.unwrap_or(0).min(10_000)
}

pub fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

pub fn validate_news_category(value: &str) -> AppResult<String> {
    match value {
        "general" | "market" | "product" | "system" | "promotion" => Ok(value.to_owned()),
        _ => Err(AppError::Validation("unsupported news category".to_owned())),
    }
}

pub fn normalize_news_country_code(value: &str) -> AppResult<String> {
    let country_code = value.to_ascii_uppercase();
    if country_code == "GLOBAL" {
        return Ok(country_code);
    }
    if country_code.len() < 2
        || country_code.len() > 16
        || !country_code.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    {
        return Err(AppError::Validation(
            "news country_code format is invalid".to_owned(),
        ));
    }
    Ok(country_code)
}

fn validate_news_locale(value: &str) -> AppResult<String> {
    if value.len() < 2
        || value.len() > 16
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err(AppError::Validation(
            "news locale format is invalid".to_owned(),
        ));
    }
    Ok(value.to_owned())
}

pub fn news_locale_search_patterns(value: &str) -> AppResult<Vec<String>> {
    let locale = validate_news_locale(value)?;
    let mut patterns = vec![locale.clone()];
    if let Some((language, _region)) = locale.split_once('-') {
        patterns.push(language.to_owned());
    } else {
        patterns.push(format!("{locale}-%"));
    }
    Ok(patterns)
}
