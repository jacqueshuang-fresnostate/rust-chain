//! admin bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务逻辑逐步迁入。

use chrono::{DateTime, TimeDelta, Utc};
use std::collections::BTreeSet;
use thiserror::Error;

/// 管理端执行上下文，承载每次请求回查到的管理员、角色与权限快照。
/// 该快照不写入 JWT，因此角色权限或账号状态改变会在下一次请求生效；
/// 权限集合已去重且按字典序保存，便于前端稳定缓存与测试比较。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminScope {
    pub admin_id: u64,
    pub username: String,
    pub role_id: u64,
    pub role_name: String,
    pub permissions: BTreeSet<String>,
}

impl AdminScope {
    /// 判定快照是否允许指定权限码。
    /// `*` 允许全部操作，`wallet.*` 这类域通配符允许对应前缀下的权限；
    /// 空权限集默认拒绝，不再沿用历史上“空配置等于全权限”的隐式行为。
    pub fn allows(&self, permission: &str) -> bool {
        if self.permissions.contains("*") || self.permissions.contains(permission) {
            return true;
        }

        permission
            .match_indices('.')
            .map(|(index, _)| format!("{}.*", &permission[..index]))
            .any(|wildcard| self.permissions.contains(&wildcard))
    }

    /// 返回稳定排序的权限数组，供传输层序列化与前端权限导航使用。
    pub fn permission_list(&self) -> Vec<String> {
        self.permissions.iter().cloned().collect()
    }
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
    /// 创建敏感管理操作确认，固化管理员、操作目标、原因以及请求和过期时间。
    /// 有效期必须为正；该领域构造不访问外部资源，校验失败不会产生任何持久化副作用。
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

    /// 返回发起该敏感操作确认的管理员标识。
    pub fn admin_id(&self) -> &str {
        &self.admin_id
    }

    /// 返回本次确认所绑定的敏感操作代码。
    pub fn operation(&self) -> &str {
        &self.operation
    }

    /// 返回敏感操作目标的资源类型。
    pub fn target_type(&self) -> &str {
        &self.target_type
    }

    /// 返回敏感操作目标的业务标识。
    pub fn target_id(&self) -> &str {
        &self.target_id
    }

    /// 返回管理员提交且已固化的操作原因。
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// 返回确认请求创建时刻，用于审计与元数据键生成。
    pub fn requested_at(&self) -> DateTime<Utc> {
        self.requested_at
    }

    /// 返回由请求时刻和正有效期推导出的失效时刻。
    pub fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }

    /// 判断给定时刻是否已到达确认失效边界；等于失效时刻也视为过期。
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        now >= self.expires_at
    }

    /// 根据管理员、操作、目标和请求秒级时间生成稳定审计元数据键。
    /// 键不包含操作原因或过期时间，调用方不得把它当作授权凭证或全局唯一数据库约束。
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
