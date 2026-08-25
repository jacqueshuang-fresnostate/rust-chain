//! 外部行情源订阅 worker：维护多供应商 WebSocket 连接、协议应答与 REST 兜底。
//!
//! 运行时配置由后台版本化下发，监督器句柄负责校验新配置、切换后台任务并对外暴露最近一次 reload 结果。
//! 每个供应商各跑一个独立的重连循环，每轮先尝试 WebSocket，失败且配置了兜底请求时才退到该供应商的 REST 抓取；
//! 失败按倍数退避并封顶 60 秒，单个供应商故障不会影响其他供应商继续订阅。
//!
//! 本文件只负责连接、帧归一化与调度：真正的解析、Redis 与 Mongo 落库、outbox 以及实时广播都发生在 ingestion 侧，
//! 因此这里的重连与退避不会回滚任何已经写入并广播出去的行情。

use crate::{
    config::Settings,
    error::{AppError, AppResult},
    modules::market::{
        MarketDataProvider, MarketDepthSnapshot, MarketKlineSnapshot, MarketTickerSnapshot,
        adapters::{
            MarketFeedChannel, MarketFeedConfig, MarketFeedFrame, MarketFeedProvider,
            MarketFeedRestFallbackConfig, MarketFeedRestFallbackHttpClient, MarketFeedSummary,
            MarketFeedWorker, MarketIngestionService, MarketIngestionSink,
            ReqwestMarketFeedRestFallbackHttpClient,
        },
    },
    state::AppState,
};
use axum::async_trait;
use flate2::read::GzDecoder;
use futures_util::{FutureExt, Sink, SinkExt, Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::Digest;
use std::{
    future::{Future, pending},
    io::Read,
    panic::AssertUnwindSafe,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::{
    sync::{Mutex, RwLock},
    task::{JoinHandle, JoinSet},
    time::{
        Duration, Instant, Interval, MissedTickBehavior, interval_at, sleep, sleep_until, timeout,
    },
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{self, Message},
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

const BITGET_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(25);
const MARKET_FEED_INBOUND_IDLE_TIMEOUT: Duration = Duration::from_secs(75);
const MARKET_FEED_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const MARKET_FEED_WRITE_TIMEOUT: Duration = Duration::from_secs(10);

/// 行情 generation 围栏；所有外部写入前必须确认绑定 generation 仍是当前值。
#[derive(Debug, Clone, Default)]
struct MarketFeedGenerationFence {
    active: Arc<RwLock<u64>>,
}

impl MarketFeedGenerationFence {
    /// 取排他锁切换 generation；必须等待所有旧代写许可释放后才能返回。
    async fn activate(&self, generation: u64) {
        *self.active.write().await = generation;
    }

    /// 取当前 generation 的共享写许可；调用方必须持有 guard 跨越整个外部副作用。
    /// 这使“检查围栏”与“写 MySQL/Redis/Mongo”不再存在 reload 可插入的空档。
    async fn enter(&self, generation: u64) -> AppResult<tokio::sync::OwnedRwLockReadGuard<u64>> {
        let guard = self.active.clone().read_owned().await;
        if *guard == generation {
            Ok(guard)
        } else {
            Err(AppError::Conflict(format!(
                "stale market feed generation {generation} is fenced"
            )))
        }
    }
}

/// 把一次完整的行情副作用绑定到 generation。
/// WebSocket 与 REST 路径都持有该许可直到存储和实时事件发布全部返回，
/// 因此 reload 的排他切换不能插入在“已写 Redis/Mongo、尚未发 event”之间。
#[derive(Clone)]
struct MarketFeedWriteFence {
    generation: u64,
    fence: MarketFeedGenerationFence,
}

impl MarketFeedWriteFence {
    fn new(generation: u64, fence: MarketFeedGenerationFence) -> Self {
        Self { generation, fence }
    }

    async fn enter(&self) -> AppResult<tokio::sync::OwnedRwLockReadGuard<u64>> {
        self.fence.enter(self.generation).await
    }
}

/// 为一个 generation 包装真实摄取器：ticker 先归档 MySQL 历史，再写 Redis/Mongo/事件链。
#[derive(Clone)]
struct GenerationBoundMarketIngestionSink {
    inner: MarketIngestionService,
    mysql: sqlx::Pool<sqlx::MySql>,
    generation: u64,
    fence: MarketFeedGenerationFence,
}

impl GenerationBoundMarketIngestionSink {
    fn from_state(
        state: &AppState,
        generation: u64,
        fence: MarketFeedGenerationFence,
    ) -> AppResult<Self> {
        let mysql = state.mysql.clone().ok_or_else(|| {
            AppError::Internal("mysql is required for replayable market feed history".to_owned())
        })?;
        Ok(Self {
            inner: MarketIngestionService::from_state(state)?,
            mysql,
            generation,
            fence,
        })
    }

    fn source(provider: MarketDataProvider) -> &'static str {
        match provider {
            MarketDataProvider::Bitget => "bitget",
            MarketDataProvider::Htx => "htx",
            MarketDataProvider::Coinbase => "coinbase",
            MarketDataProvider::Strategy => "strategy",
        }
    }

    async fn archive_ticker(&self, snapshot: &MarketTickerSnapshot) -> AppResult<()> {
        if self.generation == 0 {
            return Err(AppError::Validation(
                "market feed history generation must be positive".to_owned(),
            ));
        }
        if snapshot.last_price() <= &bigdecimal::BigDecimal::from(0) {
            return Err(AppError::Validation(
                "market feed history price must be positive".to_owned(),
            ));
        }
        let source = Self::source(snapshot.provider());
        let symbol = snapshot
            .symbol()
            .trim()
            .chars()
            .filter(|character| !matches!(character, '-' | '/' | '_'))
            .flat_map(char::to_uppercase)
            .collect::<String>();
        if symbol.is_empty() {
            return Err(AppError::Validation(
                "market feed history symbol is required".to_owned(),
            ));
        }
        let canonical = format!(
            "{}|{}|{}|{}",
            source,
            symbol,
            snapshot.observed_at().timestamp_micros(),
            snapshot.last_price().normalized()
        );
        let event_key = hex::encode(sha2::Sha256::digest(canonical.as_bytes()));
        sqlx::query(
            r#"INSERT INTO market_price_ticks
               (event_key, symbol, price, source, observed_at, generation, source_version)
               VALUES (?, ?, ?, ?, ?, ?, ?)
               ON DUPLICATE KEY UPDATE event_key = VALUES(event_key)"#,
        )
        .bind(&event_key)
        .bind(&symbol)
        .bind(snapshot.last_price())
        .bind(source)
        .bind(snapshot.observed_at().naive_utc())
        .bind(self.generation)
        .bind(&event_key)
        .execute(&self.mysql)
        .await?;
        Ok(())
    }
}

#[async_trait]
impl MarketIngestionSink for GenerationBoundMarketIngestionSink {
    async fn ingest_ticker(&self, snapshot: &MarketTickerSnapshot) -> AppResult<()> {
        let _permit = self.fence.enter(self.generation).await?;
        self.archive_ticker(snapshot).await?;
        self.inner.ingest_ticker(snapshot).await
    }

    async fn ingest_depth(&self, snapshot: &MarketDepthSnapshot) -> AppResult<()> {
        let _permit = self.fence.enter(self.generation).await?;
        self.inner.ingest_depth(snapshot).await
    }

    async fn ingest_kline(&self, snapshot: &MarketKlineSnapshot) -> AppResult<()> {
        let _permit = self.fence.enter(self.generation).await?;
        self.inner.ingest_kline(snapshot).await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketFeedRuntimeConfig {
    symbols: Vec<String>,
    intervals: Vec<String>,
    providers: Vec<MarketFeedProvider>,
    reconnect_seconds: u64,
}

impl MarketFeedRuntimeConfig {
    /// 从调用方已规范化的交易对/周期构造配置，仅解析并去重供应商码；连接重试秒数最小收敛为 1。
    /// 本入口不重新验证 symbol/interval，也不创建供应商连接、缓存写入或广播任务。
    pub fn from_normalized(
        symbols: Vec<String>,
        intervals: Vec<String>,
        providers: Vec<String>,
        reconnect_seconds: u64,
    ) -> AppResult<Self> {
        let providers = market_feed_providers(providers)?;
        Ok(Self {
            symbols,
            intervals,
            providers,
            reconnect_seconds: reconnect_seconds.max(1),
        })
    }

    /// 依据系统设置通过 provider adapter 规范化交易对、周期及供应商订阅矩阵；空交易对形成显式禁用配置。
    /// 非空配置必须至少生成一个 provider config，否则启动失败；构造不连接外网、不写 Redis/Mongo，也不广播行情。
    /// 规范化后的交易对与周期取自首个 provider 配置，因此各供应商共用同一份订阅范围；重连秒数最小收敛为 1 秒。
    pub fn new(
        settings: &Settings,
        symbols: Vec<String>,
        intervals: Vec<String>,
        providers: Vec<String>,
        reconnect_seconds: u64,
    ) -> AppResult<Self> {
        if symbols.is_empty() {
            return Ok(Self {
                symbols: Vec::new(),
                intervals: Vec::new(),
                providers: Vec::new(),
                reconnect_seconds: reconnect_seconds.max(1),
            });
        }
        let providers = market_feed_providers(providers)?;
        let symbol_refs: Vec<&str> = symbols.iter().map(String::as_str).collect();
        let interval_refs: Vec<&str> = intervals.iter().map(String::as_str).collect();
        let configs = MarketFeedWorker::<MarketIngestionService>::provider_configs_for(
            settings,
            &providers,
            &symbol_refs,
            &interval_refs,
        )?;
        let Some(first_config) = configs.first() else {
            return Err(crate::error::AppError::Internal(
                "market feed provider configs are empty".to_owned(),
            ));
        };
        Ok(Self {
            symbols: first_config.symbols().to_vec(),
            intervals: first_config.intervals().to_vec(),
            providers,
            reconnect_seconds: reconnect_seconds.max(1),
        })
    }

    /// 判断该配置是否应维持行情任务；空交易对是显式停机信号，即使仍带周期或供应商也不得继续订阅。
    pub fn enabled(&self) -> bool {
        !self.symbols.is_empty()
    }

    /// 提供本次 reload 的权威交易对集合，用于生成供应商订阅矩阵并同步可观察运行状态。
    /// 集合为空即表示这份配置处于停用状态，监督器据此停止任务，而不是去订阅一个空列表。
    pub fn symbols(&self) -> &[String] {
        &self.symbols
    }

    /// 提供各交易对需要维持的 K 线周期集合；监督器重启任务时必须整组应用，避免新旧周期混跑。
    /// 周期与交易对在订阅矩阵里按乘积展开，因此多加一个周期会让每个供应商的订阅消息同步增加。
    pub fn intervals(&self) -> &[String] {
        &self.intervals
    }

    /// 提供本轮启用的行情供应商优先集合；它决定并行连接与 REST 补偿来源，也是失败监控的归属维度。
    /// 顺序保持配置解析后的先后并已去重，运行期每个供应商各自独占一个重连任务。
    pub fn providers(&self) -> &[MarketFeedProvider] {
        &self.providers
    }

    /// 提供供应商连接失败后的重试基准秒数；构造阶段已保证至少一秒，避免网络故障触发无间隔重连。
    pub fn reconnect_seconds(&self) -> u64 {
        self.reconnect_seconds
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketFeedRuntimeStatus {
    pub applied_version: Option<u64>,
    pub generation: u64,
    pub ready: bool,
    pub symbols: Vec<String>,
    pub intervals: Vec<String>,
    pub providers: Vec<String>,
    pub last_reload_status: Option<String>,
    pub last_reload_error: Option<String>,
}

#[derive(Clone)]
pub struct MarketFeedSupervisorHandle {
    state: Arc<RwLock<MarketFeedSupervisorState>>,
    operation_lock: Arc<Mutex<()>>,
    generation_counter: Arc<AtomicU64>,
    fence: MarketFeedGenerationFence,
    runner: Arc<dyn MarketFeedGenerationRunner>,
}

struct MarketFeedSupervisorState {
    status: MarketFeedRuntimeStatus,
    task: Option<MarketFeedGenerationTask>,
}

struct MarketFeedGenerationTask {
    generation: u64,
    cancellation: CancellationToken,
    handle: JoinHandle<()>,
}

#[async_trait]
trait MarketFeedGenerationRunner: Send + Sync {
    /// 运行一个指定代际的完整行情 provider 集合，直到收到取消信号或任一受监督任务失败。
    ///
    /// 实现必须让所有持久化与事件发布持有对应 generation fence 许可，并在返回前 drain、join 全部子任务；
    /// reload/disable 依赖这一完成语义，确保旧代际不会在新配置生效后继续写 MySQL、Redis、Mongo 或事件总线。
    async fn run(
        &self,
        state: AppState,
        config: MarketFeedRuntimeConfig,
        generation: u64,
        fence: MarketFeedGenerationFence,
        cancellation: CancellationToken,
    ) -> AppResult<()>;
}

struct DefaultMarketFeedGenerationRunner;

#[async_trait]
impl MarketFeedGenerationRunner for DefaultMarketFeedGenerationRunner {
    /// 使用生产行情循环执行指定代际，并把结构化并发、取消与写入围栏参数原样传入底层编排器。
    async fn run(
        &self,
        state: AppState,
        config: MarketFeedRuntimeConfig,
        generation: u64,
        fence: MarketFeedGenerationFence,
        cancellation: CancellationToken,
    ) -> AppResult<()> {
        run_config_loop_with_generation(state, config, generation, fence, cancellation).await
    }
}

impl MarketFeedSupervisorHandle {
    /// 创建尚未应用任何版本且没有后台任务的行情监督器；首个有效配置必须经 reload 校验后才进入运行状态。
    pub fn new() -> Self {
        Self::with_runner(Arc::new(DefaultMarketFeedGenerationRunner))
    }

    fn with_runner(runner: Arc<dyn MarketFeedGenerationRunner>) -> Self {
        Self {
            state: Arc::new(RwLock::new(MarketFeedSupervisorState {
                status: MarketFeedRuntimeStatus::default(),
                task: None,
            })),
            operation_lock: Arc::new(Mutex::new(())),
            generation_counter: Arc::new(AtomicU64::new(0)),
            fence: MarketFeedGenerationFence::default(),
            runner,
        }
    }

    /// 为测试建立与生产相同的空监督状态，但不隐式启动连接，便于验证配置版本、失败和停机状态迁移。
    pub fn new_for_tests() -> Self {
        Self::new()
    }

    #[cfg(test)]
    fn with_runner_for_tests(runner: Arc<dyn MarketFeedGenerationRunner>) -> Self {
        Self::with_runner(runner)
    }

    /// 取得当前已应用配置版本及最近 reload 结果的一致快照，供管理端扫描状态；读取不会启动、停止或重试任务。
    pub async fn status(&self) -> MarketFeedRuntimeStatus {
        self.state.read().await.status.clone()
    }

    /// 串行应用版本化配置：先校验，再 cancel/join 旧 generation，最后切换围栏并启动新代。
    pub async fn reload(
        &self,
        state: AppState,
        config: MarketFeedRuntimeConfig,
        version: u64,
    ) -> AppResult<MarketFeedRuntimeStatus> {
        let _operation = self.operation_lock.lock().await;
        if config.enabled() {
            let symbol_refs: Vec<&str> = config.symbols().iter().map(String::as_str).collect();
            let interval_refs: Vec<&str> = config.intervals().iter().map(String::as_str).collect();
            MarketFeedWorker::<MarketIngestionService>::provider_configs_for(
                &state.settings,
                config.providers(),
                &symbol_refs,
                &interval_refs,
            )?;
        }
        let generation = self.generation_counter.fetch_add(1, Ordering::SeqCst) + 1;
        self.shutdown_active_generation(generation).await;
        let status_name = if config.enabled() {
            "success"
        } else {
            "skipped"
        };
        {
            let mut guard = self.state.write().await;
            guard.status = runtime_status_from_config(
                &config,
                version,
                generation,
                config.enabled(),
                status_name,
                None,
            );
        }
        if config.enabled() {
            self.spawn_generation(state, config, generation).await;
        }
        Ok(self.state.read().await.status.clone())
    }

    /// cancel 并等待当前 generation 完整退出，再推进到新的禁用 generation。
    pub async fn stop(&self) {
        let _operation = self.operation_lock.lock().await;
        let generation = self.generation_counter.fetch_add(1, Ordering::SeqCst) + 1;
        self.shutdown_active_generation(generation).await;
        let mut guard = self.state.write().await;
        guard.status.generation = generation;
        guard.status.ready = false;
        guard.status.last_reload_status = Some("skipped".to_owned());
        guard.status.last_reload_error = None;
    }

    async fn spawn_generation(
        &self,
        state: AppState,
        config: MarketFeedRuntimeConfig,
        generation: u64,
    ) {
        let cancellation = CancellationToken::new();
        let task_cancellation = cancellation.clone();
        let runner = self.runner.clone();
        let fence = self.fence.clone();
        let supervisor_state = self.state.clone();
        let handle = tokio::spawn(async move {
            let outcome = AssertUnwindSafe(runner.run(
                state,
                config,
                generation,
                fence,
                task_cancellation.clone(),
            ))
            .catch_unwind()
            .await;
            let failure = match outcome {
                Ok(Ok(())) if task_cancellation.is_cancelled() => None,
                Ok(Ok(())) => Some("market feed generation stopped unexpectedly".to_owned()),
                Ok(Err(error)) if task_cancellation.is_cancelled() => {
                    tracing::debug!(generation, %error, "行情 generation 取消后结束");
                    None
                }
                Ok(Err(error)) => Some(error.to_string()),
                Err(_) => Some("market feed generation panicked".to_owned()),
            };
            if let Some(error) = failure {
                tracing::error!(generation, %error, "行情 generation 失败并退出");
                let mut guard = supervisor_state.write().await;
                if guard.status.generation == generation {
                    guard.status.ready = false;
                    guard.status.last_reload_status = Some("failed".to_owned());
                    guard.status.last_reload_error = Some(error);
                }
            }
        });
        self.state.write().await.task = Some(MarketFeedGenerationTask {
            generation,
            cancellation,
            handle,
        });
    }

    async fn shutdown_active_generation(&self, next_generation: u64) {
        // 先 cancel 并 join 旧父子任务，保证旧代不会在新代就绪后补发 event。
        // 围栏的排他切换又会等待任何在途存储写许可，因此不存在 check/write 窗口。
        let task = self.state.write().await.task.take();
        if let Some(task) = task {
            task.cancellation.cancel();
            if let Err(error) = task.handle.await {
                tracing::error!(
                    generation = task.generation,
                    %error,
                    "行情 generation join 失败"
                );
            }
        }
        self.fence.activate(next_generation).await;
    }

    /// 记录配置扫描或 reload 失败供管理端诊断；保留上一次已应用版本和任务句柄，不把一次扫描错误误当成已成功切换。
    pub async fn record_failure(&self, error: String) -> MarketFeedRuntimeStatus {
        let mut guard = self.state.write().await;
        if guard.task.is_none() {
            guard.status.ready = false;
        }
        guard.status.last_reload_status = Some("failed".to_owned());
        guard.status.last_reload_error = Some(error);
        guard.status.clone()
    }

    /// 在不创建供应商连接的情况下模拟配置已接受，仅用于验证版本和状态发布；不会中止或替换监督器中的任务。
    pub async fn accept_config_for_tests(
        &self,
        config: MarketFeedRuntimeConfig,
        version: u64,
    ) -> AppResult<MarketFeedRuntimeStatus> {
        let generation = self.generation_counter.fetch_add(1, Ordering::SeqCst) + 1;
        self.fence.activate(generation).await;
        let mut guard = self.state.write().await;
        guard.status =
            runtime_status_from_config(&config, version, generation, false, "success", None);
        Ok(guard.status.clone())
    }
}

impl Default for MarketFeedSupervisorHandle {
    /// 委托构造函数得到同样的空监督状态，因此默认值不会隐式建立任何供应商连接。
    /// 只有经过一次成功的 reload 才会产生后台任务，默认实例的已应用版本保持为空。
    fn default() -> Self {
        Self::new()
    }
}

/// 把配置快照与本次 reload 结果折叠成对外可观察的运行状态，供应商枚举在此转成稳定代码字符串。
/// `applied_version` 一律记录本次传入的版本，即使结果是跳过，因此停用同样算一次已应用的配置变更。
/// 错误文本由调用方决定是否传入，成功与跳过场景传空，从而清除上一次遗留的失败信息。
fn runtime_status_from_config(
    config: &MarketFeedRuntimeConfig,
    version: u64,
    generation: u64,
    ready: bool,
    status: &str,
    error: Option<String>,
) -> MarketFeedRuntimeStatus {
    MarketFeedRuntimeStatus {
        applied_version: Some(version),
        generation,
        ready,
        symbols: config.symbols().to_vec(),
        intervals: config.intervals().to_vec(),
        providers: config
            .providers()
            .iter()
            .map(|provider| provider.code().to_owned())
            .collect(),
        last_reload_status: Some(status.to_owned()),
        last_reload_error: error,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketFeedTextAction {
    Frame(MarketFeedFrame),
    Reply(String),
    Ignore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketFeedSocketAction {
    Frame(MarketFeedFrame),
    Reply(Message),
    Ignore,
    Close,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketFeedSupervisorEvent {
    ProviderCycleSucceeded {
        provider: MarketFeedProvider,
    },
    ProviderCycleFailed {
        provider: MarketFeedProvider,
        delay: Duration,
        error: String,
    },
    ProviderTaskFailed {
        provider: MarketFeedProvider,
        error: String,
    },
}

#[derive(Debug)]
enum MarketFeedSocketEvent {
    Message(Option<Result<Message, tungstenite::Error>>),
    HeartbeatDue,
    IdleTimeout,
}

struct MarketFeedSocketLiveness {
    provider: MarketFeedProvider,
    idle_timeout: Duration,
    idle_deadline: Instant,
    heartbeat: Option<Interval>,
}

impl MarketFeedSocketLiveness {
    /// 按供应商协议创建连接活性状态：Bitget 主动发文本心跳，HTX 与 Coinbase 依赖服务端活性帧，三者共用入站静默上限。
    /// 构造只建立本地定时状态，不发送报文；首个心跳从完整周期后开始，避免刚完成订阅就额外占用供应商消息限额。
    fn new(provider: MarketFeedProvider) -> Self {
        Self::new_with_timing(
            provider,
            Instant::now(),
            market_feed_heartbeat_interval(provider),
            MARKET_FEED_INBOUND_IDLE_TIMEOUT,
        )
    }

    /// 允许单元测试注入起点、心跳周期和静默上限，验证截止时间迁移而不等待生产级 75 秒。
    /// 该入口仅在测试构建可见，不改变生产协议常量，也不会生成网络副作用。
    #[cfg(test)]
    fn new_for_tests(
        provider: MarketFeedProvider,
        started_at: Instant,
        heartbeat_interval: Option<Duration>,
        idle_timeout: Duration,
    ) -> Self {
        Self::new_with_timing(provider, started_at, heartbeat_interval, idle_timeout)
    }

    /// 组装可轮询的心跳 interval 与入站截止时间；零周期被视为禁用，防止 tokio interval 因非法周期 panic。
    /// interval 使用 Delay 策略，任务被调度延迟后只补一次心跳，不会突发补发多条报文触发供应商限流。
    fn new_with_timing(
        provider: MarketFeedProvider,
        started_at: Instant,
        heartbeat_interval: Option<Duration>,
        idle_timeout: Duration,
    ) -> Self {
        let heartbeat = heartbeat_interval
            .filter(|interval| !interval.is_zero())
            .map(|interval| {
                let mut timer = interval_at(started_at + interval, interval);
                timer.set_missed_tick_behavior(MissedTickBehavior::Delay);
                timer
            });
        Self {
            provider,
            idle_timeout,
            idle_deadline: started_at + idle_timeout,
            heartbeat,
        }
    }

    /// 返回当前入站静默截止点，仅用于核对活性状态；读取不会延长连接寿命或消费心跳 tick。
    #[cfg(test)]
    fn idle_deadline(&self) -> Instant {
        self.idle_deadline
    }

    /// 用当前单调时钟记录任意入站帧，行情、确认、ping、pong 与关闭帧都能证明此前链路仍可读。
    /// 本方法只延后静默截止点，不改变主动心跳节奏，避免高频行情让 Bitget 永远收不到客户端 ping。
    fn record_inbound(&mut self) {
        self.record_inbound_at(Instant::now());
    }

    /// 从指定单调时刻重算静默截止点，供生产入口和确定性测试共用；不接受墙上时钟，避免系统校时导致误判。
    fn record_inbound_at(&mut self, observed_at: Instant) {
        self.idle_deadline = observed_at + self.idle_timeout;
    }

    /// 同时等待下一条上游消息、供应商主动心跳时刻或入站静默截止点。
    /// biased 顺序让到期心跳不会被持续可读的高频行情饿死，随后仍优先消费已到达帧再判断静默截止，避免边界误重连。
    async fn wait_next<S>(&mut self, reader: &mut S) -> MarketFeedSocketEvent
    where
        S: Stream<Item = Result<Message, tungstenite::Error>> + Unpin,
    {
        let idle_deadline = self.idle_deadline;
        tokio::select! {
            biased;
            _ = wait_for_market_feed_heartbeat(&mut self.heartbeat) => {
                MarketFeedSocketEvent::HeartbeatDue
            }
            message = reader.next() => MarketFeedSocketEvent::Message(message),
            _ = sleep_until(idle_deadline) => MarketFeedSocketEvent::IdleTimeout,
        }
    }
}

/// 返回供应商主动心跳周期；Bitget 官方要求客户端定时发送文本 ping，HTX 和 Coinbase 已提供服务端活性帧。
/// `None` 表示不额外发送应用层心跳，但连接仍受统一入站静默上限保护。
fn market_feed_heartbeat_interval(provider: MarketFeedProvider) -> Option<Duration> {
    match provider {
        MarketFeedProvider::Bitget => Some(BITGET_HEARTBEAT_INTERVAL),
        MarketFeedProvider::Htx | MarketFeedProvider::Coinbase => None,
    }
}

/// 生成供应商主动心跳报文；Bitget 协议要求纯文本 `ping`，其他供应商不在客户端主动发送应用层心跳。
/// 返回值与 `market_feed_heartbeat_interval` 成对维护，调用方只在周期到期且存在报文时写入连接。
fn market_feed_heartbeat_message(provider: MarketFeedProvider) -> Option<Message> {
    match provider {
        MarketFeedProvider::Bitget => Some(Message::Text("ping".to_owned())),
        MarketFeedProvider::Htx | MarketFeedProvider::Coinbase => None,
    }
}

/// 等待可选心跳计时器的下一次 tick；未启用主动心跳时保持 pending，让消息与静默截止分支独占调度。
/// 本函数不发送报文、不重置静默截止，也不补发因调度暂停错过的多次 tick。
async fn wait_for_market_feed_heartbeat(heartbeat: &mut Option<Interval>) {
    match heartbeat {
        Some(heartbeat) => {
            heartbeat.tick().await;
        }
        None => pending::<()>().await,
    }
}

/// 将供应商 WebSocket 消息归一化为行情帧、协议回复、忽略或关闭动作。
/// HTX gzip 二进制在此解压，ping 必须回应；无法识别或错误确认帧不得进入价格缓存。
/// 协议层 Ping 一律回 Pong 并带回原载荷，Pong 直接忽略，Close 转为关闭动作让调用方结束本轮读循环。
/// 文本帧与解压后的二进制帧走同一套文本判定，因此两条路径对同一段 JSON 得到完全一致的动作。
pub fn market_feed_socket_action(
    provider: MarketFeedProvider,
    message: Message,
) -> AppResult<MarketFeedSocketAction> {
    match message {
        Message::Text(payload) => match market_feed_text_action(provider, &payload)? {
            MarketFeedTextAction::Frame(frame) => Ok(MarketFeedSocketAction::Frame(frame)),
            MarketFeedTextAction::Reply(reply) => {
                Ok(MarketFeedSocketAction::Reply(Message::Text(reply)))
            }
            MarketFeedTextAction::Ignore => Ok(MarketFeedSocketAction::Ignore),
        },
        Message::Binary(payload) => {
            let payload = market_feed_binary_payload_text(provider, &payload)?;
            match market_feed_text_action(provider, &payload)? {
                MarketFeedTextAction::Frame(frame) => Ok(MarketFeedSocketAction::Frame(frame)),
                MarketFeedTextAction::Reply(reply) => {
                    Ok(MarketFeedSocketAction::Reply(Message::Text(reply)))
                }
                MarketFeedTextAction::Ignore => Ok(MarketFeedSocketAction::Ignore),
            }
        }
        Message::Ping(payload) => Ok(MarketFeedSocketAction::Reply(Message::Pong(payload))),
        Message::Pong(_) => Ok(MarketFeedSocketAction::Ignore),
        Message::Close(_) => Ok(MarketFeedSocketAction::Close),
        _ => Ok(MarketFeedSocketAction::Ignore),
    }
}

/// 把二进制 WebSocket 帧还原为文本，目前只有 HTX 会推送 gzip 压缩帧。
/// 非 HTX 供应商的二进制帧，以及缺少 gzip 魔数的载荷，都直接返回校验错误，不做任何猜测性解码。
/// 解压失败同样按校验错误处理，因此损坏的压缩帧不会被继续当作行情解析。
fn market_feed_binary_payload_text(
    provider: MarketFeedProvider,
    payload: &[u8],
) -> AppResult<String> {
    if provider != MarketFeedProvider::Htx {
        return Err(crate::error::AppError::Validation(format!(
            "unsupported {} market feed binary websocket frame",
            provider.code()
        )));
    }
    if !payload.starts_with(&[0x1f, 0x8b]) {
        return Err(crate::error::AppError::Validation(
            "unsupported market feed binary websocket frame".to_owned(),
        ));
    }

    let mut decoder = GzDecoder::new(payload);
    let mut text = String::new();
    decoder.read_to_string(&mut text).map_err(|error| {
        crate::error::AppError::Validation(format!(
            "invalid gzip market feed binary websocket frame: {error}"
        ))
    })?;
    Ok(text)
}

/// 解析供应商文本帧并区分心跳/订阅确认与真实行情数据；错误确认直接失败，未知频道只忽略。
/// 返回的行情帧仍需经过 provider adapter 严格解析后才能成为权威价格输入。
/// Bitget 纯文本 ping/pong 在 JSON 前处理；其余非法 JSON 返回校验错误，带 `ping` 字段的心跳原值回 `pong`。
/// 只带 `event` 或 `op` 而没有 `data` 的控制帧一律忽略，避免把订阅回执当成行情写进缓存。
pub fn market_feed_text_action(
    provider: MarketFeedProvider,
    payload: &str,
) -> AppResult<MarketFeedTextAction> {
    let payload = payload.trim();
    if provider == MarketFeedProvider::Bitget {
        if payload.eq_ignore_ascii_case("pong") {
            return Ok(MarketFeedTextAction::Ignore);
        }
        if payload.eq_ignore_ascii_case("ping") {
            return Ok(MarketFeedTextAction::Reply("pong".to_owned()));
        }
    }
    let value: Value = serde_json::from_str(payload).map_err(|error| {
        crate::error::AppError::Validation(format!("invalid market feed websocket json: {error}"))
    })?;
    if let Some(ping) = value.get("ping") {
        return Ok(MarketFeedTextAction::Reply(
            json!({ "pong": ping }).to_string(),
        ));
    }
    if let Some(action) = market_feed_acknowledgement_action(provider, &value)? {
        return Ok(action);
    }
    if (value.get("event").is_some() || value.get("op").is_some()) && value.get("data").is_none() {
        return Ok(MarketFeedTextAction::Ignore);
    }
    let channel = channel_from_payload(payload);
    if channel == MarketFeedChannel::None {
        return Ok(MarketFeedTextAction::Ignore);
    }
    Ok(MarketFeedTextAction::Frame(MarketFeedFrame::new(
        provider, channel, payload,
    )))
}

/// 按供应商分派协议应答处理，把订阅确认、错误应答与真实行情三类载荷区分开。
/// 返回 `None` 表示这不是应答帧，应继续走行情解析；三家供应商的状态与错误字段各不相同，无法共用一套判断。
fn market_feed_acknowledgement_action(
    provider: MarketFeedProvider,
    value: &Value,
) -> AppResult<Option<MarketFeedTextAction>> {
    match provider {
        MarketFeedProvider::Bitget => bitget_acknowledgement_action(value),
        MarketFeedProvider::Htx => htx_acknowledgement_action(value),
        MarketFeedProvider::Coinbase => coinbase_acknowledgement_action(value),
    }
}

/// 识别 Bitget 的应答帧：既没有 `event` 也没有 `op` 时直接放行给行情解析。
/// `event` 为 error 或 `code` 不等于 0 一律按错误应答返回校验错误，避免把失败的订阅当成连接正常。
/// 成功应答且不带 `data` 时忽略该帧；带 `data` 的消息返回 `None`，继续按行情处理。
fn bitget_acknowledgement_action(value: &Value) -> AppResult<Option<MarketFeedTextAction>> {
    if value.get("event").is_none() && value.get("op").is_none() {
        return Ok(None);
    }
    if field_as_string(value, "event").as_deref() == Some("error") {
        return Err(acknowledgement_error("bitget", value, "code", "msg"));
    }
    match field_as_string(value, "code") {
        Some(code) if code != "0" => Err(acknowledgement_error("bitget", value, "code", "msg")),
        _ if value.get("data").is_none() => Ok(Some(MarketFeedTextAction::Ignore)),
        _ => Ok(None),
    }
}

/// 识别 HTX 的应答帧，只有存在 `status` 字段时才介入判断。
/// `status` 为 ok 且带 `subbed`、不带 `data` 说明是订阅确认，忽略即可；为 error 时按错误码与错误消息报错。
/// 其余取值返回 `None`，交回给行情解析继续处理。
fn htx_acknowledgement_action(value: &Value) -> AppResult<Option<MarketFeedTextAction>> {
    let Some(status) = field_as_string(value, "status") else {
        return Ok(None);
    };
    match status.as_str() {
        "ok" if value.get("subbed").is_some() && value.get("data").is_none() => {
            Ok(Some(MarketFeedTextAction::Ignore))
        }
        "error" => Err(acknowledgement_error("htx", value, "err-code", "err-msg")),
        _ => Ok(None),
    }
}

/// 识别 Coinbase 的应答帧：`type` 或 `channel` 命中 error 时按错误应答返回校验错误。
/// 两个字段都忽略大小写比较，以兼容网关不同版本对事件名的大小写写法。
/// `channel` 为 heartbeats 的心跳帧直接忽略，其余情况返回 `None`，交回给后续频道判定继续按行情解析。
fn coinbase_acknowledgement_action(value: &Value) -> AppResult<Option<MarketFeedTextAction>> {
    if field_as_string(value, "type")
        .as_deref()
        .is_some_and(|event_type| event_type.eq_ignore_ascii_case("error"))
        || field_as_string(value, "channel")
            .as_deref()
            .is_some_and(|channel| channel.eq_ignore_ascii_case("error"))
    {
        return Err(acknowledgement_error("coinbase", value, "code", "message"));
    }
    if field_as_string(value, "channel")
        .as_deref()
        .is_some_and(|channel| channel.eq_ignore_ascii_case("heartbeats"))
    {
        return Ok(Some(MarketFeedTextAction::Ignore));
    }
    Ok(None)
}

/// 用供应商名、错误码与错误消息组装统一的订阅失败错误，取哪两个键由各供应商协议决定。
/// 错误码缺失时记为 unknown，消息缺失时退回整条 JSON 原文，保证排查时不会丢失现场上下文。
fn acknowledgement_error(
    provider: &str,
    value: &Value,
    code_key: &str,
    message_key: &str,
) -> crate::error::AppError {
    let code = field_as_string(value, code_key).unwrap_or_else(|| "unknown".to_owned());
    let message = field_as_string(value, message_key).unwrap_or_else(|| value.to_string());
    crate::error::AppError::Validation(format!(
        "{provider} market feed acknowledgement error: code={code}, message={message}"
    ))
}

/// 从 JSON 对象取出一个标量字段并统一转成字符串，兼容字符串、数字与布尔三种写法。
/// 供应商的错误码时而用数字时而用字符串，这里先统一形态再按文本比较；对象与数组一律返回 `None`。
fn field_as_string(value: &Value, key: &str) -> Option<String> {
    match value.get(key)? {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

/// 对配置中的每个供应商各启动一个 WebSocket 周期并等待全部结束；任一任务失败会使本次调用失败。
/// 禁用配置立即成功且无网络/持久化副作用；有效帧的 Redis/Mongo 写入、outbox 和实时广播由 ingestion sink 在处理时执行。
pub async fn run_config_once(state: &AppState, config: &MarketFeedRuntimeConfig) -> AppResult<()> {
    if !config.enabled() {
        return Ok(());
    }
    let symbol_refs: Vec<&str> = config.symbols().iter().map(String::as_str).collect();
    let interval_refs: Vec<&str> = config.intervals().iter().map(String::as_str).collect();
    run_once_with_providers(state, config.providers(), &symbol_refs, &interval_refs).await
}

/// 为配置中的每个供应商启动独立无限重连任务；每轮优先 WebSocket，失败且配置存在请求时才执行该供应商 REST 兜底。
/// 单供应商失败按配置基准做有上限退避并继续，不影响其他供应商；若任一监督任务异常结束则本入口返回错误，交由上层配置 supervisor 标记失败。
/// 每个有效帧在 ingestion 时写 Redis/Mongo、outbox 并广播，任务间没有跨供应商事务，前序持久化不会因后续连接故障回滚。
pub async fn run_config_loop(state: AppState, config: MarketFeedRuntimeConfig) -> AppResult<()> {
    let fence = MarketFeedGenerationFence::default();
    fence.activate(1).await;
    run_config_loop_with_generation(state, config, 1, fence, CancellationToken::new()).await
}

async fn run_config_loop_with_generation(
    state: AppState,
    config: MarketFeedRuntimeConfig,
    generation: u64,
    fence: MarketFeedGenerationFence,
    cancellation: CancellationToken,
) -> AppResult<()> {
    if !config.enabled() {
        return Ok(());
    }
    let symbol_refs: Vec<&str> = config.symbols().iter().map(String::as_str).collect();
    let interval_refs: Vec<&str> = config.intervals().iter().map(String::as_str).collect();
    let provider_configs = MarketFeedWorker::<MarketIngestionService>::provider_configs_for(
        &state.settings,
        config.providers(),
        &symbol_refs,
        &interval_refs,
    )?;
    let rest_fallback_configs =
        MarketFeedWorker::<MarketIngestionService>::provider_rest_fallback_configs_for(
            &state.settings,
            config.providers(),
            &symbol_refs,
            &interval_refs,
        )?;
    let reconnect_delay = Duration::from_secs(config.reconnect_seconds());
    let mut tasks = JoinSet::new();
    for (provider_config, rest_fallback_config) in
        provider_configs.into_iter().zip(rest_fallback_configs)
    {
        let state = state.clone();
        let provider = provider_config.provider();
        let task_cancellation = cancellation.clone();
        let task_fence = fence.clone();
        tasks.spawn(async move {
            let future = run_provider_reconnect_loop(
                state,
                provider_config,
                rest_fallback_config,
                reconnect_delay,
                generation,
                task_fence,
                task_cancellation,
            );
            let result = AssertUnwindSafe(future).catch_unwind().await;
            let result = match result {
                Ok(result) => result,
                Err(_) => Err(AppError::Internal(format!(
                    "market feed provider {} panicked",
                    provider.code()
                ))),
            };
            (provider, result)
        });
    }
    tokio::select! {
        _ = cancellation.cancelled() => {
            while let Some(joined) = tasks.join_next().await {
                if let Err(error) = joined {
                    tracing::warn!(generation, %error, "取消行情 provider 时 join 失败");
                }
            }
            Ok(())
        }
        joined = tasks.join_next() => {
            let Some(joined) = joined else {
                return Err(AppError::Internal(
                    "market feed generation has no provider tasks".to_owned(),
                ));
            };
            let (provider, result) = match joined {
                Ok(joined) => joined,
                Err(join_error) => {
                    cancellation.cancel();
                    while let Some(joined) = tasks.join_next().await {
                        if let Err(error) = joined {
                            tracing::warn!(generation, %error, "join 失败后回收行情 provider 任务时再次失败");
                        }
                    }
                    return Err(AppError::Internal(format!(
                        "market feed provider join failed: {join_error}"
                    )));
                }
            };
            let error = match result {
                Ok(()) if cancellation.is_cancelled() => {
                    while let Some(joined) = tasks.join_next().await {
                        if let Err(join_error) = joined {
                            tracing::warn!(generation, %join_error, "取消后回收行情 provider 任务时 join 失败");
                        }
                    }
                    return Ok(());
                }
                Ok(()) => AppError::Internal(format!(
                    "market feed provider {} stopped unexpectedly",
                    provider.code()
                )),
                Err(error) => error,
            };
            emit_market_feed_supervisor_event(MarketFeedSupervisorEvent::ProviderTaskFailed {
                provider,
                error: error.to_string(),
            });
            cancellation.cancel();
            while let Some(joined) = tasks.join_next().await {
                if let Err(join_error) = joined {
                    tracing::warn!(generation, %join_error, "失败后回收行情 provider 任务时 join 失败");
                }
            }
            Err(error)
        }
    }
}

/// 为默认供应商集合并行执行一次 WebSocket 行情周期，使用调用方交易对/周期生成各自订阅范围。
/// 任一供应商任务失败会使本次调用失败；其他任务已完成的 Redis/Mongo、outbox 与广播副作用不回滚。
pub async fn run_once(state: &AppState, symbols: &[&str], intervals: &[&str]) -> AppResult<()> {
    run_once_with_providers(
        state,
        &MarketFeedProvider::default_providers(),
        symbols,
        intervals,
    )
    .await
}

/// 为给定供应商集合各启动一次 WebSocket 周期并等待全部结束，交易对与周期在展开配置时统一校验。
/// 任一任务 join 失败或返回错误都会让本次调用失败，且该错误不会被其余任务的成功结果改写。
/// 各任务的 Redis、Mongo 写入与广播彼此独立，先完成的副作用不会因为后来的失败而回滚。
async fn run_once_with_providers(
    state: &AppState,
    providers: &[MarketFeedProvider],
    symbols: &[&str],
    intervals: &[&str],
) -> AppResult<()> {
    let configs = MarketFeedWorker::<MarketIngestionService>::provider_configs_for(
        &state.settings,
        providers,
        symbols,
        intervals,
    )?;
    let cancellation = CancellationToken::new();
    let fence = MarketFeedGenerationFence::default();
    fence.activate(1).await;
    let mut tasks = JoinSet::new();
    for config in configs {
        let state = state.clone();
        let cancellation = cancellation.clone();
        let fence = fence.clone();
        tasks.spawn(async move {
            tokio::select! {
                _ = cancellation.cancelled() => Ok(()),
                result = run_provider_once_with_generation(
                    state,
                    config,
                    1,
                    fence,
                    cancellation.clone(),
                ) => result,
            }
        });
    }

    while let Some(joined) = tasks.join_next().await {
        let result = joined.map_err(|error| {
            crate::error::AppError::Internal(format!("market feed provider task failed: {error}"))
        });
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                cancellation.cancel();
                while let Some(joined) = tasks.join_next().await {
                    if let Err(join_error) = joined {
                        warn!(%join_error, "回收行情 provider 单次任务时 join 失败");
                    }
                }
                return Err(error);
            }
            Err(error) => {
                cancellation.cancel();
                while let Some(joined) = tasks.join_next().await {
                    if let Err(join_error) = joined {
                        warn!(%join_error, "回收行情 provider 单次任务时 join 失败");
                    }
                }
                return Err(error);
            }
        }
    }
    Ok(())
}

/// 为单个供应商装配重连循环所需的运行时：按系统设置创建 REST 兜底 HTTP 客户端，
/// 并注入真正执行 WebSocket 周期的函数、ingestion worker 构造器与监督事件回调。
/// 本函数只做装配，退避节奏、兜底触发条件和事件上报都由被调用的通用循环负责。
async fn run_provider_reconnect_loop(
    state: AppState,
    config: MarketFeedConfig,
    rest_fallback_config: MarketFeedRestFallbackConfig,
    reconnect_delay: Duration,
    generation: u64,
    fence: MarketFeedGenerationFence,
    cancellation: CancellationToken,
) -> AppResult<()> {
    let http_client = rest_fallback_http_client(&state.settings);
    let run_fence = fence.clone();
    let build_fence = fence.clone();
    let rest_write_fence = fence.clone();
    let socket_cancellation = cancellation.clone();
    run_provider_reconnect_loop_with_cancellation(
        state,
        config,
        reconnect_delay,
        move |state, config| {
            run_provider_once_with_generation(
                state,
                config,
                generation,
                run_fence.clone(),
                socket_cancellation.clone(),
            )
        },
        MarketFeedRestFallbackRuntime::new(
            rest_fallback_config,
            move |state| {
                let fence = build_fence.clone();
                async move { market_feed_worker_for_generation(&state, generation, fence) }
            },
            http_client,
        )
        .with_generation_fence(generation, rest_write_fence),
        emit_market_feed_supervisor_event,
        cancellation,
    )
    .await
}

/// 按系统设置创建 REST 兜底使用的 HTTP 客户端，超时与代理等参数全部来自配置而非硬编码。
/// 每个供应商的重连循环各自创建一个实例，不共享同一个客户端对象。
fn rest_fallback_http_client(settings: &Settings) -> ReqwestMarketFeedRestFallbackHttpClient {
    ReqwestMarketFeedRestFallbackHttpClient::from_settings(settings)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MarketFeedReconnectBackoff {
    initial_delay: Duration,
    current_delay: Duration,
    max_delay: Duration,
}

impl MarketFeedReconnectBackoff {
    /// 用配置给出的基准延迟初始化退避状态，当前延迟从基准起步，上限固定为 60 秒。
    /// 上限写死在类型内部而不来自配置，因此再大的基准值也不会让重连间隔无限增长。
    fn new(initial_delay: Duration) -> Self {
        Self {
            initial_delay,
            current_delay: initial_delay,
            max_delay: Duration::from_secs(60),
        }
    }

    /// 返回本轮结束后应等待的延迟，读取不改变状态，因此同一轮多次读取结果一致。
    /// 调用方在执行周期之前先取值，使成功与失败两条分支共用同一个已经确定的等待时长。
    fn next_delay(&self) -> Duration {
        self.current_delay
    }

    /// 记录一次周期失败并把等待时长翻倍，最长封顶在 60 秒。
    /// 只改变下一轮的延迟，既不触发睡眠也不上报事件，日志与监督事件由调用方另行处理。
    fn record_failure(&mut self) {
        self.current_delay = (self.current_delay * 2).min(self.max_delay);
    }

    /// 周期成功后把等待时长复位到配置基准，使连接恢复后不再背负之前累积的长延迟。
    /// 同样只改状态不做睡眠；由于循环成功后仍要等待一轮，复位保证这段等待回到最短值。
    fn record_success(&mut self) {
        self.current_delay = self.initial_delay;
    }
}

struct MarketFeedRestFallbackRuntime<B, C> {
    config: MarketFeedRestFallbackConfig,
    build_worker: B,
    http_client: C,
    write_fence: Option<MarketFeedWriteFence>,
}

impl<B, C> MarketFeedRestFallbackRuntime<B, C> {
    /// 打包 REST 兜底所需的三件套：请求清单配置、ingestion worker 构造器和 HTTP 客户端。
    /// 构造只是聚合依赖，既不发起请求也不判断清单是否为空，真正的触发条件由周期执行函数判断。
    fn new(config: MarketFeedRestFallbackConfig, build_worker: B, http_client: C) -> Self {
        Self {
            config,
            build_worker,
            http_client,
            write_fence: None,
        }
    }

    /// 生产代际路径在 REST 全轮摄取期间持有共享许可，保护最后一次 event 发布。
    fn with_generation_fence(mut self, generation: u64, fence: MarketFeedGenerationFence) -> Self {
        self.write_fence = Some(MarketFeedWriteFence::new(generation, fence));
        self
    }
}

/// 单个供应商的无限重连主体：每轮先取当前退避延迟，再执行一次带 REST 兜底的行情周期。
/// 成功则上报周期成功事件并把延迟复位到基准；失败则上报失败事件、打印带供应商与延迟秒数的错误日志，
/// 并把下一轮延迟翻倍直到封顶 60 秒。无论成败都会在轮末等待该延迟，因此成功也不会形成忙循环。
/// 循环没有正常退出分支，只会随任务被中止而结束；本函数不判断兜底是否可用，也不改写行情写入结果。
/// 执行函数、worker 构造器、HTTP 客户端与事件回调都由调用方注入，便于在测试中替换而不接触真实网络。
#[cfg(test)]
async fn run_provider_reconnect_loop_with<S, F, Fut, B, BuildFut, C, E>(
    state: AppState,
    config: MarketFeedConfig,
    reconnect_delay: Duration,
    run_provider: F,
    fallback: MarketFeedRestFallbackRuntime<B, C>,
    emit_event: E,
) -> AppResult<()>
where
    S: MarketIngestionSink,
    F: FnMut(AppState, MarketFeedConfig) -> Fut,
    Fut: Future<Output = AppResult<()>>,
    B: FnMut(AppState) -> BuildFut,
    BuildFut: Future<Output = AppResult<MarketFeedWorker<S>>>,
    C: MarketFeedRestFallbackHttpClient,
    E: FnMut(MarketFeedSupervisorEvent),
{
    run_provider_reconnect_loop_with_cancellation(
        state,
        config,
        reconnect_delay,
        run_provider,
        fallback,
        emit_event,
        CancellationToken::new(),
    )
    .await
}

async fn run_provider_reconnect_loop_with_cancellation<S, F, Fut, B, BuildFut, C, E>(
    state: AppState,
    config: MarketFeedConfig,
    reconnect_delay: Duration,
    mut run_provider: F,
    mut fallback: MarketFeedRestFallbackRuntime<B, C>,
    mut emit_event: E,
    cancellation: CancellationToken,
) -> AppResult<()>
where
    S: MarketIngestionSink,
    F: FnMut(AppState, MarketFeedConfig) -> Fut,
    Fut: Future<Output = AppResult<()>>,
    B: FnMut(AppState) -> BuildFut,
    BuildFut: Future<Output = AppResult<MarketFeedWorker<S>>>,
    C: MarketFeedRestFallbackHttpClient,
    E: FnMut(MarketFeedSupervisorEvent),
{
    let mut backoff = MarketFeedReconnectBackoff::new(reconnect_delay);
    loop {
        let delay = backoff.next_delay();
        let write_fence = fallback.write_fence.clone();
        let cycle = run_provider_cycle_with_rest_fallback_guarded(
            state.clone(),
            config.clone(),
            fallback.config.clone(),
            fallback.http_client.clone(),
            &mut run_provider,
            &mut fallback.build_worker,
            write_fence,
        );
        tokio::pin!(cycle);
        let result = tokio::select! {
            result = &mut cycle => result,
            _ = cancellation.cancelled() => {
                // 取消只阻止后续帧；当前摄取必须完成并释放 generation 写许可后，
                // 父任务才可被 join。直接 drop SQL/Redis/Mongo future 可能让服务端副作用
                // 在围栏推进后才落地，因此这里显式 drain 当前周期。
                let _ = cycle.await;
                return Ok(());
            },
        };
        match result {
            Ok(_) => {
                emit_event(MarketFeedSupervisorEvent::ProviderCycleSucceeded {
                    provider: config.provider(),
                });
                backoff.record_success();
            }
            Err(error) => {
                emit_event(MarketFeedSupervisorEvent::ProviderCycleFailed {
                    provider: config.provider(),
                    delay,
                    error: error.to_string(),
                });
                error!(
                    provider = ?config.provider(),
                    delay_seconds = delay.as_secs(),
                    %error,
                    "行情源周期执行失败"
                );
                backoff.record_failure();
            }
        }
        tokio::select! {
            _ = cancellation.cancelled() => return Ok(()),
            _ = sleep(delay) => {}
        }
    }
}

/// 执行单供应商周期：WebSocket 成功时不请求 REST，失败时仅在配置了兜底请求后抓取并写入有效帧。
/// REST 汇总至少必须含一个成功写入帧，全部无效时仍返回失败，禁止把空响应视为价格源恢复。
/// 该入口不自行重试；外层监督循环负责退避，写入端负责 Redis/Mongo 持久化与实时事件副作用。
pub async fn run_provider_cycle_with_rest_fallback<S, F, Fut, B, BuildFut, C>(
    state: AppState,
    config: MarketFeedConfig,
    rest_fallback_config: MarketFeedRestFallbackConfig,
    http_client: C,
    mut run_provider: F,
    mut build_worker: B,
) -> AppResult<MarketFeedSummary>
where
    S: MarketIngestionSink,
    F: FnMut(AppState, MarketFeedConfig) -> Fut,
    Fut: Future<Output = AppResult<()>>,
    B: FnMut(AppState) -> BuildFut,
    BuildFut: Future<Output = AppResult<MarketFeedWorker<S>>>,
    C: MarketFeedRestFallbackHttpClient,
{
    run_provider_cycle_with_rest_fallback_guarded(
        state,
        config,
        rest_fallback_config,
        http_client,
        &mut run_provider,
        &mut build_worker,
        None,
    )
    .await
}

/// 生产代际路径在 REST 写入与事件发布的整个过程持有 generation 许可；
/// 公开的无 generation 测试入口则传 `None`，保持原有泛型契约。
#[allow(clippy::too_many_arguments)]
async fn run_provider_cycle_with_rest_fallback_guarded<S, F, Fut, B, BuildFut, C>(
    state: AppState,
    config: MarketFeedConfig,
    rest_fallback_config: MarketFeedRestFallbackConfig,
    http_client: C,
    run_provider: &mut F,
    build_worker: &mut B,
    write_fence: Option<MarketFeedWriteFence>,
) -> AppResult<MarketFeedSummary>
where
    S: MarketIngestionSink,
    F: FnMut(AppState, MarketFeedConfig) -> Fut,
    Fut: Future<Output = AppResult<()>>,
    B: FnMut(AppState) -> BuildFut,
    BuildFut: Future<Output = AppResult<MarketFeedWorker<S>>>,
    C: MarketFeedRestFallbackHttpClient,
{
    match run_provider(state.clone(), config).await {
        Ok(()) => Ok(MarketFeedSummary::default()),
        Err(error) => {
            if rest_fallback_config.ticker_requests().is_empty()
                && rest_fallback_config.kline_requests().is_empty()
            {
                return Err(error);
            }
            warn!(%error, "行情 WebSocket 周期失败，开始执行 REST 兜底");
            let worker = build_worker(state).await?;
            let _permit = match write_fence {
                Some(write_fence) => Some(write_fence.enter().await?),
                None => None,
            };
            let summary = worker
                .run_rest_fallback_config(&rest_fallback_config, &http_client)
                .await?;
            ensure_market_feed_cycle_has_valid_frames(&summary)?;
            Ok(summary)
        }
    }
}

/// 把监督事件落成分级日志：周期成功记 info，周期失败记 warn 并带上供应商与下一轮等待秒数，
/// 任务失败记 error，因为那意味着该供应商的重连循环已经彻底退出而不只是本轮出错。
/// 本函数只写日志，不改变退避状态、不重启任务，也不影响调用方对错误本身的处理方式。
fn emit_market_feed_supervisor_event(event: MarketFeedSupervisorEvent) {
    match event {
        MarketFeedSupervisorEvent::ProviderCycleSucceeded { provider } => {
            info!(provider = ?provider, "行情源周期执行成功");
        }
        MarketFeedSupervisorEvent::ProviderCycleFailed {
            provider,
            delay,
            error,
        } => {
            warn!(
                provider = ?provider,
                delay_seconds = delay.as_secs(),
                error = %error,
                "行情订阅监督器记录到行情源周期失败"
            );
        }
        MarketFeedSupervisorEvent::ProviderTaskFailed { provider, error } => {
            error!(
                provider = ?provider,
                error = %error,
                "行情订阅监督器记录到行情源任务失败"
            );
        }
    }
}

/// 在统一写超时内发送订阅、心跳或协议回复；超时和 sink 错误都结束当前周期，让外层 REST 兜底与退避重连接管。
/// 已发送成功的前序报文不会回滚，错误文本只包含操作类型，不记录订阅载荷或潜在凭据。
async fn send_market_feed_socket_message<S>(
    writer: &mut S,
    message: Message,
    operation: &'static str,
) -> AppResult<()>
where
    S: Sink<Message, Error = tungstenite::Error> + Unpin,
{
    timeout(MARKET_FEED_WRITE_TIMEOUT, writer.send(message))
        .await
        .map_err(|_| {
            crate::error::AppError::Internal(format!("market feed websocket {operation} timed out"))
        })?
        .map_err(|error| {
            crate::error::AppError::Internal(format!(
                "market feed websocket {operation} failed: {error}"
            ))
        })
}

/// 执行一次完整的供应商 WebSocket 周期：在连接超时内建连、逐条发送订阅消息，然后并行等待消息、心跳与静默截止。
/// 行情帧交给 ingestion 落库并广播，单帧写入失败只累加失败计数并告警，不中断本次连接。
/// Bitget 每 25 秒发送纯文本 ping；任意入站帧刷新 75 秒静默截止，超时、写失败、关闭或读错误都会结束本轮。
/// 周期结束前校验不能只收到失败帧且零写入；已经完成的 Redis、Mongo 写入和广播不会因重连而回滚。
fn market_feed_worker_for_generation(
    state: &AppState,
    generation: u64,
    fence: MarketFeedGenerationFence,
) -> AppResult<MarketFeedWorker<GenerationBoundMarketIngestionSink>> {
    let worker = MarketFeedWorker::new(GenerationBoundMarketIngestionSink::from_state(
        state, generation, fence,
    )?);
    Ok(match state.event_broadcast_hub.clone() {
        Some(hub) => worker.with_broadcast_hub(hub),
        None => worker,
    })
}

async fn run_provider_once_with_generation(
    state: AppState,
    config: MarketFeedConfig,
    generation: u64,
    fence: MarketFeedGenerationFence,
    cancellation: CancellationToken,
) -> AppResult<()> {
    let worker = market_feed_worker_for_generation(&state, generation, fence.clone())?;
    run_provider_socket(
        worker,
        config,
        MarketFeedWriteFence::new(generation, fence),
        cancellation,
    )
    .await
}

async fn run_provider_socket<S>(
    worker: MarketFeedWorker<S>,
    config: MarketFeedConfig,
    write_fence: MarketFeedWriteFence,
    cancellation: CancellationToken,
) -> AppResult<()>
where
    S: MarketIngestionSink,
{
    let connection = timeout(MARKET_FEED_CONNECT_TIMEOUT, connect_async(config.url()))
        .await
        .map_err(|_| {
            crate::error::AppError::Internal("market feed websocket connect timed out".to_owned())
        })?;
    let (socket, _) = connection.map_err(|error| {
        crate::error::AppError::Internal(format!("market feed websocket connect failed: {error}"))
    })?;
    let (mut writer, mut reader) = socket.split();
    for message in config.subscription_messages() {
        send_market_feed_socket_message(&mut writer, Message::Text(message.clone()), "subscribe")
            .await?;
    }
    let provider = config.provider();
    let mut liveness = MarketFeedSocketLiveness::new(provider);
    let mut summary = MarketFeedSummary::default();
    loop {
        let socket_event = tokio::select! {
            _ = cancellation.cancelled() => break,
            event = liveness.wait_next(&mut reader) => event,
        };
        let message = match socket_event {
            MarketFeedSocketEvent::Message(Some(message)) => message.map_err(|error| {
                crate::error::AppError::Internal(format!(
                    "market feed websocket read failed: {error}"
                ))
            })?,
            MarketFeedSocketEvent::Message(None) => break,
            MarketFeedSocketEvent::HeartbeatDue => {
                if let Some(message) = market_feed_heartbeat_message(liveness.provider) {
                    send_market_feed_socket_message(&mut writer, message, "heartbeat").await?;
                }
                continue;
            }
            MarketFeedSocketEvent::IdleTimeout => {
                return Err(crate::error::AppError::Internal(format!(
                    "market feed websocket inbound idle timeout after {} seconds",
                    liveness.idle_timeout.as_secs()
                )));
            }
        };
        liveness.record_inbound();
        match market_feed_socket_action(provider, message)? {
            MarketFeedSocketAction::Frame(frame) => {
                summary.received += 1;
                let result = async {
                    let _permit = write_fence.enter().await?;
                    worker.ingest_frame(&frame).await
                }
                .await;
                match result {
                    Ok(()) => summary.ingested += 1,
                    Err(error) => {
                        summary.failed += 1;
                        warn!(
                            provider = ?provider,
                            %error,
                            "行情帧写入失败"
                        );
                    }
                }
            }
            MarketFeedSocketAction::Reply(reply) => {
                send_market_feed_socket_message(&mut writer, reply, "reply").await?;
            }
            MarketFeedSocketAction::Ignore => {}
            MarketFeedSocketAction::Close => break,
        }
    }
    ensure_market_feed_cycle_has_valid_frames(&summary)?;
    info!(
        received = summary.received,
        ingested = summary.ingested,
        failed = summary.failed,
        "行情 WebSocket 周期完成"
    );
    Ok(())
}

/// 校验行情周期不能“只收到失败帧且零写入”；纯心跳或正常空周期允许由连接生命周期决定后续处理。
pub fn ensure_market_feed_cycle_has_valid_frames(summary: &MarketFeedSummary) -> AppResult<()> {
    if summary.failed > 0 && summary.ingested == 0 {
        return Err(crate::error::AppError::Validation(
            "market feed websocket cycle received only invalid frames".to_owned(),
        ));
    }
    Ok(())
}

/// 从字符串配置完成交易对/周期/供应商校验后进入多供应商持续监督；连接失败按至少 1 秒基准退避并优先尝试 WebSocket。
/// 未配置交易对时显式停用且不连接外网、不写缓存或生成假行情；运行中的持久化与广播时机由 ingestion 合同决定。
pub async fn run_loop(
    state: AppState,
    symbols: Vec<String>,
    intervals: Vec<String>,
    providers: Vec<String>,
    reconnect_seconds: u64,
) -> AppResult<()> {
    let config = MarketFeedRuntimeConfig::new(
        &state.settings,
        symbols,
        intervals,
        providers,
        reconnect_seconds,
    )?;
    if !config.enabled() {
        info!("行情 WebSocket 循环已禁用：未配置交易对");
        return Ok(());
    }
    run_config_loop(state, config).await
}

/// 把配置里的供应商代码解析为枚举并按首次出现顺序去重，配置为空时回退到默认供应商集合。
/// 未知代码直接返回校验错误而不是静默丢弃，避免配置写错后系统只订阅了部分供应商却看起来一切正常。
fn market_feed_providers(providers: Vec<String>) -> AppResult<Vec<MarketFeedProvider>> {
    if providers.is_empty() {
        return Ok(MarketFeedProvider::default_providers().to_vec());
    }

    let mut selected = Vec::new();
    for provider in providers {
        let provider = MarketFeedProvider::from_code(&provider)?;
        if !selected.contains(&provider) {
            selected.push(provider);
        }
    }
    Ok(selected)
}

/// 用载荷子串匹配判断帧属于哪个频道，按 K 线、深度、逐笔成交、ticker 的顺序依次尝试。
/// 顺序即优先级：同时含有多个关键字的载荷会归入先匹配到的频道，因此调整判断顺序会改变分发结果。
/// 判断只看文本而不解析结构，无法归类时返回 `None` 频道，调用方据此忽略该消息而不是报错。
fn channel_from_payload(payload: &str) -> MarketFeedChannel {
    if payload.contains("kline") || payload.contains("candle") {
        // info!("进入Kline:\npayload--->{}", payload);
        MarketFeedChannel::Kline
    } else if payload.contains("depth")
        || payload.contains("books")
        || payload.contains("\"level2\"")
        || payload.contains("\"l2_data\"")
    {
        // info!("进入Depth:\npayload--->{}", payload);
        MarketFeedChannel::Depth
    } else if payload.contains("trade") {
        // info!("进入Trade:\npayload--->{}", payload);
        MarketFeedChannel::Trade
    } else if payload.contains("ticker") || payload.contains("detail") {
        // info!("进入Ticker:\npayload--->{}", payload);
        MarketFeedChannel::Ticker
    } else {
        MarketFeedChannel::None
    }
}

#[cfg(test)]
#[path = "../../tests/unit_src/src_workers_market_feed_tests.rs"]
mod tests;
