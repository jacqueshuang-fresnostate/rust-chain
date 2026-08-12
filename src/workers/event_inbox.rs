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
    /// 标识 RabbitMQ 实时消费队列，同时作为 inbox 持久化去重与重试扫描的消费边界；该值已通过启动配置的安全字符校验。
    pub fn queue_name(&self) -> &str {
        &self.queue_name
    }

    /// 标识当前 RabbitMQ consumer 实例，供 broker 区分投递所有权和排障；它不替代数据库事件幂等键。
    pub fn consumer_tag(&self) -> &str {
        &self.consumer_tag
    }

    /// 约束 RabbitMQ 已确认后数据库补偿扫描的节奏：零值回落到 10 秒，过大配置收敛到 60 秒，避免关闭补偿或让到期重试长期滞留。
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
    /// 读取 `EVENT_INBOX_QUEUE_NAME` 与可选 `EVENT_INBOX_CONSUMER_TAG`；队列缺失/空白表示同时禁用实时消费和补偿扫描，非 Unicode 值直接报错。
    pub fn from_env() -> AppResult<Self> {
        Self::from_env_values(
            optional_env("EVENT_INBOX_QUEUE_NAME")?.as_deref(),
            optional_env("EVENT_INBOX_CONSUMER_TAG")?.as_deref(),
        )
    }

    /// 由显式值构造启动配置；非空队列和 consumer tag 都必须是最长 128 字节的 ASCII 字母数字、点、冒号、下划线或连字符。
    /// 未提供 tag 时使用稳定默认值 `exchange-api-inbox`；队列为空返回 Disabled，不连接 RabbitMQ，也不启动 MySQL retry scanner。
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

    /// 指示启动阶段是否显式跳过 inbox consumer 与补偿扫描；缺少队列配置属于停机选择，不应被记录为运行故障并反复重连。
    pub fn is_disabled(&self) -> bool {
        self.startup.is_none()
    }

    /// 提供已完成队列名和 consumer tag 校验的启动参数；调用方仅在存在该配置时才应创建实时消费及数据库重试任务。
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
    /// 建立 consumer 重连退避状态；初始等待被限制在 1 到 60 秒，并作为后续稳定成功后的恢复基线。
    pub fn new(initial_delay_seconds: u64) -> Self {
        let initial_delay_seconds = initial_delay_seconds.clamp(1, 60);
        Self {
            initial_delay_seconds,
            next_delay_seconds: initial_delay_seconds,
        }
    }

    /// 读取下一次消费循环结束后应等待的秒数；观察状态本身不会推进退避，便于调度与监控使用同一时钟口径。
    pub fn next_delay_seconds(&self) -> u64 {
        self.next_delay_seconds
    }

    /// 记录一次连接或消费失败，并取得本轮实际等待时间；下一轮按指数增长且最多等待 60 秒，防止 broker 故障时形成热重连。
    pub fn record_failure(&mut self) -> u64 {
        self.record_cycle_outcome(EventInboxConsumerCycleOutcome::Failed)
    }

    /// 统一处理异常退出和意外正常结束：先采用当前等待值，再为下一轮翻倍；两类结束都必须延迟，避免已结束 consumer 被无间隔拉起。
    pub fn record_cycle_outcome(&mut self, _outcome: EventInboxConsumerCycleOutcome) -> u64 {
        let current = self.next_delay_seconds;
        self.next_delay_seconds = (self.next_delay_seconds.saturating_mul(2)).min(60);
        current
    }

    /// 在确认消费链路已经稳定成功后把后续重连等待恢复到启动基线，避免历史故障让恢复后的停机切换仍承受最大延迟。
    pub fn record_success(&mut self) {
        self.next_delay_seconds = self.initial_delay_seconds;
    }
}

/// 单轮按到期时间重放指定 consumer 最多 100 条 retry 或租约超过 300 秒的 processing 行，作为 RabbitMQ 已 ACK 后的持久化补偿路径。
/// 每条重新竞争处理租约；其他实例已领取时按重复跳过，handler 失败推进 5 次/30 秒 retry 或 dead-letter，基础设施错误终止本批，已完成前项不回滚。
/// 批次完成后只发结构化积压/死信告警，不执行 broker ACK，也不跨 consumer 重放持久化 payload。
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

/// 以调用方间隔持续运行指定 consumer 的 MySQL 补偿扫描，零值在本入口收敛为 1 秒；启动流程应先用 `retry_scan_seconds` 将配置限制到最多 60 秒。
/// 单周期错误只记录并继续；未到期、有效租约或死信记录保留在数据库，循环不依赖 RabbitMQ delivery 重新出现。
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

/// 持续消费一个已校验 RabbitMQ 队列，并把该队列名同时作为 inbox consumer 去重/重放范围；不跨队列路由消息。
/// 每条 delivery 在持久化处理结果后 ACK，坏消息和已落 retry/dead-letter 也 ACK，尚未持久化的处理错误 reject+requeue；单条错误记录后继续。
/// RabbitMQ 缺失时启动失败；连接或流结束按 1..=60 秒指数退避重建，数据库 inbox 保存幂等、租约与补偿 payload，循环自身不保存消费游标。
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
