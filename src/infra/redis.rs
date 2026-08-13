//! Redis 连接装配：同一实例同时承载登录会话、行情缓存、限频计数与多个后台任务的协调键。
//! 返回的连接管理器可克隆且内建断线重连，各处共享同一条底层连接，因此挂载到共享状态不会放大连接数。
//! 键的命名空间由各使用方自行约定，本文件不划分前缀，也不预置任何过期策略或淘汰规则。

use crate::{config::Settings, error::AppResult};
use redis::{Client, aio::ConnectionManager};

/// 使用暴露后的 Redis URL 建立带自动连接管理的可克隆句柄，供会话、缓存、行情和 worker 跨上下文共享。
/// URL、认证或初次连接错误直接上抛；本入口不选择 key 命名空间、不预读写数据，也不把 Redis 故障降级为本地缓存命中。
pub async fn connect(settings: &Settings) -> AppResult<ConnectionManager> {
    let client = Client::open(settings.exposed_redis_url())?;
    Ok(ConnectionManager::new(client).await?)
}
