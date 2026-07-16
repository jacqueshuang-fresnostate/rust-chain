use super::*;
use secrecy::SecretString;
use std::sync::{
    Arc, Mutex as StdMutex,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::{Notify, oneshot};

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

#[tokio::test]
async fn provider_task_supervisor_records_the_first_finished_provider_failure() {
    let events = Arc::new(StdMutex::new(Vec::new()));
    let events_for_task = events.clone();
    let stuck_task = MarketFeedProviderTask::spawn(MarketFeedProvider::Bitget, async {
        std::future::pending::<AppResult<()>>().await
    });
    let failed_task = MarketFeedProviderTask::spawn(MarketFeedProvider::Htx, async {
        panic!("provider panic");
        #[allow(unreachable_code)]
        Ok(())
    });

    let error = await_market_feed_provider_tasks(vec![stuck_task, failed_task], move |event| {
        events_for_task.lock().unwrap().push(event)
    })
    .await
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("market feed provider task failed")
    );
    assert_eq!(events.lock().unwrap().len(), 1);
    assert!(matches!(
        &events.lock().unwrap()[0],
        MarketFeedSupervisorEvent::ProviderTaskFailed {
            provider: MarketFeedProvider::Htx,
            error,
        } if error.contains("provider panic")
    ));
}
