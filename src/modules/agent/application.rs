//! agent bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。

use crate::{
    architecture::ApplicationLayer,
    error::{AppError, AppResult},
    modules::agent::{
        infrastructure,
        presentation::{
            AgentCommissionsResponse, AgentConvertStatsResponse, AgentDashboardResponse,
            AgentInviteCodeResponse, AgentInviteCodesResponse, AgentMeResponse,
            AgentSubAgentsResponse, AgentTeamTreeResponse, AgentUsersResponse,
            CreateInviteCodeRequest, UpdateInviteCodeStatusRequest,
        },
        repository::{AgentAccessScope, AgentInviteCodeWrite},
        service::{
            agent_admin_id_from_subject, agent_commissions_response, agent_convert_stats_response,
            generated_agent_invite_code, validate_agent_invite_code_status,
            validate_agent_invite_code_usage_limit,
        },
    },
};
use sqlx::{MySql, Pool};

#[derive(Debug)]
pub struct ApplicationLayerMarker;

impl ApplicationLayer for ApplicationLayerMarker {}

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

pub(crate) async fn get_agent_dashboard(
    mysql: Option<Pool<MySql>>,
    subject: &str,
) -> AppResult<AgentDashboardResponse> {
    let (pool, scope) = agent_context(mysql, subject).await?;
    infrastructure::load_agent_dashboard(&pool, &scope).await
}

pub(crate) async fn get_agent_convert_stats(
    mysql: Option<Pool<MySql>>,
    subject: &str,
) -> AppResult<AgentConvertStatsResponse> {
    let (pool, scope) = agent_context(mysql, subject).await?;
    let row = infrastructure::load_agent_convert_stats(&pool, &scope).await?;
    agent_convert_stats_response(row)
}

pub(crate) async fn list_agent_users(
    mysql: Option<Pool<MySql>>,
    subject: &str,
) -> AppResult<AgentUsersResponse> {
    let (pool, scope) = agent_context(mysql, subject).await?;
    let users = infrastructure::list_agent_team_users(&pool, &scope).await?;
    Ok(AgentUsersResponse { users })
}

pub(crate) async fn list_agent_sub_agents(
    mysql: Option<Pool<MySql>>,
    subject: &str,
) -> AppResult<AgentSubAgentsResponse> {
    let (pool, scope) = agent_context(mysql, subject).await?;
    let agents = infrastructure::list_agent_sub_agents(&pool, &scope).await?;
    Ok(AgentSubAgentsResponse { agents })
}

pub(crate) async fn list_agent_team_tree(
    mysql: Option<Pool<MySql>>,
    subject: &str,
) -> AppResult<AgentTeamTreeResponse> {
    let (pool, scope) = agent_context(mysql, subject).await?;
    let agents = infrastructure::list_agent_sub_agents(&pool, &scope).await?;
    let nodes = infrastructure::list_agent_team_tree_nodes(&pool, &scope).await?;

    Ok(AgentTeamTreeResponse {
        root_agent_id: scope.root_agent_id,
        agents,
        nodes,
    })
}

pub(crate) async fn list_agent_commissions(
    mysql: Option<Pool<MySql>>,
    subject: &str,
) -> AppResult<AgentCommissionsResponse> {
    let (pool, scope) = agent_context(mysql, subject).await?;
    let commissions = infrastructure::list_agent_commissions(&pool, &scope).await?;
    Ok(agent_commissions_response(scope.agent_id, commissions))
}

pub(crate) async fn list_agent_invite_codes(
    mysql: Option<Pool<MySql>>,
    subject: &str,
) -> AppResult<AgentInviteCodesResponse> {
    let (pool, scope) = agent_context(mysql, subject).await?;
    let invite_codes = infrastructure::list_agent_invite_codes(&pool, scope.agent_id).await?;
    Ok(AgentInviteCodesResponse { invite_codes })
}

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
