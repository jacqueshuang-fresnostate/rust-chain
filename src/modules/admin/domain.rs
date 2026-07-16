//! admin bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务逻辑逐步迁入。

use chrono::{DateTime, TimeDelta, Utc};
use thiserror::Error;

/// 管理端执行上下文，用于承载鉴权后的管理员身份与权限快照。
#[derive(Debug, Clone)]
pub struct AdminScope {
    pub admin_id: String,
    pub permissions: Vec<String>,
}

/// 敏感管理操作确认的领域实体。
/// 用于记录管理员在高风险操作时的元信息与过期时间。
#[derive(Debug, Clone)]
pub struct SensitiveOperationConfirmation {
    admin_id: String,
    operation: String,
    target_type: String,
    target_id: String,
    reason: String,
    requested_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

impl SensitiveOperationConfirmation {
    pub fn new(
        admin_id: impl Into<String>,
        operation: impl Into<String>,
        target_type: impl Into<String>,
        target_id: impl Into<String>,
        reason: impl Into<String>,
        requested_at: DateTime<Utc>,
        ttl: TimeDelta,
    ) -> Result<Self, SensitiveConfirmationError> {
        if ttl <= TimeDelta::zero() {
            return Err(SensitiveConfirmationError::InvalidTtl);
        }

        Ok(Self {
            admin_id: admin_id.into(),
            operation: operation.into(),
            target_type: target_type.into(),
            target_id: target_id.into(),
            reason: reason.into(),
            requested_at,
            expires_at: requested_at + ttl,
        })
    }

    pub fn admin_id(&self) -> &str {
        &self.admin_id
    }

    pub fn operation(&self) -> &str {
        &self.operation
    }

    pub fn target_type(&self) -> &str {
        &self.target_type
    }

    pub fn target_id(&self) -> &str {
        &self.target_id
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }

    pub fn requested_at(&self) -> DateTime<Utc> {
        self.requested_at
    }

    pub fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }

    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        now >= self.expires_at
    }

    pub fn audit_metadata_key(&self) -> String {
        format!(
            "admin-sensitive:{}:{}:{}:{}:{}",
            self.admin_id,
            self.operation,
            self.target_type,
            self.target_id,
            self.requested_at.timestamp()
        )
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SensitiveConfirmationError {
    #[error("sensitive confirmation ttl must be positive")]
    InvalidTtl,
}
