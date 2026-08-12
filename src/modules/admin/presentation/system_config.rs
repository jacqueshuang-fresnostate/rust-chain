//! 后台 SMTP、上传配置及 multipart 文件输入适配 DTO。

use super::*;

const MULTIPART_UPLOAD_FILE_FIELD: &str = "file";

#[derive(Debug, Deserialize)]
pub struct SaveSmtpConfigRequest {
    pub name: Option<String>,
    pub host: String,
    pub port: u16,
    pub security: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub from_email: String,
    pub from_name: Option<String>,
    pub verification_code_template_html: Option<String>,
    pub verification_code_templates: Option<Vec<VerificationCodeTemplate>>,
    pub enabled: bool,
    pub priority: Option<u32>,
    pub reason: Option<String>,
}

impl PresentationLayer for SaveSmtpConfigRequest {}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct SmtpConfigResponse {
    pub id: u64,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub security: String,
    pub username_mask: Option<String>,
    pub password_set: bool,
    pub from_email: String,
    pub from_name: Option<String>,
    pub verification_code_template_html: Option<String>,
    pub verification_code_templates: Vec<VerificationCodeTemplate>,
    pub enabled: bool,
    pub priority: u32,
}

impl PresentationLayer for SmtpConfigResponse {}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct SmtpDeliverySettingsResponse {
    pub strategy: String,
}

impl PresentationLayer for SmtpDeliverySettingsResponse {}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct SmtpConfigListResponse {
    pub configs: Vec<SmtpConfigResponse>,
    pub delivery_settings: SmtpDeliverySettingsResponse,
}

impl PresentationLayer for SmtpConfigListResponse {}

#[derive(Debug, Deserialize)]
pub struct SaveSmtpDeliverySettingsRequest {
    pub strategy: String,
    pub reason: Option<String>,
}

impl PresentationLayer for SaveSmtpDeliverySettingsRequest {}

#[derive(Debug, Deserialize)]
pub struct SendSmtpTestRequest {
    pub recipient: String,
    pub config_id: Option<u64>,
    pub reason: Option<String>,
}

impl PresentationLayer for SendSmtpTestRequest {}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct SendSmtpTestResponse {
    pub sent: bool,
    pub recipient: String,
    pub config_id: u64,
    pub config_name: String,
}

impl PresentationLayer for SendSmtpTestResponse {}

#[derive(Debug, Deserialize)]
pub struct SaveUploadConfigRequest {
    pub provider: String,
    pub endpoint: Option<String>,
    pub file_field: Option<String>,
    pub bearer_token: Option<String>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
    pub bucket: Option<String>,
    pub region: Option<String>,
    pub public_base_url: Option<String>,
    pub local_root: Option<String>,
    pub key_prefix: Option<String>,
    pub max_file_size_bytes: Option<u64>,
    pub allowed_mime_types: Option<Vec<String>>,
    pub enabled: bool,
    pub reason: Option<String>,
}

impl PresentationLayer for SaveUploadConfigRequest {}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct UploadConfigResponse {
    pub id: u64,
    pub name: String,
    pub provider: String,
    pub endpoint: Option<String>,
    pub file_field: Option<String>,
    pub bearer_token_mask: Option<String>,
    pub bearer_token_set: bool,
    pub access_key_mask: Option<String>,
    pub access_key_set: bool,
    pub secret_key_set: bool,
    pub bucket: Option<String>,
    pub region: Option<String>,
    pub public_base_url: Option<String>,
    pub local_root: Option<String>,
    pub key_prefix: Option<String>,
    pub max_file_size_bytes: u64,
    pub allowed_mime_types: Vec<String>,
    pub enabled: bool,
}

impl PresentationLayer for UploadConfigResponse {}

#[derive(Debug, Clone)]
pub struct UploadFileInput {
    pub original_filename: Option<String>,
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

impl PresentationLayer for UploadFileInput {}

/// 将上传接口的 multipart `file` 字段完整转换为应用层输入。
/// 非目标字段保持忽略，文件名、MIME 与字节不做额外归一化。
pub(crate) async fn multipart_file_input(mut multipart: Multipart) -> AppResult<UploadFileInput> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::Validation("upload multipart body is invalid".to_owned()))?
    {
        if field.name() != Some(MULTIPART_UPLOAD_FILE_FIELD) {
            continue;
        }
        let original_filename = field.file_name().map(str::to_owned);
        let mime_type = field.content_type().map(str::to_owned).ok_or_else(|| {
            AppError::Validation("upload file content type is required".to_owned())
        })?;
        let bytes = field
            .bytes()
            .await
            .map_err(|_| AppError::Validation("upload file body is invalid".to_owned()))?
            .to_vec();
        return Ok(UploadFileInput {
            original_filename,
            mime_type,
            bytes,
        });
    }

    Err(AppError::Validation("upload file is required".to_owned()))
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct UploadImageResponse {
    pub provider: String,
    pub object_key: String,
    pub download_url: String,
    pub share_url: Option<String>,
    pub delete_url: Option<String>,
    pub mime_type: String,
    pub size_bytes: u64,
}

impl PresentationLayer for UploadImageResponse {}
