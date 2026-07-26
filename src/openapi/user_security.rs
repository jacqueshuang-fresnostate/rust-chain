use super::*;

#[derive(ToSchema)]
#[schema(rename_all = "snake_case")]
pub(super) enum SecurityVerificationMethod {
    FundPassword,
    TwoFactor,
    FundPasswordAndTwoFactor,
}

#[derive(ToSchema)]
pub(super) struct PaymentPolicy {
    enabled: bool,
    method: SecurityVerificationMethod,
}

#[derive(ToSchema)]
pub(super) struct PaymentPolicies {
    withdraw: PaymentPolicy,
    spot_order: PaymentPolicy,
    convert: PaymentPolicy,
    earn_subscribe: PaymentPolicy,
}

#[derive(ToSchema)]
pub(super) struct ThirdPartyBindingPolicy {
    coinbase_wallet_enabled: bool,
    telegram_account_enabled: bool,
}

#[derive(ToSchema)]
pub(super) struct ThirdPartyBindingResponse {
    provider: String,
    account_identifier: String,
    display_name: Option<String>,
    status: String,
    #[schema(format = Int64)]
    created_at: i64,
    #[schema(format = Int64)]
    updated_at: i64,
}

#[derive(ToSchema)]
pub(super) struct ThirdPartyBindingStatusResponse {
    policy: ThirdPartyBindingPolicy,
    bindings: Vec<ThirdPartyBindingResponse>,
}

#[derive(ToSchema)]
pub(super) struct BindThirdPartyAccountRequest {
    #[schema(pattern = "^(coinbase_wallet|telegram_account)$")]
    provider: String,
    account_identifier: String,
    display_name: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct UserSecurityPolicy {
    login_2fa_mode: LoginTwoFactorMode,
    registration_invite_required: bool,
    username_login_enabled: bool,
    payment_policies: PaymentPolicies,
    third_party_bindings: ThirdPartyBindingPolicy,
}

#[derive(ToSchema)]
pub(super) struct UserProfileResponse {
    id: u64,
    username: Option<String>,
    email: Option<String>,
    phone: Option<String>,
    country_code: Option<String>,
    preferred_locale: Option<String>,
    default_locale: Option<String>,
    supported_locales: Option<Vec<String>>,
    status: String,
    kyc_level: i32,
    #[schema(format = Int64)]
    email_verified_at: Option<i64>,
    fund_password_set: bool,
    #[schema(format = Int64)]
    created_at: i64,
}

#[derive(ToSchema)]
pub(super) struct UpdateUsernameRequest {
    username: String,
}

#[derive(ToSchema)]
pub(super) struct UpdateUsernameResponse {
    username: String,
}

#[derive(ToSchema)]
pub(super) struct UserTwoFactorStatusResponse {
    totp_enabled: bool,
    login_2fa_enabled: bool,
    login_2fa_mode: LoginTwoFactorMode,
    can_toggle_login_2fa: bool,
    payment_policies: PaymentPolicies,
    third_party_bindings: ThirdPartyBindingPolicy,
}

#[derive(ToSchema)]
pub(super) struct SetupTwoFactorResponse {
    secret: String,
    otpauth_uri: String,
}

#[derive(ToSchema)]
pub(super) struct ConfirmTwoFactorRequest {
    totp_code: String,
}

#[derive(ToSchema)]
pub(super) struct UpdateLoginTwoFactorRequest {
    enabled: bool,
}

#[derive(ToSchema)]
pub(super) struct ResetTwoFactorRequest {
    code: String,
}

#[derive(ToSchema)]
pub(super) struct BindEmailCodeRequest {
    email: String,
}

#[derive(ToSchema)]
pub(super) struct BindEmailCodeResponse {
    sent: bool,
    #[schema(format = Int64)]
    expires_at: i64,
}

#[derive(ToSchema)]
pub(super) struct BindEmailRequest {
    email: String,
    code: String,
}

#[derive(ToSchema)]
pub(super) struct BindEmailResponse {
    email: String,
    #[schema(format = Int64)]
    email_verified_at: i64,
}

#[derive(ToSchema)]
pub(super) struct ChangePasswordRequest {
    old_password: String,
    new_password: String,
}

#[derive(ToSchema)]
pub(super) struct CreateFundPasswordRequest {
    login_password: String,
    fund_password: String,
}

#[derive(ToSchema)]
pub(super) struct ChangeFundPasswordRequest {
    old_fund_password: String,
    new_fund_password: String,
}

#[derive(ToSchema)]
pub(super) struct ResetFundPasswordRequest {
    code: String,
    new_fund_password: String,
}

#[derive(ToSchema)]
pub(super) struct FundPasswordResponse {
    fund_password_set: bool,
}

#[derive(ToSchema)]
pub(super) struct UpdateSecurityPolicyRequest {
    login_2fa_mode: LoginTwoFactorMode,
    registration_invite_required: bool,
    username_login_enabled: bool,
    payment_policies: PaymentPolicies,
    third_party_bindings: ThirdPartyBindingPolicy,
    reason: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct ResetUserTwoFactorRequest {
    reason: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct AdminUserTwoFactorResetResponse {
    user_id: u64,
    totp_enabled: bool,
    login_2fa_enabled: bool,
}

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
    patch,
    path = "/api/v1/user/username",
    tag = "user-security",
    summary = "更新用户登录用户名",
    request_body = UpdateUsernameRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "更新成功", body = UpdateUsernameResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 409, description = "用户名已存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn update_username() {}

#[utoipa::path(
    get,
    path = "/api/v1/user/2fa",
    tag = "user-security",
    summary = "查询用户 2FA 与安全策略状态",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = UserTwoFactorStatusResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_user_two_factor_status() {}

#[utoipa::path(
    get,
    path = "/api/v1/user/third-party-bindings",
    tag = "user-security",
    summary = "查询第三方账号绑定策略和当前绑定状态",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = ThirdPartyBindingStatusResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_user_third_party_bindings() {}

#[utoipa::path(
    post,
    path = "/api/v1/user/third-party-bindings",
    tag = "user-security",
    summary = "绑定第三方账号",
    request_body = BindThirdPartyAccountRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "绑定成功", body = ThirdPartyBindingStatusResponse),
        (status = 400, description = "参数错误或后台未开启绑定", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn bind_user_third_party_account() {}

#[utoipa::path(
    post,
    path = "/api/v1/user/2fa/setup",
    tag = "user-security",
    summary = "生成用户 2FA 绑定密钥",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "生成成功", body = SetupTwoFactorResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn setup_user_two_factor() {}

#[utoipa::path(
    post,
    path = "/api/v1/user/2fa/confirm",
    tag = "user-security",
    summary = "确认用户 2FA 绑定",
    request_body = ConfirmTwoFactorRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "确认成功", body = UserTwoFactorStatusResponse),
        (status = 400, description = "验证码错误或未开始绑定", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn confirm_user_two_factor() {}

#[utoipa::path(
    patch,
    path = "/api/v1/user/2fa/login",
    tag = "user-security",
    summary = "开启或关闭用户登录 2FA",
    request_body = UpdateLoginTwoFactorRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "更新成功", body = UserTwoFactorStatusResponse),
        (status = 400, description = "未绑定 2FA 或后台强制策略不允许关闭", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn update_user_login_two_factor() {}

#[utoipa::path(
    post,
    path = "/api/v1/user/2fa/reset-code",
    tag = "user-security",
    summary = "发送用户 2FA 重置验证码",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "发送成功", body = BindEmailCodeResponse),
        (status = 400, description = "邮箱不可用或发送过于频繁", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误或 SMTP 未配置", body = ErrorResponse)
    )
)]
fn send_user_two_factor_reset_code() {}

#[utoipa::path(
    post,
    path = "/api/v1/user/2fa/reset",
    tag = "user-security",
    summary = "通过邮箱验证码重置用户 2FA",
    request_body = ResetTwoFactorRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "重置成功", body = UserTwoFactorStatusResponse),
        (status = 400, description = "验证码错误或已过期", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn reset_user_two_factor() {}

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
    post,
    path = "/api/v1/user/fund-password/reset-code",
    tag = "user-security",
    summary = "发送资金密码重置验证码",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "发送成功", body = BindEmailCodeResponse),
        (status = 400, description = "未绑定已验证邮箱或发送过于频繁", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "尚未设置资金密码", body = ErrorResponse),
        (status = 500, description = "服务内部错误或 SMTP 未配置", body = ErrorResponse)
    )
)]
fn send_fund_password_reset_code() {}

#[utoipa::path(
    post,
    path = "/api/v1/user/fund-password/reset",
    tag = "user-security",
    summary = "通过邮箱验证码重置资金密码",
    request_body = ResetFundPasswordRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "重置成功", body = FundPasswordResponse),
        (status = 400, description = "验证码或资金密码格式错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "尚未设置资金密码", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn reset_fund_password() {}

#[utoipa::path(
    get,
    path = "/admin/api/v1/security-policy",
    tag = "admin-security",
    summary = "查询用户安全策略",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = UserSecurityPolicy),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_admin_security_policy() {}

#[utoipa::path(
    patch,
    path = "/admin/api/v1/security-policy",
    tag = "admin-security",
    summary = "更新用户安全策略",
    request_body = UpdateSecurityPolicyRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "更新成功", body = UserSecurityPolicy),
        (status = 400, description = "参数错误或缺少原因", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn update_admin_security_policy() {}

#[utoipa::path(
    post,
    path = "/admin/api/v1/users/{id}/2fa/reset",
    tag = "admin-security",
    summary = "后台重置用户 2FA",
    params(("id" = u64, Path, description = "用户 ID")),
    request_body = ResetUserTwoFactorRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "重置成功", body = AdminUserTwoFactorResetResponse),
        (status = 400, description = "参数错误或缺少原因", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "用户不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn reset_admin_user_two_factor() {}
