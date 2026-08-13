//! outbox 写入端口、批量发布编排与发布结果。
//!
//! 发布一轮的流程固定为：按上限取一批到期消息、逐条交给 publisher、按结果推进终态或退避。
//! 这里没有领取锁，多个实例可能取到同一批并各发一次，因此整体是 at-least-once；
//! 去重责任下放给消息本身的幂等键，它必须被 publisher 用作 broker 侧的消息标识，
//! 下游 inbox 再据此判重。批内消息各自独立提交状态，前面成功的不会因后面失败而回滚。

use super::{InboxRetryDecision, InboxRetryPolicy, NewOutboxEvent, OutboxInsertResult};
use crate::{
    error::{AppError, AppResult},
    modules::{events::EventOutboxRepository, market::adapters::MarketFeedEvent},
};
use axum::async_trait;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;

/// 一条待投递的 outbox 消息，是发布轮次在内存中处理的工作单元。
/// 与数据库行的差别是这里只带发布所需字段，不含状态与各类时间戳。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxMessage {
    /// 事件主键，用于发布结束后定位并推进该行状态。
    pub id: u64,
    /// 聚合类型，标明事件所属业务对象类别。
    pub aggregate_type: String,
    /// 聚合实例标识。
    pub aggregate_id: String,
    /// 事件类型名，消费方据此分派处理。
    pub event_type: String,
    /// 路由键，决定 broker 把消息投给哪些队列。
    pub routing_key: String,
    /// 幂等键，必须被 publisher 用作 broker 侧消息标识，是下游去重的依据。
    pub idempotency_key: String,
    /// 事件载荷，原样投递不做加工。
    pub payload: Value,
    /// 已累计的发布失败次数，作为退避策略的输入。
    pub retry_count: u32,
}

/// 消息投递端口，把「发到哪里、怎么发」与「什么时候发、失败怎么办」分开。
/// 实现方必须把消息的幂等键设为 broker 侧的消息标识，否则整条链路的去重能力就断了。
/// 要求可克隆且线程安全，实现的克隆应停留在句柄级别而不复制连接。
#[async_trait]
pub trait OutboxPublisher: Clone + Send + Sync + 'static {
    /// 向外部 broker 发布一条持久化 outbox；幂等键必须作为外部 message_id 保留。
    /// 返回成功的确认强度由具体适配器定义且本 trait 不更新数据库；失败由上层记录 retry/dead-letter。
    async fn publish(&self, message: &OutboxMessage) -> AppResult<()>;
}

/// 一轮发布的结果计数，同时用作手动触发接口的响应体。
/// 后三项之和等于尝试数，除非中途因仓储错误提前终止，此时三者之和会小于尝试数。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PublishedOutboxBatch {
    /// 本轮取到并准备投递的消息数。
    pub attempted: u32,
    /// 投递成功并已标记为已发布的条数。
    pub published: u32,
    /// 投递失败但仍有重试预算、已排定下次到期时间的条数。
    pub retried: u32,
    /// 重试预算耗尽、已转入死信的条数。
    pub dead_lettered: u32,
}

/// 只负责往 outbox 写入事件的轻量入口，不具备发布能力。
/// 与完整的发布服务分开，是为了让只产事件的生产者无需持有 broker 连接与重试策略。
#[derive(Clone)]
pub struct EventOutboxWriter<R> {
    repository: R,
}

impl<R> EventOutboxWriter<R> {
    /// 绑定 outbox 仓储构造 writer；构造过程不建立连接、不开启事务，也不投递任何消息。
    /// 此处不约束仓储类型，具体能力由后续带约束的实现块提供，使该构造在测试替身下同样可用。
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
}

impl<R> EventOutboxWriter<R>
where
    R: EventOutboxRepository,
{
    /// 把行情推送映射成 outbox 事件并写库，行情自带的幂等键被原样保留而不套用聚合拼接规则。
    /// 因此同一笔行情被上游重复推送时会命中已有记录并返回重复标记，不会产生第二条待发消息。
    /// 事务边界与唯一约束都在仓储侧，本入口不直接向 broker 发布任何消息。
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

/// 完整的 outbox 发布服务，把仓储、投递器与重试策略组合成一轮可执行的发布流程。
/// 对仓储与投递器都做了泛型抽象，因此可在测试中替换成内存实现而不需要 MySQL 与 RabbitMQ。
#[derive(Clone)]
pub struct EventOutboxService<R, P> {
    /// 事件持久化仓储，负责取批与推进状态。
    repository: R,
    /// 消息投递器，负责真正把消息交给 broker。
    publisher: P,
    /// 失败退避与死信判定策略，与 inbox 侧共用同一实现。
    retry_policy: InboxRetryPolicy,
    /// 单轮扫描条数上限，零值会得到空批次。
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
