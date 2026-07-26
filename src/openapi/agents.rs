use super::*;

#[derive(ToSchema)]
pub(super) struct AdminAgentResponse {
    id: u64,
    user_id: u64,
    email: Option<String>,
    parent_agent_id: Option<u64>,
    parent_agent_code: Option<String>,
    root_agent_id: u64,
    root_agent_code: String,
    agent_code: String,
    level: i32,
    path: String,
    status: String,
    direct_user_count: i64,
    team_user_count: i64,
    child_agent_count: i64,
    admin_user_id: Option<u64>,
    admin_username: Option<String>,
    admin_status: Option<String>,
    #[schema(format = Int64)]
    created_at: i64,
}

#[derive(ToSchema)]
pub(super) struct AdminAgentsResponse {
    agents: Vec<AdminAgentResponse>,
    total: i64,
}

#[derive(ToSchema)]
pub(super) struct AdminAgentUserResponse {
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
    path: String,
    #[schema(format = Int64)]
    referred_at: i64,
}

#[derive(ToSchema)]
pub(super) struct AdminAgentUsersResponse {
    users: Vec<AdminAgentUserResponse>,
    total: i64,
}

#[derive(ToSchema)]
pub(super) struct CreateAdminAgentRequest {
    user_id: u64,
    parent_agent_id: Option<u64>,
    agent_code: String,
    admin_username: String,
    admin_password: String,
    level: Option<i32>,
    reason: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct UpdateAdminAgentStatusRequest {
    #[schema(pattern = "^(active|suspended|disabled)$")]
    status: String,
    reason: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct AssignUserAgentRequest {
    agent_id: u64,
    reason: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct AdminAgentCommissionResponse {
    id: u64,
    agent_id: u64,
    user_id: u64,
    source_type: String,
    source_id: String,
    source_amount: String,
    payout_asset_id: Option<u64>,
    commission_rate: String,
    commission_amount: String,
    status: String,
    #[schema(format = Int64)]
    created_at: i64,
}

#[derive(ToSchema)]
pub(super) struct AdminAgentCommissionsResponse {
    commissions: Vec<AdminAgentCommissionResponse>,
    total: i64,
}

#[derive(ToSchema)]
pub(super) struct UpdateAdminAgentCommissionStatusRequest {
    #[schema(pattern = "^(settled|rejected)$")]
    status: String,
    reason: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct BatchUpdateAdminAgentCommissionStatusRequest {
    #[schema(max_items = 200)]
    ids: Vec<u64>,
    #[schema(pattern = "^(settled|rejected)$")]
    status: String,
    reason: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct AdminAgentCommissionBatchStatusItemResponse {
    id: u64,
    #[schema(pattern = "^(ok|failed)$")]
    status: String,
    error: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct AdminAgentCommissionBatchStatusResponse {
    results: Vec<AdminAgentCommissionBatchStatusItemResponse>,
}

#[derive(ToSchema)]
pub(super) struct AdminAgentCommissionRuleResponse {
    id: u64,
    agent_id: u64,
    #[schema(pattern = "^(convert|prediction|spot|margin|seconds_contract)$")]
    product_type: String,
    commission_rate: String,
    #[schema(pattern = "^(active|disabled)$")]
    status: String,
    #[schema(format = Int64)]
    created_at: i64,
    #[schema(format = Int64)]
    updated_at: i64,
}

#[derive(ToSchema)]
pub(super) struct AdminAgentCommissionRulesResponse {
    rules: Vec<AdminAgentCommissionRuleResponse>,
    total: i64,
}

#[derive(ToSchema)]
pub(super) struct CreateAdminAgentCommissionRuleRequest {
    agent_id: u64,
    #[schema(pattern = "^(convert|prediction|spot|margin|seconds_contract)$")]
    product_type: String,
    commission_rate: String,
    #[schema(pattern = "^(active|disabled)$")]
    status: Option<String>,
    reason: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct UpdateAdminAgentCommissionRuleRequest {
    commission_rate: Option<String>,
    #[schema(pattern = "^(active|disabled)$")]
    status: Option<String>,
    reason: Option<String>,
}

#[utoipa::path(
    get,
    path = "/admin/api/v1/agents",
    tag = "admin-agent",
    summary = "查询代理列表",
    params(
        ("agent_id" = Option<u64>, Query, description = "代理 ID"),
        ("user_id" = Option<u64>, Query, description = "绑定用户 ID"),
        ("parent_agent_id" = Option<u64>, Query, description = "直属上级代理 ID"),
        ("root_agent_id" = Option<u64>, Query, description = "总代理 ID"),
        ("level" = Option<i32>, Query, description = "代理层级，1 至 3"),
        ("agent_code" = Option<String>, Query, description = "代理编号"),
        ("email" = Option<String>, Query, description = "绑定用户邮箱"),
        ("status" = Option<String>, Query, description = "代理状态"),
        ("limit" = Option<u32>, Query, description = "分页数量"),
        ("offset" = Option<u32>, Query, description = "分页偏移")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AdminAgentsResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_admin_agents() {}

#[utoipa::path(
    post,
    path = "/admin/api/v1/agents",
    tag = "admin-agent",
    summary = "创建代理",
    request_body = CreateAdminAgentRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "创建成功", body = AdminAgentResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "用户不存在", body = ErrorResponse),
        (status = 409, description = "代理编号、用户或后台账号重复", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn create_admin_agent() {}

#[utoipa::path(
    get,
    path = "/admin/api/v1/agents/{id}",
    tag = "admin-agent",
    summary = "查询代理详情",
    params(("id" = u64, Path, description = "代理 ID")),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AdminAgentResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "代理不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_admin_agent() {}

#[utoipa::path(
    patch,
    path = "/admin/api/v1/agents/{id}/status",
    tag = "admin-agent",
    summary = "更新代理状态",
    params(("id" = u64, Path, description = "代理 ID")),
    request_body = UpdateAdminAgentStatusRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "更新成功", body = AdminAgentResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "代理不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn update_admin_agent_status() {}

#[utoipa::path(
    get,
    path = "/admin/api/v1/agents/{id}/users",
    tag = "admin-agent",
    summary = "查询代理节点及其下级代理归属的用户",
    params(
        ("id" = u64, Path, description = "代理 ID"),
        ("limit" = Option<u32>, Query, description = "返回数量"),
        ("offset" = Option<u32>, Query, description = "分页偏移")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AdminAgentUsersResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "代理不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_admin_agent_users() {}

#[utoipa::path(
    patch,
    path = "/admin/api/v1/users/{id}/agent",
    tag = "admin-agent",
    summary = "分配用户代理归属",
    params(("id" = u64, Path, description = "用户 ID")),
    request_body = AssignUserAgentRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "分配成功"),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "用户或代理不存在", body = ErrorResponse),
        (status = 409, description = "目标代理不是 active", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn assign_user_agent() {}

#[utoipa::path(
    get,
    path = "/admin/api/v1/agent-commissions",
    tag = "admin-agent",
    summary = "查询代理佣金列表",
    params(
        ("agent_id" = Option<u64>, Query, description = "代理 ID"),
        ("user_id" = Option<u64>, Query, description = "用户 ID"),
        ("email" = Option<String>, Query, description = "用户邮箱"),
        ("status" = Option<String>, Query, description = "佣金状态"),
        ("limit" = Option<u32>, Query, description = "分页数量"),
        ("offset" = Option<u32>, Query, description = "分页偏移")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AdminAgentCommissionsResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_admin_agent_commissions() {}

#[utoipa::path(
    patch,
    path = "/admin/api/v1/agent-commissions/{id}/status",
    tag = "admin-agent",
    summary = "更新代理佣金状态",
    params(("id" = u64, Path, description = "佣金记录 ID")),
    request_body = UpdateAdminAgentCommissionStatusRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "更新成功", body = AdminAgentCommissionResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "佣金记录不存在", body = ErrorResponse),
        (status = 409, description = "佣金来源不支持结算打款", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn update_admin_agent_commission_status() {}

#[utoipa::path(
    post,
    path = "/admin/api/v1/agent-commissions/batch-status",
    tag = "admin-agent",
    summary = "批量更新代理佣金状态",
    request_body = BatchUpdateAdminAgentCommissionStatusRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "批量处理完成，逐条返回结果", body = AdminAgentCommissionBatchStatusResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn update_admin_agent_commission_statuses() {}

#[utoipa::path(
    get,
    path = "/admin/api/v1/agent-commission-rules",
    tag = "admin-agent",
    summary = "查询代理佣金规则列表",
    params(
        ("agent_id" = Option<u64>, Query, description = "代理 ID"),
        ("product_type" = Option<String>, Query, description = "产品类型：convert、prediction、spot、margin 或 seconds_contract"),
        ("status" = Option<String>, Query, description = "规则状态"),
        ("limit" = Option<u32>, Query, description = "分页数量"),
        ("offset" = Option<u32>, Query, description = "分页偏移")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AdminAgentCommissionRulesResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_admin_agent_commission_rules() {}

#[utoipa::path(
    post,
    path = "/admin/api/v1/agent-commission-rules",
    tag = "admin-agent",
    summary = "创建代理佣金规则",
    request_body = CreateAdminAgentCommissionRuleRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "创建成功", body = AdminAgentCommissionRuleResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn create_admin_agent_commission_rule() {}

#[utoipa::path(
    patch,
    path = "/admin/api/v1/agent-commission-rules/{id}",
    tag = "admin-agent",
    summary = "更新代理佣金规则",
    params(("id" = u64, Path, description = "佣金规则 ID")),
    request_body = UpdateAdminAgentCommissionRuleRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "更新成功", body = AdminAgentCommissionRuleResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "佣金规则不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn update_admin_agent_commission_rule() {}
