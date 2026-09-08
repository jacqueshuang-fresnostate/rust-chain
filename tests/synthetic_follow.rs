use std::str::FromStr;

use bigdecimal::BigDecimal;
use chrono::{DateTime, TimeDelta, TimeZone, Utc};
use exchange_api::modules::market::{
    DefaultMarketParameters, SyntheticCandle, generate_default_1m,
    synthetic_default::{DefaultMarketFollowParameters, DefaultMarketMode},
    synthetic_follow::{
        FollowFrameInput, FollowMinuteState, FollowObservation, advance_follow_frame,
    },
};

fn d(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}
fn start() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 7, 9, 0, 0).unwrap()
}
fn params() -> DefaultMarketParameters {
    DefaultMarketParameters {
        mode: DefaultMarketMode::Follow,
        follow: Some(DefaultMarketFollowParameters {
            reference_pair_id: 42,
            multiplier: d("1"),
            max_move_ratio: d("0.5"),
            stale_after_seconds: 15,
        }),
        volume_min: d("60"),
        volume_max: d("60"),
        ..Default::default()
    }
}
fn candle(p: &DefaultMarketParameters, minute: i64, open: &str) -> SyntheticCandle {
    generate_default_1m(
        "FOLLOW-USDT",
        "follow-fixture",
        1,
        6,
        2,
        start() + TimeDelta::minutes(minute),
        &d(open),
        &d("1"),
        p,
    )
    .unwrap()
}
fn obs(second: i64, price: &str) -> FollowObservation {
    FollowObservation {
        event_key: format!("btc-{second}-{price}"),
        source: "bitget".into(),
        symbol: "BTCUSDT".into(),
        price: d(price),
        observed_at: start() + TimeDelta::seconds(second),
    }
}
fn step(
    p: &DefaultMarketParameters,
    c: &SyntheticCandle,
    prev: Option<&FollowMinuteState>,
    second: i64,
    reference: Option<FollowObservation>,
) -> FollowMinuteState {
    advance_follow_frame(
        &FollowFrameInput {
            symbol: "FOLLOW-USDT",
            parameters: p,
            price_precision: 6,
            qty_precision: 2,
            fallback: c,
            now: start() + TimeDelta::seconds(second),
            reference_symbol: "BTC-USDT",
            unavailable_reason: None,
        },
        prev,
        reference,
    )
    .unwrap()
}

#[test]
fn relative_returns_preserve_own_price_and_do_not_compound_duplicate_samples() {
    let p = params();
    let c = candle(&p, 0, "1");
    let a = step(&p, &c, None, 0, Some(obs(0, "50000")));
    let b = step(&p, &c, Some(&a), 1, Some(obs(1, "50500")));
    let repeated = step(&p, &c, Some(&b), 2, Some(obs(1, "50500")));
    let down = step(&p, &c, Some(&repeated), 3, Some(obs(3, "49500")));
    assert_eq!(a.frame.close(), &d("1"));
    assert_eq!(b.frame.close(), &d("1.01"));
    assert_eq!(repeated.frame.close(), b.frame.close());
    assert_eq!(down.frame.close(), &d("0.99"));
    assert_eq!(down.frame.high(), &d("1.01"));
    assert_eq!(down.frame.low(), &d("0.99"));
}

#[test]
fn multiplier_minute_cap_and_absolute_bounds_apply_to_each_observed_price() {
    let mut p = params();
    p.follow.as_mut().unwrap().multiplier = d("2");
    p.follow.as_mut().unwrap().max_move_ratio = d("0.03");
    p.price_min = Some(d("0.99"));
    let c = candle(&p, 0, "1");
    let a = step(&p, &c, None, 0, Some(obs(0, "100")));
    let b = step(&p, &c, Some(&a), 1, Some(obs(1, "101")));
    let capped = step(&p, &c, Some(&b), 2, Some(obs(2, "120")));
    let floor = step(&p, &c, Some(&capped), 3, Some(obs(3, "80")));
    assert_eq!(b.frame.close(), &d("1.02"));
    assert_eq!(capped.frame.close(), &d("1.03"));
    assert_eq!(floor.frame.close(), &d("0.99"));
}

#[test]
fn stale_reference_enters_independent_motion_and_recovery_reanchors_without_catchup() {
    let p = params();
    let c = candle(&p, 0, "1");
    let a = step(&p, &c, None, 0, Some(obs(0, "100")));
    let b = step(&p, &c, Some(&a), 1, Some(obs(1, "101")));
    let stale = step(&p, &c, Some(&b), 17, Some(obs(1, "101")));
    assert_eq!(stale.status.mode, "fallback");
    assert!(stale.status.fallback_reason.is_some());
    assert_eq!(stale.frame.close(), b.frame.close());
    let moving = step(&p, &c, Some(&stale), 18, None);
    assert_ne!(moving.frame.close(), stale.frame.close());
    let recovered = step(&p, &c, Some(&moving), 19, Some(obs(19, "200")));
    assert_eq!(recovered.frame.close(), moving.frame.close());
    assert_eq!(recovered.status.mode, "following");
    let next = step(&p, &c, Some(&recovered), 20, Some(obs(20, "202")));
    assert_eq!(
        next.frame.close(),
        &(recovered.frame.close() * d("1.01"))
            .with_scale_round(6, bigdecimal::RoundingMode::HalfUp)
    );
}

#[test]
fn regressed_or_conflicting_reference_never_resets_the_consumed_watermark() {
    let p = params();
    let c = candle(&p, 0, "1");
    let a = step(&p, &c, None, 0, Some(obs(0, "100")));
    let b = step(&p, &c, Some(&a), 2, Some(obs(2, "110")));
    let old = step(&p, &c, Some(&b), 3, Some(obs(1, "105")));
    assert_eq!(old.status.mode, "fallback");
    assert_eq!(old.last_reference.as_ref().unwrap().price, d("110"));
    let repeated = step(&p, &c, Some(&old), 4, Some(obs(1, "105")));
    assert_eq!(repeated.status.mode, "fallback");
    let recovered = step(&p, &c, Some(&repeated), 5, Some(obs(2, "110")));
    assert_eq!(recovered.frame.close(), repeated.frame.close());
    let conflict = step(&p, &c, Some(&recovered), 6, Some(obs(2, "120")));
    assert_eq!(conflict.status.mode, "fallback");
    assert_eq!(conflict.frame.close(), recovered.frame.close());
}

#[test]
fn restored_same_second_is_frozen_and_provider_changes_reanchor() {
    let p = params();
    let c = candle(&p, 0, "1");
    let a = step(&p, &c, None, 0, Some(obs(0, "100")));
    let b = step(&p, &c, Some(&a), 1, Some(obs(1, "110")));
    let restored: FollowMinuteState =
        serde_json::from_value(serde_json::to_value(&b).unwrap()).unwrap();
    assert_eq!(step(&p, &c, Some(&restored), 1, Some(obs(1, "150"))), b);
    let mut other = obs(2, "200");
    other.source = "coinbase".into();
    let changed = step(&p, &c, Some(&b), 2, Some(other));
    assert_eq!(changed.frame.close(), b.frame.close());
    let future = step(&p, &c, Some(&changed), 3, Some(obs(4, "202")));
    assert_eq!(future.status.mode, "fallback");
}

#[test]
fn minute_ohlc_tracks_only_observed_prices_and_keeps_own_volume_plan() {
    let p = params();
    let c = candle(&p, 0, "1");
    let mut state = None;
    for sec in 0..60 {
        let next = step(
            &p,
            &c,
            state.as_ref(),
            sec,
            Some(obs(sec, if sec % 2 == 0 { "100" } else { "101" })),
        );
        assert!(next.frame.low() <= next.frame.close() && next.frame.close() <= next.frame.high());
        state = Some(next);
    }
    let state = state.unwrap();
    assert_eq!(state.frame.volume(), &d("60"));
    assert_eq!(state.frame.high(), &d("1.01"));
    assert_eq!(state.frame.low(), &d("1"));
    let next_candle = candle(&p, 1, &state.frame.close().to_string());
    let next = step(&p, &next_candle, Some(&state), 60, Some(obs(60, "102.01")));
    assert_eq!(next.frame.open(), state.frame.close());
    assert_eq!(next.frame.close(), &d("1.0201"));
    assert!(next.frame.volume() < &d("60"));
}

#[test]
fn subsecond_reference_is_not_consumed_ahead_of_frozen_second_cutoff() {
    let p = params();
    let c = candle(&p, 0, "1");
    let mut tick = obs(1, "100");
    tick.observed_at += TimeDelta::milliseconds(500);
    let state = advance_follow_frame(
        &FollowFrameInput {
            symbol: "FOLLOW-USDT",
            parameters: &p,
            price_precision: 6,
            qty_precision: 2,
            fallback: &c,
            now: start() + TimeDelta::milliseconds(1800),
            reference_symbol: "BTC-USDT",
            unavailable_reason: None,
        },
        None,
        Some(tick.clone()),
    )
    .unwrap();
    assert_eq!(state.status.mode, "fallback");
    assert_eq!(state.frame.observed_at(), start() + TimeDelta::seconds(1));
    let next = step(&p, &c, Some(&state), 2, Some(tick));
    assert_eq!(next.status.mode, "following");
}
