use super::*;
use chrono::{TimeDelta, TimeZone, Utc};

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
