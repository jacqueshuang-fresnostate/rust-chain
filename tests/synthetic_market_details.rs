use bigdecimal::BigDecimal;
use chrono::{Duration, TimeZone, Utc};
use exchange_api::{
    modules::market::{
        SyntheticKlineInterval, SyntheticMarketConfig,
        synthetic_realtime::{build_forming_aggregate, build_synthetic_market_details},
    },
    workers::synthetic_market::build_realtime_plan,
};
use std::str::FromStr;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}
fn config() -> SyntheticMarketConfig {
    let start = Utc.with_ymd_and_hms(2026, 9, 6, 10, 2, 0).unwrap();
    SyntheticMarketConfig::new(SyntheticMarketConfig {
        symbol: "SIMUSDT".into(),
        seed: "book-seed".into(),
        version: 1,
        price_precision: 8,
        start_time: start,
        end_time: start + Duration::days(2),
        start_price: decimal("10"),
        target_price: decimal("20"),
        volatility: decimal("0.01"),
        volume_min: decimal("60.000000000000000017"),
        volume_max: decimal("60.000000000000000017"),
        generator: Default::default(),
        nodes: vec![],
    })
    .unwrap()
}

#[test]
fn synthetic_book_and_trades_share_price_replay_and_exact_minute_volume() {
    let config = config();
    let mut total = decimal("0");
    let mut ids = std::collections::HashSet::new();
    for second in 0..60 {
        let plan = build_realtime_plan(
            7,
            &config,
            config.start_time + Duration::seconds(second),
            &[],
        )
        .unwrap();
        let (depth, trade) = build_synthetic_market_details(7, &config, 8, plan.kline()).unwrap();
        assert_eq!(
            (depth.clone(), trade.clone()),
            build_synthetic_market_details(7, &config, 8, plan.kline()).unwrap()
        );
        assert_eq!(depth.bids().len(), 20);
        assert_eq!(depth.asks().len(), 20);
        assert!(
            depth
                .bids()
                .windows(2)
                .all(|pair| pair[0].price > pair[1].price)
        );
        assert!(
            depth
                .asks()
                .windows(2)
                .all(|pair| pair[0].price < pair[1].price)
        );
        assert!(
            depth
                .bids()
                .iter()
                .all(|level| &level.price < plan.ticker().last_price() && level.quantity > 0)
        );
        assert!(
            depth
                .asks()
                .iter()
                .all(|level| &level.price > plan.ticker().last_price() && level.quantity > 0)
        );
        let trade = trade.unwrap();
        assert_eq!(trade.price(), plan.kline().close());
        assert!(ids.insert(trade.trade_id().to_owned()));
        assert!(trade.quantity() > &decimal("0"));
        total += trade.quantity();
    }
    assert_eq!(
        total,
        config.generate_1m(config.start_time).unwrap().values.volume
    );
}

#[test]
fn low_price_and_zero_volume_never_produce_negative_bids_or_fake_trades() {
    let mut config = config();
    config.start_price = decimal("0.00000001");
    config.target_price = config.start_price.clone();
    config.volatility = decimal("0");
    config.volume_min = decimal("0");
    config.volume_max = decimal("0");
    let plan = build_realtime_plan(1, &config, config.start_time, &[]).unwrap();
    let (depth, trade) = build_synthetic_market_details(1, &config, 8, plan.kline()).unwrap();
    assert!(trade.is_none());
    assert!(depth.bids().is_empty());
    assert!(
        depth
            .asks()
            .iter()
            .all(|level| level.price > 0 && level.quantity > 0)
    );
}

#[test]
fn each_timeframe_forms_immediately_without_inventing_missing_roots() {
    let config = config();
    let now = config.start_time + Duration::minutes(2) + Duration::seconds(30);
    let current = build_realtime_plan(1, &config, now, &[]).unwrap();
    let first = config.generate_1m(config.start_time).unwrap();
    let future = config
        .generate_1m(config.start_time + Duration::minutes(3))
        .unwrap();
    for interval in [
        SyntheticKlineInterval::FiveMinutes,
        SyntheticKlineInterval::FifteenMinutes,
        SyntheticKlineInterval::OneHour,
        SyntheticKlineInterval::FourHours,
        SyntheticKlineInterval::OneDay,
    ] {
        let empty = build_forming_aggregate(current.kline(), interval, &[]).unwrap();
        assert_eq!(empty.close(), current.kline().close());
        assert_eq!(empty.volume(), current.kline().volume());
        assert_eq!(
            empty.open_time().timestamp() % (interval.minute_count() as i64 * 60),
            0
        );
        let formed = build_forming_aggregate(
            current.kline(),
            interval,
            &[future.clone(), first.clone(), first.clone()],
        )
        .unwrap();
        assert_eq!(formed.open(), &first.values.open);
        assert_eq!(formed.close(), current.kline().close());
        assert_eq!(
            formed.volume(),
            &(first.values.volume.clone() + current.kline().volume())
        );
        assert_eq!(
            formed.high(),
            &first
                .values
                .high
                .clone()
                .max(current.kline().high().clone())
        );
        assert_eq!(
            formed.low(),
            &first.values.low.clone().min(current.kline().low().clone())
        );
    }
}
