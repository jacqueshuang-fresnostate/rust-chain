use crate::{config::Settings, error::AppResult};
use redis::{Client, aio::ConnectionManager};

/// 建立可克隆的 Redis 连接管理器，供缓存、会话、风控与 worker 共享；URL 或握手错误直接阻止依赖方启动。
/// 本入口不选择 key 命名空间，也不把 Redis 故障转换为本地缓存命中。
pub async fn connect(settings: &Settings) -> AppResult<ConnectionManager> {
    let client = Client::open(settings.exposed_redis_url())?;
    Ok(ConnectionManager::new(client).await?)
}
