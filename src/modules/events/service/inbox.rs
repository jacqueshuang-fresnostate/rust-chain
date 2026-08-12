//! inbox 消息模型、处理端口与幂等消费编排。

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewInboxMessage {
    pub consumer_name: String,
    pub message_id: String,
    pub idempotency_key: String,
    pub payload_hash: String,
    pub payload: Value,
}

impl NewInboxMessage {
    /// 构造 inbox 待领取消息；全部字段必须来自同一 delivery，payload hash 用于内容冲突检测。
    /// 仅构造值，不插库、不领取处理租约，重复语义由仓储唯一约束决定。
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

    /// 生成 consumer 与 message_id 的稳定诊断键；不替代数据库唯一键或 payload hash 校验。
    /// 只分配字符串，不产生事务、ACK 或重试副作用。
    pub fn consumer_message_key(&self) -> String {
        format!("{}:{}", self.consumer_name, self.message_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundEventMessage {
    pub message_id: String,
    pub idempotency_key: String,
    pub payload: Value,
}

impl InboundEventMessage {
    /// 构造已抽取的入站消息；message_id 与 idempotency_key 去空白后不得为空。
    /// payload 原样保留，不执行 dispatch 或领取 inbox；非法标识返回 validation error。
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
    /// 仅解析不 ACK/reject；格式错误返回 validation error，供 disposition 层决定确认语义。
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

    /// 对 payload 的稳定 JSON 字节计算哈希，用于检测同 message_id 的内容冲突。
    /// 序列化失败返回内部错误；不写 inbox、不改变消息，调用方仍需仓储原子校验。
    pub fn payload_hash(&self) -> AppResult<String> {
        let bytes = serde_json::to_vec(&self.payload).map_err(|error| {
            AppError::Internal(format!("failed to serialize inbox payload: {error}"))
        })?;
        let mut hasher = DefaultHasher::new();
        hasher.write(&bytes);
        Ok(format!("{:016x}", hasher.finish()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboxClaim {
    Claimed {
        attempt_count: u32,
        processing_token: String,
    },
    Duplicate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingInboxRetry {
    pub consumer_name: String,
    pub message_id: String,
    pub idempotency_key: String,
    pub payload: Value,
}

fn inbox_message_is_already_processing(error: &AppError) -> bool {
    matches!(error, AppError::Internal(message) if message == "event inbox message is already processing")
}

#[async_trait]
pub trait EventInboxHandler: Clone + Send + Sync + 'static {
    /// 执行一条已校验入站事件的业务副作用；实现必须让相同幂等消息可安全重放。
    /// 返回错误由消费服务记录 retry/dead-letter；实现自行拥有事务，trait 不负责 commit 或 ACK。
    async fn handle(&self, message: &InboundEventMessage) -> AppResult<()>;
}

#[derive(Clone, Copy)]
pub struct NoopEventInboxHandler;

#[async_trait]
impl EventInboxHandler for NoopEventInboxHandler {
    /// 测试/显式空处理实现；接受任意已构造消息并立即成功，无事务、I/O 或业务副作用。
    async fn handle(&self, _message: &InboundEventMessage) -> AppResult<()> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct EventInboxConsumerService<R, H> {
    consumer_name: String,
    repository: R,
    handler: H,
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
    /// 重放仍须竞争处理租约；多实例已先领取时按 duplicate 跳过，consumer 归属不一致或其他错误终止本批，已完成行不会回滚。
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
