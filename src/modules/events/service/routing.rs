//! 领域事件、路由和幂等键模型。

use crate::{modules::market::adapters::MarketFeedEvent, time::unix_millis};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEvent {
    pub id: Uuid,
    pub exchange: String,
    pub routing_key: String,
    pub idempotency_key: String,
    pub payload: Value,
    #[serde(with = "unix_millis")]
    pub created_at: DateTime<Utc>,
}

impl DomainEvent {
    /// 构造带 UUIDv7 的领域事件 envelope；route、幂等键与 payload 按输入原样写入。
    /// 本函数不校验业务内容、不持久化或发布；重放应复用幂等键而非依赖新 UUID。
    pub fn new(
        route: EventRoute,
        idempotency: EventIdempotency,
        payload: Value,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            exchange: route.exchange,
            routing_key: route.routing_key,
            idempotency_key: idempotency.into_key(),
            payload,
            created_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRoute {
    pub exchange: String,
    pub routing_key: String,
}

impl EventRoute {
    /// 保存一组显式 exchange/routing key，不自动补 `exchange.events` 或业务前缀，也不校验 topic 通配范围。
    /// 本值只描述路由；RabbitMQ exchange 声明、投递确认以及失败重试均由发布适配器和 outbox 编排负责。
    pub fn new(exchange: impl Into<String>, routing_key: impl Into<String>) -> Self {
        Self {
            exchange: exchange.into(),
            routing_key: routing_key.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventIdempotency {
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub event_type: String,
}

impl EventIdempotency {
    /// 构造聚合维度的事件幂等信息；调用方需保证三个字段稳定且不存在分隔歧义。
    /// 仅保存值，不访问存储；重复业务事件必须使用相同三元组。
    pub fn new(
        aggregate_type: impl Into<String>,
        aggregate_id: impl Into<String>,
        event_type: impl Into<String>,
    ) -> Self {
        Self {
            aggregate_type: aggregate_type.into(),
            aggregate_id: aggregate_id.into(),
            event_type: event_type.into(),
        }
    }

    /// 将聚合类型、ID、事件类型编码为稳定冒号分隔幂等键。
    /// 消费 self 且无 I/O；字段合法性由构造调用方保证。
    pub fn into_key(self) -> String {
        format!(
            "{}:{}:{}",
            self.aggregate_type, self.aggregate_id, self.event_type
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxIdempotency {
    pub consumer_name: String,
    pub message_id: String,
    pub idempotency_key: String,
}

impl InboxIdempotency {
    /// 构造消费者消息幂等上下文；三项输入应来自可信传输 envelope。
    /// 本函数不领取 inbox 行，不产生事务或重试副作用。
    pub fn new(
        consumer_name: impl Into<String>,
        message_id: impl Into<String>,
        idempotency_key: impl Into<String>,
    ) -> Self {
        Self {
            consumer_name: consumer_name.into(),
            message_id: message_id.into(),
            idempotency_key: idempotency_key.into(),
        }
    }

    /// 生成 consumer 与 message_id 的稳定诊断键；不替代数据库唯一约束。
    /// 只分配字符串，无事务、状态推进或外部副作用。
    pub fn consumer_message_key(&self) -> String {
        format!("{}:{}", self.consumer_name, self.message_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewOutboxEvent {
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub event_type: String,
    pub routing_key: String,
    pub idempotency_key: String,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

impl NewOutboxEvent {
    /// 将已验证市场 feed 映射为 outbox 事件，显式保留 producer 幂等键。
    /// 只构造待写模型；是否重复由仓储唯一约束决定，不访问数据库或 broker。
    pub fn from_market_feed_event(event: MarketFeedEvent, created_at: DateTime<Utc>) -> Self {
        let mut outbox_event = Self::new(
            event.aggregate_type(),
            event.aggregate_id(),
            event.event_type(),
            event.routing_key(),
            event.payload().clone(),
            created_at,
        );
        outbox_event.idempotency_key = event.idempotency_key().to_owned();
        outbox_event
    }

    /// 构造普通 outbox 行模型，路由键由 producer 明确给出，幂等键固定为 `aggregate_type:aggregate_id:event_type`。
    /// payload 与路由原样保留；相同聚合同类事件会被视为同一幂等消息，是否插入由 outbox 唯一约束决定，本函数不持久化或广播。
    pub fn new(
        aggregate_type: impl Into<String>,
        aggregate_id: impl Into<String>,
        event_type: impl Into<String>,
        routing_key: impl Into<String>,
        payload: Value,
        created_at: DateTime<Utc>,
    ) -> Self {
        let aggregate_type = aggregate_type.into();
        let aggregate_id = aggregate_id.into();
        let event_type = event_type.into();
        let idempotency_key = EventIdempotency::new(
            aggregate_type.clone(),
            aggregate_id.clone(),
            event_type.clone(),
        )
        .into_key();

        Self {
            aggregate_type,
            aggregate_id,
            event_type,
            routing_key: routing_key.into(),
            idempotency_key,
            payload,
            created_at,
        }
    }
}

/// 构造 `user.created` outbox 合同；user_id 同时进入 aggregate、routing key 与 payload。
/// 该纯函数不预创建钱包、不持久化；相同用户重放生成同一幂等键，由写入事务去重。
pub(crate) fn user_created_outbox_event(user_id: u64, created_at: DateTime<Utc>) -> NewOutboxEvent {
    let aggregate_id = user_id.to_string();
    NewOutboxEvent {
        aggregate_type: "user".to_owned(),
        aggregate_id: aggregate_id.clone(),
        event_type: "created".to_owned(),
        routing_key: format!("user.{user_id}.created"),
        idempotency_key: EventIdempotency::new("user", &aggregate_id, "created").into_key(),
        payload: serde_json::json!({ "user_id": user_id }),
        created_at,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutboxInsertResult {
    Inserted { id: u64 },
    Duplicate { id: u64 },
}
