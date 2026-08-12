//! user bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务逻辑逐步迁入。

use crate::error::{AppError, AppResult};
use chrono::{DateTime, Utc};

pub(crate) const EMAIL_VERIFICATION_CODE_MAX_ATTEMPTS: i32 = 5;

/// 规范化必填字符串并拒绝空白值，错误消息保留调用方传入的字段名。
pub(crate) fn required_string(value: Option<String>, field: &str) -> AppResult<String> {
    optional_string(value).ok_or_else(|| AppError::Validation(format!("{field} is required")))
}

/// 去除字符串首尾空白，并把空结果统一转换为 `None`。
pub(crate) fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 判断验证码是否已经不可继续使用，统一过期时间和最大尝试次数的业务规则。
pub(crate) fn email_verification_is_expired(
    expires_at: DateTime<Utc>,
    attempt_count: i32,
    now: DateTime<Utc>,
) -> bool {
    expires_at <= now || attempt_count >= EMAIL_VERIFICATION_CODE_MAX_ATTEMPTS
}
