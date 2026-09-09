use super::*;
use chrono::TimeDelta;

fn at(seconds: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(seconds, 0).expect("valid test timestamp")
}

fn observation(
    symbol: &str,
    observed_at: Option<DateTime<Utc>>,
    ingested_at: Option<DateTime<Utc>>,
) -> MarketFeedSymbolObservation {
    MarketFeedSymbolObservation {
        symbol: symbol.to_owned(),
        observed_at,
        ingested_at,
    }
}

fn health(
    configured_symbols: &[String],
    latest_observations: &[MarketFeedSymbolObservation],
    runtime_ready: bool,
    runtime_status: Option<&str>,
    now: DateTime<Utc>,
    kline_gap_count: i64,
    kline_recovery_failed_count: i64,
) -> MarketFeedHealthResponse {
    build_market_feed_health(MarketFeedHealthInput {
        configured_symbols,
        latest_observations,
        runtime_ready,
        runtime_status,
        now,
        stale_after: std::time::Duration::from_secs(90),
        kline_gap_count,
        kline_recovery_failed_count,
    })
}

#[test]
fn health_deduplicates_symbols_and_reports_latest_ingestion() {
    let now = at(1_800_000_120);
    let observations = [observation(
        "BTCUSDT",
        Some(now - TimeDelta::seconds(5)),
        Some(now - TimeDelta::seconds(3)),
    )];
    let response = health(
        &["btc-usdt".to_owned(), "BTCUSDT".to_owned()],
        &observations,
        true,
        Some("success"),
        now,
        0,
        0,
    );

    assert_eq!(response.status, "healthy");
    assert!(response.healthy);
    assert_eq!(response.configured_symbols, 1);
    assert_eq!(response.healthy_symbols, 1);
    assert!(response.stale_symbols.is_empty());
    assert_eq!(response.last_observed_at, Some(now - TimeDelta::seconds(5)));
    assert_eq!(response.last_ingested_at, Some(now - TimeDelta::seconds(3)));
}

#[test]
fn health_prioritizes_stale_symbols_and_keeps_sorted_diagnostics() {
    let now = at(1_800_000_120);
    let observations = [observation(
        "BTCUSDT",
        Some(now - TimeDelta::seconds(91)),
        Some(now - TimeDelta::seconds(1)),
    )];
    let response = health(
        &[
            "ETH-USDT".to_owned(),
            "BTC-USDT".to_owned(),
            "ETHUSDT".to_owned(),
        ],
        &observations,
        true,
        Some("success"),
        now,
        0,
        0,
    );

    assert_eq!(response.status, "stale");
    assert!(!response.healthy);
    assert_eq!(response.configured_symbols, 2);
    assert_eq!(response.healthy_symbols, 0);
    assert_eq!(response.stale_symbols, ["BTCUSDT", "ETHUSDT"]);
    assert_eq!(response.last_ingested_at, Some(now - TimeDelta::seconds(1)));
}

#[test]
fn health_marks_runtime_or_recovery_issues_degraded() {
    let now = at(1_800_000_120);
    let observations = [observation("BTCUSDT", Some(now), Some(now))];
    let response = health(
        &["BTCUSDT".to_owned()],
        &observations,
        false,
        Some("failed"),
        now,
        2,
        1,
    );

    assert_eq!(response.status, "degraded");
    assert!(!response.healthy);
    assert_eq!(response.kline_gap_count, 2);
    assert_eq!(response.kline_recovery_failed_count, 1);
}

#[test]
fn health_without_config_still_surfaces_strategy_recovery_issues() {
    let now = at(1_800_000_120);
    let response = health(&[], &[], false, None, now, 10, 10);

    assert_eq!(response.status, "degraded");
    assert!(!response.healthy);
    assert_eq!(response.configured_symbols, 0);
    assert_eq!(response.healthy_symbols, 0);
    assert!(response.stale_symbols.is_empty());
}

#[test]
fn health_without_config_or_recovery_issues_is_explicitly_not_configured() {
    let now = at(1_800_000_120);
    let response = health(&[], &[], false, None, now, 0, 0);

    assert_eq!(response.status, "not_configured");
    assert!(!response.healthy);
    assert_eq!(response.last_observed_at, None);
    assert_eq!(response.last_ingested_at, None);
}
