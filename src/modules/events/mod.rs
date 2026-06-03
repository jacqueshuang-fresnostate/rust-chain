use crate::time::unix_millis;
use crate::{
    config::Settings,
    error::{AppError, AppResult},
    modules::{
        auth::{TokenScope, decode_claims},
        market::adapters::MarketFeedEvent,
    },
    state::AppState,
};
use axum::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use futures_util::StreamExt;
use lapin::{
    BasicProperties, Channel, ExchangeKind,
    message::Delivery,
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicPublishOptions, BasicRejectOptions,
        ExchangeDeclareOptions,
    },
    types::FieldTable,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Error as SqlxError, MySql, Pool, error::DatabaseError, types::Json as SqlxJson};
use std::{collections::hash_map::DefaultHasher, hash::Hasher, sync::Arc};
use thiserror::Error;
use tokio::sync::broadcast::{self, error::RecvError};
use uuid::Uuid;

pub mod routes;

const OUTBOX_PENDING: &str = "pending";
const OUTBOX_PUBLISHED: &str = "published";
const OUTBOX_RETRY: &str = "retry";
const OUTBOX_DEAD_LETTER: &str = "dead_letter";
const INBOX_PROCESSING: &str = "processing";
const INBOX_CONSUMED: &str = "consumed";
const INBOX_RETRY: &str = "retry";
const INBOX_DEAD_LETTER: &str = "dead_letter";
const INBOX_PROCESSING_LEASE_SECONDS: i64 = 300;
const INBOX_PROCESSING_TOKEN_FORMAT: &str = "%Y-%m-%d %H:%M:%S%.6f";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WebSocketChannel {
    pub namespace: String,
    pub topic: String,
}

impl WebSocketChannel {
    pub fn public(namespace: impl Into<String>, topic: impl Into<String>) -> AppResult<Self> {
        let namespace = validate_ws_segment(namespace.into(), "websocket namespace")?;
        let topic = validate_ws_segment(topic.into(), "websocket topic")?;
        Ok(Self { namespace, topic })
    }

    pub fn private_user(user_id: u64) -> Self {
        Self {
            namespace: "private".to_owned(),
            topic: format!("user:{user_id}"),
        }
    }

    pub fn as_text(&self) -> String {
        if self.namespace == "private" {
            return format!("private:{}", self.topic);
        }
        format!("public:{}:{}", self.namespace, self.topic)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateWsAuth {
    pub user_id: u64,
}

impl PrivateWsAuth {
    pub fn from_query(query: Option<&str>, settings: &Settings) -> AppResult<Self> {
        let token = query
            .and_then(|query| {
                query.split('&').find_map(|pair| {
                    let (key, value) = pair.split_once('=')?;
                    (key == "token" && !value.is_empty()).then_some(value)
                })
            })
            .ok_or(AppError::Unauthorized)?;
        let claims = decode_claims(settings, token)?;
        if claims.scope != TokenScope::User {
            return Err(AppError::Forbidden);
        }
        let user_id = claims
            .sub
            .strip_prefix("user:")
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or(AppError::Unauthorized)?;

        Ok(Self { user_id })
    }
}

fn validate_ws_segment(value: String, field: &str) -> AppResult<String> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(AppError::Validation(format!("invalid {field}")));
    }

    Ok(value)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventBroadcastMessage {
    channel: WebSocketChannel,
    payload: String,
}

impl EventBroadcastMessage {
    pub fn public(channel: WebSocketChannel, payload: impl Into<String>) -> Self {
        Self {
            channel,
            payload: payload.into(),
        }
    }

    pub fn private_user(user_id: u64, payload: impl Into<String>) -> Self {
        Self {
            channel: WebSocketChannel::private_user(user_id),
            payload: payload.into(),
        }
    }

    pub fn from_market_feed_event(event: &MarketFeedEvent) -> AppResult<Self> {
        Ok(Self::public(
            WebSocketChannel::public(event.public_ws_namespace(), event.public_ws_topic())?,
            event.payload().to_string(),
        ))
    }

    pub fn channel(&self) -> &WebSocketChannel {
        &self.channel
    }

    pub fn payload(&self) -> &str {
        &self.payload
    }
}

#[derive(Clone)]
pub struct EventBroadcastHub {
    sender: broadcast::Sender<EventBroadcastMessage>,
}

impl EventBroadcastHub {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity.max(1));
        Self { sender }
    }

    pub fn subscribe(&self, channel: &WebSocketChannel) -> EventBroadcastSubscription {
        EventBroadcastSubscription {
            channel: channel.clone(),
            receiver: self.sender.subscribe(),
        }
    }

    pub fn subscribe_multi(&self) -> EventBroadcastMultiSubscription {
        EventBroadcastMultiSubscription {
            receiver: self.sender.subscribe(),
        }
    }

    pub fn publish(&self, message: EventBroadcastMessage) {
        let _ = self.sender.send(message);
    }
}

pub struct EventBroadcastMultiSubscription {
    receiver: broadcast::Receiver<EventBroadcastMessage>,
}

impl EventBroadcastMultiSubscription {
    pub async fn recv(&mut self) -> AppResult<EventBroadcastMessage> {
        loop {
            match self.receiver.recv().await {
                Ok(message) => return Ok(message),
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => {
                    return Err(AppError::Internal(
                        "event broadcast channel is closed".to_owned(),
                    ));
                }
            }
        }
    }
}

pub struct EventBroadcastSubscription {
    channel: WebSocketChannel,
    receiver: broadcast::Receiver<EventBroadcastMessage>,
}

impl EventBroadcastSubscription {
    pub async fn recv(&mut self) -> AppResult<EventBroadcastMessage> {
        loop {
            match self.receiver.recv().await {
                Ok(message) if message.channel() == &self.channel => return Ok(message),
                Ok(_) | Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => {
                    return Err(AppError::Internal(
                        "event broadcast channel is closed".to_owned(),
                    ));
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEvent {
    pub id: Uuid,
    pub exchange: String,
    pub routing_key: String,
    pub idempotency_key: String,
    pub payload: Value,
    #[serde(with = "unix_millis")]
    pub created_at: DateTime<Utc>,
}

impl DomainEvent {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRoute {
    pub exchange: String,
    pub routing_key: String,
}

impl EventRoute {
    pub fn new(exchange: impl Into<String>, routing_key: impl Into<String>) -> Self {
        Self {
            exchange: exchange.into(),
            routing_key: routing_key.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventIdempotency {
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub event_type: String,
}

impl EventIdempotency {
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

    pub fn into_key(self) -> String {
        format!(
            "{}:{}:{}",
            self.aggregate_type, self.aggregate_id, self.event_type
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxIdempotency {
    pub consumer_name: String,
    pub message_id: String,
    pub idempotency_key: String,
}

impl InboxIdempotency {
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

    pub fn consumer_message_key(&self) -> String {
        format!("{}:{}", self.consumer_name, self.message_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryMetadata {
    max_attempts: u32,
    attempt_count: u32,
    backoff: TimeDelta,
    next_attempt_at: Option<DateTime<Utc>>,
}

impl RetryMetadata {
    pub fn new(max_attempts: u32, backoff: TimeDelta) -> Result<Self, RetryMetadataError> {
        if max_attempts == 0 {
            return Err(RetryMetadataError::InvalidMaxAttempts);
        }
        if backoff <= TimeDelta::zero() {
            return Err(RetryMetadataError::InvalidBackoff);
        }

        Ok(Self {
            max_attempts,
            attempt_count: 0,
            backoff,
            next_attempt_at: None,
        })
    }

    pub fn record_failure(&self, failed_at: DateTime<Utc>) -> Result<Self, RetryMetadataError> {
        let attempt_count = self
            .attempt_count
            .checked_add(1)
            .ok_or(RetryMetadataError::AttemptOverflow)?;

        Ok(Self {
            max_attempts: self.max_attempts,
            attempt_count,
            backoff: self.backoff,
            next_attempt_at: Some(failed_at + self.backoff),
        })
    }

    pub fn attempt_count(&self) -> u32 {
        self.attempt_count
    }

    pub fn next_attempt_at(&self) -> DateTime<Utc> {
        self.next_attempt_at
            .expect("next_attempt_at is set after a recorded failure")
    }

    pub fn should_dead_letter(&self) -> bool {
        self.attempt_count >= self.max_attempts
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RetryMetadataError {
    #[error("retry max attempts must be positive")]
    InvalidMaxAttempts,
    #[error("retry backoff must be positive")]
    InvalidBackoff,
    #[error("retry attempt counter overflowed")]
    AttemptOverflow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewOutboxEvent {
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub event_type: String,
    pub routing_key: String,
    pub idempotency_key: String,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

impl NewOutboxEvent {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutboxInsertResult {
    Inserted { id: u64 },
    Duplicate { id: u64 },
}

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
        .bind(SqlxJson(event.payload))
        .bind(OUTBOX_PENDING)
        .bind(event.created_at.naive_utc())
        .execute(&self.pool)
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
        .fetch_one(&self.pool)
        .await?
        .0;

        Ok(OutboxInsertResult::Duplicate { id })
    }

    async fn fetch_publishable_batch(
        &self,
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
        .fetch_all(&self.pool)
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

    async fn mark_published(&self, id: u64, published_at: DateTime<Utc>) -> AppResult<()> {
        sqlx::query(
            "UPDATE event_outbox SET status = ?, published_at = ?, updated_at = ? WHERE id = ?",
        )
        .bind(OUTBOX_PUBLISHED)
        .bind(published_at.naive_utc())
        .bind(published_at.naive_utc())
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn mark_retry(
        &self,
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
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn mark_dead_letter(
        &self,
        id: u64,
        retry_count: u32,
        failed_at: DateTime<Utc>,
    ) -> AppResult<()> {
        sqlx::query(
            "UPDATE event_outbox SET status = ?, retry_count = ?, updated_at = ? WHERE id = ?",
        )
        .bind(OUTBOX_DEAD_LETTER)
        .bind(i32::try_from(retry_count).unwrap_or(i32::MAX))
        .bind(failed_at.naive_utc())
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RabbitMqPublishEnvelope {
    pub exchange: String,
    pub routing_key: String,
    pub message_id: String,
    pub content_type: String,
    pub payload: Vec<u8>,
}

impl RabbitMqPublishEnvelope {
    pub fn from_outbox(exchange: impl Into<String>, outbox: &OutboxMessage) -> AppResult<Self> {
        let payload = serde_json::json!({
            "aggregate_type": outbox.aggregate_type,
            "aggregate_id": outbox.aggregate_id,
            "event_type": outbox.event_type,
            "routing_key": outbox.routing_key,
            "idempotency_key": outbox.idempotency_key,
            "payload": outbox.payload,
        });

        Ok(Self {
            exchange: exchange.into(),
            routing_key: outbox.routing_key.clone(),
            message_id: outbox.idempotency_key.clone(),
            content_type: "application/json".to_owned(),
            payload: serde_json::to_vec(&payload).map_err(|error| {
                AppError::Internal(format!("failed to serialize outbox payload: {error}"))
            })?,
        })
    }

    fn properties(&self) -> BasicProperties {
        BasicProperties::default()
            .with_message_id(self.message_id.clone().into())
            .with_content_type(self.content_type.clone().into())
            .with_delivery_mode(2)
    }
}

#[async_trait]
pub trait OutboxPublisher: Clone + Send + Sync + 'static {
    async fn publish(&self, message: &OutboxMessage) -> AppResult<()>;
}

#[derive(Clone)]
pub struct RabbitMqOutboxPublisher {
    connection: Arc<lapin::Connection>,
    exchange: String,
}

impl RabbitMqOutboxPublisher {
    pub fn new(connection: Arc<lapin::Connection>, exchange: impl Into<String>) -> Self {
        Self {
            connection,
            exchange: exchange.into(),
        }
    }
}

#[async_trait]
impl OutboxPublisher for RabbitMqOutboxPublisher {
    async fn publish(&self, message: &OutboxMessage) -> AppResult<()> {
        let envelope = RabbitMqPublishEnvelope::from_outbox(&self.exchange, message)?;
        let channel = self.connection.create_channel().await?;
        channel
            .exchange_declare(
                &envelope.exchange,
                ExchangeKind::Topic,
                ExchangeDeclareOptions {
                    durable: true,
                    ..ExchangeDeclareOptions::default()
                },
                FieldTable::default(),
            )
            .await?;
        channel
            .basic_publish(
                &envelope.exchange,
                &envelope.routing_key,
                BasicPublishOptions::default(),
                &envelope.payload,
                envelope.properties(),
            )
            .await?
            .await?;

        Ok(())
    }
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
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
}

impl<R> EventOutboxWriter<R>
where
    R: EventOutboxRepository,
{
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

impl EventOutboxService<MySqlEventOutboxRepository, RabbitMqOutboxPublisher> {
    pub fn from_state(state: &AppState) -> AppResult<Self> {
        Self::from_state_with_batch_size(state, 100)
    }

    pub fn from_state_with_batch_size(state: &AppState, batch_size: u32) -> AppResult<Self> {
        let pool = state.mysql.clone().ok_or_else(|| {
            AppError::Internal(
                "mysql pool is not configured for event outbox persistence".to_owned(),
            )
        })?;
        let rabbitmq = state.rabbitmq.clone().ok_or_else(|| {
            AppError::Internal(
                "rabbitmq connection is not configured for event outbox publisher".to_owned(),
            )
        })?;
        let retry_policy = InboxRetryPolicy::new(5, TimeDelta::seconds(30)).map_err(|error| {
            AppError::Internal(format!("invalid event outbox retry policy: {error}"))
        })?;

        Ok(Self::new(
            MySqlEventOutboxRepository::new(pool),
            RabbitMqOutboxPublisher::new(rabbitmq, "exchange.events"),
            retry_policy,
            batch_size,
        ))
    }
}

impl<R, P> EventOutboxService<R, P>
where
    R: EventOutboxRepository,
    P: OutboxPublisher,
{
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewInboxMessage {
    pub consumer_name: String,
    pub message_id: String,
    pub idempotency_key: String,
    pub payload_hash: String,
    pub payload: Value,
}

impl NewInboxMessage {
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

#[derive(Debug, Clone)]
pub struct MySqlEventInboxRepository {
    pool: Pool<MySql>,
}

impl MySqlEventInboxRepository {
    pub fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }
}

fn is_unique_violation(error: &SqlxError) -> bool {
    error
        .as_database_error()
        .and_then(DatabaseError::code)
        .as_deref()
        == Some("1062")
}

fn retry_is_not_due(next_retry_at: Option<chrono::NaiveDateTime>) -> bool {
    next_retry_at.is_some_and(|value| value.and_utc() > Utc::now())
}

fn processing_is_stale(updated_at: chrono::NaiveDateTime) -> bool {
    updated_at.and_utc() + TimeDelta::seconds(INBOX_PROCESSING_LEASE_SECONDS) <= Utc::now()
}

fn processing_token(value: chrono::NaiveDateTime) -> String {
    value.format(INBOX_PROCESSING_TOKEN_FORMAT).to_string()
}

fn parse_processing_token(value: &str) -> AppResult<chrono::NaiveDateTime> {
    chrono::NaiveDateTime::parse_from_str(value, INBOX_PROCESSING_TOKEN_FORMAT)
        .map_err(|_| processing_token_is_stale_error())
}

fn processing_token_is_stale_error() -> AppError {
    AppError::Internal("event inbox processing token is stale".to_owned())
}

fn inbox_message_is_already_processing(error: &AppError) -> bool {
    matches!(error, AppError::Internal(message) if message == "event inbox message is already processing")
}

#[derive(Debug, Clone)]
struct ExistingInboxMessage {
    status: String,
    retry_count: i32,
    message_id: String,
    next_retry_at: Option<chrono::NaiveDateTime>,
    updated_at: chrono::NaiveDateTime,
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

fn decide_existing_inbox_claim(
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

#[async_trait]
impl EventInboxRepository for MySqlEventInboxRepository {
    async fn fetch_due_retries(
        &self,
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
        .fetch_all(&self.pool)
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

    async fn claim_message(&self, message: NewInboxMessage) -> AppResult<InboxClaim> {
        let mut tx = self.pool.begin().await?;
        let existing = sqlx::query_as::<_, (String, i32, String, Option<chrono::NaiveDateTime>, chrono::NaiveDateTime)>(
            "SELECT status, retry_count, message_id, CAST(next_retry_at AS DATETIME(6)), CAST(updated_at AS DATETIME(6)) FROM event_inbox WHERE consumer_name = ? AND (message_id = ? OR idempotency_key = ?) LIMIT 1 FOR UPDATE",
        )
        .bind(&message.consumer_name)
        .bind(&message.message_id)
        .bind(&message.idempotency_key)
        .fetch_optional(&mut *tx)
        .await?;

        let claim = if let Some(existing) = existing {
            let existing = ExistingInboxMessage::from(existing);
            let claimed_at = Utc::now().naive_utc();
            let claim = decide_existing_inbox_claim(
                &message,
                existing.clone(),
                processing_token(claimed_at),
            )?;
            if matches!(claim, InboxClaim::Claimed { .. }) {
                sqlx::query(
                    "UPDATE event_inbox SET status = ?, error_message = NULL, updated_at = ? WHERE consumer_name = ? AND message_id = ?",
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
                    let existing = sqlx::query_as::<_, (String, i32, String, Option<chrono::NaiveDateTime>, chrono::NaiveDateTime)>(
                        "SELECT status, retry_count, message_id, CAST(next_retry_at AS DATETIME(6)), CAST(updated_at AS DATETIME(6)) FROM event_inbox WHERE consumer_name = ? AND (message_id = ? OR idempotency_key = ?) LIMIT 1 FOR UPDATE",
                    )
                    .bind(&message.consumer_name)
                    .bind(&message.message_id)
                    .bind(&message.idempotency_key)
                    .fetch_optional(&mut *tx)
                    .await?
                    .ok_or_else(|| {
                        AppError::Internal(
                            "event inbox unique conflict row was not found".to_owned(),
                        )
                    })?;
                    // 插入竞态失败后重新读取冲突行，确保 processing 状态仍触发重投而不是被误判为重复消息。
                    let existing = ExistingInboxMessage::from(existing);
                    let claimed_at = Utc::now().naive_utc();
                    let claim = decide_existing_inbox_claim(
                        &message,
                        existing.clone(),
                        processing_token(claimed_at),
                    )?;
                    if matches!(claim, InboxClaim::Claimed { .. }) {
                        sqlx::query(
                            "UPDATE event_inbox SET status = ?, error_message = NULL, updated_at = ? WHERE consumer_name = ? AND message_id = ?",
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

    async fn mark_consumed(
        &self,
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
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(processing_token_is_stale_error());
        }

        Ok(())
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
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(processing_token_is_stale_error());
        }

        Ok(())
    }
}

#[async_trait]
pub trait EventInboxHandler: Clone + Send + Sync + 'static {
    async fn handle(&self, message: &InboundEventMessage) -> AppResult<()>;
}

#[derive(Clone, Copy)]
pub struct NoopEventInboxHandler;

#[async_trait]
impl EventInboxHandler for NoopEventInboxHandler {
    async fn handle(&self, _message: &InboundEventMessage) -> AppResult<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct EventInboxProductionHandler;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProductionEventDispatch {
    WalletAccountBalanceChanged,
    WalletLedgerEntryCreated,
    SpotOrderCreated,
    SpotOrderCancelled,
    SpotOrderFilled,
    SpotTradeCreated,
    ConvertOrderConfirmed,
    ConvertOrderCompleted,
    NewCoinPurchaseSubscribed,
    NewCoinPurchasePurchased,
    NewCoinPurchaseReleased,
    StrategyMarketEventGenerated,
    MarketTickerUpdated,
    MarketDepthUpdated,
    MarketKlineUpdated,
    MarketTradeCreated,
}

#[derive(Debug, Deserialize)]
struct EventInboxDomainEnvelope {
    aggregate_type: String,
    aggregate_id: String,
    event_type: String,
    routing_key: String,
    idempotency_key: String,
    payload: Value,
}

#[async_trait]
impl EventInboxHandler for EventInboxProductionHandler {
    async fn handle(&self, message: &InboundEventMessage) -> AppResult<()> {
        ProductionEventDispatch::from_inbound(message)?.dispatch()
    }
}

impl ProductionEventDispatch {
    pub fn from_inbound(message: &InboundEventMessage) -> AppResult<Self> {
        let envelope: EventInboxDomainEnvelope = serde_json::from_value(message.payload.clone())
            .map_err(|error| AppError::Validation(format!("invalid event envelope: {error}")))?;
        envelope.dispatch(message)
    }

    pub fn dispatch_key(&self) -> &'static str {
        match self {
            Self::WalletAccountBalanceChanged => "wallet_account.balance_changed",
            Self::WalletLedgerEntryCreated => "wallet_ledger.entry_created",
            Self::SpotOrderCreated => "spot_order.created",
            Self::SpotOrderCancelled => "spot_order.cancelled",
            Self::SpotOrderFilled => "spot_order.filled",
            Self::SpotTradeCreated => "spot_trade.created",
            Self::ConvertOrderConfirmed => "convert_order.confirmed",
            Self::ConvertOrderCompleted => "convert_order.completed",
            Self::NewCoinPurchaseSubscribed => "new_coin_purchase.subscribed",
            Self::NewCoinPurchasePurchased => "new_coin_purchase.purchased",
            Self::NewCoinPurchaseReleased => "new_coin_purchase.released",
            Self::StrategyMarketEventGenerated => "strategy_market_event.generated",
            Self::MarketTickerUpdated => "market_ticker.ticker_updated",
            Self::MarketDepthUpdated => "market_depth.depth_updated",
            Self::MarketKlineUpdated => "market_kline.kline_updated",
            Self::MarketTradeCreated => "market_trade.trade_created",
        }
    }

    fn dispatch(&self) -> AppResult<()> {
        match self {
            Self::WalletAccountBalanceChanged
            | Self::WalletLedgerEntryCreated
            | Self::SpotOrderCreated
            | Self::SpotOrderCancelled
            | Self::SpotOrderFilled
            | Self::SpotTradeCreated
            | Self::ConvertOrderConfirmed
            | Self::ConvertOrderCompleted
            | Self::NewCoinPurchaseSubscribed
            | Self::NewCoinPurchasePurchased
            | Self::NewCoinPurchaseReleased
            | Self::StrategyMarketEventGenerated
            | Self::MarketTickerUpdated
            | Self::MarketDepthUpdated
            | Self::MarketKlineUpdated
            | Self::MarketTradeCreated => Ok(()),
        }
    }
}

impl EventInboxDomainEnvelope {
    fn dispatch(&self, message: &InboundEventMessage) -> AppResult<ProductionEventDispatch> {
        if self.aggregate_id.trim().is_empty()
            || self.event_type.trim().is_empty()
            || self.routing_key.trim().is_empty()
            || self.idempotency_key.trim().is_empty()
        {
            return Err(AppError::Validation("invalid event envelope".to_owned()));
        }
        if self.idempotency_key != message.idempotency_key {
            return Err(AppError::Validation(
                "event envelope idempotency key mismatch".to_owned(),
            ));
        }
        if !self.uses_explicit_producer_idempotency()
            && self.idempotency_key
                != EventIdempotency::new(
                    self.aggregate_type.clone(),
                    self.aggregate_id.clone(),
                    self.event_type.clone(),
                )
                .into_key()
        {
            return Err(AppError::Validation(
                "event envelope idempotency key is inconsistent".to_owned(),
            ));
        }
        if self.payload.is_null() {
            return Err(AppError::Validation(
                "event envelope payload is required".to_owned(),
            ));
        }

        let dispatch = self.to_dispatch()?;
        if self.routing_key != self.expected_routing_key(&dispatch) {
            return Err(AppError::Validation(
                "event envelope routing key mismatch".to_owned(),
            ));
        }
        Ok(dispatch)
    }

    fn uses_explicit_producer_idempotency(&self) -> bool {
        matches!(
            self.aggregate_type.as_str(),
            "market_ticker" | "market_depth" | "market_kline" | "market_trade"
        )
    }

    fn to_dispatch(&self) -> AppResult<ProductionEventDispatch> {
        match (self.aggregate_type.as_str(), self.event_type.as_str()) {
            ("wallet_account", "balance_changed") => {
                Ok(ProductionEventDispatch::WalletAccountBalanceChanged)
            }
            ("wallet_ledger", "entry_created") => {
                Ok(ProductionEventDispatch::WalletLedgerEntryCreated)
            }
            ("spot_order", "created") => Ok(ProductionEventDispatch::SpotOrderCreated),
            ("spot_order", "cancelled") => Ok(ProductionEventDispatch::SpotOrderCancelled),
            ("spot_order", "filled") => Ok(ProductionEventDispatch::SpotOrderFilled),
            ("spot_trade", "created") => Ok(ProductionEventDispatch::SpotTradeCreated),
            ("convert_order", "confirmed") => Ok(ProductionEventDispatch::ConvertOrderConfirmed),
            ("convert_order", "completed") => Ok(ProductionEventDispatch::ConvertOrderCompleted),
            ("new_coin_purchase", "subscribed") => {
                Ok(ProductionEventDispatch::NewCoinPurchaseSubscribed)
            }
            ("new_coin_purchase", "purchased") => {
                Ok(ProductionEventDispatch::NewCoinPurchasePurchased)
            }
            ("new_coin_purchase", "released") => {
                Ok(ProductionEventDispatch::NewCoinPurchaseReleased)
            }
            ("strategy_market_event", "generated") => {
                Ok(ProductionEventDispatch::StrategyMarketEventGenerated)
            }
            ("market_ticker", "ticker_updated") => Ok(ProductionEventDispatch::MarketTickerUpdated),
            ("market_depth", "depth_updated") => Ok(ProductionEventDispatch::MarketDepthUpdated),
            ("market_kline", "kline_updated") => Ok(ProductionEventDispatch::MarketKlineUpdated),
            ("market_trade", "trade_created") => Ok(ProductionEventDispatch::MarketTradeCreated),
            _ => Err(AppError::Validation(format!(
                "unsupported event type {}:{}",
                self.aggregate_type, self.event_type
            ))),
        }
    }

    fn expected_routing_key(&self, dispatch: &ProductionEventDispatch) -> String {
        match dispatch {
            ProductionEventDispatch::WalletAccountBalanceChanged => {
                format!("wallet.{}.balance_changed", self.aggregate_id)
            }
            ProductionEventDispatch::WalletLedgerEntryCreated => {
                format!("wallet.{}.ledger.entry_created", self.aggregate_id)
            }
            ProductionEventDispatch::SpotOrderCreated => {
                format!("spot.{}.order.created", self.aggregate_id)
            }
            ProductionEventDispatch::SpotOrderCancelled => {
                format!("spot.{}.order.cancelled", self.aggregate_id)
            }
            ProductionEventDispatch::SpotOrderFilled => {
                format!("spot.{}.order.filled", self.aggregate_id)
            }
            ProductionEventDispatch::SpotTradeCreated => {
                format!("spot.{}.trade.created", self.aggregate_id)
            }
            ProductionEventDispatch::ConvertOrderConfirmed => {
                format!("convert.order.{}", self.event_type)
            }
            ProductionEventDispatch::ConvertOrderCompleted => {
                format!("convert.order.{}", self.event_type)
            }
            ProductionEventDispatch::NewCoinPurchaseSubscribed => {
                format!("new_coin.purchase.{}", self.event_type)
            }
            ProductionEventDispatch::NewCoinPurchasePurchased => {
                format!("new_coin.purchase.{}", self.event_type)
            }
            ProductionEventDispatch::NewCoinPurchaseReleased => {
                format!("new_coin.purchase.{}", self.event_type)
            }
            ProductionEventDispatch::StrategyMarketEventGenerated => {
                format!("strategy.market.{}", self.aggregate_id)
            }
            ProductionEventDispatch::MarketTickerUpdated => {
                format!("market.{}.ticker", self.aggregate_id)
            }
            ProductionEventDispatch::MarketDepthUpdated => {
                format!("market.{}.depth", self.aggregate_id)
            }
            ProductionEventDispatch::MarketKlineUpdated => {
                let (symbol, interval) = self
                    .aggregate_id
                    .split_once(':')
                    .unwrap_or((&self.aggregate_id, ""));
                format!("market.{symbol}.kline.{interval}")
            }
            ProductionEventDispatch::MarketTradeCreated => {
                let symbol = self
                    .payload
                    .get("symbol")
                    .and_then(Value::as_str)
                    .unwrap_or(&self.aggregate_id);
                format!("market.{symbol}.trade")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsumedInboxMessage {
    Consumed,
    Duplicate,
    Malformed,
    Retried {
        attempt_count: u32,
        next_retry_at: DateTime<Utc>,
    },
    DeadLettered {
        attempt_count: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsumedInboxBatch {
    pub consumed: u32,
    pub duplicates: u32,
    pub retried: u32,
    pub dead_lettered: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EventInboxMetrics {
    pub total: u32,
    pub consumed: u32,
    pub duplicates: u32,
    pub retried: u32,
    pub dead_lettered: u32,
    pub alerts: Vec<EventInboxAlert>,
}

#[derive(Debug)]
pub struct ProcessedInboxDelivery {
    pub result: AppResult<ConsumedInboxMessage>,
    pub disposition: InboxDeliveryDisposition,
    pub alert: Option<EventInboxAlert>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EventInboxAlert {
    pub kind: EventInboxAlertKind,
    pub severity: EventInboxAlertSeverity,
    pub count: u32,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum EventInboxAlertKind {
    RetryBacklog,
    DeadLetter,
    ProcessingError,
    MalformedDelivery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum EventInboxAlertSeverity {
    Warning,
    Critical,
}

impl ConsumedInboxBatch {
    fn record(&mut self, result: ConsumedInboxMessage) {
        match result {
            ConsumedInboxMessage::Consumed => self.consumed += 1,
            ConsumedInboxMessage::Duplicate | ConsumedInboxMessage::Malformed => {
                self.duplicates += 1;
            }
            ConsumedInboxMessage::Retried { .. } => self.retried += 1,
            ConsumedInboxMessage::DeadLettered { .. } => self.dead_lettered += 1,
        }
    }

    pub fn metrics(&self) -> EventInboxMetrics {
        // 将批次结果转成运维快照，并只对需要人工关注的重试/死信生成告警。
        let mut alerts = Vec::new();
        if self.retried > 0 {
            alerts.push(EventInboxAlert::retry_backlog(self.retried));
        }
        if self.dead_lettered > 0 {
            alerts.push(EventInboxAlert::dead_letter(self.dead_lettered));
        }

        EventInboxMetrics {
            total: self.consumed + self.duplicates + self.retried + self.dead_lettered,
            consumed: self.consumed,
            duplicates: self.duplicates,
            retried: self.retried,
            dead_lettered: self.dead_lettered,
            alerts,
        }
    }
}

impl ProcessedInboxDelivery {
    pub fn from_result(result: AppResult<ConsumedInboxMessage>) -> Self {
        let disposition = InboxDeliveryDisposition::from_result(&result);
        let alert = EventInboxAlert::from_delivery_result(&result);
        let result = if disposition == InboxDeliveryDisposition::Ack
            && matches!(result, Err(ref error) if is_malformed_delivery_error(error))
        {
            Ok(ConsumedInboxMessage::Malformed)
        } else {
            result
        };

        Self {
            result,
            disposition,
            alert,
        }
    }
}

impl EventInboxAlert {
    pub fn from_processed_delivery(processed: &ProcessedInboxDelivery) -> Option<Self> {
        processed.alert.clone()
    }

    pub fn from_delivery_result(result: &AppResult<ConsumedInboxMessage>) -> Option<Self> {
        match result {
            Ok(ConsumedInboxMessage::Retried { .. }) => Some(Self::retry_backlog(1)),
            Ok(ConsumedInboxMessage::DeadLettered { .. }) => Some(Self::dead_letter(1)),
            Err(error) if is_malformed_delivery_error(error) => Some(Self::malformed_delivery()),
            Err(_) => Some(Self::processing_error()),
            Ok(
                ConsumedInboxMessage::Consumed
                | ConsumedInboxMessage::Duplicate
                | ConsumedInboxMessage::Malformed,
            ) => None,
        }
    }

    fn retry_backlog(count: u32) -> Self {
        Self {
            kind: EventInboxAlertKind::RetryBacklog,
            severity: EventInboxAlertSeverity::Warning,
            count,
            message: "事件 inbox 存在待重试消息".to_owned(),
        }
    }

    fn dead_letter(count: u32) -> Self {
        Self {
            kind: EventInboxAlertKind::DeadLetter,
            severity: EventInboxAlertSeverity::Critical,
            count,
            message: "事件 inbox 存在死信消息".to_owned(),
        }
    }

    fn processing_error() -> Self {
        Self {
            kind: EventInboxAlertKind::ProcessingError,
            severity: EventInboxAlertSeverity::Critical,
            count: 1,
            message: "事件 inbox 投递处理失败，将重新入队".to_owned(),
        }
    }

    fn malformed_delivery() -> Self {
        Self {
            kind: EventInboxAlertKind::MalformedDelivery,
            severity: EventInboxAlertSeverity::Warning,
            count: 1,
            message: "事件 inbox 投递格式异常，已确认跳过".to_owned(),
        }
    }

    pub fn emit(&self) {
        match self.severity {
            EventInboxAlertSeverity::Warning => tracing::warn!(
                kind = ?self.kind,
                count = self.count,
                message = %self.message,
                "事件 inbox 告警"
            ),
            EventInboxAlertSeverity::Critical => tracing::error!(
                kind = ?self.kind,
                count = self.count,
                message = %self.message,
                "事件 inbox 告警"
            ),
        }
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

impl EventInboxConsumerService<MySqlEventInboxRepository, EventInboxProductionHandler> {
    pub fn from_state(state: &AppState, consumer_name: impl Into<String>) -> AppResult<Self> {
        let pool = state.mysql.clone().ok_or_else(|| {
            AppError::Internal(
                "mysql pool is not configured for event inbox persistence".to_owned(),
            )
        })?;
        let retry_policy = InboxRetryPolicy::new(5, TimeDelta::seconds(30)).map_err(|error| {
            AppError::Internal(format!("invalid event inbox retry policy: {error}"))
        })?;

        Ok(Self::new(
            consumer_name,
            MySqlEventInboxRepository::new(pool),
            EventInboxProductionHandler,
            retry_policy,
        ))
    }
}

impl<R, H> EventInboxConsumerService<R, H>
where
    R: EventInboxRepository,
    H: EventInboxHandler,
{
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

#[derive(Clone)]
pub struct RabbitMqInboxConsumer {
    connection: Arc<lapin::Connection>,
    queue_name: String,
    consumer_tag: String,
}

impl RabbitMqInboxConsumer {
    pub fn new(
        connection: Arc<lapin::Connection>,
        queue_name: impl Into<String>,
        consumer_tag: impl Into<String>,
    ) -> Self {
        Self {
            connection,
            queue_name: queue_name.into(),
            consumer_tag: consumer_tag.into(),
        }
    }

    pub async fn channel(&self) -> AppResult<Channel> {
        Ok(self.connection.create_channel().await?)
    }

    pub async fn consume_loop<R, H>(
        &self,
        service: EventInboxConsumerService<R, H>,
    ) -> AppResult<()>
    where
        R: EventInboxRepository,
        H: EventInboxHandler,
    {
        let channel = self.channel().await?;
        self.consume_channel_loop(channel, service).await
    }

    pub async fn consume_channel_loop<R, H>(
        &self,
        channel: Channel,
        service: EventInboxConsumerService<R, H>,
    ) -> AppResult<()>
    where
        R: EventInboxRepository,
        H: EventInboxHandler,
    {
        let mut consumer = channel
            .basic_consume(
                &self.queue_name,
                &self.consumer_tag,
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;

        while let Some(delivery) = consumer.next().await {
            let delivery = delivery?;
            if let Err(error) = consume_delivery(&service, &delivery, Utc::now()).await {
                tracing::error!(%error, "事件 inbox 投递处理失败");
            }
        }

        Ok(())
    }
}

pub async fn consume_delivery<R, H>(
    service: &EventInboxConsumerService<R, H>,
    delivery: &Delivery,
    now: DateTime<Utc>,
) -> AppResult<ConsumedInboxMessage>
where
    R: EventInboxRepository,
    H: EventInboxHandler,
{
    let result = match InboundEventMessage::from_delivery(delivery) {
        Ok(message) => service.consume_one(message, now).await,
        Err(error) => Err(error),
    };
    let processed = ProcessedInboxDelivery::from_result(result);
    match processed.disposition {
        InboxDeliveryDisposition::Ack => delivery.ack(BasicAckOptions::default()).await?,
        InboxDeliveryDisposition::RejectRequeue => {
            delivery
                .reject(BasicRejectOptions { requeue: true })
                .await?;
        }
    }
    if let Some(alert) = &processed.alert {
        // RabbitMQ ack/requeue 后记录告警分类，便于运维区分重试积压、死信和坏消息。
        alert.emit();
    }
    processed.result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboxDeliveryDisposition {
    Ack,
    RejectRequeue,
}

impl InboxDeliveryDisposition {
    pub fn from_result(result: &AppResult<ConsumedInboxMessage>) -> Self {
        match result {
            Ok(ConsumedInboxMessage::Retried { .. }) => Self::Ack,
            Err(error) if is_malformed_delivery_error(error) => Self::Ack,
            Err(_) => Self::RejectRequeue,
            Ok(
                ConsumedInboxMessage::Consumed
                | ConsumedInboxMessage::Duplicate
                | ConsumedInboxMessage::Malformed
                | ConsumedInboxMessage::DeadLettered { .. },
            ) => Self::Ack,
        }
    }
}

fn is_malformed_delivery_error(error: &AppError) -> bool {
    matches!(error, AppError::Validation(message) if message.starts_with("invalid event payload json:") || message == "event message_id is required" || message == "event idempotency_key is required")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxRetryPolicy {
    max_attempts: u32,
    backoff: TimeDelta,
}

impl InboxRetryPolicy {
    pub fn new(max_attempts: u32, backoff: TimeDelta) -> Result<Self, RetryMetadataError> {
        if max_attempts == 0 {
            return Err(RetryMetadataError::InvalidMaxAttempts);
        }
        if backoff <= TimeDelta::zero() {
            return Err(RetryMetadataError::InvalidBackoff);
        }

        Ok(Self {
            max_attempts,
            backoff,
        })
    }

    pub fn record_failure(
        &self,
        current_attempt_count: u32,
        failed_at: DateTime<Utc>,
    ) -> Result<InboxRetryDecision, RetryMetadataError> {
        let attempt_count = current_attempt_count
            .checked_add(1)
            .ok_or(RetryMetadataError::AttemptOverflow)?;

        if attempt_count >= self.max_attempts {
            Ok(InboxRetryDecision::DeadLetter { attempt_count })
        } else {
            Ok(InboxRetryDecision::Retry {
                attempt_count,
                next_retry_at: failed_at + self.backoff,
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboxRetryDecision {
    Retry {
        attempt_count: u32,
        next_retry_at: DateTime<Utc>,
    },
    DeadLetter {
        attempt_count: u32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeDelta, TimeZone};
    use serde_json::json;

    #[test]
    fn domain_event_constructor_sets_routing_and_idempotency_fields() {
        let created_at = Utc.with_ymd_and_hms(2026, 5, 26, 9, 0, 0).unwrap();

        let event = DomainEvent::new(
            EventRoute::new("convert.events", "convert.completed"),
            EventIdempotency::new("convert_order", "42", "completed"),
            json!({ "quote_id": "quote-1" }),
            created_at,
        );

        assert_eq!(event.exchange, "convert.events");
        assert_eq!(event.routing_key, "convert.completed");
        assert_eq!(event.idempotency_key, "convert_order:42:completed");
        assert_eq!(event.payload, json!({ "quote_id": "quote-1" }));
        assert_eq!(event.created_at, created_at);
    }

    #[test]
    fn inbox_idempotency_key_scopes_message_by_consumer() {
        let key =
            InboxIdempotency::new("wallet-balance", "message-1", "convert_order:42:completed");

        assert_eq!(key.consumer_name, "wallet-balance");
        assert_eq!(key.message_id, "message-1");
        assert_eq!(key.idempotency_key, "convert_order:42:completed");
        assert_eq!(key.consumer_message_key(), "wallet-balance:message-1");
    }

    #[test]
    fn retry_metadata_tracks_next_attempt_and_dead_letter_threshold() {
        let now = Utc.with_ymd_and_hms(2026, 5, 26, 9, 0, 0).unwrap();
        let metadata = RetryMetadata::new(3, TimeDelta::seconds(10)).unwrap();

        let first_failure = metadata.record_failure(now).unwrap();
        assert_eq!(first_failure.attempt_count(), 1);
        assert_eq!(
            first_failure.next_attempt_at(),
            now + TimeDelta::seconds(10)
        );
        assert!(!first_failure.should_dead_letter());

        let third_failure = first_failure
            .record_failure(now + TimeDelta::seconds(10))
            .unwrap()
            .record_failure(now + TimeDelta::seconds(20))
            .unwrap();

        assert_eq!(third_failure.attempt_count(), 3);
        assert!(third_failure.should_dead_letter());
        assert_eq!(
            RetryMetadata::new(0, TimeDelta::seconds(10)).unwrap_err(),
            RetryMetadataError::InvalidMaxAttempts
        );
        assert_eq!(
            RetryMetadata::new(3, TimeDelta::zero()).unwrap_err(),
            RetryMetadataError::InvalidBackoff
        );
    }

    #[test]
    fn existing_processing_inbox_row_returns_error_for_requeue_after_insert_race() {
        let message = NewInboxMessage::new(
            "wallet-balance",
            "message-1",
            "convert_order:42:completed",
            "payload-hash",
            json!({ "idempotency_key": "convert_order:42:completed" }),
        );
        let existing = ExistingInboxMessage {
            status: INBOX_PROCESSING.to_owned(),
            retry_count: 0,
            message_id: message.message_id.clone(),
            next_retry_at: None,
            updated_at: Utc::now().naive_utc(),
        };

        let error =
            decide_existing_inbox_claim(&message, existing, "token-fresh".to_owned()).unwrap_err();

        assert_eq!(
            error.to_string(),
            "internal error: event inbox message is already processing"
        );
    }

    #[test]
    fn stale_processing_inbox_row_can_be_reclaimed_by_retry_scanner() {
        let message = NewInboxMessage::new(
            "wallet-balance",
            "message-1",
            "convert_order:42:completed",
            "payload-hash",
            json!({ "idempotency_key": "convert_order:42:completed" }),
        );
        let existing = ExistingInboxMessage {
            status: INBOX_PROCESSING.to_owned(),
            retry_count: 2,
            message_id: message.message_id.clone(),
            next_retry_at: None,
            updated_at: (Utc::now() - TimeDelta::seconds(301)).naive_utc(),
        };

        let claim =
            decide_existing_inbox_claim(&message, existing, "token-reclaimed".to_owned()).unwrap();

        assert_eq!(
            claim,
            InboxClaim::Claimed {
                attempt_count: 2,
                processing_token: "token-reclaimed".to_owned(),
            }
        );
    }

    #[test]
    fn stale_processing_inbox_row_with_different_message_id_stays_duplicate() {
        let message = NewInboxMessage::new(
            "wallet-balance",
            "message-2",
            "convert_order:42:completed",
            "payload-hash",
            json!({ "idempotency_key": "convert_order:42:completed" }),
        );
        let existing = ExistingInboxMessage {
            status: INBOX_PROCESSING.to_owned(),
            retry_count: 2,
            message_id: "message-1".to_owned(),
            next_retry_at: None,
            updated_at: (Utc::now() - TimeDelta::seconds(301)).naive_utc(),
        };

        let claim =
            decide_existing_inbox_claim(&message, existing, "token-duplicate".to_owned()).unwrap();

        assert_eq!(claim, InboxClaim::Duplicate);
    }
}
