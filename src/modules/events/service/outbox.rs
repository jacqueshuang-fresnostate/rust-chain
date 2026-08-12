//! outbox 写入端口、批量发布编排与发布结果。

use super::{InboxRetryDecision, InboxRetryPolicy, NewOutboxEvent, OutboxInsertResult};
use crate::{
    error::{AppError, AppResult},
    modules::{events::EventOutboxRepository, market::adapters::MarketFeedEvent},
};
use axum::async_trait;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxMessage {
    pub id: u64,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub event_type: String,
    pub routing_key: String,
    pub idempotency_key: String,
    pub payload: Value,
    pub retry_count: u32,
}

#[async_trait]
pub trait OutboxPublisher: Clone + Send + Sync + 'static {
    /// 向外部 broker 发布一条持久化 outbox；幂等键必须作为外部 message_id 保留。
    /// 返回成功的确认强度由具体适配器定义且本 trait 不更新数据库；失败由上层记录 retry/dead-letter。
    async fn publish(&self, message: &OutboxMessage) -> AppResult<()>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PublishedOutboxBatch {
    pub attempted: u32,
    pub published: u32,
    pub retried: u32,
    pub dead_lettered: u32,
}

#[derive(Clone)]
pub struct EventOutboxWriter<R> {
    repository: R,
}

impl<R> EventOutboxWriter<R> {
    /// 组装仅负责 outbox 持久化的 writer；构造不连接仓储、不开始事务或发布消息。
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
}

impl<R> EventOutboxWriter<R>
where
    R: EventOutboxRepository,
{
    /// 将市场 feed 映射并写入 outbox；producer 幂等键保持不变，重复写返回 Duplicate。
    /// 事务与唯一约束由 repository 拥有；本入口不直接发布 RabbitMQ。
    pub async fn write_market_feed_event(
        &self,
        event: MarketFeedEvent,
        created_at: DateTime<Utc>,
    ) -> AppResult<OutboxInsertResult> {
        self.repository
            .insert_event(NewOutboxEvent::from_market_feed_event(event, created_at))
            .await
    }
}

#[derive(Clone)]
pub struct EventOutboxService<R, P> {
    repository: R,
    publisher: P,
    retry_policy: InboxRetryPolicy,
    batch_size: u32,
}

impl<R, P> EventOutboxService<R, P> {
    /// 绑定 outbox 仓储、单消息 publisher、重试策略和单轮扫描上限；零上限由仓储解释为空批。
    /// 构造不读取待发布行、不建立 broker channel，也不跨业务事务发布；所有状态推进延迟到 `publish_once`。
    pub fn new(
        repository: R,
        publisher: P,
        retry_policy: InboxRetryPolicy,
        batch_size: u32,
    ) -> Self {
        Self {
            repository,
            publisher,
            retry_policy,
            batch_size,
        }
    }
}

impl<R, P> EventOutboxService<R, P>
where
    R: EventOutboxRepository,
    P: OutboxPublisher,
{
    /// 读取至多 `batch_size` 条 pending/到期 retry，并按仓储顺序逐条发布；publisher 返回其定义的成功后才标记 `published`。
    /// 发布失败不终止批次，而按持久失败次数推进固定退避 retry 或 dead-letter；任一仓储读取/状态更新失败立即终止，已发布及已落状态的前项不回滚。
    /// 扫描与发布之间没有领取锁，多实例可能重复发送同一消息；publisher 必须携带 outbox 幂等键作为 message_id，下游 inbox 必须按该键去重。
    pub async fn publish_once(&self, now: DateTime<Utc>) -> AppResult<PublishedOutboxBatch> {
        let messages = self
            .repository
            .fetch_publishable_batch(self.batch_size, now)
            .await?;
        let mut summary = PublishedOutboxBatch {
            attempted: messages.len() as u32,
            published: 0,
            retried: 0,
            dead_lettered: 0,
        };

        for message in messages {
            match self.publisher.publish(&message).await {
                Ok(()) => {
                    self.repository
                        .mark_published(message.id, Utc::now())
                        .await?;
                    summary.published += 1;
                }
                Err(_) => match self
                    .retry_policy
                    .record_failure(message.retry_count, Utc::now())
                    .map_err(|error| {
                        AppError::Internal(format!("invalid event retry state: {error}"))
                    })? {
                    InboxRetryDecision::Retry {
                        attempt_count,
                        next_retry_at,
                    } => {
                        self.repository
                            .mark_retry(message.id, attempt_count, next_retry_at)
                            .await?;
                        summary.retried += 1;
                    }
                    InboxRetryDecision::DeadLetter { attempt_count } => {
                        self.repository
                            .mark_dead_letter(message.id, attempt_count, Utc::now())
                            .await?;
                        summary.dead_lettered += 1;
                    }
                },
            }
        }

        Ok(summary)
    }
}
