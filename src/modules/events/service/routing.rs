//! 领域事件、路由和幂等键模型。
//!
//! 本文件定义事件总线的去重键构造规则，这是整条链路幂等性的源头。
//! 发布侧幂等键固定为 `聚合类型:聚合标识:事件类型` 三段冒号拼接，
//! 因此同一聚合的同类事件被视为同一条消息，重复写入由 outbox 的唯一约束吸收。
//! 该规则要求三个字段自身不含冒号，否则不同输入可能拼出相同的键。
//! 消费侧的去重上下文另加消费者名，使同一消息可被多个消费者各自独立消费而互不干扰。
//! 事件的 UUID 仅作为 envelope 标识，不参与去重，重放必须复用原幂等键而不是依赖新生成的 UUID。
//! 本文件全部为纯构造，不访问数据库、不投递消息，也不校验业务内容。

use crate::{modules::market::adapters::MarketFeedEvent, time::unix_millis};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// 领域事件 envelope，把路由信息、去重键与业务载荷包在一起用于跨进程传输。
/// 同时实现序列化与反序列化，因此该结构就是消息在 broker 上的线格式，改字段等同于改协议。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEvent {
    /// 事件唯一标识，采用 UUIDv7 因而按时间有序；仅作诊断用，不参与去重。
    pub id: Uuid,
    /// 目标交换机名称。
    pub exchange: String,
    /// 路由键，决定消息被投递到哪些队列。
    pub routing_key: String,
    /// 去重键，消费方按它判断是否已处理过同一业务事件。
    pub idempotency_key: String,
    /// 业务载荷，事件总线原样透传不做解释。
    pub payload: Value,
    /// 事件产生时刻，以毫秒时间戳传输以避免跨语言的时区解析差异。
    #[serde(with = "unix_millis")]
    pub created_at: DateTime<Utc>,
}

impl DomainEvent {
    /// 构造带 UUIDv7 的领域事件 envelope；route、幂等键与 payload 按输入原样写入。
    /// 选用 UUIDv7 使标识按生成时间有序，便于按主键排序时大致还原事件发生顺序。
    /// 该标识每次调用都不同，因此绝不能用来判重：同一业务事件的重放必须复用相同幂等键，
    /// 去重完全依赖幂等键而与此标识无关。
    /// 路由与幂等信息在此被消费并摊平进结构，载荷原样保留。
    /// 本函数不校验业务内容，也不持久化或发布任何东西。
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

/// 一次投递的目标路由，由交换机与路由键两段构成。
/// 取值完全由调用方决定，本类型不补默认交换机也不校验通配符范围。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRoute {
    /// 交换机名称。
    pub exchange: String,
    /// 路由键，需与消费端队列的绑定规则匹配才能被收到。
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

/// 发布侧去重键的三段来源，最终拼成 `聚合类型:聚合标识:事件类型`。
/// 三个字段共同决定「什么算同一条事件」，因此同一业务动作在任何重放路径下都必须给出完全相同的三元组。
/// 字段自身不得包含冒号，否则不同的三元组可能拼出相同的键而被误判为重复。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventIdempotency {
    /// 聚合类型，标明事件属于哪类业务对象。
    pub aggregate_type: String,
    /// 聚合实例标识，通常是业务主键的字符串形式。
    pub aggregate_id: String,
    /// 事件类型名，同一聚合上的不同动作以此区分。
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

    /// 将聚合类型、聚合标识、事件类型按顺序用冒号拼成幂等键，这是发布侧去重键的唯一生成规则。
    /// 三段中任何一段包含冒号都会造成歧义，可能让不同三元组拼出相同结果而被误判为同一事件，
    /// 字段合法性由构造调用方保证，此处不做校验。
    /// 消费 self 以避免多余克隆，无任何 I/O。
    pub fn into_key(self) -> String {
        format!(
            "{}:{}:{}",
            self.aggregate_type, self.aggregate_id, self.event_type
        )
    }
}

/// 消费侧的去重上下文，比发布侧多出消费者维度。
/// 加上消费者名之后，同一条消息可被多个消费者各自独立处理与去重，互不影响。
/// 消息标识与业务幂等键同时保留：前者标识这一次投递，后者标识背后的业务事件，
/// 数据库按两者的并集判重，因此同一事件即便以不同消息标识重投也能被识别出来。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxIdempotency {
    /// 消费者名称，界定去重与补偿重放的作用范围。
    pub consumer_name: String,
    /// 本次投递的消息标识，来自传输层 envelope。
    pub message_id: String,
    /// 业务幂等键，来自发布侧的三段拼接结果。
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

    /// 生成消费者名与消息标识的冒号拼接串，仅作日志与排障时的可读标识。
    /// 它不参与任何判重逻辑，真正的去重由数据库上消费者名与消息标识的唯一约束负责，
    /// 因此该值重复并不意味着消息会被拒绝，反之亦然。
    pub fn consumer_message_key(&self) -> String {
        format!("{}:{}", self.consumer_name, self.message_id)
    }
}

/// 待写入 outbox 的事件模型，是业务侧产生事件时构造的入参。
/// 幂等键通常由构造函数按三段规则自动拼出，但字段本身可写，
/// 市场行情一类由上游 producer 自带幂等键的场景会在构造后覆盖它。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewOutboxEvent {
    /// 聚合类型。
    pub aggregate_type: String,
    /// 聚合实例标识。
    pub aggregate_id: String,
    /// 事件类型名。
    pub event_type: String,
    /// 路由键，由生产方显式给出，不由聚合信息推导。
    pub routing_key: String,
    /// 去重键，唯一约束建在该列上，冲突即判定为重复事件。
    pub idempotency_key: String,
    /// 业务载荷，以 JSON 原样存储。
    pub payload: Value,
    /// 事件产生时刻，由调用方传入而非取当前时间，便于补写历史事件与测试。
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
    /// 幂等键在此自动派生而不由调用方指定，正是要强制同一聚合的同类事件落到同一个键上；
    /// 这也意味着同一聚合在同一逻辑上只能有一条该类型事件，需要多条时必须让聚合标识本身可区分。
    /// 路由键与幂等键相互独立：前者决定投递去向，后者决定去重，两者不要求形式一致。
    /// 载荷与创建时刻原样保留，创建时刻由调用方给出便于补写历史事件与测试。
    /// 本函数不持久化也不广播，是否真正插入由 outbox 的唯一约束在写库时裁决。
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

/// outbox 写入结果，用于区分首次落库与幂等命中。
/// 两个分支都带主键，因此调用方无论哪种情况都能拿到事件编号；
/// 需要「仅首次执行」的后续动作（例如记一条业务日志）必须显式判分支，不能只看是否成功。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutboxInsertResult {
    /// 首次写入成功，携带新生成的事件主键。
    Inserted { id: u64 },
    /// 幂等键已存在，未写入新行，携带既有事件的主键。
    Duplicate { id: u64 },
}
