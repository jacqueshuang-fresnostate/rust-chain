use super::*;
use chrono::{TimeDelta, TimeZone, Utc};

#[test]
fn manual_recovery_error_keeps_the_result_error_path_compact() {
    assert!(std::mem::size_of::<ManualKlineRecoveryError>() <= 64);
}

#[test]
fn recovery_gap_returns_missing_open_times_after_checkpoint_until_now() {
    let checkpoint = Utc.with_ymd_and_hms(2026, 5, 26, 10, 0, 0).unwrap();
    let now = checkpoint + TimeDelta::minutes(4);

    let gap = kline_recovery_gap(checkpoint, now, TimeDelta::minutes(1)).unwrap();

    assert_eq!(
        gap.missing_open_times(),
        &[
            checkpoint + TimeDelta::minutes(1),
            checkpoint + TimeDelta::minutes(2),
            checkpoint + TimeDelta::minutes(3),
            checkpoint + TimeDelta::minutes(4),
        ]
    );
    assert!(gap.has_gap());
}

#[test]
fn recovery_gap_is_empty_without_elapsed_interval() {
    let checkpoint = Utc.with_ymd_and_hms(2026, 5, 26, 10, 0, 0).unwrap();

    let gap = kline_recovery_gap(
        checkpoint,
        checkpoint + TimeDelta::seconds(59),
        TimeDelta::minutes(1),
    )
    .unwrap();

    assert!(!gap.has_gap());
    assert!(gap.missing_open_times().is_empty());
    assert_eq!(
        kline_recovery_gap(checkpoint, checkpoint, TimeDelta::zero()).unwrap_err(),
        KlineRecoveryGapError::InvalidInterval
    );
}

#[test]
fn recovered_kline_builds_symbol_scoped_upsert_documents() {
    use mongodb::bson::{DateTime as BsonDateTime, doc};

    let open_time = Utc.with_ymd_and_hms(2026, 5, 26, 10, 1, 0).unwrap();
    let candle = KlineRecoveryCandle::new(
        "NEW-USDT", "1m", open_time, "1.0", "2.0", "0.9", "1.5", "100.0",
    )
    .unwrap();

    assert_eq!(candle.collection_name(), "market_klines_NEWUSDT");
    assert_eq!(
        candle.upsert_filter(),
        doc! { "interval": "1m", "open_time": BsonDateTime::from_millis(open_time.timestamp_millis()) }
    );
    assert_eq!(
        candle.upsert_update(),
        doc! { "$set": {
            "interval": "1m",
            "open_time": BsonDateTime::from_millis(open_time.timestamp_millis()),
            "open": "1.0",
            "high": "2.0",
            "low": "0.9",
            "close": "1.5",
            "volume": "100.0",
        }}
    );
    assert!(
        KlineRecoveryCandle::new("NEW.USDT", "1m", open_time, "1", "1", "1", "1", "1").is_err()
    );
    assert!(
        KlineRecoveryCandle::new("NEW-USDT", "2m", open_time, "1", "1", "1", "1", "1").is_err()
    );
}

#[test]
fn recovery_gap_aligns_open_times_and_caps_batch_size() {
    let checkpoint = Utc.with_ymd_and_hms(2026, 5, 29, 10, 0, 30).unwrap();
    let now = checkpoint + TimeDelta::minutes(800);

    let gap = kline_recovery_gap(checkpoint, now, TimeDelta::minutes(1)).unwrap();

    assert_eq!(gap.missing_open_times().len(), MAX_CANDLES_PER_STRATEGY_RUN);
    assert_eq!(
        gap.missing_open_times().first().copied().unwrap(),
        Utc.with_ymd_and_hms(2026, 5, 29, 10, 1, 0).unwrap()
    );
    assert_eq!(
        gap.missing_open_times().last().copied().unwrap(),
        Utc.with_ymd_and_hms(2026, 5, 29, 18, 20, 0).unwrap()
    );
}

#[test]
fn recovery_plan_uses_only_last_closed_open_time() {
    let checkpoint = Utc.with_ymd_and_hms(2026, 5, 29, 10, 0, 0).unwrap();
    let now = Utc.with_ymd_and_hms(2026, 5, 29, 10, 3, 0).unwrap();
    let strategy = KlineRecoveryStrategyRun::new(
        9,
        "NEW-USDT",
        checkpoint,
        "1.000000000000000000",
        "1.060000000000000000",
        "0.01000000",
        "100.000000000000000000",
        "200.000000000000000000",
    )
    .unwrap();

    let plan = KlineRecoveryPlan::from_strategy(&strategy, now, TimeDelta::minutes(1)).unwrap();

    assert_eq!(plan.candles().len(), 2);
    assert_eq!(
        plan.candles().last().map(KlineRecoveryCandle::open_time),
        Some(checkpoint + TimeDelta::minutes(2))
    );
}

#[test]
fn kline_recovery_plan_scans_running_strategies_until_now() {
    let checkpoint = Utc.with_ymd_and_hms(2026, 5, 29, 10, 0, 0).unwrap();
    let now = checkpoint + TimeDelta::minutes(4) + TimeDelta::seconds(30);
    let strategy = KlineRecoveryStrategyRun::new(
        7,
        "NEW-USDT",
        checkpoint,
        "1.000000000000000000",
        "1.060000000000000000",
        "0.01000000",
        "100.000000000000000000",
        "200.000000000000000000",
    )
    .unwrap();

    let plan = KlineRecoveryPlan::from_strategy(&strategy, now, TimeDelta::minutes(1)).unwrap();

    assert_eq!(plan.strategy_id(), 7);
    assert_eq!(plan.symbol(), "NEWUSDT");
    assert_eq!(plan.interval(), "1m");
    assert_eq!(plan.candles().len(), 3);
    assert_eq!(
        plan.candles()
            .iter()
            .map(KlineRecoveryCandle::open_time)
            .collect::<Vec<_>>(),
        vec![
            checkpoint + TimeDelta::minutes(1),
            checkpoint + TimeDelta::minutes(2),
            checkpoint + TimeDelta::minutes(3),
        ]
    );
    assert_eq!(
        plan.candles()
            .last()
            .map(KlineRecoveryCandle::close)
            .unwrap(),
        "1.060000000000000000"
    );
}

#[test]
fn kline_recovery_summary_counts_scanned_recovered_and_skipped_runs() {
    let summary = summarize_recovery_plans(&[
        KlineRecoveryPlanSummary::Recovered { candles: 2 },
        KlineRecoveryPlanSummary::Skipped,
        KlineRecoveryPlanSummary::Failed,
    ]);

    assert_eq!(summary.scanned, 3);
    assert_eq!(summary.recovered_candles, 2);
    assert_eq!(summary.skipped, 1);
    assert_eq!(summary.failed, 1);
}

#[tokio::test]
async fn manual_recovery_rejects_unbounded_duplicate_and_live_slots_before_io() {
    use crate::modules::market::synthetic::SyntheticMarketConfig;
    use bigdecimal::BigDecimal;

    let start = Utc.with_ymd_and_hms(2026, 8, 12, 10, 0, 0).unwrap();
    let config = SyntheticMarketConfig::new(SyntheticMarketConfig {
        symbol: "NEW-USDT".to_owned(),
        seed: "manual-recovery-validation".to_owned(),
        version: 1,
        price_precision: 6,
        start_time: start,
        end_time: start + TimeDelta::hours(1),
        start_price: BigDecimal::from(1),
        target_price: BigDecimal::from(2),
        volatility: BigDecimal::from(0),
        volume_min: BigDecimal::from(1),
        volume_max: BigDecimal::from(2),
        generator: Default::default(),
        nodes: Vec::new(),
    })
    .unwrap();
    let observed_at = start + TimeDelta::minutes(10);
    let invalid_cases = [
        Vec::new(),
        vec![start + TimeDelta::minutes(1), start + TimeDelta::minutes(1)],
        vec![observed_at],
    ];

    // 这些输入必须在访问 Mongo 前拒绝，因此使用未连接的 lazy client 即可验证边界。
    let client = mongodb::Client::with_uri_str("mongodb://127.0.0.1:1")
        .await
        .unwrap();
    let database = client.database("manual_recovery_validation");
    for open_times in invalid_cases {
        let error = execute_manual_synthetic_recovery(&database, &config, &open_times, observed_at)
            .await
            .unwrap_err();
        assert_eq!(error.counts(), ManualKlineRecoveryCounts::default());
        assert!(error.to_string().contains("manual recovery"));
    }
}

#[test]
fn affected_aggregate_windows_are_aligned_deduplicated_and_complete() {
    let start = Utc.with_ymd_and_hms(2026, 8, 12, 10, 0, 0).unwrap();
    let missing = vec![
        start + TimeDelta::minutes(4),
        start + TimeDelta::minutes(5),
        start + TimeDelta::minutes(9),
    ];

    assert_eq!(
        affected_aggregate_window_starts(&missing, SyntheticKlineInterval::FiveMinutes),
        vec![start, start + TimeDelta::minutes(5)]
    );
    assert_eq!(
        affected_aggregate_window_starts(&missing, SyntheticKlineInterval::FifteenMinutes),
        vec![start]
    );
    assert_eq!(
        affected_aggregate_window_starts(&missing, SyntheticKlineInterval::OneHour),
        vec![start]
    );
}

#[test]
fn incomplete_aggregate_windows_are_skipped_instead_of_emitted() {
    use bigdecimal::BigDecimal;

    let incomplete = vec![SyntheticCandle {
        open_time: Utc.with_ymd_and_hms(2026, 8, 12, 10, 0, 0).unwrap(),
        values: crate::modules::market::MarketKlineValues {
            open: BigDecimal::from(1),
            high: BigDecimal::from(1),
            low: BigDecimal::from(1),
            close: BigDecimal::from(1),
            volume: BigDecimal::from(1),
        },
    }];

    assert!(complete_one_minute_window(incomplete, SyntheticKlineInterval::FiveMinutes).is_none());
}
