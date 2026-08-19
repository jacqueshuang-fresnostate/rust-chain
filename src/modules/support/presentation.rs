//! support 限界上下文的 HTTP 输入与 JSON 输出契约。
//!
//! 请求结构仅保留传输层原始形状，正文长度、幂等键、状态和分页边界由领域层收敛。
//! 响应时间统一序列化为 Unix 毫秒，会话响应同时携带服务端未读数、两条已读游标与直属代理。
//! 消息的 `read_by_recipient` 从该会话游标计算，不伪造单条回执时间。

use crate::{
    architecture::PresentationLayer,
    modules::support::repository::{SupportConversationRecord, SupportMessageRecord},
    time::{option_unix_millis, unix_millis},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 用户首次发言前会话不存在，因此查询结果以可空字段表达诚实空态。
#[derive(Debug, Serialize)]
pub(crate) struct UserSupportConversationResponse {
    pub(crate) conversation: Option<SupportConversationResponse>,
}

impl PresentationLayer for UserSupportConversationResponse {}

/// 用户、代理和管理员共用的会话读模型。
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SupportConversationResponse {
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
    #[serde(default, with = "option_unix_millis")]
    pub(crate) last_message_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) closed_at: Option<DateTime<Utc>>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

impl PresentationLayer for SupportConversationResponse {}

impl From<SupportConversationRecord> for SupportConversationResponse {
    /// 将基础设施读模型逐字段映射为对外会话契约，不重算归属或未读数。
    /// 时间保留 UTC 值到 serde 边界才转毫秒，避免应用层在多次组装中重复截断。
    fn from(record: SupportConversationRecord) -> Self {
        Self {
            id: record.id,
            user_id: record.user_id,
            user_email: record.user_email,
            user_phone: record.user_phone,
            assigned_agent_id: record.assigned_agent_id,
            assigned_agent_code: record.assigned_agent_code,
            status: record.status,
            user_read_message_id: record.user_read_message_id,
            staff_read_message_id: record.staff_read_message_id,
            user_unread_count: record.user_unread_count,
            staff_unread_count: record.staff_unread_count,
            last_message_id: record.last_message_id,
            last_message_sender_type: record.last_message_sender_type,
            last_message_sender_id: record.last_message_sender_id,
            last_message_preview: record.last_message_preview,
            last_message_at: record
                .last_message_at
                .map(|value| DateTime::from_naive_utc_and_offset(value, Utc)),
            closed_at: record
                .closed_at
                .map(|value| DateTime::from_naive_utc_and_offset(value, Utc)),
            created_at: DateTime::from_naive_utc_and_offset(record.created_at, Utc),
            updated_at: DateTime::from_naive_utc_and_offset(record.updated_at, Utc),
        }
    }
}

/// 代理或管理员队列的有界分页响应。
#[derive(Debug, Serialize)]
pub(crate) struct SupportConversationsResponse {
    pub(crate) conversations: Vec<SupportConversationResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for SupportConversationsResponse {}

/// 单条不可变客服消息，已读标记是按会话游标推导的快照。
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SupportMessageResponse {
    pub(crate) id: u64,
    pub(crate) conversation_id: u64,
    pub(crate) sender_type: String,
    pub(crate) sender_id: u64,
    pub(crate) client_message_id: String,
    pub(crate) body: String,
    pub(crate) read_by_recipient: bool,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for SupportMessageResponse {}

impl SupportMessageResponse {
    /// 按当前会话的双侧游标映射消息已读状态：用户消息看 staff 游标，
    /// 代理与管理员消息看 user 游标。未知发送类型按未读处理，不扩大回执语义；
    /// 本映射不更新游标，因此反复序列化同一快照必然得到相同结果。
    pub(crate) fn from_record(
        record: SupportMessageRecord,
        user_read_message_id: Option<u64>,
        staff_read_message_id: Option<u64>,
    ) -> Self {
        let read_by_recipient = match record.sender_type.as_str() {
            "user" => staff_read_message_id.is_some_and(|cursor| cursor >= record.id),
            "agent" | "admin" => user_read_message_id.is_some_and(|cursor| cursor >= record.id),
            _ => false,
        };
        Self {
            id: record.id,
            conversation_id: record.conversation_id,
            sender_type: record.sender_type,
            sender_id: record.sender_id,
            client_message_id: record.client_message_id,
            body: record.body,
            read_by_recipient,
            created_at: DateTime::from_naive_utc_and_offset(record.created_at, Utc),
        }
    }
}

/// 消息历史的游标分页响应，数组始终按 ID 升序供聊天界面直接追加。
#[derive(Debug, Serialize)]
pub(crate) struct SupportMessagesResponse {
    pub(crate) messages: Vec<SupportMessageResponse>,
    pub(crate) has_more: bool,
    pub(crate) next_before_id: Option<u64>,
}

impl PresentationLayer for SupportMessagesResponse {}

impl SupportMessagesResponse {
    /// 为尚未建立会话的用户返回稳定空页，不使用 404 表达正常首次使用。
    pub(crate) fn empty() -> Self {
        Self {
            messages: Vec::new(),
            has_more: false,
            next_before_id: None,
        }
    }
}

/// 发送成功响应同时返回消息与最新会话，`replayed` 指示是否命中幂等重放。
#[derive(Debug, Serialize)]
pub(crate) struct SupportSendMessageResponse {
    pub(crate) conversation: SupportConversationResponse,
    pub(crate) message: SupportMessageResponse,
    pub(crate) replayed: bool,
}

impl PresentationLayer for SupportSendMessageResponse {}

/// 代理队列查询；归属范围始终由令牌解析，请求不含代理 ID。
#[derive(Debug, Deserialize)]
pub(crate) struct AgentSupportConversationsQuery {
    pub(crate) status: Option<String>,
    pub(crate) unread_only: Option<bool>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AgentSupportConversationsQuery {}

/// 管理员队列查询，可显式选择某直属代理或仅查未分配会话。
#[derive(Debug, Deserialize)]
pub(crate) struct AdminSupportConversationsQuery {
    pub(crate) status: Option<String>,
    pub(crate) unread_only: Option<bool>,
    pub(crate) assigned_agent_id: Option<u64>,
    pub(crate) unassigned: Option<bool>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminSupportConversationsQuery {}

/// 用户、代理与管理员共用的消息游标查询。
#[derive(Debug, Deserialize)]
pub(crate) struct SupportMessagesQuery {
    pub(crate) limit: Option<u32>,
    pub(crate) before_id: Option<u64>,
}

impl PresentationLayer for SupportMessagesQuery {}

/// 发送文本请求，幂等键必须由客户端生成并在重试时复用。
#[derive(Debug, Deserialize)]
pub(crate) struct SendSupportMessageRequest {
    pub(crate) body: String,
    pub(crate) client_message_id: String,
}

impl PresentationLayer for SendSupportMessageRequest {}

/// 推进已读游标的请求，目标消息必须属于当前会话。
#[derive(Debug, Deserialize)]
pub(crate) struct MarkSupportReadRequest {
    pub(crate) message_id: u64,
}

impl PresentationLayer for MarkSupportReadRequest {}

/// 打开或关闭会话的状态请求。
#[derive(Debug, Deserialize)]
pub(crate) struct UpdateSupportStatusRequest {
    pub(crate) status: String,
}

impl PresentationLayer for UpdateSupportStatusRequest {}
