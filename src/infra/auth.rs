//! 认证会话基础设施：构造全局唯一的令牌管理器，统一承载用户、管理员与代理三端的登录态。
//! 生产入口以 Redis 为权威存储，键带固定前缀便于与其他业务键区分，也保证多实例看到同一份会话。
//! 令牌采用随机串而非自包含格式，因此服务端可以真正吊销会话，代价是每次校验都要回存储查询。
//! 会话超时复用配置里的访问令牌有效期，且不自动续期，客户端必须显式走刷新流程来延长登录态。

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

/// 连接 Redis 并构造全局认证会话管理器；生产访问令牌、登录态与互斥会话均以该存储为权威来源。
/// Redis 初始化失败必须阻止认证基础设施启动，禁止静默回退到内存存储而使旧会话失效或多实例状态分裂。
pub async fn connect(settings: &Settings) -> AppResult<Arc<SaTokenManager>> {
    let storage = RedisStorage::new(settings.exposed_redis_url(), SA_TOKEN_REDIS_KEY_PREFIX)
        .await
        .map_err(|error| AppError::Internal(format!("sa-token redis init failed: {error}")))?;

    Ok(auth_manager(settings, Arc::new(storage)))
}

/// 构造仅供测试或显式单进程场景使用的内存认证管理器；状态不会跨进程共享，也不会在重启后保留。
/// 调用方必须主动选择该入口，生产连接失败时不得把它当作降级路径。
pub fn memory_manager(settings: &Settings) -> Arc<SaTokenManager> {
    auth_manager(settings, Arc::new(MemoryStorage::new()))
}

/// 把任意会话存储实现与同一份令牌策略组合成管理器，是 Redis 版与内存版共用的最后一步。
/// 抽掉存储差异后两条路径的行为完全一致，避免测试环境与生产环境在超时、令牌格式上出现偏差。
fn auth_manager(settings: &Settings, storage: Arc<dyn SaStorage>) -> Arc<SaTokenManager> {
    Arc::new(SaTokenManager::new(
        storage,
        sa_token_config(settings).build_config(),
    ))
}

/// 固定全局令牌策略：会话超时取配置里的访问令牌有效期，令牌为六十四位随机串而非自包含格式。
/// 允许同一账号并发在线且不复用同一枚令牌，因此多设备登录会各自签发独立令牌而非共享一份。
/// 自动续期被关闭，会话到点即失效，续期只能由客户端显式走刷新接口完成，避免长期在线绕过有效期约束。
fn sa_token_config(settings: &Settings) -> sa_token_core::config::SaTokenConfigBuilder {
    SaTokenConfig::builder()
        .timeout(settings.jwt_access_ttl_seconds as i64)
        .token_style(TokenStyle::Random64)
        .storage_key_prefix(SA_TOKEN_STORAGE_KEY_PREFIX)
        .is_concurrent(true)
        .is_share(false)
        .auto_renew(false)
}
