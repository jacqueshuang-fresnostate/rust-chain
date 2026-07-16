use crate::{
    config::Settings,
    error::{AppError, AppResult},
};
use sa_token_adapter::storage::SaStorage;
use sa_token_core::{SaTokenConfig, SaTokenManager, config::TokenStyle};
use sa_token_storage_memory::MemoryStorage;
use sa_token_storage_redis::RedisStorage;
use std::sync::Arc;

pub const SA_TOKEN_REDIS_KEY_PREFIX: &str = "exchange:sa-token:";
pub const SA_TOKEN_STORAGE_KEY_PREFIX: &str = "auth:";

pub async fn connect(settings: &Settings) -> AppResult<Arc<SaTokenManager>> {
    let storage = RedisStorage::new(settings.exposed_redis_url(), SA_TOKEN_REDIS_KEY_PREFIX)
        .await
        .map_err(|error| AppError::Internal(format!("sa-token redis init failed: {error}")))?;

    Ok(auth_manager(settings, Arc::new(storage)))
}

pub fn memory_manager(settings: &Settings) -> Arc<SaTokenManager> {
    auth_manager(settings, Arc::new(MemoryStorage::new()))
}

fn auth_manager(settings: &Settings, storage: Arc<dyn SaStorage>) -> Arc<SaTokenManager> {
    Arc::new(SaTokenManager::new(
        storage,
        sa_token_config(settings).build_config(),
    ))
}

fn sa_token_config(settings: &Settings) -> sa_token_core::config::SaTokenConfigBuilder {
    SaTokenConfig::builder()
        .timeout(settings.jwt_access_ttl_seconds as i64)
        .token_style(TokenStyle::Random64)
        .storage_key_prefix(SA_TOKEN_STORAGE_KEY_PREFIX)
        .is_concurrent(true)
        .is_share(false)
        .auto_renew(false)
}
