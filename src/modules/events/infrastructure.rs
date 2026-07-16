//! events bounded context infrastructure layer.
//!
//! 基础设施层：封装事件 outbox / inbox 的 SQLx 持久化与并发保护细节。

use crate::error::{AppError, AppResult};
use axum::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::Value;
use sqlx::{Error as SqlxError, MySql, Pool, error::DatabaseError, types::Json as SqlxJson};

use crate::modules::events::domain::{
    INBOX_CONSUMED, INBOX_DEAD_LETTER, INBOX_PROCESSING, INBOX_PROCESSING_LEASE_SECONDS,
    INBOX_PROCESSING_TOKEN_FORMAT, INBOX_RETRY, OUTBOX_DEAD_LETTER, OUTBOX_PENDING,
    OUTBOX_PUBLISHED, OUTBOX_RETRY,
};
use crate::modules::events::repository::{EventInboxRepository, EventOutboxRepository};
use crate::modules::events::{
    InboxClaim, InboxRetryDecision, NewInboxMessage, NewOutboxEvent, OutboxInsertResult,
    OutboxMessage, PendingInboxRetry,
};

#[derive(Debug, Clone)]
pub struct MySqlEventOutboxRepository {
    pool: Pool<MySql>,
}

impl MySqlEventOutboxRepository {
    pub fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventOutboxRepository for MySqlEventOutboxRepository {
    async fn insert_event(&self, event: NewOutboxEvent) -> AppResult<OutboxInsertResult> {
        insert_event(&self.pool, &event).await
    }

    async fn fetch_publishable_batch(
        &self,
        limit: u32,
        now: DateTime<Utc>,
    ) -> AppResult<Vec<OutboxMessage>> {
        fetch_publishable_batch(&self.pool, limit, now).await
    }

    async fn mark_published(&self, id: u64, published_at: DateTime<Utc>) -> AppResult<()> {
        mark_published(&self.pool, id, published_at).await
    }

    async fn mark_retry(
        &self,
        id: u64,
        retry_count: u32,
        next_retry_at: DateTime<Utc>,
    ) -> AppResult<()> {
        mark_retry(&self.pool, id, retry_count, next_retry_at).await
    }

    async fn mark_dead_letter(
        &self,
        id: u64,
        retry_count: u32,
        failed_at: DateTime<Utc>,
    ) -> AppResult<()> {
        mark_dead_letter(&self.pool, id, retry_count, failed_at).await
    }
}

#[derive(Debug, Clone)]
pub struct MySqlEventInboxRepository {
    pool: Pool<MySql>,
}

impl MySqlEventInboxRepository {
    pub fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventInboxRepository for MySqlEventInboxRepository {
    async fn fetch_due_retries(
        &self,
        consumer_name: &str,
        limit: u32,
        now: DateTime<Utc>,
    ) -> AppResult<Vec<PendingInboxRetry>> {
        fetch_due_retries(&self.pool, consumer_name, limit, now).await
    }

    async fn claim_message(&self, message: NewInboxMessage) -> AppResult<InboxClaim> {
        claim_message(&self.pool, message).await
    }

    async fn mark_consumed(
        &self,
        consumer_name: &str,
        message_id: &str,
        processing_token: &str,
    ) -> AppResult<()> {
        mark_consumed(&self.pool, consumer_name, message_id, processing_token).await
    }

    async fn mark_failure(
        &self,
        consumer_name: &str,
        message_id: &str,
        processing_token: &str,
        decision: InboxRetryDecision,
        error_message: &str,
        now: DateTime<Utc>,
    ) -> AppResult<()> {
        mark_failure(
            &self.pool,
            consumer_name,
            message_id,
            processing_token,
            decision,
            error_message,
            now,
        )
        .await
    }
}

pub(crate) async fn insert_event(
    pool: &Pool<MySql>,
    event: &NewOutboxEvent,
) -> AppResult<OutboxInsertResult> {
    let result = sqlx::query(
        r#"INSERT INTO event_outbox
           (aggregate_type, aggregate_id, event_type, routing_key, idempotency_key, payload_json, status, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE idempotency_key = idempotency_key"#,
    )
    .bind(&event.aggregate_type)
    .bind(&event.aggregate_id)
    .bind(&event.event_type)
    .bind(&event.routing_key)
    .bind(&event.idempotency_key)
    .bind(SqlxJson(event.payload.clone()))
    .bind(OUTBOX_PENDING)
    .bind(event.created_at.naive_utc())
    .execute(pool)
    .await?;

    if result.last_insert_id() != 0 {
        return Ok(OutboxInsertResult::Inserted {
            id: result.last_insert_id(),
        });
    }

    let id = sqlx::query_as::<_, (u64,)>(
        "SELECT id FROM event_outbox WHERE idempotency_key = ? LIMIT 1",
    )
    .bind(&event.idempotency_key)
    .fetch_one(pool)
    .await?
    .0;

    Ok(OutboxInsertResult::Duplicate { id })
}

pub(crate) async fn fetch_publishable_batch(
    pool: &Pool<MySql>,
    limit: u32,
    now: DateTime<Utc>,
) -> AppResult<Vec<OutboxMessage>> {
    type OutboxRow = (
        u64,
        String,
        String,
        String,
        String,
        String,
        SqlxJson<Value>,
        i32,
    );

    let rows = sqlx::query_as::<_, OutboxRow>(
        r#"SELECT id, aggregate_type, aggregate_id, event_type, routing_key, idempotency_key, payload_json, retry_count
           FROM event_outbox
           WHERE status IN ('pending', 'retry') AND (next_retry_at IS NULL OR next_retry_at <= ?)
           ORDER BY id ASC
           LIMIT ?"#,
    )
    .bind(now.naive_utc())
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                aggregate_type,
                aggregate_id,
                event_type,
                routing_key,
                idempotency_key,
                SqlxJson(payload),
                retry_count,
            )| OutboxMessage {
                id,
                aggregate_type,
                aggregate_id,
                event_type,
                routing_key,
                idempotency_key,
                payload,
                retry_count: retry_count.max(0) as u32,
            },
        )
        .collect())
}

pub(crate) async fn mark_published(
    pool: &Pool<MySql>,
    id: u64,
    published_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE event_outbox SET status = ?, published_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(OUTBOX_PUBLISHED)
    .bind(published_at.naive_utc())
    .bind(published_at.naive_utc())
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn mark_retry(
    pool: &Pool<MySql>,
    id: u64,
    retry_count: u32,
    next_retry_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE event_outbox SET status = ?, retry_count = ?, next_retry_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(OUTBOX_RETRY)
    .bind(i32::try_from(retry_count).unwrap_or(i32::MAX))
    .bind(next_retry_at.naive_utc())
    .bind(Utc::now().naive_utc())
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn mark_dead_letter(
    pool: &Pool<MySql>,
    id: u64,
    retry_count: u32,
    failed_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query("UPDATE event_outbox SET status = ?, retry_count = ?, updated_at = ? WHERE id = ?")
        .bind(OUTBOX_DEAD_LETTER)
        .bind(i32::try_from(retry_count).unwrap_or(i32::MAX))
        .bind(failed_at.naive_utc())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub(crate) async fn fetch_due_retries(
    pool: &Pool<MySql>,
    consumer_name: &str,
    limit: u32,
    now: DateTime<Utc>,
) -> AppResult<Vec<PendingInboxRetry>> {
    let stale_processing_before =
        (now - TimeDelta::seconds(INBOX_PROCESSING_LEASE_SECONDS)).naive_utc();
    let rows = sqlx::query_as::<_, (String, String, SqlxJson<Value>)>(
        r#"SELECT message_id, idempotency_key, payload_json
           FROM event_inbox
           WHERE consumer_name = ?
             AND (
                (status = ? AND (next_retry_at IS NULL OR next_retry_at <= ?))
                OR (status = ? AND updated_at <= ?)
             )
           ORDER BY COALESCE(next_retry_at, updated_at) ASC, id ASC
           LIMIT ?"#,
    )
    .bind(consumer_name)
    .bind(INBOX_RETRY)
    .bind(now.naive_utc())
    .bind(INBOX_PROCESSING)
    .bind(stale_processing_before)
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(message_id, idempotency_key, SqlxJson(payload))| PendingInboxRetry {
                consumer_name: consumer_name.to_owned(),
                message_id,
                idempotency_key,
                payload,
            },
        )
        .collect())
}

pub(crate) async fn claim_message(
    pool: &Pool<MySql>,
    message: NewInboxMessage,
) -> AppResult<InboxClaim> {
    let mut tx = pool.begin().await?;
    let existing = sqlx::query_as::<
        _,
        (
            String,
            i32,
            String,
            Option<chrono::NaiveDateTime>,
            chrono::NaiveDateTime,
        ),
    >(
        r#"SELECT status, retry_count, message_id, CAST(next_retry_at AS DATETIME(6)), CAST(updated_at AS DATETIME(6))
           FROM event_inbox
           WHERE consumer_name = ? AND (message_id = ? OR idempotency_key = ?) LIMIT 1 FOR UPDATE"#,
    )
    .bind(&message.consumer_name)
    .bind(&message.message_id)
    .bind(&message.idempotency_key)
    .fetch_optional(&mut *tx)
    .await?;

    let claim = if let Some(existing) = existing {
        let existing = ExistingInboxMessage::from(existing);
        let claimed_at = Utc::now().naive_utc();
        let claim =
            decide_existing_inbox_claim(&message, existing.clone(), processing_token(claimed_at))?;
        if matches!(claim, InboxClaim::Claimed { .. }) {
            sqlx::query(
                "UPDATE event_inbox
                 SET status = ?, error_message = NULL, updated_at = ?
                 WHERE consumer_name = ? AND message_id = ?",
            )
            .bind(INBOX_PROCESSING)
            .bind(claimed_at)
            .bind(&message.consumer_name)
            .bind(&existing.message_id)
            .execute(&mut *tx)
            .await?;
        }
        claim
    } else {
        let claimed_at = Utc::now().naive_utc();
        let inserted = sqlx::query(
            r#"INSERT INTO event_inbox
               (consumer_name, message_id, idempotency_key, payload_hash, payload_json, status, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&message.consumer_name)
        .bind(&message.message_id)
        .bind(&message.idempotency_key)
        .bind(&message.payload_hash)
        .bind(SqlxJson(message.payload.clone()))
        .bind(INBOX_PROCESSING)
        .bind(claimed_at)
        .execute(&mut *tx)
        .await;

        match inserted {
            Ok(_) => InboxClaim::Claimed {
                attempt_count: 0,
                processing_token: processing_token(claimed_at),
            },
            Err(error) if is_unique_violation(&error) => {
                let existing = sqlx::query_as::<
                    _,
                    (
                        String,
                        i32,
                        String,
                        Option<chrono::NaiveDateTime>,
                        chrono::NaiveDateTime,
                    ),
                >(r#"SELECT status, retry_count, message_id, CAST(next_retry_at AS DATETIME(6)), CAST(updated_at AS DATETIME(6))
                   FROM event_inbox
                   WHERE consumer_name = ? AND (message_id = ? OR idempotency_key = ?) LIMIT 1 FOR UPDATE"#)
                .bind(&message.consumer_name)
                .bind(&message.message_id)
                .bind(&message.idempotency_key)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| {
                    AppError::Internal("event inbox unique conflict row was not found".to_owned())
                })?;
                let existing = ExistingInboxMessage::from(existing);
                let claimed_at = Utc::now().naive_utc();
                let claim = decide_existing_inbox_claim(
                    &message,
                    existing.clone(),
                    processing_token(claimed_at),
                )?;
                if matches!(claim, InboxClaim::Claimed { .. }) {
                    sqlx::query(
                        "UPDATE event_inbox
                         SET status = ?, error_message = NULL, updated_at = ?
                         WHERE consumer_name = ? AND message_id = ?",
                    )
                    .bind(INBOX_PROCESSING)
                    .bind(claimed_at)
                    .bind(&message.consumer_name)
                    .bind(&existing.message_id)
                    .execute(&mut *tx)
                    .await?;
                }
                claim
            }
            Err(error) => return Err(error.into()),
        }
    };

    tx.commit().await?;
    Ok(claim)
}

pub(crate) async fn mark_consumed(
    pool: &Pool<MySql>,
    consumer_name: &str,
    message_id: &str,
    processing_token: &str,
) -> AppResult<()> {
    let now = Utc::now().naive_utc();
    let processing_updated_at = parse_processing_token(processing_token)?;
    let result = sqlx::query(
        "UPDATE event_inbox SET status = ?, error_message = NULL, consumed_at = ?, updated_at = ? WHERE consumer_name = ? AND message_id = ? AND status = ? AND updated_at = ?",
    )
    .bind(INBOX_CONSUMED)
    .bind(now)
    .bind(now)
    .bind(consumer_name)
    .bind(message_id)
    .bind(INBOX_PROCESSING)
    .bind(processing_updated_at)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(processing_token_is_stale_error());
    }

    Ok(())
}

pub(crate) async fn mark_failure(
    pool: &Pool<MySql>,
    consumer_name: &str,
    message_id: &str,
    processing_token: &str,
    decision: InboxRetryDecision,
    error_message: &str,
    now: DateTime<Utc>,
) -> AppResult<()> {
    let (status, attempt_count, next_retry_at) = match decision {
        InboxRetryDecision::Retry {
            attempt_count,
            next_retry_at,
        } => (INBOX_RETRY, attempt_count, Some(next_retry_at)),
        InboxRetryDecision::DeadLetter { attempt_count } => {
            (INBOX_DEAD_LETTER, attempt_count, None)
        }
    };

    let processing_updated_at = parse_processing_token(processing_token)?;
    let result = sqlx::query(
        "UPDATE event_inbox SET status = ?, error_message = ?, retry_count = ?, next_retry_at = ?, updated_at = ? WHERE consumer_name = ? AND message_id = ? AND status = ? AND updated_at = ?",
    )
    .bind(status)
    .bind(error_message)
    .bind(i32::try_from(attempt_count).unwrap_or(i32::MAX))
    .bind(next_retry_at.map(|value| value.naive_utc()))
    .bind(now.naive_utc())
    .bind(consumer_name)
    .bind(message_id)
    .bind(INBOX_PROCESSING)
    .bind(processing_updated_at)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(processing_token_is_stale_error());
    }

    Ok(())
}

pub(crate) fn is_unique_violation(error: &SqlxError) -> bool {
    error
        .as_database_error()
        .and_then(DatabaseError::code)
        .as_deref()
        == Some("1062")
}

pub(crate) fn retry_is_not_due(next_retry_at: Option<chrono::NaiveDateTime>) -> bool {
    next_retry_at.is_some_and(|value| value.and_utc() > Utc::now())
}

pub(crate) fn processing_is_stale(updated_at: chrono::NaiveDateTime) -> bool {
    updated_at.and_utc() + TimeDelta::seconds(INBOX_PROCESSING_LEASE_SECONDS) <= Utc::now()
}

pub(crate) fn processing_token(value: chrono::NaiveDateTime) -> String {
    value.format(INBOX_PROCESSING_TOKEN_FORMAT).to_string()
}

pub(crate) fn parse_processing_token(value: &str) -> AppResult<chrono::NaiveDateTime> {
    chrono::NaiveDateTime::parse_from_str(value, INBOX_PROCESSING_TOKEN_FORMAT)
        .map_err(|_| processing_token_is_stale_error())
}

pub(crate) fn processing_token_is_stale_error() -> AppError {
    AppError::Internal("event inbox processing token is stale".to_owned())
}

#[derive(Debug, Clone)]
pub(crate) struct ExistingInboxMessage {
    pub status: String,
    pub retry_count: i32,
    pub message_id: String,
    pub next_retry_at: Option<chrono::NaiveDateTime>,
    pub updated_at: chrono::NaiveDateTime,
}

impl
    From<(
        String,
        i32,
        String,
        Option<chrono::NaiveDateTime>,
        chrono::NaiveDateTime,
    )> for ExistingInboxMessage
{
    fn from(
        value: (
            String,
            i32,
            String,
            Option<chrono::NaiveDateTime>,
            chrono::NaiveDateTime,
        ),
    ) -> Self {
        Self {
            status: value.0,
            retry_count: value.1,
            message_id: value.2,
            next_retry_at: value.3,
            updated_at: value.4,
        }
    }
}

pub(crate) fn decide_existing_inbox_claim(
    message: &NewInboxMessage,
    existing: ExistingInboxMessage,
    processing_token: String,
) -> AppResult<InboxClaim> {
    if existing.status == INBOX_RETRY {
        if existing.message_id != message.message_id || retry_is_not_due(existing.next_retry_at) {
            Ok(InboxClaim::Duplicate)
        } else {
            Ok(InboxClaim::Claimed {
                attempt_count: existing.retry_count.max(0) as u32,
                processing_token,
            })
        }
    } else if existing.status == INBOX_PROCESSING {
        if existing.message_id != message.message_id {
            Ok(InboxClaim::Duplicate)
        } else if processing_is_stale(existing.updated_at) {
            Ok(InboxClaim::Claimed {
                attempt_count: existing.retry_count.max(0) as u32,
                processing_token,
            })
        } else {
            Err(AppError::Internal(
                "event inbox message is already processing".to_owned(),
            ))
        }
    } else {
        Ok(InboxClaim::Duplicate)
    }
}
