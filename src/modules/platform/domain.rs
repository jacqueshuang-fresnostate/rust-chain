//! platform bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。

use crate::architecture::DomainLayer;
use chrono::{DateTime, Utc};

pub const DEFAULT_CONFIG_NAME: &str = "default";
pub const DEFAULT_CHART_PROVIDER: &str = "klinecharts";
pub const TRADINGVIEW_CHART_PROVIDER: &str = "tradingview";

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("validation error: {message}")]
pub struct PlatformBrandValidationError {
    message: String,
}

impl PlatformBrandValidationError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub(crate) fn into_message(self) -> String {
        self.message
    }
}

#[derive(Debug)]
pub struct PlatformBrandCommand {
    pub platform_name: String,
    pub logo_url: Option<String>,
    pub chart_provider: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformBrand {
    pub id: u64,
    pub name: String,
    pub platform_name: String,
    pub logo_url: Option<String>,
    pub chart_provider: String,
    pub updated_by: Option<u64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl DomainLayer for PlatformBrand {}

#[derive(Debug)]
pub struct ValidatedPlatformBrand {
    pub platform_name: String,
    pub logo_url: Option<String>,
    pub chart_provider: Option<String>,
}

impl DomainLayer for ValidatedPlatformBrand {}

pub fn validate_platform_brand(
    command: PlatformBrandCommand,
) -> Result<ValidatedPlatformBrand, PlatformBrandValidationError> {
    Ok(ValidatedPlatformBrand {
        platform_name: required_string(Some(command.platform_name), "platform_name", 128)?,
        logo_url: validate_logo_url(command.logo_url)?,
        chart_provider: validate_chart_provider(command.chart_provider)?,
    })
}

fn validate_chart_provider(
    value: Option<String>,
) -> Result<Option<String>, PlatformBrandValidationError> {
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
        Err(PlatformBrandValidationError::new(
            "chart_provider must be klinecharts or tradingview",
        ))
    }
}

fn validate_logo_url(
    value: Option<String>,
) -> Result<Option<String>, PlatformBrandValidationError> {
    let Some(logo_url) = optional_string(value) else {
        return Ok(None);
    };
    if logo_url.chars().count() > 2048 {
        return Err(PlatformBrandValidationError::new("logo_url is too long"));
    }
    if logo_url.chars().any(char::is_control) || logo_url.chars().any(char::is_whitespace) {
        return Err(PlatformBrandValidationError::new(
            "logo_url format is invalid",
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
        Err(PlatformBrandValidationError::new(
            "logo_url must be http(s), root-relative, or data:image",
        ))
    }
}

fn required_string(
    value: Option<String>,
    field: &str,
    max_chars: usize,
) -> Result<String, PlatformBrandValidationError> {
    let Some(value) = optional_string(value) else {
        return Err(PlatformBrandValidationError::new(format!(
            "{field} is required"
        )));
    };
    if value.chars().count() > max_chars {
        return Err(PlatformBrandValidationError::new(format!(
            "{field} is too long"
        )));
    }
    Ok(value)
}

fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
