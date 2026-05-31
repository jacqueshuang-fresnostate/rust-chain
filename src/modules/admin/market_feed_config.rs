use crate::{
    error::{AppError, AppResult},
    modules::market::{KlineUpsertKey, ValidatedMarketSymbol, adapters::MarketFeedProvider},
    time::option_unix_millis,
    workers::market_feed::{MarketFeedRuntimeConfig, MarketFeedRuntimeStatus},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::Utc;
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{MySql, Pool, Transaction, types::Json as SqlxJson};

const DEFAULT_CONFIG_NAME: &str = "default";
const NONCE_LEN: usize = 12;
const API_KEY_AUTH_TYPE: &str = "api_key";
const NONE_AUTH_TYPE: &str = "none";

#[derive(Debug, Deserialize)]
pub struct SaveMarketFeedConfigRequest {
    pub symbols: Vec<String>,
    pub intervals: Vec<String>,
    pub providers: Vec<String>,
    pub enabled: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct MarketFeedConfigResponse {
    pub id: u64,
    pub name: String,
    pub symbols: Vec<String>,
    pub intervals: Vec<String>,
    pub providers: Vec<String>,
    pub enabled: bool,
    pub version: u64,
    pub applied_version: Option<u64>,
    pub needs_reload: bool,
    pub last_reload_status: Option<String>,
    pub last_reload_error: Option<String>,
    #[serde(default, with = "option_unix_millis")]
    pub last_reloaded_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct MarketFeedStatusResponse {
    pub saved_config: Option<MarketFeedConfigResponse>,
    pub runtime: MarketFeedRuntimeStatus,
}

#[derive(Debug, Deserialize)]
pub struct ReloadMarketFeedRequest {
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct ReloadMarketFeedResponse {
    pub config: MarketFeedConfigResponse,
    pub runtime: MarketFeedRuntimeStatus,
}

#[derive(Debug, Deserialize)]
pub struct UpsertMarketSourceCredentialRequest {
    pub auth_type: String,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub passphrase: Option<String>,
    pub enabled: bool,
    pub reason: String,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct MarketSourceCredentialResponse {
    pub provider: String,
    pub auth_type: String,
    pub api_key_mask: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct MarketSourceCredentialsResponse {
    pub credentials: Vec<MarketSourceCredentialResponse>,
}

#[derive(Debug, Clone)]
pub struct MarketSourceCredentialSecret {
    pub provider: String,
    pub auth_type: String,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub passphrase: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct MarketFeedConfigRow {
    id: u64,
    name: String,
    symbols_json: SqlxJson<Vec<String>>,
    intervals_json: SqlxJson<Vec<String>>,
    providers_json: SqlxJson<Vec<String>>,
    enabled: bool,
    version: u64,
    applied_version: Option<u64>,
    last_reload_status: Option<String>,
    last_reload_error: Option<String>,
    last_reloaded_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, sqlx::FromRow)]
struct MarketSourceCredentialRow {
    provider: String,
    auth_type: String,
    api_key_ciphertext: Option<String>,
    api_secret_ciphertext: Option<String>,
    passphrase_ciphertext: Option<String>,
    api_key_mask: Option<String>,
    enabled: bool,
}

pub async fn load_config(pool: &Pool<MySql>) -> AppResult<Option<MarketFeedConfigResponse>> {
    let Some(row) = sqlx::query_as::<_, MarketFeedConfigRow>(
        r#"SELECT id, name, symbols_json, intervals_json, providers_json, enabled,
                  version, applied_version, last_reload_status, last_reload_error, last_reloaded_at
           FROM market_feed_configs
           WHERE name = ?"#,
    )
    .bind(DEFAULT_CONFIG_NAME)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    Ok(Some(config_response(row)))
}

pub async fn load_enabled_config_for_bootstrap(
    pool: &Pool<MySql>,
) -> AppResult<Option<MarketFeedConfigResponse>> {
    Ok(load_config(pool).await?.filter(|config| config.enabled))
}

pub async fn save_config(
    pool: &Pool<MySql>,
    admin_id: u64,
    request: SaveMarketFeedConfigRequest,
) -> AppResult<MarketFeedConfigResponse> {
    validate_reason(request.reason.as_deref())?;
    let symbols = validate_symbols(&request.symbols, request.enabled)?;
    let intervals = validate_intervals(&request.intervals)?;
    let providers = validate_providers(&request.providers)?;
    let mut tx = pool.begin().await?;
    let before = lock_config_in_tx(&mut tx).await?;
    let version = before
        .as_ref()
        .map(|config| config.version + 1)
        .unwrap_or(1);

    sqlx::query(
        r#"INSERT INTO market_feed_configs
           (name, symbols_json, intervals_json, providers_json, enabled, version, updated_by)
           VALUES (?, ?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE symbols_json = VALUES(symbols_json),
                                   intervals_json = VALUES(intervals_json),
                                   providers_json = VALUES(providers_json),
                                   enabled = VALUES(enabled),
                                   version = VALUES(version),
                                   updated_by = VALUES(updated_by)"#,
    )
    .bind(DEFAULT_CONFIG_NAME)
    .bind(SqlxJson(symbols))
    .bind(SqlxJson(intervals))
    .bind(SqlxJson(providers))
    .bind(request.enabled)
    .bind(version)
    .bind(admin_id)
    .execute(&mut *tx)
    .await?;

    let after = load_config_in_tx(&mut tx).await?;
    insert_market_feed_audit_log_in_tx(
        &mut tx,
        MarketFeedAuditEntry {
            admin_id,
            action: "market_feed_config.save",
            target_type: "market_feed_config",
            target_id: after.id,
            before_json: before.as_ref().map(config_audit_json),
            after_json: Some(config_audit_json(&after)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub async fn list_credentials(
    pool: &Pool<MySql>,
) -> AppResult<Vec<MarketSourceCredentialResponse>> {
    let rows = sqlx::query_as::<_, MarketSourceCredentialRow>(
        r#"SELECT provider, auth_type, api_key_ciphertext, api_secret_ciphertext,
                  passphrase_ciphertext, api_key_mask, enabled
           FROM market_source_credentials
           ORDER BY provider ASC"#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(credential_response).collect())
}

pub async fn upsert_credential(
    pool: &Pool<MySql>,
    admin_id: u64,
    provider: String,
    key: Option<&str>,
    request: UpsertMarketSourceCredentialRequest,
) -> AppResult<MarketSourceCredentialResponse> {
    validate_reason(Some(&request.reason))?;
    let provider = MarketFeedProvider::from_code(&provider)?.code().to_owned();
    let auth_type = validate_auth_type(&request.auth_type)?;
    let mut tx = pool.begin().await?;
    let before = lock_credential_in_tx(&mut tx, &provider).await?;

    let (api_key_ciphertext, api_secret_ciphertext, passphrase_ciphertext, api_key_mask) =
        if auth_type == API_KEY_AUTH_TYPE {
            let key = key.ok_or_else(|| {
                AppError::Internal("credential encryption key is not configured".to_owned())
            })?;
            let api_key_ciphertext = encrypt_secret_field(
                key,
                request.api_key.as_deref(),
                before
                    .as_ref()
                    .and_then(|row| row.api_key_ciphertext.clone()),
            )?;
            let api_secret_ciphertext = encrypt_secret_field(
                key,
                request.api_secret.as_deref(),
                before
                    .as_ref()
                    .and_then(|row| row.api_secret_ciphertext.clone()),
            )?;
            let passphrase_ciphertext = encrypt_secret_field(
                key,
                request.passphrase.as_deref(),
                before
                    .as_ref()
                    .and_then(|row| row.passphrase_ciphertext.clone()),
            )?;
            let api_key_mask = request
                .api_key
                .as_deref()
                .map(mask_api_key)
                .or_else(|| before.as_ref().and_then(|row| row.api_key_mask.clone()));
            (
                api_key_ciphertext,
                api_secret_ciphertext,
                passphrase_ciphertext,
                api_key_mask,
            )
        } else {
            (None, None, None, None)
        };

    sqlx::query(
        r#"INSERT INTO market_source_credentials
           (provider, auth_type, api_key_ciphertext, api_secret_ciphertext,
            passphrase_ciphertext, api_key_mask, enabled, updated_by)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE auth_type = VALUES(auth_type),
                                   api_key_ciphertext = VALUES(api_key_ciphertext),
                                   api_secret_ciphertext = VALUES(api_secret_ciphertext),
                                   passphrase_ciphertext = VALUES(passphrase_ciphertext),
                                   api_key_mask = VALUES(api_key_mask),
                                   enabled = VALUES(enabled),
                                   updated_by = VALUES(updated_by)"#,
    )
    .bind(&provider)
    .bind(&auth_type)
    .bind(&api_key_ciphertext)
    .bind(&api_secret_ciphertext)
    .bind(&passphrase_ciphertext)
    .bind(&api_key_mask)
    .bind(request.enabled)
    .bind(admin_id)
    .execute(&mut *tx)
    .await?;

    let after = load_credential_in_tx(&mut tx, &provider).await?;
    insert_market_feed_audit_log_in_tx(
        &mut tx,
        MarketFeedAuditEntry {
            admin_id,
            action: "market_source_credential.upsert",
            target_type: "market_source_credential",
            target_id: after
                .provider
                .as_bytes()
                .iter()
                .fold(0_u64, |acc, byte| acc + u64::from(*byte)),
            before_json: before.as_ref().map(credential_audit_json),
            after_json: Some(credential_audit_json(&after)),
            reason: Some(request.reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(credential_response(after))
}

pub async fn load_enabled_credentials(
    pool: &Pool<MySql>,
    providers: &[String],
    key: Option<&str>,
) -> AppResult<Vec<MarketSourceCredentialSecret>> {
    let rows = sqlx::query_as::<_, MarketSourceCredentialRow>(
        r#"SELECT provider, auth_type, api_key_ciphertext, api_secret_ciphertext,
                  passphrase_ciphertext, api_key_mask, enabled
           FROM market_source_credentials
           WHERE enabled = TRUE"#,
    )
    .fetch_all(pool)
    .await?;
    let mut selected = Vec::new();
    for provider in providers {
        let provider = MarketFeedProvider::from_code(provider)?.code().to_owned();
        if let Some(row) = rows.iter().find(|row| row.provider == provider) {
            if row.auth_type == API_KEY_AUTH_TYPE {
                let key = key.ok_or_else(|| {
                    AppError::Internal("credential encryption key is not configured".to_owned())
                })?;
                selected.push(MarketSourceCredentialSecret {
                    provider,
                    auth_type: row.auth_type.clone(),
                    api_key: decrypt_optional_secret(row.api_key_ciphertext.as_deref(), key)?,
                    api_secret: decrypt_optional_secret(row.api_secret_ciphertext.as_deref(), key)?,
                    passphrase: decrypt_optional_secret(row.passphrase_ciphertext.as_deref(), key)?,
                });
            } else {
                selected.push(MarketSourceCredentialSecret {
                    provider,
                    auth_type: NONE_AUTH_TYPE.to_owned(),
                    api_key: None,
                    api_secret: None,
                    passphrase: None,
                });
            }
        }
    }
    Ok(selected)
}

pub async fn mark_reload_success(
    pool: &Pool<MySql>,
    version: u64,
) -> AppResult<MarketFeedConfigResponse> {
    sqlx::query(
        r#"UPDATE market_feed_configs
           SET applied_version = ?, last_reload_status = 'success', last_reload_error = NULL,
               last_reloaded_at = CURRENT_TIMESTAMP(6)
           WHERE name = ?"#,
    )
    .bind(version)
    .bind(DEFAULT_CONFIG_NAME)
    .execute(pool)
    .await?;
    load_config(pool).await?.ok_or(AppError::NotFound)
}

pub async fn mark_reload_skipped(
    pool: &Pool<MySql>,
    version: u64,
) -> AppResult<MarketFeedConfigResponse> {
    sqlx::query(
        r#"UPDATE market_feed_configs
           SET applied_version = ?, last_reload_status = 'skipped', last_reload_error = NULL,
               last_reloaded_at = CURRENT_TIMESTAMP(6)
           WHERE name = ?"#,
    )
    .bind(version)
    .bind(DEFAULT_CONFIG_NAME)
    .execute(pool)
    .await?;
    load_config(pool).await?.ok_or(AppError::NotFound)
}

pub async fn mark_reload_failed(
    pool: &Pool<MySql>,
    error: &str,
) -> AppResult<MarketFeedConfigResponse> {
    sqlx::query(
        r#"UPDATE market_feed_configs
           SET last_reload_status = 'failed', last_reload_error = ?, last_reloaded_at = CURRENT_TIMESTAMP(6)
           WHERE name = ?"#,
    )
    .bind(sanitize_reload_error(error))
    .bind(DEFAULT_CONFIG_NAME)
    .execute(pool)
    .await?;
    load_config(pool).await?.ok_or(AppError::NotFound)
}

pub async fn insert_reload_audit_log(
    pool: &Pool<MySql>,
    admin_id: u64,
    config: &MarketFeedConfigResponse,
    runtime: &MarketFeedRuntimeStatus,
    reason: String,
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    insert_market_feed_audit_log_in_tx(
        &mut tx,
        MarketFeedAuditEntry {
            admin_id,
            action: "market_feed_config.reload",
            target_type: "market_feed_config",
            target_id: config.id,
            before_json: None,
            after_json: Some(json!({
                "version": config.version,
                "applied_version": config.applied_version,
                "runtime": runtime,
            })),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

pub fn runtime_config_from_response(
    settings: &crate::config::Settings,
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

pub fn mask_api_key(value: &str) -> String {
    let value = value.trim();
    if value.len() < 8 {
        return "*".repeat(value.len());
    }
    format!("{}****{}", &value[..4], &value[value.len() - 4..])
}

pub fn encrypt_credential(plaintext: &str, key: &str) -> AppResult<String> {
    let key_bytes = encryption_key_bytes(key)?;
    let unbound_key = aead::UnboundKey::new(&aead::AES_256_GCM, key_bytes)
        .map_err(|_| AppError::Internal("credential encryption key is invalid".to_owned()))?;
    let key = aead::LessSafeKey::new(unbound_key);
    let rng = SystemRandom::new();
    let mut nonce_bytes = [0_u8; NONCE_LEN];
    rng.fill(&mut nonce_bytes)
        .map_err(|_| AppError::Internal("credential nonce generation failed".to_owned()))?;
    let nonce = aead::Nonce::assume_unique_for_key(nonce_bytes);
    let mut in_out = plaintext.as_bytes().to_vec();
    key.seal_in_place_append_tag(nonce, aead::Aad::empty(), &mut in_out)
        .map_err(|_| AppError::Internal("credential encryption failed".to_owned()))?;
    let mut output = nonce_bytes.to_vec();
    output.extend(in_out);
    Ok(STANDARD.encode(output))
}

pub fn decrypt_credential(ciphertext: &str, key: &str) -> AppResult<String> {
    let key_bytes = encryption_key_bytes(key)?;
    let mut payload = STANDARD
        .decode(ciphertext)
        .map_err(|_| AppError::Validation("credential ciphertext is invalid".to_owned()))?;
    if payload.len() <= NONCE_LEN {
        return Err(AppError::Validation(
            "credential ciphertext is invalid".to_owned(),
        ));
    }
    let mut nonce_bytes = [0_u8; NONCE_LEN];
    nonce_bytes.copy_from_slice(&payload[..NONCE_LEN]);
    let mut encrypted = payload.split_off(NONCE_LEN);
    let unbound_key = aead::UnboundKey::new(&aead::AES_256_GCM, key_bytes)
        .map_err(|_| AppError::Internal("credential encryption key is invalid".to_owned()))?;
    let key = aead::LessSafeKey::new(unbound_key);
    let plaintext = key
        .open_in_place(
            aead::Nonce::assume_unique_for_key(nonce_bytes),
            aead::Aad::empty(),
            &mut encrypted,
        )
        .map_err(|_| AppError::Validation("credential ciphertext is invalid".to_owned()))?;
    String::from_utf8(plaintext.to_vec())
        .map_err(|_| AppError::Validation("credential plaintext is invalid utf8".to_owned()))
}

pub fn validate_symbols(symbols: &[String], enabled: bool) -> AppResult<Vec<String>> {
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

pub fn validate_intervals(intervals: &[String]) -> AppResult<Vec<String>> {
    if intervals.is_empty() {
        return Err(AppError::Validation(
            "market feed intervals are required".to_owned(),
        ));
    }
    intervals
        .iter()
        .map(|interval| {
            KlineUpsertKey::new(interval.trim(), Utc::now())
                .map(|key| key.interval().to_owned())
                .map_err(|error| AppError::Validation(error.to_string()))
        })
        .collect()
}

pub fn validate_providers(providers: &[String]) -> AppResult<Vec<String>> {
    if providers.is_empty() {
        return Err(AppError::Validation(
            "market feed providers are required".to_owned(),
        ));
    }
    let mut selected = Vec::new();
    for provider in providers {
        let provider = MarketFeedProvider::from_code(provider)?.code().to_owned();
        if !selected.contains(&provider) {
            selected.push(provider);
        }
    }
    Ok(selected)
}

pub fn validate_reason(reason: Option<&str>) -> AppResult<()> {
    if reason
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
        .is_none()
    {
        return Err(AppError::Validation(
            "operation reason is required".to_owned(),
        ));
    }
    Ok(())
}

fn validate_auth_type(auth_type: &str) -> AppResult<String> {
    let normalized = auth_type.trim().to_ascii_lowercase();
    match normalized.as_str() {
        NONE_AUTH_TYPE | API_KEY_AUTH_TYPE => Ok(normalized),
        _ => Err(AppError::Validation(
            "market source credential auth_type is invalid".to_owned(),
        )),
    }
}

fn encrypt_secret_field(
    key: &str,
    new_value: Option<&str>,
    existing_ciphertext: Option<String>,
) -> AppResult<Option<String>> {
    match new_value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then_some(trimmed)
    }) {
        Some(value) => encrypt_credential(value, key).map(Some),
        None => Ok(existing_ciphertext),
    }
}

fn decrypt_optional_secret(ciphertext: Option<&str>, key: &str) -> AppResult<Option<String>> {
    ciphertext
        .map(|value| decrypt_credential(value, key))
        .transpose()
}

fn encryption_key_bytes(key: &str) -> AppResult<&[u8]> {
    let key = key.as_bytes();
    if key.len() != 32 {
        return Err(AppError::Validation(
            "credential encryption key must be exactly 32 bytes".to_owned(),
        ));
    }
    Ok(key)
}

fn config_response(row: MarketFeedConfigRow) -> MarketFeedConfigResponse {
    let needs_reload = Some(row.version) != row.applied_version;
    MarketFeedConfigResponse {
        id: row.id,
        name: row.name,
        symbols: row.symbols_json.0,
        intervals: row.intervals_json.0,
        providers: row.providers_json.0,
        enabled: row.enabled,
        version: row.version,
        applied_version: row.applied_version,
        needs_reload,
        last_reload_status: row.last_reload_status,
        last_reload_error: row.last_reload_error,
        last_reloaded_at: row.last_reloaded_at,
    }
}

fn credential_response(row: MarketSourceCredentialRow) -> MarketSourceCredentialResponse {
    MarketSourceCredentialResponse {
        provider: row.provider,
        auth_type: row.auth_type,
        api_key_mask: row.api_key_mask,
        enabled: row.enabled,
    }
}

async fn lock_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<Option<MarketFeedConfigResponse>> {
    let row = sqlx::query_as::<_, MarketFeedConfigRow>(
        r#"SELECT id, name, symbols_json, intervals_json, providers_json, enabled,
                  version, applied_version, last_reload_status, last_reload_error, last_reloaded_at
           FROM market_feed_configs
           WHERE name = ?
           FOR UPDATE"#,
    )
    .bind(DEFAULT_CONFIG_NAME)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.map(config_response))
}

async fn load_config_in_tx(tx: &mut Transaction<'_, MySql>) -> AppResult<MarketFeedConfigResponse> {
    let row = sqlx::query_as::<_, MarketFeedConfigRow>(
        r#"SELECT id, name, symbols_json, intervals_json, providers_json, enabled,
                  version, applied_version, last_reload_status, last_reload_error, last_reloaded_at
           FROM market_feed_configs
           WHERE name = ?"#,
    )
    .bind(DEFAULT_CONFIG_NAME)
    .fetch_one(&mut **tx)
    .await?;
    Ok(config_response(row))
}

async fn lock_credential_in_tx(
    tx: &mut Transaction<'_, MySql>,
    provider: &str,
) -> AppResult<Option<MarketSourceCredentialRow>> {
    let row = sqlx::query_as::<_, MarketSourceCredentialRow>(
        r#"SELECT provider, auth_type, api_key_ciphertext, api_secret_ciphertext,
                  passphrase_ciphertext, api_key_mask, enabled
           FROM market_source_credentials
           WHERE provider = ?
           FOR UPDATE"#,
    )
    .bind(provider)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row)
}

async fn load_credential_in_tx(
    tx: &mut Transaction<'_, MySql>,
    provider: &str,
) -> AppResult<MarketSourceCredentialRow> {
    sqlx::query_as::<_, MarketSourceCredentialRow>(
        r#"SELECT provider, auth_type, api_key_ciphertext, api_secret_ciphertext,
                  passphrase_ciphertext, api_key_mask, enabled
           FROM market_source_credentials
           WHERE provider = ?"#,
    )
    .bind(provider)
    .fetch_one(&mut **tx)
    .await
    .map_err(AppError::Database)
}

struct MarketFeedAuditEntry {
    admin_id: u64,
    action: &'static str,
    target_type: &'static str,
    target_id: u64,
    before_json: Option<Value>,
    after_json: Option<Value>,
    reason: Option<String>,
}

async fn insert_market_feed_audit_log_in_tx(
    tx: &mut Transaction<'_, MySql>,
    entry: MarketFeedAuditEntry,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, before_json, after_json, reason)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(entry.admin_id)
    .bind(entry.action)
    .bind(entry.target_type)
    .bind(entry.target_id.to_string())
    .bind(entry.before_json.map(SqlxJson))
    .bind(entry.after_json.map(SqlxJson))
    .bind(
        entry
            .reason
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty()),
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn config_audit_json(config: &MarketFeedConfigResponse) -> Value {
    json!({
        "id": config.id,
        "name": config.name,
        "symbols": config.symbols,
        "intervals": config.intervals,
        "providers": config.providers,
        "enabled": config.enabled,
        "version": config.version,
        "applied_version": config.applied_version,
        "last_reload_status": config.last_reload_status,
        "last_reload_error": config.last_reload_error,
        "last_reloaded_at": config.last_reloaded_at.map(|value| value.timestamp_millis()),
    })
}

fn credential_audit_json(row: &MarketSourceCredentialRow) -> Value {
    json!({
        "provider": row.provider,
        "auth_type": row.auth_type,
        "api_key_mask": row.api_key_mask,
        "enabled": row.enabled,
    })
}

fn sanitize_reload_error(error: &str) -> String {
    error.chars().take(512).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_api_key_without_exposing_plaintext() {
        assert_eq!(mask_api_key("abcd1234wxyz"), "abcd****wxyz");
        assert_eq!(mask_api_key("short"), "*****");
    }

    #[test]
    fn encrypts_and_decrypts_credential() {
        let key = "0123456789abcdef0123456789abcdef";
        let ciphertext = encrypt_credential("secret-value", key).unwrap();

        assert_ne!(ciphertext, "secret-value");
        assert_eq!(
            decrypt_credential(&ciphertext, key).unwrap(),
            "secret-value"
        );
    }

    #[test]
    fn validates_market_feed_config_values() {
        assert_eq!(
            validate_symbols(&["BTC-USDT".to_owned()], true).unwrap(),
            ["BTCUSDT"]
        );
        assert!(validate_symbols(&[], true).is_err());
        assert!(validate_symbols(&[], false).unwrap().is_empty());
        assert_eq!(
            validate_intervals(&["1m".to_owned(), "1h".to_owned()]).unwrap(),
            ["1m", "1h"]
        );
        assert!(validate_intervals(&["2m".to_owned()]).is_err());
        assert_eq!(
            validate_providers(&["htx".to_owned(), "huobi".to_owned(), "bitget".to_owned()])
                .unwrap(),
            ["htx", "bitget"]
        );
    }
}
