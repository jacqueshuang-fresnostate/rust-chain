use super::*;
use chrono::{DateTime, TimeDelta, Utc};
use std::{collections::VecDeque, sync::Arc};
use tokio::sync::Mutex;

fn timestamp(seconds: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(seconds, 0).unwrap()
}

#[test]
fn coinbase_window_uses_explicit_clock_and_preserves_configured_query() {
    let original = "https://proxy.example.test/custom/BTC-USDT/candles?start=1&end=2&start=3&end=4&granularity=FIVE_MINUTE&limit=17&tenant=a%2Bb&tenant=c#fragment";
    let first_time = timestamp(1_800_000_000);
    for now in [first_time, first_time + TimeDelta::hours(12)] {
        let refreshed = coinbase_rest_kline_url_at(original, "5m", now).unwrap();
        let url = url::Url::parse(&refreshed).unwrap();
        let pairs: Vec<_> = url.query_pairs().into_owned().collect();
        assert_eq!(url.host_str(), Some("proxy.example.test"));
        assert_eq!(url.path(), "/custom/BTC-USDT/candles");
        assert_eq!(url.fragment(), Some("fragment"));
        assert_eq!(pairs.iter().filter(|(key, _)| key == "start").count(), 1);
        assert_eq!(pairs.iter().filter(|(key, _)| key == "end").count(), 1);
        assert!(pairs.contains(&("start".into(), (now.timestamp() - 90_000).to_string())));
        assert!(pairs.contains(&("end".into(), now.timestamp().to_string())));
        let retained: Vec<_> = pairs
            .into_iter()
            .filter(|(key, _)| key != "start" && key != "end")
            .collect();
        assert_eq!(
            retained,
            vec![
                ("granularity".into(), "FIVE_MINUTE".into()),
                ("limit".into(), "17".into()),
                ("tenant".into(), "a+b".into()),
                ("tenant".into(), "c".into())
            ]
        );
    }
}

#[test]
fn coinbase_window_retains_300_candle_widths_and_legacy_interval_mapping() {
    for (interval, seconds) in [
        ("1m", 60),
        ("5m", 300),
        ("15m", 900),
        ("1h", 3600),
        ("1d", 86400),
        ("4h", 60),
    ] {
        let now = timestamp(30);
        let refreshed = coinbase_rest_kline_url_at(
            "https://proxy.example.test/candles?granularity=OVERRIDE",
            interval,
            now,
        )
        .unwrap();
        let url = url::Url::parse(&refreshed).unwrap();
        let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(query["end"], "30");
        assert_eq!(query["start"], (30 - seconds * 300).to_string());
        assert_eq!(query["granularity"], "OVERRIDE");
    }
    assert!(coinbase_rest_kline_url_at("not a URL", "1m", timestamp(0)).is_err());
}

#[derive(Clone, Default)]
struct CapturingClient {
    urls: Arc<Mutex<Vec<String>>>,
    fail_first: bool,
}

#[async_trait]
impl MarketFeedRestFallbackHttpClient for CapturingClient {
    async fn get_text(&self, url: &str) -> AppResult<String> {
        let mut urls = self.urls.lock().await;
        urls.push(url.to_owned());
        if self.fail_first && urls.len() == 1 {
            return Err(AppError::Internal("recorded request failure".into()));
        }
        Ok(r#"{"candles":[]}"#.to_owned())
    }
}

#[tokio::test]
async fn coinbase_dispatch_refreshes_each_request_and_records_actual_failure_url() {
    let config = MarketFeedRestFallbackConfig::new(
        MarketFeedProvider::Coinbase,
        vec![],
        vec![
            MarketFeedRestFallbackKlineRequest::new(
                "BTCUSDT",
                "1m",
                "https://proxy.example.test/btc/candles?start=0&end=1&granularity=ONE_MINUTE",
            ),
            MarketFeedRestFallbackKlineRequest::new(
                "ETHUSDT",
                "1h",
                "https://proxy.example.test/eth/candles?start=0&end=1&granularity=ONE_HOUR",
            ),
        ],
    );
    let client = CapturingClient {
        fail_first: true,
        ..Default::default()
    };
    let mut clock = VecDeque::from([timestamp(1_800_000_000), timestamp(1_800_003_600)]);
    let frames =
        fetch_rest_fallback_frames_with_clock(&config, &client, || clock.pop_front().unwrap())
            .await
            .unwrap();
    assert!(clock.is_empty());
    let urls = client.urls.lock().await;
    assert_eq!(urls.len(), 2);
    for (url, end, width) in [
        (&urls[0], 1_800_000_000_i64, 18_000),
        (&urls[1], 1_800_003_600, 1_080_000),
    ] {
        let parsed = url::Url::parse(url).unwrap();
        let query: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
        assert_eq!(query["end"], end.to_string());
        assert_eq!(query["start"], (end - width).to_string());
    }
    assert_eq!(frames.len(), 1);
    assert!(frames[0].result.is_err());
    assert_eq!(frames[0].request.url, urls[0]);
    assert_eq!(frames[0].request.symbol, "BTCUSDT");
    assert_eq!(frames[0].request.interval.as_deref(), Some("1m"));
    let failure = MarketFeedFailureContext::new(
        config.provider(),
        &frames[0].request,
        frames[0].result.as_ref().unwrap_err(),
    );
    assert_eq!(failure.provider(), MarketFeedProvider::Coinbase);
    assert_eq!(failure.channel(), MarketFeedChannel::Kline);
    assert_eq!(failure.url(), urls[0]);
    assert_eq!(failure.symbol(), "BTCUSDT");
    assert_eq!(failure.interval(), Some("1m"));
}

#[tokio::test]
async fn coinbase_invalid_url_isolated_without_sending_it_or_stopping_later_request() {
    let config = MarketFeedRestFallbackConfig::new(
        MarketFeedProvider::Coinbase,
        vec![],
        vec![
            MarketFeedRestFallbackKlineRequest::new("BTCUSDT", "1m", "not a URL"),
            MarketFeedRestFallbackKlineRequest::new(
                "ETHUSDT",
                "1m",
                "https://proxy.example.test/eth/candles?start=0&end=1",
            ),
        ],
    );
    let client = CapturingClient::default();
    let frames =
        fetch_rest_fallback_frames_with_clock(&config, &client, || timestamp(1_800_000_000))
            .await
            .unwrap();
    assert_eq!(frames.len(), 1);
    assert!(frames[0].result.is_err());
    assert_eq!(frames[0].request.url, "not a URL");
    let urls = client.urls.lock().await;
    assert_eq!(urls.len(), 1);
    assert!(urls[0].starts_with("https://proxy.example.test/eth/candles?"));
}

#[tokio::test]
async fn static_provider_and_coinbase_ticker_urls_do_not_materialize_a_clock() {
    for provider in [
        MarketFeedProvider::Bitget,
        MarketFeedProvider::Htx,
        MarketFeedProvider::Coinbase,
    ] {
        let ticker_url = "https://proxy.example.test/ticker?start=1&end=2&token=a%20b";
        let candle_url = "https://proxy.example.test/candles?start=3&end=4&custom=a%20b";
        let candles = if provider == MarketFeedProvider::Coinbase {
            vec![]
        } else {
            vec![MarketFeedRestFallbackKlineRequest::new(
                "BTCUSDT", "1m", candle_url,
            )]
        };
        let config = MarketFeedRestFallbackConfig::new(
            provider,
            vec![MarketFeedRestFallbackTickerRequest::new(
                "BTCUSDT", ticker_url,
            )],
            candles,
        );
        let client = CapturingClient::default();
        let _ = fetch_rest_fallback_frames_with_clock(&config, &client, || {
            panic!("static request must not read window clock")
        })
        .await
        .unwrap();
        let urls = client.urls.lock().await;
        let expected = if provider == MarketFeedProvider::Coinbase {
            vec![ticker_url]
        } else {
            vec![ticker_url, candle_url]
        };
        assert_eq!(*urls, expected);
    }
}

#[test]
fn market_feed_health_marks_missing_or_stale_data_unhealthy() {
    use chrono::{Duration as ChronoDuration, Utc};
    use std::time::Duration;
    let summary = MarketFeedSummary::new(1, 1, 0);
    let now = Utc::now();
    let healthy = MarketFeedHealth::from_summary(&summary, Some(now), now, Duration::from_secs(60));
    assert!(healthy.is_healthy());
    let stale = MarketFeedHealth::from_summary(
        &summary,
        Some(now - ChronoDuration::seconds(61)),
        now,
        Duration::from_secs(60),
    );
    assert!(!stale.is_healthy());
    let missing = MarketFeedHealth::from_summary(&summary, None, now, Duration::from_secs(60));
    assert!(!missing.is_healthy());
}
