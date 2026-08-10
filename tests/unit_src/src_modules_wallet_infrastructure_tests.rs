use super::{
    return_history_historical_close_if_valid, return_history_kline_document_close_if_valid,
    today_return_ticker_price_if_current,
};
use bigdecimal::BigDecimal;
use chrono::{TimeDelta, TimeZone, Utc};
use mongodb::bson::{DateTime as BsonDateTime, doc};
use serde_json::json;
use std::{collections::BTreeSet, str::FromStr};

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

#[test]
fn return_history_close_requires_requested_utc_day_and_positive_decimal() {
    let requested_day = Utc
        .with_ymd_and_hms(2026, 8, 8, 0, 0, 0)
        .unwrap()
        .date_naive();
    let requested_days = BTreeSet::from([requested_day]);
    let open_time = requested_day.and_hms_opt(0, 0, 0).unwrap().and_utc();

    assert_eq!(
        return_history_historical_close_if_valid(
            open_time.timestamp_millis(),
            "50000.125000000000000000",
            &requested_days,
        ),
        Some((requested_day, decimal("50000.125000000000000000")))
    );

    for (millis, close) in [
        (open_time.timestamp_millis() + 1, "50000"),
        ((open_time + TimeDelta::days(1)).timestamp_millis(), "50000"),
        (open_time.timestamp_millis(), "0"),
        (open_time.timestamp_millis(), "-1"),
        (open_time.timestamp_millis(), "broken"),
    ] {
        assert_eq!(
            return_history_historical_close_if_valid(millis, close, &requested_days),
            None
        );
    }
}

#[test]
fn return_history_malformed_kline_document_becomes_missing_price() {
    let requested_day = Utc
        .with_ymd_and_hms(2026, 8, 8, 0, 0, 0)
        .unwrap()
        .date_naive();
    let requested_days = BTreeSet::from([requested_day]);
    let open_time = BsonDateTime::from_millis(
        requested_day
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis(),
    );

    assert_eq!(
        return_history_kline_document_close_if_valid(
            &doc! { "open_time": open_time, "close": "50000" },
            &requested_days,
        ),
        Some((requested_day, decimal("50000")))
    );
    for document in [
        doc! { "open_time": open_time, "close": 50000_i64 },
        doc! { "open_time": "broken", "close": "50000" },
        doc! { "open_time": open_time },
    ] {
        assert_eq!(
            return_history_kline_document_close_if_valid(&document, &requested_days),
            None
        );
    }
}
