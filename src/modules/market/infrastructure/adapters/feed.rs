//! 行情 feed 编排基础设施。
//!
//! 负责 WebSocket/REST 原始帧的有限流处理、失败汇总、持久化后广播和 provider 配置组装；
//! 单帧失败保持隔离，只有 ingestion 成功的行情才进入公开实时事件。

use super::{
    ingestion::{MarketIngestionService, MarketIngestionSink},
    provider::{
        BitgetMarketAdapter, CoinbaseMarketAdapter, HtxMarketAdapter, MarketFeedProvider,
        bitget_rest_kline_payloads, bitget_rest_ticker_payload, coinbase_rest_kline_payloads,
        coinbase_rest_ticker_payload, htx_rest_kline_payloads, htx_rest_ticker_payload,
        market_feed_payload_hash, provider_name, validation_error,
    },
};
use crate::{
    config::Settings,
    error::{AppError, AppResult},
    modules::{
        events::{EventBroadcastHub, EventBroadcastMessage, EventOutboxWriter},
        market::{
            KlineUpsertKey, MarketDepthSnapshot, MarketKlineSnapshot, MarketTickerSnapshot,
            MarketTradeTick, ValidatedMarketSymbol,
        },
    },
    state::AppState,
};
use axum::async_trait;
use chrono::Utc;
use futures_util::{Stream, StreamExt};
use serde_json::{Value, json};
use std::collections::VecDeque;

#[async_trait]
pub trait MarketFeedRestFallbackHttpClient: Clone + Send + Sync + 'static {
    /// 在受控超时内读取 REST 兜底响应正文；非成功状态或网络错误不得伪装为空行情。
    async fn get_text(&self, url: &str) -> AppResult<String>;
}

const REST_FALLBACK_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

#[derive(Clone)]
pub struct ReqwestMarketFeedRestFallbackHttpClient {
    client: reqwest::Client,
    timeout: std::time::Duration,
}

impl Default for ReqwestMarketFeedRestFallbackHttpClient {
    fn default() -> Self {
        Self::with_timeout(REST_FALLBACK_REQUEST_TIMEOUT)
    }
}

impl ReqwestMarketFeedRestFallbackHttpClient {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            timeout: REST_FALLBACK_REQUEST_TIMEOUT,
        }
    }

    pub fn with_timeout(timeout: std::time::Duration) -> Self {
        Self {
            client: reqwest::Client::new(),
            timeout,
        }
    }

    pub fn from_settings(settings: &Settings) -> Self {
        Self::with_timeout(std::time::Duration::from_secs(
            settings.market_feed_rest_fallback_timeout_seconds,
        ))
    }

    pub fn timeout(&self) -> std::time::Duration {
        self.timeout
    }
}

#[async_trait]
impl MarketFeedRestFallbackHttpClient for ReqwestMarketFeedRestFallbackHttpClient {
    async fn get_text(&self, url: &str) -> AppResult<String> {
        let response = self
            .client
            .get(url)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|error| {
                AppError::Internal(format!("market feed REST fallback request failed: {error}"))
            })?
            .error_for_status()
            .map_err(|error| {
                AppError::Internal(format!("market feed REST fallback status failed: {error}"))
            })?;
        response.text().await.map_err(|error| {
            AppError::Internal(format!("market feed REST fallback body failed: {error}"))
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketFeedChannel {
    Ticker,
    Depth,
    Kline,
    Trade,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketFeedFrame {
    provider: MarketFeedProvider,
    channel: MarketFeedChannel,
    payload: String,
}

impl MarketFeedFrame {
    pub fn new(
        provider: MarketFeedProvider,
        channel: MarketFeedChannel,
        payload: impl Into<String>,
    ) -> Self {
        Self {
            provider,
            channel,
            payload: payload.into(),
        }
    }

    pub fn bitget_ticker(payload: impl Into<String>) -> Self {
        Self::new(
            MarketFeedProvider::Bitget,
            MarketFeedChannel::Ticker,
            payload,
        )
    }

    pub fn bitget_depth(payload: impl Into<String>) -> Self {
        Self::new(
            MarketFeedProvider::Bitget,
            MarketFeedChannel::Depth,
            payload,
        )
    }

    pub fn bitget_kline(payload: impl Into<String>) -> Self {
        Self::new(
            MarketFeedProvider::Bitget,
            MarketFeedChannel::Kline,
            payload,
        )
    }

    pub fn bitget_trade(payload: impl Into<String>) -> Self {
        Self::new(
            MarketFeedProvider::Bitget,
            MarketFeedChannel::Trade,
            payload,
        )
    }

    pub fn htx_ticker(payload: impl Into<String>) -> Self {
        Self::new(MarketFeedProvider::Htx, MarketFeedChannel::Ticker, payload)
    }

    pub fn htx_depth(payload: impl Into<String>) -> Self {
        Self::new(MarketFeedProvider::Htx, MarketFeedChannel::Depth, payload)
    }

    pub fn htx_kline(payload: impl Into<String>) -> Self {
        Self::new(MarketFeedProvider::Htx, MarketFeedChannel::Kline, payload)
    }

    pub fn htx_trade(payload: impl Into<String>) -> Self {
        Self::new(MarketFeedProvider::Htx, MarketFeedChannel::Trade, payload)
    }

    pub fn coinbase_ticker(payload: impl Into<String>) -> Self {
        Self::new(
            MarketFeedProvider::Coinbase,
            MarketFeedChannel::Ticker,
            payload,
        )
    }

    pub fn coinbase_depth(payload: impl Into<String>) -> Self {
        Self::new(
            MarketFeedProvider::Coinbase,
            MarketFeedChannel::Depth,
            payload,
        )
    }

    pub fn coinbase_kline(payload: impl Into<String>) -> Self {
        Self::new(
            MarketFeedProvider::Coinbase,
            MarketFeedChannel::Kline,
            payload,
        )
    }

    pub fn coinbase_trade(payload: impl Into<String>) -> Self {
        Self::new(
            MarketFeedProvider::Coinbase,
            MarketFeedChannel::Trade,
            payload,
        )
    }

    pub fn provider(&self) -> MarketFeedProvider {
        self.provider
    }

    pub fn channel(&self) -> MarketFeedChannel {
        self.channel
    }

    pub fn payload(&self) -> &str {
        &self.payload
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketFeedConfig {
    provider: MarketFeedProvider,
    url: String,
    subscription_messages: Vec<String>,
    symbols: Vec<String>,
    intervals: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketFeedRestFallbackTickerRequest {
    symbol: String,
    url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketFeedRestFallbackKlineRequest {
    symbol: String,
    interval: String,
    url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketFeedRestFallbackConfig {
    provider: MarketFeedProvider,
    ticker_requests: Vec<MarketFeedRestFallbackTickerRequest>,
    kline_requests: Vec<MarketFeedRestFallbackKlineRequest>,
}

impl MarketFeedRestFallbackTickerRequest {
    pub fn new(symbol: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            url: url.into(),
        }
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub fn url(&self) -> &str {
        &self.url
    }
}

impl MarketFeedRestFallbackKlineRequest {
    pub fn new(
        symbol: impl Into<String>,
        interval: impl Into<String>,
        url: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            interval: interval.into(),
            url: url.into(),
        }
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub fn interval(&self) -> &str {
        &self.interval
    }

    pub fn url(&self) -> &str {
        &self.url
    }
}

impl MarketFeedRestFallbackConfig {
    pub fn new(
        provider: MarketFeedProvider,
        ticker_requests: Vec<MarketFeedRestFallbackTickerRequest>,
        kline_requests: Vec<MarketFeedRestFallbackKlineRequest>,
    ) -> Self {
        Self {
            provider,
            ticker_requests,
            kline_requests,
        }
    }

    pub fn provider(&self) -> MarketFeedProvider {
        self.provider
    }

    pub fn ticker_requests(&self) -> &[MarketFeedRestFallbackTickerRequest] {
        &self.ticker_requests
    }

    pub fn ticker_url(&self) -> &str {
        self.ticker_requests
            .first()
            .map(MarketFeedRestFallbackTickerRequest::url)
            .unwrap_or_default()
    }

    pub fn ticker_urls(&self) -> Vec<String> {
        self.ticker_requests
            .iter()
            .map(|request| request.url.clone())
            .collect()
    }

    pub fn kline_requests(&self) -> &[MarketFeedRestFallbackKlineRequest] {
        &self.kline_requests
    }

    pub fn kline_urls(&self) -> Vec<String> {
        self.kline_requests
            .iter()
            .map(|request| request.url.clone())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MarketFeedRestFallbackFrameRequest {
    channel: MarketFeedChannel,
    symbol: String,
    interval: Option<String>,
    url: String,
}

impl MarketFeedRestFallbackFrameRequest {
    fn ticker(request: &MarketFeedRestFallbackTickerRequest) -> Self {
        Self {
            channel: MarketFeedChannel::Ticker,
            symbol: request.symbol().to_owned(),
            interval: None,
            url: request.url().to_owned(),
        }
    }

    fn kline(request: &MarketFeedRestFallbackKlineRequest) -> Self {
        Self {
            channel: MarketFeedChannel::Kline,
            symbol: request.symbol().to_owned(),
            interval: Some(request.interval().to_owned()),
            url: request.url().to_owned(),
        }
    }
}

struct MarketFeedRestFallbackFrameResult {
    request: MarketFeedRestFallbackFrameRequest,
    result: Result<MarketFeedFrame, AppError>,
}

impl MarketFeedRestFallbackFrameResult {
    fn new(
        request: &MarketFeedRestFallbackFrameRequest,
        result: Result<MarketFeedFrame, AppError>,
    ) -> Self {
        Self {
            request: request.clone(),
            result,
        }
    }
}

impl MarketFeedConfig {
    pub fn new(
        provider: MarketFeedProvider,
        url: impl Into<String>,
        subscription_messages: Vec<String>,
        symbols: Vec<String>,
        intervals: Vec<String>,
    ) -> Self {
        Self {
            provider,
            url: url.into(),
            subscription_messages,
            symbols,
            intervals,
        }
    }

    pub fn provider(&self) -> MarketFeedProvider {
        self.provider
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn subscription_messages(&self) -> &[String] {
        &self.subscription_messages
    }

    pub fn symbols(&self) -> &[String] {
        &self.symbols
    }

    pub fn intervals(&self) -> &[String] {
        &self.intervals
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketFeedFailureContext {
    provider: MarketFeedProvider,
    channel: MarketFeedChannel,
    symbol: String,
    interval: Option<String>,
    url: String,
    error: String,
}

impl MarketFeedFailureContext {
    fn new(
        provider: MarketFeedProvider,
        request: &MarketFeedRestFallbackFrameRequest,
        error: &AppError,
    ) -> Self {
        Self {
            provider,
            channel: request.channel,
            symbol: request.symbol.clone(),
            interval: request.interval.clone(),
            url: request.url.clone(),
            error: error.to_string(),
        }
    }

    pub fn provider(&self) -> MarketFeedProvider {
        self.provider
    }

    pub fn channel(&self) -> MarketFeedChannel {
        self.channel
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub fn interval(&self) -> Option<&str> {
        self.interval.as_deref()
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn error(&self) -> &str {
        &self.error
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MarketFeedSummary {
    pub received: u32,
    pub ingested: u32,
    pub failed: u32,
    failure_contexts: Vec<MarketFeedFailureContext>,
}

impl MarketFeedSummary {
    pub fn new(received: u32, ingested: u32, failed: u32) -> Self {
        Self {
            received,
            ingested,
            failed,
            failure_contexts: Vec::new(),
        }
    }

    fn record_failure(&mut self) {
        self.failed += 1;
    }

    fn record_failure_context(&mut self, context: MarketFeedFailureContext) {
        self.record_failure();
        self.failure_contexts.push(context);
    }

    pub fn failure_contexts(&self) -> &[MarketFeedFailureContext] {
        &self.failure_contexts
    }
}

#[derive(Clone)]
pub struct MarketFeedWorker<S> {
    sink: S,
    broadcast_hub: Option<EventBroadcastHub>,
}

impl<S> MarketFeedWorker<S> {
    pub fn new(sink: S) -> Self {
        Self {
            sink,
            broadcast_hub: None,
        }
    }

    pub fn with_broadcast_hub(mut self, hub: EventBroadcastHub) -> Self {
        self.broadcast_hub = Some(hub);
        self
    }

    pub fn with_outbox_writer<N>(self, _outbox_writer: EventOutboxWriter<N>) -> Self {
        self
    }

    pub fn provider_configs(
        settings: &Settings,
        symbols: &[&str],
        intervals: &[&str],
    ) -> AppResult<Vec<MarketFeedConfig>> {
        Self::provider_configs_for(
            settings,
            &MarketFeedProvider::default_providers(),
            symbols,
            intervals,
        )
    }

    pub fn provider_configs_for(
        settings: &Settings,
        providers: &[MarketFeedProvider],
        symbols: &[&str],
        intervals: &[&str],
    ) -> AppResult<Vec<MarketFeedConfig>> {
        if providers.is_empty() {
            return Err(AppError::Validation(
                "market feed providers are required".to_owned(),
            ));
        }
        let symbols = validate_feed_symbols(symbols)?;
        let intervals = validate_feed_intervals(intervals)?;
        providers
            .iter()
            .map(|provider| provider.feed_config(settings, &symbols, &intervals))
            .collect()
    }

    pub fn provider_rest_fallback_configs(
        settings: &Settings,
        symbols: &[&str],
        intervals: &[&str],
    ) -> AppResult<Vec<MarketFeedRestFallbackConfig>> {
        Self::provider_rest_fallback_configs_for(
            settings,
            &MarketFeedProvider::default_providers(),
            symbols,
            intervals,
        )
    }

    pub fn provider_rest_fallback_configs_for(
        settings: &Settings,
        providers: &[MarketFeedProvider],
        symbols: &[&str],
        intervals: &[&str],
    ) -> AppResult<Vec<MarketFeedRestFallbackConfig>> {
        if providers.is_empty() {
            return Err(AppError::Validation(
                "market feed providers are required".to_owned(),
            ));
        }
        let symbols = validate_feed_symbols(symbols)?;
        let intervals = validate_feed_intervals(intervals)?;
        providers
            .iter()
            .map(|provider| provider.rest_fallback_config(settings, &symbols, &intervals))
            .collect()
    }
}

impl MarketFeedWorker<MarketIngestionService> {
    /// 从应用状态构造生产 ingestion worker；Redis 与 Mongo 缺一不可，MySQL/事件总线作为撮合与广播副作用按配置接入。
    pub fn from_state(state: &AppState) -> AppResult<Self> {
        let worker = Self::new(MarketIngestionService::from_state(state)?);
        Ok(match state.event_broadcast_hub.clone() {
            Some(hub) => worker.with_broadcast_hub(hub),
            None => worker,
        })
    }
}

impl<S> MarketFeedWorker<S>
where
    S: MarketIngestionSink,
{
    /// 消费有限行情帧流并逐帧解析、持久化和广播；单帧失败只计数，调用方依据汇总决定周期是否有效。
    pub async fn run_stream<E, St>(&self, frames: St) -> AppResult<MarketFeedSummary>
    where
        E: ToString,
        St: Stream<Item = Result<MarketFeedFrame, E>> + Send,
    {
        futures_util::pin_mut!(frames);
        let mut summary = MarketFeedSummary::default();

        while let Some(frame) = frames.next().await {
            summary.received += 1;
            match frame {
                Ok(frame) => match self.ingest_frame(&frame).await {
                    Ok(()) => summary.ingested += 1,
                    Err(_) => summary.record_failure(),
                },
                Err(_) => summary.record_failure(),
            }
        }

        Ok(summary)
    }

    /// 执行一个供应商 REST 兜底配置并逐请求归一化、持久化行情；请求或帧错误记录上下文但继续其他请求。
    /// 返回汇总不代表价格源可用，调用方必须再执行“至少一个有效写入”校验。
    pub async fn run_rest_fallback_config<C>(
        &self,
        config: &MarketFeedRestFallbackConfig,
        http_client: &C,
    ) -> AppResult<MarketFeedSummary>
    where
        C: MarketFeedRestFallbackHttpClient,
    {
        let frames = fetch_rest_fallback_frames(config, http_client).await?;
        let mut summary = MarketFeedSummary::default();
        for frame in frames {
            summary.received += 1;
            match frame.result {
                Ok(frame) => match self.ingest_frame(&frame).await {
                    Ok(()) => summary.ingested += 1,
                    Err(_) => summary.record_failure(),
                },
                Err(error) => summary.record_failure_context(MarketFeedFailureContext::new(
                    config.provider(),
                    &frame.request,
                    &error,
                )),
            }
        }
        Ok(summary)
    }

    /// 将供应商原始帧解析为领域快照，交给对应 sink 持久化，成功后才发布实时市场事件。
    /// trade 帧目前只发布而不写行情存储；解析或 sink 失败时不得广播，避免消费者看到未持久化价格。
    pub async fn ingest_frame(&self, frame: &MarketFeedFrame) -> AppResult<()> {
        let parsed = parse_feed_frame(frame)?;
        let event = MarketFeedEvent::from_parsed(&parsed)?;
        match &parsed {
            ParsedMarketFeed::Ticker(snapshot) => self.sink.ingest_ticker(snapshot).await?,
            ParsedMarketFeed::Depth(snapshot) => self.sink.ingest_depth(snapshot).await?,
            ParsedMarketFeed::Kline(snapshot) => self.sink.ingest_kline(snapshot).await?,
            ParsedMarketFeed::Trade(_) => {}
        }
        if let Some(hub) = &self.broadcast_hub {
            hub.publish(EventBroadcastMessage::from_market_feed_event(&event)?);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketFeedEvent {
    aggregate_type: String,
    aggregate_id: String,
    event_type: String,
    routing_key: String,
    idempotency_key: String,
    public_ws_namespace: String,
    public_ws_topic: String,
    payload: Value,
}

impl MarketFeedEvent {
    pub fn from_frame(frame: &MarketFeedFrame) -> AppResult<Self> {
        let parsed = parse_feed_frame(frame)?;
        Self::from_parsed(&parsed)
    }

    fn from_parsed(parsed: &ParsedMarketFeed) -> AppResult<Self> {
        match parsed {
            ParsedMarketFeed::Ticker(snapshot) => Ok(Self {
                aggregate_type: "market_ticker".to_owned(),
                aggregate_id: snapshot.symbol().to_owned(),
                event_type: "ticker_updated".to_owned(),
                routing_key: format!("market.{}.ticker", snapshot.symbol()),
                idempotency_key: format!(
                    "market_feed:{}:{}:ticker:{}",
                    provider_name(snapshot.provider()),
                    snapshot.symbol(),
                    snapshot.observed_at().timestamp_millis()
                ),
                public_ws_namespace: "ticker".to_owned(),
                public_ws_topic: snapshot.symbol().to_owned(),
                payload: json!({
                    "symbol": snapshot.symbol(),
                    "last_price": snapshot.last_price().to_string(),
                    "high_24h": snapshot.high_24h().to_string(),
                    "low_24h": snapshot.low_24h().to_string(),
                    "volume_24h": snapshot.volume_24h().to_string(),
                    "price_change_24h": snapshot.price_change_24h().to_string(),
                    "price_change_percent_24h": snapshot.price_change_percent_24h().to_string(),
                    "observed_at": snapshot.observed_at().timestamp_millis(),
                    "provider": provider_name(snapshot.provider()),
                }),
            }),
            ParsedMarketFeed::Depth(snapshot) => Ok(Self {
                aggregate_type: "market_depth".to_owned(),
                aggregate_id: snapshot.symbol().to_owned(),
                event_type: "depth_updated".to_owned(),
                routing_key: format!("market.{}.depth", snapshot.symbol()),
                idempotency_key: format!(
                    "market_feed:{}:{}:depth:{}",
                    provider_name(snapshot.provider()),
                    snapshot.symbol(),
                    snapshot.observed_at().timestamp_millis()
                ),
                public_ws_namespace: "depth".to_owned(),
                public_ws_topic: snapshot.symbol().to_owned(),
                payload: json!({
                    "symbol": snapshot.symbol(),
                    "bids": snapshot.bids(),
                    "asks": snapshot.asks(),
                    "observed_at": snapshot.observed_at().timestamp_millis(),
                    "provider": provider_name(snapshot.provider()),
                }),
            }),
            ParsedMarketFeed::Kline(snapshot) => Ok(Self {
                aggregate_type: "market_kline".to_owned(),
                aggregate_id: format!("{}:{}", snapshot.symbol(), snapshot.interval()),
                event_type: "kline_updated".to_owned(),
                routing_key: format!("market.{}.kline.{}", snapshot.symbol(), snapshot.interval()),
                idempotency_key: format!(
                    "market_feed:{}:{}:kline:{}:{}:{}",
                    provider_name(snapshot.provider()),
                    snapshot.symbol(),
                    snapshot.interval(),
                    snapshot.open_time().timestamp_millis(),
                    market_feed_payload_hash(&json!({
                        "open": snapshot.open().to_string(),
                        "high": snapshot.high().to_string(),
                        "low": snapshot.low().to_string(),
                        "close": snapshot.close().to_string(),
                        "volume": snapshot.volume().to_string(),
                        "observed_at": snapshot.observed_at().timestamp_millis(),
                    }))
                ),
                public_ws_namespace: "kline".to_owned(),
                public_ws_topic: format!("{}_{}", snapshot.symbol(), snapshot.interval()),
                payload: json!({
                    "symbol": snapshot.symbol(),
                    "interval": snapshot.interval(),
                    "open_time": snapshot.open_time().timestamp_millis(),
                    "open": snapshot.open().to_string(),
                    "high": snapshot.high().to_string(),
                    "low": snapshot.low().to_string(),
                    "close": snapshot.close().to_string(),
                    "volume": snapshot.volume().to_string(),
                    "observed_at": snapshot.observed_at().timestamp_millis(),
                    "provider": provider_name(snapshot.provider()),
                }),
            }),
            ParsedMarketFeed::Trade(tick) => Ok(Self {
                aggregate_type: "market_trade".to_owned(),
                aggregate_id: tick.trade_id().to_owned(),
                event_type: "trade_created".to_owned(),
                routing_key: format!("market.{}.trade", tick.symbol()),
                idempotency_key: format!(
                    "market_feed:{}:{}:trade:{}",
                    provider_name(tick.provider()),
                    tick.symbol(),
                    tick.trade_id()
                ),
                public_ws_namespace: "trade".to_owned(),
                public_ws_topic: tick.symbol().to_owned(),
                payload: json!({
                    "symbol": tick.symbol(),
                    "trade_id": tick.trade_id(),
                    "side": tick.side(),
                    "price": tick.price().to_string(),
                    "quantity": tick.quantity().to_string(),
                    "traded_at": tick.traded_at().timestamp_millis(),
                    "provider": provider_name(tick.provider()),
                }),
            }),
        }
    }

    pub fn aggregate_type(&self) -> &str {
        &self.aggregate_type
    }

    pub fn aggregate_id(&self) -> &str {
        &self.aggregate_id
    }

    pub fn event_type(&self) -> &str {
        &self.event_type
    }

    pub fn routing_key(&self) -> &str {
        &self.routing_key
    }

    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }

    pub fn public_ws_namespace(&self) -> &str {
        &self.public_ws_namespace
    }

    pub fn public_ws_topic(&self) -> &str {
        &self.public_ws_topic
    }

    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

enum ParsedMarketFeed {
    Ticker(MarketTickerSnapshot),
    Depth(MarketDepthSnapshot),
    Kline(MarketKlineSnapshot),
    Trade(MarketTradeTick),
}

async fn fetch_rest_fallback_frames<C>(
    config: &MarketFeedRestFallbackConfig,
    http_client: &C,
) -> AppResult<Vec<MarketFeedRestFallbackFrameResult>>
where
    C: MarketFeedRestFallbackHttpClient,
{
    let mut requests =
        VecDeque::with_capacity(config.ticker_requests().len() + config.kline_requests().len());
    requests.extend(
        config
            .ticker_requests()
            .iter()
            .map(MarketFeedRestFallbackFrameRequest::ticker),
    );
    requests.extend(
        config
            .kline_requests()
            .iter()
            .map(MarketFeedRestFallbackFrameRequest::kline),
    );

    let mut frames = Vec::with_capacity(requests.len());
    while let Some(request) = requests.pop_front() {
        match http_client.get_text(&request.url).await {
            Ok(payload) => match rest_fallback_frames(config.provider(), &request, &payload) {
                Ok(payload_frames) => frames.extend(
                    payload_frames
                        .into_iter()
                        .map(|frame| MarketFeedRestFallbackFrameResult::new(&request, Ok(frame))),
                ),
                Err(error) => {
                    frames.push(MarketFeedRestFallbackFrameResult::new(&request, Err(error)))
                }
            },
            Err(error) => frames.push(MarketFeedRestFallbackFrameResult::new(&request, Err(error))),
        }
    }
    Ok(frames)
}

fn rest_fallback_frames(
    provider: MarketFeedProvider,
    request: &MarketFeedRestFallbackFrameRequest,
    payload: &str,
) -> AppResult<Vec<MarketFeedFrame>> {
    let channel = request.channel;
    let payloads = match (provider, channel) {
        (MarketFeedProvider::Bitget, MarketFeedChannel::Ticker) => {
            vec![bitget_rest_ticker_payload(payload, &request.symbol)?]
        }
        (MarketFeedProvider::Bitget, MarketFeedChannel::Kline) => bitget_rest_kline_payloads(
            payload,
            &request.symbol,
            required_rest_fallback_interval(request)?,
        )?,
        (MarketFeedProvider::Htx, MarketFeedChannel::Ticker) => {
            vec![htx_rest_ticker_payload(payload, &request.symbol)?]
        }
        (MarketFeedProvider::Htx, MarketFeedChannel::Kline) => htx_rest_kline_payloads(
            payload,
            &request.symbol,
            required_rest_fallback_interval(request)?,
        )?,
        (MarketFeedProvider::Coinbase, MarketFeedChannel::Ticker) => {
            vec![coinbase_rest_ticker_payload(payload, &request.symbol)?]
        }
        (MarketFeedProvider::Coinbase, MarketFeedChannel::Kline) => coinbase_rest_kline_payloads(
            payload,
            &request.symbol,
            required_rest_fallback_interval(request)?,
        )?,
        (_, MarketFeedChannel::Depth | MarketFeedChannel::Trade | MarketFeedChannel::None) => {
            return Err(AppError::Validation(
                "unsupported market feed REST fallback channel".to_owned(),
            ));
        }
    };
    Ok(payloads
        .into_iter()
        .map(|payload| MarketFeedFrame::new(provider, channel, payload))
        .collect())
}

fn required_rest_fallback_interval(
    request: &MarketFeedRestFallbackFrameRequest,
) -> AppResult<&str> {
    request.interval.as_deref().ok_or_else(|| {
        AppError::Validation("market feed REST fallback interval is required".to_owned())
    })
}

fn parse_feed_frame(frame: &MarketFeedFrame) -> AppResult<ParsedMarketFeed> {
    match (frame.provider(), frame.channel()) {
        (MarketFeedProvider::Bitget, MarketFeedChannel::Ticker) => {
            BitgetMarketAdapter::ticker_from_ws(frame.payload()).map(ParsedMarketFeed::Ticker)
        }
        (MarketFeedProvider::Bitget, MarketFeedChannel::Depth) => {
            BitgetMarketAdapter::depth_from_ws(frame.payload()).map(ParsedMarketFeed::Depth)
        }
        (MarketFeedProvider::Bitget, MarketFeedChannel::Kline) => {
            BitgetMarketAdapter::kline_from_ws(frame.payload()).map(ParsedMarketFeed::Kline)
        }
        (MarketFeedProvider::Bitget, MarketFeedChannel::Trade) => {
            BitgetMarketAdapter::trade_from_ws(frame.payload()).map(ParsedMarketFeed::Trade)
        }
        (MarketFeedProvider::Htx, MarketFeedChannel::Ticker) => {
            HtxMarketAdapter::ticker_from_ws(frame.payload()).map(ParsedMarketFeed::Ticker)
        }
        (MarketFeedProvider::Htx, MarketFeedChannel::Depth) => {
            HtxMarketAdapter::depth_from_ws(frame.payload()).map(ParsedMarketFeed::Depth)
        }
        (MarketFeedProvider::Htx, MarketFeedChannel::Kline) => {
            HtxMarketAdapter::kline_from_ws(frame.payload()).map(ParsedMarketFeed::Kline)
        }
        (MarketFeedProvider::Htx, MarketFeedChannel::Trade) => {
            HtxMarketAdapter::trade_from_ws(frame.payload()).map(ParsedMarketFeed::Trade)
        }
        (MarketFeedProvider::Coinbase, MarketFeedChannel::Ticker) => {
            CoinbaseMarketAdapter::ticker_from_ws(frame.payload()).map(ParsedMarketFeed::Ticker)
        }
        (MarketFeedProvider::Coinbase, MarketFeedChannel::Depth) => {
            CoinbaseMarketAdapter::depth_from_ws(frame.payload()).map(ParsedMarketFeed::Depth)
        }
        (MarketFeedProvider::Coinbase, MarketFeedChannel::Kline) => {
            CoinbaseMarketAdapter::kline_from_ws(frame.payload()).map(ParsedMarketFeed::Kline)
        }
        (MarketFeedProvider::Coinbase, MarketFeedChannel::Trade) => {
            CoinbaseMarketAdapter::trade_from_ws(frame.payload()).map(ParsedMarketFeed::Trade)
        }
        (_, MarketFeedChannel::None) => Err(AppError::Validation(
            "unsupported market feed channel".to_owned(),
        )),
    }
}

fn validate_feed_symbols(symbols: &[&str]) -> AppResult<Vec<String>> {
    if symbols.is_empty() {
        return Err(AppError::Validation(
            "market feed symbols are required".to_owned(),
        ));
    }

    symbols
        .iter()
        .map(|symbol| {
            ValidatedMarketSymbol::from_raw(symbol)
                .map(|symbol| symbol.as_str().to_owned())
                .map_err(validation_error)
        })
        .collect()
}

fn validate_feed_intervals(intervals: &[&str]) -> AppResult<Vec<String>> {
    intervals
        .iter()
        .map(|interval| {
            KlineUpsertKey::new(*interval, Utc::now())
                .map(|key| key.interval().to_owned())
                .map_err(validation_error)
        })
        .collect()
}
