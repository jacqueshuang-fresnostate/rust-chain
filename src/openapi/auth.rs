//! 认证相关的 OpenAPI 契约：覆盖用户、管理员与代理三端的注册、登录、二次验证和令牌刷新。
//! 三端各自签发带作用域的令牌，令牌不能跨端使用，刷新接口也按端分开，互相之间不通用。
//! 登录二次验证靠 challenge 串起多步流程，因此这里同时声明了发起、绑定、确认与重置四类请求体。
//! 本文件只声明契约，真实处理逻辑在 auth 模块，调整路由时必须同步维护此处的路径与响应定义。

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
    cf_turnstile_enabled: bool,
    cf_turnstile_site_key: Option<String>,
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
pub(super) struct LoginTwoFactorSetupRequest {
    setup_challenge_id: String,
}

#[derive(ToSchema)]
pub(super) struct LoginTwoFactorSetupConfirmRequest {
    setup_challenge_id: String,
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
pub(super) struct LoginTwoFactorSetupResponse {
    secret: String,
    otpauth_uri: String,
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

/// 公开返回注册表单的开关组合，前端据此决定是否展示邮箱验证码与邀请码输入框。
/// 无需携带令牌，取值来自后台安全策略，响应里不含任何用户数据。
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

/// 公开返回用户端登录页所需的开关，包括是否允许用户名登录以及人机校验是否启用。
/// 启用人机校验时会一并给出站点公钥，该值本就用于前端渲染，不属于机密配置。
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

/// 返回后台登录页的同款开关结构，与用户端分开取值，便于两端采用不同的人机校验策略。
/// 该接口不要求登录，因为管理员在拿到令牌之前就需要知道登录页该渲染哪些控件。
#[utoipa::path(
    get,
    path = "/admin/api/v1/auth/login/config",
    tag = "auth",
    summary = "查询后台登录配置",
    responses(
        (status = 200, description = "查询成功", body = LoginConfigResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_admin_login_config() {}

/// 向待注册邮箱发送注册验证码，邮箱已被占用时返回冲突而不会照常发信。
/// 发送依赖后台已配置可用的邮件服务器，响应只回报是否发出与有效期，不回显验证码本身。
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

/// 完成用户注册并直接签发访问与刷新令牌，注册成功后无需再调一次登录接口。
/// 邮箱验证码与邀请码是否必填由注册配置决定；账号已存在返回冲突，其余入参问题返回参数错误。
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

/// 校验用户凭据并签发令牌，支持邮箱、手机号或用户名登录，允许哪种由登录配置控制。
/// 凭据错误统一返回未认证且不区分账号不存在与密码错误，避免接口被用来枚举已注册账号。
/// 若账号需要二次验证，本接口不直接给出令牌，而是转入 2FA challenge 流程。
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

/// 提交登录环节的动态口令完成二次验证，通过后才真正签发访问与刷新令牌。
/// 必须带上登录时拿到的 challenge 标识，challenge 过期与口令错误都归为参数错误。
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

/// 为强制启用二次验证但尚未绑定的账号生成密钥，返回密钥与可直接生成二维码的绑定链接。
/// challenge 类型不符、已过期或已被消费都会拒绝，防止同一次登录反复领取新密钥。
#[utoipa::path(
    post,
    path = "/api/v1/auth/login/2fa/setup",
    tag = "auth",
    summary = "首次强制 2FA 登录 challenge 生成 TOTP 密钥",
    request_body = LoginTwoFactorSetupRequest,
    responses(
        (status = 200, description = "生成成功", body = LoginTwoFactorSetupResponse),
        (status = 400, description = "challenge 类型错误、已过期或已消费", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn user_login_two_factor_setup() {}

/// 用刚绑定的验证器提交一次动态口令，确认绑定成功并在同一步完成登录、签发令牌。
/// 把绑定与登录合成一步，是为了避免用户扫码之后还要退回登录页从头再走一遍。
#[utoipa::path(
    post,
    path = "/api/v1/auth/login/2fa/setup/confirm",
    tag = "auth",
    summary = "确认首次强制 2FA 并完成登录",
    request_body = LoginTwoFactorSetupConfirmRequest,
    responses(
        (status = 200, description = "确认成功并返回 token", body = TokenResponse),
        (status = 400, description = "challenge 无效或 TOTP 验证码错误", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn user_login_two_factor_setup_confirm() {}

/// 在登录被二次验证拦住时，向账号已验证邮箱发送重置验证码，用于验证器丢失后的自助找回。
/// 此时用户尚未登录，只能凭 challenge 标识发起；邮箱不可用或邮件服务未配置都会失败。
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

/// 凭邮箱验证码在未登录状态下解除账号的二次验证绑定，成功后要求用户重新登录一次。
/// 本接口不签发令牌，只返回重置结果与是否需要重新登录的标记。
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

/// 用刷新令牌换取新的用户访问令牌，让前端在访问令牌到期时完成无感续期。
/// 刷新令牌无效或已过期返回未认证，此时只能引导用户重新输入账号口令登录。
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

/// 创建后台管理员账号，仅在管理员表为空的引导场景，或由已登录管理员携带有效凭证时才允许。
/// 初始化完成后缺少或携带无效凭证返回未认证，凭证对应管理员不存在或已停用返回禁止访问。
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

/// 校验管理员用户名与口令并签发后台作用域令牌，该令牌不能用于用户端接口。
/// 认证失败统一返回未认证，同样不区分账号不存在与口令错误。
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

/// 为后台会话续期，用刷新令牌换取新的管理员访问令牌，作用域仍限定在后台。
/// 刷新令牌失效返回未认证，此时管理员需要重新登录后台。
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

/// 代理自助注册通道已关闭，任何请求都直接返回禁止访问，不会创建任何账号。
/// 保留该路径只是为了给旧客户端一个明确的拒绝语义，代理账号必须由后台创建。
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

/// 校验代理凭据并签发代理作用域令牌，供代理门户查询团队、佣金与邀请码。
/// 代理账号由后台创建，因此登录失败通常意味着账号尚未开通或已被停用。
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

/// 为代理门户会话续期，用刷新令牌换取新的代理访问令牌，作用域保持不变。
/// 与另外两端的刷新接口互不通用，令牌作用域不匹配会被直接拒绝。
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

#[cfg(test)]
#[path = "../../tests/unit_src/src_openapi_auth_tests.rs"]
mod tests;
