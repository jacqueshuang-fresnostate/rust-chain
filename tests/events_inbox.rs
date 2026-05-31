use axum::async_trait;
use chrono::{TimeDelta, TimeZone, Utc};
use exchange_api::{
    error::{AppError, AppResult},
    modules::events::{
        ConsumedInboxMessage, EventInboxAlert, EventInboxAlertKind, EventInboxAlertSeverity,
        EventInboxConsumerService, EventInboxHandler, EventInboxProductionHandler,
        EventInboxRepository, InboundEventMessage, InboxClaim, InboxDeliveryDisposition,
        InboxRetryDecision, InboxRetryPolicy, MySqlEventInboxRepository, NewInboxMessage,
        PendingInboxRetry, ProcessedInboxDelivery, ProductionEventDispatch,
    },
    workers::event_inbox::{
        EventInboxConsumerCycleOutcome, EventInboxReconnectBackoff, EventInboxWorkerConfig,
    },
};
use lapin::{BasicProperties, acker::Acker, message::Delivery, types::ShortString};
use serde_json::{Value, json};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions, types::Json as SqlxJson};
use std::{collections::VecDeque, sync::Arc};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Clone, Default)]
struct FakeInboxRepository {
    state: Arc<Mutex<FakeInboxRepositoryState>>,
}

#[derive(Default)]
struct FakeInboxRepositoryState {
    claims: VecDeque<InboxClaim>,
    claim_failures: VecDeque<AppError>,
    pending_retries: VecDeque<PendingInboxRetry>,
    claimed_messages: Vec<NewInboxMessage>,
    consumed_messages: Vec<(String, String, String)>,
    failed_messages: Vec<RecordedFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordedFailure {
    consumer_name: String,
    message_id: String,
    processing_token: String,
    decision: InboxRetryDecision,
    error_message: String,
}

fn claimed(attempt_count: u32, processing_token: &str) -> InboxClaim {
    InboxClaim::Claimed {
        attempt_count,
        processing_token: processing_token.to_owned(),
    }
}

impl FakeInboxRepository {
    async fn with_claims(claims: impl IntoIterator<Item = InboxClaim>) -> Self {
        let repository = Self::default();
        repository.state.lock().await.claims = claims.into_iter().collect();
        repository
    }

    async fn with_pending_retries(retries: impl IntoIterator<Item = PendingInboxRetry>) -> Self {
        let repository = Self::default();
        repository.state.lock().await.pending_retries = retries.into_iter().collect();
        repository
    }

    async fn claimed_messages(&self) -> Vec<NewInboxMessage> {
        self.state.lock().await.claimed_messages.clone()
    }

    async fn consumed_messages(&self) -> Vec<(String, String, String)> {
        self.state.lock().await.consumed_messages.clone()
    }

    async fn failed_messages(&self) -> Vec<RecordedFailure> {
        self.state.lock().await.failed_messages.clone()
    }
}

#[async_trait]
impl EventInboxRepository for FakeInboxRepository {
    async fn fetch_due_retries(
        &self,
        _consumer_name: &str,
        limit: u32,
        _now: chrono::DateTime<Utc>,
    ) -> AppResult<Vec<PendingInboxRetry>> {
        let mut state = self.state.lock().await;
        let limit = (limit as usize).min(state.pending_retries.len());
        Ok(state.pending_retries.drain(..limit).collect())
    }

    async fn claim_message(&self, message: NewInboxMessage) -> AppResult<InboxClaim> {
        let mut state = self.state.lock().await;
        state.claimed_messages.push(message);
        if let Some(error) = state.claim_failures.pop_front() {
            return Err(error);
        }
        Ok(state.claims.pop_front().unwrap_or(InboxClaim::Duplicate))
    }

    async fn mark_consumed(
        &self,
        consumer_name: &str,
        message_id: &str,
        processing_token: &str,
    ) -> AppResult<()> {
        self.state.lock().await.consumed_messages.push((
            consumer_name.to_owned(),
            message_id.to_owned(),
            processing_token.to_owned(),
        ));
        Ok(())
    }

    async fn mark_failure(
        &self,
        consumer_name: &str,
        message_id: &str,
        processing_token: &str,
        decision: InboxRetryDecision,
        error_message: &str,
        _now: chrono::DateTime<Utc>,
    ) -> AppResult<()> {
        self.state
            .lock()
            .await
            .failed_messages
            .push(RecordedFailure {
                consumer_name: consumer_name.to_owned(),
                message_id: message_id.to_owned(),
                processing_token: processing_token.to_owned(),
                decision,
                error_message: error_message.to_owned(),
            });
        Ok(())
    }
}

#[derive(Clone, Default)]
struct FakeInboxHandler {
    state: Arc<Mutex<FakeInboxHandlerState>>,
}

#[derive(Default)]
struct FakeInboxHandlerState {
    handled_messages: Vec<InboundEventMessage>,
    failures: VecDeque<&'static str>,
}

impl FakeInboxHandler {
    async fn fail_next(&self, message: &'static str) {
        self.state.lock().await.failures.push_back(message);
    }

    async fn handled_messages(&self) -> Vec<InboundEventMessage> {
        self.state.lock().await.handled_messages.clone()
    }
}

#[async_trait]
impl EventInboxHandler for FakeInboxHandler {
    async fn handle(&self, message: &InboundEventMessage) -> AppResult<()> {
        let mut state = self.state.lock().await;
        state.handled_messages.push(message.clone());
        if let Some(message) = state.failures.pop_front() {
            return Err(AppError::Internal(message.to_owned()));
        }
        Ok(())
    }
}

async fn mysql_pool() -> Option<MySqlPool> {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!(
                "skipping MySQL event inbox integration test because DATABASE_URL is not set"
            );
            return None;
        }
    };

    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    Some(pool)
}

fn unique_suffix() -> String {
    Uuid::now_v7().simple().to_string()
}

fn retry_payload(idempotency_key: &str, order_id: &str) -> Value {
    json!({
        "aggregate_type": "convert_order",
        "aggregate_id": order_id,
        "event_type": "completed",
        "routing_key": "convert.order.completed",
        "idempotency_key": idempotency_key,
        "payload": { "order_id": order_id }
    })
}

fn mysql_processing_token(value: chrono::NaiveDateTime) -> String {
    value.format("%Y-%m-%d %H:%M:%S%.6f").to_string()
}

fn event_inbox_external_dependencies_are_available() -> bool {
    std::env::var("REDIS_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .is_some()
        && std::env::var("MONGO_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .is_some()
}

#[tokio::test]
async fn inbox_consumer_claims_message_handles_once_and_marks_consumed() {
    let now = Utc.with_ymd_and_hms(2026, 5, 26, 15, 0, 0).unwrap();
    let repository = FakeInboxRepository::with_claims([claimed(0, "token-0")]).await;
    let handler = FakeInboxHandler::default();
    let service = inbox_service(repository.clone(), handler.clone());
    let message = inbox_message("message-1");

    let result = service.consume_one(message.clone(), now).await.unwrap();

    assert_eq!(result, ConsumedInboxMessage::Consumed);
    assert_eq!(handler.handled_messages().await, vec![message]);
    assert_eq!(
        repository.consumed_messages().await,
        vec![(
            "wallet-balance".to_owned(),
            "message-1".to_owned(),
            "token-0".to_owned()
        )]
    );
    let claimed_messages = repository.claimed_messages().await;
    assert_eq!(claimed_messages.len(), 1);
    assert_eq!(claimed_messages[0].consumer_name, "wallet-balance");
    assert_eq!(claimed_messages[0].message_id, "message-1");
    assert_eq!(
        claimed_messages[0].idempotency_key,
        "convert_order:42:completed"
    );
    assert!(!claimed_messages[0].payload_hash.is_empty());
}

#[tokio::test]
async fn inbox_consumer_skips_duplicate_without_running_handler() {
    let now = Utc.with_ymd_and_hms(2026, 5, 26, 15, 0, 0).unwrap();
    let repository = FakeInboxRepository::with_claims([InboxClaim::Duplicate]).await;
    let handler = FakeInboxHandler::default();
    let service = inbox_service(repository.clone(), handler.clone());

    let result = service
        .consume_one(inbox_message("message-1"), now)
        .await
        .unwrap();

    assert_eq!(result, ConsumedInboxMessage::Duplicate);
    assert!(handler.handled_messages().await.is_empty());
    assert!(repository.consumed_messages().await.is_empty());
    assert!(repository.failed_messages().await.is_empty());
}

#[tokio::test]
async fn inbox_consumer_marks_retry_when_handler_fails_before_threshold() {
    let now = Utc.with_ymd_and_hms(2026, 5, 26, 15, 0, 0).unwrap();
    let repository = FakeInboxRepository::with_claims([claimed(0, "token-0")]).await;
    let handler = FakeInboxHandler::default();
    handler.fail_next("temporary wallet error").await;
    let service = inbox_service(repository.clone(), handler);

    let result = service
        .consume_one(inbox_message("message-1"), now)
        .await
        .unwrap();

    assert_eq!(
        result,
        ConsumedInboxMessage::Retried {
            attempt_count: 1,
            next_retry_at: now + TimeDelta::seconds(10),
        }
    );
    assert_eq!(
        repository.failed_messages().await,
        vec![RecordedFailure {
            consumer_name: "wallet-balance".to_owned(),
            message_id: "message-1".to_owned(),
            processing_token: "token-0".to_owned(),
            decision: InboxRetryDecision::Retry {
                attempt_count: 1,
                next_retry_at: now + TimeDelta::seconds(10),
            },
            error_message: "internal error: temporary wallet error".to_owned(),
        }]
    );
}

#[tokio::test]
async fn inbox_consumer_marks_dead_letter_at_retry_threshold() {
    let now = Utc.with_ymd_and_hms(2026, 5, 26, 15, 0, 0).unwrap();
    let repository = FakeInboxRepository::with_claims([claimed(2, "token-2")]).await;
    let handler = FakeInboxHandler::default();
    handler.fail_next("permanent wallet error").await;
    let service = inbox_service(repository.clone(), handler);

    let result = service
        .consume_one(inbox_message("message-1"), now)
        .await
        .unwrap();

    assert_eq!(
        result,
        ConsumedInboxMessage::DeadLettered { attempt_count: 3 }
    );
    assert_eq!(
        repository.failed_messages().await,
        vec![RecordedFailure {
            consumer_name: "wallet-balance".to_owned(),
            message_id: "message-1".to_owned(),
            processing_token: "token-2".to_owned(),
            decision: InboxRetryDecision::DeadLetter { attempt_count: 3 },
            error_message: "internal error: permanent wallet error".to_owned(),
        }]
    );
}

#[tokio::test]
async fn inbox_retry_scanner_replays_due_retry_rows_with_stored_payload() {
    let now = Utc.with_ymd_and_hms(2026, 5, 26, 15, 0, 0).unwrap();
    let retry = PendingInboxRetry {
        consumer_name: "wallet-balance".to_owned(),
        message_id: "message-1".to_owned(),
        idempotency_key: "convert_order:42:completed".to_owned(),
        payload: json!({ "quote_id": "quote-42" }),
    };
    let repository = FakeInboxRepository::with_pending_retries([retry]).await;
    repository
        .state
        .lock()
        .await
        .claims
        .push_back(claimed(1, "token-1"));
    let handler = FakeInboxHandler::default();
    let service = inbox_service(repository.clone(), handler.clone());

    let batch = service.replay_due_retries(now, 10).await.unwrap();

    assert_eq!(batch.consumed, 1);
    assert_eq!(batch.retried, 0);
    assert_eq!(batch.dead_lettered, 0);
    assert_eq!(
        handler.handled_messages().await,
        vec![inbox_message("message-1")]
    );
    assert_eq!(
        repository.consumed_messages().await,
        vec![(
            "wallet-balance".to_owned(),
            "message-1".to_owned(),
            "token-1".to_owned()
        )]
    );
}

#[tokio::test]
async fn inbox_retry_scanner_records_failure_for_due_retry_rows() {
    let now = Utc.with_ymd_and_hms(2026, 5, 26, 15, 0, 0).unwrap();
    let retry = PendingInboxRetry {
        consumer_name: "wallet-balance".to_owned(),
        message_id: "message-2".to_owned(),
        idempotency_key: "convert_order:42:completed".to_owned(),
        payload: json!({ "idempotency_key": "convert_order:42:completed" }),
    };
    let repository = FakeInboxRepository::with_pending_retries([retry]).await;
    repository
        .state
        .lock()
        .await
        .claims
        .push_back(claimed(2, "token-2"));
    let handler = FakeInboxHandler::default();
    handler.fail_next("temporary wallet error again").await;
    let service = inbox_service(repository.clone(), handler);

    let batch = service.replay_due_retries(now, 10).await.unwrap();

    assert_eq!(batch.consumed, 0);
    assert_eq!(batch.dead_lettered, 1);
    assert_eq!(
        repository.failed_messages().await,
        vec![RecordedFailure {
            consumer_name: "wallet-balance".to_owned(),
            message_id: "message-2".to_owned(),
            processing_token: "token-2".to_owned(),
            decision: InboxRetryDecision::DeadLetter { attempt_count: 3 },
            error_message: "internal error: temporary wallet error again".to_owned(),
        }]
    );
}

#[tokio::test]
async fn inbox_retry_scanner_skips_rows_claimed_by_another_scanner() {
    let now = Utc.with_ymd_and_hms(2026, 5, 26, 15, 0, 0).unwrap();
    let skipped_retry = PendingInboxRetry {
        consumer_name: "wallet-balance".to_owned(),
        message_id: "message-claimed-elsewhere".to_owned(),
        idempotency_key: "convert_order:42:claimed-elsewhere".to_owned(),
        payload: json!({ "idempotency_key": "convert_order:42:claimed-elsewhere" }),
    };
    let consumed_retry = PendingInboxRetry {
        consumer_name: "wallet-balance".to_owned(),
        message_id: "message-available".to_owned(),
        idempotency_key: "convert_order:42:available".to_owned(),
        payload: json!({ "idempotency_key": "convert_order:42:available" }),
    };
    let repository =
        FakeInboxRepository::with_pending_retries([skipped_retry, consumed_retry]).await;
    {
        let mut state = repository.state.lock().await;
        state.claim_failures.push_back(AppError::Internal(
            "event inbox message is already processing".to_owned(),
        ));
        state.claims.push_back(claimed(1, "token-1"));
    }
    let handler = FakeInboxHandler::default();
    let service = inbox_service(repository.clone(), handler.clone());

    let batch = service.replay_due_retries(now, 10).await.unwrap();

    assert_eq!(batch.consumed, 1);
    assert_eq!(batch.duplicates, 1);
    assert_eq!(
        handler.handled_messages().await,
        vec![
            InboundEventMessage::new(
                "message-available",
                "convert_order:42:available",
                json!({ "idempotency_key": "convert_order:42:available" })
            )
            .unwrap()
        ]
    );
    assert_eq!(
        repository.consumed_messages().await,
        vec![(
            "wallet-balance".to_owned(),
            "message-available".to_owned(),
            "token-1".to_owned()
        )]
    );
}

#[tokio::test]
async fn inbox_retry_scanner_rejects_mismatched_consumer_rows() {
    let now = Utc.with_ymd_and_hms(2026, 5, 26, 15, 0, 0).unwrap();
    let retry = PendingInboxRetry {
        consumer_name: "other-consumer".to_owned(),
        message_id: "message-3".to_owned(),
        idempotency_key: "convert_order:42:completed".to_owned(),
        payload: json!({ "idempotency_key": "convert_order:42:completed" }),
    };
    let repository = FakeInboxRepository::with_pending_retries([retry]).await;
    let service = inbox_service(repository, FakeInboxHandler::default());

    let error = service.replay_due_retries(now, 10).await.unwrap_err();

    assert_eq!(
        error.to_string(),
        "internal error: event inbox retry consumer mismatch"
    );
}

#[tokio::test]
async fn mysql_inbox_retry_scanner_fetches_only_due_rows_with_stored_payload() {
    let Some(pool) = mysql_pool().await else {
        return;
    };
    let repository = MySqlEventInboxRepository::new(pool.clone());
    let suffix = unique_suffix();
    let now = Utc.with_ymd_and_hms(2026, 5, 26, 15, 0, 0).unwrap();
    let consumer = format!("wallet.retry.{suffix}");
    let other_consumer = format!("wallet.other.{suffix}");
    let due_key = format!("convert_order:{suffix}:due");
    let null_due_key = format!("convert_order:{suffix}:null-due");
    let stale_processing_key = format!("convert_order:{suffix}:stale-processing");
    let fresh_processing_key = format!("convert_order:{suffix}:fresh-processing");
    let future_key = format!("convert_order:{suffix}:future");
    let consumed_key = format!("convert_order:{suffix}:consumed");
    let other_key = format!("convert_order:{suffix}:other");
    let due_payload = retry_payload(&due_key, "due-order");
    let null_due_payload = retry_payload(&null_due_key, "null-due-order");
    let stale_processing_payload = retry_payload(&stale_processing_key, "stale-processing-order");

    for (
        message_id,
        idempotency_key,
        payload,
        status,
        next_retry_at,
        updated_at,
        target_consumer,
    ) in [
        (
            format!("message-due-{suffix}"),
            due_key.clone(),
            due_payload.clone(),
            "retry",
            Some((now - TimeDelta::seconds(1)).naive_utc()),
            now.naive_utc(),
            consumer.clone(),
        ),
        (
            format!("message-null-due-{suffix}"),
            null_due_key.clone(),
            null_due_payload.clone(),
            "retry",
            None,
            now.naive_utc(),
            consumer.clone(),
        ),
        (
            format!("message-stale-processing-{suffix}"),
            stale_processing_key.clone(),
            stale_processing_payload.clone(),
            "processing",
            None,
            (now - TimeDelta::seconds(301)).naive_utc(),
            consumer.clone(),
        ),
        (
            format!("message-fresh-processing-{suffix}"),
            fresh_processing_key,
            retry_payload("fresh-processing-key", "fresh-processing-order"),
            "processing",
            None,
            now.naive_utc(),
            consumer.clone(),
        ),
        (
            format!("message-future-{suffix}"),
            future_key,
            retry_payload("future-key", "future-order"),
            "retry",
            Some((now + TimeDelta::seconds(60)).naive_utc()),
            now.naive_utc(),
            consumer.clone(),
        ),
        (
            format!("message-consumed-{suffix}"),
            consumed_key,
            retry_payload("consumed-key", "consumed-order"),
            "consumed",
            Some((now - TimeDelta::seconds(1)).naive_utc()),
            now.naive_utc(),
            consumer.clone(),
        ),
        (
            format!("message-other-{suffix}"),
            other_key,
            retry_payload("other-key", "other-order"),
            "retry",
            Some((now - TimeDelta::seconds(1)).naive_utc()),
            now.naive_utc(),
            other_consumer,
        ),
    ] {
        // 在真实 MySQL 表中写入不同状态/时间/consumer 的行，验证 scanner 只领取当前 consumer 已到期重试。
        sqlx::query(
            r#"INSERT INTO event_inbox
               (consumer_name, message_id, idempotency_key, payload_hash, payload_json, status, retry_count, next_retry_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?)"#,
        )
        .bind(target_consumer)
        .bind(message_id)
        .bind(idempotency_key)
        .bind(format!("hash-{suffix}"))
        .bind(SqlxJson(payload))
        .bind(status)
        .bind(next_retry_at)
        .bind(updated_at)
        .execute(&pool)
        .await
        .unwrap();
    }

    let retries = repository
        .fetch_due_retries(&consumer, 10, now)
        .await
        .unwrap();

    assert_eq!(retries.len(), 3);
    assert!(retries.iter().any(|retry| {
        retry.idempotency_key == stale_processing_key && retry.payload == stale_processing_payload
    }));
    assert!(retries
        .iter()
        .any(|retry| retry.idempotency_key == null_due_key && retry.payload == null_due_payload));
    assert!(
        retries
            .iter()
            .any(|retry| retry.idempotency_key == due_key && retry.payload == due_payload)
    );
}

#[tokio::test]
async fn mysql_inbox_completion_rejects_stale_processing_token() {
    let Some(pool) = mysql_pool().await else {
        return;
    };
    if !event_inbox_external_dependencies_are_available() {
        eprintln!(
            "skipping MySQL event inbox integration test because REDIS_URL or MONGO_URL is not set"
        );
        return;
    }
    let repository = MySqlEventInboxRepository::new(pool.clone());
    let suffix = unique_suffix();
    let consumer = format!("wallet.fence.{suffix}");
    let message_id = format!("message-fence-{suffix}");
    let idempotency_key = format!("convert_order:{suffix}:fence");
    let payload = retry_payload(&idempotency_key, "fence-order");
    let stale_updated_at = (Utc::now() - TimeDelta::seconds(301)).naive_utc();

    sqlx::query(
        r#"INSERT INTO event_inbox
           (consumer_name, message_id, idempotency_key, payload_hash, payload_json, status, retry_count, updated_at)
           VALUES (?, ?, ?, ?, ?, 'processing', 1, ?)"#,
    )
    .bind(&consumer)
    .bind(&message_id)
    .bind(&idempotency_key)
    .bind(format!("hash-{suffix}"))
    .bind(SqlxJson(payload.clone()))
    .bind(stale_updated_at)
    .execute(&pool)
    .await
    .unwrap();

    let claim = repository
        .claim_message(NewInboxMessage::new(
            consumer.clone(),
            message_id.clone(),
            idempotency_key,
            format!("hash-reclaim-{suffix}"),
            payload,
        ))
        .await
        .unwrap();
    let processing_token = match claim {
        InboxClaim::Claimed {
            attempt_count,
            processing_token,
        } => {
            assert_eq!(attempt_count, 1);
            processing_token
        }
        InboxClaim::Duplicate => panic!("stale processing row should be reclaimable"),
    };

    let stale_error = repository
        .mark_consumed(
            &consumer,
            &message_id,
            &mysql_processing_token(stale_updated_at),
        )
        .await
        .unwrap_err();
    assert_eq!(
        stale_error.to_string(),
        "internal error: event inbox processing token is stale"
    );
    let stale_failure = repository
        .mark_failure(
            &consumer,
            &message_id,
            &mysql_processing_token(stale_updated_at),
            InboxRetryDecision::Retry {
                attempt_count: 2,
                next_retry_at: Utc::now() + TimeDelta::seconds(10),
            },
            "old worker failure must not overwrite the new claim",
            Utc::now(),
        )
        .await
        .unwrap_err();
    assert_eq!(
        stale_failure.to_string(),
        "internal error: event inbox processing token is stale"
    );

    repository
        .mark_consumed(&consumer, &message_id, &processing_token)
        .await
        .unwrap();

    let (status, consumed_at) = sqlx::query_as::<_, (String, Option<chrono::NaiveDateTime>)>(
        "SELECT status, CAST(consumed_at AS DATETIME(6)) FROM event_inbox WHERE consumer_name = ? AND message_id = ?",
    )
    .bind(&consumer)
    .bind(&message_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "consumed");
    assert!(consumed_at.is_some());
}

#[test]
fn event_inbox_payload_backfill_migration_marks_missing_outbox_rows() {
    let migration = include_str!("../migrations/0012_event_inbox_payload_json.sql");

    assert!(migration.contains("WHEN outbox.id IS NOT NULL THEN JSON_OBJECT"));
    assert!(migration.contains("legacy_missing_payload"));
    assert!(!migration.contains("COALESCE(\n        JSON_OBJECT"));
}

#[test]
fn event_inbox_follow_up_migration_dead_letters_missing_processing_payloads() {
    let migration =
        include_str!("../migrations/0013_event_inbox_missing_payload_processing_dead_letter.sql");

    assert!(migration.contains("status = 'dead_letter'"));
    assert!(migration.contains("WHERE status = 'processing'"));
    assert!(migration.contains("legacy_missing_payload"));
}

#[tokio::test]
async fn inbox_consumer_batch_exposes_metrics_snapshot_and_alerts() {
    let now = Utc.with_ymd_and_hms(2026, 5, 26, 15, 0, 0).unwrap();
    let repository = FakeInboxRepository::with_claims([
        InboxClaim::Duplicate,
        claimed(0, "token-0"),
        claimed(2, "token-2"),
        claimed(0, "token-0"),
    ])
    .await;
    let handler = FakeInboxHandler::default();
    handler.fail_next("temporary wallet error").await;
    handler.fail_next("permanent wallet error").await;
    let service = inbox_service(repository, handler);

    let batch = service
        .consume_batch(
            vec![
                inbox_message("message-1"),
                inbox_message("message-2"),
                inbox_message("message-3"),
                inbox_message("message-4"),
            ],
            now,
        )
        .await
        .unwrap();

    let metrics = batch.metrics();
    assert_eq!(metrics.total, 4);
    assert_eq!(metrics.consumed, 1);
    assert_eq!(metrics.duplicates, 1);
    assert_eq!(metrics.retried, 1);
    assert_eq!(metrics.dead_lettered, 1);
    assert_eq!(metrics.alerts.len(), 2);
    assert_eq!(
        metrics.alerts,
        vec![
            EventInboxAlert {
                kind: EventInboxAlertKind::RetryBacklog,
                severity: EventInboxAlertSeverity::Warning,
                count: 1,
                message: "事件 inbox 存在待重试消息".to_owned(),
            },
            EventInboxAlert {
                kind: EventInboxAlertKind::DeadLetter,
                severity: EventInboxAlertSeverity::Critical,
                count: 1,
                message: "事件 inbox 存在死信消息".to_owned(),
            },
        ]
    );
}

#[test]
fn inbox_alert_classifies_delivery_errors_for_operations() {
    assert_eq!(
        EventInboxAlert::from_processed_delivery(&ProcessedInboxDelivery::from_result(Err(
            AppError::Internal("mysql unavailable".to_owned())
        ))),
        Some(EventInboxAlert {
            kind: EventInboxAlertKind::ProcessingError,
            severity: EventInboxAlertSeverity::Critical,
            count: 1,
            message: "事件 inbox 投递处理失败，将重新入队".to_owned(),
        })
    );
    assert_eq!(
        EventInboxAlert::from_processed_delivery(&ProcessedInboxDelivery::from_result(Err(
            AppError::Validation("invalid event payload json: expected value".to_owned())
        ))),
        Some(EventInboxAlert {
            kind: EventInboxAlertKind::MalformedDelivery,
            severity: EventInboxAlertSeverity::Warning,
            count: 1,
            message: "事件 inbox 投递格式异常，已确认跳过".to_owned(),
        })
    );
    assert_eq!(
        EventInboxAlert::from_processed_delivery(&ProcessedInboxDelivery::from_result(Ok(
            ConsumedInboxMessage::Consumed
        ))),
        None
    );
}

#[test]
fn processed_delivery_suppresses_outer_error_for_malformed_ack() {
    let processed = ProcessedInboxDelivery::from_result(Err(AppError::Validation(
        "event idempotency_key is required".to_owned(),
    )));

    assert_eq!(processed.disposition, InboxDeliveryDisposition::Ack);
    assert!(processed.result.is_ok());
    assert_eq!(
        processed.alert,
        Some(EventInboxAlert {
            kind: EventInboxAlertKind::MalformedDelivery,
            severity: EventInboxAlertSeverity::Warning,
            count: 1,
            message: "事件 inbox 投递格式异常，已确认跳过".to_owned(),
        })
    );
}

#[test]
fn processed_delivery_acks_recorded_retry_until_due_scanner_replays_it() {
    let next_retry_at = Utc.with_ymd_and_hms(2026, 5, 26, 15, 0, 10).unwrap();
    let processed = ProcessedInboxDelivery::from_result(Ok(ConsumedInboxMessage::Retried {
        attempt_count: 1,
        next_retry_at,
    }));

    assert_eq!(processed.disposition, InboxDeliveryDisposition::Ack);
    assert!(processed.result.is_ok());
    assert_eq!(
        processed.alert,
        Some(EventInboxAlert {
            kind: EventInboxAlertKind::RetryBacklog,
            severity: EventInboxAlertSeverity::Warning,
            count: 1,
            message: "事件 inbox 存在待重试消息".to_owned(),
        })
    );
}

#[test]
fn delivery_disposition_acks_recorded_retry_and_malformed_but_requeues_internal_errors() {
    assert_eq!(
        InboxDeliveryDisposition::from_result(&Ok(ConsumedInboxMessage::Consumed)),
        InboxDeliveryDisposition::Ack
    );
    assert_eq!(
        InboxDeliveryDisposition::from_result(&Ok(ConsumedInboxMessage::Duplicate)),
        InboxDeliveryDisposition::Ack
    );
    assert_eq!(
        InboxDeliveryDisposition::from_result(&Ok(ConsumedInboxMessage::DeadLettered {
            attempt_count: 3,
        })),
        InboxDeliveryDisposition::Ack
    );
    assert_eq!(
        InboxDeliveryDisposition::from_result(&Ok(ConsumedInboxMessage::Retried {
            attempt_count: 1,
            next_retry_at: Utc.with_ymd_and_hms(2026, 5, 26, 15, 0, 10).unwrap(),
        })),
        InboxDeliveryDisposition::Ack
    );
    assert_eq!(
        InboxDeliveryDisposition::from_result(&Err(AppError::Internal(
            "mysql unavailable".to_owned()
        ))),
        InboxDeliveryDisposition::RejectRequeue
    );
    assert_eq!(
        InboxDeliveryDisposition::from_result(&Err(AppError::Validation(
            "invalid event payload json: expected value".to_owned()
        ))),
        InboxDeliveryDisposition::Ack
    );
    assert_eq!(
        InboxDeliveryDisposition::from_result(&Err(AppError::Validation(
            "event idempotency_key is required".to_owned()
        ))),
        InboxDeliveryDisposition::Ack
    );
}

#[test]
fn inbound_delivery_requires_payload_idempotency_key() {
    let message = InboundEventMessage::from_delivery(&delivery_with_message_id(
        r#"{"payload":{"quote_id":"quote-42"}}"#,
        Some("message-1"),
    ));

    assert_eq!(
        message.unwrap_err().to_string(),
        "validation error: event idempotency_key is required"
    );
}

#[test]
fn event_inbox_worker_config_stays_disabled_without_queue_name() {
    let config = EventInboxWorkerConfig::from_env_values(None, None).unwrap();

    assert!(config.is_disabled());
    assert!(config.startup().is_none());
}

#[test]
fn event_inbox_worker_config_treats_blank_queue_as_disabled() {
    let config = EventInboxWorkerConfig::from_env_values(Some("  "), Some("bad tag")).unwrap();

    assert!(config.is_disabled());
    assert!(config.startup().is_none());
}

#[test]
fn event_inbox_worker_config_trims_queue_and_defaults_consumer_tag() {
    let config =
        EventInboxWorkerConfig::from_env_values(Some(" exchange.events.wallet "), Some("  "))
            .unwrap();
    let startup = config.startup().unwrap();

    assert_eq!(startup.queue_name(), "exchange.events.wallet");
    assert_eq!(startup.consumer_tag(), "exchange-api-inbox");
}

#[test]
fn event_inbox_worker_config_schedules_retry_scanner() {
    let config =
        EventInboxWorkerConfig::from_env_values(Some(" exchange.events.wallet "), Some("worker-1"))
            .unwrap();
    let startup = config.startup().unwrap();

    assert_eq!(startup.retry_scan_seconds(0), 10);
    assert_eq!(startup.retry_scan_seconds(4), 4);
}

#[test]
fn event_inbox_worker_config_rejects_invalid_enabled_values() {
    assert!(EventInboxWorkerConfig::from_env_values(Some("bad queue"), None).is_err());
    assert!(EventInboxWorkerConfig::from_env_values(Some("queue"), Some("bad tag")).is_err());
    assert!(EventInboxWorkerConfig::from_env_values(Some(&"q".repeat(129)), None).is_err());
}

#[test]
fn event_inbox_reconnect_backoff_caps_and_resets() {
    let mut backoff = EventInboxReconnectBackoff::new(5);

    assert_eq!(backoff.next_delay_seconds(), 5);
    assert_eq!(backoff.record_failure(), 5);
    assert_eq!(backoff.next_delay_seconds(), 10);
    assert_eq!(backoff.record_failure(), 10);
    assert_eq!(backoff.next_delay_seconds(), 20);
    assert_eq!(backoff.record_failure(), 20);
    assert_eq!(backoff.next_delay_seconds(), 40);
    assert_eq!(backoff.record_failure(), 40);
    assert_eq!(backoff.next_delay_seconds(), 60);
    backoff.record_success();
    assert_eq!(backoff.next_delay_seconds(), 5);
}

#[test]
fn event_inbox_supervision_reconnects_after_each_ended_cycle() {
    let mut backoff = EventInboxReconnectBackoff::new(5);

    let first_delay = backoff.record_cycle_outcome(EventInboxConsumerCycleOutcome::Ended);
    let second_delay = backoff.record_cycle_outcome(EventInboxConsumerCycleOutcome::Ended);

    assert_eq!(first_delay, 5);
    assert_eq!(second_delay, 10);
    assert_eq!(backoff.next_delay_seconds(), 20);
}

#[test]
fn event_inbox_supervision_reconnects_after_each_failed_cycle() {
    let mut backoff = EventInboxReconnectBackoff::new(5);

    let first_delay = backoff.record_cycle_outcome(EventInboxConsumerCycleOutcome::Failed);
    let second_delay = backoff.record_cycle_outcome(EventInboxConsumerCycleOutcome::Failed);

    assert_eq!(first_delay, 5);
    assert_eq!(second_delay, 10);
    assert_eq!(backoff.next_delay_seconds(), 20);
}

#[tokio::test]
async fn production_inbox_handler_accepts_known_domain_event_envelope() {
    let handler = EventInboxProductionHandler;
    let message = InboundEventMessage::new(
        "message-1",
        "convert_order:42:confirmed",
        json!({
            "aggregate_type": "convert_order",
            "aggregate_id": "42",
            "event_type": "confirmed",
            "routing_key": "convert.order.confirmed",
            "idempotency_key": "convert_order:42:confirmed",
            "payload": { "quote_id": "quote-42" }
        }),
    )
    .unwrap();

    handler.handle(&message).await.unwrap();
    assert_eq!(
        ProductionEventDispatch::from_inbound(&message)
            .unwrap()
            .dispatch_key(),
        "convert_order.confirmed"
    );
}

#[tokio::test]
async fn production_inbox_handler_accepts_market_feed_event_envelope() {
    let handler = EventInboxProductionHandler;
    let message = InboundEventMessage::new(
        "market_feed:bitget:BTCUSDT:ticker:1710000000000",
        "market_feed:bitget:BTCUSDT:ticker:1710000000000",
        json!({
            "aggregate_type": "market_ticker",
            "aggregate_id": "BTCUSDT",
            "event_type": "ticker_updated",
            "routing_key": "market.BTCUSDT.ticker",
            "idempotency_key": "market_feed:bitget:BTCUSDT:ticker:1710000000000",
            "payload": { "symbol": "BTCUSDT" }
        }),
    )
    .unwrap();

    handler.handle(&message).await.unwrap();
    assert_eq!(
        ProductionEventDispatch::from_inbound(&message)
            .unwrap()
            .dispatch_key(),
        "market_ticker.ticker_updated"
    );
}

#[tokio::test]
async fn production_inbox_handler_rejects_unknown_domain_event_envelope() {
    let handler = EventInboxProductionHandler;
    let message = InboundEventMessage::new(
        "message-1",
        "unknown:42:created",
        json!({
            "aggregate_type": "unknown",
            "aggregate_id": "42",
            "event_type": "created",
            "routing_key": "unknown.created",
            "idempotency_key": "unknown:42:created",
            "payload": {}
        }),
    )
    .unwrap();

    assert!(handler.handle(&message).await.is_err());
}

#[tokio::test]
async fn production_inbox_handler_rejects_malformed_domain_event_envelope() {
    let handler = EventInboxProductionHandler;
    let message = InboundEventMessage::new(
        "message-1",
        "convert_order:42:confirmed",
        json!({
            "aggregate_type": "convert_order",
            "aggregate_id": "42",
            "event_type": "confirmed"
        }),
    )
    .unwrap();

    assert!(handler.handle(&message).await.is_err());
}

#[tokio::test]
async fn production_inbox_handler_rejects_idempotency_mismatch() {
    let handler = EventInboxProductionHandler;
    let message = InboundEventMessage::new(
        "message-1",
        "convert_order:42:confirmed",
        json!({
            "aggregate_type": "convert_order",
            "aggregate_id": "42",
            "event_type": "confirmed",
            "routing_key": "convert.order.confirmed",
            "idempotency_key": "convert_order:42:completed",
            "payload": { "quote_id": "quote-42" }
        }),
    )
    .unwrap();

    assert!(handler.handle(&message).await.is_err());
}

#[tokio::test]
async fn production_inbox_handler_rejects_routing_mismatch() {
    let handler = EventInboxProductionHandler;
    let message = InboundEventMessage::new(
        "message-1",
        "convert_order:42:confirmed",
        json!({
            "aggregate_type": "convert_order",
            "aggregate_id": "42",
            "event_type": "confirmed",
            "routing_key": "convert.order.completed",
            "idempotency_key": "convert_order:42:confirmed",
            "payload": { "quote_id": "quote-42" }
        }),
    )
    .unwrap();

    assert!(handler.handle(&message).await.is_err());
}

fn delivery_with_message_id(payload: &str, message_id: Option<&str>) -> Delivery {
    let properties = message_id
        .map(|value| BasicProperties::default().with_message_id(value.into()))
        .unwrap_or_default();

    Delivery {
        delivery_tag: 1,
        exchange: ShortString::from("exchange.events"),
        routing_key: ShortString::from("event.test"),
        redelivered: false,
        properties,
        data: payload.as_bytes().to_vec(),
        acker: Acker::default(),
    }
}

fn inbox_service(
    repository: FakeInboxRepository,
    handler: FakeInboxHandler,
) -> EventInboxConsumerService<FakeInboxRepository, FakeInboxHandler> {
    EventInboxConsumerService::new(
        "wallet-balance",
        repository,
        handler,
        InboxRetryPolicy::new(3, TimeDelta::seconds(10)).unwrap(),
    )
}

fn inbox_message(message_id: &str) -> InboundEventMessage {
    InboundEventMessage::new(
        message_id,
        "convert_order:42:completed",
        json!({ "quote_id": "quote-42" }),
    )
    .unwrap()
}
