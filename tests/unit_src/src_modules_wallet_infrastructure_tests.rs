use super::today_return_ticker_price_if_current;
use bigdecimal::BigDecimal;
use chrono::{TimeDelta, TimeZone, Utc};
use serde_json::json;
use std::str::FromStr;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

fn ticker_payload(symbol: &str, price: &str, observed_at: chrono::DateTime<Utc>) -> String {
    json!({
        "symbol": symbol,
        "last_price": price,
        "observed_at": observed_at.timestamp_millis(),
    })
    .to_string()
}

#[test]
fn today_return_ticker_requires_matching_positive_fresh_payload() {
    let calculated_at = Utc.with_ymd_and_hms(2026, 8, 9, 18, 0, 0).unwrap();
    let fresh = ticker_payload(
        "BTCUSDT",
        "50000.125000000000000000",
        calculated_at - TimeDelta::seconds(60),
    );

    assert_eq!(
        today_return_ticker_price_if_current("btc", &fresh, calculated_at),
        Some(decimal("50000.125000000000000000"))
    );

    for payload in [
        ticker_payload("BTCUSDT", "50000", calculated_at - TimeDelta::seconds(61)),
        ticker_payload(
            "BTCUSDT",
            "50000",
            calculated_at + TimeDelta::milliseconds(1),
        ),
        ticker_payload("ETHUSDT", "50000", calculated_at),
        ticker_payload("BTCUSDT", "0", calculated_at),
        ticker_payload("BTCUSDT", "-1", calculated_at),
        r#"{"symbol":"BTCUSDT","last_price":"broken","observed_at":0}"#.to_owned(),
        r#"{"symbol":"BTCUSDT","last_price":"50000"}"#.to_owned(),
    ] {
        assert_eq!(
            today_return_ticker_price_if_current("BTC", &payload, calculated_at),
            None,
            "payload must not be called current: {payload}"
        );
    }
}
