use super::*;

#[derive(ToSchema)]
pub(super) struct UserAuthRequest {
    email: Option<String>,
    phone: Option<String>,
    username: Option<String>,
    password: Option<String>,
    country_code: Option<String>,
    code: Option<String>,
    invite_code: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct RegisterConfigResponse {
    email_code_required: bool,
    invite_code_required: bool,
}

#[derive(ToSchema)]
pub(super) struct LoginConfigResponse {
    username_login_enabled: bool,
}

#[derive(ToSchema)]
pub(super) struct RegisterEmailCodeRequest {
    email: String,
}

#[derive(ToSchema)]
pub(super) struct RegisterEmailCodeResponse {
    sent: bool,
    expires_in_seconds: i64,
}

#[derive(ToSchema)]
pub(super) struct AdminAuthRequest {
    username: Option<String>,
    password: Option<String>,
    role_id: Option<u64>,
}

#[derive(ToSchema)]
pub(super) struct AgentAuthRequest {
    username: Option<String>,
    password: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct RefreshRequest {
    refresh_token: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct TokenResponse {
    access_token: String,
    refresh_token: String,
    token_type: String,
    scope: TokenScope,
}

#[derive(ToSchema)]
#[schema(rename_all = "snake_case")]
pub(super) enum TokenScope {
    User,
    Admin,
    Agent,
}

#[derive(ToSchema)]
#[schema(rename_all = "snake_case")]
pub(super) enum LoginTwoFactorMode {
    None,
    UserEnabled,
    Mandatory,
}

#[derive(ToSchema)]
pub(super) struct LoginTwoFactorRequest {
    challenge_id: String,
    totp_code: String,
}

#[derive(ToSchema)]
pub(super) struct LoginTwoFactorResetCodeRequest {
    challenge_id: String,
}

#[derive(ToSchema)]
pub(super) struct LoginTwoFactorResetRequest {
    challenge_id: String,
    code: String,
}

#[derive(ToSchema)]
pub(super) struct LoginTwoFactorChallengeResponse {
    requires_2fa: bool,
    challenge_id: String,
    expires_in_seconds: i64,
}

#[derive(ToSchema)]
pub(super) struct LoginTwoFactorSetupChallengeResponse {
    requires_2fa_setup: bool,
    setup_challenge_id: String,
    expires_in_seconds: i64,
}

#[derive(ToSchema)]
pub(super) struct LoginTwoFactorCodeResponse {
    sent: bool,
    expires_in_seconds: i64,
}

#[derive(ToSchema)]
pub(super) struct LoginTwoFactorResetResponse {
    reset: bool,
    requires_relogin: bool,
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/register/config",
    tag = "auth",
    summary = "查询用户注册配置",
    responses(
        (status = 200, description = "查询成功", body = RegisterConfigResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_register_config() {}

#[utoipa::path(
    get,
    path = "/api/v1/auth/login/config",
    tag = "auth",
    summary = "查询用户登录配置",
    responses(
        (status = 200, description = "查询成功", body = LoginConfigResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_login_config() {}

#[utoipa::path(
    post,
    path = "/api/v1/auth/register/email-code",
    tag = "auth",
    summary = "发送注册邮箱验证码",
    request_body = RegisterEmailCodeRequest,
    responses(
        (status = 200, description = "发送成功", body = RegisterEmailCodeResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 409, description = "邮箱已存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn send_register_email_code() {}

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
    path = "/api/v1/auth/login/2fa",
    tag = "auth",
    summary = "提交用户登录 2FA 验证码",
    request_body = LoginTwoFactorRequest,
    responses(
        (status = 200, description = "验证成功并返回 token", body = TokenResponse),
        (status = 400, description = "challenge 过期或验证码错误", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn user_login_two_factor() {}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login/2fa/reset-code",
    tag = "auth",
    summary = "登录 2FA challenge 上下文发送重置验证码",
    request_body = LoginTwoFactorResetCodeRequest,
    responses(
        (status = 200, description = "发送成功", body = LoginTwoFactorCodeResponse),
        (status = 400, description = "challenge 过期或邮箱不可用", body = ErrorResponse),
        (status = 500, description = "服务内部错误或 SMTP 未配置", body = ErrorResponse)
    )
)]
fn send_login_two_factor_reset_code() {}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login/2fa/reset",
    tag = "auth",
    summary = "登录 2FA challenge 上下文重置用户 2FA",
    request_body = LoginTwoFactorResetRequest,
    responses(
        (status = 200, description = "重置成功，需重新登录", body = LoginTwoFactorResetResponse),
        (status = 400, description = "challenge 或验证码无效", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn reset_login_two_factor() {}

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
    summary = "管理员注册（仅空表引导或现有管理员创建）",
    request_body = AdminAuthRequest,
    responses(
        (status = 200, description = "注册成功", body = TokenResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "已初始化后缺少或携带无效管理员凭证", body = ErrorResponse),
        (status = 403, description = "凭证对应管理员不存在或已停用", body = ErrorResponse),
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
