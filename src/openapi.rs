//! Public OpenAPI contract only; internal rows, ciphertexts, and test helpers stay out of schemas.

#![allow(dead_code)]

use axum::Router;
use utoipa::{
    Modify, OpenApi, ToSchema,
    openapi::security::{Http, HttpAuthScheme, SecurityScheme},
};
use utoipa_swagger_ui::SwaggerUi;

use crate::{HealthResponse, error::ErrorResponse, state::AppState};

pub fn routes() -> Router<AppState> {
    let docs: Router<AppState> = SwaggerUi::new("/docs")
        .url("/openapi.json", ApiDoc::openapi())
        .into();
    let api_docs: Router<AppState> = SwaggerUi::new("/api/docs")
        .url("/api/openapi.json", ApiDoc::openapi())
        .into();

    docs.merge(api_docs)
}

#[derive(OpenApi)]
#[openapi(
    paths(
        health,
        user_register,
        user_login,
        user_refresh,
        admin_register,
        admin_login,
        admin_refresh,
        agent_register,
        agent_login,
        agent_refresh,
        get_agent_me,
        get_agent_dashboard,
        list_agent_users,
        list_agent_invite_codes,
        create_agent_invite_code,
        update_agent_invite_code_status,
        list_agent_commissions,
        get_agent_convert_stats,
        get_agent_team_tree,
        user_profile,
        send_email_bind_code,
        bind_email,
        change_password,
        create_fund_password,
        change_fund_password,
        get_smtp_config,
        save_smtp_config,
        send_smtp_test,
        list_admin_agents,
        create_admin_agent,
        get_admin_agent,
        update_admin_agent_status,
        assign_user_agent,
        list_admin_agent_commissions,
        update_admin_agent_commission_status,
        list_admin_agent_commission_rules,
        create_admin_agent_commission_rule,
        update_admin_agent_commission_rule,
        list_admin_news,
        create_admin_news,
        get_admin_news,
        update_admin_news,
        update_admin_news_status,
        list_public_news,
        get_public_news
    ),
    components(schemas(
        ErrorResponse,
        HealthResponse,
        UserAuthRequest,
        AdminAuthRequest,
        AgentAuthRequest,
        RefreshRequest,
        TokenResponse,
        UserProfileResponse,
        BindEmailCodeRequest,
        BindEmailCodeResponse,
        BindEmailRequest,
        BindEmailResponse,
        ChangePasswordRequest,
        CreateFundPasswordRequest,
        ChangeFundPasswordRequest,
        FundPasswordResponse,
        SaveSmtpConfigRequest,
        SmtpConfigResponse,
        SendSmtpTestRequest,
        SendSmtpTestResponse,
        AdminAgentResponse,
        AdminAgentsResponse,
        CreateAdminAgentRequest,
        UpdateAdminAgentStatusRequest,
        AssignUserAgentRequest,
        AdminAgentCommissionResponse,
        AdminAgentCommissionsResponse,
        UpdateAdminAgentCommissionStatusRequest,
        AdminAgentCommissionRuleResponse,
        AdminAgentCommissionRulesResponse,
        CreateAdminAgentCommissionRuleRequest,
        UpdateAdminAgentCommissionRuleRequest,
        NewsRichTextLeaf,
        NewsRichTextBlock,
        NewsContentTranslation,
        NewsContentDocument,
        AdminNewsItemResponse,
        AdminNewsItemsResponse,
        CreateAdminNewsItemRequest,
        UpdateAdminNewsItemRequest,
        UpdateAdminNewsStatusRequest,
        PublicNewsItemResponse,
        PublicNewsItemsResponse,
        AgentMeResponse,
        AgentDashboardResponse,
        AgentTeamUserResponse,
        AgentUsersResponse,
        CreateAgentInviteCodeRequest,
        UpdateAgentInviteCodeStatusRequest,
        AgentInviteCodeResponse,
        AgentInviteCodesResponse,
        AgentCommissionResponse,
        AgentCommissionsResponse,
        AgentConvertStatsResponse,
        AgentTeamTreeNodeResponse,
        AgentTeamTreeResponse
    )),
    tags(
        (name = "health", description = "服务健康检查"),
        (name = "auth", description = "用户、管理员和代理认证"),
        (name = "user-security", description = "用户邮箱、登录密码和资金密码"),
        (name = "admin-smtp", description = "后台 SMTP 邮件配置"),
        (name = "admin-agent", description = "后台代理、归属和佣金管理"),
        (name = "admin-news", description = "后台新闻中心管理"),
        (name = "news", description = "用户端公开新闻中心"),
        (name = "agent-portal", description = "代理门户数据查询和邀请码管理")
    ),
    modifiers(&SecurityAddon)
)]
struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearerAuth",
            SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
        );
    }
}

#[derive(ToSchema)]
struct UserAuthRequest {
    email: Option<String>,
    phone: Option<String>,
    password: Option<String>,
}

#[derive(ToSchema)]
struct AdminAuthRequest {
    username: Option<String>,
    password: Option<String>,
    role_id: Option<u64>,
}

#[derive(ToSchema)]
struct AgentAuthRequest {
    username: Option<String>,
    password: Option<String>,
}

#[derive(ToSchema)]
struct RefreshRequest {
    refresh_token: Option<String>,
}

#[derive(ToSchema)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    token_type: String,
    scope: TokenScope,
}

#[derive(ToSchema)]
#[schema(rename_all = "snake_case")]
enum TokenScope {
    User,
    Admin,
    Agent,
}

#[derive(ToSchema)]
struct UserProfileResponse {
    id: u64,
    email: Option<String>,
    phone: Option<String>,
    status: String,
    kyc_level: i32,
    #[schema(format = Int64)]
    email_verified_at: Option<i64>,
    fund_password_set: bool,
    #[schema(format = Int64)]
    created_at: i64,
}

#[derive(ToSchema)]
struct BindEmailCodeRequest {
    email: String,
}

#[derive(ToSchema)]
struct BindEmailCodeResponse {
    sent: bool,
    #[schema(format = Int64)]
    expires_at: i64,
}

#[derive(ToSchema)]
struct BindEmailRequest {
    email: String,
    code: String,
}

#[derive(ToSchema)]
struct BindEmailResponse {
    email: String,
    #[schema(format = Int64)]
    email_verified_at: i64,
}

#[derive(ToSchema)]
struct ChangePasswordRequest {
    old_password: String,
    new_password: String,
}

#[derive(ToSchema)]
struct CreateFundPasswordRequest {
    login_password: String,
    fund_password: String,
}

#[derive(ToSchema)]
struct ChangeFundPasswordRequest {
    old_fund_password: String,
    new_fund_password: String,
}

#[derive(ToSchema)]
struct FundPasswordResponse {
    fund_password_set: bool,
}

#[derive(ToSchema)]
struct SaveSmtpConfigRequest {
    host: String,
    port: u16,
    #[schema(pattern = "^(none|starttls|tls)$")]
    security: String,
    username: Option<String>,
    #[schema(nullable = true)]
    password: Option<String>,
    from_email: String,
    from_name: Option<String>,
    enabled: bool,
    reason: Option<String>,
}

#[derive(ToSchema)]
struct SmtpConfigResponse {
    id: u64,
    name: String,
    host: String,
    port: u16,
    security: String,
    username_mask: Option<String>,
    password_set: bool,
    from_email: String,
    from_name: Option<String>,
    enabled: bool,
}

#[derive(ToSchema)]
struct SendSmtpTestRequest {
    recipient: String,
    reason: Option<String>,
}

#[derive(ToSchema)]
struct SendSmtpTestResponse {
    sent: bool,
    recipient: String,
}

#[derive(ToSchema)]
struct AdminAgentResponse {
    id: u64,
    user_id: u64,
    email: Option<String>,
    agent_code: String,
    level: i32,
    status: String,
    admin_user_id: Option<u64>,
    admin_username: Option<String>,
    admin_status: Option<String>,
    #[schema(format = Int64)]
    created_at: i64,
}

#[derive(ToSchema)]
struct AdminAgentsResponse {
    agents: Vec<AdminAgentResponse>,
}

#[derive(ToSchema)]
struct CreateAdminAgentRequest {
    user_id: u64,
    agent_code: String,
    admin_username: String,
    admin_password: String,
    level: Option<i32>,
    reason: Option<String>,
}

#[derive(ToSchema)]
struct UpdateAdminAgentStatusRequest {
    #[schema(pattern = "^(active|suspended|disabled)$")]
    status: String,
    reason: Option<String>,
}

#[derive(ToSchema)]
struct AssignUserAgentRequest {
    agent_id: u64,
    reason: Option<String>,
}

#[derive(ToSchema)]
struct AdminAgentCommissionResponse {
    id: u64,
    agent_id: u64,
    user_id: u64,
    source_type: String,
    source_id: String,
    source_amount: String,
    commission_amount: String,
    status: String,
    #[schema(format = Int64)]
    created_at: i64,
}

#[derive(ToSchema)]
struct AdminAgentCommissionsResponse {
    commissions: Vec<AdminAgentCommissionResponse>,
}

#[derive(ToSchema)]
struct UpdateAdminAgentCommissionStatusRequest {
    #[schema(pattern = "^(settled|rejected)$")]
    status: String,
    reason: Option<String>,
}

#[derive(ToSchema)]
struct AdminAgentCommissionRuleResponse {
    id: u64,
    agent_id: u64,
    #[schema(pattern = "^convert$")]
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
struct AdminAgentCommissionRulesResponse {
    rules: Vec<AdminAgentCommissionRuleResponse>,
}

#[derive(ToSchema)]
struct CreateAdminAgentCommissionRuleRequest {
    agent_id: u64,
    #[schema(pattern = "^convert$")]
    product_type: String,
    commission_rate: String,
    #[schema(pattern = "^(active|disabled)$")]
    status: Option<String>,
    reason: Option<String>,
}

#[derive(ToSchema)]
struct UpdateAdminAgentCommissionRuleRequest {
    commission_rate: Option<String>,
    #[schema(pattern = "^(active|disabled)$")]
    status: Option<String>,
    reason: Option<String>,
}

#[derive(ToSchema)]
struct NewsRichTextLeaf {
    text: String,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
}

#[derive(ToSchema)]
struct NewsRichTextBlock {
    #[schema(pattern = "^(p|h1|h2|h3|blockquote)$")]
    r#type: String,
    children: Vec<NewsRichTextLeaf>,
}

#[derive(ToSchema)]
struct NewsContentTranslation {
    locale: String,
    country_code: String,
    title: String,
    summary: Option<String>,
    content: Vec<NewsRichTextBlock>,
}

#[derive(ToSchema)]
struct NewsContentDocument {
    version: u8,
    default_locale: String,
    items: Vec<NewsContentTranslation>,
}

#[derive(ToSchema)]
struct AdminNewsItemResponse {
    id: u64,
    title: String,
    #[schema(pattern = "^(general|market|product|system|promotion)$")]
    category: String,
    #[schema(pattern = "^(draft|published|archived)$")]
    status: String,
    country_code: Option<String>,
    default_locale: String,
    content_json: NewsContentDocument,
    #[schema(format = Int64)]
    published_at: Option<i64>,
    created_by_admin_id: Option<u64>,
    updated_by_admin_id: Option<u64>,
    #[schema(format = Int64)]
    created_at: i64,
    #[schema(format = Int64)]
    updated_at: i64,
}

#[derive(ToSchema)]
struct AdminNewsItemsResponse {
    news: Vec<AdminNewsItemResponse>,
}

#[derive(ToSchema)]
struct PublicNewsItemResponse {
    id: u64,
    title: String,
    #[schema(pattern = "^(general|market|product|system|promotion)$")]
    category: String,
    #[schema(pattern = "^published$")]
    status: String,
    country_code: Option<String>,
    default_locale: String,
    content_json: NewsContentDocument,
    #[schema(format = Int64)]
    published_at: Option<i64>,
    #[schema(format = Int64)]
    created_at: i64,
    #[schema(format = Int64)]
    updated_at: i64,
}

#[derive(ToSchema)]
struct PublicNewsItemsResponse {
    news: Vec<PublicNewsItemResponse>,
}

#[derive(ToSchema)]
struct CreateAdminNewsItemRequest {
    title: String,
    #[schema(pattern = "^(general|market|product|system|promotion)$")]
    category: String,
    #[schema(pattern = "^(draft|published|archived)$")]
    status: Option<String>,
    country_code: Option<String>,
    default_locale: String,
    content_json: NewsContentDocument,
    reason: Option<String>,
}

#[derive(ToSchema)]
struct UpdateAdminNewsItemRequest {
    title: String,
    #[schema(pattern = "^(general|market|product|system|promotion)$")]
    category: String,
    country_code: Option<String>,
    default_locale: String,
    content_json: NewsContentDocument,
    reason: Option<String>,
}

#[derive(ToSchema)]
struct UpdateAdminNewsStatusRequest {
    #[schema(pattern = "^(draft|published|archived)$")]
    status: String,
    reason: Option<String>,
}

#[derive(ToSchema)]
struct AgentMeResponse {
    agent_admin_id: u64,
    agent_id: u64,
    username: String,
    agent_code: String,
    level: i32,
    agent_status: String,
    admin_status: String,
    #[schema(format = Int64)]
    last_login_at: Option<i64>,
}

#[derive(ToSchema)]
struct AgentDashboardResponse {
    agent_id: u64,
    team_user_count: i64,
    active_invite_code_count: i64,
    commission_record_count: i64,
    pending_commission_amount: String,
    settled_commission_amount: String,
    total_commission_amount: String,
}

#[derive(ToSchema)]
struct AgentTeamUserResponse {
    user_id: u64,
    email: Option<String>,
    phone: Option<String>,
    status: String,
    kyc_level: i32,
    root_agent_id: u64,
    depth: i32,
    #[schema(format = Int64)]
    referred_at: i64,
}

#[derive(ToSchema)]
struct AgentUsersResponse {
    users: Vec<AgentTeamUserResponse>,
}

#[derive(ToSchema)]
struct CreateAgentInviteCodeRequest {
    usage_limit: Option<i32>,
}

#[derive(ToSchema)]
struct UpdateAgentInviteCodeStatusRequest {
    #[schema(pattern = "^(active|disabled)$")]
    status: String,
}

#[derive(ToSchema)]
struct AgentInviteCodeResponse {
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
struct AgentInviteCodesResponse {
    invite_codes: Vec<AgentInviteCodeResponse>,
}

#[derive(ToSchema)]
struct AgentCommissionResponse {
    id: u64,
    user_id: u64,
    email: Option<String>,
    source_type: String,
    source_id: String,
    source_amount: String,
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
struct AgentCommissionsResponse {
    agent_id: u64,
    total_records: u64,
    total_commission_amount: String,
    commissions: Vec<AgentCommissionResponse>,
}

#[derive(ToSchema)]
struct AgentConvertStatsResponse {
    agent_id: u64,
    total_orders: i64,
    pending_orders: i64,
    completed_orders: i64,
    total_from_amount: String,
    total_to_amount: String,
}

#[derive(ToSchema)]
struct AgentTeamTreeNodeResponse {
    user_id: u64,
    email: Option<String>,
    phone: Option<String>,
    status: String,
    direct_inviter_id: Option<u64>,
    direct_inviter_type: Option<String>,
    depth: i32,
    path: String,
    #[schema(format = Int64)]
    referred_at: i64,
}

#[derive(ToSchema)]
struct AgentTeamTreeResponse {
    root_agent_id: u64,
    nodes: Vec<AgentTeamTreeNodeResponse>,
}

#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    summary = "服务健康检查",
    responses((status = 200, description = "服务可用", body = HealthResponse))
)]
fn health() {}

#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    tag = "auth",
    summary = "用户注册",
    request_body = UserAuthRequest,
    responses(
        (status = 200, description = "注册成功", body = TokenResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 409, description = "账号已存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn user_register() {}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    tag = "auth",
    summary = "用户登录",
    request_body = UserAuthRequest,
    responses(
        (status = 200, description = "登录成功", body = TokenResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "认证失败", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn user_login() {}

#[utoipa::path(
    post,
    path = "/api/v1/auth/refresh",
    tag = "auth",
    summary = "刷新用户 token",
    request_body = RefreshRequest,
    responses(
        (status = 200, description = "刷新成功", body = TokenResponse),
        (status = 401, description = "refresh token 无效", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn user_refresh() {}

#[utoipa::path(
    post,
    path = "/admin/api/v1/auth/register",
    tag = "auth",
    summary = "管理员注册",
    request_body = AdminAuthRequest,
    responses(
        (status = 200, description = "注册成功", body = TokenResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 409, description = "账号已存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn admin_register() {}

#[utoipa::path(
    post,
    path = "/admin/api/v1/auth/login",
    tag = "auth",
    summary = "管理员登录",
    request_body = AdminAuthRequest,
    responses(
        (status = 200, description = "登录成功", body = TokenResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "认证失败", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn admin_login() {}

#[utoipa::path(
    post,
    path = "/admin/api/v1/auth/refresh",
    tag = "auth",
    summary = "刷新管理员 token",
    request_body = RefreshRequest,
    responses(
        (status = 200, description = "刷新成功", body = TokenResponse),
        (status = 401, description = "refresh token 无效", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn admin_refresh() {}

#[utoipa::path(
    post,
    path = "/agent/api/v1/auth/register",
    tag = "auth",
    summary = "代理自助注册已关闭",
    request_body = AgentAuthRequest,
    responses(
        (status = 403, description = "代理账号必须由后台创建", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn agent_register() {}

#[utoipa::path(
    post,
    path = "/agent/api/v1/auth/login",
    tag = "auth",
    summary = "代理登录",
    request_body = AgentAuthRequest,
    responses(
        (status = 200, description = "登录成功", body = TokenResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "认证失败", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn agent_login() {}

#[utoipa::path(
    post,
    path = "/agent/api/v1/auth/refresh",
    tag = "auth",
    summary = "刷新代理 token",
    request_body = RefreshRequest,
    responses(
        (status = 200, description = "刷新成功", body = TokenResponse),
        (status = 401, description = "refresh token 无效", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn agent_refresh() {}

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

#[utoipa::path(
    get,
    path = "/api/v1/user/profile",
    tag = "user-security",
    summary = "获取用户资料和安全状态",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = UserProfileResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn user_profile() {}

#[utoipa::path(
    post,
    path = "/api/v1/user/email/bind-code",
    tag = "user-security",
    summary = "发送邮箱绑定验证码",
    request_body = BindEmailCodeRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "发送成功", body = BindEmailCodeResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 409, description = "邮箱已被占用", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn send_email_bind_code() {}

#[utoipa::path(
    post,
    path = "/api/v1/user/email/bind",
    tag = "user-security",
    summary = "绑定并验证邮箱",
    request_body = BindEmailRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "绑定成功", body = BindEmailResponse),
        (status = 400, description = "验证码错误或已过期", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 409, description = "邮箱已被占用", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn bind_email() {}

#[utoipa::path(
    patch,
    path = "/api/v1/user/password",
    tag = "user-security",
    summary = "修改登录密码",
    request_body = ChangePasswordRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "修改成功并返回新 token", body = TokenResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "旧密码错误或未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn change_password() {}

#[utoipa::path(
    post,
    path = "/api/v1/user/fund-password",
    tag = "user-security",
    summary = "新建资金密码",
    request_body = CreateFundPasswordRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "设置成功", body = FundPasswordResponse),
        (status = 400, description = "资金密码格式错误", body = ErrorResponse),
        (status = 401, description = "登录密码错误或未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 409, description = "资金密码已存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn create_fund_password() {}

#[utoipa::path(
    patch,
    path = "/api/v1/user/fund-password",
    tag = "user-security",
    summary = "修改资金密码",
    request_body = ChangeFundPasswordRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "修改成功", body = FundPasswordResponse),
        (status = 400, description = "资金密码格式错误", body = ErrorResponse),
        (status = 401, description = "旧资金密码错误或未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "尚未设置资金密码", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn change_fund_password() {}

#[utoipa::path(
    get,
    path = "/admin/api/v1/agents",
    tag = "admin-agent",
    summary = "查询代理列表",
    params(
        ("agent_id" = Option<u64>, Query, description = "代理 ID"),
        ("user_id" = Option<u64>, Query, description = "绑定用户 ID"),
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
        ("limit" = Option<u32>, Query, description = "分页数量")
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
    get,
    path = "/admin/api/v1/agent-commission-rules",
    tag = "admin-agent",
    summary = "查询代理佣金规则列表",
    params(
        ("agent_id" = Option<u64>, Query, description = "代理 ID"),
        ("product_type" = Option<String>, Query, description = "产品类型；本轮仅支持 convert"),
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

#[utoipa::path(
    get,
    path = "/admin/api/v1/news",
    tag = "admin-news",
    summary = "查询后台新闻列表",
    params(
        ("status" = Option<String>, Query, description = "新闻状态"),
        ("category" = Option<String>, Query, description = "新闻分类"),
        ("country_code" = Option<String>, Query, description = "国家或地区代码"),
        ("locale" = Option<String>, Query, description = "语言代码"),
        ("q" = Option<String>, Query, description = "标题或内容关键词"),
        ("limit" = Option<u32>, Query, description = "分页数量"),
        ("offset" = Option<u32>, Query, description = "分页偏移")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AdminNewsItemsResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_admin_news() {}

#[utoipa::path(
    post,
    path = "/admin/api/v1/news",
    tag = "admin-news",
    summary = "创建后台新闻",
    request_body = CreateAdminNewsItemRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "创建成功", body = AdminNewsItemResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn create_admin_news() {}

#[utoipa::path(
    get,
    path = "/admin/api/v1/news/{id}",
    tag = "admin-news",
    summary = "查询后台新闻详情",
    params(("id" = u64, Path, description = "新闻 ID")),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AdminNewsItemResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "新闻不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_admin_news() {}

#[utoipa::path(
    patch,
    path = "/admin/api/v1/news/{id}",
    tag = "admin-news",
    summary = "更新后台新闻",
    params(("id" = u64, Path, description = "新闻 ID")),
    request_body = UpdateAdminNewsItemRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "更新成功", body = AdminNewsItemResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "新闻不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn update_admin_news() {}

#[utoipa::path(
    patch,
    path = "/admin/api/v1/news/{id}/status",
    tag = "admin-news",
    summary = "更新后台新闻状态",
    params(("id" = u64, Path, description = "新闻 ID")),
    request_body = UpdateAdminNewsStatusRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "更新成功", body = AdminNewsItemResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "新闻不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn update_admin_news_status() {}

#[utoipa::path(
    get,
    path = "/api/v1/news",
    tag = "news",
    summary = "查询用户端公开新闻列表",
    params(
        ("category" = Option<String>, Query, description = "新闻分类"),
        ("country_code" = Option<String>, Query, description = "国家或地区代码"),
        ("locale" = Option<String>, Query, description = "语言代码"),
        ("q" = Option<String>, Query, description = "标题或内容关键词"),
        ("limit" = Option<u32>, Query, description = "分页数量"),
        ("offset" = Option<u32>, Query, description = "分页偏移")
    ),
    responses(
        (status = 200, description = "查询成功", body = PublicNewsItemsResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_public_news() {}

#[utoipa::path(
    get,
    path = "/api/v1/news/{id}",
    tag = "news",
    summary = "查询用户端公开新闻详情",
    params(("id" = u64, Path, description = "新闻 ID")),
    responses(
        (status = 200, description = "查询成功", body = PublicNewsItemResponse),
        (status = 404, description = "新闻不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_public_news() {}

#[utoipa::path(
    get,
    path = "/admin/api/v1/smtp/config",
    tag = "admin-smtp",
    summary = "查询 SMTP 配置",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = SmtpConfigResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "配置不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_smtp_config() {}

#[utoipa::path(
    patch,
    path = "/admin/api/v1/smtp/config",
    tag = "admin-smtp",
    summary = "保存 SMTP 配置",
    request_body = SaveSmtpConfigRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "保存成功", body = SmtpConfigResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn save_smtp_config() {}

#[utoipa::path(
    post,
    path = "/admin/api/v1/smtp/test",
    tag = "admin-smtp",
    summary = "发送 SMTP 测试邮件",
    request_body = SendSmtpTestRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "发送成功", body = SendSmtpTestResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "启用配置不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn send_smtp_test() {}
