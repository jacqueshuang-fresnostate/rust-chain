use std::str::FromStr;

use bigdecimal::BigDecimal;
use chrono::{DateTime, Duration, TimeZone, Utc};
use exchange_api::modules::market::{
    SyntheticExecutionMode, SyntheticKlineInterval, SyntheticMarketConfig, SyntheticMarketError,
    SyntheticMarketNode, SyntheticTargetType, aggregate_1m_candles,
};

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).expect("valid decimal fixture")
}

fn time(hour: u32, minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 12, hour, minute, 0)
        .single()
        .expect("valid UTC fixture")
}

fn node(
    target_time: DateTime<Utc>,
    target_type: SyntheticTargetType,
    target_value: &str,
    execution_mode: SyntheticExecutionMode,
    tolerance: &str,
) -> SyntheticMarketNode {
    SyntheticMarketNode {
        target_time,
        target_type,
        target_value: decimal(target_value),
        execution_mode,
        tolerance: decimal(tolerance),
        volatility: decimal("0.015"),
        volume_min: Some(decimal("20")),
        volume_max: Some(decimal("40")),
    }
}

fn config(start: DateTime<Utc>, end: DateTime<Utc>) -> SyntheticMarketConfig {
    SyntheticMarketConfig::new(SyntheticMarketConfig {
        symbol: "syn-usdt".to_owned(),
        seed: "version-seed-42".to_owned(),
        version: 7,
        price_precision: 6,
        start_time: start,
        end_time: end,
        start_price: decimal("100"),
        target_price: decimal("140"),
        volatility: decimal("0.02"),
        volume_min: decimal("10"),
        volume_max: decimal("50"),
        nodes: vec![],
    })
    .expect("valid synthetic config")
}

#[test]
fn same_slot_is_replay_stable_and_adjacent_candles_are_continuous() {
    let config = config(time(0, 0), time(0, 10));
    let first = config.generate_1m(time(0, 3)).unwrap();
    let replay = config.generate_1m(time(0, 3)).unwrap();
    let next = config.generate_1m(time(0, 4)).unwrap();

    assert_eq!(first, replay);
    assert_eq!(first.values.close, next.values.open);
    assert!(first.values.open > decimal("0"));
    assert!(first.values.high >= first.values.open);
    assert!(first.values.high >= first.values.close);
    assert!(first.values.low <= first.values.open);
    assert!(first.values.low <= first.values.close);
    assert!(first.values.low > decimal("0"));
    assert!(first.values.volume >= decimal("10"));
    assert!(first.values.volume <= decimal("50"));
}

#[test]
fn hard_nodes_hit_close_exactly_and_percent_targets_use_correct_bases() {
    let mut config = config(time(0, 0), time(0, 30));
    config.nodes = vec![
        node(
            time(0, 10),
            SyntheticTargetType::PercentFromStart,
            "10",
            SyntheticExecutionMode::Hard,
            "0",
        ),
        node(
            time(0, 20),
            SyntheticTargetType::PercentFromPrevious,
            "10",
            SyntheticExecutionMode::Hard,
            "0",
        ),
    ];
    let config = SyntheticMarketConfig::new(config).unwrap();

    assert_eq!(
        config.generate_1m(time(0, 9)).unwrap().values.close,
        decimal("110.000000")
    );
    assert_eq!(
        config.generate_1m(time(0, 19)).unwrap().values.close,
        decimal("121.000000")
    );
    assert_eq!(
        config.generate_1m(time(0, 29)).unwrap().values.close,
        decimal("140.000000")
    );
    assert_eq!(
        config.generate_1m(time(0, 10)).unwrap().values.open,
        decimal("110.000000")
    );
}

#[test]
fn soft_and_range_nodes_stay_inside_percentage_tolerance() {
    for mode in [SyntheticExecutionMode::Soft, SyntheticExecutionMode::Range] {
        let mut config = config(time(0, 0), time(0, 20));
        config.nodes = vec![node(
            time(0, 10),
            SyntheticTargetType::AbsolutePrice,
            "120",
            mode,
            "2",
        )];
        let config = SyntheticMarketConfig::new(config).unwrap();
        let close = config.generate_1m(time(0, 9)).unwrap().values.close;

        assert!(close >= decimal("117.600000"));
        assert!(close <= decimal("122.400000"));
    }
}

#[test]
fn zero_volatility_produces_no_wicks() {
    let mut config = config(time(0, 0), time(0, 10));
    config.volatility = decimal("0");
    let config = SyntheticMarketConfig::new(config).unwrap();
    let candle = config.generate_1m(time(0, 5)).unwrap();

    assert_eq!(
        candle.values.high,
        candle.values.open.clone().max(candle.values.close.clone())
    );
    assert_eq!(
        candle.values.low,
        candle.values.open.clone().min(candle.values.close.clone())
    );
}

#[test]
fn strategy_and_nodes_require_utc_minute_alignment_and_half_open_range() {
    let mut misaligned = config(time(0, 0), time(0, 10));
    misaligned.start_time += Duration::seconds(1);
    assert_eq!(
        SyntheticMarketConfig::new(misaligned).unwrap_err(),
        SyntheticMarketError::MinuteAlignment
    );

    let strategy = config(time(0, 0), time(0, 10));
    assert_eq!(
        strategy.generate_1m(time(0, 10)).unwrap_err(),
        SyntheticMarketError::InvalidOpenTime
    );

    let mut invalid_nodes = config(time(0, 0), time(0, 10));
    invalid_nodes.nodes = vec![node(
        time(0, 10),
        SyntheticTargetType::AbsolutePrice,
        "110",
        SyntheticExecutionMode::Hard,
        "0",
    )];
    assert_eq!(
        SyntheticMarketConfig::new(invalid_nodes).unwrap_err(),
        SyntheticMarketError::InvalidNodeOrder
    );
}

#[test]
fn aggregates_every_supported_interval_from_complete_continuous_1m_windows() {
    for interval in [
        SyntheticKlineInterval::FiveMinutes,
        SyntheticKlineInterval::FifteenMinutes,
        SyntheticKlineInterval::OneHour,
        SyntheticKlineInterval::FourHours,
        SyntheticKlineInterval::OneDay,
    ] {
        let start = time(0, 0);
        let end = start + Duration::minutes(interval.minute_count() as i64);
        let config = config(start, end);
        let candles = (0..interval.minute_count())
            .map(|minute| {
                config
                    .generate_1m(start + Duration::minutes(minute as i64))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let aggregate = aggregate_1m_candles(&candles, interval).unwrap();

        assert_eq!(aggregate.interval.as_str(), interval.as_str());
        assert_eq!(aggregate.open_time, start);
        assert_eq!(aggregate.values.open, candles[0].values.open);
        assert_eq!(aggregate.values.close, candles.last().unwrap().values.close);
        assert_eq!(
            aggregate.values.high,
            candles
                .iter()
                .map(|candle| candle.values.high.clone())
                .max()
                .unwrap()
        );
        assert_eq!(
            aggregate.values.low,
            candles
                .iter()
                .map(|candle| candle.values.low.clone())
                .min()
                .unwrap()
        );
        assert_eq!(
            aggregate.values.volume,
            candles.iter().fold(BigDecimal::from(0), |sum, candle| {
                sum + &candle.values.volume
            })
        );
    }
}

#[test]
fn aggregation_rejects_partial_unaligned_or_discontinuous_windows() {
    let config = config(time(0, 0), time(0, 10));
    let mut candles = (0..5)
        .map(|minute| config.generate_1m(time(0, minute as u32)).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        aggregate_1m_candles(&candles[..4], SyntheticKlineInterval::FiveMinutes).unwrap_err(),
        SyntheticMarketError::IncompleteAggregateWindow
    );

    candles[2].values.open = candles[2].values.close.clone();
    assert_eq!(
        aggregate_1m_candles(&candles, SyntheticKlineInterval::FiveMinutes).unwrap_err(),
        SyntheticMarketError::NonContinuousCandles
    );

    let unaligned = (1..=5)
        .map(|minute| config.generate_1m(time(0, minute as u32)).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        aggregate_1m_candles(&unaligned, SyntheticKlineInterval::FiveMinutes).unwrap_err(),
        SyntheticMarketError::IncompleteAggregateWindow
    );
}
