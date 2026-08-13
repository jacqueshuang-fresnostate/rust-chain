//! 事件 inbox 消费 worker：把 RabbitMQ 实时投递与数据库补偿扫描两条路径拉起并维持运行。
//!
//! 两条路径互补而非重复。实时路径从队列取消息，处理完再确认；
//! 补偿路径定时扫描 inbox 中到期待重试和租约超时的行，从数据库存档的载荷重放。
//! 补偿之所以必要，是因为业务失败后消息已被确认，broker 侧不会再投，重放只能靠本地存档。
//!
//! 队列名同时充当 inbox 的消费者名，因此它决定了去重范围与补偿扫描范围，
//! 改队列名等同于换一个消费者身份，历史去重记录不再适用。
//!
//! 未配置队列名视为显式停用而非故障，此时两条路径都不启动，也不会反复尝试连接。
//! 实时路径的连接与消费循环一旦结束就按 1 到 60 秒的指数退避重建，
//! 这里的指数退避与消息级重试的固定退避是两套机制，前者防止 broker 故障时形成热重连。

use crate::{
    error::{AppError, AppResult},
    infra,
    modules::events::{EventInboxConsumerService, RabbitMqInboxConsumer},
    state::AppState,
};
use chrono::Utc;
use std::{env, time::Duration};
use tracing::{error, info, warn};

/// 未显式配置消费者标签时使用的稳定默认值，保持它稳定可使 broker 侧的消费者视图不随重启变化。
const DEFAULT_CONSUMER_TAG: &str = "exchange-api-inbox";

/// 已通过字符校验的 inbox 启动参数，只有存在该结构时才应拉起消费与补偿两条路径。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventInboxStartupConfig {
    /// 目标队列名，同时充当 inbox 的消费者名，因而决定去重与补偿扫描的范围。
    queue_name: String,
    /// broker 侧的消费者标签，仅用于运维观察，不参与业务去重。
    consumer_tag: String,
}

impl EventInboxStartupConfig {
    /// 标识 RabbitMQ 实时消费队列，同时作为 inbox 持久化去重与重试扫描的消费边界；该值已通过启动配置的安全字符校验。
    pub fn queue_name(&self) -> &str {
        &self.queue_name
    }

    /// 返回 broker 侧的消费者标签，仅用于在管理界面区分实例与排障。
    /// 它不参与数据库层的去重与租约判定，改标签不会影响任何幂等语义。
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

/// inbox worker 的顶层配置，用是否持有启动参数来表达启用与停用两种状态。
/// 停用是显式选择而非异常，因此不应被记录成故障或触发重连。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventInboxWorkerConfig {
    /// 启动参数；为空表示本进程不承担事件消费职责。
    startup: Option<EventInboxStartupConfig>,
}

impl EventInboxWorkerConfig {
    /// 从环境变量读取队列名与可选的消费者标签，得到本进程的 inbox 配置。
    /// 队列名缺失或为空白表示显式停用，此时实时消费与补偿扫描都不会启动，且不视为故障。
    /// 变量含非 Unicode 字节时直接报错而不是当作未设置，避免部署脚本写错导致消费者悄无声息地不工作。
    pub fn from_env() -> AppResult<Self> {
        Self::from_env_values(
            optional_env("EVENT_INBOX_QUEUE_NAME")?.as_deref(),
            optional_env("EVENT_INBOX_CONSUMER_TAG")?.as_deref(),
        )
    }

    /// 由显式值构造启动配置；非空队列和 consumer tag 都必须是最长 128 字节的 ASCII 字母数字、点、冒号、下划线或连字符。
    /// 队列名先裁剪空白再判空，因此只填空格等同于未配置，直接返回停用状态并短路后续校验。
    /// 消费者标签同样先裁剪，为空时回落到稳定默认值而不是报错，因为它只影响运维观察不影响正确性。
    /// 两个值都必须通过字符校验，任一非法即返回错误而不是降级使用默认值，
    /// 因为队列名同时充当数据库中的消费者名，取值错误会造成去重范围错乱。
    /// 停用时不连接 broker，也不启动补偿扫描任务。
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

/// 读取可选环境变量，把「未设置」与「值非法」区分开。
/// 变量缺失返回 `Ok(None)` 由调用方按停用处理；含非 Unicode 字节则返回校验错误而不是静默忽略，
/// 因为那通常意味着部署脚本写错了值，静默跳过会让消费者莫名不启动。
fn optional_env(key: &str) -> AppResult<Option<String>> {
    match env::var(key) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => {
            Err(AppError::Validation(format!("{key} must be valid unicode")))
        }
    }
}

/// 校验队列名或消费者标签：非空、不超过 128 字节，且只含 ASCII 字母数字与点、短横线、下划线、冒号。
/// 字符集收紧是因为这两个值会直接进入 AMQP 协议字段，含空格或控制字符会导致连接层报错难以定位。
/// 队列名还兼作数据库中的消费者名，稳定的字符集也避免了不同写法被当成两个消费者。
/// 校验通过后原样返回，不做大小写折叠。
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

/// 一轮消费循环的结束方式，两者都会触发退避等待。
/// 正常结束同样需要延迟：消费循环本不该自行退出，无间隔重启只会形成空转。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventInboxConsumerCycleOutcome {
    /// 消费流正常结束，通常意味着 broker 侧关闭了 channel 或队列。
    Ended,
    /// 连接建立或消费过程中出错。
    Failed,
}

/// 消费循环的重连退避状态，按指数增长并封顶在 60 秒。
/// 与消息级重试的固定间隔退避不同，这里针对的是连接层故障，指数增长可避免 broker 长时间不可用时反复热重连。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventInboxReconnectBackoff {
    /// 启动基线等待秒数，稳定成功后会恢复到该值。
    initial_delay_seconds: u64,
    /// 下一轮结束后应等待的秒数，每次结束翻倍直到封顶。
    next_delay_seconds: u64,
}

impl EventInboxReconnectBackoff {
    /// 建立消费循环的重连退避状态，初始等待被夹到 1 至 60 秒。
    /// 下限 1 秒防止配置为零导致无间隔热重连，上限 60 秒与后续翻倍的封顶值保持一致。
    /// 夹取后的值同时成为下一次等待值与恢复基线，稳定成功后退避会回落到它。
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
