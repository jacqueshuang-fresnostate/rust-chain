use crate::{config::Settings, error::AppResult};
use lapin::{Connection, ConnectionProperties};

/// 建立 RabbitMQ 跨上下文消息连接；连接失败直接上抛，由启动流程决定是否继续，禁止悄悄丢弃事件。
/// 本入口只负责连接，不声明队列、不消费消息，也不承担发布确认与重试策略。
pub async fn connect(settings: &Settings) -> AppResult<Connection> {
    Ok(Connection::connect(
        settings.exposed_rabbitmq_url(),
        ConnectionProperties::default(),
    )
    .await?)
}
