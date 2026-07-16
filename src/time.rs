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
#[path = "../tests/unit_src/src_time_tests.rs"]
mod tests;
