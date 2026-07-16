use axum::async_trait;
use chrono::Utc;
use exchange_api::{
    config::Settings,
    modules::{
        events::{
            EventBroadcastHub, EventBroadcastMessage, EventOutboxRepository, NewOutboxEvent,
            OutboxInsertResult, OutboxMessage, WebSocketChannel,
        },
        market::{
            MarketDepthSnapshot, MarketKlineSnapshot, MarketTickerSnapshot,
            adapters::{
                MarketFeedChannel, MarketFeedEvent, MarketFeedFrame, MarketFeedProvider,
                MarketFeedRestFallbackHttpClient, MarketFeedWorker, MarketIngestionSink,
                ReqwestMarketFeedRestFallbackHttpClient,
            },
        },
    },
    state::AppState,
    workers::market_feed::{
        MarketFeedRuntimeConfig, MarketFeedSocketAction, MarketFeedSupervisorHandle,
        MarketFeedTextAction, ensure_market_feed_cycle_has_valid_frames, market_feed_socket_action,
        market_feed_text_action, run_provider_cycle_with_rest_fallback,
    },
};
use flate2::{Compression, write::GzEncoder};
use futures_util::stream;
use secrecy::SecretString;
use std::{collections::HashMap, error::Error, io::Write, sync::Arc};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;

fn gzip_payload(payload: &str) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(payload.as_bytes()).unwrap();
    encoder.finish().unwrap()
}

fn test_settings() -> Settings {
    Settings {
        app_env: "test".to_owned(),
        app_host: "127.0.0.1".parse().unwrap(),
        app_port: 0,
        database_url: SecretString::new("mysql://test:test@localhost/test".to_owned()),
        mongodb_uri: SecretString::new("mongodb://localhost:27017".to_owned()),
        mongodb_database: "exchange_test".to_owned(),
        redis_url: SecretString::new("redis://localhost:6379".to_owned()),
        rabbitmq_url: SecretString::new("amqp://guest:guest@localhost:5672/%2f".to_owned()),
        jwt_secret: SecretString::new("test-secret".to_owned()),
        credential_encryption_key: Some(SecretString::new(
            "0123456789abcdef0123456789abcdef".to_owned(),
        )),
        jwt_access_ttl_seconds: 900,
        jwt_refresh_ttl_seconds: 2_592_000,
        bitget_rest_base_url: "https://bitget.test".to_owned(),
        bitget_ws_url: "wss://bitget.test/ws".to_owned(),
        htx_rest_base_url: "https://htx.test".to_owned(),
        htx_ws_url: "wss://htx.test/ws".to_owned(),
        coinbase_rest_base_url: "https://coinbase.test".to_owned(),
        coinbase_ws_url: "wss://coinbase.test/ws".to_owned(),
        market_feed_symbols: Vec::new(),
        market_feed_intervals: Vec::new(),
        market_feed_providers: Vec::new(),
        market_feed_reconnect_seconds: 5,
        market_feed_rest_fallback_timeout_seconds: 3,
        event_inbox_retry_scan_seconds: 10,
        event_outbox_publisher_enabled: true,
        event_outbox_publisher_interval_seconds: 5,
        unlock_scanner_enabled: true,
        unlock_scanner_interval_seconds: 10,
        unlock_scanner_batch_limit: 100,
        kline_recovery_enabled: true,
        kline_recovery_interval_seconds: 30,
        kline_recovery_batch_limit: 100,
        seconds_contract_settlement_enabled: true,
        seconds_contract_settlement_interval_seconds: 5,
        seconds_contract_settlement_batch_limit: 100,
        earn_auto_redemption_enabled: true,
        earn_auto_redemption_interval_seconds: 60,
        earn_auto_redemption_batch_limit: 100,
        margin_liquidation_enabled: true,
        margin_liquidation_interval_seconds: 5,
        margin_liquidation_batch_limit: 100,
        margin_interest_enabled: true,
        margin_interest_interval_seconds: 60,
        margin_interest_batch_limit: 100,
    }
}

#[derive(Clone, Default)]
struct RecordedIngestionSink {
    events: Arc<Mutex<Vec<String>>>,
}

#[derive(Clone, Default)]
struct RecordingOutboxRepository {
    events: Arc<Mutex<Vec<NewOutboxEvent>>>,
}

#[derive(Clone, Default)]
struct RecordedRestFallbackHttpClient {
    responses: Arc<HashMap<String, String>>,
}

impl RecordedRestFallbackHttpClient {
    fn new(responses: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            responses: Arc::new(responses.into_iter().collect()),
        }
    }
}

#[async_trait]
impl MarketFeedRestFallbackHttpClient for RecordedRestFallbackHttpClient {
    async fn get_text(&self, url: &str) -> exchange_api::error::AppResult<String> {
        self.responses.get(url).cloned().ok_or_else(|| {
            exchange_api::error::AppError::Internal(format!("missing response for {url}"))
        })
    }
}

#[async_trait]
impl EventOutboxRepository for RecordingOutboxRepository {
    async fn insert_event(
        &self,
        event: NewOutboxEvent,
    ) -> exchange_api::error::AppResult<OutboxInsertResult> {
        let mut events = self.events.lock().await;
        events.push(event);
        Ok(OutboxInsertResult::Inserted {
            id: events.len() as u64,
        })
    }

    async fn fetch_publishable_batch(
        &self,
        _limit: u32,
        _now: chrono::DateTime<Utc>,
    ) -> exchange_api::error::AppResult<Vec<OutboxMessage>> {
        Ok(Vec::new())
    }

    async fn mark_published(
        &self,
        _id: u64,
        _published_at: chrono::DateTime<Utc>,
    ) -> exchange_api::error::AppResult<()> {
        Ok(())
    }

    async fn mark_retry(
        &self,
        _id: u64,
        _retry_count: u32,
        _next_retry_at: chrono::DateTime<Utc>,
    ) -> exchange_api::error::AppResult<()> {
        Ok(())
    }

    async fn mark_dead_letter(
        &self,
        _id: u64,
        _retry_count: u32,
        _failed_at: chrono::DateTime<Utc>,
    ) -> exchange_api::error::AppResult<()> {
        Ok(())
    }
}

#[async_trait]
impl MarketIngestionSink for RecordedIngestionSink {
    async fn ingest_ticker(
        &self,
        snapshot: &MarketTickerSnapshot,
    ) -> exchange_api::error::AppResult<()> {
        self.events.lock().await.push(format!(
            "ticker:{}:{}",
            snapshot.provider() as u8,
            snapshot.symbol()
        ));
        Ok(())
    }

    async fn ingest_depth(
        &self,
        snapshot: &MarketDepthSnapshot,
    ) -> exchange_api::error::AppResult<()> {
        self.events.lock().await.push(format!(
            "depth:{}:{}",
            snapshot.provider() as u8,
            snapshot.symbol()
        ));
        Ok(())
    }

    async fn ingest_kline(
        &self,
        snapshot: &MarketKlineSnapshot,
    ) -> exchange_api::error::AppResult<()> {
        self.events.lock().await.push(format!(
            "kline:{}:{}:{}",
            snapshot.provider() as u8,
            snapshot.symbol(),
            snapshot.interval()
        ));
        Ok(())
    }
}

#[tokio::test]
async fn market_feed_worker_routes_provider_frames_to_ingestion_sink() -> Result<(), Box<dyn Error>>
{
    let sink = RecordedIngestionSink::default();
    let worker = MarketFeedWorker::new(sink.clone());
    let frames = stream::iter([
        Ok::<MarketFeedFrame, String>(MarketFeedFrame::bitget_ticker(
            r#"{"arg":{"instId":"BTCUSDT"},"data":[{"lastPr":"70000.12","baseVolume":"125.50","ts":"1710000000000"}]}"#,
        )),
        Ok::<MarketFeedFrame, String>(MarketFeedFrame::bitget_kline(
            r#"{"arg":{"channel":"candle1m","instId":"BTCUSDT"},"data":[["1710000000000","70000.00","70010.00","69990.00","70005.00","12.30"]]}"#,
        )),
        Ok::<MarketFeedFrame, String>(MarketFeedFrame::htx_depth(
            r#"{"ch":"market.ethusdt.depth.step0","ts":1710000000001,"tick":{"bids":[["3000.00","1.20"]],"asks":[["3001.00","1.10"]],"ts":1710000000001}}"#,
        )),
        Ok::<MarketFeedFrame, String>(MarketFeedFrame::coinbase_ticker(
            r#"{"channel":"ticker","timestamp":"2026-06-15T01:00:00Z","events":[{"type":"snapshot","tickers":[{"product_id":"SOL-USDT","price":"150.12","volume_24_h":"25.50"}]}]}"#,
        )),
    ]);

    let summary = worker.run_stream(frames).await?;
    let events = sink.events.lock().await.clone();

    assert_eq!(summary.received, 4);
    assert_eq!(summary.ingested, 4);
    assert_eq!(summary.failed, 0);
    assert_eq!(
        events,
        vec![
            "ticker:0:BTCUSDT".to_owned(),
            "kline:0:BTCUSDT:1m".to_owned(),
            "depth:1:ETHUSDT".to_owned(),
            "ticker:3:SOLUSDT".to_owned(),
        ]
    );
    Ok(())
}

#[tokio::test]
async fn market_feed_worker_counts_invalid_frames_without_stopping() -> Result<(), Box<dyn Error>> {
    let sink = RecordedIngestionSink::default();
    let worker = MarketFeedWorker::new(sink.clone());
    let frames = stream::iter([
        Ok::<MarketFeedFrame, String>(MarketFeedFrame::bitget_trade(
            r#"{"arg":{"instId":"BTCUSDT"},"data":[{"tradeId":"bt-1","side":"unknown","price":"1","size":"1","ts":"1710000000000"}]}"#,
        )),
        Ok::<MarketFeedFrame, String>(MarketFeedFrame::htx_ticker(
            r#"{"ch":"market.btcusdt.detail","ts":1710000000002,"tick":{"close":"70001.00","amount":"99.00"}}"#,
        )),
    ]);

    let summary = worker.run_stream(frames).await?;
    let events = sink.events.lock().await.clone();

    assert_eq!(summary.received, 2);
    assert_eq!(summary.ingested, 1);
    assert_eq!(summary.failed, 1);
    assert_eq!(events, vec!["ticker:1:BTCUSDT".to_owned()]);
    Ok(())
}

#[test]
fn market_feed_cycle_rejects_only_invalid_frames() {
    let invalid_only = exchange_api::modules::market::adapters::MarketFeedSummary::new(1, 0, 1);
    let mixed = exchange_api::modules::market::adapters::MarketFeedSummary::new(2, 1, 1);

    let error = ensure_market_feed_cycle_has_valid_frames(&invalid_only)
        .unwrap_err()
        .to_string();
    assert!(error.contains("market feed websocket cycle received only invalid frames"));
    ensure_market_feed_cycle_has_valid_frames(&mixed).unwrap();
}

#[test]
fn market_feed_event_payloads_are_ready_for_outbox_fanout() -> Result<(), Box<dyn Error>> {
    let frame = MarketFeedFrame::bitget_ticker(
        r#"{"arg":{"instId":"BTCUSDT"},"data":[{"lastPr":"70000.12","open24h":"69000.00","high24h":"70100.00","low24h":"68000.00","baseVolume":"125.50","ts":"1710000000000"}]}"#,
    );

    let event = MarketFeedEvent::from_frame(&frame)?;
    let broadcast = EventBroadcastMessage::from_market_feed_event(&event)?;

    assert_eq!(event.aggregate_type(), "market_ticker");
    assert_eq!(event.aggregate_id(), "BTCUSDT");
    assert_eq!(event.event_type(), "ticker_updated");
    assert_eq!(event.routing_key(), "market.BTCUSDT.ticker");
    assert_eq!(
        event.idempotency_key(),
        "market_feed:bitget:BTCUSDT:ticker:1710000000000"
    );
    assert_eq!(event.payload()["symbol"], "BTCUSDT");
    assert_eq!(event.payload()["last_price"], "70000.12");
    assert_eq!(event.payload()["high_24h"], "70100.00");
    assert_eq!(event.payload()["low_24h"], "68000.00");
    assert_eq!(event.payload()["price_change_24h"], "1000.12");
    assert_eq!(
        broadcast.channel(),
        &WebSocketChannel::public("ticker", "BTCUSDT")?
    );
    assert_eq!(broadcast.payload(), event.payload().to_string());
    Ok(())
}

#[test]
fn market_feed_provider_codes_are_validated_for_runtime_selection() {
    assert_eq!(
        MarketFeedProvider::from_code("bitget").unwrap(),
        MarketFeedProvider::Bitget
    );
    assert_eq!(
        MarketFeedProvider::from_code("HTX").unwrap(),
        MarketFeedProvider::Htx
    );
    assert_eq!(
        MarketFeedProvider::from_code("huobi").unwrap(),
        MarketFeedProvider::Htx
    );
    assert_eq!(
        MarketFeedProvider::from_code("Coinbase_Advanced_Trade").unwrap(),
        MarketFeedProvider::Coinbase
    );
    assert_eq!(MarketFeedProvider::Bitget.code(), "bitget");
    assert_eq!(MarketFeedProvider::Htx.code(), "htx");
    assert_eq!(MarketFeedProvider::Coinbase.code(), "coinbase");
    assert_eq!(MarketFeedProvider::Bitget.aliases(), &["bitget"]);
    assert_eq!(MarketFeedProvider::Htx.aliases(), &["htx", "huobi"]);
    assert!(MarketFeedProvider::Coinbase.aliases().contains(&"coinbase"));
    assert!(MarketFeedProvider::from_code("unknown").is_err());
}

#[test]
fn provider_feed_configs_can_select_runtime_providers() -> Result<(), Box<dyn Error>> {
    let settings = test_settings();

    let configs = MarketFeedWorker::<RecordedIngestionSink>::provider_configs_for(
        &settings,
        &[MarketFeedProvider::Coinbase],
        &["BTCUSDT"],
        &["5m"],
    )?;

    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0].provider(), MarketFeedProvider::Coinbase);
    assert_eq!(configs[0].url(), "wss://coinbase.test/ws");
    assert!(
        configs[0]
            .subscription_messages()
            .iter()
            .any(|message| message.contains("\"channel\":\"candles\""))
    );
    assert!(
        configs[0]
            .subscription_messages()
            .iter()
            .any(|message| message.contains("BTC-USDT"))
    );
    Ok(())
}

#[test]
fn provider_feed_configs_use_provider_registry_metadata() -> Result<(), Box<dyn Error>> {
    let settings = test_settings();

    let configs = MarketFeedWorker::<RecordedIngestionSink>::provider_configs_for(
        &settings,
        &[
            MarketFeedProvider::Htx,
            MarketFeedProvider::Bitget,
            MarketFeedProvider::Coinbase,
        ],
        &["BTCUSDT"],
        &["1m", "5m"],
    )?;

    assert_eq!(configs.len(), 3);
    assert_eq!(configs[0].provider(), MarketFeedProvider::Htx);
    assert_eq!(configs[0].url(), "wss://htx.test/ws");
    assert!(
        configs[0]
            .subscription_messages()
            .iter()
            .any(|message| message.contains("market.btcusdt.detail"))
    );
    assert_eq!(configs[1].provider(), MarketFeedProvider::Bitget);
    assert_eq!(configs[1].url(), "wss://bitget.test/ws");
    assert!(
        configs[1]
            .subscription_messages()
            .iter()
            .any(|message| message.contains("candle1m"))
    );
    assert_eq!(configs[2].provider(), MarketFeedProvider::Coinbase);
    assert_eq!(configs[2].url(), "wss://coinbase.test/ws");
    assert!(
        configs[2]
            .subscription_messages()
            .iter()
            .any(|message| message.contains("\"channel\":\"level2\""))
    );
    assert!(
        configs[2]
            .subscription_messages()
            .iter()
            .any(|message| message.contains("\"channel\":\"candles\""))
    );
    Ok(())
}

#[test]
fn runtime_provider_codes_default_and_deduplicate_in_order() -> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let default_config = MarketFeedRuntimeConfig::new(
        &settings,
        vec!["BTCUSDT".to_owned()],
        vec!["1m".to_owned()],
        Vec::new(),
        5,
    )?;
    let deduplicated_config = MarketFeedRuntimeConfig::new(
        &settings,
        vec!["BTCUSDT".to_owned()],
        vec!["1m".to_owned()],
        vec![
            "htx".to_owned(),
            "huobi".to_owned(),
            "coinbase".to_owned(),
            "bitget".to_owned(),
        ],
        5,
    )?;

    assert_eq!(
        default_config.providers(),
        &[MarketFeedProvider::Bitget, MarketFeedProvider::Htx]
    );
    assert_eq!(
        deduplicated_config.providers(),
        &[
            MarketFeedProvider::Htx,
            MarketFeedProvider::Coinbase,
            MarketFeedProvider::Bitget
        ]
    );
    Ok(())
}

#[test]
fn runtime_config_preserves_validated_input_symbols_and_intervals_without_provider_payload_parsing()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();

    let config = MarketFeedRuntimeConfig::new(
        &settings,
        vec!["btc-usdt".to_owned(), "eth_usdt".to_owned()],
        vec!["1h".to_owned(), "1d".to_owned()],
        vec!["htx".to_owned()],
        5,
    )?;

    assert_eq!(config.symbols(), &["BTCUSDT", "ETHUSDT"]);
    assert_eq!(config.intervals(), &["1h", "1d"]);
    Ok(())
}

#[test]
fn provider_rest_fallback_configs_use_settings_urls_and_validated_pairs()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();

    let configs = MarketFeedWorker::<RecordedIngestionSink>::provider_rest_fallback_configs_for(
        &settings,
        &[
            MarketFeedProvider::Bitget,
            MarketFeedProvider::Htx,
            MarketFeedProvider::Coinbase,
        ],
        &["BTCUSDT"],
        &["1m", "1h"],
    )?;

    assert_eq!(configs.len(), 3);
    assert_eq!(configs[0].provider(), MarketFeedProvider::Bitget);
    assert_eq!(
        configs[0].ticker_url(),
        "https://bitget.test/api/v2/spot/market/tickers?symbol=BTCUSDT"
    );
    assert!(configs[0].kline_urls().iter().any(|url| {
        url == "https://bitget.test/api/v2/spot/market/candles?symbol=BTCUSDT&granularity=1min"
    }));
    assert!(configs[0].kline_urls().iter().any(|url| {
        url == "https://bitget.test/api/v2/spot/market/candles?symbol=BTCUSDT&granularity=1h"
    }));

    let daily_config =
        MarketFeedWorker::<RecordedIngestionSink>::provider_rest_fallback_configs_for(
            &settings,
            &[MarketFeedProvider::Bitget],
            &["BTCUSDT"],
            &["1d"],
        )?;
    assert_eq!(
        daily_config[0].kline_urls()[0],
        "https://bitget.test/api/v2/spot/market/candles?symbol=BTCUSDT&granularity=1day"
    );
    assert_eq!(configs[1].provider(), MarketFeedProvider::Htx);
    assert_eq!(
        configs[1].ticker_url(),
        "https://htx.test/market/detail/merged?symbol=btcusdt"
    );
    assert!(
        configs[1].kline_urls().iter().any(|url| {
            url == "https://htx.test/market/history/kline?symbol=btcusdt&period=1min"
        })
    );
    assert!(
        configs[1].kline_urls().iter().any(|url| {
            url == "https://htx.test/market/history/kline?symbol=btcusdt&period=60min"
        })
    );
    assert_eq!(configs[2].provider(), MarketFeedProvider::Coinbase);
    assert_eq!(
        configs[2].ticker_url(),
        "https://coinbase.test/api/v3/brokerage/market/products/BTC-USDT"
    );
    assert!(configs[2].kline_urls().iter().any(|url| {
        url.starts_with("https://coinbase.test/api/v3/brokerage/market/products/BTC-USDT/candles?")
            && url.contains("granularity=ONE_MINUTE")
    }));
    assert!(configs[2].kline_urls().iter().any(|url| {
        url.starts_with("https://coinbase.test/api/v3/brokerage/market/products/BTC-USDT/candles?")
            && url.contains("granularity=ONE_HOUR")
    }));
    Ok(())
}

#[tokio::test]
async fn market_feed_rest_fallback_fetches_and_ingests_tickers_and_klines_for_all_symbols()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let config = MarketFeedWorker::<RecordedIngestionSink>::provider_rest_fallback_configs_for(
        &settings,
        &[MarketFeedProvider::Bitget],
        &["BTCUSDT", "ETHUSDT"],
        &["1m", "1d"],
    )?
    .remove(0);
    let ticker_urls = config.ticker_urls();
    let kline_urls = config.kline_urls();
    let http_client = bitget_rest_fallback_http_client(&ticker_urls, &kline_urls);
    let sink = RecordedIngestionSink::default();
    let worker = MarketFeedWorker::new(sink.clone());

    let summary = worker
        .run_rest_fallback_config(&config, &http_client)
        .await?;
    let events = sink.events.lock().await.clone();

    assert_eq!(summary.received, 7);
    assert_eq!(summary.ingested, 7);
    assert_eq!(summary.failed, 0);
    assert_eq!(
        events,
        vec![
            "ticker:0:BTCUSDT".to_owned(),
            "ticker:0:ETHUSDT".to_owned(),
            "kline:0:BTCUSDT:1m".to_owned(),
            "kline:0:BTCUSDT:1m".to_owned(),
            "kline:0:BTCUSDT:1d".to_owned(),
            "kline:0:ETHUSDT:1m".to_owned(),
            "kline:0:ETHUSDT:1d".to_owned(),
        ]
    );
    Ok(())
}

fn bitget_rest_fallback_http_client(
    ticker_urls: &[String],
    kline_urls: &[String],
) -> RecordedRestFallbackHttpClient {
    RecordedRestFallbackHttpClient::new([
        (
            ticker_urls[0].clone(),
            r#"{"data":[{"instId":"BTCUSDT","lastPr":"70000.12","baseVolume":"125.50","ts":"1710000000000"}]}"#.to_owned(),
        ),
        (
            ticker_urls[1].clone(),
            r#"{"data":[{"instId":"ETHUSDT","lastPr":"3000.12","baseVolume":"225.50","ts":"1710000000100"}]}"#.to_owned(),
        ),
        (
            kline_urls[0].clone(),
            r#"{"data":[["1710000000000","70000.00","70010.00","69990.00","70005.00","12.30"],["1710000060000","70005.00","70020.00","70000.00","70015.00","13.30"]]}"#.to_owned(),
        ),
        (
            kline_urls[1].clone(),
            r#"{"data":[["1710000000000","70000.00","70100.00","69900.00","70050.00","120.00"]]}"#.to_owned(),
        ),
        (
            kline_urls[2].clone(),
            r#"{"data":[["1710000000000","3000.00","3010.00","2990.00","3005.00","22.30"]]}"#.to_owned(),
        ),
        (
            kline_urls[3].clone(),
            r#"{"data":[["1710000000000","3000.00","3100.00","2900.00","3050.00","220.00"]]}"#.to_owned(),
        ),
    ])
}

#[tokio::test]
async fn coinbase_rest_fallback_fetches_and_ingests_ticker_and_kline() -> Result<(), Box<dyn Error>>
{
    let settings = test_settings();
    let config = MarketFeedWorker::<RecordedIngestionSink>::provider_rest_fallback_configs_for(
        &settings,
        &[MarketFeedProvider::Coinbase],
        &["BTCUSDT"],
        &["5m"],
    )?
    .remove(0);
    let ticker_urls = config.ticker_urls();
    let kline_urls = config.kline_urls();
    let http_client = RecordedRestFallbackHttpClient::new([
        (
            ticker_urls[0].clone(),
            r#"{"product_id":"BTC-USDT","price":"70000.12","volume_24h":"125.50","price_percentage_change_24h":"1.45%"}"#.to_owned(),
        ),
        (
            kline_urls[0].clone(),
            r#"{"candles":[{"start":"1710000000","low":"69990.00","high":"70010.00","open":"70000.00","close":"70005.00","volume":"12.30"}]}"#.to_owned(),
        ),
    ]);
    let sink = RecordedIngestionSink::default();
    let worker = MarketFeedWorker::new(sink.clone());

    let summary = worker
        .run_rest_fallback_config(&config, &http_client)
        .await?;
    let events = sink.events.lock().await.clone();

    assert_eq!(summary.received, 2);
    assert_eq!(summary.ingested, 2);
    assert_eq!(summary.failed, 0);
    assert_eq!(
        events,
        vec![
            "ticker:3:BTCUSDT".to_owned(),
            "kline:3:BTCUSDT:5m".to_owned(),
        ]
    );
    Ok(())
}

#[test]
fn default_rest_fallback_http_client_uses_request_timeout() {
    let client = ReqwestMarketFeedRestFallbackHttpClient::default();

    assert_eq!(client.timeout(), std::time::Duration::from_secs(3));
}

#[test]
fn rest_fallback_http_client_uses_settings_timeout() {
    let mut settings = test_settings();
    settings.market_feed_rest_fallback_timeout_seconds = 7;

    let client = ReqwestMarketFeedRestFallbackHttpClient::from_settings(&settings);

    assert_eq!(client.timeout(), std::time::Duration::from_secs(7));
}

#[tokio::test]
async fn market_feed_rest_fallback_keeps_successful_frames_when_one_request_fails()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let config = MarketFeedWorker::<RecordedIngestionSink>::provider_rest_fallback_configs_for(
        &settings,
        &[MarketFeedProvider::Bitget],
        &["BTCUSDT", "ETHUSDT"],
        &["1m"],
    )?
    .remove(0);
    let ticker_urls = config.ticker_urls();
    let kline_urls = config.kline_urls();
    let http_client = RecordedRestFallbackHttpClient::new([
        (
            ticker_urls[0].clone(),
            r#"{"data":[{"instId":"BTCUSDT","lastPr":"70000.12","baseVolume":"125.50","ts":"1710000000000"}]}"#.to_owned(),
        ),
        (
            kline_urls[0].clone(),
            r#"{"data":[["1710000000000","70000.00","70010.00","69990.00","70005.00","12.30"]]}"#.to_owned(),
        ),
        (
            kline_urls[1].clone(),
            r#"{"data":[["1710000000000","3000.00","3010.00","2990.00","3005.00","22.30"]]}"#.to_owned(),
        ),
    ]);
    let sink = RecordedIngestionSink::default();
    let worker = MarketFeedWorker::new(sink.clone());

    let summary = worker
        .run_rest_fallback_config(&config, &http_client)
        .await?;
    let events = sink.events.lock().await.clone();

    assert_eq!(summary.received, 4);
    assert_eq!(summary.ingested, 3);
    assert_eq!(summary.failed, 1);
    assert_eq!(
        events,
        vec![
            "ticker:0:BTCUSDT".to_owned(),
            "kline:0:BTCUSDT:1m".to_owned(),
            "kline:0:ETHUSDT:1m".to_owned(),
        ]
    );
    Ok(())
}

#[tokio::test]
async fn market_feed_rest_fallback_reports_request_and_conversion_failure_context()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let config = MarketFeedWorker::<RecordedIngestionSink>::provider_rest_fallback_configs_for(
        &settings,
        &[MarketFeedProvider::Bitget],
        &["BTCUSDT", "ETHUSDT"],
        &["1m"],
    )?
    .remove(0);
    let ticker_urls = config.ticker_urls();
    let kline_urls = config.kline_urls();
    let http_client = RecordedRestFallbackHttpClient::new([
        (
            ticker_urls[0].clone(),
            r#"{"data":[{"instId":"BTCUSDT","lastPr":"70000.12","baseVolume":"125.50","ts":"1710000000000"}]}"#.to_owned(),
        ),
        (kline_urls[0].clone(), r#"{"data":{}}"#.to_owned()),
        (
            kline_urls[1].clone(),
            r#"{"data":[["1710000000000","3000.00","3010.00","2990.00","3005.00","22.30"]]}"#.to_owned(),
        ),
    ]);
    let sink = RecordedIngestionSink::default();
    let worker = MarketFeedWorker::new(sink.clone());

    let summary = worker
        .run_rest_fallback_config(&config, &http_client)
        .await?;
    let events = sink.events.lock().await.clone();
    let failures = summary.failure_contexts();

    assert_eq!(summary.received, 4);
    assert_eq!(summary.ingested, 2);
    assert_eq!(summary.failed, 2);
    assert_eq!(events, vec!["ticker:0:BTCUSDT", "kline:0:ETHUSDT:1m"]);
    assert_eq!(failures.len(), 2);
    assert_eq!(failures[0].provider(), MarketFeedProvider::Bitget);
    assert_eq!(failures[0].channel(), MarketFeedChannel::Ticker);
    assert_eq!(failures[0].symbol(), "ETHUSDT");
    assert_eq!(failures[0].interval(), None);
    assert_eq!(failures[0].url(), ticker_urls[1]);
    assert!(failures[0].error().contains("missing response"));
    assert_eq!(failures[1].provider(), MarketFeedProvider::Bitget);
    assert_eq!(failures[1].channel(), MarketFeedChannel::Kline);
    assert_eq!(failures[1].symbol(), "BTCUSDT");
    assert_eq!(failures[1].interval(), Some("1m"));
    assert_eq!(failures[1].url(), kline_urls[0]);
    assert!(
        failures[1]
            .error()
            .contains("bitget REST kline data is required")
    );
    Ok(())
}

#[tokio::test]
async fn market_feed_provider_cycle_uses_rest_fallback_after_websocket_failure()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let ws_config = MarketFeedWorker::<RecordedIngestionSink>::provider_configs_for(
        &settings,
        &[MarketFeedProvider::Bitget],
        &["BTCUSDT", "ETHUSDT"],
        &["1m", "1d"],
    )?
    .remove(0);
    let rest_config =
        MarketFeedWorker::<RecordedIngestionSink>::provider_rest_fallback_configs_for(
            &settings,
            &[MarketFeedProvider::Bitget],
            &["BTCUSDT", "ETHUSDT"],
            &["1m", "1d"],
        )?
        .remove(0);
    let ticker_urls = rest_config.ticker_urls();
    let kline_urls = rest_config.kline_urls();
    let http_client = bitget_rest_fallback_http_client(&ticker_urls, &kline_urls);
    let sink = RecordedIngestionSink::default();
    let worker = MarketFeedWorker::new(sink.clone());

    let summary = run_provider_cycle_with_rest_fallback(
        AppState::new(settings),
        ws_config,
        rest_config,
        http_client,
        move |_state, _config| async {
            Err(exchange_api::error::AppError::Internal(
                "websocket unavailable".to_owned(),
            ))
        },
        move |_state| {
            let worker = worker.clone();
            async move { Ok(worker) }
        },
    )
    .await?;
    let events = sink.events.lock().await.clone();

    assert_eq!(summary.received, 7);
    assert_eq!(summary.ingested, 7);
    assert_eq!(summary.failed, 0);
    assert_eq!(events.len(), 7);
    assert_eq!(events[0], "ticker:0:BTCUSDT");
    Ok(())
}

#[test]
fn provider_rest_fallback_configs_reject_invalid_symbols_and_intervals() {
    let settings = test_settings();

    assert!(
        MarketFeedWorker::<RecordedIngestionSink>::provider_rest_fallback_configs(
            &settings,
            &["***"],
            &["1m"],
        )
        .is_err()
    );
    assert!(
        MarketFeedWorker::<RecordedIngestionSink>::provider_rest_fallback_configs(
            &settings,
            &["BTCUSDT"],
            &["2m"],
        )
        .is_err()
    );
}

#[test]
fn provider_feed_configs_use_settings_urls_and_channel_payloads() -> Result<(), Box<dyn Error>> {
    let settings = test_settings();

    let configs = MarketFeedWorker::<RecordedIngestionSink>::provider_configs(
        &settings,
        &["BTCUSDT"],
        &["1m", "5m"],
    )?;

    assert_eq!(configs.len(), 2);
    assert_eq!(configs[0].url(), "wss://bitget.test/ws");
    assert!(
        configs[0]
            .subscription_messages()
            .iter()
            .any(|message| message.contains("ticker"))
    );
    assert!(subscription_has_channel(
        configs[0].subscription_messages(),
        "books50"
    )?);
    assert!(!subscription_has_channel(
        configs[0].subscription_messages(),
        "books5"
    )?);
    assert!(
        configs[0]
            .subscription_messages()
            .iter()
            .any(|message| message.contains("candle1m"))
    );
    assert_eq!(configs[1].url(), "wss://htx.test/ws");
    assert!(
        configs[1]
            .subscription_messages()
            .iter()
            .any(|message| message.contains("market.btcusdt.detail"))
    );
    assert!(
        configs[1]
            .subscription_messages()
            .iter()
            .any(|message| message.contains("market.btcusdt.kline.1min"))
    );
    Ok(())
}

fn subscription_has_channel(messages: &[String], channel: &str) -> Result<bool, serde_json::Error> {
    messages.iter().try_fold(false, |found, message| {
        if found {
            return Ok(true);
        }

        let value: serde_json::Value = serde_json::from_str(message)?;
        Ok(value
            .get("args")
            .and_then(|args| args.as_array())
            .is_some_and(|args| {
                args.iter()
                    .any(|arg| arg.get("channel").and_then(|value| value.as_str()) == Some(channel))
            }))
    })
}

#[tokio::test]
async fn market_feed_worker_publishes_events_to_broadcast_hub() -> Result<(), Box<dyn Error>> {
    let sink = RecordedIngestionSink::default();
    let hub = EventBroadcastHub::new(16);
    let worker = MarketFeedWorker::new(sink).with_broadcast_hub(hub.clone());
    let channel = WebSocketChannel::public("ticker", "BTCUSDT")?;
    let mut receiver = hub.subscribe(&channel);
    let frame = MarketFeedFrame::bitget_ticker(
        r#"{"arg":{"instId":"BTCUSDT"},"data":[{"lastPr":"70000.12","baseVolume":"125.50","ts":"1710000000000"}]}"#,
    );

    worker.ingest_frame(&frame).await?;

    let message = receiver.recv().await?;
    assert_eq!(message.channel(), &channel);
    assert_eq!(
        message.payload(),
        MarketFeedEvent::from_frame(&frame)?.payload().to_string()
    );
    Ok(())
}

#[tokio::test]
async fn market_feed_worker_does_not_write_provider_events_to_outbox() -> Result<(), Box<dyn Error>>
{
    let sink = RecordedIngestionSink::default();
    let repository = RecordingOutboxRepository::default();
    let worker = MarketFeedWorker::new(sink).with_outbox_writer(
        exchange_api::modules::events::EventOutboxWriter::new(repository.clone()),
    );
    let frame = MarketFeedFrame::bitget_ticker(
        r#"{"arg":{"instId":"BTCUSDT"},"data":[{"lastPr":"70000.12","baseVolume":"125.50","ts":"1710000000000"}]}"#,
    );

    worker.ingest_frame(&frame).await?;

    let events = repository.events.lock().await;
    assert!(events.is_empty());
    Ok(())
}

#[test]
fn provider_feed_configs_reject_invalid_symbols_and_intervals() {
    let settings = test_settings();

    assert!(
        MarketFeedWorker::<RecordedIngestionSink>::provider_configs(&settings, &["***"], &["1m"])
            .is_err()
    );
    assert!(
        MarketFeedWorker::<RecordedIngestionSink>::provider_configs(
            &settings,
            &["BTCUSDT"],
            &["2m"]
        )
        .is_err()
    );
}

#[test]
fn market_feed_text_action_handles_provider_heartbeats_and_data_frames() {
    assert_eq!(
        market_feed_text_action(MarketFeedProvider::Htx, r#"{"ping":1710000000000}"#).unwrap(),
        MarketFeedTextAction::Reply(r#"{"pong":1710000000000}"#.to_owned())
    );
    assert_eq!(
        market_feed_text_action(
            MarketFeedProvider::Bitget,
            r#"{"event":"subscribe","code":"0"}"#
        )
        .unwrap(),
        MarketFeedTextAction::Ignore
    );
    assert_eq!(
        market_feed_text_action(
            MarketFeedProvider::Coinbase,
            r#"{"channel":"heartbeats","timestamp":"2026-06-15T01:00:00Z","events":[{"current_time":"2026-06-15 01:00:00.000000000"}]}"#
        )
        .unwrap(),
        MarketFeedTextAction::Ignore
    );
    assert_eq!(
        market_feed_text_action(
            MarketFeedProvider::Bitget,
            r#"{"arg":{"channel":"ticker","instId":"BTCUSDT"},"data":[{"lastPr":"70000.12","baseVolume":"125.50","ts":"1710000000000"}]}"#,
        )
        .unwrap(),
        MarketFeedTextAction::Frame(MarketFeedFrame::new(
            MarketFeedProvider::Bitget,
            MarketFeedChannel::Ticker,
            r#"{"arg":{"channel":"ticker","instId":"BTCUSDT"},"data":[{"lastPr":"70000.12","baseVolume":"125.50","ts":"1710000000000"}]}"#,
        ))
    );
}

#[test]
fn market_feed_text_action_handles_subscription_acknowledgements() {
    assert_eq!(
        market_feed_text_action(
            MarketFeedProvider::Htx,
            r#"{"status":"ok","subbed":"market.btcusdt.kline.1min","ts":1710000000000}"#,
        )
        .unwrap(),
        MarketFeedTextAction::Ignore
    );

    let bitget_error = market_feed_text_action(
        MarketFeedProvider::Bitget,
        r#"{"event":"error","code":"30001","msg":"bad request"}"#,
    )
    .unwrap_err()
    .to_string();
    assert!(bitget_error.contains("bitget market feed acknowledgement error"));
    assert!(bitget_error.contains("30001"));
    assert!(bitget_error.contains("bad request"));

    let htx_error = market_feed_text_action(
        MarketFeedProvider::Htx,
        r#"{"status":"error","err-code":"bad-request","err-msg":"invalid topic"}"#,
    )
    .unwrap_err()
    .to_string();
    assert!(htx_error.contains("htx market feed acknowledgement error"));
    assert!(htx_error.contains("bad-request"));
    assert!(htx_error.contains("invalid topic"));

    let coinbase_error = market_feed_text_action(
        MarketFeedProvider::Coinbase,
        r#"{"type":"error","code":"bad_request","message":"invalid channel"}"#,
    )
    .unwrap_err()
    .to_string();
    assert!(coinbase_error.contains("coinbase market feed acknowledgement error"));
    assert!(coinbase_error.contains("bad_request"));
    assert!(coinbase_error.contains("invalid channel"));
}

#[test]
fn market_feed_text_action_preserves_data_frames_with_control_fields() {
    let bitget_error = market_feed_text_action(
        MarketFeedProvider::Bitget,
        r#"{"event":"error","code":"30001","msg":"bad request","data":null}"#,
    )
    .unwrap_err()
    .to_string();
    assert!(bitget_error.contains("bitget market feed acknowledgement error"));

    assert_eq!(
        market_feed_text_action(
            MarketFeedProvider::Bitget,
            r#"{"event":"snapshot","arg":{"channel":"ticker","instId":"BTCUSDT"},"data":[{"lastPr":"70000.12","baseVolume":"125.50","ts":"1710000000000"}]}"#,
        )
        .unwrap(),
        MarketFeedTextAction::Frame(MarketFeedFrame::new(
            MarketFeedProvider::Bitget,
            MarketFeedChannel::Ticker,
            r#"{"event":"snapshot","arg":{"channel":"ticker","instId":"BTCUSDT"},"data":[{"lastPr":"70000.12","baseVolume":"125.50","ts":"1710000000000"}]}"#,
        ))
    );

    assert_eq!(
        market_feed_text_action(
            MarketFeedProvider::Htx,
            r#"{"status":"ok","rep":"market.btcusdt.kline.1min","data":[{"id":1710000000,"open":"70000.00","close":"70005.00","low":"69990.00","high":"70010.00","amount":"12.30"}]}"#,
        )
        .unwrap(),
        MarketFeedTextAction::Frame(MarketFeedFrame::new(
            MarketFeedProvider::Htx,
            MarketFeedChannel::Kline,
            r#"{"status":"ok","rep":"market.btcusdt.kline.1min","data":[{"id":1710000000,"open":"70000.00","close":"70005.00","low":"69990.00","high":"70010.00","amount":"12.30"}]}"#,
        ))
    );

    assert_eq!(
        market_feed_text_action(
            MarketFeedProvider::Coinbase,
            r#"{"channel":"l2_data","timestamp":"2026-06-15T01:00:01Z","events":[{"type":"snapshot","product_id":"BTC-USDT","updates":[{"side":"bid","event_time":"2026-06-15T01:00:01Z","price_level":"70000.00","new_quantity":"0.50"}]}]}"#,
        )
        .unwrap(),
        MarketFeedTextAction::Frame(MarketFeedFrame::new(
            MarketFeedProvider::Coinbase,
            MarketFeedChannel::Depth,
            r#"{"channel":"l2_data","timestamp":"2026-06-15T01:00:01Z","events":[{"type":"snapshot","product_id":"BTC-USDT","updates":[{"side":"bid","event_time":"2026-06-15T01:00:01Z","price_level":"70000.00","new_quantity":"0.50"}]}]}"#,
        ))
    );
}

#[test]
fn market_feed_socket_action_handles_compressed_binary_frames() {
    let payload = r#"{"ch":"market.btcusdt.detail","ts":1710000000002,"tick":{"close":"70001.00","amount":"99.00"}}"#;

    assert_eq!(
        market_feed_socket_action(
            MarketFeedProvider::Htx,
            Message::Binary(gzip_payload(payload))
        )
        .unwrap(),
        MarketFeedSocketAction::Frame(MarketFeedFrame::new(
            MarketFeedProvider::Htx,
            MarketFeedChannel::Ticker,
            payload,
        ))
    );

    let unsupported =
        market_feed_socket_action(MarketFeedProvider::Htx, Message::Binary(vec![0, 1, 2, 3]))
            .unwrap_err()
            .to_string();
    assert!(unsupported.contains("unsupported market feed binary websocket frame"));

    let bitget_gzip = market_feed_socket_action(
        MarketFeedProvider::Bitget,
        Message::Binary(gzip_payload(payload)),
    )
    .unwrap_err()
    .to_string();
    assert!(bitget_gzip.contains("unsupported bitget market feed binary websocket frame"));

    let htx_error = market_feed_socket_action(
        MarketFeedProvider::Htx,
        Message::Binary(gzip_payload(
            r#"{"status":"error","err-code":"bad-request","err-msg":"invalid topic"}"#,
        )),
    )
    .unwrap_err()
    .to_string();
    assert!(htx_error.contains("htx market feed acknowledgement error"));
    assert!(htx_error.contains("bad-request"));
}

#[test]
fn market_feed_socket_action_handles_pings_closes_and_data_frames() {
    assert_eq!(
        market_feed_socket_action(MarketFeedProvider::Bitget, Message::Ping(vec![1, 2, 3]))
            .unwrap(),
        MarketFeedSocketAction::Reply(Message::Pong(vec![1, 2, 3]))
    );
    assert_eq!(
        market_feed_socket_action(MarketFeedProvider::Bitget, Message::Close(None)).unwrap(),
        MarketFeedSocketAction::Close
    );
    assert!(matches!(
        market_feed_socket_action(
            MarketFeedProvider::Bitget,
            Message::Text(
                r#"{"arg":{"channel":"ticker","instId":"BTCUSDT"},"data":[{"lastPr":"70000.12","baseVolume":"125.50","ts":"1710000000000"}]}"#.to_owned()
            )
        )
        .unwrap(),
        MarketFeedSocketAction::Frame(_)
    ));
}

#[test]
fn market_feed_socket_action_ignores_unrecognized_channel_payloads() {
    let action = market_feed_socket_action(
        MarketFeedProvider::Bitget,
        Message::Text(
            r#"{"arg":{"channel":"account","instId":"BTCUSDT"},"data":[{"balance":"1"}]}"#
                .to_owned(),
        ),
    )
    .unwrap();

    assert_eq!(action, MarketFeedSocketAction::Ignore);
}

#[test]
fn market_feed_runtime_config_validates_startup_symbols_and_intervals() {
    let settings = test_settings();

    let config = MarketFeedRuntimeConfig::new(
        &settings,
        vec!["BTC-USDT".to_owned()],
        vec!["1m".to_owned()],
        vec!["htx".to_owned()],
        0,
    )
    .unwrap();

    assert_eq!(config.symbols(), &["BTCUSDT".to_owned()]);
    assert_eq!(config.intervals(), &["1m".to_owned()]);
    assert_eq!(config.providers(), &[MarketFeedProvider::Htx]);
    assert_eq!(config.reconnect_seconds(), 1);
    assert!(
        MarketFeedRuntimeConfig::new(
            &settings,
            vec!["***".to_owned()],
            vec!["1m".to_owned()],
            Vec::new(),
            1
        )
        .is_err()
    );
    assert!(
        MarketFeedRuntimeConfig::new(
            &settings,
            vec!["BTCUSDT".to_owned()],
            vec!["2m".to_owned()],
            Vec::new(),
            1
        )
        .is_err()
    );
}

#[test]
fn market_feed_runtime_config_can_be_disabled_by_empty_symbols() {
    let settings = test_settings();

    let config = MarketFeedRuntimeConfig::new(
        &settings,
        Vec::new(),
        vec!["1m".to_owned()],
        vec!["bitget".to_owned()],
        15,
    )
    .unwrap();

    assert!(!config.enabled());
    assert!(config.symbols().is_empty());
    assert_eq!(config.reconnect_seconds(), 15);
}

#[tokio::test]
async fn market_feed_run_config_once_skips_when_disabled() -> Result<(), Box<dyn Error>> {
    let state = exchange_api::state::AppState::new(test_settings());
    let config = MarketFeedRuntimeConfig::new(
        &state.settings,
        Vec::new(),
        vec!["1m".to_owned()],
        vec!["htx".to_owned()],
        1,
    )?;

    exchange_api::workers::market_feed::run_config_once(&state, &config).await?;

    Ok(())
}

#[tokio::test]
async fn market_feed_supervisor_status_tracks_reload_success() -> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let config = MarketFeedRuntimeConfig::new(
        &settings,
        vec!["BTC-USDT".to_owned(), "ETH-USDT".to_owned()],
        vec!["1m".to_owned(), "1h".to_owned()],
        vec!["htx".to_owned(), "bitget".to_owned()],
        5,
    )?;
    let supervisor = MarketFeedSupervisorHandle::new_for_tests();

    let accepted = supervisor.accept_config_for_tests(config, 42).await?;
    let status = supervisor.status().await;

    assert_eq!(accepted, status);
    assert_eq!(status.applied_version, Some(42));
    assert_eq!(status.symbols, vec!["BTCUSDT", "ETHUSDT"]);
    assert_eq!(status.intervals, vec!["1m", "1h"]);
    assert_eq!(status.providers, vec!["htx", "bitget"]);
    assert_eq!(status.last_reload_status.as_deref(), Some("success"));
    assert!(status.last_reload_error.is_none());
    Ok(())
}
