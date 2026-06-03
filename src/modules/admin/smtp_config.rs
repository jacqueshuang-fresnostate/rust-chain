use crate::{
    error::{AppError, AppResult},
    infra::{
        email::{
            EmailMessage, EmailSender, SmtpEmailConfig, parse_smtp_security, smtp_security_code,
        },
        secrets::{decrypt_optional_secret, encrypt_secret_field, mask_secret},
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{MySql, Pool, Transaction, types::Json as SqlxJson};

const DEFAULT_SMTP_CONFIG_NAME: &str = "default";
const ADMIN_AUDIT_REASON_MAX_LEN: usize = 512;

#[derive(Debug, Deserialize)]
pub struct SaveSmtpConfigRequest {
    pub host: String,
    pub port: u16,
    pub security: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub from_email: String,
    pub from_name: Option<String>,
    pub enabled: bool,
    pub reason: Option<String>,
}

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
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct SendSmtpTestRequest {
    pub recipient: String,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct SendSmtpTestResponse {
    pub sent: bool,
    pub recipient: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct SmtpConfigRow {
    id: u64,
    name: String,
    host: String,
    port: u16,
    security: String,
    username_ciphertext: Option<String>,
    password_ciphertext: Option<String>,
    username_mask: Option<String>,
    from_email: String,
    from_name: Option<String>,
    enabled: bool,
}

pub async fn load_smtp_config(pool: &Pool<MySql>) -> AppResult<Option<SmtpConfigResponse>> {
    let row = sqlx::query_as::<_, SmtpConfigRow>(
        r#"SELECT id, name, host, port, security, username_ciphertext, password_ciphertext,
                  username_mask, from_email, from_name, enabled
           FROM smtp_configs
           WHERE name = ?"#,
    )
    .bind(DEFAULT_SMTP_CONFIG_NAME)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(smtp_config_response))
}

pub async fn save_smtp_config(
    pool: &Pool<MySql>,
    admin_id: u64,
    key: Option<&str>,
    request: SaveSmtpConfigRequest,
) -> AppResult<SmtpConfigResponse> {
    let reason = required_reason(request.reason.clone())?;
    let config = validate_save_request(&request)?;
    let mut tx = pool.begin().await?;
    let before = lock_smtp_config_in_tx(&mut tx).await?;
    let needs_key = request_has_new_secret(&request);
    let key = if needs_key {
        Some(key.ok_or_else(|| {
            AppError::Internal("credential encryption key is not configured".to_owned())
        })?)
    } else {
        key
    };

    let username_ciphertext = match key {
        Some(key) => encrypt_secret_field(
            key,
            request.username.as_deref(),
            before
                .as_ref()
                .and_then(|row| row.username_ciphertext.clone()),
        )?,
        None => before
            .as_ref()
            .and_then(|row| row.username_ciphertext.clone()),
    };
    let password_ciphertext = match key {
        Some(key) => encrypt_secret_field(
            key,
            request.password.as_deref(),
            before
                .as_ref()
                .and_then(|row| row.password_ciphertext.clone()),
        )?,
        None => before
            .as_ref()
            .and_then(|row| row.password_ciphertext.clone()),
    };
    if username_ciphertext.is_some() != password_ciphertext.is_some() {
        return Err(AppError::Validation(
            "smtp username and password must be configured together".to_owned(),
        ));
    }
    let username_mask = request
        .username
        .as_deref()
        .and_then(optional_str)
        .map(mask_secret)
        .or_else(|| before.as_ref().and_then(|row| row.username_mask.clone()));

    sqlx::query(
        r#"INSERT INTO smtp_configs
           (name, host, port, security, username_ciphertext, password_ciphertext,
            username_mask, from_email, from_name, enabled, updated_by)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE host = VALUES(host),
                                   port = VALUES(port),
                                   security = VALUES(security),
                                   username_ciphertext = VALUES(username_ciphertext),
                                   password_ciphertext = VALUES(password_ciphertext),
                                   username_mask = VALUES(username_mask),
                                   from_email = VALUES(from_email),
                                   from_name = VALUES(from_name),
                                   enabled = VALUES(enabled),
                                   updated_by = VALUES(updated_by)"#,
    )
    .bind(DEFAULT_SMTP_CONFIG_NAME)
    .bind(&config.host)
    .bind(config.port)
    .bind(&config.security)
    .bind(&username_ciphertext)
    .bind(&password_ciphertext)
    .bind(&username_mask)
    .bind(&config.from_email)
    .bind(&config.from_name)
    .bind(config.enabled)
    .bind(admin_id)
    .execute(&mut *tx)
    .await?;

    let after = load_smtp_config_in_tx(&mut tx).await?;
    insert_smtp_audit_log_in_tx(
        &mut tx,
        admin_id,
        "smtp_config.save",
        after.id,
        before.as_ref().map(smtp_config_audit_json),
        Some(smtp_config_audit_json(&after)),
        reason,
    )
    .await?;
    tx.commit().await?;
    Ok(smtp_config_response(after))
}

pub async fn send_smtp_test_email(
    pool: &Pool<MySql>,
    admin_id: u64,
    key: Option<&str>,
    sender: &dyn EmailSender,
    request: SendSmtpTestRequest,
) -> AppResult<SendSmtpTestResponse> {
    let reason = required_reason(request.reason)?;
    let recipient = validate_email(&request.recipient, "recipient")?;
    let row = load_enabled_smtp_config_row(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    let smtp = smtp_email_config(&row, key)?;
    let mut tx = pool.begin().await?;
    insert_smtp_audit_log_in_tx(
        &mut tx,
        admin_id,
        "smtp_config.test",
        row.id,
        Some(smtp_config_audit_json(&row)),
        Some(json!({
            "status": "attempted",
            "recipient": recipient.clone(),
            "config": smtp_config_audit_json(&row),
        })),
        reason,
    )
    .await?;
    tx.commit().await?;

    sender
        .send(
            smtp,
            EmailMessage {
                to: recipient.clone(),
                subject: "SMTP test".to_owned(),
                text_body: "SMTP configuration test email.".to_owned(),
            },
        )
        .await?;

    Ok(SendSmtpTestResponse {
        sent: true,
        recipient,
    })
}

pub async fn load_enabled_smtp_config(
    pool: &Pool<MySql>,
    key: Option<&str>,
) -> AppResult<Option<SmtpEmailConfig>> {
    load_enabled_smtp_config_row(pool)
        .await?
        .map(|row| smtp_email_config(&row, key))
        .transpose()
}

fn validate_save_request(request: &SaveSmtpConfigRequest) -> AppResult<ValidatedSmtpConfig> {
    let host = optional_string(Some(request.host.clone()))
        .ok_or_else(|| AppError::Validation("smtp host is required".to_owned()))?;
    if host.len() > 255 {
        return Err(AppError::Validation("smtp host is too long".to_owned()));
    }
    if request.port == 0 {
        return Err(AppError::Validation("smtp port is invalid".to_owned()));
    }
    let security = smtp_security_code(parse_smtp_security(&request.security)?).to_owned();
    let from_email = validate_email(&request.from_email, "from_email")?;
    let from_name = optional_string(request.from_name.clone());
    if let Some(from_name) = &from_name
        && from_name.len() > 128
    {
        return Err(AppError::Validation("from_name is too long".to_owned()));
    }

    Ok(ValidatedSmtpConfig {
        host,
        port: request.port,
        security,
        from_email,
        from_name,
        enabled: request.enabled,
    })
}

fn smtp_email_config(row: &SmtpConfigRow, key: Option<&str>) -> AppResult<SmtpEmailConfig> {
    let key = if row.username_ciphertext.is_some() || row.password_ciphertext.is_some() {
        Some(key.ok_or_else(|| {
            AppError::Internal("credential encryption key is not configured".to_owned())
        })?)
    } else {
        None
    };
    let username = match key {
        Some(key) => decrypt_optional_secret(row.username_ciphertext.as_deref(), key)?,
        None => None,
    };
    let password = match key {
        Some(key) => decrypt_optional_secret(row.password_ciphertext.as_deref(), key)?,
        None => None,
    };
    Ok(SmtpEmailConfig {
        host: row.host.clone(),
        port: row.port,
        security: parse_smtp_security(&row.security)?,
        username,
        password,
        from_email: row.from_email.clone(),
        from_name: row.from_name.clone(),
    })
}

struct ValidatedSmtpConfig {
    host: String,
    port: u16,
    security: String,
    from_email: String,
    from_name: Option<String>,
    enabled: bool,
}

fn request_has_new_secret(request: &SaveSmtpConfigRequest) -> bool {
    request.username.as_deref().and_then(optional_str).is_some()
        || request.password.as_deref().and_then(optional_str).is_some()
}

async fn lock_smtp_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<Option<SmtpConfigRow>> {
    let row = sqlx::query_as::<_, SmtpConfigRow>(
        r#"SELECT id, name, host, port, security, username_ciphertext, password_ciphertext,
                  username_mask, from_email, from_name, enabled
           FROM smtp_configs
           WHERE name = ?
           FOR UPDATE"#,
    )
    .bind(DEFAULT_SMTP_CONFIG_NAME)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row)
}

async fn load_smtp_config_in_tx(tx: &mut Transaction<'_, MySql>) -> AppResult<SmtpConfigRow> {
    sqlx::query_as::<_, SmtpConfigRow>(
        r#"SELECT id, name, host, port, security, username_ciphertext, password_ciphertext,
                  username_mask, from_email, from_name, enabled
           FROM smtp_configs
           WHERE name = ?"#,
    )
    .bind(DEFAULT_SMTP_CONFIG_NAME)
    .fetch_one(&mut **tx)
    .await
    .map_err(AppError::Database)
}

async fn load_enabled_smtp_config_row(pool: &Pool<MySql>) -> AppResult<Option<SmtpConfigRow>> {
    let row = sqlx::query_as::<_, SmtpConfigRow>(
        r#"SELECT id, name, host, port, security, username_ciphertext, password_ciphertext,
                  username_mask, from_email, from_name, enabled
           FROM smtp_configs
           WHERE name = ? AND enabled = TRUE"#,
    )
    .bind(DEFAULT_SMTP_CONFIG_NAME)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

async fn insert_smtp_audit_log_in_tx(
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
           VALUES (?, ?, 'smtp_config', ?, ?, ?, ?)"#,
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

fn smtp_config_response(row: SmtpConfigRow) -> SmtpConfigResponse {
    SmtpConfigResponse {
        id: row.id,
        name: row.name,
        host: row.host,
        port: row.port,
        security: row.security,
        username_mask: row.username_mask,
        password_set: row.password_ciphertext.is_some(),
        from_email: row.from_email,
        from_name: row.from_name,
        enabled: row.enabled,
    }
}

fn smtp_config_audit_json(row: &SmtpConfigRow) -> Value {
    json!({
        "id": row.id,
        "name": row.name,
        "host": row.host,
        "port": row.port,
        "security": row.security,
        "username_mask": row.username_mask,
        "password_set": row.password_ciphertext.is_some(),
        "from_email": row.from_email,
        "from_name": row.from_name,
        "enabled": row.enabled,
    })
}

fn validate_email(value: &str, field: &str) -> AppResult<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_smtp_save_request() {
        let request = SaveSmtpConfigRequest {
            host: " smtp.example.test ".to_owned(),
            port: 587,
            security: "STARTTLS".to_owned(),
            username: None,
            password: None,
            from_email: " noreply@example.test ".to_owned(),
            from_name: Some(" Exchange ".to_owned()),
            enabled: true,
            reason: Some("configure smtp".to_owned()),
        };

        let config = validate_save_request(&request).unwrap();
        assert_eq!(config.host, "smtp.example.test");
        assert_eq!(config.security, "starttls");
        assert_eq!(config.from_email, "noreply@example.test");
        assert_eq!(config.from_name.as_deref(), Some("Exchange"));
    }

    #[test]
    fn rejects_invalid_smtp_values() {
        let mut request = SaveSmtpConfigRequest {
            host: "smtp.example.test".to_owned(),
            port: 0,
            security: "ssl".to_owned(),
            username: None,
            password: None,
            from_email: "noreply.example.test".to_owned(),
            from_name: None,
            enabled: true,
            reason: Some("configure smtp".to_owned()),
        };

        assert!(validate_save_request(&request).is_err());
        request.port = 587;
        assert!(validate_save_request(&request).is_err());
        request.security = "tls".to_owned();
        assert!(validate_save_request(&request).is_err());
    }
}
