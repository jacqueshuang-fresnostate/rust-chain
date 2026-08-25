use super::*;
use futures_util::stream;
use secrecy::SecretString;
use std::sync::{
    Arc, Mutex as StdMutex,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::{Notify, oneshot};
use tokio::time::Instant;

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
        agent_commission_auto_settle_enabled: false,
        agent_commission_auto_settle_interval_seconds: 60,
        agent_commission_auto_settle_min_age_seconds: 3600,
        agent_commission_auto_settle_batch_limit: 100,
    }
}

#[tokio::test]
async fn provider_reconnect_loop_records_supervisor_events_for_success_and_failure() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let retried = Arc::new(Notify::new());
    let events = Arc::new(StdMutex::new(Vec::new()));
    let attempts_for_runner = attempts.clone();
    let retried_for_runner = retried.clone();
    let events_for_runner = events.clone();
    let state = AppState::new(test_settings());
    let config = MarketFeedConfig::new(
        MarketFeedProvider::Bitget,
        "wss://bitget.test/ws",
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );

    let rest_config =
        MarketFeedRestFallbackConfig::new(MarketFeedProvider::Bitget, Vec::new(), Vec::new());
    let handle = tokio::spawn(run_provider_reconnect_loop_with(
        state,
        config,
        Duration::ZERO,
        move |_state, _config| {
            let attempts = attempts_for_runner.clone();
            let retried = retried_for_runner.clone();
            async move {
                let attempt = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                if attempt >= 2 {
                    retried.notify_one();
                    Ok(())
                } else {
                    Err(crate::error::AppError::Internal("cycle failed".to_owned()))
                }
            }
        },
        MarketFeedRestFallbackRuntime::new(
            rest_config,
            |state| async move { MarketFeedWorker::<MarketIngestionService>::from_state(&state) },
            ReqwestMarketFeedRestFallbackHttpClient::default(),
        ),
        move |event| events_for_runner.lock().unwrap().push(event),
    ));

    tokio::time::timeout(Duration::from_millis(100), retried.notified())
        .await
        .unwrap();
    handle.abort();
    assert!(attempts.load(Ordering::SeqCst) >= 2);
    assert_eq!(
        events.lock().unwrap().as_slice(),
        &[
            MarketFeedSupervisorEvent::ProviderCycleFailed {
                provider: MarketFeedProvider::Bitget,
                delay: Duration::ZERO,
                error: "internal error: cycle failed".to_owned(),
            },
            MarketFeedSupervisorEvent::ProviderCycleSucceeded {
                provider: MarketFeedProvider::Bitget,
            },
        ]
    );
}

#[tokio::test]
async fn provider_reconnect_loop_records_the_delay_used_before_next_attempt() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let (attempt_sender, attempt_receiver) = oneshot::channel();
    let attempt_sender = Arc::new(StdMutex::new(Some(attempt_sender)));
    let events = Arc::new(StdMutex::new(Vec::new()));
    let attempts_for_runner = attempts.clone();
    let attempt_sender_for_runner = attempt_sender.clone();
    let events_for_runner = events.clone();
    let state = AppState::new(test_settings());
    let config = MarketFeedConfig::new(
        MarketFeedProvider::Htx,
        "wss://htx.test/ws",
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );

    let rest_config =
        MarketFeedRestFallbackConfig::new(MarketFeedProvider::Htx, Vec::new(), Vec::new());
    let handle = tokio::spawn(run_provider_reconnect_loop_with(
        state,
        config,
        Duration::from_millis(20),
        move |_state, _config| {
            let attempts = attempts_for_runner.clone();
            let attempt_sender = attempt_sender_for_runner.clone();
            async move {
                let attempt = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                if attempt >= 2 {
                    if let Some(sender) = attempt_sender.lock().unwrap().take() {
                        let _ = sender.send(());
                    }
                    Ok(())
                } else {
                    Err(crate::error::AppError::Internal("cycle failed".to_owned()))
                }
            }
        },
        MarketFeedRestFallbackRuntime::new(
            rest_config,
            |state| async move { MarketFeedWorker::<MarketIngestionService>::from_state(&state) },
            ReqwestMarketFeedRestFallbackHttpClient::default(),
        ),
        move |event| events_for_runner.lock().unwrap().push(event),
    ));

    tokio::time::timeout(Duration::from_millis(200), attempt_receiver)
        .await
        .unwrap()
        .unwrap();
    handle.abort();
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    assert_eq!(
        events.lock().unwrap()[0],
        MarketFeedSupervisorEvent::ProviderCycleFailed {
            provider: MarketFeedProvider::Htx,
            delay: Duration::from_millis(20),
            error: "internal error: cycle failed".to_owned(),
        }
    );
}

#[test]
fn provider_reconnect_backoff_caps_after_failures_and_resets_after_success() {
    let mut backoff = MarketFeedReconnectBackoff::new(Duration::from_secs(5));

    assert_eq!(backoff.next_delay(), Duration::from_secs(5));
    backoff.record_failure();
    assert_eq!(backoff.next_delay(), Duration::from_secs(10));
    backoff.record_failure();
    assert_eq!(backoff.next_delay(), Duration::from_secs(20));
    backoff.record_failure();
    assert_eq!(backoff.next_delay(), Duration::from_secs(40));
    backoff.record_failure();
    assert_eq!(backoff.next_delay(), Duration::from_secs(60));
    backoff.record_success();
    assert_eq!(backoff.next_delay(), Duration::from_secs(5));
}

#[derive(Default)]
struct CountingGenerationRunner {
    active: Arc<AtomicUsize>,
    maximum: Arc<AtomicUsize>,
    started: Arc<AtomicUsize>,
    stopped: Arc<AtomicUsize>,
}

#[async_trait]
impl MarketFeedGenerationRunner for CountingGenerationRunner {
    async fn run(
        &self,
        _state: AppState,
        config: MarketFeedRuntimeConfig,
        _generation: u64,
        _fence: MarketFeedGenerationFence,
        cancellation: CancellationToken,
    ) -> AppResult<()> {
        let mut tasks = JoinSet::new();
        for _ in config.providers() {
            let active = self.active.clone();
            let maximum = self.maximum.clone();
            let started = self.started.clone();
            let stopped = self.stopped.clone();
            let cancellation = cancellation.clone();
            tasks.spawn(async move {
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                maximum.fetch_max(current, Ordering::SeqCst);
                started.fetch_add(1, Ordering::SeqCst);
                cancellation.cancelled().await;
                active.fetch_sub(1, Ordering::SeqCst);
                stopped.fetch_add(1, Ordering::SeqCst);
            });
        }
        cancellation.cancelled().await;
        while let Some(result) = tasks.join_next().await {
            result.map_err(|error| AppError::Internal(error.to_string()))?;
        }
        Ok(())
    }
}

#[tokio::test]
async fn supervisor_waits_every_old_generation_across_reloads_and_disable() {
    let runner = Arc::new(CountingGenerationRunner::default());
    let supervisor = MarketFeedSupervisorHandle::with_runner_for_tests(runner.clone());
    let state = AppState::new(test_settings());
    let enabled = MarketFeedRuntimeConfig::from_normalized(
        vec!["BTCUSDT".to_owned()],
        Vec::new(),
        vec!["bitget".to_owned(), "htx".to_owned()],
        1,
    )
    .unwrap();
    for version in 1..=10 {
        let status = supervisor
            .reload(state.clone(), enabled.clone(), version)
            .await
            .unwrap();
        assert_eq!(status.generation, version);
        assert!(status.ready);
        tokio::time::timeout(Duration::from_secs(1), async {
            while runner.started.load(Ordering::SeqCst) < version as usize * 2 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    supervisor.stop().await;
    let status = supervisor.status().await;
    assert_eq!(status.generation, 11);
    assert!(!status.ready);
    assert_eq!(status.last_reload_status.as_deref(), Some("skipped"));
    assert_eq!(runner.active.load(Ordering::SeqCst), 0);
    assert_eq!(runner.started.load(Ordering::SeqCst), 20);
    assert_eq!(runner.stopped.load(Ordering::SeqCst), 20);
    assert_eq!(runner.maximum.load(Ordering::SeqCst), 2);
    assert!(supervisor.fence.enter(10).await.is_err());
    assert!(supervisor.fence.enter(11).await.is_ok());
}

#[tokio::test]
async fn generation_fence_rejects_old_generation_writes() {
    let fence = MarketFeedGenerationFence::default();
    fence.activate(7).await;
    assert!(fence.enter(7).await.is_ok());
    fence.activate(8).await;
    assert!(fence.enter(7).await.is_err());
    assert!(fence.enter(8).await.is_ok());
}

#[tokio::test]
async fn generation_fence_waits_for_inflight_storage_and_event_scope_before_switching() {
    let fence = MarketFeedGenerationFence::default();
    fence.activate(7).await;
    let write_fence = MarketFeedWriteFence::new(7, fence.clone());
    let permit = write_fence.enter().await.unwrap();
    let next_fence = fence.clone();
    let switch = tokio::spawn(async move {
        next_fence.activate(8).await;
    });
    tokio::task::yield_now().await;
    assert!(!switch.is_finished());

    drop(permit);
    tokio::time::timeout(Duration::from_secs(1), switch)
        .await
        .unwrap()
        .unwrap();
    assert!(fence.enter(7).await.is_err());
    assert!(fence.enter(8).await.is_ok());
}

#[tokio::test]
async fn provider_cancellation_drains_inflight_cycle_before_join() {
    let cancellation = CancellationToken::new();
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let entered_for_cycle = entered.clone();
    let release_for_cycle = release.clone();
    let config = MarketFeedConfig::new(
        MarketFeedProvider::Bitget,
        "wss://bitget.test/ws",
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let rest_config =
        MarketFeedRestFallbackConfig::new(MarketFeedProvider::Bitget, Vec::new(), Vec::new());
    let handle = tokio::spawn(run_provider_reconnect_loop_with_cancellation(
        AppState::new(test_settings()),
        config,
        Duration::ZERO,
        move |_state, _config| {
            let entered = entered_for_cycle.clone();
            let release = release_for_cycle.clone();
            async move {
                entered.notify_one();
                release.notified().await;
                Ok(())
            }
        },
        MarketFeedRestFallbackRuntime::new(
            rest_config,
            |state| async move { MarketFeedWorker::<MarketIngestionService>::from_state(&state) },
            ReqwestMarketFeedRestFallbackHttpClient::default(),
        ),
        |_| {},
        cancellation.clone(),
    ));

    tokio::time::timeout(Duration::from_secs(1), entered.notified())
        .await
        .unwrap();
    cancellation.cancel();
    tokio::task::yield_now().await;
    assert!(
        !handle.is_finished(),
        "cancellation must drain the in-flight storage/event cycle before join"
    );
    release.notify_waiters();
    tokio::time::timeout(Duration::from_secs(1), handle)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

struct PanickingGenerationRunner;

#[async_trait]
impl MarketFeedGenerationRunner for PanickingGenerationRunner {
    async fn run(
        &self,
        _state: AppState,
        _config: MarketFeedRuntimeConfig,
        _generation: u64,
        _fence: MarketFeedGenerationFence,
        _cancellation: CancellationToken,
    ) -> AppResult<()> {
        panic!("generation panic")
    }
}

#[tokio::test]
async fn generation_panic_is_observable_as_readiness_failure() {
    let supervisor =
        MarketFeedSupervisorHandle::with_runner_for_tests(Arc::new(PanickingGenerationRunner));
    let config = MarketFeedRuntimeConfig::from_normalized(
        vec!["BTCUSDT".to_owned()],
        Vec::new(),
        vec!["bitget".to_owned()],
        1,
    )
    .unwrap();
    supervisor
        .reload(AppState::new(test_settings()), config, 1)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if !supervisor.status().await.ready {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let status = supervisor.status().await;
    assert_eq!(status.last_reload_status.as_deref(), Some("failed"));
    assert!(
        status
            .last_reload_error
            .as_deref()
            .unwrap()
            .contains("panicked")
    );
}

#[test]
fn bitget_liveness_uses_text_heartbeat_and_accepts_plain_control_frames() {
    assert_eq!(
        market_feed_heartbeat_message(MarketFeedProvider::Bitget),
        Some(Message::Text("ping".to_owned()))
    );
    assert_eq!(market_feed_heartbeat_message(MarketFeedProvider::Htx), None);
    assert_eq!(
        market_feed_heartbeat_message(MarketFeedProvider::Coinbase),
        None
    );
    assert_eq!(
        market_feed_text_action(MarketFeedProvider::Bitget, "pong").unwrap(),
        MarketFeedTextAction::Ignore
    );
    assert_eq!(
        market_feed_text_action(MarketFeedProvider::Bitget, " ping ").unwrap(),
        MarketFeedTextAction::Reply("pong".to_owned())
    );
}

#[tokio::test]
async fn provider_liveness_refreshes_on_inbound_activity_and_times_out_a_silent_socket() {
    let start = Instant::now();
    let mut refreshed = MarketFeedSocketLiveness::new_for_tests(
        MarketFeedProvider::Htx,
        start,
        None,
        Duration::from_secs(75),
    );
    assert_eq!(refreshed.idle_deadline(), start + Duration::from_secs(75));
    refreshed.record_inbound_at(start + Duration::from_secs(20));
    assert_eq!(refreshed.idle_deadline(), start + Duration::from_secs(95));

    let mut silent = MarketFeedSocketLiveness::new_for_tests(
        MarketFeedProvider::Htx,
        Instant::now(),
        None,
        Duration::from_millis(5),
    );
    let mut reader = stream::pending::<Result<Message, tungstenite::Error>>();
    let event = tokio::time::timeout(Duration::from_millis(250), silent.wait_next(&mut reader))
        .await
        .expect("silent websocket should hit the bounded idle deadline");
    assert!(matches!(event, MarketFeedSocketEvent::IdleTimeout));
}

#[tokio::test]
async fn bitget_liveness_emits_a_heartbeat_before_the_idle_deadline() {
    let mut liveness = MarketFeedSocketLiveness::new_for_tests(
        MarketFeedProvider::Bitget,
        Instant::now(),
        Some(Duration::from_millis(5)),
        Duration::from_millis(100),
    );
    let mut reader = stream::pending::<Result<Message, tungstenite::Error>>();
    let event = tokio::time::timeout(Duration::from_millis(250), liveness.wait_next(&mut reader))
        .await
        .expect("Bitget heartbeat should be scheduled before the idle timeout");
    assert!(matches!(event, MarketFeedSocketEvent::HeartbeatDue));
}

#[tokio::test]
async fn due_bitget_heartbeat_wins_when_a_market_frame_is_already_ready() {
    let mut liveness = MarketFeedSocketLiveness::new_for_tests(
        MarketFeedProvider::Bitget,
        Instant::now(),
        Some(Duration::from_millis(5)),
        Duration::from_millis(100),
    );
    tokio::time::sleep(Duration::from_millis(10)).await;
    let mut busy_reader = stream::iter([Ok(Message::Pong(Vec::new()))])
        .chain(stream::pending::<Result<Message, tungstenite::Error>>());
    let event = liveness.wait_next(&mut busy_reader).await;
    assert!(matches!(event, MarketFeedSocketEvent::HeartbeatDue));
}
