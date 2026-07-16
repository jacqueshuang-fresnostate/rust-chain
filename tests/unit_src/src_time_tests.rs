use super::*;
use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct TimestampPayload {
    #[serde(with = "unix_millis")]
    at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Serialize)]
struct OptionalTimestampPayload {
    #[serde(default, with = "option_unix_millis")]
    at: Option<DateTime<Utc>>,
}

#[test]
fn unix_millis_serializes_datetime_as_number() {
    let payload = TimestampPayload {
        at: Utc.with_ymd_and_hms(2026, 5, 29, 13, 30, 15).unwrap(),
    };

    let value = serde_json::to_value(payload).unwrap();

    assert_eq!(value["at"], 1_780_061_415_000_i64);
}

#[test]
fn unix_millis_deserializes_datetime_from_number() {
    let payload: TimestampPayload = serde_json::from_value(serde_json::json!({
        "at": 1_780_061_415_000_i64
    }))
    .unwrap();

    assert_eq!(
        payload.at,
        Utc.with_ymd_and_hms(2026, 5, 29, 13, 30, 15).unwrap()
    );
}

#[test]
fn option_unix_millis_handles_null_and_numbers() {
    let missing: OptionalTimestampPayload = serde_json::from_value(serde_json::json!({
        "at": null
    }))
    .unwrap();
    assert_eq!(missing.at, None);

    let present: OptionalTimestampPayload = serde_json::from_value(serde_json::json!({
        "at": 1_780_061_415_000_i64
    }))
    .unwrap();
    assert_eq!(
        present.at,
        Some(Utc.with_ymd_and_hms(2026, 5, 29, 13, 30, 15).unwrap())
    );

    let value = serde_json::to_value(present).unwrap();
    assert_eq!(value["at"], 1_780_061_415_000_i64);
}
