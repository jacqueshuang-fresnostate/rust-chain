use super::*;
use crate::modules::market::{
    MarketDataProvider, MarketKlineSnapshot,
    synthetic_realtime::{build_synthetic_market_details_from_candle, forming_1m_values},
};
use chrono::{Duration, TimeZone};

#[test]
fn default_second_path_matches_closed_candle_and_exact_volume_after_restart() {
    let open_time = Utc.with_ymd_and_hms(2026, 9, 7, 10, 0, 0).unwrap();
    let parameters = DefaultMarketParameters {
        volume_min: BigDecimal::new(60000000000000000017_i128.into(), 18),
        volume_max: BigDecimal::new(60000000000000000017_i128.into(), 18),
        depth_levels: 3,
        ..Default::default()
    };
    let closed = generate_default_1m(
        "DFTUSDT",
        "persisted-seed",
        1,
        8,
        18,
        open_time,
        &BigDecimal::from(10),
        &BigDecimal::from(10),
        &parameters,
    )
    .unwrap();
    let mut total = BigDecimal::from(0);
    let mut ids = std::collections::HashSet::new();
    for elapsed in 0..60 {
        let observed_at = open_time + Duration::seconds(elapsed);
        let values = forming_1m_values(&closed.values, open_time, observed_at, 8).unwrap();
        assert!(values.high <= closed.values.high && values.low >= closed.values.low);
        let current = MarketKlineSnapshot::new(
            MarketDataProvider::Strategy,
            "DFTUSDT",
            "1m",
            open_time,
            values.clone(),
            observed_at,
        )
        .unwrap();
        let details = build_synthetic_market_details_from_candle(
            "default:1:g1",
            "persisted-seed",
            1,
            8,
            18,
            3,
            &closed,
            &current,
        )
        .unwrap();
        let replayed_closed = generate_default_1m(
            "DFTUSDT",
            "persisted-seed",
            1,
            8,
            18,
            open_time,
            &BigDecimal::from(10),
            &BigDecimal::from(10),
            &parameters,
        )
        .unwrap();
        assert_eq!(closed, replayed_closed);
        assert_eq!(
            details,
            build_synthetic_market_details_from_candle(
                "default:1:g1",
                "persisted-seed",
                1,
                8,
                18,
                3,
                &replayed_closed,
                &current
            )
            .unwrap()
        );
        let trade = details.1.unwrap();
        assert!(ids.insert(trade.trade_id().to_owned()));
        assert_eq!(trade.price(), current.close());
        total += trade.quantity();
        if elapsed == 59 {
            assert_eq!(values, closed.values);
        }
    }
    assert_eq!(total, closed.values.volume);
}

#[test]
fn default_minimum_price_and_zero_volume_do_not_fabricate_trade_or_negative_depth() {
    let open_time = Utc.with_ymd_and_hms(2026, 9, 7, 10, 0, 0).unwrap();
    let parameters = DefaultMarketParameters {
        volume_min: BigDecimal::from(0),
        volume_max: BigDecimal::from(0),
        ..Default::default()
    };
    let price = BigDecimal::new(1.into(), 18);
    let closed = generate_default_1m(
        "DFTUSDT",
        "seed",
        1,
        18,
        18,
        open_time,
        &price,
        &price,
        &parameters,
    )
    .unwrap();
    let current = MarketKlineSnapshot::new(
        MarketDataProvider::Strategy,
        "DFTUSDT",
        "1m",
        open_time,
        forming_1m_values(&closed.values, open_time, open_time, 18).unwrap(),
        open_time,
    )
    .unwrap();
    let (depth, trade) = build_synthetic_market_details_from_candle(
        "default:1:g1",
        "seed",
        1,
        18,
        18,
        20,
        &closed,
        &current,
    )
    .unwrap();
    assert!(depth.bids().is_empty());
    assert!(
        depth
            .asks()
            .iter()
            .all(|row| row.price > price && row.quantity > 0)
    );
    assert!(trade.is_none());
}

#[test]
fn generic_details_reject_wrong_candle_time_source_or_forming_values() {
    let open_time = Utc.with_ymd_and_hms(2026, 9, 7, 10, 0, 0).unwrap();
    let closed = generate_default_1m(
        "DFTUSDT",
        "seed",
        1,
        8,
        8,
        open_time,
        &BigDecimal::from(10),
        &BigDecimal::from(10),
        &DefaultMarketParameters::default(),
    )
    .unwrap();
    for (provider, slot, observation, values) in [
        (
            MarketDataProvider::Coinbase,
            open_time,
            open_time,
            closed.values.clone(),
        ),
        (
            MarketDataProvider::Strategy,
            open_time + Duration::minutes(1),
            open_time + Duration::minutes(1),
            closed.values.clone(),
        ),
        (
            MarketDataProvider::Strategy,
            open_time,
            open_time,
            closed.values.clone(),
        ),
        (
            MarketDataProvider::Strategy,
            open_time,
            open_time - Duration::seconds(1),
            closed.values.clone(),
        ),
        (
            MarketDataProvider::Strategy,
            open_time,
            open_time + Duration::minutes(1),
            closed.values.clone(),
        ),
    ] {
        let current =
            MarketKlineSnapshot::new(provider, "DFTUSDT", "1m", slot, values, observation).unwrap();
        assert!(
            build_synthetic_market_details_from_candle(
                "default:1:g1",
                "seed",
                1,
                8,
                8,
                20,
                &closed,
                &current
            )
            .is_err()
        );
    }
}

#[test]
fn default_trade_deltas_honor_coarse_quantity_precision_without_losing_minute_volume() {
    let open_time = Utc.with_ymd_and_hms(2026, 9, 7, 10, 0, 0).unwrap();
    for qty_precision in [0, 2, 8, 18] {
        let volume = BigDecimal::from(10) + BigDecimal::new(3.into(), i64::from(qty_precision));
        let parameters = DefaultMarketParameters {
            volume_min: volume.clone(),
            volume_max: volume.clone(),
            ..Default::default()
        };
        let closed = generate_default_1m(
            "DFTUSDT",
            "seed",
            1,
            8,
            qty_precision,
            open_time,
            &BigDecimal::from(10),
            &BigDecimal::from(10),
            &parameters,
        )
        .unwrap();
        let mut total = BigDecimal::from(0);
        for second in 0..60 {
            let observed_at = open_time + Duration::seconds(second);
            let values =
                forming_default_1m_values(&closed.values, open_time, observed_at, 8, qty_precision)
                    .unwrap();
            assert_eq!(
                values.volume,
                values.volume.with_scale(i64::from(qty_precision))
            );
            let current = MarketKlineSnapshot::new(
                MarketDataProvider::Strategy,
                "DFTUSDT",
                "1m",
                open_time,
                values,
                observed_at,
            )
            .unwrap();
            let (_, trade) = build_synthetic_market_details_from_candle(
                "default:1:g1",
                "seed",
                1,
                8,
                qty_precision,
                3,
                &closed,
                &current,
            )
            .unwrap();
            if let Some(trade) = trade {
                assert!(trade.quantity() > &BigDecimal::from(0));
                assert_eq!(
                    *trade.quantity(),
                    trade.quantity().with_scale(i64::from(qty_precision))
                );
                total += trade.quantity();
            }
        }
        assert_eq!(total, volume);
    }
}
