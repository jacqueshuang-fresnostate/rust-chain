//! 时间字段的序列化适配：把内部统一使用的 UTC 时间与对外 JSON 契约中的毫秒级 Unix 时间戳互相转换。
//! 对外一律以整数毫秒表达时刻，既避免各端解析字符串格式的差异，也和前端图表、K 线的时间轴口径保持一致。
//! 两个子模块分别面向必填与可空字段，通过 `#[serde(with = ...)]` 挂到具体 DTO 字段上，不改变字段本身的可空性。
//! 编解码与数据库时间容量校验分开；业务有效期通过受检加法生成，不让配置溢出为永久会话。

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};

/// 检查当前业务表 TIMESTAMP(6) 的可存范围；不将超界时间交由数据库截断或转换为零日期。
pub fn ensure_timestamp_storage(value: &DateTime<Utc>, label: &str) -> crate::error::AppResult<()> {
    let seconds = value.timestamp();
    if !(1..=i64::from(i32::MAX)).contains(&seconds)
        || (seconds == i64::from(i32::MAX) && value.timestamp_subsec_nanos() > 499_999_000)
    {
        return Err(crate::error::AppError::Validation(format!(
            "{label} is outside the supported TIMESTAMP range"
        )));
    }
    Ok(())
}

/// 从无符号存活秒数计算可写入当前 TIMESTAMP 列的到期时间；溢出、零 TTL 或存储越界均返回错误。
pub fn checked_expiry(
    now: DateTime<Utc>,
    ttl_seconds: u64,
    label: &str,
) -> crate::error::AppResult<DateTime<Utc>> {
    let expiry = i64::try_from(ttl_seconds)
        .ok()
        .filter(|seconds| *seconds > 0)
        .and_then(Duration::try_seconds)
        .and_then(|duration| now.checked_add_signed(duration));
    let expiry = expiry.ok_or_else(|| {
        crate::error::AppError::Validation(format!("{label} is outside the supported time range"))
    })?;
    ensure_timestamp_storage(&expiry, label)?;
    Ok(expiry)
}

/// 必填时间字段的毫秒时间戳编解码，用于那些业务上一定存在取值的时刻，例如创建时间与成交时间。
pub mod unix_millis {
    use super::*;

    /// 把 UTC 时刻写成毫秒级 Unix 时间戳整数，秒以下的更高精度会被直接丢弃且不做四舍五入。
    /// 输出为有符号整数，1970 年之前的时刻会写成负数，因此消费端不能假定该字段恒为非负。
    pub fn serialize<S>(value: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(value.timestamp_millis())
    }

    /// 把毫秒级 Unix 时间戳整数还原成 UTC 时刻，输入必须是整数，字符串或浮点会在解析阶段直接失败。
    /// 数值超出可表示范围时返回自定义反序列化错误，不会回落到纪元零点或当前时间等默认值。
    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = i64::deserialize(deserializer)?;
        DateTime::<Utc>::from_timestamp_millis(millis)
            .ok_or_else(|| D::Error::custom("timestamp millis is out of range"))
    }
}

/// 可空时间字段的毫秒时间戳编解码，用于完成时间、审核时间这类只有在特定状态下才有取值的时刻。
pub mod option_unix_millis {
    use super::*;

    /// 把可空 UTC 时刻写成毫秒时间戳，空值原样输出为 JSON null，而不是零或负数哨兵值。
    /// 有值时的截断规则与必填版本一致，只保留到毫秒；调用方不应用 null 与 0 表达同一种业务含义。
    pub fn serialize<S>(value: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value
            .map(|datetime| datetime.timestamp_millis())
            .serialize(serializer)
    }

    /// 把可空毫秒时间戳还原成可空 UTC 时刻，null 与缺省都得到 `None`，不视为错误。
    /// 有值时按同样的范围规则校验，越界会让整次反序列化失败，而不是把该字段降级成 `None` 继续解析。
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
