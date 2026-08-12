//! RabbitMQ outbox 发布与 inbox delivery 适配。

use super::{
    ConsumedInboxMessage, EventInboxConsumerService, EventInboxHandler, InboundEventMessage,
    InboxDeliveryDisposition, OutboxMessage, OutboxPublisher, ProcessedInboxDelivery,
};
use crate::{
    error::{AppError, AppResult},
    modules::events::EventInboxRepository,
};
use axum::async_trait;
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use lapin::{
    BasicProperties, Channel, ExchangeKind,
    message::Delivery,
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicPublishOptions, BasicRejectOptions,
        ExchangeDeclareOptions,
    },
    types::FieldTable,
};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RabbitMqPublishEnvelope {
    pub exchange: String,
    pub routing_key: String,
    pub message_id: String,
    pub content_type: String,
    pub payload: Vec<u8>,
}

impl RabbitMqPublishEnvelope {
    /// 将 outbox 行编码为跨上下文 JSON envelope：聚合、事件类型、routing key、幂等键和业务 payload 全部保留。
    /// AMQP message_id 固定使用 outbox 幂等键、content-type 为 JSON、后续属性设为持久消息；这里只序列化，不声明 exchange 或发送网络请求。
    pub fn from_outbox(exchange: impl Into<String>, outbox: &OutboxMessage) -> AppResult<Self> {
        let payload = serde_json::json!({
            "aggregate_type": outbox.aggregate_type,
            "aggregate_id": outbox.aggregate_id,
            "event_type": outbox.event_type,
            "routing_key": outbox.routing_key,
            "idempotency_key": outbox.idempotency_key,
            "payload": outbox.payload,
        });

        Ok(Self {
            exchange: exchange.into(),
            routing_key: outbox.routing_key.clone(),
            message_id: outbox.idempotency_key.clone(),
            content_type: "application/json".to_owned(),
            payload: serde_json::to_vec(&payload).map_err(|error| {
                AppError::Internal(format!("failed to serialize outbox payload: {error}"))
            })?,
        })
    }

    fn properties(&self) -> BasicProperties {
        BasicProperties::default()
            .with_message_id(self.message_id.clone().into())
            .with_content_type(self.content_type.clone().into())
            .with_delivery_mode(2)
    }
}

#[derive(Clone)]
pub struct RabbitMqOutboxPublisher {
    connection: Arc<lapin::Connection>,
    exchange: String,
}

impl RabbitMqOutboxPublisher {
    /// 绑定共享 RabbitMQ 连接与唯一目标 exchange；每条 outbox 自带 routing key，publisher 不改写其业务路由范围。
    /// 构造不创建 channel、不声明 exchange 或发布消息；重放去重依赖 message_id 中的 outbox 幂等键，而不是连接状态。
    pub fn new(connection: Arc<lapin::Connection>, exchange: impl Into<String>) -> Self {
        Self {
            connection,
            exchange: exchange.into(),
        }
    }
}

#[async_trait]
impl OutboxPublisher for RabbitMqOutboxPublisher {
    /// 为本条消息创建 channel，幂等声明 durable topic exchange，并按 outbox routing key 以 `delivery_mode=2` 发布 JSON envelope。
    /// 等待 `basic_publish` 返回的 future 后成功；本实现未调用 `confirm_select`，因此结果是 `NotRequested` 而非 broker ACK，不能证明 broker 已持久接收。
    /// channel、声明或发送错误均不更新 outbox并由上层落 retry/dead-letter；发送成功与数据库标记之间仍有重复/丢失窗口，消费者须按 message_id 去重。
    async fn publish(&self, message: &OutboxMessage) -> AppResult<()> {
        let envelope = RabbitMqPublishEnvelope::from_outbox(&self.exchange, message)?;
        let channel = self.connection.create_channel().await?;
        channel
            .exchange_declare(
                &envelope.exchange,
                ExchangeKind::Topic,
                ExchangeDeclareOptions {
                    durable: true,
                    ..ExchangeDeclareOptions::default()
                },
                FieldTable::default(),
            )
            .await?;
        channel
            .basic_publish(
                &envelope.exchange,
                &envelope.routing_key,
                BasicPublishOptions::default(),
                &envelope.payload,
                envelope.properties(),
            )
            .await?
            .await?;

        Ok(())
    }
}

#[derive(Clone)]
pub struct RabbitMqInboxConsumer {
    connection: Arc<lapin::Connection>,
    queue_name: String,
    consumer_tag: String,
}

impl RabbitMqInboxConsumer {
    /// 绑定 RabbitMQ 连接、队列名与 consumer tag；调用方须预先校验名称并确保队列已声明。
    /// 构造不创建 channel 或开始消费，也不拥有 inbox 事务。
    pub fn new(
        connection: Arc<lapin::Connection>,
        queue_name: impl Into<String>,
        consumer_tag: impl Into<String>,
    ) -> Self {
        Self {
            connection,
            queue_name: queue_name.into(),
            consumer_tag: consumer_tag.into(),
        }
    }

    /// 从共享连接创建独立 channel；连接故障原样返回，不自动重连或修改消费状态。
    pub async fn channel(&self) -> AppResult<Channel> {
        Ok(self.connection.create_channel().await?)
    }

    /// 创建 channel 后进入持续消费循环；循环结束或 channel 创建失败直接返回调用方监督重连。
    /// 每条 delivery 的幂等、ACK/requeue 和业务事务由消费服务与 `consume_delivery` 协作完成。
    pub async fn consume_loop<R, H>(
        &self,
        service: EventInboxConsumerService<R, H>,
    ) -> AppResult<()>
    where
        R: EventInboxRepository,
        H: EventInboxHandler,
    {
        let channel = self.channel().await?;
        self.consume_channel_loop(channel, service).await
    }

    /// 在调用方给定 channel 上以已校验 queue/tag 注册 consumer，并串行处理 delivery，直到 broker 流关闭或流读取失败。
    /// 每条消息都在返回循环前完成 ACK 或 reject+requeue；单条业务/确认错误只记录并继续下一条，流级错误返回监督器触发重连。
    pub async fn consume_channel_loop<R, H>(
        &self,
        channel: Channel,
        service: EventInboxConsumerService<R, H>,
    ) -> AppResult<()>
    where
        R: EventInboxRepository,
        H: EventInboxHandler,
    {
        let mut consumer = channel
            .basic_consume(
                &self.queue_name,
                &self.consumer_tag,
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;

        while let Some(delivery) = consumer.next().await {
            let delivery = delivery?;
            if let Err(error) = consume_delivery(&service, &delivery, Utc::now()).await {
                tracing::error!(%error, "事件 inbox 投递处理失败");
            }
        }

        Ok(())
    }
}

/// 解析并消费单条 RabbitMQ delivery，再严格按结果执行 ACK 或 reject+requeue。
/// malformed/已持久化 retry/终态均 ACK；未落持久化状态的处理错误重入队，避免消息丢失。
/// 告警在 broker disposition 成功后输出；函数不拥有 handler 的业务事务。
pub async fn consume_delivery<R, H>(
    service: &EventInboxConsumerService<R, H>,
    delivery: &Delivery,
    now: DateTime<Utc>,
) -> AppResult<ConsumedInboxMessage>
where
    R: EventInboxRepository,
    H: EventInboxHandler,
{
    let result = match InboundEventMessage::from_delivery(delivery) {
        Ok(message) => service.consume_one(message, now).await,
        Err(error) => Err(error),
    };
    let processed = ProcessedInboxDelivery::from_result(result);
    match processed.disposition {
        InboxDeliveryDisposition::Ack => delivery.ack(BasicAckOptions::default()).await?,
        InboxDeliveryDisposition::RejectRequeue => {
            delivery
                .reject(BasicRejectOptions { requeue: true })
                .await?;
        }
    }
    if let Some(alert) = &processed.alert {
        // RabbitMQ ack/requeue 后记录告警分类，便于运维区分重试积压、死信和坏消息。
        alert.emit();
    }
    processed.result
}
