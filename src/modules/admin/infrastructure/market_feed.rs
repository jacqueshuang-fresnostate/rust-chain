use super::*;
use crate::{
    modules::admin::service::MarketFeedRuntimeStatusSource, state::AppState,
    workers::market_feed::MarketFeedRuntimeStatus,
};

impl MarketFeedRuntimeStatusSource for AppState {
    /// 从应用状态中的行情监督器读取只读快照；读取过程不会改变后台任务生命周期。
    /// 未安装监督器表示当前部署未启用该运行组件，此时保持历史行为并返回空状态。
    async fn market_feed_runtime_status(&self) -> MarketFeedRuntimeStatus {
        match &self.market_feed_supervisor {
            Some(supervisor) => supervisor.status().await,
            None => MarketFeedRuntimeStatus::default(),
        }
    }
}

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

/// 按传入主键或筛选条件从连接池读取行情订阅配置并映射为应用层所需的可选记录。
/// 行情订阅配置不追加行锁，查询不创建事务；记录缺失时返回空值，SQL 或字段解码失败直接返回错误，不产生审计副作用。
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

/// 仅限启用记录、供启动装载从连接池读取行情订阅配置并映射为应用层所需的可选记录。
/// 行情订阅配置不追加行锁，查询不创建事务；记录缺失时按查询本身语义处理，SQL 或字段解码失败直接返回错误，不产生审计副作用。
pub(crate) async fn load_enabled_admin_market_feed_config_for_bootstrap(
    pool: &Pool<MySql>,
) -> AppResult<Option<AdminMarketFeedConfigRecord>> {
    Ok(load_admin_market_feed_config(pool)
        .await?
        .filter(|record| record.enabled))
}

/// 在调用方事务中按固定 default 名称锁定行情订阅配置，并以 Option 返回保存前快照。
/// 命中记录以 `FOR UPDATE` 持锁至保存事务结束；首次配置返回 None，函数不创建默认行、提交或触发监督器。
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

/// 在调用方事务中按固定 default 名称回读行情订阅配置，返回版本、应用版本和最近重载信息。
/// 查询不追加锁且要求记录存在；无记录或 JSON/SQL 映射失败使保存用例回滚，函数不提交或执行重载。
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

/// 在调用方事务中按固定 default 名称新增或覆盖订阅符号、周期、提供商、启用开关和保存版本。
/// 唯一键命中时不改 applied_version/重载状态；调用方须先锁旧配置并与版本审计原子提交，函数不通知监督器。
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

/// 读取全部行情源凭据的脱敏元数据，按提供方升序返回是否配置 API key/secret/passphrase。
/// 查询不分页、不返回密文也不加锁；并发 upsert 可能改变下一次结果，SQL 或行映射失败直接返回错误。
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

/// 在调用方事务中按 provider 锁定行情源凭据，并返回含密文的可选旧记录。
/// 首次写入返回 None；命中行锁持有至凭据事务结束，函数不解密、提交或重载行情源。
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

/// 在调用方事务中按 provider 回读已保存行情源凭据，供脱敏响应和审计组装。
/// 查询不追加锁且要求记录存在；无记录或 SQL 映射失败由上层回滚，函数不解密或暴露密文到接口响应。
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

/// 在调用方事务中按 provider 新增或覆盖认证类型、三类凭据密文、掩码和启用状态。
/// upsert 使相同 provider 重放覆盖当前值；调用方负责先锁记录、完成加密并与脱敏审计统一提交，函数不重载运行时。
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

/// 从全部 enabled 凭据中选择运行配置要求的 provider，解密并返回监督器使用的认证材料。
/// 查询不加锁，结果按 providers 输入筛选；缺少必需密钥、密文解密或数据库失败返回错误，函数不修改凭据或启动连接。
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

/// 将指定保存版本标记为已成功应用，清空错误并记录成功时间，供后台判断无需再次重载。
/// 该更新通过连接池独立提交且不操作监督器；SQL 失败返回错误，调用方随后决定是否写审计。
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

/// 将禁用配置的重载标记为已跳过，并把当前版本视为已处理以清除待重载提示。
/// 该更新通过连接池独立提交且不停止监督器；运行时停止由应用层负责，SQL 失败直接返回错误。
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

/// 记录行情重载失败状态和清理后的错误摘要，同时保留未应用版本供后台继续提示重试。
/// 该更新通过连接池独立提交且不恢复监督器；SQL 失败返回错误，重复记录仅覆盖最近失败信息。
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
