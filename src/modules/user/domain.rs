//! user bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。
//! 当前文件承载用户上下文中最基础的输入规范化原语和邮箱验证码失效判定，
//! 全部为纯函数，不触碰数据库、缓存或外部服务，可被 service 与 application 层安全复用。
//! 这里刻意不放置任何隐私字段的脱敏逻辑，脱敏由 presentation 层在出站时统一处理。

use crate::error::{AppError, AppResult};
use chrono::{DateTime, Utc};

/// 单条邮箱验证码允许的最大校验失败次数，达到该值后即使未到过期时间也判定为不可用，
/// 用于阻断针对六位数字验证码的暴力枚举。
pub(crate) const EMAIL_VERIFICATION_CODE_MAX_ATTEMPTS: i32 = 5;

/// 规范化必填字符串：去除首尾空白后要求结果非空，否则返回 `AppError::Validation`。
/// 错误消息固定为 `<field> is required`，字段名由调用方传入，因此同一函数可服务多个表单字段。
/// 只做存在性与空白校验，不限制长度、字符集或格式，具体格式约束由各自的 service 校验函数负责。
pub(crate) fn required_string(value: Option<String>, field: &str) -> AppResult<String> {
    optional_string(value).ok_or_else(|| AppError::Validation(format!("{field} is required")))
}

/// 规范化可选字符串：去除首尾空白，并把纯空白或空串统一折叠为 `None`。
/// 这样上层就无需区分「字段缺省」与「字段传了空串」两种前端写法，二者语义一致视为未填写。
/// 返回 `Some` 时保证内容非空且两端无空白，可直接落库或参与后续长度判定。
pub(crate) fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 判定一条待验证的邮箱验证码是否已经失效，把时间维度和次数维度两条规则收敛到同一处。
/// 满足任一条件即失效：`expires_at` 不晚于 `now`（含边界，到点即过期），
/// 或累计失败次数已达 `EMAIL_VERIFICATION_CODE_MAX_ATTEMPTS`。
/// `now` 由调用方注入而非内部取系统时间，便于测试固定时钟，也保证同一请求内多处判定使用同一时间基准。
/// 本函数只做判定，不负责把记录标记为作废，作废写入由 infrastructure 层在事务内完成。
pub(crate) fn email_verification_is_expired(
    expires_at: DateTime<Utc>,
    attempt_count: i32,
    now: DateTime<Utc>,
) -> bool {
    expires_at <= now || attempt_count >= EMAIL_VERIFICATION_CODE_MAX_ATTEMPTS
}
