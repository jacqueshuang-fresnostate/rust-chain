//! 代理门户的 OpenAPI 契约：面向代理本人，提供身份、团队、邀请码、佣金与业务统计的自助查询。
//! 所有端点的数据范围都由令牌对应的代理确定，不接受代理标识参数，因此不存在跨代理越权查询。
//! 代理或其链路上任一上级被停用时访问即被拒绝，避免停用分支继续获取团队数据。
//! 门户只提供邀请码的创建与启停两个写操作，佣金状态与用户归属的变更必须由后台完成。

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
pub(super) struct AgentDashboardAssetSummaryResponse {
    payout_asset_id: Option<u64>,
    commission_record_count: i64,
    pending_commission_amount: String,
    settled_commission_amount: String,
    total_commission_amount: String,
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
    commission_assets: Vec<AgentDashboardAssetSummaryResponse>,
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

/// 返回当前登录代理的自身身份信息，包括代理编号、层级与绑定账号。
/// 代理被停用后即便令牌未过期也会被拒绝，因此该接口也能用来探测账号是否仍然有效。
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

/// 返回代理门户首页的汇总数据，包含团队规模与资产统计等总览指标。
/// 统计口径覆盖该代理的整棵团队树，供门户首屏一次性渲染，不需要前端再逐项拼装。
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

/// 分页查询归属于当前代理的团队用户，单页最多返回一百条。
/// 只能看到自己团队内的用户，代理无法通过参数越权查询其他代理的成员。
#[utoipa::path(
    get,
    path = "/agent/api/v1/users",
    tag = "agent-portal",
    summary = "查询代理团队用户",
    params(
        ("limit" = Option<u32>, Query, description = "返回条数，默认 100，最大 100"),
        ("offset" = Option<u32>, Query, description = "偏移量，默认 0")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AgentUsersResponse),
        (status = 401, description = "未登录或代理已停用", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_agent_users() {}

/// 分页查询当前代理名下的邀请码及其状态，单页最多返回一百条。
/// 邀请码是新用户注册时归属到该代理的凭据，列表主要用于确认哪些码仍然可用。
#[utoipa::path(
    get,
    path = "/agent/api/v1/invite-codes",
    tag = "agent-portal",
    summary = "查询代理邀请码",
    params(
        ("limit" = Option<u32>, Query, description = "返回条数，默认 100，最大 100"),
        ("offset" = Option<u32>, Query, description = "偏移量，默认 0")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AgentInviteCodesResponse),
        (status = 401, description = "未登录或代理已停用", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_agent_invite_codes() {}

/// 为当前代理创建一个新的邀请码，创建后即可分发给潜在用户用于注册归属。
/// 代理只能给自己创建邀请码，归属由令牌确定，请求中无法指定挂到其他代理名下。
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

/// 启用或停用自己名下的某个邀请码，停用之后该码不能再用于新用户注册。
/// 已经通过该码注册的用户归属不受影响，邀请码不存在时返回资源不存在。
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

/// 分页查询当前代理的佣金记录，按创建时间倒序返回，单页最多一百条。
/// 门户侧仅供查看，代理不能在这里变更佣金状态，也无法自行触发结算打款。
#[utoipa::path(
    get,
    path = "/agent/api/v1/commissions",
    tag = "agent-portal",
    summary = "查询代理佣金记录（按创建时间倒序）",
    params(
        ("limit" = Option<u32>, Query, description = "返回条数，默认 100，最大 100"),
        ("offset" = Option<u32>, Query, description = "偏移量，默认 0")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AgentCommissionsResponse),
        (status = 401, description = "未登录或代理已停用", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_agent_commissions() {}

/// 返回当前代理团队的闪兑业务统计，用于评估该产品线对团队收益的贡献度。
/// 口径限定在自己的团队范围内，不包含平台整体数据，也不含其他代理的成交。
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

/// 分页查询当前代理的全部下级代理，单页最多返回五百条。
/// 只要链路上任一上级被停用即拒绝访问，避免停用的分支仍能借下级查询继续拿到团队数据。
#[utoipa::path(
    get,
    path = "/agent/api/v1/sub-agents",
    tag = "agent-portal",
    summary = "查询当前代理的全部下级代理",
    params(
        ("limit" = Option<u32>, Query, description = "返回条数，默认 500，最大 500"),
        ("offset" = Option<u32>, Query, description = "偏移量，默认 0")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AgentSubAgentsResponse),
        (status = 401, description = "未登录、当前代理或任一上级已停用", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_agent_sub_agents() {}

/// 以树形结构返回代理团队的层级关系，单页最多五百条，用于门户绘制组织结构图。
/// 与下级代理列表的区别在于这里保留父子层级，而不是拉平成一维列表。
#[utoipa::path(
    get,
    path = "/agent/api/v1/team-tree",
    tag = "agent-portal",
    summary = "查询代理团队树",
    params(
        ("limit" = Option<u32>, Query, description = "返回条数，默认 500，最大 500"),
        ("offset" = Option<u32>, Query, description = "偏移量，默认 0")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AgentTeamTreeResponse),
        (status = 401, description = "未登录或代理已停用", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_agent_team_tree() {}
