//! 跟随历史与运行链共享的纯证据选择合同；不接触在线行情或本地数据库。

use chrono::{DateTime, Duration, TimeZone, Utc};
use exchange_api::modules::market::synthetic_follow::{FollowObservation, select_follow_reference};

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 7, 10, 0, 0).unwrap()
}
fn tick(source: &str, age: i64, price: i32) -> FollowObservation {
    FollowObservation {
        event_key: format!("{source}:{age}:{price}"),
        source: source.into(),
        symbol: "BTCUSDT".into(),
        price: price.into(),
        observed_at: now() - Duration::seconds(age),
    }
}

#[test]
fn preferred_healthy_provider_is_stable_then_changes_only_after_expiry() {
    let candidates = vec![tick("bitget", 40, 100), tick("htx", 1, 110)];
    let selected = select_follow_reference("BTC-USDT", &candidates, Some("bitget"), now(), 60);
    assert_eq!(selected.observation.unwrap().source, "bitget");
    let selected = select_follow_reference("BTC-USDT", &candidates, Some("bitget"), now(), 15);
    assert_eq!(selected.observation.unwrap().source, "htx");
    assert!(selected.reason.is_none());
}

#[test]
fn future_stale_nonexternal_wrong_symbol_blank_identity_and_nonpositive_prices_are_not_reference() {
    let mut wrong = tick("bitget", 0, 100);
    wrong.symbol = "ETHUSDT".into();
    let mut blank = tick("bitget", 0, 100);
    blank.event_key.clear();
    for candidates in [
        vec![tick("bitget", -1, 100)],
        vec![tick("htx", 61, 100)],
        vec![tick("default", 0, 100)],
        vec![tick("strategy", 0, 100)],
        vec![wrong],
        vec![blank],
        vec![tick("coinbase", 0, 0)],
        vec![tick("coinbase", 0, -1)],
    ] {
        let selected = select_follow_reference("BTCUSDT", &candidates, None, now(), 60);
        assert!(selected.observation.is_none());
        assert!(selected.reason.unwrap().contains("独立震荡"));
    }
    assert!(
        select_follow_reference("BTCUSDT", &[tick("bitget", 60, 100)], None, now(), 60)
            .observation
            .is_some()
    );
}

#[test]
fn same_timestamp_collision_fails_closed_but_equal_duplicates_are_stable() {
    let candidates = vec![
        tick("bitget", 1, 100),
        tick("bitget", 1, 101),
        tick("htx", 0, 200),
    ];
    let selected = select_follow_reference("BTCUSDT", &candidates, Some("bitget"), now(), 60);
    assert!(selected.observation.is_none());
    assert!(selected.reason.unwrap().contains("冲突"));
    let mut same = tick("bitget", 1, 100);
    same.event_key = "zz-final".into();
    let candidates = vec![same.clone(), tick("bitget", 1, 100)];
    assert_eq!(
        select_follow_reference("BTCUSDT", &candidates, None, now(), 60).observation,
        Some(same)
    );
}

#[test]
fn candidate_order_does_not_change_provider_or_event_selection() {
    let mut candidates = vec![
        tick("bitget", 1, 100),
        tick("htx", 1, 100),
        tick("coinbase", 1, 100),
    ];
    let expected = select_follow_reference("BTCUSDT", &candidates, None, now(), 60);
    candidates.reverse();
    assert_eq!(
        select_follow_reference("BTCUSDT", &candidates, None, now(), 60),
        expected
    );
}
