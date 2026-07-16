//! events bounded context repository layer.
//!
//! 仓储层：定义事件出站/入站持久化边界与仓储接口。
//! 出站/入站仓储的实现放在 infrastructure 层，避免 `mod.rs` 夹带 SQL 细节。

use crate::architecture::RepositoryLayer;
use crate::error::AppResult;
use crate::modules::events::{
    InboxClaim, InboxRetryDecision, NewInboxMessage, NewOutboxEvent, OutboxInsertResult,
    OutboxMessage, PendingInboxRetry,
};
use axum::async_trait;
use chrono::{DateTime, Utc};

#[derive(Debug)]
pub struct RepositoryLayerMarker;

impl RepositoryLayer for RepositoryLayerMarker {}

#[async_trait]
pub trait EventOutboxRepository: Clone + Send + Sync + 'static {
    async fn insert_event(&self, event: NewOutboxEvent) -> AppResult<OutboxInsertResult>;

    async fn fetch_publishable_batch(
        &self,
        limit: u32,
        now: DateTime<Utc>,
    ) -> AppResult<Vec<OutboxMessage>>;

    async fn mark_published(&self, id: u64, published_at: DateTime<Utc>) -> AppResult<()>;

    async fn mark_retry(
        &self,
        id: u64,
        retry_count: u32,
        next_retry_at: DateTime<Utc>,
    ) -> AppResult<()>;

    async fn mark_dead_letter(
        &self,
        id: u64,
        retry_count: u32,
        failed_at: DateTime<Utc>,
    ) -> AppResult<()>;
}

#[async_trait]
pub trait EventInboxRepository: Clone + Send + Sync + 'static {
    async fn fetch_due_retries(
        &self,
        consumer_name: &str,
        limit: u32,
        now: DateTime<Utc>,
    ) -> AppResult<Vec<PendingInboxRetry>>;

    async fn claim_message(&self, message: NewInboxMessage) -> AppResult<InboxClaim>;

    async fn mark_consumed(
        &self,
        consumer_name: &str,
        message_id: &str,
        processing_token: &str,
    ) -> AppResult<()>;

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
