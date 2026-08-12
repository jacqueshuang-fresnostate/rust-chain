//! events bounded context infrastructure layer.
//!
//! 基础设施层：封装事件 outbox / inbox 的 SQLx 持久化与并发保护细节。

use crate::error::{AppError, AppResult};
use axum::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::Value;
use sqlx::{
    Error as SqlxError, MySql, Pool, Transaction, error::DatabaseError, types::Json as SqlxJson,
};

use crate::modules::events::domain::{
    INBOX_CONSUMED, INBOX_DEAD_LETTER, INBOX_PROCESSING, INBOX_PROCESSING_LEASE_SECONDS,
    INBOX_PROCESSING_TOKEN_FORMAT, INBOX_RETRY, OUTBOX_DEAD_LETTER, OUTBOX_PENDING,
    OUTBOX_PUBLISHED, OUTBOX_RETRY,
};
use crate::modules::events::repository::{
    EventInboxRepository, EventOutboxRepository, UserWalletInitializer,
};
use crate::modules::events::{
    InboxClaim, InboxRetryDecision, NewInboxMessage, NewOutboxEvent, OutboxInsertResult,
    OutboxMessage, PendingInboxRetry,
};

#[derive(Debug, Clone)]
pub struct MySqlEventOutboxRepository {
    pool: Pool<MySql>,
}

impl MySqlEventOutboxRepository {
    /// 绑定承载 `event_outbox` 的 MySQL 连接池；不立即获取连接、扫描消息或开启发布事务。
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
    /// 绑定承载 `event_inbox` 的 MySQL 连接池；不立即领取消息、创建处理租约或执行消费副作用。
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

/// 将领域事件写入 outbox；幂等键冲突时返回既有记录而不创建重复待发布消息。
/// 独立连接写入由本函数完成，数据库错误直接返回且不会发布外部消息。
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

/// 把领域事件追加到调用方事务中的 outbox，使业务写入与待发布记录保持原子提交。
/// 幂等键冲突沿用既有去重语义；事务回滚时事件记录一并消失，提交前不产生外部发布。
pub(crate) async fn insert_event_in_tx(
    tx: &mut sqlx::Transaction<'_, MySql>,
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
    .execute(&mut **tx)
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
    .fetch_one(&mut **tx)
    .await?
    .0;

    Ok(OutboxInsertResult::Duplicate { id })
}

/// 在用户创建事务内按启用资产补齐钱包账户，依赖唯一键保证并发或重放不产生重复账户。
/// 任何插入错误交由调用方回滚；该步骤只建零余额账户，不生成资金流水或余额变动。
pub(crate) async fn create_wallet_accounts_for_user_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT IGNORE INTO wallet_accounts (user_id, asset_id, available, frozen, locked)
           SELECT ?, id, 0, 0, 0
           FROM assets"#,
    )
    .bind(user_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// 生产环境用户钱包初始化适配器；持有 MySQL 池并为每次事件建立独立原子事务。
#[derive(Debug, Clone)]
pub(crate) struct MySqlUserWalletInitializer {
    pool: Pool<MySql>,
}

impl MySqlUserWalletInitializer {
    /// 绑定 MySQL 池；构造不访问数据库，也不预创建任何账户。
    pub(crate) fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserWalletInitializer for MySqlUserWalletInitializer {
    /// 在单事务内执行 `INSERT IGNORE ... SELECT assets`，确保用户事件重放不会重复创建钱包。
    /// 任一 SQL 或 commit 失败均返回错误并回滚；成功只产生缺失账户，不写余额流水或发布事件。
    async fn initialize_user_wallets(&self, user_id: u64) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;
        create_wallet_accounts_for_user_in_tx(&mut tx, user_id).await?;
        tx.commit().await?;
        Ok(())
    }
}

/// 按 ID 升序读取至多 `limit` 条 `pending` 或 `next_retry_at <= now` 的 `retry` outbox，作为本轮可发布快照。
/// 查询不加领取锁也不预改状态，多实例可能读到同一行；broker message_id 与下游 inbox 幂等负责吸收重复发布。
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

/// 在 publisher 报告 `basic_publish` 完成后把指定 outbox 行标记为 `published`，同时记录该时间与更新时间。
/// 本更新不校验前态也不清空既有重试字段；当前 publisher 未启用 broker confirm，调用方不得把该终态解释为 broker 已持久确认。
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

/// 把一次未获 broker confirm 的 outbox 发布持久化为 `retry`，写入饱和到 `i32::MAX` 的失败次数及下次到期时间。
/// 本函数不重新发送消息；数据库更新失败时原行保持原态，由上层中止本轮并在后续扫描重新判断。
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

/// 把达到策略阈值的 outbox 行标记为 `dead_letter` 并保存最终失败次数与时间，使普通发布扫描不再选中。
/// 不发送死信到另一 exchange，也不自动告警或重排；恢复只能经带管理员审计的显式重排用例完成。
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

/// 按 `consumer_name` 读取至多 `limit` 条到期 retry，或超过 300 秒租约的 processing 行，并携带持久化 payload 供 RabbitMQ ACK 后补偿重放。
/// 结果按到期/更新时间和 ID 排序；该查询不取得处理权，多实例仍须通过 `claim_message` 的行锁重新竞争。
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

/// 在事务内领取收件箱消息并生成处理令牌，使用行锁协调重复投递与并发消费者。
/// 已消费消息保持幂等成功，未到重试时间或仍被有效租约占用时不转移处理权。
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

/// 仅当 consumer、message_id、`processing` 状态和微秒级处理令牌全部匹配时，把 inbox 原子推进为 `consumed`。
/// 零行更新表示租约已被接管或状态已变化并返回陈旧令牌错误；成功只提交 inbox 终态，不负责 RabbitMQ ACK。
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

/// 以处理令牌为条件，把 handler 失败原子落为带到期时间的 `retry` 或无下次时间的 `dead_letter`，并保存错误摘要与失败次数。
/// 零行更新返回陈旧令牌错误，避免旧 worker 覆盖新租约；本入口只持久化消费状态，不执行重放或 broker ACK。
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

/// 判断 SQLx 数据库错误是否为 MySQL 1062 唯一键冲突，供并发插入 inbox 时转入已存在行的锁定判定。
pub(crate) fn is_unique_violation(error: &SqlxError) -> bool {
    error
        .as_database_error()
        .and_then(DatabaseError::code)
        .as_deref()
        == Some("1062")
}

/// 判断持久化的 `next_retry_at` 是否晚于当前 UTC；`None` 视为已经到期，供领取逻辑阻止提前重放。
pub(crate) fn retry_is_not_due(next_retry_at: Option<chrono::NaiveDateTime>) -> bool {
    next_retry_at.is_some_and(|value| value.and_utc() > Utc::now())
}

/// 判断 processing 行的 `updated_at + 300 秒` 是否已到期；到期仅代表允许重新竞争，不在此处修改租约所有权。
pub(crate) fn processing_is_stale(updated_at: chrono::NaiveDateTime) -> bool {
    updated_at.and_utc() + TimeDelta::seconds(INBOX_PROCESSING_LEASE_SECONDS) <= Utc::now()
}

/// 把领取时的 MySQL 微秒时间戳编码为稳定处理令牌，后续 consumed/retry/dead-letter 更新以该值做乐观租约条件。
pub(crate) fn processing_token(value: chrono::NaiveDateTime) -> String {
    value.format(INBOX_PROCESSING_TOKEN_FORMAT).to_string()
}

/// 严格按 MySQL 微秒时间格式解析处理令牌；格式损坏统一视为陈旧租约，调用方不得据此更新 inbox。
pub(crate) fn parse_processing_token(value: &str) -> AppResult<chrono::NaiveDateTime> {
    chrono::NaiveDateTime::parse_from_str(value, INBOX_PROCESSING_TOKEN_FORMAT)
        .map_err(|_| processing_token_is_stale_error())
}

/// 构造统一的 inbox 陈旧处理令牌错误，供解析失败或条件更新零行时保持同一并发失败语义。
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

/// 对已锁定 inbox 行决定是否重新领取：到期 retry 与超过 300 秒的同 message processing 可取得新令牌，其余终态或不同 message 视为重复。
/// 同一 message 尚持有效 processing 租约时返回明确错误，避免并发 handler；本纯决策不更新行，实际所有权由外层事务提交。
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

#[derive(Debug, Clone, Copy)]
/// 持久化层的事件记录筛选参数，值已由表现层完成边界归一化。
pub(crate) struct EventRecordListFilter<'a> {
    pub(crate) status: Option<&'a str>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug, sqlx::FromRow)]
/// outbox SQL 行模型，只在 infrastructure/application 边界内流转。
pub(crate) struct OutboxRecordRow {
    pub(crate) id: u64,
    pub(crate) aggregate_type: String,
    pub(crate) aggregate_id: String,
    pub(crate) event_type: String,
    pub(crate) routing_key: String,
    pub(crate) status: String,
    pub(crate) retry_count: i32,
    pub(crate) next_retry_at: Option<DateTime<Utc>>,
    pub(crate) published_at: Option<DateTime<Utc>>,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
/// inbox SQL 行模型，只在 infrastructure/application 边界内流转。
pub(crate) struct InboxRecordRow {
    pub(crate) id: u64,
    pub(crate) consumer_name: String,
    pub(crate) message_id: String,
    pub(crate) status: String,
    pub(crate) retry_count: i32,
    pub(crate) error_message: Option<String>,
    pub(crate) consumed_at: Option<DateTime<Utc>>,
    pub(crate) created_at: DateTime<Utc>,
}

/// 死信与积压事件的运维查询：计数与行查询使用同一筛选条件。
/// 行查询与总数共享运维筛选；数据库失败不返回不完整死信或积压页。
pub(crate) async fn list_outbox_records(
    pool: &Pool<MySql>,
    filter: EventRecordListFilter<'_>,
) -> AppResult<(Vec<OutboxRecordRow>, i64)> {
    let rows = sqlx::query_as::<_, OutboxRecordRow>(
        r#"SELECT id, aggregate_type, aggregate_id, event_type, routing_key, status,
                  retry_count, next_retry_at, published_at, created_at
           FROM event_outbox
           WHERE (? IS NULL OR status = ?)
           ORDER BY id DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(filter.status)
    .bind(filter.status)
    .bind(i64::from(filter.limit))
    .bind(i64::from(filter.offset))
    .fetch_all(pool)
    .await?;
    let (total,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM event_outbox WHERE (? IS NULL OR status = ?)")
            .bind(filter.status)
            .bind(filter.status)
            .fetch_one(pool)
            .await?;

    Ok((rows, total))
}

/// inbox 运维查询仅提供持久化记录，权限控制和响应映射由应用层负责。
/// 按消费者和状态返回 inbox 运维页及一致总数，不领取消息或推进重试。
pub(crate) async fn list_inbox_records(
    pool: &Pool<MySql>,
    filter: EventRecordListFilter<'_>,
) -> AppResult<(Vec<InboxRecordRow>, i64)> {
    let rows = sqlx::query_as::<_, InboxRecordRow>(
        r#"SELECT id, consumer_name, message_id, status, retry_count, error_message,
                  consumed_at, created_at
           FROM event_inbox
           WHERE (? IS NULL OR status = ?)
           ORDER BY id DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(filter.status)
    .bind(filter.status)
    .bind(i64::from(filter.limit))
    .bind(i64::from(filter.offset))
    .fetch_all(pool)
    .await?;
    let (total,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM event_inbox WHERE (? IS NULL OR status = ?)")
            .bind(filter.status)
            .bind(filter.status)
            .fetch_one(pool)
            .await?;

    Ok((rows, total))
}

/// 在调用方事务中锁定并重排死信，同时写入管理员审计；调用方负责提交或回滚。
///
/// 只有 `dead_letter` 可转为 `pending`，重复重排会返回冲突且不会新增审计记录。
pub(crate) async fn requeue_outbox_dead_letter_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    id: u64,
    reason: &str,
) -> AppResult<OutboxRecordRow> {
    let before = sqlx::query_as::<_, OutboxRecordRow>(
        r#"SELECT id, aggregate_type, aggregate_id, event_type, routing_key, status,
                  retry_count, next_retry_at, published_at, created_at
           FROM event_outbox WHERE id = ? FOR UPDATE"#,
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if before.status != OUTBOX_DEAD_LETTER {
        return Err(AppError::Conflict(
            "only dead-lettered outbox events can be requeued".to_owned(),
        ));
    }

    sqlx::query(
        "UPDATE event_outbox SET status = ?, retry_count = 0, next_retry_at = NULL WHERE id = ?",
    )
    .bind(OUTBOX_PENDING)
    .bind(id)
    .execute(&mut **tx)
    .await?;
    insert_event_admin_audit_log_in_tx(
        tx,
        admin_id,
        "event_outbox.requeue",
        "event_outbox",
        &id.to_string(),
        &before,
        reason,
    )
    .await?;
    let after = OutboxRecordRow {
        status: OUTBOX_PENDING.to_owned(),
        retry_count: 0,
        next_retry_at: None,
        ..before
    };
    Ok(after)
}

async fn insert_event_admin_audit_log_in_tx(
    tx: &mut sqlx::Transaction<'_, MySql>,
    admin_id: u64,
    action: &str,
    target_type: &str,
    target_id: &str,
    before: &OutboxRecordRow,
    reason: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, before_json, after_json, reason)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(admin_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(SqlxJson(serde_json::json!({
        "status": before.status,
        "retry_count": before.retry_count,
    })))
    .bind(SqlxJson(serde_json::json!({
        "status": OUTBOX_PENDING,
        "retry_count": 0,
    })))
    .bind(reason)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
