//! agent bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。

use crate::{
    error::{AppError, AppResult},
    modules::{
        agent::{
            infrastructure,
            presentation::{
                AgentCommissionsResponse, AgentConvertStatsResponse, AgentDashboardResponse,
                AgentInviteCodeResponse, AgentInviteCodesResponse, AgentListQuery, AgentMeResponse,
                AgentPasswordChangeResponse, AgentSubAgentsResponse, AgentTeamTreeResponse,
                AgentUsersResponse, ChangeAgentPasswordRequest, CreateInviteCodeRequest,
                UpdateInviteCodeStatusRequest,
            },
            repository::{AgentAccessScope, AgentInviteCodeWrite},
            service::{
                agent_admin_id_from_subject, agent_commissions_response,
                agent_convert_stats_response, agent_dashboard_response, agent_list_page,
                generated_agent_invite_code, validate_agent_invite_code_status,
                validate_agent_invite_code_usage_limit, validate_agent_password_change,
            },
        },
        auth::{ActorType, AuthActor, hash_password, revoke_actor_auth_sessions, verify_password},
    },
    state::AppState,
};
use sqlx::{MySql, Pool};

/// 按认证主体读取代理管理员及其代理节点，任一未启用祖先都按未授权处理。
pub(crate) async fn get_agent_me(
    mysql: Option<Pool<MySql>>,
    subject: &str,
) -> AppResult<AgentMeResponse> {
    let agent_admin_id = agent_admin_id_from_subject(subject)?;
    let pool = agent_mysql_pool(mysql)?;

    infrastructure::load_agent_me(&pool, agent_admin_id)
        .await?
        .ok_or(AppError::Unauthorized)
}

/// 在当前代理子树内汇总团队用户、有效邀请码和分资产佣金。
/// scope、计数和资产汇总分别查询且不开事务，并发归属/结算时各块数据可能来自不同快照。
pub(crate) async fn get_agent_dashboard(
    mysql: Option<Pool<MySql>>,
    subject: &str,
) -> AppResult<AgentDashboardResponse> {
    let (pool, scope) = agent_context(mysql, subject).await?;
    let counts = infrastructure::load_agent_dashboard_counts(&pool, &scope).await?;
    let assets = infrastructure::load_agent_dashboard_asset_summaries(&pool, &scope).await?;
    Ok(agent_dashboard_response(scope.agent_id, counts, assets))
}

/// 先验证代理账号及祖先状态，再按物化路径统计整个子树的兑换订单数与金额。
/// scope 与聚合是两个无事务查询；两者之间发生代理停用时，本次调用不会再次校验权限快照。
pub(crate) async fn get_agent_convert_stats(
    mysql: Option<Pool<MySql>>,
    subject: &str,
) -> AppResult<AgentConvertStatsResponse> {
    let (pool, scope) = agent_context(mysql, subject).await?;
    let row = infrastructure::load_agent_convert_stats(&pool, &scope).await?;
    agent_convert_stats_response(row)
}

/// 分页列出归属当前代理或后代节点的用户，页大小限制为一百且不泄露父级或兄弟树。
pub(crate) async fn list_agent_users(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    query: AgentListQuery,
) -> AppResult<AgentUsersResponse> {
    let (pool, scope) = agent_context(mysql, subject).await?;
    let page = agent_list_page(query.limit, query.offset, 100);
    let users = infrastructure::list_agent_team_users(&pool, &scope, page).await?;
    Ok(AgentUsersResponse { users })
}

/// 分页列出当前节点之下的代理，排除自身并以物化路径限定最大五百条。
pub(crate) async fn list_agent_sub_agents(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    query: AgentListQuery,
) -> AppResult<AgentSubAgentsResponse> {
    let (pool, scope) = agent_context(mysql, subject).await?;
    let page = agent_list_page(query.limit, query.offset, 500);
    let agents = infrastructure::list_agent_sub_agents(&pool, &scope, page).await?;
    Ok(AgentSubAgentsResponse { agents })
}

/// 先读取服务端代理 scope，再分别读取子代理和用户邀请节点，组装团队树而不修改归属。
/// 三次查询不开事务，不保证同一数据库快照；任一失败则整体失败，根 ID 不接受客户端覆盖。
pub(crate) async fn list_agent_team_tree(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    query: AgentListQuery,
) -> AppResult<AgentTeamTreeResponse> {
    let (pool, scope) = agent_context(mysql, subject).await?;
    let page = agent_list_page(query.limit, query.offset, 500);
    let agents = infrastructure::list_agent_sub_agents(&pool, &scope, page).await?;
    let nodes = infrastructure::list_agent_team_tree_nodes(&pool, &scope, page).await?;

    Ok(AgentTeamTreeResponse {
        root_agent_id: scope.root_agent_id,
        agents,
        nodes,
    })
}

/// 仅列出当前代理拥有的佣金记录，同时用子树归属约束业务用户范围。
pub(crate) async fn list_agent_commissions(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    query: AgentListQuery,
) -> AppResult<AgentCommissionsResponse> {
    let (pool, scope) = agent_context(mysql, subject).await?;
    let page = agent_list_page(query.limit, query.offset, 100);
    let commissions = infrastructure::list_agent_commissions(&pool, &scope, page).await?;
    Ok(agent_commissions_response(scope.agent_id, commissions))
}

/// 分页读取当前代理直接拥有的邀请码，不合并子代理邀请码且无写入副作用。
pub(crate) async fn list_agent_invite_codes(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    query: AgentListQuery,
) -> AppResult<AgentInviteCodesResponse> {
    let (pool, scope) = agent_context(mysql, subject).await?;
    let page = agent_list_page(query.limit, query.offset, 100);
    let invite_codes = infrastructure::list_agent_invite_codes(&pool, scope.agent_id, page).await?;
    Ok(AgentInviteCodesResponse { invite_codes })
}

/// 校验使用上限后为当前代理生成新邀请码，插入成功后再按所有权回读完整快照。
/// 冲突或数据库错误直接失败；本用例未开显式事务，回读缺失按未找到处理。
pub(crate) async fn create_agent_invite_code(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    request: CreateInviteCodeRequest,
) -> AppResult<AgentInviteCodeResponse> {
    let (pool, scope) = agent_context(mysql, subject).await?;
    validate_agent_invite_code_usage_limit(request.usage_limit)?;

    let write = AgentInviteCodeWrite {
        agent_id: scope.agent_id,
        code: generated_agent_invite_code(),
        usage_limit: request.usage_limit,
    };
    let invite_code_id = infrastructure::insert_agent_invite_code(&pool, write).await?;

    infrastructure::load_agent_invite_code_by_id(&pool, scope.agent_id, invite_code_id)
        .await?
        .ok_or(AppError::NotFound)
}

/// 仅允许邀请码所属代理在启用与停用之间切换，不匹配所有权时返回未找到。
/// 单语句更新后另行回读权威快照；重复设置不改使用次数或邀请关系，但数据库若对同值更新
/// 报告零受影响行，本用例会返回未找到，因此不承诺同值请求幂等成功。
pub(crate) async fn update_agent_invite_code_status(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    invite_code_id: u64,
    request: UpdateInviteCodeStatusRequest,
) -> AppResult<AgentInviteCodeResponse> {
    let status = validate_agent_invite_code_status(&request.status)?;
    let (pool, scope) = agent_context(mysql, subject).await?;
    let updated = infrastructure::update_agent_invite_code_status(
        &pool,
        scope.agent_id,
        invite_code_id,
        status,
    )
    .await?;

    if !updated {
        return Err(AppError::NotFound);
    }

    infrastructure::load_agent_invite_code_by_id(&pool, scope.agent_id, invite_code_id)
        .await?
        .ok_or(AppError::NotFound)
}

/// 校验新旧口令后锁定代理管理员凭证，同事务更新哈希并撤销 MySQL 刷新令牌。
/// 旧口令或账号状态不符时不写入；提交后尝试撤销 Sa-Token/Redis 会话且不签发新令牌。
/// 外部失败时新密码已生效；令牌枚举若被会话助手降级为空集合，也不能证明全部旧访问令牌已删除。
pub(crate) async fn change_agent_password(
    state: AppState,
    subject: &str,
    request: ChangeAgentPasswordRequest,
) -> AppResult<AgentPasswordChangeResponse> {
    let agent_admin_id = agent_admin_id_from_subject(subject)?;
    let (current_password, new_password) =
        validate_agent_password_change(request.current_password, request.new_password)?;
    let password_hash = hash_password(&new_password)?;
    let pool = agent_mysql_pool(state.mysql.clone())?;

    // 校验旧密码、写入新密码和吊销刷新令牌同事务提交，避免旧凭证在改密后仍可续期。
    let mut tx = pool.begin().await?;
    let credential = infrastructure::lock_agent_admin_credential_in_tx(&mut tx, agent_admin_id)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if credential.status != "active" {
        return Err(AppError::Unauthorized);
    }
    if !verify_password(&credential.password_hash, &current_password)? {
        return Err(AppError::Validation(
            "current_password is incorrect".to_owned(),
        ));
    }
    infrastructure::update_agent_admin_password_in_tx(&mut tx, agent_admin_id, &password_hash)
        .await?;
    infrastructure::revoke_agent_admin_refresh_tokens_in_tx(&mut tx, agent_admin_id).await?;
    tx.commit().await?;

    revoke_actor_auth_sessions(
        &state,
        &AuthActor::new(ActorType::Agent, agent_admin_id, None),
    )
    .await?;
    Ok(AgentPasswordChangeResponse {
        changed: true,
        requires_relogin: true,
    })
}

async fn agent_context(
    mysql: Option<Pool<MySql>>,
    subject: &str,
) -> AppResult<(Pool<MySql>, AgentAccessScope)> {
    let agent_admin_id = agent_admin_id_from_subject(subject)?;
    let pool = agent_mysql_pool(mysql)?;
    // 每个代理只能访问自己的 materialized-path 子树，不能借用顶级代理 ID 越权查看兄弟团队。
    let scope = infrastructure::load_agent_access_scope_for_admin(&pool, agent_admin_id)
        .await?
        .ok_or(AppError::Unauthorized)?;
    Ok((pool, scope))
}

fn agent_mysql_pool(mysql: Option<Pool<MySql>>) -> AppResult<Pool<MySql>> {
    mysql.ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for agent routes".to_owned())
    })
}
