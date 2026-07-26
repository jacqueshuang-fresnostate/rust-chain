use super::*;

#[derive(ToSchema)]
pub(super) struct AgentMeResponse {
    agent_admin_id: u64,
    agent_id: u64,
    username: String,
    agent_code: String,
    parent_agent_id: Option<u64>,
    root_agent_id: u64,
    level: i32,
    path: String,
    agent_status: String,
    admin_status: String,
    #[schema(format = Int64)]
    last_login_at: Option<i64>,
}

#[derive(ToSchema)]
pub(super) struct AgentDashboardResponse {
    agent_id: u64,
    team_user_count: i64,
    active_invite_code_count: i64,
    commission_record_count: i64,
    pending_commission_amount: String,
    settled_commission_amount: String,
    total_commission_amount: String,
}

#[derive(ToSchema)]
pub(super) struct AgentTeamUserResponse {
    user_id: u64,
    email: Option<String>,
    phone: Option<String>,
    status: String,
    kyc_level: i32,
    owner_agent_id: u64,
    root_agent_id: u64,
    owner_agent_code: String,
    owner_agent_level: i32,
    direct_inviter_id: Option<u64>,
    direct_inviter_type: Option<String>,
    depth: i32,
    #[schema(format = Int64)]
    referred_at: i64,
}

#[derive(ToSchema)]
pub(super) struct AgentUsersResponse {
    users: Vec<AgentTeamUserResponse>,
}

#[derive(ToSchema)]
pub(super) struct CreateAgentInviteCodeRequest {
    usage_limit: Option<i32>,
}

#[derive(ToSchema)]
pub(super) struct UpdateAgentInviteCodeStatusRequest {
    #[schema(pattern = "^(active|disabled)$")]
    status: String,
}

#[derive(ToSchema)]
pub(super) struct AgentInviteCodeResponse {
    id: u64,
    owner_id: u64,
    code: String,
    usage_limit: Option<i32>,
    used_count: i32,
    #[schema(pattern = "^(active|disabled)$")]
    status: String,
    #[schema(format = Int64)]
    created_at: i64,
}

#[derive(ToSchema)]
pub(super) struct AgentInviteCodesResponse {
    invite_codes: Vec<AgentInviteCodeResponse>,
}

#[derive(ToSchema)]
pub(super) struct AgentCommissionResponse {
    id: u64,
    user_id: u64,
    email: Option<String>,
    source_type: String,
    source_id: String,
    source_amount: String,
    commission_rate: String,
    commission_amount: String,
    status: String,
    depth: i32,
    payout_ledger_id: Option<u64>,
    payout_asset_id: Option<u64>,
    payout_amount: Option<String>,
    payout_balance_after: Option<String>,
    #[schema(format = Int64)]
    payout_created_at: Option<i64>,
    #[schema(format = Int64)]
    created_at: i64,
}

#[derive(ToSchema)]
pub(super) struct AgentCommissionsResponse {
    agent_id: u64,
    total_records: u64,
    total_commission_amount: String,
    commissions: Vec<AgentCommissionResponse>,
}

#[derive(ToSchema)]
pub(super) struct AgentConvertStatsResponse {
    agent_id: u64,
    total_orders: i64,
    pending_orders: i64,
    completed_orders: i64,
    total_from_amount: String,
    total_to_amount: String,
}

#[derive(ToSchema)]
pub(super) struct AgentSubAgentResponse {
    id: u64,
    parent_agent_id: Option<u64>,
    root_agent_id: u64,
    agent_code: String,
    level: i32,
    path: String,
    status: String,
    direct_user_count: i64,
    team_user_count: i64,
}

#[derive(ToSchema)]
pub(super) struct AgentSubAgentsResponse {
    agents: Vec<AgentSubAgentResponse>,
}

#[derive(ToSchema)]
pub(super) struct AgentTeamTreeNodeResponse {
    user_id: u64,
    email: Option<String>,
    phone: Option<String>,
    status: String,
    direct_inviter_id: Option<u64>,
    direct_inviter_type: Option<String>,
    owner_agent_id: u64,
    owner_agent_code: String,
    owner_agent_level: i32,
    depth: i32,
    path: String,
    #[schema(format = Int64)]
    referred_at: i64,
}

#[derive(ToSchema)]
pub(super) struct AgentTeamTreeResponse {
    root_agent_id: u64,
    agents: Vec<AgentSubAgentResponse>,
    nodes: Vec<AgentTeamTreeNodeResponse>,
}

#[utoipa::path(
    get,
    path = "/agent/api/v1/me",
    tag = "agent-portal",
    summary = "查询当前代理身份",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AgentMeResponse),
        (status = 401, description = "未登录或代理已停用", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_agent_me() {}

#[utoipa::path(
    get,
    path = "/agent/api/v1/dashboard",
    tag = "agent-portal",
    summary = "查询代理总览",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AgentDashboardResponse),
        (status = 401, description = "未登录或代理已停用", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_agent_dashboard() {}

#[utoipa::path(
    get,
    path = "/agent/api/v1/users",
    tag = "agent-portal",
    summary = "查询代理团队用户",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AgentUsersResponse),
        (status = 401, description = "未登录或代理已停用", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_agent_users() {}

#[utoipa::path(
    get,
    path = "/agent/api/v1/invite-codes",
    tag = "agent-portal",
    summary = "查询代理邀请码",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AgentInviteCodesResponse),
        (status = 401, description = "未登录或代理已停用", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_agent_invite_codes() {}

#[utoipa::path(
    post,
    path = "/agent/api/v1/invite-codes",
    tag = "agent-portal",
    summary = "创建代理邀请码",
    request_body = CreateAgentInviteCodeRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "创建成功", body = AgentInviteCodeResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录或代理已停用", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn create_agent_invite_code() {}

#[utoipa::path(
    patch,
    path = "/agent/api/v1/invite-codes/{id}/status",
    tag = "agent-portal",
    summary = "更新代理邀请码状态",
    params(("id" = u64, Path, description = "邀请码 ID")),
    request_body = UpdateAgentInviteCodeStatusRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "更新成功", body = AgentInviteCodeResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录或代理已停用", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "邀请码不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn update_agent_invite_code_status() {}

#[utoipa::path(
    get,
    path = "/agent/api/v1/commissions",
    tag = "agent-portal",
    summary = "查询代理佣金记录",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AgentCommissionsResponse),
        (status = 401, description = "未登录或代理已停用", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_agent_commissions() {}

#[utoipa::path(
    get,
    path = "/agent/api/v1/convert/stats",
    tag = "agent-portal",
    summary = "查询代理闪兑统计",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AgentConvertStatsResponse),
        (status = 401, description = "未登录或代理已停用", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_agent_convert_stats() {}

#[utoipa::path(
    get,
    path = "/agent/api/v1/sub-agents",
    tag = "agent-portal",
    summary = "查询当前代理的全部下级代理",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AgentSubAgentsResponse),
        (status = 401, description = "未登录、当前代理或任一上级已停用", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_agent_sub_agents() {}

#[utoipa::path(
    get,
    path = "/agent/api/v1/team-tree",
    tag = "agent-portal",
    summary = "查询代理团队树",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AgentTeamTreeResponse),
        (status = 401, description = "未登录或代理已停用", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_agent_team_tree() {}
