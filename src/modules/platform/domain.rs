//! platform bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。

use crate::{
    architecture::DomainLayer,
    error::{AppError, AppResult},
    modules::platform::presentation::SavePlatformBrandRequest,
};

pub const DEFAULT_CONFIG_NAME: &str = "default";
pub const DEFAULT_CHART_PROVIDER: &str = "klinecharts";
pub const TRADINGVIEW_CHART_PROVIDER: &str = "tradingview";

#[derive(Debug)]
pub struct ValidatedPlatformBrand {
    pub platform_name: String,
    pub logo_url: Option<String>,
    pub chart_provider: Option<String>,
}

impl DomainLayer for ValidatedPlatformBrand {}

pub fn validate_platform_brand(
    request: &SavePlatformBrandRequest,
) -> AppResult<ValidatedPlatformBrand> {
    Ok(ValidatedPlatformBrand {
        platform_name: required_string(Some(request.platform_name.clone()), "platform_name", 128)?,
        logo_url: validate_logo_url(request.logo_url.clone())?,
        chart_provider: validate_chart_provider(request.chart_provider.clone())?,
    })
}

fn validate_chart_provider(value: Option<String>) -> AppResult<Option<String>> {
    let Some(provider) = optional_string(value) else {
        return Ok(None);
    };
    let provider = provider.to_ascii_lowercase();
    if matches!(
        provider.as_str(),
        DEFAULT_CHART_PROVIDER | TRADINGVIEW_CHART_PROVIDER
    ) {
        Ok(Some(provider))
    } else {
        Err(AppError::Validation(
            "chart_provider must be klinecharts or tradingview".to_owned(),
        ))
    }
}

fn validate_logo_url(value: Option<String>) -> AppResult<Option<String>> {
    let Some(logo_url) = optional_string(value) else {
        return Ok(None);
    };
    if logo_url.chars().count() > 2048 {
        return Err(AppError::Validation("logo_url is too long".to_owned()));
    }
    if logo_url.chars().any(char::is_control) || logo_url.chars().any(char::is_whitespace) {
        return Err(AppError::Validation(
            "logo_url format is invalid".to_owned(),
        ));
    }
    let lower = logo_url.to_ascii_lowercase();
    if lower.starts_with("https://")
        || lower.starts_with("http://")
        || lower.starts_with("/")
        || lower.starts_with("data:image/")
    {
        Ok(Some(logo_url))
    } else {
        Err(AppError::Validation(
            "logo_url must be http(s), root-relative, or data:image".to_owned(),
        ))
    }
}

fn required_string(value: Option<String>, field: &str, max_chars: usize) -> AppResult<String> {
    let Some(value) = optional_string(value) else {
        return Err(AppError::Validation(format!("{field} is required")));
    };
    if value.chars().count() > max_chars {
        return Err(AppError::Validation(format!("{field} is too long")));
    }
    Ok(value)
}

fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
