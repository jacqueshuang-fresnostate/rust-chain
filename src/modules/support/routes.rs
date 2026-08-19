//! support 限界上下文的用户、代理与管理员 HTTP 路由。
//!
//! 三套路由只负责身份提取、Path/Query/JSON 解析、运行时依赖转交和 JSON 封装。
//! 文本校验、代理精确归属、行锁、幂等、已读游标与状态迁移全部由应用层处理。
//! 管理员路由仍使用 `AdminAuth`，因此会先经运行时 `support.conversations.read/write` 映射；
//! 代理路由不接受 agent_id，任何可见范围都只能由 token subject 解析。

use crate::{
    error::AppResult,
    modules::{
        auth::{AdminAuth, AgentAuth, UserAuth},
        support::{
            application::{
                get_admin_support_conversation, get_agent_support_conversation,
                get_user_support_conversation, list_admin_support_conversations,
                list_admin_support_messages, list_agent_support_conversations,
                list_agent_support_messages, list_user_support_messages, mark_admin_support_read,
                mark_agent_support_read, mark_user_support_read, send_admin_support_message,
                send_agent_support_message, send_user_support_message, update_admin_support_status,
                update_agent_support_status, update_user_support_status,
            },
            presentation::{
                AdminSupportConversationsQuery, AgentSupportConversationsQuery,
                MarkSupportReadRequest, SendSupportMessageRequest, SupportConversationResponse,
                SupportConversationsResponse, SupportMessagesQuery, SupportMessagesResponse,
                SupportSendMessageResponse, UpdateSupportStatusRequest,
                UserSupportConversationResponse,
            },
        },
    },
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, patch, post},
};

/// 装配 `/api/v1/support` 下的用户会话、历史、发送、已读与状态端点。
/// 每个 handler 都强制 `UserAuth`，路径上不暴露 user_id 或 conversation_id，
/// 因此用户无法通过参数选择他人会话；首条发送由应用层按用户唯一键创建。
pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/support/conversation", get(user_conversation))
        .route(
            "/support/conversation/messages",
            get(user_messages).post(user_send_message),
        )
        .route("/support/conversation/read", post(user_mark_read))
        .route("/support/conversation/status", patch(user_update_status))
}

/// 装配 `/agent/api/v1/support` 下的精确所有者队列与会话操作。
/// 所有 handler 仅接受 `AgentAuth` subject 与会话 ID，不接受代理树 path、root ID 或客户端归属；
/// 父代报表的子树能力不会进入本路由，不属于当前代理的 ID 统一返回不可见。
pub fn agent_routes() -> Router<AppState> {
    Router::new()
        .route("/support/conversations", get(agent_conversations))
        .route("/support/conversations/:id", get(agent_conversation))
        .route(
            "/support/conversations/:id/messages",
            get(agent_messages).post(agent_send_message),
        )
        .route("/support/conversations/:id/read", post(agent_mark_read))
        .route(
            "/support/conversations/:id/status",
            patch(agent_update_status),
        )
}

/// 装配 `/admin/api/v1/support` 下的全局队列与会话操作，包含未分配用户。
/// `AdminAuth` 会根据 HTTP 方法在每次请求中回查 read/write 权限，未映射路由不会默认放行；
/// 应用层保留管理员作为独立发送者，不会把平台回复伪装成代理回复。
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/support/conversations", get(admin_conversations))
        .route("/support/conversations/:id", get(admin_conversation))
        .route(
            "/support/conversations/:id/messages",
            get(admin_messages).post(admin_send_message),
        )
        .route("/support/conversations/:id/read", post(admin_mark_read))
        .route(
            "/support/conversations/:id/status",
            patch(admin_update_status),
        )
}

/// 查询当前用户会话；无会话时返回 `{conversation:null}`。
async fn user_conversation(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<UserSupportConversationResponse>> {
    Ok(Json(
        get_user_support_conversation(state.mysql.clone(), &claims.sub).await?,
    ))
}

/// 分页查询当前用户历史，不隐式标记已读。
async fn user_messages(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<SupportMessagesQuery>,
) -> AppResult<Json<SupportMessagesResponse>> {
    Ok(Json(
        list_user_support_messages(state.mysql.clone(), &claims.sub, query).await?,
    ))
}

/// 提交当前用户文本，并仅在事务提交后转交可选广播 hub。
async fn user_send_message(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<SendSupportMessageRequest>,
) -> AppResult<Json<SupportSendMessageResponse>> {
    Ok(Json(
        send_user_support_message(
            state.mysql.clone(),
            &claims.sub,
            request,
            state.event_broadcast_hub.as_ref(),
        )
        .await?,
    ))
}

/// 推进当前用户已读游标并返回更新后会话。
async fn user_mark_read(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<MarkSupportReadRequest>,
) -> AppResult<Json<SupportConversationResponse>> {
    Ok(Json(
        mark_user_support_read(state.mysql.clone(), &claims.sub, request).await?,
    ))
}

/// 打开或关闭当前用户会话并返回最新状态。
async fn user_update_status(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<UpdateSupportStatusRequest>,
) -> AppResult<Json<SupportConversationResponse>> {
    Ok(Json(
        update_user_support_status(state.mysql.clone(), &claims.sub, request).await?,
    ))
}

/// 查询当前精确代理队列，代理 ID 只取自 token。
async fn agent_conversations(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
    Query(query): Query<AgentSupportConversationsQuery>,
) -> AppResult<Json<SupportConversationsResponse>> {
    Ok(Json(
        list_agent_support_conversations(state.mysql.clone(), &claims.sub, query).await?,
    ))
}

/// 查询当前精确代理所属的单条会话。
async fn agent_conversation(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
    Path(conversation_id): Path<u64>,
) -> AppResult<Json<SupportConversationResponse>> {
    Ok(Json(
        get_agent_support_conversation(state.mysql.clone(), &claims.sub, conversation_id).await?,
    ))
}

/// 分页查询当前精确代理所属会话的消息。
async fn agent_messages(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
    Path(conversation_id): Path<u64>,
    Query(query): Query<SupportMessagesQuery>,
) -> AppResult<Json<SupportMessagesResponse>> {
    Ok(Json(
        list_agent_support_messages(state.mysql.clone(), &claims.sub, conversation_id, query)
            .await?,
    ))
}

/// 以当前精确代理身份回复所属会话。
async fn agent_send_message(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
    Path(conversation_id): Path<u64>,
    Json(request): Json<SendSupportMessageRequest>,
) -> AppResult<Json<SupportSendMessageResponse>> {
    Ok(Json(
        send_agent_support_message(
            state.mysql.clone(),
            &claims.sub,
            conversation_id,
            request,
            state.event_broadcast_hub.as_ref(),
        )
        .await?,
    ))
}

/// 推进当前精确代理会话的 staff 已读游标。
async fn agent_mark_read(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
    Path(conversation_id): Path<u64>,
    Json(request): Json<MarkSupportReadRequest>,
) -> AppResult<Json<SupportConversationResponse>> {
    Ok(Json(
        mark_agent_support_read(state.mysql.clone(), &claims.sub, conversation_id, request).await?,
    ))
}

/// 打开或关闭当前精确代理所属的会话。
async fn agent_update_status(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
    Path(conversation_id): Path<u64>,
    Json(request): Json<UpdateSupportStatusRequest>,
) -> AppResult<Json<SupportConversationResponse>> {
    Ok(Json(
        update_agent_support_status(state.mysql.clone(), &claims.sub, conversation_id, request)
            .await?,
    ))
}

/// 查询管理员全局队列，运行时需要 support read 权限。
async fn admin_conversations(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminSupportConversationsQuery>,
) -> AppResult<Json<SupportConversationsResponse>> {
    Ok(Json(
        list_admin_support_conversations(state.mysql.clone(), query).await?,
    ))
}

/// 查询管理员全局范围内的单条会话。
async fn admin_conversation(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(conversation_id): Path<u64>,
) -> AppResult<Json<SupportConversationResponse>> {
    Ok(Json(
        get_admin_support_conversation(state.mysql.clone(), conversation_id).await?,
    ))
}

/// 分页查询管理员全局范围内的会话消息。
async fn admin_messages(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(conversation_id): Path<u64>,
    Query(query): Query<SupportMessagesQuery>,
) -> AppResult<Json<SupportMessagesResponse>> {
    Ok(Json(
        list_admin_support_messages(state.mysql.clone(), conversation_id, query).await?,
    ))
}

/// 以已鉴权管理员的 subject 回复任意会话。
async fn admin_send_message(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(conversation_id): Path<u64>,
    Json(request): Json<SendSupportMessageRequest>,
) -> AppResult<Json<SupportSendMessageResponse>> {
    Ok(Json(
        send_admin_support_message(
            state.mysql.clone(),
            &claims.sub,
            conversation_id,
            request,
            state.event_broadcast_hub.as_ref(),
        )
        .await?,
    ))
}

/// 推进管理员全局会话的 staff 已读游标。
async fn admin_mark_read(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(conversation_id): Path<u64>,
    Json(request): Json<MarkSupportReadRequest>,
) -> AppResult<Json<SupportConversationResponse>> {
    Ok(Json(
        mark_admin_support_read(state.mysql.clone(), conversation_id, request).await?,
    ))
}

/// 打开或关闭管理员全局范围内的会话。
async fn admin_update_status(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(conversation_id): Path<u64>,
    Json(request): Json<UpdateSupportStatusRequest>,
) -> AppResult<Json<SupportConversationResponse>> {
    Ok(Json(
        update_admin_support_status(state.mysql.clone(), conversation_id, request).await?,
    ))
}
