use super::*;

#[derive(Debug, Clone, sqlx::FromRow)]
struct AdminMarketFeedConfigRow {
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
    last_reloaded_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct AdminMarketSourceCredentialRow {
    provider: String,
    auth_type: String,
    api_key_ciphertext: Option<String>,
    api_secret_ciphertext: Option<String>,
    passphrase_ciphertext: Option<String>,
    api_key_mask: Option<String>,
    enabled: bool,
}

pub(crate) async fn load_admin_market_feed_config(
    pool: &Pool<MySql>,
) -> AppResult<Option<AdminMarketFeedConfigRecord>> {
    let row =
        sqlx::query_as::<_, AdminMarketFeedConfigRow>(&select_admin_market_feed_config_sql(false))
            .bind(DEFAULT_MARKET_FEED_CONFIG_NAME)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(admin_market_feed_config_record))
}

pub(crate) async fn load_enabled_admin_market_feed_config_for_bootstrap(
    pool: &Pool<MySql>,
) -> AppResult<Option<AdminMarketFeedConfigRecord>> {
    Ok(load_admin_market_feed_config(pool)
        .await?
        .filter(|record| record.enabled))
}

pub(crate) async fn lock_admin_market_feed_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<Option<AdminMarketFeedConfigRecord>> {
    sqlx::query_as::<_, AdminMarketFeedConfigRow>(&select_admin_market_feed_config_sql(true))
        .bind(DEFAULT_MARKET_FEED_CONFIG_NAME)
        .fetch_optional(&mut **tx)
        .await
        .map(|row| row.map(admin_market_feed_config_record))
        .map_err(AppError::Database)
}

pub(crate) async fn load_admin_market_feed_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<AdminMarketFeedConfigRecord> {
    sqlx::query_as::<_, AdminMarketFeedConfigRow>(&select_admin_market_feed_config_sql(false))
        .bind(DEFAULT_MARKET_FEED_CONFIG_NAME)
        .fetch_one(&mut **tx)
        .await
        .map(admin_market_feed_config_record)
        .map_err(AppError::Database)
}

pub(crate) async fn upsert_admin_market_feed_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminMarketFeedConfigWrite,
) -> AppResult<()> {
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
    .bind(DEFAULT_MARKET_FEED_CONFIG_NAME)
    .bind(SqlxJson(input.symbols))
    .bind(SqlxJson(input.intervals))
    .bind(SqlxJson(input.providers))
    .bind(input.enabled)
    .bind(input.version)
    .bind(input.updated_by)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn list_admin_market_source_credentials(
    pool: &Pool<MySql>,
) -> AppResult<Vec<AdminMarketSourceCredentialRecord>> {
    let rows = sqlx::query_as::<_, AdminMarketSourceCredentialRow>(
        r#"SELECT provider, auth_type, api_key_ciphertext, api_secret_ciphertext,
                  passphrase_ciphertext, api_key_mask, enabled
           FROM market_source_credentials
           ORDER BY provider ASC"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(admin_market_source_credential_record)
        .collect())
}

pub(crate) async fn lock_admin_market_source_credential_in_tx(
    tx: &mut Transaction<'_, MySql>,
    provider: &str,
) -> AppResult<Option<AdminMarketSourceCredentialRecord>> {
    let row = sqlx::query_as::<_, AdminMarketSourceCredentialRow>(
        r#"SELECT provider, auth_type, api_key_ciphertext, api_secret_ciphertext,
                  passphrase_ciphertext, api_key_mask, enabled
           FROM market_source_credentials
           WHERE provider = ?
           FOR UPDATE"#,
    )
    .bind(provider)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.map(admin_market_source_credential_record))
}

pub(crate) async fn load_admin_market_source_credential_in_tx(
    tx: &mut Transaction<'_, MySql>,
    provider: &str,
) -> AppResult<AdminMarketSourceCredentialRecord> {
    sqlx::query_as::<_, AdminMarketSourceCredentialRow>(
        r#"SELECT provider, auth_type, api_key_ciphertext, api_secret_ciphertext,
                  passphrase_ciphertext, api_key_mask, enabled
           FROM market_source_credentials
           WHERE provider = ?"#,
    )
    .bind(provider)
    .fetch_one(&mut **tx)
    .await
    .map(admin_market_source_credential_record)
    .map_err(AppError::Database)
}

pub(crate) async fn upsert_admin_market_source_credential_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminMarketSourceCredentialWrite,
) -> AppResult<()> {
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
    .bind(&input.provider)
    .bind(&input.auth_type)
    .bind(&input.api_key_ciphertext)
    .bind(&input.api_secret_ciphertext)
    .bind(&input.passphrase_ciphertext)
    .bind(&input.api_key_mask)
    .bind(input.enabled)
    .bind(input.updated_by)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn load_enabled_admin_market_source_credential_secrets(
    pool: &Pool<MySql>,
    providers: &[String],
    key: Option<&str>,
) -> AppResult<Vec<MarketSourceCredentialSecret>> {
    let rows = sqlx::query_as::<_, AdminMarketSourceCredentialRow>(
        r#"SELECT provider, auth_type, api_key_ciphertext, api_secret_ciphertext,
                  passphrase_ciphertext, api_key_mask, enabled
           FROM market_source_credentials
           WHERE enabled = TRUE"#,
    )
    .fetch_all(pool)
    .await?;
    let records: Vec<_> = rows
        .into_iter()
        .map(admin_market_source_credential_record)
        .collect();
    let mut selected = Vec::new();
    for provider in providers {
        let provider = MarketFeedProvider::from_code(provider)?.code().to_owned();
        if let Some(record) = records.iter().find(|record| record.provider == provider) {
            if record.auth_type == MARKET_SOURCE_AUTH_TYPE_API_KEY {
                let key = key.ok_or_else(|| {
                    AppError::Internal("credential encryption key is not configured".to_owned())
                })?;
                selected.push(MarketSourceCredentialSecret {
                    provider,
                    auth_type: record.auth_type.clone(),
                    api_key: decrypt_optional_secret(record.api_key_ciphertext.as_deref(), key)?,
                    api_secret: decrypt_optional_secret(
                        record.api_secret_ciphertext.as_deref(),
                        key,
                    )?,
                    passphrase: decrypt_optional_secret(
                        record.passphrase_ciphertext.as_deref(),
                        key,
                    )?,
                });
            } else {
                selected.push(MarketSourceCredentialSecret {
                    provider,
                    auth_type: MARKET_SOURCE_AUTH_TYPE_NONE.to_owned(),
                    api_key: None,
                    api_secret: None,
                    passphrase: None,
                });
            }
        }
    }
    Ok(selected)
}

pub(crate) async fn mark_admin_market_feed_reload_success(
    pool: &Pool<MySql>,
    version: u64,
) -> AppResult<AdminMarketFeedConfigRecord> {
    sqlx::query(
        r#"UPDATE market_feed_configs
           SET applied_version = ?, last_reload_status = 'success', last_reload_error = NULL,
               last_reloaded_at = CURRENT_TIMESTAMP(6)
           WHERE name = ?"#,
    )
    .bind(version)
    .bind(DEFAULT_MARKET_FEED_CONFIG_NAME)
    .execute(pool)
    .await?;
    load_admin_market_feed_config(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn mark_admin_market_feed_reload_skipped(
    pool: &Pool<MySql>,
    version: u64,
) -> AppResult<AdminMarketFeedConfigRecord> {
    sqlx::query(
        r#"UPDATE market_feed_configs
           SET applied_version = ?, last_reload_status = 'skipped', last_reload_error = NULL,
               last_reloaded_at = CURRENT_TIMESTAMP(6)
           WHERE name = ?"#,
    )
    .bind(version)
    .bind(DEFAULT_MARKET_FEED_CONFIG_NAME)
    .execute(pool)
    .await?;
    load_admin_market_feed_config(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn mark_admin_market_feed_reload_failed(
    pool: &Pool<MySql>,
    error: &str,
) -> AppResult<AdminMarketFeedConfigRecord> {
    sqlx::query(
        r#"UPDATE market_feed_configs
           SET last_reload_status = 'failed', last_reload_error = ?, last_reloaded_at = CURRENT_TIMESTAMP(6)
           WHERE name = ?"#,
    )
    .bind(sanitize_market_feed_reload_error(error))
    .bind(DEFAULT_MARKET_FEED_CONFIG_NAME)
    .execute(pool)
    .await?;
    load_admin_market_feed_config(pool)
        .await?
        .ok_or(AppError::NotFound)
}

fn select_admin_market_feed_config_sql(for_update: bool) -> String {
    let mut sql = r#"SELECT id, name, symbols_json, intervals_json, providers_json, enabled,
              version, applied_version, last_reload_status, last_reload_error, last_reloaded_at
       FROM market_feed_configs
       WHERE name = ?"#
        .to_owned();
    if for_update {
        sql.push_str(" FOR UPDATE");
    }
    sql
}

fn admin_market_feed_config_record(row: AdminMarketFeedConfigRow) -> AdminMarketFeedConfigRecord {
    AdminMarketFeedConfigRecord {
        id: row.id,
        name: row.name,
        symbols: row.symbols_json.0,
        intervals: row.intervals_json.0,
        providers: row.providers_json.0,
        enabled: row.enabled,
        version: row.version,
        applied_version: row.applied_version,
        last_reload_status: row.last_reload_status,
        last_reload_error: row.last_reload_error,
        last_reloaded_at: row.last_reloaded_at,
    }
}

fn admin_market_source_credential_record(
    row: AdminMarketSourceCredentialRow,
) -> AdminMarketSourceCredentialRecord {
    AdminMarketSourceCredentialRecord {
        provider: row.provider,
        auth_type: row.auth_type,
        api_key_ciphertext: row.api_key_ciphertext,
        api_secret_ciphertext: row.api_secret_ciphertext,
        passphrase_ciphertext: row.passphrase_ciphertext,
        api_key_mask: row.api_key_mask,
        enabled: row.enabled,
    }
}
