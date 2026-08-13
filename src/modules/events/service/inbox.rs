//! inbox 消息模型、处理端口与幂等消费编排。
//!
//! 消费一条消息的固定顺序是：竞争租约、执行业务、按结果落终态。
//! 只有真正拿到租约才会执行业务，命中去重时直接返回而不重放任何副作用，这是消费侧幂等的核心。
//! 业务失败不会让消息回到 broker，而是先在本地落成待重试或死信，再由补偿扫描从 inbox 存档的载荷重放；
//! 这样即便 broker 那一侧的投递已被确认，消息也不会丢。
//! 批量消费按输入顺序逐条独立处理，各条自成一体，前面成功的不会因后面失败而回滚，因此没有批级事务。

use super::{ConsumedInboxBatch, ConsumedInboxMessage, InboxRetryDecision, InboxRetryPolicy};
use crate::{
    error::{AppError, AppResult},
    modules::events::EventInboxRepository,
};
use axum::async_trait;
use chrono::{DateTime, Utc};
use lapin::message::Delivery;
use serde_json::Value;
use std::{collections::hash_map::DefaultHasher, hash::Hasher};

/// 提交给仓储去竞争租约的 inbox 登记参数，比传输层消息多出消费者名与载荷摘要。
/// 载荷会被完整存档，使消息在 broker 侧被确认之后仍能从数据库重放。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewInboxMessage {
    /// 消费者名称，界定去重与补偿重放的作用范围。
    pub consumer_name: String,
    /// 本次投递的消息标识，与消费者名共同构成主要去重键。
    pub message_id: String,
    /// 业务幂等键，作为第二个去重维度，与消息标识取并集判重。
    pub idempotency_key: String,
    /// 载荷内容摘要，用于识别同一消息标识下载荷不一致的异常情况。
    pub payload_hash: String,
    /// 完整载荷，存档后成为补偿重放的数据来源。
    pub payload: Value,
}

impl NewInboxMessage {
    /// 构造 inbox 待领取消息；全部字段必须来自同一 delivery，payload hash 用于内容冲突检测。
    /// 消费者名由消费服务提供而非消息自带，其余四项均来自同一条投递，混用不同投递的字段会破坏去重判定。
    /// 载荷同时以原文和摘要两种形态携带：原文用于存档以支撑补偿重放，摘要用于识别同一消息标识下内容不一致。
    /// 仅构造值，不插库、不领取处理租约，重复语义最终由仓储的唯一约束裁决。
    pub fn new(
        consumer_name: impl Into<String>,
        message_id: impl Into<String>,
        idempotency_key: impl Into<String>,
        payload_hash: impl Into<String>,
        payload: Value,
    ) -> Self {
        Self {
            consumer_name: consumer_name.into(),
            message_id: message_id.into(),
            idempotency_key: idempotency_key.into(),
            payload_hash: payload_hash.into(),
            payload,
        }
    }

    /// 生成 consumer 与 message_id 的稳定诊断键，主要用于日志中定位同一条消息的处理轨迹。
    /// 它只是可读标识，不替代数据库唯一约束，也不参与载荷摘要比对，不能据此判定消息是否重复。
    pub fn consumer_message_key(&self) -> String {
        format!("{}:{}", self.consumer_name, self.message_id)
    }
}

/// 已完成基本校验的入站事件，是消费流程的统一输入。
/// 无论来自 broker 实时投递还是来自 inbox 存档的补偿重放，都先收敛成本类型再进入同一条消费路径。
/// 不含消费者名：消费者身份由消费服务持有，消息本身不携带。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundEventMessage {
    /// 消息标识，去空白后保证非空。
    pub message_id: String,
    /// 业务幂等键，去空白后保证非空。
    pub idempotency_key: String,
    /// 业务载荷，原样保留不做解释。
    pub payload: Value,
}

impl InboundEventMessage {
    /// 构造已抽取的入站消息；message_id 与 idempotency_key 去空白后不得为空。
    /// 两项标识是消费侧去重的全部依据，任一为空都会让去重失效，因此在此直接拒绝而不给默认值。
    /// 注意校验只用于判空，字段本身保存的仍是未裁剪的原值，比对时按原文进行。
    /// 载荷不做任何校验或解释，结构合法性留给业务分派阶段判断。
    /// 返回的校验错误会被确认层识别为不可重投的报文问题，从而确认跳过而非无限重投。
    pub fn new(
        message_id: impl Into<String>,
        idempotency_key: impl Into<String>,
        payload: Value,
    ) -> AppResult<Self> {
        let message_id = message_id.into();
        if message_id.trim().is_empty() {
            return Err(AppError::Validation(
                "event message_id is required".to_owned(),
            ));
        }
        let idempotency_key = idempotency_key.into();
        if idempotency_key.trim().is_empty() {
            return Err(AppError::Validation(
                "event idempotency_key is required".to_owned(),
            ));
        }

        Ok(Self {
            message_id,
            idempotency_key,
            payload,
        })
    }

    /// 从 RabbitMQ delivery 提取 message_id、JSON payload 与兼容的幂等键位置。
    /// 消息标识取自 AMQP 属性而非载荷，因此发布方必须把 outbox 幂等键设进该属性，否则消息在此即被判为非法。
    /// 幂等键的读取兼容两种载荷布局：先看顶层字段，再回落到嵌套的 `event` 对象内，
    /// 这条回落路径服务于外层再包一层 envelope 的旧版生产者。
    /// 仅解析不做确认或拒收；缺字段与 JSON 非法都返回校验错误，
    /// 这三类错误会被确认层识别为「重投也不会成功」，从而确认跳过而非无限重投。
    pub fn from_delivery(delivery: &Delivery) -> AppResult<Self> {
        let message_id = delivery
            .properties
            .message_id()
            .as_ref()
            .map(ToString::to_string)
            .ok_or_else(|| AppError::Validation("event message_id is required".to_owned()))?;
        let payload: Value = serde_json::from_slice(&delivery.data).map_err(|error| {
            AppError::Validation(format!("invalid event payload json: {error}"))
        })?;
        let idempotency_key = payload
            .get("idempotency_key")
            .and_then(Value::as_str)
            .or_else(|| {
                payload
                    .get("event")
                    .and_then(|event| event.get("idempotency_key"))
                    .and_then(Value::as_str)
            })
            .map(str::to_owned)
            .ok_or_else(|| AppError::Validation("event idempotency_key is required".to_owned()))?;
        Self::new(message_id, idempotency_key, payload)
    }

    /// 对载荷的 JSON 字节计算摘要并输出定长十六进制文本，用于识别同一消息标识下载荷不一致的异常。
    /// 采用标准库默认哈希器，只用于检测内容差异，不具备抗碰撞的密码学强度，不可用作安全校验。
    /// 摘要值也不跨进程或跨版本稳定，因此只应在同一次运行内做比较，不适合长期持久化后再比对。
    /// 序列化失败返回内部错误；本方法不写 inbox 也不改变消息，去重仍以仓储的原子约束为准。
    pub fn payload_hash(&self) -> AppResult<String> {
        let bytes = serde_json::to_vec(&self.payload).map_err(|error| {
            AppError::Internal(format!("failed to serialize inbox payload: {error}"))
        })?;
        let mut hasher = DefaultHasher::new();
        hasher.write(&bytes);
        Ok(format!("{:016x}", hasher.finish()))
    }
}

/// 租约竞争结果，决定本次是否真的要执行业务处理。
/// 只有取得租约才允许调用业务 handler，重复分支必须直接返回，不得重放任何副作用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboxClaim {
    /// 成功取得处理权。
    Claimed {
        /// 该消息此前已累计的失败次数，作为退避策略的输入。
        attempt_count: u32,
        /// 处理令牌，后续推进消费终态时必须原样带上作为乐观条件。
        processing_token: String,
    },
    /// 消息已被消费、已进终态或尚未到重试时间，本次不处理。
    Duplicate,
}

/// 从 inbox 存档中取出的待重放消息，用于 broker 侧已确认后的本地补偿。
/// 载荷来自数据库而非 broker，因此重放不依赖原始 delivery 是否还在队列里。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingInboxRetry {
    /// 记录所属消费者，重放前会校验它与当前消费者一致。
    pub consumer_name: String,
    /// 消息标识。
    pub message_id: String,
    /// 业务幂等键。
    pub idempotency_key: String,
    /// 存档的完整载荷，是重放能够成立的前提。
    pub payload: Value,
}

/// 识别「消息正被其他实例处理」这一特定并发错误，用于把它与真实故障区分开。
/// 通过错误文案精确匹配，因此仓储侧改动该文案会静默改变补偿扫描的行为，需同步更新此处。
/// 命中时补偿扫描应把该条按重复跳过并继续处理后续消息，而不是中断整批。
fn inbox_message_is_already_processing(error: &AppError) -> bool {
    matches!(error, AppError::Internal(message) if message == "event inbox message is already processing")
}

/// 业务处理端口，把「怎么处理这条事件」从消费编排中分离出去。
/// 实现必须自身幂等：同一条消息可能因重试或多实例竞争而被处理多次，重复执行不得产生额外副作用。
/// 实现自行拥有并提交业务事务；返回错误即交由消费服务按退避策略落重试或死信。
#[async_trait]
pub trait EventInboxHandler: Clone + Send + Sync + 'static {
    /// 执行一条已校验入站事件的业务副作用；实现必须让相同幂等消息可安全重放。
    /// 调用发生在成功取得处理租约之后，但同一消息仍可能因重试或租约超时被重新处理，故实现须自身幂等。
    /// 返回错误由消费服务按退避策略记录为待重试或死信；实现自行拥有并提交事务，本方法不负责提交或消息确认。
    async fn handle(&self, message: &InboundEventMessage) -> AppResult<()>;
}

/// 空实现的业务处理器，接受任何消息并立即成功。
/// 用于单元测试隔离消费编排逻辑，或在只需验证去重与状态流转的场景下占位。
/// 生产配置不应使用它，否则消息会被标记为已消费但实际没有任何业务效果。
#[derive(Clone, Copy)]
pub struct NoopEventInboxHandler;

#[async_trait]
impl EventInboxHandler for NoopEventInboxHandler {
    /// 空处理实现：接受任意已构造消息并立即返回成功，不做事务、不发起任何输入输出、不产生业务副作用。
    /// 由于恒定成功，消息会被直接推进为已消费终态，因此仅适用于测试或明确不需要处理的场景。
    async fn handle(&self, _message: &InboundEventMessage) -> AppResult<()> {
        Ok(())
    }
}

/// 消费编排服务，把租约竞争、业务执行与终态落库串成一条固定流程。
/// 仓储与处理器均为泛型，因此可在测试中替换成内存实现而无需 MySQL 与 RabbitMQ。
#[derive(Clone)]
pub struct EventInboxConsumerService<R, H> {
    /// 本服务代表的消费者名称，同时决定去重范围与补偿扫描范围。
    consumer_name: String,
    /// inbox 持久化仓储，负责租约与状态。
    repository: R,
    /// 业务处理器，只在成功取得租约后才被调用。
    handler: H,
    /// 失败退避与死信判定策略，与 outbox 侧共用同一实现。
    retry_policy: InboxRetryPolicy,
}

impl<R, H> EventInboxConsumerService<R, H> {
    /// 组装 consumer、仓储、业务 handler 与重试策略；consumer_name 应稳定对应唯一消费边界。
    /// 构造不连接数据库或 broker，也不消费消息；策略有效性由调用方在注入前保证。
    pub fn new(
        consumer_name: impl Into<String>,
        repository: R,
        handler: H,
        retry_policy: InboxRetryPolicy,
    ) -> Self {
        Self {
            consumer_name: consumer_name.into(),
            repository,
            handler,
            retry_policy,
        }
    }
}

impl<R, H> EventInboxConsumerService<R, H>
where
    R: EventInboxRepository,
    H: EventInboxHandler,
{
    /// 按输入顺序消费完整内存批次，逐条独立领取租约、执行 handler 并落 consumed/retry/dead-letter；本入口不另加扫描上限。
    /// duplicate 不重放业务副作用，handler 失败被持久化后继续下一条；领取或状态仓储错误立即终止，已完成前项不回滚，消息间没有总事务。
    pub async fn consume_batch(
        &self,
        messages: Vec<InboundEventMessage>,
        now: DateTime<Utc>,
    ) -> AppResult<ConsumedInboxBatch> {
        let mut batch = ConsumedInboxBatch {
            consumed: 0,
            duplicates: 0,
            retried: 0,
            dead_lettered: 0,
        };

        for message in messages {
            batch.record(self.consume_one(message, now).await?);
        }

        Ok(batch)
    }

    /// 从持久化 payload 读取指定 consumer 至多 `limit` 条到期 retry/过期 processing，并按顺序重新进入普通消费流程。
    /// 这是 broker 已确认之后唯一的重放来源：消息从数据库存档的载荷重建，不依赖原始投递是否还在队列里。
    /// 重建前逐条核对记录归属的消费者与本服务一致，不一致直接判为内部错误并终止整批，绝不跨消费者重放。
    /// 重放仍须重新竞争处理租约；若其他实例已先领取，本条按重复计入并继续处理后续行而不是中断，
    /// 因为多实例同时扫到同一条到期记录属于预期内的竞争而非故障。
    /// 其余错误立即终止本批，已完成的前几条不会回滚，批内没有整体事务。
    pub async fn replay_due_retries(
        &self,
        now: DateTime<Utc>,
        limit: u32,
    ) -> AppResult<ConsumedInboxBatch> {
        let retries = self
            .repository
            .fetch_due_retries(&self.consumer_name, limit, now)
            .await?;
        let mut messages = Vec::with_capacity(retries.len());
        for retry in retries {
            if retry.consumer_name != self.consumer_name {
                return Err(AppError::Internal(
                    "event inbox retry consumer mismatch".to_owned(),
                ));
            }
            // 从 inbox 持久化 payload 重建消息，避免 RabbitMQ 当前 delivery ACK 后重试行失去重放来源。
            messages.push(InboundEventMessage::new(
                retry.message_id,
                retry.idempotency_key,
                retry.payload,
            )?);
        }

        let mut batch = ConsumedInboxBatch {
            consumed: 0,
            duplicates: 0,
            retried: 0,
            dead_lettered: 0,
        };
        for message in messages {
            match self.consume_one(message, now).await {
                Ok(result) => batch.record(result),
                Err(error) if inbox_message_is_already_processing(&error) => {
                    // 多实例 scanner 可能同时读到同一条到期行；若另一实例已先领取，就把本条当作重复跳过，继续处理后续行。
                    batch.record(ConsumedInboxMessage::Duplicate);
                }
                Err(error) => return Err(error),
            }
        }

        Ok(batch)
    }

    /// 以 consumer/message_id/idempotency_key 与 payload hash 原子领取单条 inbox，取得处理令牌后才执行 handler。
    /// 终态或等价重复不调用 handler；业务失败按持久尝试次数推进 retry/dead-letter，成功和失败状态都以令牌条件更新，陈旧 worker 不得覆盖新租约。
    /// handler 自行拥有并提交业务事务，本服务只编排 inbox 状态；RabbitMQ ACK/requeue 由 delivery 适配层在本结果持久化后决定。
    /// 业务事务与 inbox 状态处于两个独立事务，因此两者之间存在窗口：业务已提交而状态更新失败时，
    /// 该消息会按重试再跑一遍，这正是要求 handler 自身幂等的原因。
    /// 载荷摘要在领取前计算，序列化失败会让消息在触碰数据库之前就失败。
    pub async fn consume_one(
        &self,
        message: InboundEventMessage,
        now: DateTime<Utc>,
    ) -> AppResult<ConsumedInboxMessage> {
        let claim = self
            .repository
            .claim_message(NewInboxMessage::new(
                self.consumer_name.clone(),
                message.message_id.clone(),
                message.idempotency_key.clone(),
                message.payload_hash()?,
                message.payload.clone(),
            ))
            .await?;

        let (attempt_count, processing_token) = match claim {
            InboxClaim::Claimed {
                attempt_count,
                processing_token,
            } => (attempt_count, processing_token),
            InboxClaim::Duplicate => return Ok(ConsumedInboxMessage::Duplicate),
        };

        match self.handler.handle(&message).await {
            Ok(()) => {
                self.repository
                    .mark_consumed(&self.consumer_name, &message.message_id, &processing_token)
                    .await?;
                Ok(ConsumedInboxMessage::Consumed)
            }
            Err(error) => {
                let error_message = error.to_string();
                let decision = self
                    .retry_policy
                    .record_failure(attempt_count, now)
                    .map_err(|error| {
                        AppError::Internal(format!("invalid event inbox retry state: {error}"))
                    })?;
                self.repository
                    .mark_failure(
                        &self.consumer_name,
                        &message.message_id,
                        &processing_token,
                        decision.clone(),
                        &error_message,
                        now,
                    )
                    .await?;
                Ok(match decision {
                    InboxRetryDecision::Retry {
                        attempt_count,
                        next_retry_at,
                    } => ConsumedInboxMessage::Retried {
                        attempt_count,
                        next_retry_at,
                    },
                    InboxRetryDecision::DeadLetter { attempt_count } => {
                        ConsumedInboxMessage::DeadLettered { attempt_count }
                    }
                })
            }
        }
    }
}
