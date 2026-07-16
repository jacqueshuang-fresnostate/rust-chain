use super::{
    application::{
        create_agent_invite_code, get_agent_convert_stats, get_agent_dashboard, get_agent_me,
        list_agent_commissions, list_agent_invite_codes, list_agent_sub_agents,
        list_agent_team_tree, list_agent_users, update_agent_invite_code_status,
    },
    presentation::{
        AgentCommissionsResponse, AgentConvertStatsResponse, AgentDashboardResponse,
        AgentInviteCodeResponse, AgentInviteCodesResponse, AgentMeResponse, AgentSubAgentsResponse,
        AgentTeamTreeResponse, AgentUsersResponse, CreateInviteCodeRequest,
        UpdateInviteCodeStatusRequest,
    },
};
use crate::{error::AppResult, modules::auth::AgentAuth, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, patch},
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/me", get(me))
        .route("/dashboard", get(dashboard))
        .route("/users", get(list_users))
        .route(
            "/invite-codes",
            get(list_invite_codes).post(create_invite_code),
        )
        .route("/invite-codes/:id/status", patch(update_invite_code_status))
        .route("/commissions", get(list_commissions))
        .route("/convert/stats", get(convert_stats))
        .route("/sub-agents", get(sub_agents))
        .route("/team-tree", get(team_tree))
}

async fn me(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
) -> AppResult<Json<AgentMeResponse>> {
    Ok(Json(get_agent_me(state.mysql.clone(), &claims.sub).await?))
}

async fn dashboard(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
) -> AppResult<Json<AgentDashboardResponse>> {
    Ok(Json(
        get_agent_dashboard(state.mysql.clone(), &claims.sub).await?,
    ))
}

async fn convert_stats(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
) -> AppResult<Json<AgentConvertStatsResponse>> {
    Ok(Json(
        get_agent_convert_stats(state.mysql.clone(), &claims.sub).await?,
    ))
}

async fn list_users(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
) -> AppResult<Json<AgentUsersResponse>> {
    Ok(Json(
        list_agent_users(state.mysql.clone(), &claims.sub).await?,
    ))
}

async fn team_tree(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
) -> AppResult<Json<AgentTeamTreeResponse>> {
    Ok(Json(
        list_agent_team_tree(state.mysql.clone(), &claims.sub).await?,
    ))
}

async fn sub_agents(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
) -> AppResult<Json<AgentSubAgentsResponse>> {
    Ok(Json(
        list_agent_sub_agents(state.mysql.clone(), &claims.sub).await?,
    ))
}

async fn list_commissions(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
) -> AppResult<Json<AgentCommissionsResponse>> {
    Ok(Json(
        list_agent_commissions(state.mysql.clone(), &claims.sub).await?,
    ))
}

async fn list_invite_codes(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
) -> AppResult<Json<AgentInviteCodesResponse>> {
    Ok(Json(
        list_agent_invite_codes(state.mysql.clone(), &claims.sub).await?,
    ))
}

async fn create_invite_code(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateInviteCodeRequest>,
) -> AppResult<Json<AgentInviteCodeResponse>> {
    Ok(Json(
        create_agent_invite_code(state.mysql.clone(), &claims.sub, request).await?,
    ))
}

async fn update_invite_code_status(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
    Path(invite_code_id): Path<u64>,
    Json(request): Json<UpdateInviteCodeStatusRequest>,
) -> AppResult<Json<AgentInviteCodeResponse>> {
    Ok(Json(
        update_agent_invite_code_status(state.mysql.clone(), &claims.sub, invite_code_id, request)
            .await?,
    ))
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_agent_routes_tests.rs"]
mod tests;
