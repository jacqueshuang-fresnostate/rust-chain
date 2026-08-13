//! 用户安全的 OpenAPI 契约：覆盖资料查询、用户名修改、邮箱绑定、登录密码、资金密码与二次验证。
//! 资金密码独立于登录密码，用于提现等资金操作的二次确认，两者的修改与重置流程各自成套。
//! 二次验证提供绑定、开关与邮箱验证码重置三条路径，登录尚未完成时另有一套基于 challenge 的版本。
//! 文件末尾另含后台安全策略读写与强制重置用户二次验证的管理端接口，它们要求后台作用域令牌。

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

/// 返回当前登录用户的资料与安全状态汇总，供个人中心首屏一次性渲染。
/// 只读取令牌对应的用户，不接受任何用户标识参数，因此不存在越权查看他人资料的入口。
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

/// 修改登录用户名并返回更新结果，用户名已被他人占用时返回冲突。
/// 用户名本身是可选的登录方式之一，改名会直接影响之后能否再用旧名登录。
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

/// 查询当前用户的二次验证绑定与开关状态，同时带回后台下发的安全策略。
/// 前端据此判断该展示绑定引导、开关控件，还是策略强制开启且不可关闭的提示。
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

/// 查询第三方账号的绑定策略与当前绑定情况，策略决定哪些渠道允许绑定。
/// 后台关闭某个渠道后已有绑定仍会返回，但前端不应再展示该渠道的新增入口。
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

/// 为当前用户绑定一个第三方账号，返回绑定后的完整状态便于前端直接刷新页面。
/// 后台未开启对应渠道时按参数错误拒绝，不会先写入绑定再回头校验策略。
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

/// 为已登录用户生成二次验证密钥与绑定链接，用于在验证器应用中扫码添加。
/// 本步只是发放密钥，绑定尚未生效，必须再调确认接口提交一次动态口令才算完成。
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

/// 提交验证器生成的动态口令以确认绑定，通过后返回最新的二次验证状态。
/// 未先领取密钥或口令错误都返回参数错误，失败时不会残留半绑定状态。
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

/// 开启或关闭登录环节的二次验证要求，只影响登录，不会解除验证器绑定本身。
/// 尚未绑定验证器就想开启，或后台策略强制开启时想关闭，两种情况都会被拒绝。
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

/// 在已登录状态下向账号已验证邮箱发送重置验证码，用于验证器丢失后的自助解绑。
/// 邮箱不可用或发送过于频繁返回参数错误，邮件服务未配置则归为服务内部错误。
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

/// 凭邮箱验证码在已登录状态下解除二次验证绑定，返回重置之后的安全状态。
/// 与登录环节的重置接口区别在于此处身份已确定，无需再携带 challenge 标识。
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

/// 向待绑定邮箱发送验证码，该邮箱已被其他账号占用时返回冲突且不会发信。
/// 验证码只服务于本次绑定流程，响应中不回显验证码内容，仅给出有效期。
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

/// 用验证码完成邮箱绑定并同时标记为已验证，绑定后该邮箱可用于登录与各类安全验证。
/// 验证码错误或过期返回参数错误，邮箱在此期间被他人抢先绑定则返回冲突。
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

/// 修改登录密码，需要同时提供旧密码，成功后直接返回新令牌，避免改密后立即掉线。
/// 旧密码错误返回未认证，与未登录共用同一状态码，前端需要结合业务上下文加以区分。
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

/// 首次设置资金密码，需要用登录密码确认身份；已设置过则返回冲突，不允许重复创建。
/// 资金密码与登录密码相互独立，用于提现这类资金操作的二次确认。
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

/// 凭旧资金密码修改为新的资金密码，尚未设置过资金密码时返回资源不存在。
/// 旧资金密码错误返回未认证，新密码格式不符合要求则返回参数错误。
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

/// 向账号已验证邮箱发送资金密码重置验证码，未绑定已验证邮箱时无法使用这种找回方式。
/// 尚未设置过资金密码时返回资源不存在，因为此时应当走创建流程而不是重置。
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

/// 凭邮箱验证码在忘记旧资金密码的情况下重置为新值，不需要再提供旧资金密码。
/// 验证码或新密码格式不符合要求返回参数错误，尚未设置过资金密码返回资源不存在。
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

/// 后台查询全站用户安全策略，包括登录二次验证的强制程度与各类资金操作的校验要求。
/// 该策略是用户端安全接口的判定依据，读取本身不会产生任何变更。
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

/// 更新全站用户安全策略，变更即时对所有用户生效，因此必须同时提供审计原因。
/// 缺少原因或参数不合法都会被拒绝，避免无据可查地放宽全站安全要求。
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

/// 管理员为指定用户强制解除二次验证绑定，用于用户既丢失验证器又收不到邮件的兜底场景。
/// 必须填写原因以便审计留痕，目标用户不存在时返回资源不存在。
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
