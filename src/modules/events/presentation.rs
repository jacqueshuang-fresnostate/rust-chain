//! events bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub(crate) struct PrivateWsQuery {
    pub(crate) token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PublicWsCommand {
    pub(crate) op: String,
    pub(crate) channel: String,
    pub(crate) symbol: Option<String>,
    pub(crate) interval: Option<String>,
}

#[derive(Debug, Deserialize)]
/// 管理端事件列表查询；分页和空状态由表现层统一规范化。
pub(crate) struct EventRecordsQuery {
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl EventRecordsQuery {
    /// 将 HTTP 查询参数规范化为应用层输入，统一空状态与分页边界。
    pub(crate) fn normalize(self) -> EventRecordListParams {
        EventRecordListParams {
            status: self
                .status
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty()),
            limit: self.limit.unwrap_or(50).clamp(1, 100),
            offset: self.offset.unwrap_or(0).min(100_000),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// 应用层可直接消费的事件列表查询参数。
pub(crate) struct EventRecordListParams {
    pub(crate) status: Option<String>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug, Serialize)]
/// 事件运维列表响应，顶层 JSON 合同固定为 `{records,total}`。
pub(crate) struct EventRecordsResponse<T> {
    pub(crate) records: Vec<T>,
    pub(crate) total: i64,
}

impl<T> EventRecordsResponse<T> {
    /// 构造事件运维列表合同，固定保留 `records` 与 `total` 两个顶层字段。
    pub(crate) fn new(records: Vec<T>, total: i64) -> Self {
        Self { records, total }
    }
}

#[derive(Debug, Serialize)]
/// outbox 运维记录响应，同时作为死信重排成功合同。
pub(crate) struct OutboxRecordResponse {
    pub(crate) id: u64,
    pub(crate) aggregate_type: String,
    pub(crate) aggregate_id: String,
    pub(crate) event_type: String,
    pub(crate) routing_key: String,
    pub(crate) status: String,
    pub(crate) retry_count: i32,
    #[serde(with = "crate::time::option_unix_millis")]
    pub(crate) next_retry_at: Option<DateTime<Utc>>,
    #[serde(with = "crate::time::option_unix_millis")]
    pub(crate) published_at: Option<DateTime<Utc>>,
    #[serde(with = "crate::time::unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
/// inbox 运维记录响应。
pub(crate) struct InboxRecordResponse {
    pub(crate) id: u64,
    pub(crate) consumer_name: String,
    pub(crate) message_id: String,
    pub(crate) status: String,
    pub(crate) retry_count: i32,
    pub(crate) error_message: Option<String>,
    #[serde(with = "crate::time::option_unix_millis")]
    pub(crate) consumed_at: Option<DateTime<Utc>>,
    #[serde(with = "crate::time::unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
/// outbox 死信重排请求；`reason` 将写入管理员审计日志。
pub(crate) struct RequeueOutboxRequest {
    pub(crate) reason: Option<String>,
}

impl RequeueOutboxRequest {
    /// 规范化死信重排原因；运维干预必须留下非空且可追溯的审计说明。
    pub(crate) fn require_reason(&self) -> crate::error::AppResult<String> {
        let reason = self.reason.as_deref().unwrap_or_default().trim().to_owned();
        if reason.is_empty() {
            return Err(crate::error::AppError::Validation(
                "reason is required".to_owned(),
            ));
        }
        Ok(reason)
    }
}
