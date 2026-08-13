//! RabbitMQ outbox 发布与 inbox delivery 适配。
//!
//! 这是事件总线与 broker 之间唯一的适配层，两个方向各一条链路。
//! 发布方向把 outbox 行编码成 JSON envelope，用 outbox 幂等键充当 AMQP 消息标识，
//! 并以持久化投递模式发到 durable topic 交换机；由于未开启发布确认，
//! 发送返回成功只代表消息已交给客户端库，不等于 broker 已持久化，这是 at-least-once 中丢失窗口的来源。
//! 消费方向从队列取 delivery，交消费服务处理后按结果确认或拒收重投，
//! 确认动作严格发生在本地状态落库之后，从而保证消息不会在处理完成前被移出队列。

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

/// 一条即将发往 broker 的消息的完整描述，把编码与发送两步分开以便单独测试编码结果。
/// 消息标识取自 outbox 幂等键，这是消费侧能够去重的前提，不可改用随机值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RabbitMqPublishEnvelope {
    /// 目标交换机名称。
    pub exchange: String,
    /// 路由键，直接沿用 outbox 行上的取值，适配层不改写业务路由。
    pub routing_key: String,
    /// AMQP 消息标识，固定为 outbox 幂等键，消费侧据此判重。
    pub message_id: String,
    /// 内容类型，固定为 JSON。
    pub content_type: String,
    /// 已序列化的消息体字节。
    pub payload: Vec<u8>,
}

impl RabbitMqPublishEnvelope {
    /// 将 outbox 行编码为跨上下文 JSON envelope：聚合、事件类型、routing key、幂等键和业务 payload 全部保留。
    /// 路由键与幂等键在消息体内再存一份而非只放协议头，消费侧因此可以校验两者一致，识别被篡改或错配的消息。
    /// AMQP message_id 固定取 outbox 幂等键，这是消费侧去重的唯一依据，改用其他值会让整条链路失去幂等能力。
    /// 业务载荷嵌在固定键下原样保留，事件总线不解释也不改写其内容。
    /// 序列化失败归为内部错误；本函数不声明交换机、不建立连接、不发送任何网络请求。
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

    /// 组装 AMQP 消息属性：写入消息标识与内容类型，并把投递模式设为 2 即持久化消息。
    /// 持久化投递配合 durable 交换机与队列，才能让 broker 重启后消息不丢；三者缺一即失去持久性。
    /// 消息标识必须原样带上，它是下游去重的唯一依据。
    fn properties(&self) -> BasicProperties {
        BasicProperties::default()
            .with_message_id(self.message_id.clone().into())
            .with_content_type(self.content_type.clone().into())
            .with_delivery_mode(2)
    }
}

/// 基于 RabbitMQ 的消息投递适配器，是投递端口的生产实现。
/// 共享一条连接但每条消息单开 channel，换取实现简单与故障隔离，代价是高频发布时开销偏高。
#[derive(Clone)]
pub struct RabbitMqOutboxPublisher {
    /// 共享的 broker 连接，克隆时只增加引用计数。
    connection: Arc<lapin::Connection>,
    /// 唯一目标交换机名称，所有事件都发往这里，靠路由键区分去向。
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

/// 基于 RabbitMQ 的消息拉取适配器，负责注册消费者并把 delivery 逐条交给消费服务。
/// 只做传输层适配，不持有 inbox 状态也不实现任何业务逻辑。
#[derive(Clone)]
pub struct RabbitMqInboxConsumer {
    /// 共享的 broker 连接。
    connection: Arc<lapin::Connection>,
    /// 目标队列名，须由调用方预先声明并绑定到交换机，本类型不负责声明。
    queue_name: String,
    /// 消费者标签，用于在 broker 侧标识本消费者，便于运维观察与取消订阅。
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

    /// 从共享连接开出一条独立 channel 供本次消费使用。
    /// 独立 channel 使某个消费者出错关闭时不影响同连接上的其他消费者与发布者。
    /// 连接故障原样上抛，本方法不做自动重连，重连由上层监督逻辑负责；也不修改任何消费状态。
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
    /// 串行而非并发处理是刻意取舍：同一队列的消息逐条走完「领租约、执行、落状态、确认」全过程，
    /// 避免并发时多条消息竞争同一租约造成大量无效重复；代价是单条慢消息会阻塞后续消息。
    /// 单条业务或确认错误只记日志并继续下一条，不中断整个循环，因为那通常只是该消息自身的问题；
    /// 流级错误则向上返回，交由外层监督逻辑按退避重建连接。
    /// 队列必须已由调用方声明并绑定，本方法只注册消费者不做任何拓扑声明。
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
/// 顺序不可调换：先完成消费与状态落库，再对 broker 表态，最后才发告警。
/// 若提前确认，进程在处理完成前崩溃就会让消息既不在队列也未落库，从而彻底丢失。
/// malformed、已落库的待重试与各类终态一律确认，因为本地已记下后续处置，重投只会制造重复；
/// 只有未能落库的处理错误才拒收重入队，让 broker 稍后再投一次。
/// 解析失败的消息不会进入消费服务，直接以错误参与确认判定。
/// 告警放在表态之后输出，保证日志里出现告警时消息的归属已经确定。
/// 本函数不拥有业务事务，也不感知 handler 内部做了什么。
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
