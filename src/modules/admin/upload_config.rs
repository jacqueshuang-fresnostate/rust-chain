use crate::{
    error::{AppError, AppResult},
    infra::secrets::{decrypt_optional_secret, encrypt_secret_field, mask_secret},
};
use base64::{Engine as _, engine::general_purpose};
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::Digest;
use sqlx::{MySql, Pool, Transaction, types::Json as SqlxJson};
use std::path::PathBuf;
use uuid::Uuid;

const DEFAULT_CONFIG_NAME: &str = "default";
const DEFAULT_FILE_FIELD: &str = "file";
const DEFAULT_MAX_FILE_SIZE_BYTES: u64 = 10 * 1024 * 1024;
const MAX_FILE_SIZE_BYTES: u64 = 100 * 1024 * 1024;
pub const MAX_UPLOAD_BODY_SIZE_BYTES: usize = (MAX_FILE_SIZE_BYTES as usize) + 1024 * 1024;
const ADMIN_AUDIT_REASON_MAX_LEN: usize = 512;
const IMAGE_MIME_TYPES: &[&str] = &["image/png", "image/jpeg", "image/webp", "image/gif"];

type HmacSha256 = Hmac<sha2::Sha256>;
type HmacSha1 = Hmac<sha1::Sha1>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UploadProvider {
    ImageBed,
    Oss,
    S3,
    Local,
}

impl UploadProvider {
    fn parse(value: &str) -> AppResult<Self> {
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

    const fn code(self) -> &'static str {
        match self {
            Self::ImageBed => "image_bed",
            Self::Oss => "oss",
            Self::S3 => "s3",
            Self::Local => "local",
        }
    }

    const fn uses_bearer(self) -> bool {
        matches!(self, Self::ImageBed)
    }

    const fn uses_access_secret(self) -> bool {
        matches!(self, Self::Oss | Self::S3)
    }
}

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

#[derive(Debug, Clone)]
pub struct UploadFileInput {
    pub original_filename: Option<String>,
    pub mime_type: String,
    pub bytes: Vec<u8>,
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

#[derive(Debug, Clone, sqlx::FromRow)]
struct UploadConfigRow {
    id: u64,
    name: String,
    provider: String,
    endpoint: Option<String>,
    file_field: Option<String>,
    bearer_token_ciphertext: Option<String>,
    bearer_token_mask: Option<String>,
    access_key_ciphertext: Option<String>,
    access_key_mask: Option<String>,
    secret_key_ciphertext: Option<String>,
    bucket: Option<String>,
    region: Option<String>,
    public_base_url: Option<String>,
    local_root: Option<String>,
    key_prefix: Option<String>,
    max_file_size_bytes: u64,
    allowed_mime_types_json: SqlxJson<Vec<String>>,
    enabled: bool,
}

#[derive(Debug)]
struct ValidatedUploadConfig {
    provider: UploadProvider,
    endpoint: Option<String>,
    file_field: Option<String>,
    bucket: Option<String>,
    region: Option<String>,
    public_base_url: Option<String>,
    local_root: Option<String>,
    key_prefix: Option<String>,
    max_file_size_bytes: u64,
    allowed_mime_types: Vec<String>,
    enabled: bool,
}

pub async fn load_upload_config(pool: &Pool<MySql>) -> AppResult<Option<UploadConfigResponse>> {
    let row = sqlx::query_as::<_, UploadConfigRow>(&select_upload_config_sql(false))
        .bind(DEFAULT_CONFIG_NAME)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(upload_config_response))
}

pub async fn save_upload_config(
    pool: &Pool<MySql>,
    admin_id: u64,
    key: Option<&str>,
    request: SaveUploadConfigRequest,
) -> AppResult<UploadConfigResponse> {
    let reason = required_reason(request.reason.clone())?;
    let mut tx = pool.begin().await?;
    let before = lock_upload_config_in_tx(&mut tx).await?;
    let config = validate_upload_config(&request)?;
    let existing_same_provider = before
        .as_ref()
        .filter(|row| row.provider == config.provider.code())
        .filter(|row| upload_secret_destination_unchanged(row, &config));

    let (bearer_token_ciphertext, bearer_token_mask) = if config.provider.uses_bearer() {
        let existing_ciphertext =
            existing_same_provider.and_then(|row| row.bearer_token_ciphertext.clone());
        let existing_mask = existing_same_provider.and_then(|row| row.bearer_token_mask.clone());
        let ciphertext =
            encrypt_optional_secret(key, request.bearer_token.as_deref(), existing_ciphertext)?;
        let mask = request
            .bearer_token
            .as_deref()
            .and_then(optional_str)
            .map(mask_secret)
            .or(existing_mask);
        if config.enabled && ciphertext.is_none() {
            return Err(AppError::Validation(
                "image bed bearer token is required".to_owned(),
            ));
        }
        (ciphertext, mask)
    } else {
        (None, None)
    };

    let (access_key_ciphertext, access_key_mask, secret_key_ciphertext) =
        if config.provider.uses_access_secret() {
            let existing_access_ciphertext =
                existing_same_provider.and_then(|row| row.access_key_ciphertext.clone());
            let existing_secret_ciphertext =
                existing_same_provider.and_then(|row| row.secret_key_ciphertext.clone());
            let existing_access_mask =
                existing_same_provider.and_then(|row| row.access_key_mask.clone());
            let access_ciphertext = encrypt_optional_secret(
                key,
                request.access_key.as_deref(),
                existing_access_ciphertext,
            )?;
            let secret_ciphertext = encrypt_optional_secret(
                key,
                request.secret_key.as_deref(),
                existing_secret_ciphertext,
            )?;
            let access_mask = request
                .access_key
                .as_deref()
                .and_then(optional_str)
                .map(mask_secret)
                .or(existing_access_mask);
            if config.enabled && (access_ciphertext.is_none() || secret_ciphertext.is_none()) {
                return Err(AppError::Validation(
                    "upload access key and secret key are required".to_owned(),
                ));
            }
            (access_ciphertext, access_mask, secret_ciphertext)
        } else {
            (None, None, None)
        };

    sqlx::query(
        r#"INSERT INTO upload_storage_configs
           (name, provider, endpoint, file_field, bearer_token_ciphertext, bearer_token_mask,
            access_key_ciphertext, access_key_mask, secret_key_ciphertext, bucket, region,
            public_base_url, local_root, key_prefix, max_file_size_bytes, allowed_mime_types_json,
            enabled, updated_by)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE provider = VALUES(provider),
                                   endpoint = VALUES(endpoint),
                                   file_field = VALUES(file_field),
                                   bearer_token_ciphertext = VALUES(bearer_token_ciphertext),
                                   bearer_token_mask = VALUES(bearer_token_mask),
                                   access_key_ciphertext = VALUES(access_key_ciphertext),
                                   access_key_mask = VALUES(access_key_mask),
                                   secret_key_ciphertext = VALUES(secret_key_ciphertext),
                                   bucket = VALUES(bucket),
                                   region = VALUES(region),
                                   public_base_url = VALUES(public_base_url),
                                   local_root = VALUES(local_root),
                                   key_prefix = VALUES(key_prefix),
                                   max_file_size_bytes = VALUES(max_file_size_bytes),
                                   allowed_mime_types_json = VALUES(allowed_mime_types_json),
                                   enabled = VALUES(enabled),
                                   updated_by = VALUES(updated_by)"#,
    )
    .bind(DEFAULT_CONFIG_NAME)
    .bind(config.provider.code())
    .bind(&config.endpoint)
    .bind(&config.file_field)
    .bind(&bearer_token_ciphertext)
    .bind(&bearer_token_mask)
    .bind(&access_key_ciphertext)
    .bind(&access_key_mask)
    .bind(&secret_key_ciphertext)
    .bind(&config.bucket)
    .bind(&config.region)
    .bind(&config.public_base_url)
    .bind(&config.local_root)
    .bind(&config.key_prefix)
    .bind(config.max_file_size_bytes)
    .bind(SqlxJson(config.allowed_mime_types))
    .bind(config.enabled)
    .bind(admin_id)
    .execute(&mut *tx)
    .await?;

    let after = load_upload_config_in_tx(&mut tx).await?;
    insert_upload_audit_log_in_tx(
        &mut tx,
        admin_id,
        "upload_storage_config.save",
        after.id,
        before.as_ref().map(upload_config_audit_json),
        Some(upload_config_audit_json(&after)),
        reason,
    )
    .await?;
    tx.commit().await?;
    Ok(upload_config_response(after))
}

pub async fn upload_image(
    pool: &Pool<MySql>,
    admin_id: u64,
    key: Option<&str>,
    input: UploadFileInput,
) -> AppResult<UploadImageResponse> {
    let row = load_enabled_upload_config_row(pool)
        .await?
        .ok_or_else(|| AppError::Validation("upload storage is not enabled".to_owned()))?;
    validate_upload_file(&row, &input)?;
    let provider = UploadProvider::parse(&row.provider)?;
    let response = match provider {
        UploadProvider::ImageBed => upload_to_image_bed(&row, key, &input).await?,
        UploadProvider::Local => upload_to_local(&row, &input).await?,
        UploadProvider::S3 => upload_to_s3(&row, key, &input).await?,
        UploadProvider::Oss => upload_to_oss(&row, key, &input).await?,
    };
    record_upload_object(pool, admin_id, &input, &response).await?;
    Ok(response)
}

fn validate_upload_config(request: &SaveUploadConfigRequest) -> AppResult<ValidatedUploadConfig> {
    let provider = UploadProvider::parse(&request.provider)?;
    let endpoint = optional_string(request.endpoint.clone());
    let public_base_url = optional_string(request.public_base_url.clone());
    let local_root = optional_string(request.local_root.clone());
    let bucket = optional_string(request.bucket.clone());
    let region = optional_string(request.region.clone());
    validate_optional_len(endpoint.as_deref(), "endpoint", 512)?;
    validate_optional_len(public_base_url.as_deref(), "public_base_url", 512)?;
    validate_optional_len(local_root.as_deref(), "local_root", 512)?;
    let key_prefix = normalize_key_prefix(request.key_prefix.clone())?;
    let file_field = Some(validate_len(
        optional_string(request.file_field.clone())
            .unwrap_or_else(|| DEFAULT_FILE_FIELD.to_owned()),
        "file_field",
        64,
    )?);
    let max_file_size_bytes = request
        .max_file_size_bytes
        .unwrap_or(DEFAULT_MAX_FILE_SIZE_BYTES);
    if max_file_size_bytes == 0 || max_file_size_bytes > MAX_FILE_SIZE_BYTES {
        return Err(AppError::Validation(
            "max_file_size_bytes is invalid".to_owned(),
        ));
    }
    let allowed_mime_types = normalize_mime_types(request.allowed_mime_types.clone())?;

    match provider {
        UploadProvider::ImageBed => {
            validate_credential_url(endpoint.as_deref(), "image bed endpoint")?;
        }
        UploadProvider::Local => {
            require_value(local_root.as_deref(), "local_root")?;
            validate_url(public_base_url.as_deref(), "public_base_url")?;
        }
        UploadProvider::S3 => {
            validate_bucket_name(bucket.as_deref())?;
            validate_region(region.as_deref())?;
            if let Some(endpoint) = &endpoint {
                validate_credential_url(Some(endpoint), "s3 endpoint")?;
            }
            if let Some(public_base_url) = &public_base_url {
                validate_url(Some(public_base_url), "public_base_url")?;
            }
        }
        UploadProvider::Oss => {
            validate_credential_url(endpoint.as_deref(), "oss endpoint")?;
            validate_bucket_name(bucket.as_deref())?;
            if let Some(public_base_url) = &public_base_url {
                validate_url(Some(public_base_url), "public_base_url")?;
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

fn validate_upload_file(row: &UploadConfigRow, input: &UploadFileInput) -> AppResult<()> {
    if input.bytes.is_empty() {
        return Err(AppError::Validation("upload file is required".to_owned()));
    }
    validate_image_bytes(&input.bytes, &input.mime_type)?;
    let size = input.bytes.len() as u64;
    if size > row.max_file_size_bytes {
        return Err(AppError::Validation("upload file is too large".to_owned()));
    }
    if !row
        .allowed_mime_types_json
        .0
        .iter()
        .any(|mime| mime == &input.mime_type)
    {
        return Err(AppError::Validation(
            "upload file mime type is not allowed".to_owned(),
        ));
    }
    Ok(())
}

fn validate_image_bytes(bytes: &[u8], mime_type: &str) -> AppResult<()> {
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

async fn upload_to_image_bed(
    row: &UploadConfigRow,
    key: Option<&str>,
    input: &UploadFileInput,
) -> AppResult<UploadImageResponse> {
    let endpoint = row
        .endpoint
        .as_deref()
        .ok_or_else(|| AppError::Validation("image bed endpoint is not configured".to_owned()))?;
    let token = decrypt_required(row.bearer_token_ciphertext.as_deref(), key, "bearer token")?;
    let field = row.file_field.as_deref().unwrap_or(DEFAULT_FILE_FIELD);
    let filename = safe_upload_filename(input.original_filename.as_deref(), &input.mime_type);
    let part = Part::bytes(input.bytes.clone())
        .file_name(filename)
        .mime_str(&input.mime_type)
        .map_err(|_| AppError::Validation("upload file mime type is invalid".to_owned()))?;
    let form = Form::new().part(field.to_owned(), part);
    let response = reqwest::Client::new()
        .post(endpoint)
        .bearer_auth(token)
        .multipart(form)
        .send()
        .await
        .map_err(|_| AppError::Validation("image bed upload failed".to_owned()))?;
    if !response.status().is_success() {
        return Err(AppError::Validation(format!(
            "image bed upload failed with status {}",
            response.status().as_u16()
        )));
    }
    let payload = response
        .json::<ImageBedUploadResponse>()
        .await
        .map_err(|_| AppError::Validation("image bed upload response is invalid".to_owned()))?;
    if payload.success == Some(false) {
        return Err(AppError::Validation("image bed upload failed".to_owned()));
    }
    let download_url = safe_response_url(
        payload.links.download.as_deref(),
        "image bed download url",
        true,
    )?
    .ok_or_else(|| AppError::Validation("image bed download url is missing".to_owned()))?;
    let share_url =
        safe_response_url(payload.links.share.as_deref(), "image bed share url", false)?;
    let delete_url = safe_response_url(
        payload.links.delete.as_deref(),
        "image bed delete url",
        false,
    )?;
    let object_key = payload
        .file
        .as_ref()
        .and_then(|file| file.id.as_deref())
        .map(safe_key_segment)
        .filter(|value| !value.is_empty() && value.len() <= 512)
        .unwrap_or_else(|| Uuid::now_v7().to_string());
    let size_bytes = payload
        .file
        .as_ref()
        .and_then(|file| file.size)
        .unwrap_or(input.bytes.len() as u64);
    let mime_type = payload
        .file
        .and_then(|file| file.file_type)
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| IMAGE_MIME_TYPES.contains(&value.as_str()))
        .unwrap_or_else(|| input.mime_type.clone());
    Ok(UploadImageResponse {
        provider: UploadProvider::ImageBed.code().to_owned(),
        object_key,
        download_url,
        share_url,
        delete_url,
        mime_type,
        size_bytes,
    })
}

async fn upload_to_local(
    row: &UploadConfigRow,
    input: &UploadFileInput,
) -> AppResult<UploadImageResponse> {
    let root = row
        .local_root
        .as_deref()
        .ok_or_else(|| AppError::Validation("local_root is not configured".to_owned()))?;
    let base_url = row
        .public_base_url
        .as_deref()
        .ok_or_else(|| AppError::Validation("public_base_url is not configured".to_owned()))?;
    let object_key = generated_object_key(row.key_prefix.as_deref(), &input.mime_type);
    let path = PathBuf::from(root).join(&object_key);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|_| AppError::Internal("failed to create upload directory".to_owned()))?;
    }
    tokio::fs::write(&path, &input.bytes)
        .await
        .map_err(|_| AppError::Internal("failed to write upload file".to_owned()))?;
    Ok(UploadImageResponse {
        provider: UploadProvider::Local.code().to_owned(),
        download_url: join_public_url(base_url, &object_key),
        share_url: None,
        delete_url: None,
        object_key,
        mime_type: input.mime_type.clone(),
        size_bytes: input.bytes.len() as u64,
    })
}

async fn upload_to_s3(
    row: &UploadConfigRow,
    key: Option<&str>,
    input: &UploadFileInput,
) -> AppResult<UploadImageResponse> {
    let access_key = decrypt_required(row.access_key_ciphertext.as_deref(), key, "access key")?;
    let secret_key = decrypt_required(row.secret_key_ciphertext.as_deref(), key, "secret key")?;
    let bucket = row
        .bucket
        .as_deref()
        .ok_or_else(|| AppError::Validation("bucket is not configured".to_owned()))?;
    let region = row
        .region
        .as_deref()
        .ok_or_else(|| AppError::Validation("region is not configured".to_owned()))?;
    let endpoint = row
        .endpoint
        .clone()
        .unwrap_or_else(|| format!("https://s3.{region}.amazonaws.com"));
    let object_key = generated_object_key(row.key_prefix.as_deref(), &input.mime_type);
    let url = join_endpoint_path(&endpoint, &[bucket, &object_key])?;
    let parsed_url = reqwest::Url::parse(&url)
        .map_err(|_| AppError::Validation("s3 endpoint is invalid".to_owned()))?;
    let host = url_host(&parsed_url)?;
    let now = Utc::now();
    let date = now.format("%Y%m%d").to_string();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let payload_hash = sha256_hex(&input.bytes);
    let canonical_uri = parsed_url.path();
    let canonical_request = format!(
        "PUT\n{canonical_uri}\n\ncontent-type:{}\nhost:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n\ncontent-type;host;x-amz-content-sha256;x-amz-date\n{payload_hash}",
        input.mime_type
    );
    let scope = format!("{date}/{region}/s3/aws4_request");
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        sha256_hex(canonical_request.as_bytes())
    );
    let signature = s3_signature(&secret_key, &date, region, &string_to_sign);
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={access_key}/{scope}, SignedHeaders=content-type;host;x-amz-content-sha256;x-amz-date, Signature={signature}"
    );
    let response = reqwest::Client::new()
        .put(&url)
        .header("content-type", &input.mime_type)
        .header("x-amz-content-sha256", payload_hash)
        .header("x-amz-date", amz_date)
        .header("authorization", authorization)
        .body(input.bytes.clone())
        .send()
        .await
        .map_err(|_| AppError::Validation("s3 upload failed".to_owned()))?;
    if !response.status().is_success() {
        return Err(AppError::Validation(format!(
            "s3 upload failed with status {}",
            response.status().as_u16()
        )));
    }
    Ok(UploadImageResponse {
        provider: UploadProvider::S3.code().to_owned(),
        download_url: row
            .public_base_url
            .as_deref()
            .map(|base| join_public_url(base, &object_key))
            .unwrap_or(url),
        share_url: None,
        delete_url: None,
        object_key,
        mime_type: input.mime_type.clone(),
        size_bytes: input.bytes.len() as u64,
    })
}

async fn upload_to_oss(
    row: &UploadConfigRow,
    key: Option<&str>,
    input: &UploadFileInput,
) -> AppResult<UploadImageResponse> {
    let access_key = decrypt_required(row.access_key_ciphertext.as_deref(), key, "access key")?;
    let secret_key = decrypt_required(row.secret_key_ciphertext.as_deref(), key, "secret key")?;
    let endpoint = row
        .endpoint
        .as_deref()
        .ok_or_else(|| AppError::Validation("oss endpoint is not configured".to_owned()))?;
    let bucket = row
        .bucket
        .as_deref()
        .ok_or_else(|| AppError::Validation("bucket is not configured".to_owned()))?;
    let object_key = generated_object_key(row.key_prefix.as_deref(), &input.mime_type);
    let url = join_endpoint_path(endpoint, &[bucket, &object_key])?;
    let date = Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();
    let canonical_resource = format!("/{bucket}/{object_key}");
    let string_to_sign = format!("PUT\n\n{}\n{date}\n{canonical_resource}", input.mime_type);
    let signature = hmac_sha1_base64(secret_key.as_bytes(), &string_to_sign);
    let authorization = format!("OSS {access_key}:{signature}");
    let response = reqwest::Client::new()
        .put(&url)
        .header("date", date)
        .header("content-type", &input.mime_type)
        .header("authorization", authorization)
        .body(input.bytes.clone())
        .send()
        .await
        .map_err(|_| AppError::Validation("oss upload failed".to_owned()))?;
    if !response.status().is_success() {
        return Err(AppError::Validation(format!(
            "oss upload failed with status {}",
            response.status().as_u16()
        )));
    }
    Ok(UploadImageResponse {
        provider: UploadProvider::Oss.code().to_owned(),
        download_url: row
            .public_base_url
            .as_deref()
            .map(|base| join_public_url(base, &object_key))
            .unwrap_or(url),
        share_url: None,
        delete_url: None,
        object_key,
        mime_type: input.mime_type.clone(),
        size_bytes: input.bytes.len() as u64,
    })
}

async fn record_upload_object(
    pool: &Pool<MySql>,
    admin_id: u64,
    input: &UploadFileInput,
    response: &UploadImageResponse,
) -> AppResult<()> {
    let original_filename =
        safe_upload_filename(input.original_filename.as_deref(), &input.mime_type);
    sqlx::query(
        r#"INSERT INTO upload_objects
           (provider, object_key, public_url, share_url, delete_url, mime_type, size_bytes,
            original_filename, uploaded_by)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&response.provider)
    .bind(&response.object_key)
    .bind(&response.download_url)
    .bind(&response.share_url)
    .bind(&response.delete_url)
    .bind(&response.mime_type)
    .bind(response.size_bytes)
    .bind(original_filename)
    .bind(admin_id)
    .execute(pool)
    .await?;
    Ok(())
}

fn select_upload_config_sql(for_update: bool) -> String {
    let mut sql = r#"SELECT id, name, provider, endpoint, file_field, bearer_token_ciphertext,
              bearer_token_mask, access_key_ciphertext, access_key_mask, secret_key_ciphertext,
              bucket, region, public_base_url, local_root, key_prefix, max_file_size_bytes,
              allowed_mime_types_json, enabled
       FROM upload_storage_configs
       WHERE name = ?"#
        .to_owned();
    if for_update {
        sql.push_str(" FOR UPDATE");
    }
    sql
}

async fn lock_upload_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<Option<UploadConfigRow>> {
    sqlx::query_as::<_, UploadConfigRow>(&select_upload_config_sql(true))
        .bind(DEFAULT_CONFIG_NAME)
        .fetch_optional(&mut **tx)
        .await
        .map_err(AppError::Database)
}

async fn load_upload_config_in_tx(tx: &mut Transaction<'_, MySql>) -> AppResult<UploadConfigRow> {
    sqlx::query_as::<_, UploadConfigRow>(&select_upload_config_sql(false))
        .bind(DEFAULT_CONFIG_NAME)
        .fetch_one(&mut **tx)
        .await
        .map_err(AppError::Database)
}

async fn load_enabled_upload_config_row(pool: &Pool<MySql>) -> AppResult<Option<UploadConfigRow>> {
    let row = sqlx::query_as::<_, UploadConfigRow>(
        r#"SELECT id, name, provider, endpoint, file_field, bearer_token_ciphertext,
                  bearer_token_mask, access_key_ciphertext, access_key_mask, secret_key_ciphertext,
                  bucket, region, public_base_url, local_root, key_prefix, max_file_size_bytes,
                  allowed_mime_types_json, enabled
           FROM upload_storage_configs
           WHERE name = ? AND enabled = TRUE"#,
    )
    .bind(DEFAULT_CONFIG_NAME)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

async fn insert_upload_audit_log_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    action: &'static str,
    target_id: u64,
    before_json: Option<Value>,
    after_json: Option<Value>,
    reason: String,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, before_json, after_json, reason)
           VALUES (?, ?, 'upload_storage_config', ?, ?, ?, ?)"#,
    )
    .bind(admin_id)
    .bind(action)
    .bind(target_id.to_string())
    .bind(before_json.map(SqlxJson))
    .bind(after_json.map(SqlxJson))
    .bind(reason)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn upload_config_response(row: UploadConfigRow) -> UploadConfigResponse {
    UploadConfigResponse {
        id: row.id,
        name: row.name,
        provider: row.provider,
        endpoint: row.endpoint,
        file_field: row.file_field,
        bearer_token_mask: row.bearer_token_mask,
        bearer_token_set: row.bearer_token_ciphertext.is_some(),
        access_key_mask: row.access_key_mask,
        access_key_set: row.access_key_ciphertext.is_some(),
        secret_key_set: row.secret_key_ciphertext.is_some(),
        bucket: row.bucket,
        region: row.region,
        public_base_url: row.public_base_url,
        local_root: row.local_root,
        key_prefix: row.key_prefix,
        max_file_size_bytes: row.max_file_size_bytes,
        allowed_mime_types: row.allowed_mime_types_json.0,
        enabled: row.enabled,
    }
}

fn upload_secret_destination_unchanged(
    row: &UploadConfigRow,
    config: &ValidatedUploadConfig,
) -> bool {
    row.endpoint == config.endpoint && row.bucket == config.bucket && row.region == config.region
}

fn upload_config_audit_json(row: &UploadConfigRow) -> Value {
    json!({
        "id": row.id,
        "name": row.name,
        "provider": row.provider,
        "endpoint": row.endpoint,
        "file_field": row.file_field,
        "bearer_token_mask": row.bearer_token_mask,
        "bearer_token_set": row.bearer_token_ciphertext.is_some(),
        "access_key_mask": row.access_key_mask,
        "access_key_set": row.access_key_ciphertext.is_some(),
        "secret_key_set": row.secret_key_ciphertext.is_some(),
        "bucket": row.bucket,
        "region": row.region,
        "public_base_url": row.public_base_url,
        "local_root": row.local_root,
        "key_prefix": row.key_prefix,
        "max_file_size_bytes": row.max_file_size_bytes,
        "allowed_mime_types": row.allowed_mime_types_json.0,
        "enabled": row.enabled,
    })
}

fn encrypt_optional_secret(
    key: Option<&str>,
    new_value: Option<&str>,
    existing_ciphertext: Option<String>,
) -> AppResult<Option<String>> {
    if new_value.and_then(optional_str).is_some() {
        let key = key.ok_or_else(|| {
            AppError::Internal("credential encryption key is not configured".to_owned())
        })?;
        encrypt_secret_field(key, new_value, existing_ciphertext)
    } else {
        Ok(existing_ciphertext)
    }
}

fn decrypt_required(ciphertext: Option<&str>, key: Option<&str>, field: &str) -> AppResult<String> {
    let ciphertext =
        ciphertext.ok_or_else(|| AppError::Validation(format!("{field} is not configured")))?;
    let key = key.ok_or_else(|| {
        AppError::Internal("credential encryption key is not configured".to_owned())
    })?;
    decrypt_optional_secret(Some(ciphertext), key)?
        .ok_or_else(|| AppError::Validation(format!("{field} is not configured")))
}

fn normalize_mime_types(value: Option<Vec<String>>) -> AppResult<Vec<String>> {
    let values = value.unwrap_or_else(|| {
        IMAGE_MIME_TYPES
            .iter()
            .map(|item| (*item).to_owned())
            .collect()
    });
    let mut normalized = Vec::new();
    for item in values {
        let mime = optional_string(Some(item))
            .ok_or_else(|| AppError::Validation("allowed mime type is invalid".to_owned()))?
            .to_ascii_lowercase();
        if !IMAGE_MIME_TYPES.contains(&mime.as_str()) {
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

fn normalize_key_prefix(value: Option<String>) -> AppResult<Option<String>> {
    let Some(value) = optional_string(value) else {
        return Ok(None);
    };
    let mut segments = Vec::new();
    for segment in value.replace('\\', "/").split('/').filter_map(optional_str) {
        if matches!(segment, "." | "..") {
            return Err(AppError::Validation("key_prefix is invalid".to_owned()));
        }
        let safe_segment = safe_key_segment(segment);
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

fn generated_object_key(prefix: Option<&str>, mime_type: &str) -> String {
    let date = Utc::now().format("%Y/%m/%d");
    let suffix = extension_for_mime(mime_type);
    let key = format!("{date}/{}.{}", Uuid::now_v7().simple(), suffix);
    match prefix.and_then(optional_str) {
        Some(prefix) => format!("{}/{}", prefix.trim_matches('/'), key),
        None => key,
    }
}

fn safe_upload_filename(original: Option<&str>, mime_type: &str) -> String {
    let extension = extension_for_mime(mime_type);
    let Some(original) = original.and_then(optional_str) else {
        return format!("upload.{extension}");
    };
    let normalized = original.replace('\\', "/");
    let candidate = normalized.split('/').next_back().unwrap_or("upload");
    let name = safe_key_segment(candidate);
    let name = if name.is_empty() {
        format!("upload.{extension}")
    } else {
        name
    };
    truncate_filename(name, extension, 255)
}

fn truncate_filename(name: String, extension: &str, max_len: usize) -> String {
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

fn safe_key_segment(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        .collect()
}

fn extension_for_mime(mime_type: &str) -> &'static str {
    match mime_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "bin",
    }
}

fn validate_len(value: String, field: &str, max_len: usize) -> AppResult<String> {
    if value.len() > max_len {
        Err(AppError::Validation(format!("{field} is invalid")))
    } else {
        Ok(value)
    }
}

fn validate_optional_len(value: Option<&str>, field: &str, max_len: usize) -> AppResult<()> {
    if value.is_some_and(|value| value.len() > max_len) {
        Err(AppError::Validation(format!("{field} is invalid")))
    } else {
        Ok(())
    }
}

fn validate_url(value: Option<&str>, field: &str) -> AppResult<()> {
    let value = require_value(value, field)?;
    validate_safe_url(value, field, false).map(|_| ())
}

fn validate_credential_url(value: Option<&str>, field: &str) -> AppResult<()> {
    let value = require_value(value, field)?;
    validate_safe_url(value, field, true).map(|_| ())
}

fn safe_response_url(
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
    validate_safe_url(value, field, false).map(Some)
}

fn validate_safe_url(value: &str, field: &str, require_https: bool) -> AppResult<String> {
    let url = reqwest::Url::parse(value)
        .map_err(|_| AppError::Validation(format!("{field} is invalid")))?;
    let valid_scheme = if require_https {
        url.scheme() == "https" || (url.scheme() == "http" && is_loopback_url(&url))
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

fn is_loopback_url(url: &reqwest::Url) -> bool {
    matches!(
        url.host_str(),
        Some("localhost") | Some("127.0.0.1") | Some("::1")
    )
}

fn validate_bucket_name(value: Option<&str>) -> AppResult<()> {
    let value = require_value(value, "bucket")?;
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

fn validate_region(value: Option<&str>) -> AppResult<()> {
    let value = require_value(value, "region")?;
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

fn require_value<'a>(value: Option<&'a str>, field: &str) -> AppResult<&'a str> {
    value
        .and_then(optional_str)
        .ok_or_else(|| AppError::Validation(format!("{field} is required")))
}

fn join_public_url(base: &str, object_key: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        object_key.trim_start_matches('/')
    )
}

fn join_endpoint_path(endpoint: &str, parts: &[&str]) -> AppResult<String> {
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

fn url_host(url: &reqwest::Url) -> AppResult<String> {
    let host = url
        .host_str()
        .ok_or_else(|| AppError::Validation("upload endpoint host is invalid".to_owned()))?;
    Ok(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    })
}

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(sha2::Sha256::digest(data))
}

fn hmac_sha256(key: &[u8], data: &str) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

fn hmac_sha1_base64(key: &[u8], data: &str) -> String {
    let mut mac = HmacSha1::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data.as_bytes());
    general_purpose::STANDARD.encode(mac.finalize().into_bytes())
}

fn s3_signature(secret: &str, date: &str, region: &str, string_to_sign: &str) -> String {
    let k_date = hmac_sha256(format!("AWS4{secret}").as_bytes(), date);
    let k_region = hmac_sha256(&k_date, region);
    let k_service = hmac_sha256(&k_region, "s3");
    let k_signing = hmac_sha256(&k_service, "aws4_request");
    hex::encode(hmac_sha256(&k_signing, string_to_sign))
}

fn required_reason(value: Option<String>) -> AppResult<String> {
    let Some(reason) = optional_string(value) else {
        return Err(AppError::Validation("reason is required".to_owned()));
    };
    if reason.chars().count() > ADMIN_AUDIT_REASON_MAX_LEN {
        return Err(AppError::Validation("reason is too long".to_owned()));
    }
    Ok(reason)
}

fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn optional_str(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

#[derive(Debug, Deserialize)]
struct ImageBedUploadResponse {
    success: Option<bool>,
    file: Option<ImageBedFile>,
    links: ImageBedLinks,
}

#[derive(Debug, Deserialize)]
struct ImageBedFile {
    id: Option<String>,
    size: Option<u64>,
    #[serde(rename = "type")]
    file_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ImageBedLinks {
    download: Option<String>,
    share: Option<String>,
    delete: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_upload_provider_config() {
        let config = validate_upload_config(&SaveUploadConfigRequest {
            provider: "image-bed".to_owned(),
            endpoint: Some(" https://oss.example.test/api/v1/upload ".to_owned()),
            file_field: None,
            bearer_token: None,
            access_key: None,
            secret_key: None,
            bucket: None,
            region: None,
            public_base_url: None,
            local_root: None,
            key_prefix: Some(" /images//2026 ".to_owned()),
            max_file_size_bytes: None,
            allowed_mime_types: Some(vec![" image/png ".to_owned(), "image/png".to_owned()]),
            enabled: true,
            reason: Some("configure upload".to_owned()),
        })
        .unwrap();

        assert_eq!(config.provider.code(), "image_bed");
        assert_eq!(config.file_field.as_deref(), Some("file"));
        assert_eq!(config.key_prefix.as_deref(), Some("images/2026"));
        assert_eq!(config.allowed_mime_types, ["image/png"]);
    }

    #[test]
    fn rejects_invalid_upload_provider_config() {
        let request = SaveUploadConfigRequest {
            provider: "local".to_owned(),
            endpoint: None,
            file_field: None,
            bearer_token: None,
            access_key: None,
            secret_key: None,
            bucket: None,
            region: None,
            public_base_url: None,
            local_root: None,
            key_prefix: None,
            max_file_size_bytes: Some(0),
            allowed_mime_types: Some(vec!["text/plain".to_owned()]),
            enabled: true,
            reason: Some("configure upload".to_owned()),
        };

        assert!(validate_upload_config(&request).is_err());
    }
}
