use crate::{
    error::{AppError, AppResult},
    infra,
    modules::events::{EventInboxConsumerService, RabbitMqInboxConsumer},
    state::AppState,
};
use chrono::Utc;
use std::{env, time::Duration};
use tracing::{error, info, warn};

const DEFAULT_CONSUMER_TAG: &str = "exchange-api-inbox";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventInboxStartupConfig {
    queue_name: String,
    consumer_tag: String,
}

impl EventInboxStartupConfig {
    pub fn queue_name(&self) -> &str {
        &self.queue_name
    }

    pub fn consumer_tag(&self) -> &str {
        &self.consumer_tag
    }

    pub fn retry_scan_seconds(&self, configured_seconds: u64) -> u64 {
        if configured_seconds == 0 {
            10
        } else {
            configured_seconds.min(60)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventInboxWorkerConfig {
    startup: Option<EventInboxStartupConfig>,
}

impl EventInboxWorkerConfig {
    pub fn from_env() -> AppResult<Self> {
        Self::from_env_values(
            optional_env("EVENT_INBOX_QUEUE_NAME")?.as_deref(),
            optional_env("EVENT_INBOX_CONSUMER_TAG")?.as_deref(),
        )
    }

    pub fn from_env_values(
        queue_name: Option<&str>,
        consumer_tag: Option<&str>,
    ) -> AppResult<Self> {
        let Some(queue_name) = queue_name.map(str::trim).filter(|value| !value.is_empty()) else {
            return Ok(Self { startup: None });
        };
        let queue_name = validate_segment(queue_name, "event inbox queue name")?;
        let consumer_tag = consumer_tag
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(DEFAULT_CONSUMER_TAG);

        Ok(Self {
            startup: Some(EventInboxStartupConfig {
                queue_name,
                consumer_tag: validate_segment(consumer_tag, "event inbox consumer tag")?,
            }),
        })
    }

    pub fn is_disabled(&self) -> bool {
        self.startup.is_none()
    }

    pub fn startup(&self) -> Option<&EventInboxStartupConfig> {
        self.startup.as_ref()
    }
}

fn optional_env(key: &str) -> AppResult<Option<String>> {
    match env::var(key) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => {
            Err(AppError::Validation(format!("{key} must be valid unicode")))
        }
    }
}

fn validate_segment(value: &str, field: &str) -> AppResult<String> {
    if value.is_empty()
        || value.len() > 128
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_' | ':')
        })
    {
        return Err(AppError::Validation(format!("invalid {field}")));
    }

    Ok(value.to_owned())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventInboxConsumerCycleOutcome {
    Ended,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventInboxReconnectBackoff {
    initial_delay_seconds: u64,
    next_delay_seconds: u64,
}

impl EventInboxReconnectBackoff {
    pub fn new(initial_delay_seconds: u64) -> Self {
        let initial_delay_seconds = initial_delay_seconds.clamp(1, 60);
        Self {
            initial_delay_seconds,
            next_delay_seconds: initial_delay_seconds,
        }
    }

    pub fn next_delay_seconds(&self) -> u64 {
        self.next_delay_seconds
    }

    pub fn record_failure(&mut self) -> u64 {
        self.record_cycle_outcome(EventInboxConsumerCycleOutcome::Failed)
    }

    pub fn record_cycle_outcome(&mut self, _outcome: EventInboxConsumerCycleOutcome) -> u64 {
        let current = self.next_delay_seconds;
        self.next_delay_seconds = (self.next_delay_seconds.saturating_mul(2)).min(60);
        current
    }

    pub fn record_success(&mut self) {
        self.next_delay_seconds = self.initial_delay_seconds;
    }
}

pub async fn run_retry_scanner_once(state: &AppState, consumer_name: &str) -> AppResult<()> {
    let service = EventInboxConsumerService::from_state(state, consumer_name.to_owned())?;
    let batch = service.replay_due_retries(Utc::now(), 100).await?;
    let metrics = batch.metrics();
    info!(
        total = metrics.total,
        consumed = metrics.consumed,
        duplicates = metrics.duplicates,
        retried = metrics.retried,
        dead_lettered = metrics.dead_lettered,
        "事件 inbox 重试扫描完成"
    );
    for alert in &metrics.alerts {
        // DB 补偿扫描产生的 retry/dead-letter 也走统一告警出口，避免只监控 RabbitMQ delivery 路径。
        alert.emit();
    }

    Ok(())
}

pub async fn run_retry_scanner_loop(
    state: AppState,
    consumer_name: String,
    interval_seconds: u64,
) -> AppResult<()> {
    let mut ticker = tokio::time::interval(Duration::from_secs(interval_seconds.max(1)));

    loop {
        ticker.tick().await;
        // 定时扫描 MySQL 中到期的 retry 行，补上 RabbitMQ delivery 已 ACK 后的重放路径。
        if let Err(error) = run_retry_scanner_once(&state, &consumer_name).await {
            error!(%error, "事件 inbox 重试扫描失败");
        }
    }
}

pub async fn run_loop(
    state: AppState,
    queue_name: impl Into<String>,
    consumer_tag: impl Into<String>,
) -> AppResult<()> {
    if state.rabbitmq.is_none() {
        return Err(AppError::Internal(
            "rabbitmq connection is not configured for event inbox consumer".to_owned(),
        ));
    }

    let queue_name = queue_name.into();
    let consumer_tag = consumer_tag.into();
    let mut backoff = EventInboxReconnectBackoff::new(state.settings.market_feed_reconnect_seconds);

    loop {
        let service = EventInboxConsumerService::from_state(&state, queue_name.clone())?;
        let rabbitmq = match infra::rabbitmq::connect(&state.settings).await {
            Ok(connection) => connection,
            Err(error) => {
                let delay_seconds =
                    backoff.record_cycle_outcome(EventInboxConsumerCycleOutcome::Failed);
                error!(%error, delay_seconds, "事件 inbox RabbitMQ 重连失败");
                tokio::time::sleep(Duration::from_secs(delay_seconds)).await;
                continue;
            }
        };
        let consumer =
            RabbitMqInboxConsumer::new(rabbitmq.into(), queue_name.clone(), consumer_tag.clone());

        let (outcome, error) = match consumer.consume_loop(service).await {
            Ok(()) => (EventInboxConsumerCycleOutcome::Ended, None),
            Err(error) => (EventInboxConsumerCycleOutcome::Failed, Some(error)),
        };
        let delay_seconds = backoff.record_cycle_outcome(outcome);

        if let Some(error) = error {
            error!(%error, delay_seconds, "事件 inbox 消费循环失败");
        } else {
            warn!(delay_seconds, "事件 inbox 消费循环结束");
        }

        tokio::time::sleep(Duration::from_secs(delay_seconds)).await;
    }
}
