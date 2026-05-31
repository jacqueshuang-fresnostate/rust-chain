use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};

pub mod unix_millis {
    use super::*;

    pub fn serialize<S>(value: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(value.timestamp_millis())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = i64::deserialize(deserializer)?;
        DateTime::<Utc>::from_timestamp_millis(millis)
            .ok_or_else(|| D::Error::custom("timestamp millis is out of range"))
    }
}

pub mod option_unix_millis {
    use super::*;

    pub fn serialize<S>(value: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value
            .map(|datetime| datetime.timestamp_millis())
            .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<i64>::deserialize(deserializer)?
            .map(|millis| {
                DateTime::<Utc>::from_timestamp_millis(millis)
                    .ok_or_else(|| D::Error::custom("timestamp millis is out of range"))
            })
            .transpose()
    }
}

#[cfg(test)]
mod tests {
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
}
