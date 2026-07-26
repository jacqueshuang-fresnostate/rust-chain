//! events bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务逻辑逐步迁入。

use serde::Deserialize;

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
pub(crate) struct EventRecordsQuery {
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl EventRecordsQuery {
    pub(crate) fn as_filter(
        &self,
    ) -> crate::modules::events::infrastructure::EventRecordListFilter<'_> {
        crate::modules::events::infrastructure::EventRecordListFilter {
            status: self
                .status
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
            limit: self.limit.unwrap_or(50).clamp(1, 100),
            offset: self.offset.unwrap_or(0).min(100_000),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct RequeueOutboxRequest {
    pub(crate) reason: Option<String>,
}

impl RequeueOutboxRequest {
    /// 重放属于运维干预，必须留下可追溯的原因。
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
