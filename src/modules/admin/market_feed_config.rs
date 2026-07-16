use crate::{
    config::Settings,
    error::AppResult,
    modules::admin::{
        application::{
            get_admin_market_feed_config, list_admin_market_feed_credentials,
            load_enabled_admin_market_feed_config, save_admin_market_feed_config,
            upsert_admin_market_feed_credential,
        },
        infrastructure::{
            AdminAuditLogEntry, insert_admin_audit_log_entry_in_tx,
            load_enabled_admin_market_source_credential_secrets,
            mark_admin_market_feed_reload_failed, mark_admin_market_feed_reload_skipped,
            mark_admin_market_feed_reload_success,
        },
        service::{
            market_feed_config_response, market_feed_reload_audit_json,
            market_feed_runtime_config_from_response as build_market_feed_runtime_config,
            validate_market_feed_intervals, validate_market_feed_providers,
            validate_market_feed_reason, validate_market_feed_symbols,
        },
    },
    workers::market_feed::{MarketFeedRuntimeConfig, MarketFeedRuntimeStatus},
};
use sqlx::{MySql, Pool};

pub use crate::modules::admin::presentation::{
    MarketFeedConfigResponse, MarketFeedStatusResponse, MarketSourceCredentialResponse,
    MarketSourceCredentialSecret, MarketSourceCredentialsResponse, ReloadMarketFeedRequest,
    ReloadMarketFeedResponse, SaveMarketFeedConfigRequest, UpsertMarketSourceCredentialRequest,
};

pub async fn load_config(pool: &Pool<MySql>) -> AppResult<Option<MarketFeedConfigResponse>> {
    get_admin_market_feed_config(Some(pool.clone())).await
}

pub async fn load_enabled_config_for_bootstrap(
    pool: &Pool<MySql>,
) -> AppResult<Option<MarketFeedConfigResponse>> {
    load_enabled_admin_market_feed_config(pool).await
}

pub async fn save_config(
    pool: &Pool<MySql>,
    admin_id: u64,
    request: SaveMarketFeedConfigRequest,
) -> AppResult<MarketFeedConfigResponse> {
    save_admin_market_feed_config(Some(pool.clone()), admin_id, request).await
}

pub async fn list_credentials(
    pool: &Pool<MySql>,
) -> AppResult<Vec<MarketSourceCredentialResponse>> {
    Ok(list_admin_market_feed_credentials(Some(pool.clone()))
        .await?
        .credentials)
}

pub async fn upsert_credential(
    pool: &Pool<MySql>,
    admin_id: u64,
    provider: String,
    key: Option<&str>,
    request: UpsertMarketSourceCredentialRequest,
) -> AppResult<MarketSourceCredentialResponse> {
    upsert_admin_market_feed_credential(Some(pool.clone()), admin_id, provider, key, request).await
}

pub async fn load_enabled_credentials(
    pool: &Pool<MySql>,
    providers: &[String],
    key: Option<&str>,
) -> AppResult<Vec<MarketSourceCredentialSecret>> {
    load_enabled_admin_market_source_credential_secrets(pool, providers, key).await
}

pub async fn mark_reload_success(
    pool: &Pool<MySql>,
    version: u64,
) -> AppResult<MarketFeedConfigResponse> {
    mark_admin_market_feed_reload_success(pool, version)
        .await
        .map(market_feed_config_response)
}

pub async fn mark_reload_skipped(
    pool: &Pool<MySql>,
    version: u64,
) -> AppResult<MarketFeedConfigResponse> {
    mark_admin_market_feed_reload_skipped(pool, version)
        .await
        .map(market_feed_config_response)
}

pub async fn mark_reload_failed(
    pool: &Pool<MySql>,
    error: &str,
) -> AppResult<MarketFeedConfigResponse> {
    mark_admin_market_feed_reload_failed(pool, error)
        .await
        .map(market_feed_config_response)
}

pub async fn insert_reload_audit_log(
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

pub fn runtime_config_from_response(
    settings: &Settings,
    config: &MarketFeedConfigResponse,
) -> AppResult<MarketFeedRuntimeConfig> {
    build_market_feed_runtime_config(settings, config)
}

pub fn validate_symbols(symbols: &[String], enabled: bool) -> AppResult<Vec<String>> {
    validate_market_feed_symbols(symbols, enabled)
}

pub fn validate_intervals(intervals: &[String]) -> AppResult<Vec<String>> {
    validate_market_feed_intervals(intervals)
}

pub fn validate_providers(providers: &[String]) -> AppResult<Vec<String>> {
    validate_market_feed_providers(providers)
}

pub fn validate_reason(reason: Option<&str>) -> AppResult<()> {
    validate_market_feed_reason(reason)
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_admin_market_feed_config_tests.rs"]
mod tests;
