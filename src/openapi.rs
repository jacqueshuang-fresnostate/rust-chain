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
        user_profile,
        send_email_bind_code,
        bind_email,
        change_password,
        create_fund_password,
        change_fund_password,
        get_smtp_config,
        save_smtp_config,
        send_smtp_test
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
        SendSmtpTestResponse
    )),
    tags(
        (name = "health", description = "服务健康检查"),
        (name = "auth", description = "用户、管理员和代理认证"),
        (name = "user-security", description = "用户邮箱、登录密码和资金密码"),
        (name = "admin-smtp", description = "后台 SMTP 邮件配置")
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
    agent_id: Option<u64>,
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
    summary = "代理注册",
    request_body = AgentAuthRequest,
    responses(
        (status = 200, description = "注册成功", body = TokenResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 409, description = "账号已存在", body = ErrorResponse),
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
