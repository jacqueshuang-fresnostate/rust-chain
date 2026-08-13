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
    error::AppResult,
    modules::market::adapters::{
        MarketFeedChannel, MarketFeedConfig, MarketFeedFrame, MarketFeedProvider,
        MarketFeedRestFallbackConfig, MarketFeedRestFallbackHttpClient, MarketFeedSummary,
        MarketFeedWorker, MarketIngestionService, MarketIngestionSink,
        ReqwestMarketFeedRestFallbackHttpClient,
    },
    state::AppState,
};
use flate2::read::GzDecoder;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{future::Future, io::Read, pin::Pin, sync::Arc};
use tokio::{
    sync::RwLock,
    task::JoinHandle,
    time::{Duration, sleep},
};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};

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
    pub symbols: Vec<String>,
    pub intervals: Vec<String>,
    pub providers: Vec<String>,
    pub last_reload_status: Option<String>,
    pub last_reload_error: Option<String>,
}

#[derive(Clone)]
pub struct MarketFeedSupervisorHandle {
    state: Arc<RwLock<MarketFeedSupervisorState>>,
}

struct MarketFeedSupervisorState {
    status: MarketFeedRuntimeStatus,
    task: Option<JoinHandle<()>>,
}

impl MarketFeedSupervisorHandle {
    /// 创建尚未应用任何版本且没有后台任务的行情监督器；首个有效配置必须经 reload 校验后才进入运行状态。
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(MarketFeedSupervisorState {
                status: MarketFeedRuntimeStatus::default(),
                task: None,
            })),
        }
    }

    /// 为测试建立与生产相同的空监督状态，但不隐式启动连接，便于验证配置版本、失败和停机状态迁移。
    pub fn new_for_tests() -> Self {
        Self::new()
    }

    /// 取得当前已应用配置版本及最近 reload 结果的一致快照，供管理端扫描状态；读取不会启动、停止或重试任务。
    pub async fn status(&self) -> MarketFeedRuntimeStatus {
        self.state.read().await.status.clone()
    }

    /// 应用一个版本化行情配置：先用 adapter 校验新订阅矩阵并启动新监督任务，再在写锁内中止旧任务、替换句柄并发布成功状态。
    /// 禁用配置直接停止旧任务并记 skipped；校验失败发生在替换前，旧任务继续运行，调用方可另行用 `record_failure` 暴露错误。
    pub async fn reload(
        &self,
        state: AppState,
        config: MarketFeedRuntimeConfig,
        version: u64,
    ) -> AppResult<MarketFeedRuntimeStatus> {
        if !config.enabled() {
            self.stop().await;
            let mut guard = self.state.write().await;
            guard.status = runtime_status_from_config(&config, version, "skipped", None);
            return Ok(guard.status.clone());
        }
        let startup_config = config.clone();
        MarketFeedWorker::<MarketIngestionService>::provider_configs_for(
            &state.settings,
            startup_config.providers(),
            &startup_config
                .symbols()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            &startup_config
                .intervals()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        )?;
        let worker_state = state.clone();
        let task_config = config.clone();
        let task = tokio::spawn(async move {
            if let Err(error) = run_config_loop(worker_state, task_config).await {
                tracing::error!(%error, "行情订阅受控循环已停止");
            }
        });
        let mut guard = self.state.write().await;
        if let Some(previous) = guard.task.take() {
            previous.abort();
        }
        guard.task = Some(task);
        guard.status = runtime_status_from_config(&config, version, "success", None);
        Ok(guard.status.clone())
    }

    /// 中止当前行情任务并把最近 reload 状态记为跳过；保留已应用版本和配置快照、清除旧错误，表达受控停机而非供应商故障。
    pub async fn stop(&self) {
        let mut guard = self.state.write().await;
        if let Some(task) = guard.task.take() {
            task.abort();
        }
        guard.status.last_reload_status = Some("skipped".to_owned());
        guard.status.last_reload_error = None;
    }

    /// 记录配置扫描或 reload 失败供管理端诊断；保留上一次已应用版本和任务句柄，不把一次扫描错误误当成已成功切换。
    pub async fn record_failure(&self, error: String) -> MarketFeedRuntimeStatus {
        let mut guard = self.state.write().await;
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
        let mut guard = self.state.write().await;
        guard.status = runtime_status_from_config(&config, version, "success", None);
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
    status: &str,
    error: Option<String>,
) -> MarketFeedRuntimeStatus {
    MarketFeedRuntimeStatus {
        applied_version: Some(version),
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
/// JSON 无法解析直接返回校验错误；带 `ping` 字段的应用层心跳在此原值回 `pong`，不再进入频道判定。
/// 只带 `event` 或 `op` 而没有 `data` 的控制帧一律忽略，避免把订阅回执当成行情写进缓存。
pub fn market_feed_text_action(
    provider: MarketFeedProvider,
    payload: &str,
) -> AppResult<MarketFeedTextAction> {
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
    let mut tasks = Vec::with_capacity(provider_configs.len());
    for (provider_config, rest_fallback_config) in
        provider_configs.into_iter().zip(rest_fallback_configs)
    {
        let state = state.clone();
        let provider = provider_config.provider();
        tasks.push(MarketFeedProviderTask::spawn(provider, async move {
            run_provider_reconnect_loop(
                state,
                provider_config,
                rest_fallback_config,
                reconnect_delay,
            )
            .await
        }));
    }

    await_market_feed_provider_tasks(tasks, emit_market_feed_supervisor_event).await
}

struct MarketFeedProviderTask {
    provider: MarketFeedProvider,
    handle: Pin<Box<tokio::task::JoinHandle<AppResult<()>>>>,
}

impl MarketFeedProviderTask {
    /// 把某个供应商的重连循环交给 tokio 执行，并把供应商标识与任务句柄绑定在一起。
    /// 绑定后即使任务异常终止也能定位到具体供应商；句柄固定在堆上，便于后续轮询它的完成状态。
    fn spawn<F>(provider: MarketFeedProvider, future: F) -> Self
    where
        F: Future<Output = AppResult<()>> + Send + 'static,
    {
        Self {
            provider,
            handle: Box::pin(tokio::spawn(future)),
        }
    }
}

/// 轮询等待多个供应商任务，只要有一个结束就取出它的结果，并把该结果作为整个等待的结果返回。
/// 轮询间隔固定 10 毫秒，用 swap_remove 摘出已完成项，因此剩余任务的相对顺序不保证稳定。
/// 正常运行时重连循环不会自行退出，任何一个结束都说明该供应商出了问题；本函数只等待，不主动中止其余任务。
async fn await_market_feed_provider_tasks<F>(
    mut tasks: Vec<MarketFeedProviderTask>,
    mut emit_event: F,
) -> AppResult<()>
where
    F: FnMut(MarketFeedSupervisorEvent),
{
    while !tasks.is_empty() {
        for index in 0..tasks.len() {
            if tasks[index].handle.as_mut().is_finished() {
                let task = tasks.swap_remove(index);
                return await_finished_market_feed_provider_task(task, &mut emit_event).await;
            }
        }
        sleep(Duration::from_millis(10)).await;
    }
    Ok(())
}

/// 取回已结束供应商任务的结果，并区分业务错误与 tokio 侧任务失败两种情况。
/// 任务自身返回的错误原样上抛；join 失败说明任务 panic 或被中止，此时先发出监督事件再包装成内部错误。
/// 事件里带上供应商标识，使日志能够直接定位是哪一家的连接任务异常终止。
async fn await_finished_market_feed_provider_task<F>(
    task: MarketFeedProviderTask,
    emit_event: &mut F,
) -> AppResult<()>
where
    F: FnMut(MarketFeedSupervisorEvent),
{
    match task.handle.await {
        Ok(result) => result,
        Err(error) => {
            let error = error.to_string();
            emit_event(MarketFeedSupervisorEvent::ProviderTaskFailed {
                provider: task.provider,
                error: error.clone(),
            });
            Err(crate::error::AppError::Internal(format!(
                "market feed provider task failed: {error}"
            )))
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
    let mut handles = Vec::with_capacity(configs.len());
    for config in configs {
        let state = state.clone();
        handles.push(tokio::spawn(async move {
            run_provider_once(state, config).await
        }));
    }

    for handle in handles {
        handle.await.map_err(|error| {
            crate::error::AppError::Internal(format!("market feed provider task failed: {error}"))
        })??;
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
) -> AppResult<()> {
    let http_client = rest_fallback_http_client(&state.settings);
    run_provider_reconnect_loop_with(
        state,
        config,
        reconnect_delay,
        run_provider_once,
        MarketFeedRestFallbackRuntime::new(
            rest_fallback_config,
            |state| async move { MarketFeedWorker::<MarketIngestionService>::from_state(&state) },
            http_client,
        ),
        emit_market_feed_supervisor_event,
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
}

impl<B, C> MarketFeedRestFallbackRuntime<B, C> {
    /// 打包 REST 兜底所需的三件套：请求清单配置、ingestion worker 构造器和 HTTP 客户端。
    /// 构造只是聚合依赖，既不发起请求也不判断清单是否为空，真正的触发条件由周期执行函数判断。
    fn new(config: MarketFeedRestFallbackConfig, build_worker: B, http_client: C) -> Self {
        Self {
            config,
            build_worker,
            http_client,
        }
    }
}

/// 单个供应商的无限重连主体：每轮先取当前退避延迟，再执行一次带 REST 兜底的行情周期。
/// 成功则上报周期成功事件并把延迟复位到基准；失败则上报失败事件、打印带供应商与延迟秒数的错误日志，
/// 并把下一轮延迟翻倍直到封顶 60 秒。无论成败都会在轮末等待该延迟，因此成功也不会形成忙循环。
/// 循环没有正常退出分支，只会随任务被中止而结束；本函数不判断兜底是否可用，也不改写行情写入结果。
/// 执行函数、worker 构造器、HTTP 客户端与事件回调都由调用方注入，便于在测试中替换而不接触真实网络。
async fn run_provider_reconnect_loop_with<F, Fut, B, BuildFut, C, E>(
    state: AppState,
    config: MarketFeedConfig,
    reconnect_delay: Duration,
    mut run_provider: F,
    mut fallback: MarketFeedRestFallbackRuntime<B, C>,
    mut emit_event: E,
) -> AppResult<()>
where
    F: FnMut(AppState, MarketFeedConfig) -> Fut,
    Fut: Future<Output = AppResult<()>>,
    B: FnMut(AppState) -> BuildFut,
    BuildFut: Future<Output = AppResult<MarketFeedWorker<MarketIngestionService>>>,
    C: MarketFeedRestFallbackHttpClient,
    E: FnMut(MarketFeedSupervisorEvent),
{
    let mut backoff = MarketFeedReconnectBackoff::new(reconnect_delay);
    loop {
        let delay = backoff.next_delay();
        match run_provider_cycle_with_rest_fallback(
            state.clone(),
            config.clone(),
            fallback.config.clone(),
            fallback.http_client.clone(),
            &mut run_provider,
            &mut fallback.build_worker,
        )
        .await
        {
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
        sleep(delay).await;
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

/// 执行一次完整的供应商 WebSocket 周期：建立连接、逐条发送订阅消息，然后持续读取并归一化消息。
/// 行情帧交给 ingestion 落库并广播，单帧写入失败只累加失败计数并告警，不中断本次连接。
/// 协议要求的回复原样写回连接，回写失败按内部错误终止周期；收到关闭帧或读到流末尾则退出读循环。
/// 周期结束前校验不能只收到失败帧且零写入，纯失败的周期按校验错误返回，交由外层退避后重连。
/// 连接、订阅与读取失败一律包成内部错误；已经完成的 Redis、Mongo 写入和广播不会因此回滚。
async fn run_provider_once(state: AppState, config: MarketFeedConfig) -> AppResult<()> {
    let worker = MarketFeedWorker::<MarketIngestionService>::from_state(&state)?;
    let (socket, _) = connect_async(config.url()).await.map_err(|error| {
        crate::error::AppError::Internal(format!("market feed websocket connect failed: {error}"))
    })?;
    let (mut writer, mut reader) = socket.split();
    for message in config.subscription_messages() {
        writer
            .send(Message::Text(message.clone()))
            .await
            .map_err(|error| {
                crate::error::AppError::Internal(format!("market feed subscribe failed: {error}"))
            })?;
    }
    let provider = config.provider();
    let mut summary = MarketFeedSummary::default();
    loop {
        let Some(message) = reader.next().await else {
            break;
        };
        let message = message.map_err(|error| {
            crate::error::AppError::Internal(format!("market feed websocket read failed: {error}"))
        })?;
        match market_feed_socket_action(provider, message)? {
            MarketFeedSocketAction::Frame(frame) => {
                summary.received += 1;
                match worker.ingest_frame(&frame).await {
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
                writer.send(reply).await.map_err(|error| {
                    crate::error::AppError::Internal(format!(
                        "market feed websocket reply failed: {error}"
                    ))
                })?;
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
