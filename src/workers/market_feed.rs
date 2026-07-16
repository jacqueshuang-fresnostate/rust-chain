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

    pub fn enabled(&self) -> bool {
        !self.symbols.is_empty()
    }

    pub fn symbols(&self) -> &[String] {
        &self.symbols
    }

    pub fn intervals(&self) -> &[String] {
        &self.intervals
    }

    pub fn providers(&self) -> &[MarketFeedProvider] {
        &self.providers
    }

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
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(MarketFeedSupervisorState {
                status: MarketFeedRuntimeStatus::default(),
                task: None,
            })),
        }
    }

    pub fn new_for_tests() -> Self {
        Self::new()
    }

    pub async fn status(&self) -> MarketFeedRuntimeStatus {
        self.state.read().await.status.clone()
    }

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

    pub async fn stop(&self) {
        let mut guard = self.state.write().await;
        if let Some(task) = guard.task.take() {
            task.abort();
        }
        guard.status.last_reload_status = Some("skipped".to_owned());
        guard.status.last_reload_error = None;
    }

    pub async fn record_failure(&self, error: String) -> MarketFeedRuntimeStatus {
        let mut guard = self.state.write().await;
        guard.status.last_reload_status = Some("failed".to_owned());
        guard.status.last_reload_error = Some(error);
        guard.status.clone()
    }

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
    fn default() -> Self {
        Self::new()
    }
}

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

fn field_as_string(value: &Value, key: &str) -> Option<String> {
    match value.get(key)? {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

pub async fn run_config_once(state: &AppState, config: &MarketFeedRuntimeConfig) -> AppResult<()> {
    if !config.enabled() {
        return Ok(());
    }
    let symbol_refs: Vec<&str> = config.symbols().iter().map(String::as_str).collect();
    let interval_refs: Vec<&str> = config.intervals().iter().map(String::as_str).collect();
    run_once_with_providers(state, config.providers(), &symbol_refs, &interval_refs).await
}

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

pub async fn run_once(state: &AppState, symbols: &[&str], intervals: &[&str]) -> AppResult<()> {
    run_once_with_providers(
        state,
        &MarketFeedProvider::default_providers(),
        symbols,
        intervals,
    )
    .await
}

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
    fn new(initial_delay: Duration) -> Self {
        Self {
            initial_delay,
            current_delay: initial_delay,
            max_delay: Duration::from_secs(60),
        }
    }

    fn next_delay(&self) -> Duration {
        self.current_delay
    }

    fn record_failure(&mut self) {
        self.current_delay = (self.current_delay * 2).min(self.max_delay);
    }

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
    fn new(config: MarketFeedRestFallbackConfig, build_worker: B, http_client: C) -> Self {
        Self {
            config,
            build_worker,
            http_client,
        }
    }
}

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

pub fn ensure_market_feed_cycle_has_valid_frames(summary: &MarketFeedSummary) -> AppResult<()> {
    if summary.failed > 0 && summary.ingested == 0 {
        return Err(crate::error::AppError::Validation(
            "market feed websocket cycle received only invalid frames".to_owned(),
        ));
    }
    Ok(())
}

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
