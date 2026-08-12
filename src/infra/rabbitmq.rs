use crate::{config::Settings, error::AppResult};
use lapin::{Connection, ConnectionProperties};

/// 使用暴露后的 RabbitMQ URL 建立跨上下文 AMQP 连接；握手、认证或网络错误直接上抛，启动方不得降级为静默丢弃事件。
/// 返回成功只提供共享连接，不声明 exchange/queue、不设置绑定、不启用消费或 publisher confirm；这些路由与确认副作用由 events 适配器拥有。
pub async fn connect(settings: &Settings) -> AppResult<Connection> {
    Ok(Connection::connect(
        settings.exposed_rabbitmq_url(),
        ConnectionProperties::default(),
    )
    .await?)
}
