//! events bounded context repository layer.
//!
//! 仓储层：定义事件出站/入站持久化边界与仓储接口。
//! 出站/入站仓储的实现放在 infrastructure 层，避免 `mod.rs` 夹带 SQL 细节。
//!
//! 这里定义的是端口而非实现：service 层只依赖这些 trait，因而可以在单元测试中换成内存实现，
//! 不必依赖真实数据库。三个 trait 分别覆盖发布侧持久化、消费侧持久化，以及用户钱包初始化这一具体副作用。
//! trait 上的文档即实现方必须遵守的行为契约，尤其是幂等性、租约令牌校验和「不得越权代做」这几条：
//! 仓储只负责持久化状态，既不发送消息也不做 broker 确认，更不执行业务处理逻辑。

use crate::error::AppResult;
use crate::modules::events::{
    InboxClaim, InboxRetryDecision, NewInboxMessage, NewOutboxEvent, OutboxInsertResult,
    OutboxMessage, PendingInboxRetry,
};
use axum::async_trait;
use chrono::{DateTime, Utc};

/// 用户创建事件触发的钱包初始化端口；service 只依赖该抽象，不感知 MySQL 或事务实现。
///
/// 实现必须保证相同 user_id 重放不会重复产生余额或流水；具体事务、锁和 SQL 副作用由适配器拥有。
#[async_trait]
pub trait UserWalletInitializer: Send + Sync + 'static {
    /// 为已持久化用户补齐全部资产钱包账户；user_id 必须来自通过 envelope 一致性校验的事件。
    /// 重放应幂等，失败不得留下部分账户；错误返回后 inbox 会按既有策略重试或进入死信。
    async fn initialize_user_wallets(&self, user_id: u64) -> AppResult<()>;
}

/// 事件发布侧的持久化端口，覆盖写入、扫描与三种终态推进。
/// 要求可克隆且线程安全，因为发布服务会被多个任务共享；实现应把克隆代价控制在句柄级别。
/// 实现方不得在任何方法中真正发送消息，投递由 publisher 负责，本端口只记录状态。
#[async_trait]
pub trait EventOutboxRepository: Clone + Send + Sync + 'static {
    /// 原子插入一条 outbox 事件；idempotency_key 重复时返回既有 ID，不重复创建消息。
    /// 实现拥有数据库事务/唯一约束，失败不得伪造 Inserted 或发布外部消息。
    async fn insert_event(&self, event: NewOutboxEvent) -> AppResult<OutboxInsertResult>;

    /// 读取当前可发布的 pending/retry 批次；limit 约束规模，now 决定到期边界。
    /// 只读操作不改变状态；重复读取允许返回尚未被成功标记的同一消息。
    async fn fetch_publishable_batch(
        &self,
        limit: u32,
        now: DateTime<Utc>,
    ) -> AppResult<Vec<OutboxMessage>>;

    /// 在 publisher 返回成功后把指定消息标记为 published，并记录状态推进时间；仓储不判断该成功是否为 broker confirm。
    /// 重放必须保持终态幂等；数据库失败返回错误，调用方下轮可能再次发布同一 message_id。
    async fn mark_published(&self, id: u64, published_at: DateTime<Utc>) -> AppResult<()>;

    /// 记录一次可重试发布失败及下次到期时间；retry_count 必须来自策略计算结果。
    /// 仅推进 outbox 状态，不重新发布；更新失败原样返回，不吞掉待处理消息。
    async fn mark_retry(
        &self,
        id: u64,
        retry_count: u32,
        next_retry_at: DateTime<Utc>,
    ) -> AppResult<()>;

    /// 达到阈值后把消息标记为 dead_letter；终态不得被普通发布扫描再次领取。
    /// 仅记录持久状态，不发送告警或外部消息；人工重排由独立应用用例负责。
    async fn mark_dead_letter(
        &self,
        id: u64,
        retry_count: u32,
        failed_at: DateTime<Utc>,
    ) -> AppResult<()>;
}

/// 事件消费侧的持久化端口，覆盖补偿扫描、租约领取与消费终态推进。
/// 与发布侧的关键差别是所有状态推进都必须校验处理令牌，实现方不得提供绕过令牌的更新路径，
/// 否则崩溃重启后的旧 worker 会覆盖新持有者的处理结果。
/// 同样要求可克隆且线程安全，实现不得在方法内执行业务 handler 或对 broker 做确认。
#[async_trait]
pub trait EventInboxRepository: Clone + Send + Sync + 'static {
    /// 读取指定 consumer 当前到期的 retry 行；limit 限制批次，now 定义到期边界。
    /// 只读不获取最终处理所有权，调用方仍须通过 claim_message 竞争处理租约。
    async fn fetch_due_retries(
        &self,
        consumer_name: &str,
        limit: u32,
        now: DateTime<Utc>,
    ) -> AppResult<Vec<PendingInboxRetry>>;

    /// 以 consumer/message_id 原子领取 inbox 消息并校验幂等键与 payload hash。
    /// 实现必须保证领取是原子的：并发调用中最多只有一方能取得处理权，其余得到重复结论。
    /// 已进入终态、尚未到退避时间或租约仍被他人有效持有的消息都不得被领取。
    /// 成功时返回处理令牌，调用方后续推进状态必须原样带上它。
    async fn claim_message(&self, message: NewInboxMessage) -> AppResult<InboxClaim>;

    /// 使用 processing_token 把已成功处理的消息推进为 consumed。
    /// 令牌过期/不匹配必须失败，避免旧 worker 覆盖新租约；不负责业务事务或 broker ACK。
    async fn mark_consumed(
        &self,
        consumer_name: &str,
        message_id: &str,
        processing_token: &str,
    ) -> AppResult<()>;

    /// 使用处理租约记录业务失败，并按策略推进 retry 或 dead_letter 与错误摘要。
    /// 状态更新失败原样返回，broker delivery 不应被误 ACK；本方法不重执业务 handler。
    async fn mark_failure(
        &self,
        consumer_name: &str,
        message_id: &str,
        processing_token: &str,
        decision: InboxRetryDecision,
        error_message: &str,
        now: DateTime<Utc>,
    ) -> AppResult<()>;
}
