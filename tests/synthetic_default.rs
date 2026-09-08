use std::str::FromStr;

use bigdecimal::BigDecimal;
use chrono::{Duration, TimeZone, Utc};
use exchange_api::modules::market::{
    DefaultMarketParameters, MarketDataProvider, MarketKlineSnapshot, SyntheticCandle,
    generate_default_1m, synthetic_realtime::build_synthetic_market_details_from_candle,
};
use serde_json::json;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}
fn minute() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 7, 10, 0, 0).unwrap()
}
fn candle(parameters: &DefaultMarketParameters, price: &str) -> SyntheticCandle {
    generate_default_1m(
        "DFTUSDT",
        "persisted-seed",
        1,
        8,
        8,
        minute(),
        &decimal(price),
        &decimal("10"),
        parameters,
    )
    .unwrap()
}

#[test]
fn default_parameters_are_conservative_serialized_decimals_without_an_implicit_initial_price() {
    let defaults = DefaultMarketParameters::default();
    assert_eq!(
        serde_json::from_value::<DefaultMarketParameters>(json!({})).unwrap(),
        defaults
    );
    assert_eq!(
        serde_json::to_value(&defaults).unwrap(),
        json!({
            "mode": "independent",
            "follow": null,
            "volatility": "0.0015", "mean_reversion": "0.05", "price_min": null, "price_max": null,
            "volume_min": "10", "volume_max": "20", "wick_strength": "0.25", "depth_levels": 20,
        })
    );
    assert!(
        serde_json::from_value::<DefaultMarketParameters>(json!({"initial_price":"1"})).is_err()
    );
    assert!(
        serde_json::from_value::<DefaultMarketParameters>(json!({"volatility":"NaN"})).is_err()
    );
    assert!(
        serde_json::from_value::<DefaultMarketParameters>(json!({"volatility":"Infinity"}))
            .is_err()
    );
}

#[test]
fn default_generation_replays_from_persisted_minute_open_and_changes_with_identity() {
    let parameters = DefaultMarketParameters::default();
    let expected = candle(&parameters, "10");
    let restored: DefaultMarketParameters =
        serde_json::from_value(serde_json::to_value(&parameters).unwrap()).unwrap();
    assert_eq!(expected, candle(&restored, "10"));
    let normalized = generate_default_1m(
        "dft-usdt",
        "persisted-seed",
        1,
        8,
        8,
        minute(),
        &decimal("10"),
        &decimal("10"),
        &parameters,
    )
    .unwrap();
    assert_eq!(expected, normalized);
    for (symbol, seed, version) in [
        ("OTHERUSDT", "persisted-seed", 1),
        ("DFTUSDT", "other-seed", 1),
        ("DFTUSDT", "persisted-seed", 2),
    ] {
        let other = generate_default_1m(
            symbol,
            seed,
            version,
            8,
            8,
            minute(),
            &decimal("10"),
            &decimal("10"),
            &parameters,
        )
        .unwrap();
        assert_ne!(expected, other);
    }
}

#[test]
fn consecutive_default_minutes_are_continuous_positive_bounded_and_precision_exact() {
    let parameters = DefaultMarketParameters {
        price_min: Some(decimal("9.99")),
        price_max: Some(decimal("10.01")),
        ..Default::default()
    };
    let mut open = decimal("10");
    for index in 0..1440 {
        let generated = generate_default_1m(
            "DFTUSDT",
            "persisted-seed",
            1,
            8,
            3,
            minute() + Duration::minutes(index),
            &open,
            &decimal("10"),
            &parameters,
        )
        .unwrap();
        let values = generated.values;
        assert_eq!(values.open, open);
        assert!((&values.close - &open).abs() <= &open * &parameters.volatility);
        assert!(values.high >= values.open.clone().max(values.close.clone()));
        assert!(values.low <= values.open.clone().min(values.close.clone()));
        assert!(values.high <= decimal("10.01") && values.low >= decimal("9.99"));
        for price in [&values.open, &values.high, &values.low, &values.close] {
            assert_eq!(*price, price.with_scale(8));
        }
        assert!(values.volume >= parameters.volume_min && values.volume <= parameters.volume_max);
        assert_eq!(values.volume, values.volume.with_scale(3));
        open = values.close;
    }
}

#[test]
fn zero_volatility_zero_volume_and_minimum_tick_allow_truthful_flat_candles() {
    let parameters = DefaultMarketParameters {
        volatility: decimal("0"),
        volume_min: decimal("0"),
        volume_max: decimal("0"),
        ..Default::default()
    };
    for precision in [0, 8, 18] {
        let price = BigDecimal::new(1.into(), i64::from(precision));
        let generated = generate_default_1m(
            "DFTUSDT",
            "seed",
            1,
            precision,
            precision,
            minute(),
            &price,
            &price,
            &parameters,
        )
        .unwrap();
        assert_eq!(generated.values.open, price);
        assert_eq!(generated.values.high, price);
        assert_eq!(generated.values.low, price);
        assert_eq!(generated.values.close, price);
        assert_eq!(generated.values.volume, decimal("0"));
    }
    let flat = generate_default_1m(
        "DFTUSDT",
        "seed",
        1,
        0,
        0,
        minute(),
        &decimal("1"),
        &decimal("1"),
        &DefaultMarketParameters::default(),
    )
    .unwrap();
    assert_eq!(flat.values.close, decimal("1"));
}

#[test]
fn parameter_validation_rejects_invalid_bounds_ratios_precisions_and_storage_overflow() {
    for field in ["volatility", "mean_reversion", "wick_strength"] {
        for value in ["-0.1", "1.00000001"] {
            let parameters: DefaultMarketParameters =
                serde_json::from_value(json!({field: value})).unwrap();
            assert!(parameters.validate(8, 8).is_err(), "{field}={value}");
        }
    }
    for value in [
        json!({"price_min":"0"}),
        json!({"price_max":"-1"}),
        json!({"price_min":"2", "price_max":"1"}),
        json!({"price_min":"0.000000001"}),
        json!({"volume_min":"-1"}),
        json!({"volume_min":"21"}),
        json!({"volume_max":"20.000000001"}),
        json!({"depth_levels":0}),
        json!({"depth_levels":21}),
        json!({"price_max":"1e100000"}),
        json!({"volatility":"1e-100000"}),
    ] {
        let parameters: DefaultMarketParameters = serde_json::from_value(value.clone()).unwrap();
        assert!(parameters.validate(8, 8).is_err(), "{value}");
    }
    assert!(DefaultMarketParameters::default().validate(19, 8).is_err());
    assert!(DefaultMarketParameters::default().validate(8, 19).is_err());
}

#[test]
fn generation_rejects_invalid_identity_time_open_anchor_and_out_of_band_open() {
    let parameters = DefaultMarketParameters::default();
    for (symbol, seed, version, time, open, anchor) in [
        ("", "seed", 1, minute(), "10", "10"),
        ("DFTUSDT", "", 1, minute(), "10", "10"),
        ("DFTUSDT", "seed", 0, minute(), "10", "10"),
        (
            "DFTUSDT",
            "seed",
            1,
            minute() + Duration::milliseconds(1),
            "10",
            "10",
        ),
        (
            "DFTUSDT",
            "seed",
            1,
            minute() + Duration::seconds(1),
            "10",
            "10",
        ),
        ("DFTUSDT", "seed", 1, minute(), "0", "10"),
        ("DFTUSDT", "seed", 1, minute(), "10", "-1"),
        ("DFTUSDT", "seed", 1, minute(), "10.000000001", "10"),
        ("DFTUSDT", "seed", 1, minute(), "10", "10.000000001"),
    ] {
        assert!(
            generate_default_1m(
                symbol,
                seed,
                version,
                8,
                8,
                time,
                &decimal(open),
                &decimal(anchor),
                &parameters
            )
            .is_err()
        );
    }
    let bounded = DefaultMarketParameters {
        price_min: Some(decimal("11")),
        ..Default::default()
    };
    assert!(
        generate_default_1m(
            "DFTUSDT",
            "seed",
            1,
            8,
            8,
            minute(),
            &decimal("10"),
            &decimal("10"),
            &bounded
        )
        .is_err()
    );
}

#[test]
fn generic_details_respect_depth_limit_deterministic_identity_price_and_zero_volume() {
    let parameters = DefaultMarketParameters {
        depth_levels: 3,
        volume_min: decimal("60"),
        volume_max: decimal("60"),
        ..Default::default()
    };
    let closed = candle(&parameters, "10");
    let current = MarketKlineSnapshot::new(
        MarketDataProvider::Strategy,
        "DFTUSDT",
        "1m",
        minute(),
        closed.values.clone(),
        minute() + Duration::seconds(59),
    )
    .unwrap();
    let result = build_synthetic_market_details_from_candle(
        "default:7:g1",
        "persisted-seed",
        1,
        8,
        8,
        3,
        &closed,
        &current,
    )
    .unwrap();
    assert_eq!(
        result,
        build_synthetic_market_details_from_candle(
            "default:7:g1",
            "persisted-seed",
            1,
            8,
            8,
            3,
            &closed,
            &current
        )
        .unwrap()
    );
    let (depth, trade) = result;
    assert_eq!(depth.bids().len(), 3);
    assert_eq!(depth.asks().len(), 3);
    assert!(
        depth
            .bids()
            .iter()
            .all(|row| row.price < *current.close() && row.price > 0 && row.quantity > 0)
    );
    assert!(
        depth
            .asks()
            .iter()
            .all(|row| row.price > *current.close() && row.quantity > 0)
    );
    let trade = trade.unwrap();
    assert_eq!(
        trade.trade_id(),
        format!("default:7:g1:{}", current.observed_at().timestamp())
    );
    assert_eq!(trade.price(), current.close());
    assert_eq!(trade.quantity(), &decimal("1"));
    for depth_levels in [0, 21] {
        assert!(
            build_synthetic_market_details_from_candle(
                "default:7:g1",
                "persisted-seed",
                1,
                8,
                8,
                depth_levels,
                &closed,
                &current
            )
            .is_err()
        );
    }
}
