use super::*;

pub async fn load_enabled_admin_market_feed_config(
    pool: &Pool<MySql>,
) -> AppResult<Option<MarketFeedConfigResponse>> {
    Ok(
        load_enabled_admin_market_feed_config_for_bootstrap_from_store(pool)
            .await?
            .map(market_feed_config_response),
    )
}

pub(crate) async fn get_admin_market_feed_config(
    pool: Option<Pool<MySql>>,
) -> AppResult<Option<MarketFeedConfigResponse>> {
    let pool = admin_mysql_pool(pool)?;
    Ok(load_admin_market_feed_config_from_store(&pool)
        .await?
        .map(market_feed_config_response))
}

pub(crate) async fn save_admin_market_feed_config(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: SaveMarketFeedConfigRequest,
) -> AppResult<MarketFeedConfigResponse> {
    validate_market_feed_reason(request.reason.as_deref())?;
    let symbols = validate_market_feed_symbols(&request.symbols, request.enabled)?;
    let intervals = validate_market_feed_intervals(&request.intervals)?;
    let providers = validate_market_feed_providers(&request.providers)?;
    let pool = admin_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = lock_admin_market_feed_config_in_tx(&mut tx).await?;
    let version = before
        .as_ref()
        .map(|config| config.version + 1)
        .unwrap_or(1);

    // 行情配置的订阅集合和版本号必须同事务更新，避免 reload 读取到半更新状态。
    upsert_admin_market_feed_config_in_tx(
        &mut tx,
        AdminMarketFeedConfigWrite {
            symbols,
            intervals,
            providers,
            enabled: request.enabled,
            version,
            updated_by: admin_id,
        },
    )
    .await?;
    let after = load_admin_market_feed_config_in_tx(&mut tx).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "market_feed_config.save",
            target_type: "market_feed_config",
            target_id: after.id,
            before_json: before.as_ref().map(market_feed_config_audit_json),
            after_json: Some(market_feed_config_audit_json(&after)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(market_feed_config_response(after))
}

pub(crate) async fn get_admin_market_feed_status(
    pool: Option<Pool<MySql>>,
    runtime: MarketFeedRuntimeStatus,
) -> AppResult<MarketFeedStatusResponse> {
    let pool = admin_mysql_pool(pool)?;
    let saved_config = load_admin_market_feed_config_from_store(&pool)
        .await?
        .map(market_feed_config_response);
    Ok(MarketFeedStatusResponse {
        saved_config,
        runtime,
    })
}

pub(crate) async fn list_admin_market_feed_credentials(
    pool: Option<Pool<MySql>>,
) -> AppResult<MarketSourceCredentialsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let credentials = list_admin_market_source_credentials_from_store(&pool)
        .await?
        .into_iter()
        .map(market_source_credential_response)
        .collect();
    Ok(MarketSourceCredentialsResponse { credentials })
}

pub(crate) async fn upsert_admin_market_feed_credential(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    provider: String,
    key: Option<&str>,
    request: UpsertMarketSourceCredentialRequest,
) -> AppResult<MarketSourceCredentialResponse> {
    validate_market_feed_reason(Some(&request.reason))?;
    let provider = crate::modules::market::adapters::MarketFeedProvider::from_code(&provider)?
        .code()
        .to_owned();
    let auth_type = validate_market_source_auth_type(&request.auth_type)?;
    let pool = admin_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = lock_admin_market_source_credential_in_tx(&mut tx, &provider).await?;
    let secret_fields =
        prepare_market_source_credential_secret_fields(&request, before.as_ref(), &auth_type, key)?;
    upsert_admin_market_source_credential_in_tx(
        &mut tx,
        AdminMarketSourceCredentialWrite {
            provider: provider.clone(),
            auth_type,
            api_key_ciphertext: secret_fields.api_key_ciphertext,
            api_secret_ciphertext: secret_fields.api_secret_ciphertext,
            passphrase_ciphertext: secret_fields.passphrase_ciphertext,
            api_key_mask: secret_fields.api_key_mask,
            enabled: request.enabled,
            updated_by: admin_id,
        },
    )
    .await?;
    let after = load_admin_market_source_credential_in_tx(&mut tx, &provider).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "market_source_credential.upsert",
            target_type: "market_source_credential",
            target_id: market_source_credential_target_id(&after.provider),
            before_json: before.as_ref().map(market_source_credential_audit_json),
            after_json: Some(market_source_credential_audit_json(&after)),
            reason: Some(request.reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(market_source_credential_response(after))
}

/// 将已保存的行情源配置加载到运行时监督器，并记录本次重载结果。
/// 调用方须已完成管理员鉴权、提供操作原因，且运行时必须配置行情监督器和所需凭证。
/// 禁用配置会停止监督器并标记跳过；启用配置在解密校验凭证后才执行实际重载。
/// 运行时切换与数据库状态并非同一事务；构建或重载失败会持久化失败状态、记录审计后返回原错误。
/// 重复调用会再次触发运行时重载和审计，不提供请求级幂等保证。
pub(crate) async fn reload_admin_market_feed_config(
    state: AppState,
    admin_id: u64,
    request: ReloadMarketFeedRequest,
) -> AppResult<ReloadMarketFeedResponse> {
    let reason = optional_string(request.reason)
        .ok_or_else(|| AppError::Validation("operation reason is required".to_owned()))?;
    let pool = admin_mysql_pool(state.mysql.clone())?;
    let config = load_admin_market_feed_config_from_store(&pool)
        .await?
        .ok_or(AppError::NotFound)?;
    let config_response = market_feed_config_response(config.clone());
    let supervisor = state
        .market_feed_supervisor
        .clone()
        .ok_or_else(|| AppError::Internal("market feed supervisor is not configured".to_owned()))?;

    if !config_response.enabled {
        supervisor.stop().await;
        let config = mark_admin_market_feed_reload_skipped(&pool, config_response.version).await?;
        let config = market_feed_config_response(config);
        let runtime = supervisor.status().await;
        insert_admin_market_feed_reload_audit(&pool, admin_id, &config, &runtime, reason).await?;
        return Ok(ReloadMarketFeedResponse { config, runtime });
    }

    let credentials = load_enabled_admin_market_source_credential_secrets(
        &pool,
        &config_response.providers,
        state.settings.exposed_credential_encryption_key(),
    )
    .await?;
    validate_loaded_market_feed_credentials(&config_response.providers, &credentials)?;
    drop(credentials);

    let runtime_config =
        match market_feed_runtime_config_from_response(&state.settings, &config_response) {
            Ok(runtime_config) => runtime_config,
            Err(error) => {
                let config =
                    mark_admin_market_feed_reload_failed(&pool, &error.to_string()).await?;
                let config = market_feed_config_response(config);
                let runtime = supervisor.record_failure(error.to_string()).await;
                insert_admin_market_feed_reload_audit(&pool, admin_id, &config, &runtime, reason)
                    .await?;
                return Err(error);
            }
        };

    let runtime = match supervisor
        .reload(state.clone(), runtime_config, config_response.version)
        .await
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let config = mark_admin_market_feed_reload_failed(&pool, &error.to_string()).await?;
            let config = market_feed_config_response(config);
            let runtime = supervisor.record_failure(error.to_string()).await;
            insert_admin_market_feed_reload_audit(&pool, admin_id, &config, &runtime, reason)
                .await?;
            return Err(error);
        }
    };
    let config = mark_admin_market_feed_reload_success(&pool, config_response.version).await?;
    let config = market_feed_config_response(config);
    insert_admin_market_feed_reload_audit(&pool, admin_id, &config, &runtime, reason).await?;
    Ok(ReloadMarketFeedResponse { config, runtime })
}

struct MarketSourceCredentialSecretFields {
    api_key_ciphertext: Option<String>,
    api_secret_ciphertext: Option<String>,
    passphrase_ciphertext: Option<String>,
    api_key_mask: Option<String>,
}

fn prepare_market_source_credential_secret_fields(
    request: &UpsertMarketSourceCredentialRequest,
    before: Option<&AdminMarketSourceCredentialRecord>,
    auth_type: &str,
    key: Option<&str>,
) -> AppResult<MarketSourceCredentialSecretFields> {
    if auth_type != MARKET_SOURCE_AUTH_TYPE_API_KEY {
        return Ok(MarketSourceCredentialSecretFields {
            api_key_ciphertext: None,
            api_secret_ciphertext: None,
            passphrase_ciphertext: None,
            api_key_mask: None,
        });
    }

    let key = key.ok_or_else(|| {
        AppError::Internal("credential encryption key is not configured".to_owned())
    })?;
    let api_key_ciphertext = encrypt_secret_field(
        key,
        request.api_key.as_deref(),
        before.and_then(|record| record.api_key_ciphertext.clone()),
    )?;
    let api_secret_ciphertext = encrypt_secret_field(
        key,
        request.api_secret.as_deref(),
        before.and_then(|record| record.api_secret_ciphertext.clone()),
    )?;
    let passphrase_ciphertext = encrypt_secret_field(
        key,
        request.passphrase.as_deref(),
        before.and_then(|record| record.passphrase_ciphertext.clone()),
    )?;
    let api_key_mask = request
        .api_key
        .as_deref()
        .map(mask_secret)
        .or_else(|| before.and_then(|record| record.api_key_mask.clone()));

    Ok(MarketSourceCredentialSecretFields {
        api_key_ciphertext,
        api_secret_ciphertext,
        passphrase_ciphertext,
        api_key_mask,
    })
}

async fn insert_admin_market_feed_reload_audit(
    pool: &Pool<MySql>,
    admin_id: u64,
    config: &MarketFeedConfigResponse,
    runtime: &MarketFeedRuntimeStatus,
    reason: String,
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "market_feed_config.reload",
            target_type: "market_feed_config",
            target_id: config.id,
            before_json: None,
            after_json: Some(market_feed_reload_audit_json(config, runtime)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

fn validate_loaded_market_feed_credentials(
    providers: &[String],
    credentials: &[MarketSourceCredentialSecret],
) -> AppResult<()> {
    for provider in providers {
        let missing_api_key = credentials
            .iter()
            .find(|credential| credential.provider == *provider)
            .is_some_and(|credential| {
                credential.auth_type == "api_key"
                    && credential.api_key.as_deref().unwrap_or("").is_empty()
            });
        if missing_api_key {
            return Err(AppError::Validation(format!(
                "market feed provider {provider} api_key is required"
            )));
        }
    }
    Ok(())
}
