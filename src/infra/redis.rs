use crate::{config::Settings, error::AppResult};
use redis::{Client, aio::ConnectionManager};

/// 使用暴露后的 Redis URL 建立带自动连接管理的可克隆句柄，供会话、缓存、行情和 worker 跨上下文共享。
/// URL、认证或初次连接错误直接上抛；本入口不选择 key 命名空间、不预读写数据，也不把 Redis 故障降级为本地缓存命中。
pub async fn connect(settings: &Settings) -> AppResult<ConnectionManager> {
    let client = Client::open(settings.exposed_redis_url())?;
    Ok(ConnectionManager::new(client).await?)
}
