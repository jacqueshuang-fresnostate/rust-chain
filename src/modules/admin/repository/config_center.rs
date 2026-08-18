//! 配置中心只读事实契约。

use chrono::{DateTime, Utc};

/// 单个配置域从权威存储读取的最小事实集合。
/// 该结构只携带计数、版本、运行结果与时间，不包含 SMTP、对象存储或行情源的任何凭据字段；
/// `recent_error` 仍属于内部原始摘要，进入对外 DTO 前必须经过服务层脱敏与长度裁剪。
#[derive(Debug, Clone)]
pub(crate) struct AdminConfigCenterFactRecord {
    pub(crate) code: String,
    pub(crate) configured_count: u64,
    pub(crate) pending_apply_count: u64,
    pub(crate) published_version: Option<u64>,
    pub(crate) applied_version: Option<u64>,
    pub(crate) runtime_status: String,
    pub(crate) last_modified_at: Option<DateTime<Utc>>,
    pub(crate) last_applied_at: Option<DateTime<Utc>>,
    pub(crate) last_tested_at: Option<DateTime<Utc>>,
    pub(crate) recent_error: Option<String>,
}
