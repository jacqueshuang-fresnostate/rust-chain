//! support 限界上下文的用例编排与事务边界。
//!
//! 本层将用户、代理、管理员三类身份收敛为服务端主体，组织会话归属同步、
//! 精确所有权行锁、发送幂等、消息与摘要原子提交、已读游标和状态迁移。
//! 客服消息始终先提交 MySQL，再向用户与当时的精确直属代理发送进程内刷新提示；
//! 广播不落库、不重试，客户端任何时候都必须以 REST 重新对齐为权威路径。

use crate::{
    error::{AppError, AppResult},
    modules::{
        admin::service::admin_id_from_subject,
        agent::application::resolve_active_agent_id,
        events::{EventBroadcastHub, EventBroadcastMessage},
        support::{
            domain::{
                SupportActor, SupportConversationAccess, SupportConversationStatus,
                ValidatedSupportMessage, optional_support_status, support_message_page,
                support_offset_page, validate_support_message,
            },
            infrastructure::{
                advance_support_read_cursor_in_tx, ensure_support_conversation_in_tx,
                find_idempotent_support_message_in_tx, insert_support_message_in_tx,
                list_support_conversations as list_support_conversations_from_store,
                list_support_messages as list_support_messages_from_store,
                load_support_conversation_for_admin, load_support_conversation_for_agent,
                load_support_conversation_for_user, lock_support_conversation_in_tx,
                resolve_active_support_agent_in_tx, sync_support_conversation_assignment_in_tx,
                sync_support_conversation_subtree_assignments_in_tx,
                update_support_conversation_after_message_in_tx,
                update_support_conversation_status_in_tx,
            },
            presentation::{
                AdminSupportConversationsQuery, AgentSupportConversationsQuery,
                MarkSupportReadRequest, SendSupportMessageRequest, SupportConversationResponse,
                SupportConversationsResponse, SupportMessagesQuery, SupportMessagesResponse,
                SupportSendMessageResponse, UpdateSupportStatusRequest,
                UserSupportConversationResponse,
            },
            repository::{
                SupportConversationListFilter, SupportConversationLockRecord,
                SupportConversationRecord, SupportMessageRecord, SupportStaffScope,
            },
        },
        user::service::user_id_from_subject,
    },
};
use serde_json::json;
use sqlx::{MySql, Pool, Transaction};

struct SupportSendOutcome {
    message: SupportMessageRecord,
    replayed: bool,
    user_id: u64,
    assigned_agent_id: Option<u64>,
}

fn support_mysql_pool(mysql: Option<Pool<MySql>>) -> AppResult<Pool<MySql>> {
    mysql.ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for support routes".to_owned())
    })
}

async fn synchronize_existing_user_conversation(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Option<SupportConversationRecord>> {
    if load_support_conversation_for_user(pool, user_id)
        .await?
        .is_none()
    {
        return Ok(None);
    }
    let mut tx = pool.begin().await?;
    let assigned_agent_id = resolve_active_support_agent_in_tx(&mut tx, user_id).await?;
    sync_support_conversation_assignment_in_tx(&mut tx, user_id, assigned_agent_id).await?;
    tx.commit().await?;
    load_support_conversation_for_user(pool, user_id).await
}

fn messages_response(
    conversation: &SupportConversationRecord,
    messages: Vec<SupportMessageRecord>,
    has_more: bool,
    next_before_id: Option<u64>,
) -> SupportMessagesResponse {
    SupportMessagesResponse {
        messages: messages
            .into_iter()
            .map(|message| {
                crate::modules::support::presentation::SupportMessageResponse::from_record(
                    message,
                    conversation.user_read_message_id,
                    conversation.staff_read_message_id,
                )
            })
            .collect(),
        has_more,
        next_before_id,
    }
}

async fn append_message_in_tx(
    tx: &mut Transaction<'_, MySql>,
    conversation: &SupportConversationLockRecord,
    actor: SupportActor,
    input: &ValidatedSupportMessage,
) -> AppResult<SupportSendOutcome> {
    if let Some(existing) =
        find_idempotent_support_message_in_tx(tx, conversation.id, actor, &input.client_message_id)
            .await?
    {
        if existing.body != input.body {
            return Err(AppError::Conflict(
                "client_message_id was already used with a different body".to_owned(),
            ));
        }
        return Ok(SupportSendOutcome {
            message: existing,
            replayed: true,
            user_id: conversation.user_id,
            assigned_agent_id: conversation.assigned_agent_id,
        });
    }

    let message = insert_support_message_in_tx(tx, conversation.id, actor, input).await?;
    update_support_conversation_after_message_in_tx(
        tx,
        conversation.id,
        actor,
        &message,
        &input.preview,
    )
    .await?;
    Ok(SupportSendOutcome {
        message,
        replayed: false,
        user_id: conversation.user_id,
        assigned_agent_id: conversation.assigned_agent_id,
    })
}

fn publish_committed_message_refresh(
    hub: Option<&EventBroadcastHub>,
    outcome: &SupportSendOutcome,
) {
    if outcome.replayed {
        return;
    }
    let Some(hub) = hub else {
        return;
    };
    let payload = json!({
        "type": "support.refresh",
        "reason": "message_committed",
        "conversation_id": outcome.message.conversation_id,
        "message_id": outcome.message.id,
    })
    .to_string();
    hub.publish(EventBroadcastMessage::private_user(
        outcome.user_id,
        payload.clone(),
    ));
    if let Some(agent_id) = outcome.assigned_agent_id {
        hub.publish(EventBroadcastMessage::private_agent(agent_id, payload));
    }
}

async fn load_staff_messages(
    pool: &Pool<MySql>,
    conversation: SupportConversationRecord,
    query: SupportMessagesQuery,
    access: SupportConversationAccess,
) -> AppResult<SupportMessagesResponse> {
    let page = support_message_page(query.limit, query.before_id)?;
    let (messages, has_more, next_before_id) =
        list_support_messages_from_store(pool, conversation.id, page, access).await?;
    Ok(messages_response(
        &conversation,
        messages,
        has_more,
        next_before_id,
    ))
}

async fn send_existing_conversation_message(
    pool: &Pool<MySql>,
    conversation_id: u64,
    access: SupportConversationAccess,
    actor: SupportActor,
    input: ValidatedSupportMessage,
    hub: Option<&EventBroadcastHub>,
) -> AppResult<SupportSendOutcome> {
    let mut tx = pool.begin().await?;
    let conversation = lock_support_conversation_in_tx(&mut tx, conversation_id, access)
        .await?
        .ok_or(AppError::NotFound)?;
    let outcome = append_message_in_tx(&mut tx, &conversation, actor, &input).await?;
    tx.commit().await?;
    publish_committed_message_refresh(hub, &outcome);
    Ok(outcome)
}

/// 返回当前用户的唯一客服会话，首条消息之前以 `conversation: null` 表达正常空态。
/// 已存在会话会先锁定 referral，仅从 `user_referrals.root_agent_id` 及 active 祖先链重新同步直属代理；
/// 归属改变时与同步更新同事务清空 staff 已读游标，查询本身不创建会话。
pub(crate) async fn get_user_support_conversation(
    mysql: Option<Pool<MySql>>,
    subject: &str,
) -> AppResult<UserSupportConversationResponse> {
    let user_id = user_id_from_subject(subject)?;
    let pool = support_mysql_pool(mysql)?;
    Ok(UserSupportConversationResponse {
        conversation: synchronize_existing_user_conversation(&pool, user_id)
            .await?
            .map(Into::into),
    })
}

/// 分页返回当前用户会话的历史消息，首次使用时返回稳定空页而非 404。
/// 读取前与会话查询一样同步服务端直属代理，但不因读取历史自动推进用户游标；
/// 页内数组按 ID 升序，页大小最多一百，客户端必须显式调用 read 端点确认已读。
pub(crate) async fn list_user_support_messages(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    query: SupportMessagesQuery,
) -> AppResult<SupportMessagesResponse> {
    let user_id = user_id_from_subject(subject)?;
    let pool = support_mysql_pool(mysql)?;
    let Some(conversation) = synchronize_existing_user_conversation(&pool, user_id).await? else {
        return Ok(SupportMessagesResponse::empty());
    };
    load_staff_messages(
        &pool,
        conversation,
        query,
        SupportConversationAccess::User(user_id),
    )
    .await
}

/// 为当前用户发送文本：锁定 referral，解析 active 直属代理，并发创建唯一会话后锁行追加消息。
/// 同会话、用户身份与客户端键的重试返回原消息且 `replayed=true`，同键不同正文返回冲突；
/// 新消息与会话摘要同事务提交并自动重开 closed 会话，仅首次提交成功后才尽力广播刷新提示。
pub(crate) async fn send_user_support_message(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    request: SendSupportMessageRequest,
    hub: Option<&EventBroadcastHub>,
) -> AppResult<SupportSendMessageResponse> {
    let user_id = user_id_from_subject(subject)?;
    let input = validate_support_message(request.body, request.client_message_id)?;
    let pool = support_mysql_pool(mysql)?;
    let mut tx = pool.begin().await?;
    let assigned_agent_id = resolve_active_support_agent_in_tx(&mut tx, user_id).await?;
    let conversation_id =
        ensure_support_conversation_in_tx(&mut tx, user_id, assigned_agent_id).await?;
    sync_support_conversation_assignment_in_tx(&mut tx, user_id, assigned_agent_id).await?;
    let conversation = lock_support_conversation_in_tx(
        &mut tx,
        conversation_id,
        SupportConversationAccess::User(user_id),
    )
    .await?
    .ok_or(AppError::NotFound)?;
    let outcome =
        append_message_in_tx(&mut tx, &conversation, SupportActor::User(user_id), &input).await?;
    tx.commit().await?;
    publish_committed_message_refresh(hub, &outcome);

    let conversation = load_support_conversation_for_user(&pool, user_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let message = crate::modules::support::presentation::SupportMessageResponse::from_record(
        outcome.message,
        conversation.user_read_message_id,
        conversation.staff_read_message_id,
    );
    Ok(SupportSendMessageResponse {
        conversation: conversation.into(),
        message,
        replayed: outcome.replayed,
    })
}

/// 把当前用户的已读游标单调推进到指定会话消息，目标不属于该会话时返回 404。
/// 操作前在同一事务内重新同步直属代理，再以 user_id 所有权锁定会话；
/// 延迟的旧 message_id 不会倒退游标，读回的未读数来自提交后权威数据。
pub(crate) async fn mark_user_support_read(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    request: MarkSupportReadRequest,
) -> AppResult<SupportConversationResponse> {
    if request.message_id == 0 {
        return Err(AppError::Validation(
            "message_id must be greater than zero".to_owned(),
        ));
    }
    let user_id = user_id_from_subject(subject)?;
    let pool = support_mysql_pool(mysql)?;
    let conversation_id = load_support_conversation_for_user(&pool, user_id)
        .await?
        .map(|conversation| conversation.id)
        .ok_or(AppError::NotFound)?;
    let mut tx = pool.begin().await?;
    let assigned_agent_id = resolve_active_support_agent_in_tx(&mut tx, user_id).await?;
    sync_support_conversation_assignment_in_tx(&mut tx, user_id, assigned_agent_id).await?;
    lock_support_conversation_in_tx(
        &mut tx,
        conversation_id,
        SupportConversationAccess::User(user_id),
    )
    .await?
    .ok_or(AppError::NotFound)?;
    advance_support_read_cursor_in_tx(&mut tx, conversation_id, request.message_id, true).await?;
    tx.commit().await?;
    load_support_conversation_for_user(&pool, user_id)
        .await?
        .map(Into::into)
        .ok_or(AppError::NotFound)
}

/// 由当前用户显式关闭或重开其唯一会话，未创建会话时返回 404。
/// 状态先经领域白名单校验，事务内先对齐 referral 归属再以 user_id 锁行；
/// 关闭不删除历史，重开清空 closed_at，后续用户新消息也会自动重开。
pub(crate) async fn update_user_support_status(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    request: UpdateSupportStatusRequest,
) -> AppResult<SupportConversationResponse> {
    let status = SupportConversationStatus::parse(&request.status)?;
    let user_id = user_id_from_subject(subject)?;
    let pool = support_mysql_pool(mysql)?;
    let conversation_id = load_support_conversation_for_user(&pool, user_id)
        .await?
        .map(|conversation| conversation.id)
        .ok_or(AppError::NotFound)?;
    let mut tx = pool.begin().await?;
    let assigned_agent_id = resolve_active_support_agent_in_tx(&mut tx, user_id).await?;
    sync_support_conversation_assignment_in_tx(&mut tx, user_id, assigned_agent_id).await?;
    lock_support_conversation_in_tx(
        &mut tx,
        conversation_id,
        SupportConversationAccess::User(user_id),
    )
    .await?
    .ok_or(AppError::NotFound)?;
    update_support_conversation_status_in_tx(&mut tx, conversation_id, status).await?;
    tx.commit().await?;
    load_support_conversation_for_user(&pool, user_id)
        .await?
        .map(Into::into)
        .ok_or(AppError::NotFound)
}

/// 按令牌解析出的 active 代理 ID 分页返回精确归属队列，不采用子树物化路径。
/// status 只允许 open/closed，unread_only 只计算用户消息超过 staff 游标的会话；
/// 页大小最多一百、offset 最多十万，父代、兄弟与子代代理的会话都不会混入。
pub(crate) async fn list_agent_support_conversations(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    query: AgentSupportConversationsQuery,
) -> AppResult<SupportConversationsResponse> {
    let pool = support_mysql_pool(mysql)?;
    let agent_id = resolve_active_agent_id(Some(pool.clone()), subject).await?;
    let page = support_offset_page(query.limit, query.offset);
    let (conversations, total) = list_support_conversations_from_store(
        &pool,
        SupportConversationListFilter {
            scope: SupportStaffScope::Agent(agent_id),
            status: optional_support_status(query.status)?,
            unread_only: query.unread_only.unwrap_or(false),
            assigned_agent_id: None,
            unassigned_only: false,
            limit: page.limit,
            offset: page.offset,
        },
    )
    .await?;
    Ok(SupportConversationsResponse {
        conversations: conversations.into_iter().map(Into::into).collect(),
        total,
    })
}

/// 返回指定会话的代理详情，身份只取自 agent token 并回查 active 祖先链。
/// 数据库谓词严格要求 `assigned_agent_id` 等于当前代理，父代即使能查团队报表也无客服权限；
/// 会话不存在或归属他人均返回 404，不暴露越权目标是否存在。
pub(crate) async fn get_agent_support_conversation(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    conversation_id: u64,
) -> AppResult<SupportConversationResponse> {
    let pool = support_mysql_pool(mysql)?;
    let agent_id = resolve_active_agent_id(Some(pool.clone()), subject).await?;
    load_support_conversation_for_agent(&pool, conversation_id, agent_id)
        .await?
        .map(Into::into)
        .ok_or(AppError::NotFound)
}

/// 为当前精确直属代理读取指定会话的游标消息页。
/// 详情查询与后续消息查询都会精确匹配 assigned_agent_id，因此改派竞态或猜测 ID 都不能绕过代理边界；
/// 读取不推进 staff 游标，前端只能在真正展示后调用 read 端点。
pub(crate) async fn list_agent_support_messages(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    conversation_id: u64,
    query: SupportMessagesQuery,
) -> AppResult<SupportMessagesResponse> {
    let pool = support_mysql_pool(mysql)?;
    let agent_id = resolve_active_agent_id(Some(pool.clone()), subject).await?;
    let conversation = load_support_conversation_for_agent(&pool, conversation_id, agent_id)
        .await?
        .ok_or(AppError::NotFound)?;
    load_staff_messages(
        &pool,
        conversation,
        query,
        SupportConversationAccess::Agent(agent_id),
    )
    .await
}

/// 以令牌对应的精确 agent_id 向所属会话发送回复，请求不接受任何代理归属字段。
/// 会话行在幂等查找与追加期间持有精确所有权锁，改派与发送不会交叉越权；
/// 新回复会重开 closed 会话，提交后向用户和该精确代理广播不含正文的 REST 刷新提示。
pub(crate) async fn send_agent_support_message(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    conversation_id: u64,
    request: SendSupportMessageRequest,
    hub: Option<&EventBroadcastHub>,
) -> AppResult<SupportSendMessageResponse> {
    let input = validate_support_message(request.body, request.client_message_id)?;
    let pool = support_mysql_pool(mysql)?;
    let agent_id = resolve_active_agent_id(Some(pool.clone()), subject).await?;
    let outcome = send_existing_conversation_message(
        &pool,
        conversation_id,
        SupportConversationAccess::Agent(agent_id),
        SupportActor::Agent(agent_id),
        input,
        hub,
    )
    .await?;
    let conversation = load_support_conversation_for_agent(&pool, conversation_id, agent_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let message = crate::modules::support::presentation::SupportMessageResponse::from_record(
        outcome.message,
        conversation.user_read_message_id,
        conversation.staff_read_message_id,
    );
    Ok(SupportSendMessageResponse {
        conversation: conversation.into(),
        message,
        replayed: outcome.replayed,
    })
}

/// 单调推进当前精确代理所属会话的 staff 已读游标，不接受子树权限。
/// 会话与目标消息在同一事务内验证，会话归属不匹配或消息属于其他会话均返回 404；
/// 改派事务会在新所有者生效时清空该游标，因此新代理必须自行重新阅读。
pub(crate) async fn mark_agent_support_read(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    conversation_id: u64,
    request: MarkSupportReadRequest,
) -> AppResult<SupportConversationResponse> {
    if request.message_id == 0 {
        return Err(AppError::Validation(
            "message_id must be greater than zero".to_owned(),
        ));
    }
    let pool = support_mysql_pool(mysql)?;
    let agent_id = resolve_active_agent_id(Some(pool.clone()), subject).await?;
    let mut tx = pool.begin().await?;
    lock_support_conversation_in_tx(
        &mut tx,
        conversation_id,
        SupportConversationAccess::Agent(agent_id),
    )
    .await?
    .ok_or(AppError::NotFound)?;
    advance_support_read_cursor_in_tx(&mut tx, conversation_id, request.message_id, false).await?;
    tx.commit().await?;
    load_support_conversation_for_agent(&pool, conversation_id, agent_id)
        .await?
        .map(Into::into)
        .ok_or(AppError::NotFound)
}

/// 关闭或重开当前精确代理所属的会话，猜测他人会话 ID 不会泄露任何状态。
/// 状态字面量在开事务前校验，所有权在 `FOR UPDATE` 谓词中校验，避免先查后改的改派窗口；
/// 状态迁移不删消消息与游标，后续任一成功新回复会再次打开会话。
pub(crate) async fn update_agent_support_status(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    conversation_id: u64,
    request: UpdateSupportStatusRequest,
) -> AppResult<SupportConversationResponse> {
    let status = SupportConversationStatus::parse(&request.status)?;
    let pool = support_mysql_pool(mysql)?;
    let agent_id = resolve_active_agent_id(Some(pool.clone()), subject).await?;
    let mut tx = pool.begin().await?;
    lock_support_conversation_in_tx(
        &mut tx,
        conversation_id,
        SupportConversationAccess::Agent(agent_id),
    )
    .await?
    .ok_or(AppError::NotFound)?;
    update_support_conversation_status_in_tx(&mut tx, conversation_id, status).await?;
    tx.commit().await?;
    load_support_conversation_for_agent(&pool, conversation_id, agent_id)
        .await?
        .map(Into::into)
        .ok_or(AppError::NotFound)
}

/// 为管理员分页返回全局客服队列，包含 `assigned_agent_id IS NULL` 的未分配用户。
/// 可按 open/closed、staff 未读、某精确代理或未分配筛选；指定代理与未分配同时传入会在查库前拒绝。
/// 本用例依赖 `AdminAuth` 已按 HTTP 方法实时校验 `support.conversations.read`，应用层不使用客户端身份扩大范围。
pub(crate) async fn list_admin_support_conversations(
    mysql: Option<Pool<MySql>>,
    query: AdminSupportConversationsQuery,
) -> AppResult<SupportConversationsResponse> {
    let unassigned_only = query.unassigned.unwrap_or(false);
    if unassigned_only && query.assigned_agent_id.is_some() {
        return Err(AppError::Validation(
            "assigned_agent_id and unassigned=true cannot be combined".to_owned(),
        ));
    }
    if query.assigned_agent_id == Some(0) {
        return Err(AppError::Validation(
            "assigned_agent_id must be greater than zero".to_owned(),
        ));
    }
    let pool = support_mysql_pool(mysql)?;
    let page = support_offset_page(query.limit, query.offset);
    let (conversations, total) = list_support_conversations_from_store(
        &pool,
        SupportConversationListFilter {
            scope: SupportStaffScope::Admin,
            status: optional_support_status(query.status)?,
            unread_only: query.unread_only.unwrap_or(false),
            assigned_agent_id: query.assigned_agent_id,
            unassigned_only,
            limit: page.limit,
            offset: page.offset,
        },
    )
    .await?;
    Ok(SupportConversationsResponse {
        conversations: conversations.into_iter().map(Into::into).collect(),
        total,
    })
}

/// 为已获得 `support.conversations.read` 的管理员返回任意会话，包含未分配会话。
/// 本查询不要求 assigned_agent_id，因此当直属代理停用、离线或用户从未分配时仍可由平台兜底；
/// 会话不存在返回 404，读取不改变 staff 已读游标。
pub(crate) async fn get_admin_support_conversation(
    mysql: Option<Pool<MySql>>,
    conversation_id: u64,
) -> AppResult<SupportConversationResponse> {
    let pool = support_mysql_pool(mysql)?;
    load_support_conversation_for_admin(&pool, conversation_id)
        .await?
        .map(Into::into)
        .ok_or(AppError::NotFound)
}

/// 为管理员读取任意会话的游标历史页，未分配状态不影响可用性。
/// 先确认会话存在再执行管理员全局范围的有界消息查询，返回的已读标记由当前双侧游标推导；
/// 端点只读，必须由客户端在真正展示后单独提交 read 请求。
pub(crate) async fn list_admin_support_messages(
    mysql: Option<Pool<MySql>>,
    conversation_id: u64,
    query: SupportMessagesQuery,
) -> AppResult<SupportMessagesResponse> {
    let pool = support_mysql_pool(mysql)?;
    let conversation = load_support_conversation_for_admin(&pool, conversation_id)
        .await?
        .ok_or(AppError::NotFound)?;
    load_staff_messages(&pool, conversation, query, SupportConversationAccess::Admin).await
}

/// 以令牌 subject 中的管理员 ID 回复任意会话，发送者记录为 `admin` 而不冒充代理。
/// 未分配用户与已分配用户走同一不可变消息模型；幂等键在管理员身份内独立，
/// 首次提交后向用户与当时已分配的精确代理发刷新提示，未分配时只提示用户。
pub(crate) async fn send_admin_support_message(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    conversation_id: u64,
    request: SendSupportMessageRequest,
    hub: Option<&EventBroadcastHub>,
) -> AppResult<SupportSendMessageResponse> {
    let admin_id = admin_id_from_subject(subject)?;
    let input = validate_support_message(request.body, request.client_message_id)?;
    let pool = support_mysql_pool(mysql)?;
    let outcome = send_existing_conversation_message(
        &pool,
        conversation_id,
        SupportConversationAccess::Admin,
        SupportActor::Admin(admin_id),
        input,
        hub,
    )
    .await?;
    let conversation = load_support_conversation_for_admin(&pool, conversation_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let message = crate::modules::support::presentation::SupportMessageResponse::from_record(
        outcome.message,
        conversation.user_read_message_id,
        conversation.staff_read_message_id,
    );
    Ok(SupportSendMessageResponse {
        conversation: conversation.into(),
        message,
        replayed: outcome.replayed,
    })
}

/// 以管理员全局范围单调推进指定会话的 staff 已读游标，未分配会话同样可用。
/// 该游标与代理端共用“客服侧”语义，便于平台介入后不重复显示用户未读；
/// 若后续改派到新代理，改派事务会清空该游标，新所有者不继承管理员或旧代理的进度。
pub(crate) async fn mark_admin_support_read(
    mysql: Option<Pool<MySql>>,
    conversation_id: u64,
    request: MarkSupportReadRequest,
) -> AppResult<SupportConversationResponse> {
    if request.message_id == 0 {
        return Err(AppError::Validation(
            "message_id must be greater than zero".to_owned(),
        ));
    }
    let pool = support_mysql_pool(mysql)?;
    let mut tx = pool.begin().await?;
    lock_support_conversation_in_tx(&mut tx, conversation_id, SupportConversationAccess::Admin)
        .await?
        .ok_or(AppError::NotFound)?;
    advance_support_read_cursor_in_tx(&mut tx, conversation_id, request.message_id, false).await?;
    tx.commit().await?;
    load_support_conversation_for_admin(&pool, conversation_id)
        .await?
        .map(Into::into)
        .ok_or(AppError::NotFound)
}

/// 以管理员全局范围关闭或重开指定会话，不要求会话已分配代理。
/// 状态校验与行锁更新共用用户/代理端的同一规则，只改 status 与 closed_at；
/// 历史消息、归属、双侧游标和最后消息摘要全部保留，以便任一方重开后恢复。
pub(crate) async fn update_admin_support_status(
    mysql: Option<Pool<MySql>>,
    conversation_id: u64,
    request: UpdateSupportStatusRequest,
) -> AppResult<SupportConversationResponse> {
    let status = SupportConversationStatus::parse(&request.status)?;
    let pool = support_mysql_pool(mysql)?;
    let mut tx = pool.begin().await?;
    lock_support_conversation_in_tx(&mut tx, conversation_id, SupportConversationAccess::Admin)
        .await?
        .ok_or(AppError::NotFound)?;
    update_support_conversation_status_in_tx(&mut tx, conversation_id, status).await?;
    tx.commit().await?;
    load_support_conversation_for_admin(&pool, conversation_id)
        .await?
        .map(Into::into)
        .ok_or(AppError::NotFound)
}

/// 在已有管理员用户改派事务中同步根用户及已迁移邀请子树的客服所有者，不开启或提交独立事务。
/// 调用方必须先完成 referral 批量迁移并验证新代理层级 active；查询用新根 path 与根代理双重限定，
/// 只更新已有且所有者确实变化的会话并清空其 staff 游标，子树外会话和重复改派游标保持不变。
/// 任一后续审计或 referral 写入失败都会让整棵子树的客服同步一并回滚。
pub(crate) async fn synchronize_conversation_subtree_assignments_in_tx(
    tx: &mut Transaction<'_, MySql>,
    root_user_id: u64,
    new_root_agent_id: u64,
    new_root_path: &str,
) -> AppResult<u64> {
    sync_support_conversation_subtree_assignments_in_tx(
        tx,
        root_user_id,
        new_root_agent_id,
        new_root_path,
    )
    .await
}
