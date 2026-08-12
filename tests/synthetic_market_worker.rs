use std::str::FromStr;

use bigdecimal::BigDecimal;
use chrono::{Duration, TimeZone, Utc};
use exchange_api::{
    modules::market::{
        MarketKlineValues,
        synthetic::{SyntheticKlineInterval, SyntheticMarketConfig},
    },
    workers::synthetic_market::{
        build_aggregate_kline_snapshot, build_online_minute_close_plan, build_realtime_plan,
        completed_aggregate_intervals,
    },
};

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).expect("valid decimal fixture")
}

#[test]
fn realtime_plan_only_uses_current_minute_and_keeps_ticker_equal_to_kline_close() {
    let start = Utc.with_ymd_and_hms(2026, 8, 12, 10, 0, 0).unwrap();
    let config = SyntheticMarketConfig::new(SyntheticMarketConfig {
        symbol: "tick-usdt".to_owned(),
        seed: "worker-plan-seed".to_owned(),
        version: 3,
        price_precision: 6,
        start_time: start,
        end_time: start + Duration::hours(1),
        start_price: decimal("100"),
        target_price: decimal("120"),
        volatility: decimal("0.01"),
        volume_min: decimal("10"),
        volume_max: decimal("20"),
        nodes: Vec::new(),
    })
    .unwrap();
    let historical = vec![MarketKlineValues {
        open: decimal("98"),
        high: decimal("110"),
        low: decimal("90"),
        close: decimal("100"),
        volume: decimal("50"),
    }];
    let observed_at = start + Duration::minutes(17) + Duration::seconds(42);

    let plan = build_realtime_plan(9, &config, observed_at, &historical).unwrap();
    let replay = build_realtime_plan(9, &config, observed_at, &historical).unwrap();

    assert_eq!(plan, replay);
    assert_eq!(plan.strategy_id(), 9);
    assert_eq!(plan.version(), 3);
    assert_eq!(plan.kline().open_time(), start + Duration::minutes(17));
    assert_eq!(plan.ticker().last_price(), plan.kline().close());
    assert!(plan.ticker().high_24h() >= plan.kline().high());
    assert!(plan.ticker().low_24h() <= plan.kline().low());
    assert_eq!(
        plan.ticker().volume_24h(),
        &(decimal("50") + plan.kline().volume())
    );

    let later = build_realtime_plan(
        9,
        &config,
        start + Duration::minutes(17) + Duration::seconds(43),
        &historical,
    )
    .unwrap();
    assert_eq!(later.kline().open_time(), plan.kline().open_time());
    assert_ne!(later.kline().close(), plan.kline().close());
    assert_eq!(later.ticker().last_price(), later.kline().close());
}

#[test]
fn realtime_plan_does_not_generate_stopped_history_between_calls() {
    let start = Utc.with_ymd_and_hms(2026, 8, 12, 0, 0, 0).unwrap();
    let config = SyntheticMarketConfig::new(SyntheticMarketConfig {
        symbol: "restart-usdt".to_owned(),
        seed: "restart-seed".to_owned(),
        version: 1,
        price_precision: 4,
        start_time: start,
        end_time: start + Duration::days(1),
        start_price: decimal("1"),
        target_price: decimal("2"),
        volatility: decimal("0.02"),
        volume_min: decimal("1"),
        volume_max: decimal("2"),
        nodes: Vec::new(),
    })
    .unwrap();

    let restarted_at = start + Duration::hours(8) + Duration::minutes(31) + Duration::seconds(37);
    let plan = build_realtime_plan(1, &config, restarted_at, &[]).unwrap();

    assert_eq!(
        plan.kline().open_time(),
        start + Duration::hours(8) + Duration::minutes(31)
    );
}

#[test]
fn realtime_plan_uses_observed_second_and_finishes_with_deterministic_1m_values() {
    let start = Utc.with_ymd_and_hms(2026, 8, 12, 10, 0, 0).unwrap();
    let config = SyntheticMarketConfig::new(SyntheticMarketConfig {
        symbol: "partial-usdt".to_owned(),
        seed: "partial-minute-seed".to_owned(),
        version: 5,
        price_precision: 8,
        start_time: start,
        end_time: start + Duration::hours(1),
        start_price: decimal("100"),
        target_price: decimal("130"),
        volatility: decimal("0.03"),
        volume_min: decimal("60"),
        volume_max: decimal("120"),
        nodes: Vec::new(),
    })
    .unwrap();
    let open_time = start + Duration::minutes(12);
    let second_5 = build_realtime_plan(3, &config, open_time + Duration::seconds(5), &[]).unwrap();
    let second_45 =
        build_realtime_plan(3, &config, open_time + Duration::seconds(45), &[]).unwrap();
    let second_59 =
        build_realtime_plan(3, &config, open_time + Duration::seconds(59), &[]).unwrap();
    let closed = config.generate_1m(open_time).unwrap();

    assert_eq!(second_5.kline().open_time(), open_time);
    assert_eq!(second_45.kline().open_time(), open_time);
    assert_ne!(second_5.kline().close(), second_45.kline().close());
    assert!(second_5.kline().volume() < second_45.kline().volume());
    assert_eq!(second_59.kline().open(), &closed.values.open);
    assert_eq!(second_59.kline().high(), &closed.values.high);
    assert_eq!(second_59.kline().low(), &closed.values.low);
    assert_eq!(second_59.kline().close(), &closed.values.close);
    assert_eq!(second_59.kline().volume(), &closed.values.volume);
    assert_eq!(second_59.ticker().last_price(), second_59.kline().close());
}

#[test]
fn subsecond_observations_share_the_same_deterministic_second_snapshot() {
    let start = Utc.with_ymd_and_hms(2026, 8, 12, 10, 0, 0).unwrap();
    let config = SyntheticMarketConfig::new(SyntheticMarketConfig {
        symbol: "subsecond-usdt".to_owned(),
        seed: "subsecond-seed".to_owned(),
        version: 2,
        price_precision: 6,
        start_time: start,
        end_time: start + Duration::hours(1),
        start_price: decimal("10"),
        target_price: decimal("20"),
        volatility: decimal("0.02"),
        volume_min: decimal("10"),
        volume_max: decimal("20"),
        nodes: Vec::new(),
    })
    .unwrap();
    let second = start + Duration::minutes(3) + Duration::seconds(21);
    let early = build_realtime_plan(4, &config, second + Duration::milliseconds(1), &[]).unwrap();
    let late = build_realtime_plan(4, &config, second + Duration::milliseconds(999), &[]).unwrap();

    assert_eq!(early.kline().open(), late.kline().open());
    assert_eq!(early.kline().high(), late.kline().high());
    assert_eq!(early.kline().low(), late.kline().low());
    assert_eq!(early.kline().close(), late.kline().close());
    assert_eq!(early.kline().volume(), late.kline().volume());
    assert_eq!(early.ticker().last_price(), early.kline().close());
    assert_eq!(late.ticker().last_price(), late.kline().close());
}

#[test]
fn runtime_contract_uses_one_second_tick_and_half_open_strategy_range() {
    let main_source = include_str!("../src/main.rs");
    let worker_source = include_str!("../src/workers/synthetic_market.rs");

    assert!(main_source.contains("state.settings.kline_recovery_enabled"));
    assert!(main_source.contains("state.settings.kline_recovery_batch_limit"));
    assert!(main_source.contains("synthetic_market_state,"));
    assert!(main_source.contains("max_strategies_per_round,"));
    assert!(main_source.contains("历史缺口只能由后台预览确认后手动执行"));
    assert!(!main_source.contains("kline_recovery::run_loop"));
    assert!(worker_source.contains("AND strategies.end_time > ?"));
    assert!(!worker_source.contains("AND strategies.end_time >= ?"));
}

#[test]
fn checkpoint_sql_keeps_lease_owner_expiry_and_latest_version_guards() {
    let worker_source = include_str!("../src/workers/synthetic_market.rs");

    assert!(worker_source.contains("AND lease_owner = ?"));
    assert!(worker_source.contains("AND lease_expires_at >= ?"));
    assert!(worker_source.contains("AND active_version = ?"));
    assert!(worker_source.contains("AND versions.version = runs.active_version"));
    assert!(worker_source.contains("last_tick_at IS NULL OR last_tick_at <= ?"));
    assert!(worker_source.contains("last_kline_open_time IS NULL OR last_kline_open_time <= ?"));
    assert!(worker_source.contains("SyntheticIngestionOutcome::RejectedStale"));
    assert!(!worker_source.contains("SELECT MAX(latest.version)"));
}

#[test]
fn continuous_online_minute_closes_previous_1m_and_selects_completed_aggregates() {
    let start = Utc.with_ymd_and_hms(2026, 8, 11, 0, 0, 0).unwrap();
    let config = SyntheticMarketConfig::new(SyntheticMarketConfig {
        symbol: "aggregate-usdt".to_owned(),
        seed: "aggregate-seed".to_owned(),
        version: 8,
        price_precision: 6,
        start_time: start,
        end_time: start + Duration::days(2),
        start_price: decimal("100"),
        target_price: decimal("200"),
        volatility: decimal("0.02"),
        volume_min: decimal("30"),
        volume_max: decimal("90"),
        nodes: Vec::new(),
    })
    .unwrap();
    let previous_open = start + Duration::days(1) - Duration::minutes(1);
    let previous =
        build_realtime_plan(12, &config, previous_open + Duration::seconds(59), &[]).unwrap();
    let close_plan = build_online_minute_close_plan(
        Some(&previous),
        12,
        &config,
        previous_open + Duration::minutes(1),
    )
    .unwrap()
    .expect("continuous online transition closes previous minute");
    let deterministic = config.generate_1m(previous_open).unwrap();

    assert_eq!(close_plan.kline().open_time(), previous_open);
    assert_eq!(close_plan.kline().open(), &deterministic.values.open);
    assert_eq!(close_plan.kline().high(), &deterministic.values.high);
    assert_eq!(close_plan.kline().low(), &deterministic.values.low);
    assert_eq!(close_plan.kline().close(), &deterministic.values.close);
    assert_eq!(close_plan.kline().volume(), &deterministic.values.volume);
    assert_eq!(
        close_plan
            .aggregate_intervals()
            .iter()
            .map(|interval| interval.as_str())
            .collect::<Vec<_>>(),
        ["5m", "15m", "1h", "4h", "1d"]
    );
}

#[test]
fn online_close_rebuilds_aggregate_from_authoritative_one_minute_window() {
    let start = Utc.with_ymd_and_hms(2026, 8, 12, 10, 0, 0).unwrap();
    let config = SyntheticMarketConfig::new(SyntheticMarketConfig {
        symbol: "window-usdt".to_owned(),
        seed: "window-seed".to_owned(),
        version: 6,
        price_precision: 6,
        start_time: start,
        end_time: start + Duration::hours(1),
        start_price: decimal("20"),
        target_price: decimal("40"),
        volatility: decimal("0.02"),
        volume_min: decimal("5"),
        volume_max: decimal("15"),
        nodes: Vec::new(),
    })
    .unwrap();
    let candles = (0..5)
        .map(|minute| {
            config
                .generate_1m(start + Duration::minutes(minute))
                .unwrap()
        })
        .collect::<Vec<_>>();
    let observed_at = start + Duration::minutes(5);
    let aggregate = build_aggregate_kline_snapshot(
        &config.symbol,
        SyntheticKlineInterval::FiveMinutes,
        &candles,
        observed_at,
    )
    .unwrap();

    assert_eq!(aggregate.interval(), "5m");
    assert_eq!(aggregate.open_time(), start);
    assert_eq!(aggregate.open(), &candles[0].values.open);
    assert_eq!(aggregate.close(), &candles[4].values.close);
    assert_eq!(
        aggregate.high(),
        &candles
            .iter()
            .map(|candle| candle.values.high.clone())
            .max()
            .unwrap()
    );
    assert_eq!(
        aggregate.low(),
        &candles
            .iter()
            .map(|candle| candle.values.low.clone())
            .min()
            .unwrap()
    );
    assert_eq!(
        aggregate.volume(),
        &candles.iter().fold(BigDecimal::from(0), |sum, candle| {
            sum + &candle.values.volume
        })
    );
}

#[test]
fn completed_boundary_schedules_each_supported_authoritative_aggregate() {
    let start = Utc.with_ymd_and_hms(2026, 8, 11, 0, 0, 0).unwrap();
    let cases = [
        (5, vec!["5m"]),
        (15, vec!["5m", "15m"]),
        (60, vec!["5m", "15m", "1h"]),
        (240, vec!["5m", "15m", "1h", "4h"]),
        (1_440, vec!["5m", "15m", "1h", "4h", "1d"]),
    ];

    for (minutes, expected) in cases {
        assert_eq!(
            completed_aggregate_intervals(start, start + Duration::minutes(minutes))
                .iter()
                .map(|interval| interval.as_str())
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn restart_or_skipped_slot_never_creates_automatic_history_close() {
    let start = Utc.with_ymd_and_hms(2026, 8, 12, 0, 0, 0).unwrap();
    let config = SyntheticMarketConfig::new(SyntheticMarketConfig {
        symbol: "gap-usdt".to_owned(),
        seed: "gap-seed".to_owned(),
        version: 4,
        price_precision: 6,
        start_time: start,
        end_time: start + Duration::days(1),
        start_price: decimal("10"),
        target_price: decimal("30"),
        volatility: decimal("0.01"),
        volume_min: decimal("10"),
        volume_max: decimal("20"),
        nodes: Vec::new(),
    })
    .unwrap();

    assert!(
        build_online_minute_close_plan(None, 22, &config, start + Duration::hours(8))
            .unwrap()
            .is_none()
    );

    let before_gap = build_realtime_plan(
        22,
        &config,
        start + Duration::minutes(10) + Duration::seconds(59),
        &[],
    )
    .unwrap();
    assert!(
        build_online_minute_close_plan(
            Some(&before_gap),
            22,
            &config,
            start + Duration::minutes(14),
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn at_most_five_second_observation_gap_closes_previous_slot() {
    let start = Utc.with_ymd_and_hms(2026, 8, 12, 12, 0, 0).unwrap();
    let config = SyntheticMarketConfig::new(SyntheticMarketConfig {
        symbol: "delay-usdt".to_owned(),
        seed: "delay-seed".to_owned(),
        version: 10,
        price_precision: 6,
        start_time: start,
        end_time: start + Duration::hours(1),
        start_price: decimal("10"),
        target_price: decimal("20"),
        volatility: decimal("0.01"),
        volume_min: decimal("1"),
        volume_max: decimal("2"),
        nodes: Vec::new(),
    })
    .unwrap();
    let previous = build_realtime_plan(
        31,
        &config,
        start + Duration::minutes(4) + Duration::seconds(58),
        &[],
    )
    .unwrap();

    let close = build_online_minute_close_plan(
        Some(&previous),
        31,
        &config,
        start + Duration::minutes(5) + Duration::seconds(3),
    )
    .unwrap();

    assert!(close.is_some());
    assert_eq!(
        close.unwrap().kline().open_time(),
        start + Duration::minutes(4)
    );
}

#[test]
fn observation_gap_over_five_seconds_does_not_auto_close_history() {
    let start = Utc.with_ymd_and_hms(2026, 8, 12, 12, 0, 0).unwrap();
    let config = SyntheticMarketConfig::new(SyntheticMarketConfig {
        symbol: "pause-usdt".to_owned(),
        seed: "pause-seed".to_owned(),
        version: 10,
        price_precision: 6,
        start_time: start,
        end_time: start + Duration::hours(1),
        start_price: decimal("10"),
        target_price: decimal("20"),
        volatility: decimal("0.01"),
        volume_min: decimal("1"),
        volume_max: decimal("2"),
        nodes: Vec::new(),
    })
    .unwrap();
    let previous = build_realtime_plan(
        32,
        &config,
        start + Duration::minutes(4) + Duration::seconds(55),
        &[],
    )
    .unwrap();

    assert!(
        build_online_minute_close_plan(
            Some(&previous),
            32,
            &config,
            start + Duration::minutes(5) + Duration::seconds(1),
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn strategy_or_version_change_breaks_online_continuity() {
    let start = Utc.with_ymd_and_hms(2026, 8, 12, 13, 0, 0).unwrap();
    let base = SyntheticMarketConfig::new(SyntheticMarketConfig {
        symbol: "version-usdt".to_owned(),
        seed: "version-seed".to_owned(),
        version: 1,
        price_precision: 6,
        start_time: start,
        end_time: start + Duration::hours(1),
        start_price: decimal("10"),
        target_price: decimal("20"),
        volatility: decimal("0.01"),
        volume_min: decimal("1"),
        volume_max: decimal("2"),
        nodes: Vec::new(),
    })
    .unwrap();
    let previous = build_realtime_plan(
        41,
        &base,
        start + Duration::minutes(6) + Duration::seconds(59),
        &[],
    )
    .unwrap();
    let mut changed = base.clone();
    changed.version = 2;
    let changed = SyntheticMarketConfig::new(changed).unwrap();
    let next = start + Duration::minutes(7);

    assert!(
        build_online_minute_close_plan(Some(&previous), 41, &changed, next)
            .unwrap()
            .is_none()
    );
    assert!(
        build_online_minute_close_plan(Some(&previous), 42, &base, next)
            .unwrap()
            .is_none()
    );
}
