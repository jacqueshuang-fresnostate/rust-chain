//! 用户、代理与管理员在线客服 REST 契约。
//!
//! 文档形状与生产路由保持一致：用户无路径会话 ID，代理只能访问精确归属会话，
//! 管理员依赖运行时 `support.conversations.read/write` 并可作为未分配用户的全局兜底。
//! 所有时间都是 Unix 毫秒，消息页有界且按 ID 升序，WebSocket 只是可丢失的刷新提示，
//! 因此本文件仅把 REST 声明为权威数据合同。

use super::*;

#[derive(ToSchema)]
pub(super) struct UserSupportConversationResponse {
    conversation: Option<SupportConversationResponse>,
}

#[derive(ToSchema)]
pub(super) struct SupportConversationResponse {
    id: u64,
    user_id: u64,
    user_email: Option<String>,
    user_phone: Option<String>,
    assigned_agent_id: Option<u64>,
    assigned_agent_code: Option<String>,
    #[schema(pattern = "^(open|closed)$")]
    status: String,
    user_read_message_id: Option<u64>,
    staff_read_message_id: Option<u64>,
    user_unread_count: i64,
    staff_unread_count: i64,
    last_message_id: Option<u64>,
    #[schema(pattern = "^(user|agent|admin)$")]
    last_message_sender_type: Option<String>,
    last_message_sender_id: Option<u64>,
    last_message_preview: Option<String>,
    #[schema(format = Int64)]
    last_message_at: Option<i64>,
    #[schema(format = Int64)]
    closed_at: Option<i64>,
    #[schema(format = Int64)]
    created_at: i64,
    #[schema(format = Int64)]
    updated_at: i64,
}

#[derive(ToSchema)]
pub(super) struct SupportConversationsResponse {
    conversations: Vec<SupportConversationResponse>,
    total: i64,
}

#[derive(ToSchema)]
pub(super) struct SupportMessageResponse {
    id: u64,
    conversation_id: u64,
    #[schema(pattern = "^(user|agent|admin)$")]
    sender_type: String,
    sender_id: u64,
    client_message_id: String,
    body: String,
    read_by_recipient: bool,
    #[schema(format = Int64)]
    created_at: i64,
}

#[derive(ToSchema)]
pub(super) struct SupportMessagesResponse {
    messages: Vec<SupportMessageResponse>,
    has_more: bool,
    next_before_id: Option<u64>,
}

#[derive(ToSchema)]
pub(super) struct SendSupportMessageRequest {
    #[schema(pattern = "^[\\s\\S]{1,2000}$")]
    body: String,
    #[schema(pattern = "^[A-Za-z0-9_-]{8,64}$")]
    client_message_id: String,
}

#[derive(ToSchema)]
pub(super) struct SupportSendMessageResponse {
    conversation: SupportConversationResponse,
    message: SupportMessageResponse,
    replayed: bool,
}

#[derive(ToSchema)]
pub(super) struct MarkSupportReadRequest {
    message_id: u64,
}

#[derive(ToSchema)]
pub(super) struct UpdateSupportStatusRequest {
    #[schema(pattern = "^(open|closed)$")]
    status: String,
}

/// 查询当前用户唯一会话；首次发言前 conversation 为 null。
#[utoipa::path(
    get,
    path = "/api/v1/support/conversation",
    tag = "support",
    summary = "查询我的客服会话",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = UserSupportConversationResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "令牌作用域不符", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_user_support_conversation() {}

/// 按消息 ID 游标分页查询当前用户历史，读取不自动标记已读。
#[utoipa::path(
    get,
    path = "/api/v1/support/conversation/messages",
    tag = "support",
    summary = "查询我的客服消息",
    params(
        ("limit" = Option<u32>, Query, description = "返回条数，默认 50，最大 100"),
        ("before_id" = Option<u64>, Query, description = "仅返回 ID 小于该值的消息")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = SupportMessagesResponse),
        (status = 400, description = "分页参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_user_support_messages() {}

/// 发送用户文本；首条创建会话，新消息自动重开 closed 会话。
#[utoipa::path(
    post,
    path = "/api/v1/support/conversation/messages",
    tag = "support",
    summary = "发送客服消息",
    request_body = SendSupportMessageRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "发送或幂等重放成功", body = SupportSendMessageResponse),
        (status = 400, description = "正文或客户端键错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 409, description = "同键被用于不同正文", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn send_user_support_message() {}

/// 单调推进当前用户的已读游标。
#[utoipa::path(
    post,
    path = "/api/v1/support/conversation/read",
    tag = "support",
    summary = "标记我的客服消息已读",
    request_body = MarkSupportReadRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "更新成功", body = SupportConversationResponse),
        (status = 400, description = "消息 ID 错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 404, description = "会话或消息不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn mark_user_support_read() {}

/// 由当前用户打开或关闭会话。
#[utoipa::path(
    patch,
    path = "/api/v1/support/conversation/status",
    tag = "support",
    summary = "更新我的客服会话状态",
    request_body = UpdateSupportStatusRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "更新成功", body = SupportConversationResponse),
        (status = 400, description = "状态错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 404, description = "会话不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn update_user_support_status() {}

/// 分页查询当前代理精确所有的会话队列。
#[utoipa::path(
    get,
    path = "/agent/api/v1/support/conversations",
    tag = "agent-support",
    summary = "查询代理客服队列",
    params(
        ("status" = Option<String>, Query, description = "open 或 closed"),
        ("unread_only" = Option<bool>, Query, description = "仅客服侧未读"),
        ("limit" = Option<u32>, Query, description = "默认 50，最大 100"),
        ("offset" = Option<u32>, Query, description = "默认 0，最大 100000")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = SupportConversationsResponse),
        (status = 400, description = "筛选错误", body = ErrorResponse),
        (status = 401, description = "未登录或代理链停用", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_agent_support_conversations() {}

/// 查询当前代理精确所有的单条会话。
#[utoipa::path(
    get,
    path = "/agent/api/v1/support/conversations/{id}",
    tag = "agent-support",
    summary = "查询代理客服会话",
    params(("id" = u64, Path, description = "会话 ID")),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = SupportConversationResponse),
        (status = 401, description = "未登录或代理链停用", body = ErrorResponse),
        (status = 404, description = "会话不存在或不属于当前代理", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_agent_support_conversation() {}

/// 分页查询当前代理精确所有会话的消息。
#[utoipa::path(
    get,
    path = "/agent/api/v1/support/conversations/{id}/messages",
    tag = "agent-support",
    summary = "查询代理客服消息",
    params(
        ("id" = u64, Path, description = "会话 ID"),
        ("limit" = Option<u32>, Query, description = "默认 50，最大 100"),
        ("before_id" = Option<u64>, Query, description = "仅返回 ID 小于该值的消息")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = SupportMessagesResponse),
        (status = 400, description = "分页错误", body = ErrorResponse),
        (status = 401, description = "未登录或代理链停用", body = ErrorResponse),
        (status = 404, description = "会话不存在或不属于当前代理", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_agent_support_messages() {}

/// 当前代理回复其精确所有的会话。
#[utoipa::path(
    post,
    path = "/agent/api/v1/support/conversations/{id}/messages",
    tag = "agent-support",
    summary = "回复代理客服会话",
    params(("id" = u64, Path, description = "会话 ID")),
    request_body = SendSupportMessageRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "发送或幂等重放成功", body = SupportSendMessageResponse),
        (status = 400, description = "发送内容错误", body = ErrorResponse),
        (status = 401, description = "未登录或代理链停用", body = ErrorResponse),
        (status = 404, description = "会话不存在或不属于当前代理", body = ErrorResponse),
        (status = 409, description = "同键不同正文", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn send_agent_support_message() {}

/// 推进当前代理精确所有会话的 staff 已读游标。
#[utoipa::path(
    post,
    path = "/agent/api/v1/support/conversations/{id}/read",
    tag = "agent-support",
    summary = "标记代理客服会话已读",
    params(("id" = u64, Path, description = "会话 ID")),
    request_body = MarkSupportReadRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "更新成功", body = SupportConversationResponse),
        (status = 400, description = "消息 ID 错误", body = ErrorResponse),
        (status = 401, description = "未登录或代理链停用", body = ErrorResponse),
        (status = 404, description = "会话或消息不可见", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn mark_agent_support_read() {}

/// 当前代理关闭或重开其精确所有会话。
#[utoipa::path(
    patch,
    path = "/agent/api/v1/support/conversations/{id}/status",
    tag = "agent-support",
    summary = "更新代理客服会话状态",
    params(("id" = u64, Path, description = "会话 ID")),
    request_body = UpdateSupportStatusRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "更新成功", body = SupportConversationResponse),
        (status = 400, description = "状态错误", body = ErrorResponse),
        (status = 401, description = "未登录或代理链停用", body = ErrorResponse),
        (status = 404, description = "会话不存在或不属于当前代理", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn update_agent_support_status() {}

/// 管理员分页查询全局队列，可筛选未分配用户。
#[utoipa::path(
    get,
    path = "/admin/api/v1/support/conversations",
    tag = "admin-support",
    summary = "查询全局客服队列",
    params(
        ("status" = Option<String>, Query, description = "open 或 closed"),
        ("unread_only" = Option<bool>, Query, description = "仅客服侧未读"),
        ("assigned_agent_id" = Option<u64>, Query, description = "精确直属代理 ID"),
        ("unassigned" = Option<bool>, Query, description = "仅未分配，不可与 assigned_agent_id 并用"),
        ("limit" = Option<u32>, Query, description = "默认 50，最大 100"),
        ("offset" = Option<u32>, Query, description = "默认 0，最大 100000")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = SupportConversationsResponse),
        (status = 400, description = "筛选错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "缺少 support.conversations.read", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_admin_support_conversations() {}

/// 管理员查询任意已分配或未分配会话。
#[utoipa::path(
    get,
    path = "/admin/api/v1/support/conversations/{id}",
    tag = "admin-support",
    summary = "查询全局客服会话",
    params(("id" = u64, Path, description = "会话 ID")),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = SupportConversationResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "缺少 support.conversations.read", body = ErrorResponse),
        (status = 404, description = "会话不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_admin_support_conversation() {}

/// 管理员分页查询任意会话的消息。
#[utoipa::path(
    get,
    path = "/admin/api/v1/support/conversations/{id}/messages",
    tag = "admin-support",
    summary = "查询全局客服消息",
    params(
        ("id" = u64, Path, description = "会话 ID"),
        ("limit" = Option<u32>, Query, description = "默认 50，最大 100"),
        ("before_id" = Option<u64>, Query, description = "仅返回 ID 小于该值的消息")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = SupportMessagesResponse),
        (status = 400, description = "分页错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "缺少 support.conversations.read", body = ErrorResponse),
        (status = 404, description = "会话不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_admin_support_messages() {}

/// 管理员以独立 admin 发送者身份回复任意会话。
#[utoipa::path(
    post,
    path = "/admin/api/v1/support/conversations/{id}/messages",
    tag = "admin-support",
    summary = "管理员回复客服会话",
    params(("id" = u64, Path, description = "会话 ID")),
    request_body = SendSupportMessageRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "发送或幂等重放成功", body = SupportSendMessageResponse),
        (status = 400, description = "发送内容错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "缺少 support.conversations.write", body = ErrorResponse),
        (status = 404, description = "会话不存在", body = ErrorResponse),
        (status = 409, description = "同键不同正文", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn send_admin_support_message() {}

/// 管理员推进任意会话的 staff 已读游标。
#[utoipa::path(
    post,
    path = "/admin/api/v1/support/conversations/{id}/read",
    tag = "admin-support",
    summary = "管理员标记客服会话已读",
    params(("id" = u64, Path, description = "会话 ID")),
    request_body = MarkSupportReadRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "更新成功", body = SupportConversationResponse),
        (status = 400, description = "消息 ID 错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "缺少 support.conversations.write", body = ErrorResponse),
        (status = 404, description = "会话或消息不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn mark_admin_support_read() {}

/// 管理员关闭或重开任意已分配或未分配会话。
#[utoipa::path(
    patch,
    path = "/admin/api/v1/support/conversations/{id}/status",
    tag = "admin-support",
    summary = "管理员更新客服会话状态",
    params(("id" = u64, Path, description = "会话 ID")),
    request_body = UpdateSupportStatusRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "更新成功", body = SupportConversationResponse),
        (status = 400, description = "状态错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "缺少 support.conversations.write", body = ErrorResponse),
        (status = 404, description = "会话不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn update_admin_support_status() {}
