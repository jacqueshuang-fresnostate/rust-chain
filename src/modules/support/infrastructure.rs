//! support 限界上下文的 MySQL 适配器。
//!
//! 所有客服 SQL 都收敛在本文件：会话队列与历史读取、直属代理解析、
//! 会话行锁、发送幂等查找、不可变消息追加、双侧已读游标和状态更新。
//! 带 `_in_tx` 的入口一律复用调用方事务，不自行提交或回滚；应用层因此能保证
//! “消息插入 + 会话摘要”以及“用户改派 + 客服归属”在同一提交点生效。

use crate::{
    error::{AppError, AppResult},
    modules::support::{
        domain::{
            SupportActor, SupportConversationAccess, SupportConversationStatus, SupportMessagePage,
            ValidatedSupportMessage,
        },
        repository::{
            SupportConversationListFilter, SupportConversationLockRecord,
            SupportConversationRecord, SupportMessageRecord, SupportStaffScope,
        },
    },
};
use sqlx::{MySql, Pool, QueryBuilder, Transaction};

const SUPPORT_CONVERSATION_SELECT: &str = r#"
SELECT conversations.id,
       conversations.user_id,
       users.email AS user_email,
       users.phone AS user_phone,
       conversations.assigned_agent_id,
       agents.agent_code AS assigned_agent_code,
       conversations.status,
       conversations.user_read_message_id,
       conversations.staff_read_message_id,
       (
           SELECT COUNT(*)
           FROM support_messages user_unread
           WHERE user_unread.conversation_id = conversations.id
             AND user_unread.sender_type IN ('agent', 'admin')
             AND user_unread.id > COALESCE(conversations.user_read_message_id, 0)
       ) AS user_unread_count,
       (
           SELECT COUNT(*)
           FROM support_messages staff_unread
           WHERE staff_unread.conversation_id = conversations.id
             AND staff_unread.sender_type = 'user'
             AND staff_unread.id > COALESCE(conversations.staff_read_message_id, 0)
       ) AS staff_unread_count,
       conversations.last_message_id,
       conversations.last_message_sender_type,
       conversations.last_message_sender_id,
       conversations.last_message_preview,
       conversations.last_message_at,
       conversations.closed_at,
       conversations.created_at,
       conversations.updated_at
FROM support_conversations conversations
INNER JOIN users ON users.id = conversations.user_id
LEFT JOIN agents ON agents.id = conversations.assigned_agent_id
"#;

fn conversation_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::new(SUPPORT_CONVERSATION_SELECT)
}

fn push_conversation_filters(
    builder: &mut QueryBuilder<'_, MySql>,
    filter: &SupportConversationListFilter,
) {
    builder.push(" WHERE 1 = 1");
    match filter.scope {
        SupportStaffScope::Agent(agent_id) => {
            builder.push(" AND conversations.assigned_agent_id = ");
            builder.push_bind(agent_id);
        }
        SupportStaffScope::Admin => {}
    }
    if let Some(status) = filter.status.clone() {
        builder.push(" AND conversations.status = ");
        builder.push_bind(status);
    }
    if filter.unread_only {
        builder.push(
            r#" AND EXISTS (
                    SELECT 1
                    FROM support_messages unread_messages
                    WHERE unread_messages.conversation_id = conversations.id
                      AND unread_messages.sender_type = 'user'
                      AND unread_messages.id > COALESCE(conversations.staff_read_message_id, 0)
                )"#,
        );
    }
    if let SupportStaffScope::Admin = filter.scope {
        if let Some(agent_id) = filter.assigned_agent_id {
            builder.push(" AND conversations.assigned_agent_id = ");
            builder.push_bind(agent_id);
        }
        if filter.unassigned_only {
            builder.push(" AND conversations.assigned_agent_id IS NULL");
        }
    }
}

/// 按用户 ID 读取其唯一客服会话与双侧未读数。
/// 本查询不加锁、不创建空会话；首次发言前返回 `None` 是正常空态。
/// 归属是当前会话快照，需要与 referral 对齐时由应用层先执行同步事务。
pub(crate) async fn load_support_conversation_for_user(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Option<SupportConversationRecord>> {
    let mut query = conversation_query();
    query.push(" WHERE conversations.user_id = ");
    query.push_bind(user_id);
    query.push(" LIMIT 1");
    Ok(query
        .build_query_as::<SupportConversationRecord>()
        .fetch_optional(pool)
        .await?)
}

/// 按会话 ID 和令牌解析出的精确代理 ID 读取详情。
/// 查询只使用 `assigned_agent_id = agent_id`，不联表物化路径、不扩展到子代或父代；
/// 不属于该代理时返回 `None`，上层统一映射为 404 以避免泄露会话存在性。
pub(crate) async fn load_support_conversation_for_agent(
    pool: &Pool<MySql>,
    conversation_id: u64,
    agent_id: u64,
) -> AppResult<Option<SupportConversationRecord>> {
    let mut query = conversation_query();
    query.push(" WHERE conversations.id = ");
    query.push_bind(conversation_id);
    query.push(" AND conversations.assigned_agent_id = ");
    query.push_bind(agent_id);
    query.push(" LIMIT 1");
    Ok(query
        .build_query_as::<SupportConversationRecord>()
        .fetch_optional(pool)
        .await?)
}

/// 为已通过运行时 RBAC 的管理员按 ID 读取任意会话。
/// 范围包含未分配和已分配会话，本适配器不重复解析管理员权限；
/// 记录不存在返回 `None`，查询不加锁也不改变已读游标。
pub(crate) async fn load_support_conversation_for_admin(
    pool: &Pool<MySql>,
    conversation_id: u64,
) -> AppResult<Option<SupportConversationRecord>> {
    let mut query = conversation_query();
    query.push(" WHERE conversations.id = ");
    query.push_bind(conversation_id);
    query.push(" LIMIT 1");
    Ok(query
        .build_query_as::<SupportConversationRecord>()
        .fetch_optional(pool)
        .await?)
}

/// 按服务端范围与筛选分页读取客服队列，并用同一组谓词计算总数。
/// 代理队列始终精确匹配本代理；管理员可查全局、指定代理或仅未分配。
/// 结果按最后消息时间与 ID 倒序，limit/offset 已由领域层限制，本函数不接受无界页。
pub(crate) async fn list_support_conversations(
    pool: &Pool<MySql>,
    filter: SupportConversationListFilter,
) -> AppResult<(Vec<SupportConversationRecord>, i64)> {
    let mut rows = conversation_query();
    let mut total =
        QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM support_conversations conversations");
    push_conversation_filters(&mut rows, &filter);
    push_conversation_filters(&mut total, &filter);
    rows.push(" ORDER BY conversations.last_message_at DESC, conversations.id DESC LIMIT ");
    rows.push_bind(i64::from(filter.limit));
    rows.push(" OFFSET ");
    rows.push_bind(i64::from(filter.offset));

    let conversations = rows
        .build_query_as::<SupportConversationRecord>()
        .fetch_all(pool)
        .await?;
    let total = total.build_query_scalar::<i64>().fetch_one(pool).await?;
    Ok((conversations, total))
}

/// 按消息 ID 游标读取一页历史，数据库先倒序取 `limit + 1` 条探测后续，
/// 再在内存中恢复升序，使聊天界面从旧到新渲染。`next_before_id` 是当前页最旧一条 ID，
/// 下一页严格查询小于该值的消息，因此边界不重复；本查询不自动推进任何已读游标。
pub(crate) async fn list_support_messages(
    pool: &Pool<MySql>,
    conversation_id: u64,
    page: SupportMessagePage,
    access: SupportConversationAccess,
) -> AppResult<(Vec<SupportMessageRecord>, bool, Option<u64>)> {
    let mut query = QueryBuilder::<MySql>::new(
        r#"SELECT messages.id, messages.conversation_id, messages.sender_type, messages.sender_id,
                  messages.client_message_id, messages.body, messages.created_at
           FROM support_messages messages
           WHERE messages.conversation_id = "#,
    );
    query.push_bind(conversation_id);
    match access {
        SupportConversationAccess::User(user_id) => {
            query.push(
                r#" AND EXISTS (
                        SELECT 1
                        FROM support_conversations visible_conversation
                        WHERE visible_conversation.id = messages.conversation_id
                          AND visible_conversation.user_id = "#,
            );
            query.push_bind(user_id);
            query.push(")");
        }
        SupportConversationAccess::Agent(agent_id) => {
            query.push(
                r#" AND EXISTS (
                        SELECT 1
                        FROM support_conversations visible_conversation
                        WHERE visible_conversation.id = messages.conversation_id
                          AND visible_conversation.assigned_agent_id = "#,
            );
            query.push_bind(agent_id);
            query.push(")");
        }
        SupportConversationAccess::Admin => {}
    }
    if let Some(before_id) = page.before_id {
        query.push(" AND messages.id < ");
        query.push_bind(before_id);
    }
    query.push(" ORDER BY messages.id DESC LIMIT ");
    query.push_bind(i64::from(page.limit) + 1);
    let mut messages = query
        .build_query_as::<SupportMessageRecord>()
        .fetch_all(pool)
        .await?;
    let has_more = messages.len() > page.limit as usize;
    if has_more {
        messages.pop();
    }
    messages.reverse();
    let next_before_id = has_more
        .then(|| messages.first().map(|message| message.id))
        .flatten();
    Ok((messages, has_more, next_before_id))
}

/// 在调用方事务内锁定用户 referral，并解析当前可接待的直属代理。
/// 只读 `user_referrals.root_agent_id` 这个直接所有者兼容列，绝不用 `agents.root_agent_id` 或 path 扩展客服可见范围；
/// 代理自身或任一祖先非 active、路径未初始化、用户未分配时都返回 `None`，使会话保留管理员全局兜底。
pub(crate) async fn resolve_active_support_agent_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<Option<u64>> {
    let owner_agent_id = sqlx::query_scalar::<_, Option<u64>>(
        r#"SELECT root_agent_id
           FROM user_referrals
           WHERE user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .flatten();
    let Some(owner_agent_id) = owner_agent_id else {
        return Ok(None);
    };

    Ok(sqlx::query_scalar::<_, u64>(
        r#"SELECT owner_agents.id
           FROM agents owner_agents
           WHERE owner_agents.id = ?
             AND owner_agents.status = 'active'
             AND owner_agents.path <> ''
             AND NOT EXISTS (
                 SELECT 1
                 FROM agents ancestors
                 WHERE (ancestors.path = owner_agents.path
                    OR owner_agents.path LIKE CONCAT(ancestors.path, '/%'))
                   AND ancestors.status <> 'active'
             )
           LIMIT 1"#,
    )
    .bind(owner_agent_id)
    .fetch_optional(&mut **tx)
    .await?)
}

/// 在首条用户消息事务中确保会话存在，并返回用户唯一会话 ID。
/// 唯一键 `(user_id)` 吸收并发首发，`LAST_INSERT_ID(id)` 让新增与命中旧行共用同一返回路径；
/// 命中旧会话时不改归属、游标或状态，后续同步与发送更新由独立步骤完成。
pub(crate) async fn ensure_support_conversation_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    assigned_agent_id: Option<u64>,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO support_conversations (user_id, assigned_agent_id)
           VALUES (?, ?)
           ON DUPLICATE KEY UPDATE id = LAST_INSERT_ID(id)"#,
    )
    .bind(user_id)
    .bind(assigned_agent_id)
    .execute(&mut **tx)
    .await?;
    let conversation_id = result.last_insert_id();
    if conversation_id == 0 {
        return Err(AppError::Internal(
            "support conversation id was not returned".to_owned(),
        ));
    }
    Ok(conversation_id)
}

/// 在现有事务中把用户客服会话同步到服务端权威直属代理。
/// 本函数不创建空会话；仅在 null-safe 比较确认所有者真正变化时更新 `assigned_agent_id`，
/// 并把 staff 已读游标置空，防止新代理继承旧代理的阅读进度；消息与用户游标保持不变。
pub(crate) async fn sync_support_conversation_assignment_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    assigned_agent_id: Option<u64>,
) -> AppResult<bool> {
    let result = sqlx::query(
        r#"UPDATE support_conversations
           SET assigned_agent_id = ?, staff_read_message_id = NULL
           WHERE user_id = ?
             AND NOT (assigned_agent_id <=> ?)"#,
    )
    .bind(assigned_agent_id)
    .bind(user_id)
    .bind(assigned_agent_id)
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// 在管理员改派已完成 referral 子树迁移后，按新 path 与新根代理批量同步已有客服会话。
/// 查询同时约束根用户、后代 path 和服务端已写入的 `root_agent_id`，因此旧 path 下但属于其他根代理、
/// 或同一旧代理下不在该邀请子树的用户都不会被误迁；没有会话的 referral 不会触发隐式创建。
/// 仅所有者真实变化的行会被更新并清空 staff 已读游标，重复改派保持原游标；本入口复用调用方事务，
/// referral、全部受影响会话与管理员审计在同一提交点生效，任一步失败都会整体回滚。
pub(crate) async fn sync_support_conversation_subtree_assignments_in_tx(
    tx: &mut Transaction<'_, MySql>,
    root_user_id: u64,
    new_root_agent_id: u64,
    new_root_path: &str,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"UPDATE support_conversations conversations
           INNER JOIN user_referrals referrals ON referrals.user_id = conversations.user_id
           SET conversations.assigned_agent_id = referrals.root_agent_id,
               conversations.staff_read_message_id = NULL
           WHERE (referrals.user_id = ? OR referrals.path LIKE CONCAT(?, '/%'))
             AND referrals.root_agent_id = ?
             AND NOT (conversations.assigned_agent_id <=> referrals.root_agent_id)"#,
    )
    .bind(root_user_id)
    .bind(new_root_path)
    .bind(new_root_agent_id)
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected())
}

/// 按调用方身份锁定一条会话，并于同一条 `SELECT ... FOR UPDATE` 中执行所有权边界。
/// 用户必须匹配 user_id，代理必须精确匹配 assigned_agent_id，管理员的全局权限已由 `AdminAuth` 在路由前校验；
/// 未命中返回 `None` 而不区分不存在与他人所有，锁持有到调用方提交或回滚为止。
pub(crate) async fn lock_support_conversation_in_tx(
    tx: &mut Transaction<'_, MySql>,
    conversation_id: u64,
    access: SupportConversationAccess,
) -> AppResult<Option<SupportConversationLockRecord>> {
    let mut query = QueryBuilder::<MySql>::new(
        r#"SELECT id, user_id, assigned_agent_id
           FROM support_conversations
           WHERE id = "#,
    );
    query.push_bind(conversation_id);
    match access {
        SupportConversationAccess::User(user_id) => {
            query.push(" AND user_id = ");
            query.push_bind(user_id);
        }
        SupportConversationAccess::Agent(agent_id) => {
            query.push(" AND assigned_agent_id = ");
            query.push_bind(agent_id);
        }
        SupportConversationAccess::Admin => {}
    }
    query.push(" LIMIT 1 FOR UPDATE");
    Ok(query
        .build_query_as::<SupportConversationLockRecord>()
        .fetch_optional(&mut **tx)
        .await?)
}

/// 在当前会话与发送者命名空间中查找已提交的幂等消息。
/// 唯一范围是 `(conversation_id, sender_type, sender_id, client_message_id)`，因此两个管理员或代理可安全使用相同客户端键；
/// 只读不改状态，应用层必须再比较正文，以拒绝同键不同载荷。
pub(crate) async fn find_idempotent_support_message_in_tx(
    tx: &mut Transaction<'_, MySql>,
    conversation_id: u64,
    actor: SupportActor,
    client_message_id: &str,
) -> AppResult<Option<SupportMessageRecord>> {
    Ok(sqlx::query_as::<_, SupportMessageRecord>(
        r#"SELECT id, conversation_id, sender_type, sender_id,
                  client_message_id, body, created_at
           FROM support_messages
           WHERE conversation_id = ?
             AND sender_type = ?
             AND sender_id = ?
             AND client_message_id = ?
           LIMIT 1"#,
    )
    .bind(conversation_id)
    .bind(actor.sender_type())
    .bind(actor.sender_id())
    .bind(client_message_id)
    .fetch_optional(&mut **tx)
    .await?)
}

/// 向已锁定会话追加一条不可变消息，并回读数据库时间快照。
/// 输入必须先通过领域层长度与安全 token 校验；本入口不使用 upsert，不会覆盖旧消息，
/// 若调用方跳过幂等查找而命中唯一键，整个事务以数据库错误失败，不产生覆盖。
pub(crate) async fn insert_support_message_in_tx(
    tx: &mut Transaction<'_, MySql>,
    conversation_id: u64,
    actor: SupportActor,
    message: &ValidatedSupportMessage,
) -> AppResult<SupportMessageRecord> {
    let message_id = sqlx::query(
        r#"INSERT INTO support_messages
              (conversation_id, sender_type, sender_id, client_message_id, body)
           VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(conversation_id)
    .bind(actor.sender_type())
    .bind(actor.sender_id())
    .bind(&message.client_message_id)
    .bind(&message.body)
    .execute(&mut **tx)
    .await?
    .last_insert_id();
    sqlx::query_as::<_, SupportMessageRecord>(
        r#"SELECT id, conversation_id, sender_type, sender_id,
                  client_message_id, body, created_at
           FROM support_messages
           WHERE id = ? AND conversation_id = ?
           LIMIT 1"#,
    )
    .bind(message_id)
    .bind(conversation_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Internal("inserted support message was not found".to_owned()))
}

/// 在消息插入的同一事务内更新会话最后消息摘要，并重新打开已关闭会话。
/// 摘要只使用已验证消息的安全预览，同时保存消息 ID、发送类型、发送者和数据库提交时间；
/// 未更新到会话行视为内部一致性错误，由调用方回滚刚插入的消息，绝不允许孤立消息提交。
pub(crate) async fn update_support_conversation_after_message_in_tx(
    tx: &mut Transaction<'_, MySql>,
    conversation_id: u64,
    actor: SupportActor,
    message: &SupportMessageRecord,
    preview: &str,
) -> AppResult<()> {
    let result = sqlx::query(
        r#"UPDATE support_conversations
           SET status = 'open',
               closed_at = NULL,
               last_message_id = ?,
               last_message_sender_type = ?,
               last_message_sender_id = ?,
               last_message_preview = ?,
               last_message_at = ?
           WHERE id = ?"#,
    )
    .bind(message.id)
    .bind(actor.sender_type())
    .bind(actor.sender_id())
    .bind(preview)
    .bind(message.created_at)
    .bind(conversation_id)
    .execute(&mut **tx)
    .await?;
    if result.rows_affected() != 1 {
        return Err(AppError::Internal(
            "support conversation metadata was not updated".to_owned(),
        ));
    }
    Ok(())
}

/// 在已锁定会话中单调推进用户或客服已读游标。
/// 目标消息必须真实属于该会话，否则返回 404 且不更新；`GREATEST` 保证延迟到达的旧回执不会倒退游标。
/// 用户身份只能更新 user 游标，代理或管理员只能更新 staff 游标；本函数不更改会话状态。
pub(crate) async fn advance_support_read_cursor_in_tx(
    tx: &mut Transaction<'_, MySql>,
    conversation_id: u64,
    message_id: u64,
    user_side: bool,
) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, u64>(
        "SELECT id FROM support_messages WHERE conversation_id = ? AND id = ? LIMIT 1",
    )
    .bind(conversation_id)
    .bind(message_id)
    .fetch_optional(&mut **tx)
    .await?
    .is_some();
    if !exists {
        return Err(AppError::NotFound);
    }

    let sql = if user_side {
        r#"UPDATE support_conversations
           SET user_read_message_id = GREATEST(COALESCE(user_read_message_id, 0), ?)
           WHERE id = ?"#
    } else {
        r#"UPDATE support_conversations
           SET staff_read_message_id = GREATEST(COALESCE(staff_read_message_id, 0), ?)
           WHERE id = ?"#
    };
    sqlx::query(sql)
        .bind(message_id)
        .bind(conversation_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 在已锁定会话上应用 open/closed 状态，关闭时记录当前数据库时间，重开时清空。
/// 重复提交同一状态是幂等的最终值写入；已读游标、归属与消息摘要不受影响。
/// 若会话行在锁定后异常消失，返回内部一致性错误并由调用方回滚。
pub(crate) async fn update_support_conversation_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    conversation_id: u64,
    status: SupportConversationStatus,
) -> AppResult<()> {
    let result =
        match status {
            SupportConversationStatus::Open => sqlx::query(
                "UPDATE support_conversations SET status = 'open', closed_at = NULL WHERE id = ?",
            )
            .bind(conversation_id)
            .execute(&mut **tx)
            .await?,
            SupportConversationStatus::Closed => {
                sqlx::query(
                    r#"UPDATE support_conversations
                   SET status = 'closed', closed_at = CURRENT_TIMESTAMP(6)
                   WHERE id = ?"#,
                )
                .bind(conversation_id)
                .execute(&mut **tx)
                .await?
            }
        };
    if result.rows_affected() != 1 {
        return Err(AppError::Internal(
            "support conversation status was not updated".to_owned(),
        ));
    }
    Ok(())
}
