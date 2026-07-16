//! admin bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务服务逐步迁入。

use crate::{
    architecture::ServiceLayer,
    config::Settings,
    error::{AppError, AppResult},
    infra::email::{VerificationCodeTemplate, parse_smtp_security, smtp_security_code},
    modules::{
        admin::presentation::{
            AdminAgentCommissionResponse, AdminAgentCommissionRuleResponse, AdminAgentResponse,
            AdminAssetResponse, AdminCountryResponse, AdminDepositAddressPoolResponse,
            AdminDepositNetworkConfigResponse, AdminMarketStrategyResponse, AdminNewsItemResponse,
            AdminTradingPairResponse, AdminUserRechargeRequest, AdminUserRechargeResponse,
            AdminUserReferralResponse, AdminUserResponse, ConvertPairResponse,
            CreateAdminUserRequest, CreateAgentRequest, CreateAssetRequest,
            CreateConvertPairRequest, CreateDepositAddressPoolEntryRequest,
            CreateMarketStrategyRequest, CreateNewCoinProjectRequest, CreateRiskRuleRequest,
            CreateTradingPairRequest, DistributeNewCoinRequest, MarketFeedConfigResponse,
            MarketSourceCredentialResponse, NewCoinConvertRuleResponse,
            NewCoinDistributionResponse, NewCoinProjectResponse, RiskRuleResponse,
            SaveSmtpConfigRequest, SaveUploadConfigRequest, SmtpConfigResponse,
            SmtpDeliverySettingsResponse, UpdateAssetRequest, UpdateMarketStrategyRequest,
            UpdateNewCoinPostListingPurchaseRequest, UpdateNewCoinUnlockFeeRuleRequest,
            UpdateNewCoinUnlockRuleRequest, UpdateTradingPairRequest, UploadFileInput,
            UpsertNewCoinConvertRuleRequest,
        },
        admin::repository::{
            AdminMarketFeedConfigRecord, AdminMarketSourceCredentialRecord,
            AdminNewCoinLockPositionWrite, AdminSmtpConfigRecord, AdminSmtpDeliverySettingsRecord,
            AdminUploadConfigRecord,
        },
        auth::hash_password,
        countries::{
            ensure_default_locale_supported, normalize_country_code, normalize_locale,
            normalize_supported_locales,
        },
        market::{KlineUpsertKey, ValidatedMarketSymbol, adapters::MarketFeedProvider},
        new_coin::{LifecycleStatus, UnlockRule, UnlockSource, apply_unlock_rule},
        security::{PaymentPolicies, SecurityAction, UserSecurityPolicy, UserTwoFactorSettings},
        wallet::{WithdrawFeeTier, normalize_withdraw_fee_tiers},
    },
    state::AppState,
    workers::market_feed::{MarketFeedRuntimeConfig, MarketFeedRuntimeStatus},
};
use base64::{Engine as _, engine::general_purpose};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Digest;
use std::collections::HashSet;
use std::default::Default;
use uuid::Uuid;

const ADMIN_AUDIT_REASON_MAX_LEN: usize = 512;
pub(crate) const DEFAULT_SMTP_CONFIG_NAME: &str = "default";
pub(crate) const DEFAULT_SMTP_CONFIG_PRIORITY: u32 = 100;
pub(crate) const SMTP_DELIVERY_SETTINGS_ID: u8 = 1;
pub(crate) const SMTP_DELIVERY_STRATEGY_PRIORITY: &str = "priority";
pub(crate) const SMTP_DELIVERY_STRATEGY_ROUND_ROBIN: &str = "round_robin";
pub(crate) const DEFAULT_MARKET_FEED_CONFIG_NAME: &str = "default";
pub(crate) const MARKET_SOURCE_AUTH_TYPE_API_KEY: &str = "api_key";
pub(crate) const MARKET_SOURCE_AUTH_TYPE_NONE: &str = "none";
pub(crate) const DEFAULT_UPLOAD_FILE_FIELD: &str = "file";
const DEFAULT_UPLOAD_MAX_FILE_SIZE_BYTES: u64 = 10 * 1024 * 1024;
const MAX_UPLOAD_FILE_SIZE_BYTES: u64 = 100 * 1024 * 1024;
pub(crate) const MAX_UPLOAD_BODY_SIZE_BYTES: usize =
    (MAX_UPLOAD_FILE_SIZE_BYTES as usize) + 1024 * 1024;
pub(crate) const UPLOAD_IMAGE_MIME_TYPES: &[&str] =
    &["image/png", "image/jpeg", "image/webp", "image/gif"];

type HmacSha256 = Hmac<sha2::Sha256>;
type HmacSha1 = Hmac<sha1::Sha1>;

#[derive(Debug)]
pub struct ServiceLayerMarker;

impl ServiceLayer for ServiceLayerMarker {}

#[derive(Debug)]
pub(crate) struct SmtpValidatedConfig {
    pub(crate) name: String,
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) security: String,
    pub(crate) from_email: String,
    pub(crate) from_name: Option<String>,
    pub(crate) verification_code_template_html: Option<String>,
    pub(crate) verification_code_templates: Vec<VerificationCodeTemplate>,
    pub(crate) enabled: bool,
    pub(crate) priority: u32,
}

pub(crate) fn validate_smtp_save_request(
    request: &SaveSmtpConfigRequest,
    fallback_name: Option<&str>,
    fallback_priority: Option<u32>,
) -> AppResult<SmtpValidatedConfig> {
    let name = validate_smtp_config_name(request.name.clone(), fallback_name)?;
    let host = optional_string(Some(request.host.clone()))
        .ok_or_else(|| AppError::Validation("smtp host is required".to_owned()))?;
    if host.len() > 255 {
        return Err(AppError::Validation("smtp host is too long".to_owned()));
    }
    if request.port == 0 {
        return Err(AppError::Validation("smtp port is invalid".to_owned()));
    }
    let security = smtp_security_code(parse_smtp_security(&request.security)?).to_owned();
    let from_email = validate_smtp_email(&request.from_email, "from_email")?;
    let from_name = optional_string(request.from_name.clone());
    if let Some(from_name) = &from_name
        && from_name.len() > 128
    {
        return Err(AppError::Validation("from_name is too long".to_owned()));
    }
    let verification_code_template_html =
        optional_string(request.verification_code_template_html.clone());
    if let Some(template_html) = &verification_code_template_html
        && template_html.len() > 20_000
    {
        return Err(AppError::Validation(
            "verification_code_template_html is too long".to_owned(),
        ));
    }
    let verification_code_templates =
        validate_smtp_verification_code_templates(request.verification_code_templates.clone())?;
    let priority = request.priority.or(fallback_priority).unwrap_or(100);
    if priority > 9999 {
        return Err(AppError::Validation(
            "smtp priority cannot exceed 9999".to_owned(),
        ));
    }

    Ok(SmtpValidatedConfig {
        name,
        host,
        port: request.port,
        security,
        from_email,
        from_name,
        verification_code_template_html,
        verification_code_templates,
        enabled: request.enabled,
        priority,
    })
}

pub(crate) fn validate_smtp_delivery_strategy(value: &str) -> AppResult<String> {
    match value.trim() {
        "priority" => Ok("priority".to_owned()),
        "round_robin" => Ok("round_robin".to_owned()),
        _ => Err(AppError::Validation(
            "smtp delivery strategy is invalid".to_owned(),
        )),
    }
}

pub(crate) fn validate_smtp_email(value: &str, field: &str) -> AppResult<String> {
    let email = optional_string(Some(value.to_owned()))
        .ok_or_else(|| AppError::Validation(format!("smtp {field} is required")))?;
    let mut parts = email.split('@');
    let local = parts.next().unwrap_or_default();
    let domain = parts.next().unwrap_or_default();
    if email.len() > 255
        || local.is_empty()
        || domain.is_empty()
        || parts.next().is_some()
        || email.chars().any(char::is_whitespace)
    {
        return Err(AppError::Validation(format!("smtp {field} is invalid")));
    }
    Ok(email)
}

pub(crate) fn required_smtp_audit_reason(value: Option<String>) -> AppResult<String> {
    let Some(reason) = optional_string(value) else {
        return Err(AppError::Validation("reason is required".to_owned()));
    };
    if reason.chars().count() > ADMIN_AUDIT_REASON_MAX_LEN {
        return Err(AppError::Validation("reason is too long".to_owned()));
    }
    Ok(reason)
}

pub(crate) fn smtp_request_has_new_secret(request: &SaveSmtpConfigRequest) -> bool {
    request.username.as_deref().and_then(optional_str).is_some()
        || request.password.as_deref().and_then(optional_str).is_some()
}

pub(crate) fn default_smtp_delivery_settings_record() -> AdminSmtpDeliverySettingsRecord {
    AdminSmtpDeliverySettingsRecord {
        strategy: SMTP_DELIVERY_STRATEGY_PRIORITY.to_owned(),
        round_robin_cursor: None,
    }
}

pub(crate) fn smtp_delivery_settings_response(
    record: AdminSmtpDeliverySettingsRecord,
) -> SmtpDeliverySettingsResponse {
    SmtpDeliverySettingsResponse {
        strategy: record.strategy,
    }
}

pub(crate) fn smtp_delivery_settings_audit_json(record: &AdminSmtpDeliverySettingsRecord) -> Value {
    json!({
        "strategy": record.strategy,
        "round_robin_cursor": record.round_robin_cursor,
    })
}

pub(crate) fn smtp_config_response(record: AdminSmtpConfigRecord) -> SmtpConfigResponse {
    let verification_code_templates = smtp_templates_from_record(&record);
    SmtpConfigResponse {
        id: record.id,
        name: record.name,
        host: record.host,
        port: record.port,
        security: record.security,
        username_mask: record.username_mask,
        password_set: record.password_ciphertext.is_some(),
        from_email: record.from_email,
        from_name: record.from_name,
        verification_code_template_html: record.verification_code_template_html.clone(),
        verification_code_templates,
        enabled: record.enabled,
        priority: record.priority,
    }
}

pub(crate) fn smtp_config_audit_json(record: &AdminSmtpConfigRecord) -> Value {
    json!({
        "id": record.id,
        "name": record.name,
        "host": record.host,
        "port": record.port,
        "security": record.security,
        "username_mask": record.username_mask,
        "password_set": record.password_ciphertext.is_some(),
        "from_email": record.from_email,
        "from_name": record.from_name,
        "verification_code_template_html": record.verification_code_template_html,
        "verification_code_templates": smtp_templates_from_record(record),
        "enabled": record.enabled,
        "priority": record.priority,
    })
}

/// 统一读取市场推送运行时状态，避免 routes 层重复处理空 supervisor。
pub(crate) async fn load_market_feed_runtime(state: &AppState) -> MarketFeedRuntimeStatus {
    match &state.market_feed_supervisor {
        Some(supervisor) => supervisor.status().await,
        None => Default::default(),
    }
}

pub(crate) fn smtp_templates_from_record(
    record: &AdminSmtpConfigRecord,
) -> Vec<VerificationCodeTemplate> {
    if !record.verification_code_templates.is_empty() {
        return record.verification_code_templates.clone();
    }

    record
        .verification_code_template_html
        .as_deref()
        .and_then(optional_str)
        .map(|html| VerificationCodeTemplate {
            key: "default".to_owned(),
            name: "通用验证码模板".to_owned(),
            purpose: None,
            html: html.to_owned(),
            enabled: true,
        })
        .into_iter()
        .collect()
}

pub(crate) fn select_smtp_delivery_config(
    settings: &AdminSmtpDeliverySettingsRecord,
    records: &[AdminSmtpConfigRecord],
) -> Option<AdminSmtpConfigRecord> {
    if records.is_empty() {
        return None;
    }
    if settings.strategy != SMTP_DELIVERY_STRATEGY_ROUND_ROBIN {
        return records.first().cloned();
    }

    let next_index = settings
        .round_robin_cursor
        .and_then(|cursor| records.iter().position(|record| record.id == cursor))
        .map(|index| (index + 1) % records.len())
        .unwrap_or(0);
    records.get(next_index).cloned()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UploadProvider {
    ImageBed,
    Oss,
    S3,
    Local,
}

impl UploadProvider {
    pub(crate) fn parse(value: &str) -> AppResult<Self> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "image_bed" | "imagebed" => Ok(Self::ImageBed),
            "oss" => Ok(Self::Oss),
            "s3" => Ok(Self::S3),
            "local" => Ok(Self::Local),
            _ => Err(AppError::Validation(
                "upload provider is invalid".to_owned(),
            )),
        }
    }

    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::ImageBed => "image_bed",
            Self::Oss => "oss",
            Self::S3 => "s3",
            Self::Local => "local",
        }
    }

    pub(crate) const fn uses_bearer(self) -> bool {
        matches!(self, Self::ImageBed)
    }

    pub(crate) const fn uses_access_secret(self) -> bool {
        matches!(self, Self::Oss | Self::S3)
    }
}

#[derive(Debug)]
pub(crate) struct ValidatedUploadConfig {
    pub(crate) provider: UploadProvider,
    pub(crate) endpoint: Option<String>,
    pub(crate) file_field: Option<String>,
    pub(crate) bucket: Option<String>,
    pub(crate) region: Option<String>,
    pub(crate) public_base_url: Option<String>,
    pub(crate) local_root: Option<String>,
    pub(crate) key_prefix: Option<String>,
    pub(crate) max_file_size_bytes: u64,
    pub(crate) allowed_mime_types: Vec<String>,
    pub(crate) enabled: bool,
}

pub(crate) fn validate_upload_config(
    request: &SaveUploadConfigRequest,
) -> AppResult<ValidatedUploadConfig> {
    let provider = UploadProvider::parse(&request.provider)?;
    let endpoint = optional_string(request.endpoint.clone());
    let public_base_url = optional_string(request.public_base_url.clone());
    let local_root = optional_string(request.local_root.clone());
    let bucket = optional_string(request.bucket.clone());
    let region = optional_string(request.region.clone());
    validate_upload_optional_len(endpoint.as_deref(), "endpoint", 512)?;
    validate_upload_optional_len(public_base_url.as_deref(), "public_base_url", 512)?;
    validate_upload_optional_len(local_root.as_deref(), "local_root", 512)?;
    let key_prefix = normalize_upload_key_prefix(request.key_prefix.clone())?;
    let file_field = Some(validate_upload_len(
        optional_string(request.file_field.clone())
            .unwrap_or_else(|| DEFAULT_UPLOAD_FILE_FIELD.to_owned()),
        "file_field",
        64,
    )?);
    let max_file_size_bytes = request
        .max_file_size_bytes
        .unwrap_or(DEFAULT_UPLOAD_MAX_FILE_SIZE_BYTES);
    if max_file_size_bytes == 0 || max_file_size_bytes > MAX_UPLOAD_FILE_SIZE_BYTES {
        return Err(AppError::Validation(
            "max_file_size_bytes is invalid".to_owned(),
        ));
    }
    let allowed_mime_types = normalize_upload_mime_types(request.allowed_mime_types.clone())?;

    match provider {
        UploadProvider::ImageBed => {
            validate_upload_credential_url(endpoint.as_deref(), "image bed endpoint")?;
        }
        UploadProvider::Local => {
            require_upload_value(local_root.as_deref(), "local_root")?;
            validate_upload_url(public_base_url.as_deref(), "public_base_url")?;
        }
        UploadProvider::S3 => {
            validate_upload_bucket_name(bucket.as_deref())?;
            validate_upload_region(region.as_deref())?;
            if let Some(endpoint) = &endpoint {
                validate_upload_credential_url(Some(endpoint), "s3 endpoint")?;
            }
            if let Some(public_base_url) = &public_base_url {
                validate_upload_url(Some(public_base_url), "public_base_url")?;
            }
        }
        UploadProvider::Oss => {
            validate_upload_credential_url(endpoint.as_deref(), "oss endpoint")?;
            validate_upload_bucket_name(bucket.as_deref())?;
            if let Some(public_base_url) = &public_base_url {
                validate_upload_url(Some(public_base_url), "public_base_url")?;
            }
        }
    }

    Ok(ValidatedUploadConfig {
        provider,
        endpoint,
        file_field,
        bucket,
        region,
        public_base_url,
        local_root,
        key_prefix,
        max_file_size_bytes,
        allowed_mime_types,
        enabled: request.enabled,
    })
}

pub(crate) fn validate_upload_file(
    max_file_size_bytes: u64,
    allowed_mime_types: &[String],
    input: &UploadFileInput,
) -> AppResult<()> {
    if input.bytes.is_empty() {
        return Err(AppError::Validation("upload file is required".to_owned()));
    }
    validate_upload_image_bytes(&input.bytes, &input.mime_type)?;
    let size = input.bytes.len() as u64;
    if size > max_file_size_bytes {
        return Err(AppError::Validation("upload file is too large".to_owned()));
    }
    if !allowed_mime_types
        .iter()
        .any(|mime| mime == &input.mime_type)
    {
        return Err(AppError::Validation(
            "upload file mime type is not allowed".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn required_upload_audit_reason(value: Option<String>) -> AppResult<String> {
    let Some(reason) = optional_string(value) else {
        return Err(AppError::Validation("reason is required".to_owned()));
    };
    if reason.chars().count() > ADMIN_AUDIT_REASON_MAX_LEN {
        return Err(AppError::Validation("reason is too long".to_owned()));
    }
    Ok(reason)
}

pub(crate) fn upload_config_secret_destination_unchanged(
    record: &AdminUploadConfigRecord,
    config: &ValidatedUploadConfig,
) -> bool {
    record.endpoint == config.endpoint
        && record.bucket == config.bucket
        && record.region == config.region
}

pub(crate) fn upload_config_audit_json(record: &AdminUploadConfigRecord) -> Value {
    json!({
        "id": record.id,
        "name": record.name,
        "provider": record.provider,
        "endpoint": record.endpoint,
        "file_field": record.file_field,
        "bearer_token_mask": record.bearer_token_mask,
        "bearer_token_set": record.bearer_token_ciphertext.is_some(),
        "access_key_mask": record.access_key_mask,
        "access_key_set": record.access_key_ciphertext.is_some(),
        "secret_key_set": record.secret_key_ciphertext.is_some(),
        "bucket": record.bucket,
        "region": record.region,
        "public_base_url": record.public_base_url,
        "local_root": record.local_root,
        "key_prefix": record.key_prefix,
        "max_file_size_bytes": record.max_file_size_bytes,
        "allowed_mime_types": record.allowed_mime_types,
        "enabled": record.enabled,
    })
}

pub(crate) fn generated_upload_object_key(prefix: Option<&str>, mime_type: &str) -> String {
    let date = Utc::now().format("%Y/%m/%d");
    let suffix = upload_extension_for_mime(mime_type);
    let key = format!("{date}/{}.{}", Uuid::now_v7().simple(), suffix);
    match prefix.and_then(optional_str) {
        Some(prefix) => format!("{}/{}", prefix.trim_matches('/'), key),
        None => key,
    }
}

pub(crate) fn safe_upload_filename(original: Option<&str>, mime_type: &str) -> String {
    let extension = upload_extension_for_mime(mime_type);
    let Some(original) = original.and_then(optional_str) else {
        return format!("upload.{extension}");
    };
    let normalized = original.replace('\\', "/");
    let candidate = normalized.split('/').next_back().unwrap_or("upload");
    let name = safe_upload_key_segment(candidate);
    let name = if name.is_empty() {
        format!("upload.{extension}")
    } else {
        name
    };
    truncate_upload_filename(name, extension, 255)
}

pub(crate) fn safe_upload_key_segment(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        .collect()
}

pub(crate) fn safe_upload_response_url(
    value: Option<&str>,
    field: &str,
    required: bool,
) -> AppResult<Option<String>> {
    let Some(value) = value.and_then(optional_str) else {
        return if required {
            Err(AppError::Validation(format!("{field} is missing")))
        } else {
            Ok(None)
        };
    };
    validate_upload_safe_url(value, field, false).map(Some)
}

pub(crate) fn join_upload_public_url(base: &str, object_key: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        object_key.trim_start_matches('/')
    )
}

pub(crate) fn join_upload_endpoint_path(endpoint: &str, parts: &[&str]) -> AppResult<String> {
    let base = endpoint.trim_end_matches('/');
    let path = parts
        .iter()
        .map(|part| part.trim_matches('/'))
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    let url = format!("{base}/{path}");
    reqwest::Url::parse(&url)
        .map_err(|_| AppError::Validation("upload endpoint is invalid".to_owned()))?;
    Ok(url)
}

pub(crate) fn upload_url_host(url: &reqwest::Url) -> AppResult<String> {
    let host = url
        .host_str()
        .ok_or_else(|| AppError::Validation("upload endpoint host is invalid".to_owned()))?;
    Ok(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    })
}

pub(crate) fn sha256_hex(data: &[u8]) -> String {
    hex::encode(sha2::Sha256::digest(data))
}

pub(crate) fn hmac_sha1_base64(key: &[u8], data: &str) -> String {
    let mut mac = HmacSha1::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data.as_bytes());
    general_purpose::STANDARD.encode(mac.finalize().into_bytes())
}

pub(crate) fn s3_upload_signature(
    secret: &str,
    date: &str,
    region: &str,
    string_to_sign: &str,
) -> String {
    let k_date = hmac_sha256(format!("AWS4{secret}").as_bytes(), date);
    let k_region = hmac_sha256(&k_date, region);
    let k_service = hmac_sha256(&k_region, "s3");
    let k_signing = hmac_sha256(&k_service, "aws4_request");
    hex::encode(hmac_sha256(&k_signing, string_to_sign))
}

fn validate_upload_image_bytes(bytes: &[u8], mime_type: &str) -> AppResult<()> {
    let valid = match mime_type {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "image/webp" => bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP",
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(AppError::Validation(
            "upload file content is invalid".to_owned(),
        ))
    }
}

fn normalize_upload_mime_types(value: Option<Vec<String>>) -> AppResult<Vec<String>> {
    let values = value.unwrap_or_else(|| {
        UPLOAD_IMAGE_MIME_TYPES
            .iter()
            .map(|item| (*item).to_owned())
            .collect()
    });
    let mut normalized = Vec::new();
    for item in values {
        let mime = optional_string(Some(item))
            .ok_or_else(|| AppError::Validation("allowed mime type is invalid".to_owned()))?
            .to_ascii_lowercase();
        if !UPLOAD_IMAGE_MIME_TYPES.contains(&mime.as_str()) {
            return Err(AppError::Validation(
                "allowed mime type is invalid".to_owned(),
            ));
        }
        if !normalized.contains(&mime) {
            normalized.push(mime);
        }
    }
    if normalized.is_empty() {
        return Err(AppError::Validation(
            "allowed mime types are required".to_owned(),
        ));
    }
    Ok(normalized)
}

fn normalize_upload_key_prefix(value: Option<String>) -> AppResult<Option<String>> {
    let Some(value) = optional_string(value) else {
        return Ok(None);
    };
    let mut segments = Vec::new();
    for segment in value.replace('\\', "/").split('/').filter_map(optional_str) {
        if matches!(segment, "." | "..") {
            return Err(AppError::Validation("key_prefix is invalid".to_owned()));
        }
        let safe_segment = safe_upload_key_segment(segment);
        if !safe_segment.is_empty() {
            segments.push(safe_segment);
        }
    }
    let prefix = segments.join("/");
    if prefix.len() > 128 {
        return Err(AppError::Validation("key_prefix is invalid".to_owned()));
    }
    Ok((!prefix.is_empty()).then_some(prefix))
}

fn truncate_upload_filename(name: String, extension: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        return name;
    }
    let suffix = format!(".{extension}");
    if name.ends_with(&suffix) && max_len > suffix.len() {
        let stem_len = max_len - suffix.len();
        format!("{}{}", &name[..stem_len], suffix)
    } else {
        name[..max_len].to_owned()
    }
}

fn upload_extension_for_mime(mime_type: &str) -> &'static str {
    match mime_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "bin",
    }
}

fn validate_upload_len(value: String, field: &str, max_len: usize) -> AppResult<String> {
    if value.len() > max_len {
        Err(AppError::Validation(format!("{field} is invalid")))
    } else {
        Ok(value)
    }
}

fn validate_upload_optional_len(value: Option<&str>, field: &str, max_len: usize) -> AppResult<()> {
    if value.is_some_and(|value| value.len() > max_len) {
        Err(AppError::Validation(format!("{field} is invalid")))
    } else {
        Ok(())
    }
}

fn validate_upload_url(value: Option<&str>, field: &str) -> AppResult<()> {
    let value = require_upload_value(value, field)?;
    validate_upload_safe_url(value, field, false).map(|_| ())
}

fn validate_upload_credential_url(value: Option<&str>, field: &str) -> AppResult<()> {
    let value = require_upload_value(value, field)?;
    validate_upload_safe_url(value, field, true).map(|_| ())
}

fn validate_upload_safe_url(value: &str, field: &str, require_https: bool) -> AppResult<String> {
    let url = reqwest::Url::parse(value)
        .map_err(|_| AppError::Validation(format!("{field} is invalid")))?;
    let valid_scheme = if require_https {
        url.scheme() == "https" || (url.scheme() == "http" && is_loopback_upload_url(&url))
    } else {
        matches!(url.scheme(), "http" | "https")
    };
    if !valid_scheme
        || value.len() > 2048
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(AppError::Validation(format!("{field} is invalid")));
    }
    Ok(value.to_owned())
}

fn is_loopback_upload_url(url: &reqwest::Url) -> bool {
    matches!(
        url.host_str(),
        Some("localhost") | Some("127.0.0.1") | Some("::1")
    )
}

fn validate_upload_bucket_name(value: Option<&str>) -> AppResult<()> {
    let value = require_upload_value(value, "bucket")?;
    let valid = (3..=255).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'));
    if valid {
        Ok(())
    } else {
        Err(AppError::Validation("bucket is invalid".to_owned()))
    }
}

fn validate_upload_region(value: Option<&str>) -> AppResult<()> {
    let value = require_upload_value(value, "region")?;
    let valid = value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
    if valid {
        Ok(())
    } else {
        Err(AppError::Validation("region is invalid".to_owned()))
    }
}

fn require_upload_value<'a>(value: Option<&'a str>, field: &str) -> AppResult<&'a str> {
    value
        .and_then(optional_str)
        .ok_or_else(|| AppError::Validation(format!("{field} is required")))
}

fn hmac_sha256(key: &[u8], data: &str) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

pub(crate) fn validate_market_feed_symbols(
    symbols: &[String],
    enabled: bool,
) -> AppResult<Vec<String>> {
    if enabled && symbols.is_empty() {
        return Err(AppError::Validation(
            "market feed symbols are required when enabled".to_owned(),
        ));
    }
    symbols
        .iter()
        .map(|symbol| {
            ValidatedMarketSymbol::from_raw(symbol)
                .map(|symbol| symbol.as_str().to_owned())
                .map_err(|error| AppError::Validation(error.to_string()))
        })
        .collect()
}

pub(crate) fn validate_market_feed_intervals(intervals: &[String]) -> AppResult<Vec<String>> {
    if intervals.is_empty() {
        return Err(AppError::Validation(
            "market feed intervals are required".to_owned(),
        ));
    }
    intervals
        .iter()
        .map(|interval| match interval.trim() {
            value => KlineUpsertKey::new(value, Utc::now())
                .map(|key| key.interval().to_owned())
                .map_err(|error| AppError::Validation(error.to_string())),
        })
        .collect()
}

pub(crate) fn validate_market_feed_providers(providers: &[String]) -> AppResult<Vec<String>> {
    if providers.is_empty() {
        return Err(AppError::Validation(
            "market feed providers are required".to_owned(),
        ));
    }
    let mut normalized = Vec::new();
    for provider in providers {
        let provider = MarketFeedProvider::from_code(provider)?.code().to_owned();
        if !normalized.contains(&provider) {
            normalized.push(provider);
        }
    }
    if normalized.len() > 1 {
        return Err(AppError::Validation(
            "market feed only supports one enabled provider".to_owned(),
        ));
    }
    Ok(normalized)
}

pub(crate) fn validate_market_feed_reason(reason: Option<&str>) -> AppResult<()> {
    let Some(reason) = reason.map(str::trim).filter(|reason| !reason.is_empty()) else {
        return Err(AppError::Validation(
            "operation reason is required".to_owned(),
        ));
    };
    if reason.chars().count() > ADMIN_AUDIT_REASON_MAX_LEN {
        return Err(AppError::Validation(
            "operation reason is too long".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_market_source_auth_type(auth_type: &str) -> AppResult<String> {
    let normalized = auth_type.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "none" | "api_key" => Ok(normalized),
        _ => Err(AppError::Validation(
            "market source credential auth_type is invalid".to_owned(),
        )),
    }
}

pub(crate) fn market_feed_config_response(
    record: AdminMarketFeedConfigRecord,
) -> MarketFeedConfigResponse {
    // 行情配置的 version/applied_version 差异就是后台是否需要 reload 的唯一判断来源。
    let needs_reload = Some(record.version) != record.applied_version;
    MarketFeedConfigResponse {
        id: record.id,
        name: record.name,
        symbols: record.symbols,
        intervals: record.intervals,
        providers: record.providers,
        enabled: record.enabled,
        version: record.version,
        applied_version: record.applied_version,
        needs_reload,
        last_reload_status: record.last_reload_status,
        last_reload_error: record.last_reload_error,
        last_reloaded_at: record.last_reloaded_at,
    }
}

pub(crate) fn market_source_credential_response(
    record: AdminMarketSourceCredentialRecord,
) -> MarketSourceCredentialResponse {
    MarketSourceCredentialResponse {
        provider: record.provider,
        auth_type: record.auth_type,
        api_key_mask: record.api_key_mask,
        enabled: record.enabled,
    }
}

pub(crate) fn market_feed_config_audit_json(record: &AdminMarketFeedConfigRecord) -> Value {
    json!({
        "id": record.id,
        "name": record.name,
        "symbols": &record.symbols,
        "intervals": &record.intervals,
        "providers": &record.providers,
        "enabled": record.enabled,
        "version": record.version,
        "applied_version": record.applied_version,
        "last_reload_status": record.last_reload_status,
        "last_reload_error": record.last_reload_error,
        "last_reloaded_at": record.last_reloaded_at.as_ref().map(|value| value.timestamp_millis()),
    })
}

pub(crate) fn market_source_credential_audit_json(
    record: &AdminMarketSourceCredentialRecord,
) -> Value {
    json!({
        "provider": record.provider,
        "auth_type": record.auth_type,
        "api_key_mask": record.api_key_mask,
        "enabled": record.enabled,
    })
}

pub(crate) fn market_feed_reload_audit_json(
    config: &MarketFeedConfigResponse,
    runtime: &MarketFeedRuntimeStatus,
) -> Value {
    json!({
        "version": config.version,
        "applied_version": config.applied_version,
        "runtime": runtime,
    })
}

pub(crate) fn market_source_credential_target_id(provider: &str) -> u64 {
    provider
        .as_bytes()
        .iter()
        .fold(0_u64, |acc, byte| acc + u64::from(*byte))
}

pub(crate) fn sanitize_market_feed_reload_error(error: &str) -> String {
    error.chars().take(512).collect()
}

pub fn market_feed_runtime_config_from_response(
    settings: &Settings,
    config: &MarketFeedConfigResponse,
) -> AppResult<MarketFeedRuntimeConfig> {
    MarketFeedRuntimeConfig::new(
        settings,
        config.symbols.clone(),
        config.intervals.clone(),
        config.providers.clone(),
        settings.market_feed_reconnect_seconds,
    )
}

pub(crate) fn validate_country_code(value: &str) -> AppResult<String> {
    normalize_country_code(value)
}

pub(crate) fn validate_country_name(value: &str) -> AppResult<String> {
    let Some(country_name) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("country_name is required".to_owned()));
    };
    if country_name.chars().count() > 128 {
        return Err(AppError::Validation("country_name is too long".to_owned()));
    }
    Ok(country_name)
}

pub(crate) fn validate_country_remark(value: &str) -> AppResult<String> {
    let Some(remark) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("remark is required".to_owned()));
    };
    if remark.chars().count() > 128 {
        return Err(AppError::Validation("remark is too long".to_owned()));
    }
    Ok(remark)
}

pub(crate) fn validate_country_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "active" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "unsupported country status".to_owned(),
        )),
    }
}

pub(crate) fn validate_country_locale_config(
    default_locale: &str,
    supported_locales: Vec<String>,
) -> AppResult<(String, Vec<String>)> {
    let default_locale = normalize_locale(default_locale)?;
    let supported_locales = normalize_supported_locales(supported_locales)?;
    ensure_default_locale_supported(&default_locale, &supported_locales)?;
    Ok((default_locale, supported_locales))
}

pub(crate) fn required_admin_audit_reason(value: Option<String>) -> AppResult<String> {
    let Some(reason) = optional_string(value) else {
        return Err(AppError::Validation("reason is required".to_owned()));
    };
    if reason.chars().count() > ADMIN_AUDIT_REASON_MAX_LEN {
        return Err(AppError::Validation("reason is too long".to_owned()));
    }
    Ok(reason)
}

pub(crate) fn validate_agent_commission_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "settled" | "rejected" => Ok(status),
        _ => Err(AppError::Validation(
            "unsupported agent commission status".to_owned(),
        )),
    }
}

pub(crate) fn validate_agent_commission_rule_product_type(value: &str) -> AppResult<String> {
    crate::modules::agent::service::normalize_agent_commission_product_type(value)
}

pub(crate) fn validate_agent_commission_rule_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "active" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "unsupported agent commission rule status".to_owned(),
        )),
    }
}

pub(crate) fn validate_agent_commission_rate(value: &BigDecimal) -> AppResult<()> {
    if value < &BigDecimal::from(0) || value > &BigDecimal::from(1) {
        return Err(AppError::Validation(
            "commission_rate must be between 0 and 1".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn agent_commission_audit_json(commission: &AdminAgentCommissionResponse) -> Value {
    json!({
        "id": commission.id,
        "agent_id": commission.agent_id,
        "user_id": commission.user_id,
        "source_type": commission.source_type,
        "source_id": commission.source_id,
        "source_amount": commission.source_amount,
        "commission_rate": commission.commission_rate,
        "commission_amount": commission.commission_amount,
        "status": commission.status,
        "created_at": commission.created_at.timestamp_millis(),
    })
}

pub(crate) fn agent_commission_rule_audit_json(rule: &AdminAgentCommissionRuleResponse) -> Value {
    json!({
        "id": rule.id,
        "agent_id": rule.agent_id,
        "product_type": rule.product_type,
        "commission_rate": rule.commission_rate,
        "status": rule.status,
        "created_at": rule.created_at.timestamp_millis(),
        "updated_at": rule.updated_at.timestamp_millis(),
    })
}

pub(crate) fn validate_create_risk_rule(request: &CreateRiskRuleRequest) -> AppResult<()> {
    if optional_string(Some(request.rule_type.clone())).is_none() {
        return Err(AppError::Validation("rule_type is required".to_owned()));
    }
    if optional_string(Some(request.target_type.clone())).is_none() {
        return Err(AppError::Validation("target_type is required".to_owned()));
    }
    if request.config_json.is_null() {
        return Err(AppError::Validation("config_json is required".to_owned()));
    }
    Ok(())
}

pub(crate) fn risk_rule_audit_json(rule: &RiskRuleResponse) -> Value {
    json!({
        "id": rule.id,
        "rule_type": rule.rule_type,
        "target_type": rule.target_type,
        "target_id": rule.target_id,
        "config_json": rule.config_json.0,
        "enabled": rule.enabled,
        "created_by": rule.created_by,
    })
}

pub(crate) fn validate_admin_user_recharge(request: &AdminUserRechargeRequest) -> AppResult<()> {
    if request.asset_id == 0 {
        return Err(AppError::Validation("asset_id is required".to_owned()));
    }
    if request.amount <= 0 {
        return Err(AppError::Validation("amount must be positive".to_owned()));
    }
    required_admin_audit_reason(request.reason.clone())?;
    Ok(())
}

pub(crate) fn validate_create_admin_user_request(
    request: &CreateAdminUserRequest,
) -> AppResult<()> {
    if optional_string(request.email.clone()).is_none()
        && optional_string(request.phone.clone()).is_none()
    {
        return Err(AppError::Validation(
            "email or phone is required".to_owned(),
        ));
    }
    if let Some(email) = optional_string(request.email.clone())
        && (email.len() > 255 || !email.contains('@'))
    {
        return Err(AppError::Validation("email format is invalid".to_owned()));
    }
    if let Some(phone) = optional_string(request.phone.clone())
        && phone.len() > 32
    {
        return Err(AppError::Validation("phone is too long".to_owned()));
    }
    if optional_string(Some(request.password.clone())).is_none() {
        return Err(AppError::Validation("password is required".to_owned()));
    }
    if let Some(status) = request.status.as_deref() {
        validate_user_status(status)?;
    }
    if request.kyc_level.unwrap_or(0) < 0 {
        return Err(AppError::Validation(
            "kyc_level must be non-negative".to_owned(),
        ));
    }
    required_admin_audit_reason(request.reason.clone())?;
    Ok(())
}

pub(crate) fn validate_user_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "active" | "suspended" | "disabled" => Ok(status),
        _ => Err(AppError::Validation("unsupported user status".to_owned())),
    }
}

pub(crate) fn hash_admin_user_password(password: &str) -> AppResult<String> {
    hash_password(password)
}

pub(crate) fn user_audit_json(user: &AdminUserResponse) -> Value {
    json!({
        "id": user.id,
        "email": user.email,
        "phone": user.phone,
        "status": user.status,
        "kyc_level": user.kyc_level,
        "created_at": user.created_at.timestamp_millis(),
        "updated_at": user.updated_at.timestamp_millis(),
    })
}

pub(crate) fn recharge_audit_json(recharge: &AdminUserRechargeResponse) -> Value {
    json!({
        "recharge_id": recharge.recharge_id,
        "user_id": recharge.user_id,
        "asset_id": recharge.asset_id,
        "asset_symbol": recharge.asset_symbol,
        "amount": format!("{:.18}", recharge.amount),
        "available": format!("{:.18}", recharge.available),
        "frozen": format!("{:.18}", recharge.frozen),
        "locked": format!("{:.18}", recharge.locked),
    })
}

pub(crate) fn validate_create_agent_request(request: &CreateAgentRequest) -> AppResult<()> {
    if request.user_id == 0 {
        return Err(AppError::Validation("user_id is required".to_owned()));
    }
    if optional_string(Some(request.agent_code.clone())).is_none() {
        return Err(AppError::Validation("agent_code is required".to_owned()));
    }
    if optional_string(Some(request.admin_username.clone())).is_none() {
        return Err(AppError::Validation(
            "admin_username is required".to_owned(),
        ));
    }
    if request.parent_agent_id == Some(0) {
        return Err(AppError::Validation(
            "parent_agent_id must be positive".to_owned(),
        ));
    }
    if optional_string(request.admin_password.clone()).is_none()
        && optional_string(request.admin_password_hash.clone()).is_none()
    {
        return Err(AppError::Validation(
            "admin_password is required".to_owned(),
        ));
    }
    if request.level.is_some_and(|level| !(1..=3).contains(&level)) {
        return Err(AppError::Validation(
            "level must be between 1 and 3".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn agent_password_hash(request: &CreateAgentRequest) -> AppResult<String> {
    if let Some(password) = optional_string(request.admin_password.clone()) {
        return hash_password(&password);
    }
    optional_string(request.admin_password_hash.clone())
        .ok_or_else(|| AppError::Validation("admin_password is required".to_owned()))
}

pub(crate) fn validate_agent_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "active" | "suspended" | "disabled" => Ok(status),
        _ => Err(AppError::Validation("unsupported agent status".to_owned())),
    }
}

pub(crate) fn agent_audit_json(agent: &AdminAgentResponse) -> Value {
    json!({
        "id": agent.id,
        "user_id": agent.user_id,
        "email": agent.email,
        "parent_agent_id": agent.parent_agent_id,
        "parent_agent_code": agent.parent_agent_code,
        "root_agent_id": agent.root_agent_id,
        "root_agent_code": agent.root_agent_code,
        "agent_code": agent.agent_code,
        "level": agent.level,
        "path": agent.path,
        "status": agent.status,
        "direct_user_count": agent.direct_user_count,
        "team_user_count": agent.team_user_count,
        "child_agent_count": agent.child_agent_count,
        "admin_user_id": agent.admin_user_id,
        "admin_username": agent.admin_username,
        "admin_status": agent.admin_status,
        "created_at": agent.created_at.timestamp_millis(),
    })
}

pub(crate) fn user_referral_audit_json(referral: &AdminUserReferralResponse) -> Value {
    json!({
        "user_id": referral.user_id,
        "direct_inviter_id": referral.direct_inviter_id,
        "direct_inviter_type": referral.direct_inviter_type,
        "root_agent_id": referral.root_agent_id,
        "depth": referral.depth,
        "path": referral.path,
        "created_at": referral.created_at.timestamp_millis(),
    })
}

pub(crate) fn validate_security_policy(policies: &PaymentPolicies) -> AppResult<()> {
    let _ = policies.policy_for(SecurityAction::Withdraw);
    let _ = policies.policy_for(SecurityAction::SpotOrder);
    let _ = policies.policy_for(SecurityAction::Convert);
    let _ = policies.policy_for(SecurityAction::EarnSubscribe);
    Ok(())
}

pub(crate) fn security_policy_audit_json(policy: &UserSecurityPolicy) -> AppResult<Value> {
    serde_json::to_value(policy).map_err(|error| {
        AppError::Internal(format!("failed to serialize security policy: {error}"))
    })
}

pub(crate) fn two_factor_audit_json(settings: &UserTwoFactorSettings) -> Value {
    json!({
        "user_id": settings.user_id,
        "totp_enabled": settings.totp_enabled,
        "login_2fa_enabled": settings.login_2fa_enabled,
        "confirmed_at": settings.confirmed_at.map(|value| value.timestamp_millis()),
        "last_verified_at": settings.last_verified_at.map(|value| value.timestamp_millis()),
    })
}

pub(crate) fn country_config_audit_json(country: &AdminCountryResponse) -> Value {
    json!({
        "id": country.id,
        "country_code": country.country_code,
        "country_name": country.country_name,
        "remark": country.remark,
        "default_locale": country.default_locale,
        "supported_locales": country.supported_locales.0.clone(),
        "registration_enabled": country.registration_enabled,
        "status": country.status,
        "sort_order": country.sort_order,
        "created_at": country.created_at.timestamp_millis(),
        "updated_at": country.updated_at.timestamp_millis(),
    })
}

pub(crate) fn validate_optional_image_url(
    value: Option<String>,
    field: &str,
) -> AppResult<Option<String>> {
    let Some(url) = optional_string(value) else {
        return Ok(None);
    };
    if url.chars().count() > 2048 {
        return Err(AppError::Validation(format!("{field} is too long")));
    }
    Ok(Some(url))
}

pub(crate) fn validate_news_title(value: &str) -> AppResult<String> {
    let Some(title) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("news title is required".to_owned()));
    };
    if title.chars().count() > 255 {
        return Err(AppError::Validation("news title is too long".to_owned()));
    }
    Ok(title)
}

pub(crate) fn validate_news_category(value: &str) -> AppResult<String> {
    let Some(category) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("news category is required".to_owned()));
    };
    match category.as_str() {
        "general" | "market" | "product" | "system" | "promotion" => Ok(category),
        _ => Err(AppError::Validation("unsupported news category".to_owned())),
    }
}

pub(crate) fn validate_news_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("news status is required".to_owned()));
    };
    match status.as_str() {
        "draft" | "published" | "archived" => Ok(status),
        _ => Err(AppError::Validation("unsupported news status".to_owned())),
    }
}

pub(crate) fn normalize_optional_news_country_code(
    value: Option<String>,
) -> AppResult<Option<String>> {
    value
        .map(|value| normalize_news_country_code(&value))
        .transpose()
}

pub(crate) fn normalize_news_country_code(value: &str) -> AppResult<String> {
    let Some(country_code) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "news country_code is required".to_owned(),
        ));
    };
    let country_code = country_code.to_ascii_uppercase();
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

pub(crate) fn validate_news_locale(value: &str) -> AppResult<String> {
    let Some(locale) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("news locale is required".to_owned()));
    };
    if locale.len() < 2
        || locale.len() > 16
        || !locale
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err(AppError::Validation(
            "news locale format is invalid".to_owned(),
        ));
    }
    Ok(locale)
}

pub(crate) fn validate_news_content_document(
    value: Value,
    default_locale: &str,
) -> AppResult<Value> {
    let object = value
        .as_object()
        .ok_or_else(|| AppError::Validation("news content must be an object".to_owned()))?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "version" | "default_locale" | "items"))
    {
        return Err(AppError::Validation(
            "news content field is unsupported".to_owned(),
        ));
    }
    if object.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(AppError::Validation(
            "news content version must be 1".to_owned(),
        ));
    }
    let content_default_locale = object
        .get("default_locale")
        .and_then(Value::as_str)
        .map(validate_news_locale)
        .transpose()?
        .ok_or_else(|| {
            AppError::Validation("news content default_locale is required".to_owned())
        })?;
    if content_default_locale != default_locale {
        return Err(AppError::Validation(
            "news content default_locale must match request default_locale".to_owned(),
        ));
    }
    let items = object
        .get("items")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or_else(|| AppError::Validation("news content items are required".to_owned()))?;
    let mut has_default_locale = false;
    let mut seen = HashSet::new();
    for item in items {
        let item_object = item.as_object().ok_or_else(|| {
            AppError::Validation("news content item must be an object".to_owned())
        })?;
        if item_object.keys().any(|key| {
            !matches!(
                key.as_str(),
                "locale" | "country_code" | "title" | "summary" | "content"
            )
        }) {
            return Err(AppError::Validation(
                "news content item field is unsupported".to_owned(),
            ));
        }
        let locale = required_news_content_string(item_object.get("locale"), "locale")?;
        let locale = validate_news_locale(locale)?;
        if locale == default_locale {
            has_default_locale = true;
        }
        let country_code =
            required_news_content_string(item_object.get("country_code"), "country_code")?;
        let country_code = normalize_news_country_code(country_code)?;
        if !seen.insert((locale, country_code)) {
            return Err(AppError::Validation(
                "news content locale and country_code must be unique".to_owned(),
            ));
        }
        validate_news_title(required_news_content_string(
            item_object.get("title"),
            "title",
        )?)?;
        if let Some(summary) = item_object.get("summary") {
            validate_news_summary(summary)?;
        }
        let content = item_object
            .get("content")
            .and_then(Value::as_array)
            .filter(|content| !content.is_empty())
            .ok_or_else(|| {
                AppError::Validation("news content item content is required".to_owned())
            })?;
        if !validate_news_rich_text_content(content)? {
            return Err(AppError::Validation(
                "news content body is required".to_owned(),
            ));
        }
    }
    if !has_default_locale {
        return Err(AppError::Validation(
            "news content default_locale must exist in items".to_owned(),
        ));
    }
    Ok(value)
}

pub(crate) fn admin_news_item_audit_json(news: &AdminNewsItemResponse) -> Value {
    json!({
        "id": news.id,
        "title": news.title,
        "banner_url": news.banner_url,
        "small_logo_url": news.small_logo_url,
        "category": news.category,
        "status": news.status,
        "country_code": news.country_code,
        "default_locale": news.default_locale,
        "published_at": news.published_at.map(|value| value.timestamp_millis()),
        "created_by_admin_id": news.created_by_admin_id,
        "updated_by_admin_id": news.updated_by_admin_id,
        "created_at": news.created_at.timestamp_millis(),
        "updated_at": news.updated_at.timestamp_millis(),
    })
}

pub(crate) fn validate_create_asset_request(request: &CreateAssetRequest) -> AppResult<()> {
    normalize_asset_symbol(&request.symbol)?;
    validate_asset_name(&request.name)?;
    validate_asset_precision(request.precision_scale)?;
    validate_optional_asset_amount(request.min_deposit_amount.as_ref(), "min_deposit_amount")?;
    validate_optional_asset_amount(request.deposit_fee.as_ref(), "deposit_fee")?;
    validate_optional_asset_amount(request.withdraw_fee.as_ref(), "withdraw_fee")?;
    validate_optional_withdraw_fee_tiers(request.withdraw_fee_tiers.as_deref())?;
    if let Some(asset_type) = request.asset_type.as_deref() {
        validate_asset_type(asset_type)?;
    }
    if let Some(status) = request.status.as_deref() {
        validate_asset_status(status)?;
    }
    Ok(())
}

pub(crate) fn validate_update_asset_request(request: &UpdateAssetRequest) -> AppResult<()> {
    validate_asset_name(&request.name)?;
    validate_asset_precision(request.precision_scale)?;
    validate_asset_type(&request.asset_type)?;
    validate_asset_status(&request.status)?;
    validate_optional_asset_amount(request.min_deposit_amount.as_ref(), "min_deposit_amount")?;
    validate_optional_asset_amount(request.deposit_fee.as_ref(), "deposit_fee")?;
    validate_optional_asset_amount(request.withdraw_fee.as_ref(), "withdraw_fee")?;
    validate_optional_withdraw_fee_tiers(request.withdraw_fee_tiers.as_deref())?;
    required_admin_audit_reason(request.reason.clone())?;
    Ok(())
}

pub(crate) fn validate_asset_fee_settings(
    min_deposit_amount: &BigDecimal,
    deposit_fee: &BigDecimal,
    withdraw_fee: &BigDecimal,
) -> AppResult<()> {
    validate_asset_amount(min_deposit_amount, "min_deposit_amount")?;
    validate_asset_amount(deposit_fee, "deposit_fee")?;
    validate_asset_amount(withdraw_fee, "withdraw_fee")
}

pub(crate) fn normalize_asset_withdraw_fee_tiers(
    tiers: Vec<WithdrawFeeTier>,
) -> AppResult<Vec<WithdrawFeeTier>> {
    normalize_withdraw_fee_tiers(tiers).map_err(AppError::Validation)
}

pub(crate) fn validate_asset_name(value: &str) -> AppResult<String> {
    let Some(name) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("asset name is required".to_owned()));
    };
    if name.len() > 128 {
        return Err(AppError::Validation(
            "asset name must be at most 128 characters".to_owned(),
        ));
    }
    Ok(name)
}

pub(crate) fn validate_asset_precision(value: i32) -> AppResult<()> {
    if !(0..=18).contains(&value) {
        return Err(AppError::Validation(
            "asset precision_scale must be between 0 and 18".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn normalize_asset_symbol(value: &str) -> AppResult<String> {
    let Some(symbol) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("asset symbol is required".to_owned()));
    };
    if symbol.len() > 32 || !symbol.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        return Err(AppError::Validation(
            "asset symbol format is invalid".to_owned(),
        ));
    }
    Ok(symbol.to_ascii_uppercase())
}

pub(crate) fn validate_asset_type(value: &str) -> AppResult<String> {
    let Some(asset_type) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("asset_type is required".to_owned()));
    };
    match asset_type.as_str() {
        "coin" | "fiat" | "stablecoin" | "platform" => Ok(asset_type),
        _ => Err(AppError::Validation("unsupported asset_type".to_owned())),
    }
}

pub(crate) fn validate_asset_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "active" | "disabled" => Ok(status),
        _ => Err(AppError::Validation("unsupported asset status".to_owned())),
    }
}

pub(crate) fn asset_audit_json(asset: &AdminAssetResponse) -> Value {
    json!({
        "id": asset.id,
        "symbol": asset.symbol,
        "name": asset.name,
        "logo_url": asset.logo_url,
        "precision_scale": asset.precision_scale,
        "asset_type": asset.asset_type,
        "status": asset.status,
        "deposit_enabled": asset.deposit_enabled,
        "withdraw_enabled": asset.withdraw_enabled,
        "min_deposit_amount": asset.min_deposit_amount,
        "deposit_fee": asset.deposit_fee,
        "withdraw_fee": asset.withdraw_fee,
        "withdraw_fee_tiers": asset.withdraw_fee_tiers.0.clone(),
        "created_at": asset.created_at.timestamp_millis(),
    })
}

pub(crate) fn normalize_deposit_network(value: &str) -> AppResult<String> {
    let Some(network) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("network is required".to_owned()));
    };
    match network.to_ascii_lowercase().as_str() {
        "eth" | "ethereum" | "erc20" => Ok("eth".to_owned()),
        "base" => Ok("base".to_owned()),
        "tron" | "trx" | "trc20" => Ok("tron".to_owned()),
        "btc" | "bitcoin" => Ok("btc".to_owned()),
        "sol" | "solana" => Ok("solana".to_owned()),
        _ => Err(AppError::Validation(
            "unsupported deposit network".to_owned(),
        )),
    }
}

pub(crate) fn validate_deposit_network_display_name(value: &str) -> AppResult<String> {
    let Some(display_name) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("display_name is required".to_owned()));
    };
    if display_name.chars().count() > 64 {
        return Err(AppError::Validation("display_name is too long".to_owned()));
    }
    Ok(display_name)
}

pub(crate) fn validate_address_group_code(value: &str) -> AppResult<String> {
    let Some(code) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "address_group_code is required".to_owned(),
        ));
    };
    if code.chars().count() > 64
        || !code
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(AppError::Validation(
            "address_group_code format is invalid".to_owned(),
        ));
    }
    Ok(code.to_ascii_uppercase())
}

pub(crate) fn validate_deposit_network_config_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "active" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "unsupported deposit network config status".to_owned(),
        )),
    }
}

pub(crate) fn validate_optional_length(
    value: Option<String>,
    field: &str,
    max_len: usize,
) -> AppResult<Option<String>> {
    let Some(value) = optional_string(value) else {
        return Ok(None);
    };
    if value.chars().count() > max_len {
        return Err(AppError::Validation(format!("{field} is too long")));
    }
    Ok(Some(value))
}

pub(crate) fn normalize_deposit_asset_symbols(
    asset_symbol: Option<String>,
    asset_symbols: Option<Vec<String>>,
) -> AppResult<Vec<String>> {
    let mut symbols = Vec::new();
    let mut seen = HashSet::new();

    if let Some(values) = asset_symbols {
        for value in values {
            let Some(raw_symbol) = optional_string(Some(value)) else {
                continue;
            };
            let symbol = normalize_asset_symbol(&raw_symbol)?;
            if seen.insert(symbol.clone()) {
                symbols.push(symbol);
            }
        }
    }

    if symbols.is_empty() {
        if let Some(raw_symbol) = optional_string(asset_symbol) {
            let symbol = normalize_asset_symbol(&raw_symbol)?;
            if seen.insert(symbol.clone()) {
                symbols.push(symbol);
            }
        }
    }

    if symbols.len() > 50 {
        return Err(AppError::Validation(
            "asset_symbols cannot contain more than 50 assets".to_owned(),
        ));
    }

    Ok(symbols)
}

pub(crate) fn deposit_network_config_audit_json(
    config: &AdminDepositNetworkConfigResponse,
) -> Value {
    json!({
        "id": config.id,
        "network": config.network,
        "display_name": config.display_name,
        "address_group_code": config.address_group_code,
        "address_group_name": config.address_group_name,
        "asset_symbols": config.asset_symbols.0.clone(),
        "status": config.status,
        "sort_order": config.sort_order,
        "created_at": config.created_at.timestamp_millis(),
        "updated_at": config.updated_at.timestamp_millis(),
    })
}

#[derive(Debug)]
pub(crate) struct NormalizedDepositAddressPoolEntry {
    pub(crate) address: String,
    pub(crate) memo: Option<String>,
    pub(crate) remark: Option<String>,
}

pub(crate) fn ensure_deposit_asset_symbols_allowed_by_network(
    asset_symbols: &[String],
    network_config: &AdminDepositNetworkConfigResponse,
) -> AppResult<()> {
    if network_config.status != "active" {
        return Err(AppError::Validation(
            "deposit network config is disabled".to_owned(),
        ));
    }
    if asset_symbols.is_empty() || network_config.asset_symbols.0.is_empty() {
        return Ok(());
    }

    let allowed = network_config
        .asset_symbols
        .0
        .iter()
        .map(|symbol| symbol.to_ascii_uppercase())
        .collect::<HashSet<_>>();
    let unsupported = asset_symbols
        .iter()
        .find(|symbol| !allowed.contains(symbol.as_str()));
    if let Some(symbol) = unsupported {
        return Err(AppError::Validation(format!(
            "asset {symbol} does not support deposit network {}",
            network_config.network
        )));
    }
    Ok(())
}

pub(crate) fn resolve_deposit_address_group_code(
    requested_group_code: Option<String>,
    network_config: &AdminDepositNetworkConfigResponse,
) -> AppResult<String> {
    let configured_group_code = validate_address_group_code(&network_config.address_group_code)?;
    let Some(requested_group_code) = requested_group_code else {
        return Ok(configured_group_code);
    };
    let requested_group_code = validate_address_group_code(&requested_group_code)?;
    if requested_group_code != configured_group_code {
        return Err(AppError::Validation(
            "address_group_code must match deposit network config".to_owned(),
        ));
    }
    Ok(requested_group_code)
}

pub(crate) fn validate_deposit_address(value: &str) -> AppResult<String> {
    let Some(address) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("address is required".to_owned()));
    };
    if address.chars().count() > 255 {
        return Err(AppError::Validation("address is too long".to_owned()));
    }
    Ok(address)
}

pub(crate) fn normalize_deposit_address_batch_entries(
    entries: Vec<CreateDepositAddressPoolEntryRequest>,
) -> AppResult<Vec<NormalizedDepositAddressPoolEntry>> {
    if entries.is_empty() {
        return Err(AppError::Validation(
            "at least one deposit address is required".to_owned(),
        ));
    }
    if entries.len() > 100 {
        return Err(AppError::Validation(
            "a single batch cannot contain more than 100 deposit addresses".to_owned(),
        ));
    }

    let mut normalized_entries = Vec::with_capacity(entries.len());
    let mut seen = HashSet::new();
    for entry in entries {
        let address = validate_deposit_address(&entry.address)?;
        if !seen.insert(address.clone()) {
            return Err(AppError::Validation(
                "duplicate deposit address in batch".to_owned(),
            ));
        }
        normalized_entries.push(NormalizedDepositAddressPoolEntry {
            address,
            memo: validate_optional_length(entry.memo, "memo", 255)?,
            remark: validate_optional_length(entry.remark, "remark", 512)?,
        });
    }

    Ok(normalized_entries)
}

pub(crate) fn validate_deposit_address_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "available" | "assigned" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "unsupported deposit address status".to_owned(),
        )),
    }
}

pub(crate) fn validate_deposit_address_assignable_status(value: &str) -> AppResult<String> {
    let status = validate_deposit_address_status(value)?;
    match status.as_str() {
        "available" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "assigned status is managed by user allocation".to_owned(),
        )),
    }
}

pub(crate) fn deposit_address_pool_audit_json(address: &AdminDepositAddressPoolResponse) -> Value {
    json!({
        "id": address.id,
        "network": address.network,
        "address_group_code": address.address_group_code,
        "address": address.address,
        "asset_symbol": address.asset_symbol,
        "asset_symbols": address.asset_symbols.0.clone(),
        "status": address.status,
        "assigned_user_id": address.assigned_user_id,
        "assigned_user_email": address.assigned_user_email,
        "assigned_asset_symbol": address.assigned_asset_symbol,
        "assigned_at": address.assigned_at.map(|value| value.timestamp_millis()),
        "memo": address.memo,
        "remark": address.remark,
        "created_at": address.created_at.timestamp_millis(),
        "updated_at": address.updated_at.timestamp_millis(),
    })
}

pub(crate) fn validate_create_trading_pair_request(
    request: &CreateTradingPairRequest,
) -> AppResult<()> {
    if request.base_asset_id == request.quote_asset_id {
        return Err(AppError::Validation(
            "trading pair assets must be different".to_owned(),
        ));
    }
    normalize_trading_pair_symbol(&request.symbol)?;
    validate_trading_pair_config(
        request.price_precision,
        request.qty_precision,
        &request.min_order_value,
    )?;
    if let Some(status) = request.status.as_deref() {
        validate_trading_pair_status(status)?;
    }
    if let Some(market_type) = request.market_type.as_deref() {
        validate_trading_pair_market_type(market_type)?;
    }
    Ok(())
}

pub(crate) fn validate_update_trading_pair_request(
    request: &UpdateTradingPairRequest,
) -> AppResult<()> {
    validate_trading_pair_config(
        request.price_precision,
        request.qty_precision,
        &request.min_order_value,
    )?;
    validate_trading_pair_status(&request.status)?;
    validate_trading_pair_market_type(&request.market_type)?;
    Ok(())
}

pub(crate) fn normalize_trading_pair_symbol(value: &str) -> AppResult<String> {
    let Some(value) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("symbol is required".to_owned()));
    };
    if value.len() > 64
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '/'))
    {
        return Err(AppError::Validation(
            "trading pair symbol format is invalid".to_owned(),
        ));
    }
    Ok(value.to_ascii_uppercase().replace(['_', '/'], "-"))
}

pub(crate) fn validate_trading_pair_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "active" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "unsupported trading pair status".to_owned(),
        )),
    }
}

pub(crate) fn validate_trading_pair_market_type(value: &str) -> AppResult<String> {
    let Some(market_type) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("market_type is required".to_owned()));
    };
    match market_type.as_str() {
        "external" | "internal" | "strategy" => Ok(market_type),
        _ => Err(AppError::Validation(
            "unsupported trading pair market_type".to_owned(),
        )),
    }
}

pub(crate) fn trading_pair_audit_json(pair: &AdminTradingPairResponse) -> Value {
    json!({
        "id": pair.id,
        "base_asset_id": pair.base_asset_id,
        "quote_asset_id": pair.quote_asset_id,
        "symbol": pair.symbol,
        "logo_url": pair.logo_url,
        "base_asset": pair.base_asset,
        "quote_asset": pair.quote_asset,
        "price_precision": pair.price_precision,
        "qty_precision": pair.qty_precision,
        "min_order_value": pair.min_order_value,
        "status": pair.status,
        "market_type": pair.market_type,
        "created_at": pair.created_at.timestamp_millis(),
    })
}

pub(crate) fn validate_create_market_strategy(
    request: &CreateMarketStrategyRequest,
) -> AppResult<()> {
    if request.pair_id == 0 {
        return Err(AppError::Validation("pair_id is required".to_owned()));
    }
    validate_market_strategy_config(MarketStrategyConfigValidation {
        strategy_type: &request.strategy_type,
        start_price: &request.start_price,
        target_price: &request.target_price,
        start_time: request.start_time,
        end_time: request.end_time,
        volatility: &request.volatility,
        volume_min: &request.volume_min,
        volume_max: &request.volume_max,
    })?;
    if let Some(status) = request.status.as_deref() {
        validate_market_strategy_status(status)?;
    }
    Ok(())
}

pub(crate) fn validate_update_market_strategy(
    request: &UpdateMarketStrategyRequest,
) -> AppResult<()> {
    validate_market_strategy_config(MarketStrategyConfigValidation {
        strategy_type: &request.strategy_type,
        start_price: &request.start_price,
        target_price: &request.target_price,
        start_time: request.start_time,
        end_time: request.end_time,
        volatility: &request.volatility,
        volume_min: &request.volume_min,
        volume_max: &request.volume_max,
    })
}

struct MarketStrategyConfigValidation<'a> {
    strategy_type: &'a str,
    start_price: &'a BigDecimal,
    target_price: &'a BigDecimal,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    volatility: &'a BigDecimal,
    volume_min: &'a BigDecimal,
    volume_max: &'a BigDecimal,
}

fn validate_market_strategy_config(config: MarketStrategyConfigValidation<'_>) -> AppResult<()> {
    if optional_string(Some(config.strategy_type.to_owned())).is_none() {
        return Err(AppError::Validation("strategy_type is required".to_owned()));
    }
    if config.start_price <= &BigDecimal::from(0) || config.target_price <= &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "strategy prices must be positive".to_owned(),
        ));
    }
    if config.end_time <= config.start_time {
        return Err(AppError::Validation(
            "end_time must be after start_time".to_owned(),
        ));
    }
    if config.volatility < &BigDecimal::from(0)
        || config.volume_min < &BigDecimal::from(0)
        || config.volume_max < &BigDecimal::from(0)
    {
        return Err(AppError::Validation(
            "volatility and volume must be non-negative".to_owned(),
        ));
    }
    if config.volume_max < config.volume_min {
        return Err(AppError::Validation(
            "volume_max must be greater than or equal to volume_min".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_market_strategy_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "draft" | "active" | "paused" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "unsupported market strategy status".to_owned(),
        )),
    }
}

pub(crate) fn market_strategy_run_status(status: &str) -> &'static str {
    match status {
        "active" => "running",
        "paused" => "paused",
        "disabled" => "stopped",
        _ => "draft",
    }
}

pub(crate) fn market_strategy_config_json(
    request: &CreateMarketStrategyRequest,
    status: &str,
    market_type: &str,
) -> Value {
    market_strategy_config_value(MarketStrategyConfigValue {
        pair_id: Some(request.pair_id),
        market_type,
        strategy_type: request.strategy_type.trim(),
        start_price: &request.start_price,
        target_price: &request.target_price,
        start_time: request.start_time,
        end_time: request.end_time,
        volatility: &request.volatility,
        volume_min: &request.volume_min,
        volume_max: &request.volume_max,
        status,
    })
}

pub(crate) fn market_strategy_update_config_json(
    request: &UpdateMarketStrategyRequest,
    status: &str,
    market_type: &str,
) -> Value {
    market_strategy_config_value(MarketStrategyConfigValue {
        pair_id: None,
        market_type,
        strategy_type: request.strategy_type.trim(),
        start_price: &request.start_price,
        target_price: &request.target_price,
        start_time: request.start_time,
        end_time: request.end_time,
        volatility: &request.volatility,
        volume_min: &request.volume_min,
        volume_max: &request.volume_max,
        status,
    })
}

struct MarketStrategyConfigValue<'a> {
    pair_id: Option<u64>,
    market_type: &'a str,
    strategy_type: &'a str,
    start_price: &'a BigDecimal,
    target_price: &'a BigDecimal,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    volatility: &'a BigDecimal,
    volume_min: &'a BigDecimal,
    volume_max: &'a BigDecimal,
    status: &'a str,
}

fn market_strategy_config_value(config: MarketStrategyConfigValue<'_>) -> Value {
    let mut value = json!({
        "market_type": config.market_type,
        "strategy_type": config.strategy_type,
        "start_price": config.start_price,
        "target_price": config.target_price,
        "start_time": config.start_time.timestamp_millis(),
        "end_time": config.end_time.timestamp_millis(),
        "volatility": config.volatility,
        "volume_min": config.volume_min,
        "volume_max": config.volume_max,
        "status": config.status,
    });
    if let Some(pair_id) = config.pair_id {
        value["pair_id"] = json!(pair_id);
    }
    value
}

pub(crate) fn market_strategy_audit_json(strategy: &AdminMarketStrategyResponse) -> Value {
    json!({
        "id": strategy.id,
        "pair_id": strategy.pair_id,
        "symbol": strategy.symbol,
        "market_type": strategy.market_type,
        "strategy_type": strategy.strategy_type,
        "start_price": strategy.start_price,
        "target_price": strategy.target_price,
        "start_time": strategy.start_time.timestamp_millis(),
        "end_time": strategy.end_time.timestamp_millis(),
        "volatility": strategy.volatility,
        "volume_min": strategy.volume_min,
        "volume_max": strategy.volume_max,
        "status": strategy.status,
        "run_status": strategy.run_status,
        "current_price": strategy.current_price,
        "last_generated_at": strategy.last_generated_at.map(|value| value.timestamp_millis()),
        "last_kline_open_time": strategy.last_kline_open_time.map(|value| value.timestamp_millis()),
        "recovery_status": strategy.recovery_status,
        "created_at": strategy.created_at.timestamp_millis(),
    })
}

pub(crate) fn validate_create_convert_pair(request: &CreateConvertPairRequest) -> AppResult<()> {
    let zero = BigDecimal::from(0);
    let fee_rate = request.fee_rate.as_ref().unwrap_or(&zero);
    let target_min_amount = request
        .target_min_amount
        .as_ref()
        .unwrap_or(&request.min_amount);
    let target_max_amount = request
        .target_max_amount
        .as_ref()
        .or(request.max_amount.as_ref());

    validate_convert_pair_values(
        request.from_asset_id,
        request.to_asset_id,
        &request.pricing_mode,
        &request.spread_rate,
        fee_rate,
        &request.min_amount,
        request.max_amount.as_ref(),
        target_min_amount,
        target_max_amount,
    )
}

pub(crate) fn validate_convert_pair_values(
    from_asset_id: u64,
    to_asset_id: u64,
    pricing_mode: &str,
    spread_rate: &BigDecimal,
    fee_rate: &BigDecimal,
    min_amount: &BigDecimal,
    max_amount: Option<&BigDecimal>,
    target_min_amount: &BigDecimal,
    target_max_amount: Option<&BigDecimal>,
) -> AppResult<()> {
    if from_asset_id == to_asset_id {
        return Err(AppError::Validation(
            "convert pair assets must be different".to_owned(),
        ));
    }
    if optional_string(Some(pricing_mode.to_owned())).is_none() {
        return Err(AppError::Validation("pricing_mode is required".to_owned()));
    }
    let zero = BigDecimal::from(0);
    if min_amount < &zero {
        return Err(AppError::Validation(
            "min_amount must be non-negative".to_owned(),
        ));
    }
    if spread_rate < &zero {
        return Err(AppError::Validation(
            "spread_rate must be non-negative".to_owned(),
        ));
    }
    if fee_rate < &zero || fee_rate >= &BigDecimal::from(1) {
        return Err(AppError::Validation(
            "fee_rate must be greater than or equal to 0 and less than 1".to_owned(),
        ));
    }
    if let Some(max_amount) = max_amount
        && max_amount < min_amount
    {
        return Err(AppError::Validation(
            "max_amount must be greater than or equal to min_amount".to_owned(),
        ));
    }
    if target_min_amount < &zero {
        return Err(AppError::Validation(
            "target_min_amount must be non-negative".to_owned(),
        ));
    }
    if let Some(target_max_amount) = target_max_amount
        && target_max_amount < target_min_amount
    {
        return Err(AppError::Validation(
            "target_max_amount must be greater than or equal to target_min_amount".to_owned(),
        ));
    }

    Ok(())
}

pub(crate) fn convert_pair_audit_json(pair: &ConvertPairResponse) -> Value {
    json!({
        "id": pair.id,
        "from_asset_id": pair.from_asset_id,
        "from_asset_symbol": pair.from_asset_symbol,
        "to_asset_id": pair.to_asset_id,
        "to_asset_symbol": pair.to_asset_symbol,
        "pricing_mode": pair.pricing_mode,
        "spread_rate": pair.spread_rate,
        "fee_rate": pair.fee_rate,
        "min_amount": pair.min_amount,
        "max_amount": pair.max_amount,
        "target_min_amount": pair.target_min_amount,
        "target_max_amount": pair.target_max_amount,
        "enabled": pair.enabled,
    })
}

pub(crate) fn validate_distribute_new_coin(request: &DistributeNewCoinRequest) -> AppResult<()> {
    if request.quantity <= 0 {
        return Err(AppError::Validation("quantity must be positive".to_owned()));
    }
    if optional_string(Some(request.idempotency_key.clone())).is_none() {
        return Err(AppError::Validation(
            "idempotency_key must not be empty".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_update_new_coin_unlock_rule(
    request: &UpdateNewCoinUnlockRuleRequest,
) -> AppResult<()> {
    validate_unlock_rule_shape(
        &request.unlock_type,
        request.listed_at,
        request.fixed_unlock_at,
        request.relative_unlock_seconds,
    )
}

pub(crate) fn validate_update_new_coin_unlock_fee_rule(
    request: &UpdateNewCoinUnlockFeeRuleRequest,
) -> AppResult<()> {
    validate_unlock_fee_rule_shape(
        request.unlock_fee_enabled,
        request.unlock_fee_rate.as_ref(),
        request.unlock_fee_basis.clone(),
        request.unlock_fee_asset,
    )
}

pub(crate) fn validate_update_new_coin_post_listing_purchase(
    request: &UpdateNewCoinPostListingPurchaseRequest,
) -> AppResult<()> {
    if request.enabled && request.pair_id.unwrap_or(0) == 0 {
        return Err(AppError::Validation(
            "pair_id is required when post-listing purchase is enabled".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_create_new_coin_project(
    request: &CreateNewCoinProjectRequest,
) -> AppResult<()> {
    let Some(lifecycle_status) = optional_string(Some(request.lifecycle_status.clone())) else {
        return Err(AppError::Validation(
            "lifecycle_status is required".to_owned(),
        ));
    };
    parse_lifecycle_status_from_request(&lifecycle_status)?;
    if request.total_supply <= 0 {
        return Err(AppError::Validation(
            "total_supply must be positive".to_owned(),
        ));
    }
    if request.issue_price < 0 {
        return Err(AppError::Validation(
            "issue_price must be non-negative".to_owned(),
        ));
    }
    if optional_string(Some(request.symbol.clone())).is_none() {
        return Err(AppError::Validation("symbol is required".to_owned()));
    }
    validate_unlock_rule_shape(
        &request.unlock_type,
        request.listed_at,
        request.fixed_unlock_at,
        request.relative_unlock_seconds,
    )?;
    validate_unlock_fee_rule_shape(
        request.unlock_fee_enabled.unwrap_or(false),
        request.unlock_fee_rate.as_ref(),
        request.unlock_fee_basis.clone(),
        request.unlock_fee_asset,
    )?;

    Ok(())
}

pub(crate) fn validate_new_coin_convert_rule(
    request: &UpsertNewCoinConvertRuleRequest,
) -> AppResult<()> {
    let Some(rate_source) = optional_string(Some(request.rate_source.clone())) else {
        return Err(AppError::Validation("rate_source is required".to_owned()));
    };
    if rate_source != "fixed" {
        return Err(AppError::Validation(
            "only fixed rate_source is supported for new coin convert rules".to_owned(),
        ));
    }
    if request.fixed_rate.is_none() {
        return Err(AppError::Validation(
            "fixed_rate is required for fixed rate_source".to_owned(),
        ));
    }
    if let Some(fixed_rate) = &request.fixed_rate
        && fixed_rate <= 0
    {
        return Err(AppError::Validation(
            "fixed_rate must be positive".to_owned(),
        ));
    }
    if optional_string(request.status.clone()).is_none() && request.status.is_some() {
        return Err(AppError::Validation("status is required".to_owned()));
    }

    Ok(())
}

pub(crate) fn ensure_post_listing_purchase_lifecycle(
    project: &NewCoinProjectResponse,
) -> AppResult<()> {
    if parse_lifecycle_status_from_db(&project.lifecycle_status)? != LifecycleStatus::Listed {
        return Err(AppError::Validation(
            "post-listing purchase can only be configured for listed projects".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn ensure_distribution_lifecycle(project: &NewCoinProjectResponse) -> AppResult<()> {
    if parse_lifecycle_status_from_db(&project.lifecycle_status)? != LifecycleStatus::Distribution {
        return Err(AppError::Validation(
            "new coin project must be in distribution lifecycle before distribution".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn parse_lifecycle_status_from_request(value: &str) -> AppResult<LifecycleStatus> {
    let Some(value) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "lifecycle_status is required".to_owned(),
        ));
    };
    parse_lifecycle_status(&value)
}

pub(crate) fn parse_lifecycle_status_from_db(value: &str) -> AppResult<LifecycleStatus> {
    parse_lifecycle_status(value).map_err(|_| {
        AppError::Internal(format!(
            "stored new coin lifecycle_status is unsupported: {value}"
        ))
    })
}

pub(crate) fn lifecycle_status_value(status: LifecycleStatus) -> &'static str {
    match status {
        LifecycleStatus::Preheat => "preheat",
        LifecycleStatus::Subscription => "subscription",
        LifecycleStatus::Distribution => "distribution",
        LifecycleStatus::Listed => "listed",
    }
}

pub(crate) fn lock_positions_for_distribution(
    project: &NewCoinProjectResponse,
    user_id: u64,
    asset_id: u64,
    source_id: &str,
    quantity: BigDecimal,
    source_time: DateTime<Utc>,
) -> AppResult<Vec<AdminNewCoinLockPositionWrite>> {
    let unlock_rule = unlock_rule_from_project(project)?;
    let application = apply_unlock_rule(
        &unlock_rule,
        vec![UnlockSource {
            user_id: user_id.to_string(),
            asset_id: asset_id.to_string(),
            source_id: source_id.to_owned(),
            amount: quantity,
            source_time,
        }],
    )
    .map_err(|error| AppError::Validation(format!("invalid new coin unlock rule: {error:?}")))?;

    Ok(application
        .lock_positions
        .into_iter()
        .map(|position| AdminNewCoinLockPositionWrite {
            user_id,
            asset_id,
            unlock_type: position.unlock_type,
            unlock_at: position.unlock_at,
            amount: position.remaining_amount,
            merge_key: position.merge_key,
            source_time,
            source_type: "new_coin_distribution".to_owned(),
            source_id: source_id.to_owned(),
        })
        .collect())
}

pub(crate) fn new_coin_project_audit_json(project: &NewCoinProjectResponse) -> Value {
    json!({
        "id": project.id,
        "asset_id": project.asset_id,
        "symbol": project.symbol,
        "lifecycle_status": project.lifecycle_status,
        "total_supply": project.total_supply,
        "issue_price": project.issue_price,
        "listed_at": project.listed_at.map(|value| value.timestamp_millis()),
        "unlock_type": project.unlock_type,
        "fixed_unlock_at": project.fixed_unlock_at.map(|value| value.timestamp_millis()),
        "relative_unlock_seconds": project.relative_unlock_seconds,
        "unlock_fee_enabled": project.unlock_fee_enabled,
        "unlock_fee_rate": project.unlock_fee_rate,
        "unlock_fee_basis": project.unlock_fee_basis,
        "unlock_fee_asset": project.unlock_fee_asset,
        "status": project.status,
        "post_listing_purchase_enabled": project.post_listing_purchase_enabled,
        "post_listing_pair_id": project.post_listing_pair_id,
        "post_listing_pair_status": project.post_listing_pair_status,
    })
}

pub(crate) fn new_coin_distribution_audit_json(
    distribution: &NewCoinDistributionResponse,
) -> Value {
    json!({
        "id": distribution.id,
        "project_id": distribution.project_id,
        "user_id": distribution.user_id,
        "subscription_id": distribution.subscription_id,
        "asset_id": distribution.asset_id,
        "quantity": distribution.quantity,
        "lock_position_id": distribution.lock_position_id,
        "status": distribution.status,
        "idempotency_key": distribution.idempotency_key,
        "created_at": distribution.created_at.timestamp_millis(),
    })
}

pub(crate) fn new_coin_convert_rule_audit_json(rule: &NewCoinConvertRuleResponse) -> Value {
    json!({
        "id": rule.id,
        "convert_pair_id": rule.convert_pair_id,
        "rate_source": rule.rate_source,
        "fixed_rate": rule.fixed_rate,
        "floating_rate_json": rule.floating_rate_json.as_ref().map(|value| &value.0),
        "status": rule.status,
        "created_by": rule.created_by,
    })
}

fn validate_unlock_rule_shape(
    unlock_type: &str,
    listed_at: Option<DateTime<Utc>>,
    fixed_unlock_at: Option<DateTime<Utc>>,
    relative_unlock_seconds: Option<u64>,
) -> AppResult<()> {
    match optional_string(Some(unlock_type.to_owned())).as_deref() {
        Some("immediate_on_listing") => {
            if listed_at.is_none() {
                return Err(AppError::Validation(
                    "listed_at is required for immediate_on_listing unlock".to_owned(),
                ));
            }
            if fixed_unlock_at.is_some() || relative_unlock_seconds.is_some() {
                return Err(AppError::Validation(
                    "immediate_on_listing unlock cannot include fixed or relative unlock fields"
                        .to_owned(),
                ));
            }
        }
        Some("fixed_time") => {
            if fixed_unlock_at.is_none() {
                return Err(AppError::Validation(
                    "fixed_unlock_at is required for fixed_time unlock".to_owned(),
                ));
            }
            if listed_at.is_some() || relative_unlock_seconds.is_some() {
                return Err(AppError::Validation(
                    "fixed_time unlock cannot include listed_at or relative_unlock_seconds"
                        .to_owned(),
                ));
            }
        }
        Some("relative_period") => {
            if relative_unlock_seconds.unwrap_or(0) == 0 {
                return Err(AppError::Validation(
                    "relative_unlock_seconds is required for relative_period unlock".to_owned(),
                ));
            }
            if listed_at.is_some() || fixed_unlock_at.is_some() {
                return Err(AppError::Validation(
                    "relative_period unlock cannot include listed_at or fixed_unlock_at".to_owned(),
                ));
            }
        }
        Some(_) => {
            return Err(AppError::Validation(
                "unsupported new coin unlock_type".to_owned(),
            ));
        }
        None => return Err(AppError::Validation("unlock_type is required".to_owned())),
    }

    Ok(())
}

fn validate_unlock_fee_rule_shape(
    unlock_fee_enabled: bool,
    unlock_fee_rate: Option<&BigDecimal>,
    unlock_fee_basis: Option<String>,
    unlock_fee_asset: Option<u64>,
) -> AppResult<()> {
    if !unlock_fee_enabled {
        return Ok(());
    }
    if unlock_fee_rate.is_none_or(|rate| rate <= 0) {
        return Err(AppError::Validation(
            "unlock_fee_rate must be positive when unlock fee is enabled".to_owned(),
        ));
    }
    match optional_string(unlock_fee_basis).as_deref() {
        Some("market_value" | "profit") => {}
        Some(_) => {
            return Err(AppError::Validation(
                "unsupported unlock_fee_basis".to_owned(),
            ));
        }
        None => {
            return Err(AppError::Validation(
                "unlock_fee_basis is required when unlock fee is enabled".to_owned(),
            ));
        }
    }
    if unlock_fee_asset.is_none() {
        return Err(AppError::Validation(
            "unlock_fee_asset is required when unlock fee is enabled".to_owned(),
        ));
    }

    Ok(())
}

fn parse_lifecycle_status(value: &str) -> AppResult<LifecycleStatus> {
    match value {
        "preheat" => Ok(LifecycleStatus::Preheat),
        "subscription" => Ok(LifecycleStatus::Subscription),
        "distribution" => Ok(LifecycleStatus::Distribution),
        "listed" => Ok(LifecycleStatus::Listed),
        _ => Err(AppError::Validation(
            "unsupported new coin lifecycle_status".to_owned(),
        )),
    }
}

fn unlock_rule_from_project(project: &NewCoinProjectResponse) -> AppResult<UnlockRule> {
    match project.unlock_type.as_str() {
        "immediate_on_listing" => Ok(UnlockRule::ImmediateOnListing {
            listed_at: project.listed_at.ok_or_else(|| {
                AppError::Validation("listed_at is required for immediate unlock".to_owned())
            })?,
        }),
        "fixed_time" => Ok(UnlockRule::FixedTime {
            unlock_at: project.fixed_unlock_at.ok_or_else(|| {
                AppError::Validation("fixed_unlock_at is required for fixed unlock".to_owned())
            })?,
        }),
        "relative_period" => Ok(UnlockRule::RelativePeriod {
            seconds_after_source: project
                .relative_unlock_seconds
                .ok_or_else(|| {
                    AppError::Validation(
                        "relative_unlock_seconds is required for relative unlock".to_owned(),
                    )
                })?
                .try_into()
                .map_err(|_| {
                    AppError::Validation("relative unlock period is too large".to_owned())
                })?,
        }),
        _ => Err(AppError::Validation(
            "unsupported new coin unlock_type".to_owned(),
        )),
    }
}

fn validate_trading_pair_config(
    price_precision: i32,
    qty_precision: i32,
    min_order_value: &BigDecimal,
) -> AppResult<()> {
    if price_precision < 0 || qty_precision < 0 {
        return Err(AppError::Validation(
            "trading pair precision must be non-negative".to_owned(),
        ));
    }
    if min_order_value <= &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "min_order_value must be positive".to_owned(),
        ));
    }
    Ok(())
}

fn validate_optional_withdraw_fee_tiers(value: Option<&[WithdrawFeeTier]>) -> AppResult<()> {
    if let Some(tiers) = value {
        normalize_asset_withdraw_fee_tiers(tiers.to_vec())?;
    }
    Ok(())
}

fn validate_optional_asset_amount(value: Option<&BigDecimal>, field: &str) -> AppResult<()> {
    if let Some(value) = value {
        validate_asset_amount(value, field)?;
    }
    Ok(())
}

fn validate_asset_amount(value: &BigDecimal, field: &str) -> AppResult<()> {
    if value < &BigDecimal::from(0) {
        return Err(AppError::Validation(format!(
            "{field} must be non-negative"
        )));
    }
    Ok(())
}

fn validate_news_rich_text_content(content: &[Value]) -> AppResult<bool> {
    let mut has_content = false;
    for node in content {
        has_content = validate_news_rich_text_block(node)? || has_content;
    }
    Ok(has_content)
}

fn validate_news_summary(value: &Value) -> AppResult<()> {
    if value.is_null() {
        return Ok(());
    }
    if let Some(summary) = value.as_str() {
        if summary.chars().count() > 512 {
            return Err(AppError::Validation(
                "news content summary is too long".to_owned(),
            ));
        }
        return Ok(());
    }
    let summary = value
        .as_array()
        .filter(|summary| !summary.is_empty())
        .ok_or_else(|| {
            AppError::Validation("news content summary must be a string or rich text".to_owned())
        })?;
    validate_news_rich_text_content(summary)?;
    if news_rich_text_text_length(summary) > 512 {
        return Err(AppError::Validation(
            "news content summary is too long".to_owned(),
        ));
    }
    Ok(())
}

fn news_rich_text_text_length(content: &[Value]) -> usize {
    content
        .iter()
        .filter_map(Value::as_object)
        .filter_map(|object| object.get("children"))
        .filter_map(Value::as_array)
        .flat_map(|children| children.iter())
        .filter_map(Value::as_object)
        .filter_map(|leaf| leaf.get("text"))
        .filter_map(Value::as_str)
        .map(|text| text.chars().count())
        .sum()
}

fn validate_news_rich_text_block(node: &Value) -> AppResult<bool> {
    let object = node
        .as_object()
        .ok_or_else(invalid_news_rich_text_content)?;
    let node_type = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(invalid_news_rich_text_content)?;
    if node_type == "image" {
        return validate_news_rich_text_image_block(object);
    }
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "type" | "children"))
    {
        return Err(invalid_news_rich_text_content());
    }
    if !matches!(node_type, "p" | "h1" | "h2" | "h3" | "blockquote") {
        return Err(invalid_news_rich_text_content());
    }
    let children = object
        .get("children")
        .and_then(Value::as_array)
        .filter(|children| !children.is_empty())
        .ok_or_else(invalid_news_rich_text_content)?;
    let mut has_text = false;
    for child in children {
        has_text = validate_news_rich_text_child(child)? || has_text;
    }
    Ok(has_text)
}

fn validate_news_rich_text_image_block(object: &serde_json::Map<String, Value>) -> AppResult<bool> {
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "type" | "url" | "alt"))
    {
        return Err(invalid_news_rich_text_content());
    }
    let url = required_news_content_string(object.get("url"), "image url")?;
    validate_optional_image_url(Some(url.to_owned()), "news content image url")?;
    if let Some(alt) = object.get("alt") {
        let alt = alt.as_str().ok_or_else(invalid_news_rich_text_content)?;
        if alt.chars().count() > 256 {
            return Err(invalid_news_rich_text_content());
        }
    }
    Ok(true)
}

fn validate_news_rich_text_child(node: &Value) -> AppResult<bool> {
    let object = node
        .as_object()
        .ok_or_else(invalid_news_rich_text_content)?;
    let text = object
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(invalid_news_rich_text_content)?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "text" | "bold" | "italic" | "underline"))
    {
        return Err(invalid_news_rich_text_content());
    }
    for mark in ["bold", "italic", "underline"] {
        if let Some(value) = object.get(mark)
            && !value.is_boolean()
        {
            return Err(invalid_news_rich_text_content());
        }
    }
    Ok(!text.trim().is_empty())
}

fn invalid_news_rich_text_content() -> AppError {
    AppError::Validation("news content node is invalid".to_owned())
}

fn required_news_content_string<'a>(value: Option<&'a Value>, field: &str) -> AppResult<&'a str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation(format!("news content {field} is required")))
}

fn validate_smtp_config_name(value: Option<String>, fallback: Option<&str>) -> AppResult<String> {
    let name =
        optional_string(value).or_else(|| fallback.and_then(optional_str).map(str::to_owned));
    let Some(name) = name else {
        return Err(AppError::Validation(
            "smtp config name is required".to_owned(),
        ));
    };
    if name.len() > 64 {
        return Err(AppError::Validation(
            "smtp config name is too long".to_owned(),
        ));
    }
    Ok(name)
}

fn validate_smtp_verification_code_templates(
    templates: Option<Vec<VerificationCodeTemplate>>,
) -> AppResult<Vec<VerificationCodeTemplate>> {
    let Some(templates) = templates else {
        return Ok(Vec::new());
    };
    if templates.len() > 20 {
        return Err(AppError::Validation(
            "verification_code_templates cannot exceed 20 templates".to_owned(),
        ));
    }

    let mut keys = HashSet::new();
    templates
        .into_iter()
        .map(|template| {
            let key = optional_string(Some(template.key)).ok_or_else(|| {
                AppError::Validation("verification_code_template key is required".to_owned())
            })?;
            if key.len() > 64 {
                return Err(AppError::Validation(
                    "verification_code_template key is too long".to_owned(),
                ));
            }
            if !keys.insert(key.clone()) {
                return Err(AppError::Validation(
                    "verification_code_template key must be unique".to_owned(),
                ));
            }

            let name = optional_string(Some(template.name)).ok_or_else(|| {
                AppError::Validation("verification_code_template name is required".to_owned())
            })?;
            if name.len() > 128 {
                return Err(AppError::Validation(
                    "verification_code_template name is too long".to_owned(),
                ));
            }

            let purpose = optional_string(template.purpose)
                .filter(|purpose| purpose != "default")
                .map(|purpose| {
                    if purpose.len() > 64 {
                        return Err(AppError::Validation(
                            "verification_code_template purpose is too long".to_owned(),
                        ));
                    }
                    Ok(purpose)
                })
                .transpose()?;

            let html = optional_string(Some(template.html)).ok_or_else(|| {
                AppError::Validation("verification_code_template html is required".to_owned())
            })?;
            if html.len() > 20_000 {
                return Err(AppError::Validation(
                    "verification_code_template html is too long".to_owned(),
                ));
            }

            Ok(VerificationCodeTemplate {
                key,
                name,
                purpose,
                html,
                enabled: template.enabled,
            })
        })
        .collect()
}

fn optional_str(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 从 admin 认证 subject 中提取管理员 ID。
pub(crate) fn admin_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("admin:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}
