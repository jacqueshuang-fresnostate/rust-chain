//! support 限界上下文的持久化契约。
//!
//! 本层只定义会话、消息、锁定快照与队列筛选的数据形状，不拼接 SQL、不开启事务。
//! 这些结构是 infrastructure 与 application 之间的稳定边界：基础设施负责填充，
//! 应用层只依据明确字段做授权、幂等和提交后广播编排。

use crate::architecture::RepositoryLayer;
use chrono::NaiveDateTime;

/// 会话详情读模型，同时携带服务端计算的双侧未读数。
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct SupportConversationRecord {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    pub(crate) user_email: Option<String>,
    pub(crate) user_phone: Option<String>,
    pub(crate) assigned_agent_id: Option<u64>,
    pub(crate) assigned_agent_code: Option<String>,
    pub(crate) status: String,
    pub(crate) user_read_message_id: Option<u64>,
    pub(crate) staff_read_message_id: Option<u64>,
    pub(crate) user_unread_count: i64,
    pub(crate) staff_unread_count: i64,
    pub(crate) last_message_id: Option<u64>,
    pub(crate) last_message_sender_type: Option<String>,
    pub(crate) last_message_sender_id: Option<u64>,
    pub(crate) last_message_preview: Option<String>,
    pub(crate) last_message_at: Option<NaiveDateTime>,
    pub(crate) closed_at: Option<NaiveDateTime>,
    pub(crate) created_at: NaiveDateTime,
    pub(crate) updated_at: NaiveDateTime,
}

impl RepositoryLayer for SupportConversationRecord {}

/// 事务内锁定的会话最小快照，不包含列表联表与计数子查询。
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct SupportConversationLockRecord {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    pub(crate) assigned_agent_id: Option<u64>,
}

impl RepositoryLayer for SupportConversationLockRecord {}

/// 不可变消息读模型；发送者类型与 ID 必须成对解释。
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct SupportMessageRecord {
    pub(crate) id: u64,
    pub(crate) conversation_id: u64,
    pub(crate) sender_type: String,
    pub(crate) sender_id: u64,
    pub(crate) client_message_id: String,
    pub(crate) body: String,
    pub(crate) created_at: NaiveDateTime,
}

impl RepositoryLayer for SupportMessageRecord {}

/// 客服队列的服务端可见范围；代理范围只是精确所有者 ID。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SupportStaffScope {
    Agent(u64),
    Admin,
}

impl RepositoryLayer for SupportStaffScope {}

/// 客服队列查询契约，所有分页值在进入基础设施层前已收敛。
#[derive(Debug, Clone)]
pub(crate) struct SupportConversationListFilter {
    pub(crate) scope: SupportStaffScope,
    pub(crate) status: Option<String>,
    pub(crate) unread_only: bool,
    pub(crate) assigned_agent_id: Option<u64>,
    pub(crate) unassigned_only: bool,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

impl RepositoryLayer for SupportConversationListFilter {}
