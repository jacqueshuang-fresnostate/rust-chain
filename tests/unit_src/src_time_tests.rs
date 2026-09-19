use super::*;

#[test]
fn expiry_rejects_unsigned_wrap_and_timestamp_storage_overflow() {
    let now = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
    assert_eq!(
        checked_expiry(now, 60, "TTL").unwrap().timestamp(),
        1_700_000_060
    );
    for ttl in [0, u64::MAX, i64::MAX as u64, 253_402_300_799] {
        assert!(checked_expiry(now, ttl, "TTL").is_err(), "{ttl}");
    }
    let last_second = DateTime::<Utc>::from_timestamp(253_402_300_799, 0).unwrap();
    assert!(checked_expiry(last_second, 1, "TTL").is_err());
    let last_storable = DateTime::<Utc>::from_timestamp(i64::from(i32::MAX), 499_999_000).unwrap();
    assert!(ensure_timestamp_storage(&last_storable, "expiry").is_ok());
    assert!(checked_expiry(last_storable, 1, "TTL").is_err());
    for (seconds, nanos) in [
        (0, 0),
        (i64::from(i32::MAX), 500_000_000),
        (i64::from(i32::MAX) + 1, 0),
    ] {
        assert!(
            ensure_timestamp_storage(&DateTime::from_timestamp(seconds, nanos).unwrap(), "expiry")
                .is_err()
        );
    }
}
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
