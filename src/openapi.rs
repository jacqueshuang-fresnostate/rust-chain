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
        get_register_config,
        get_login_config,
        send_register_email_code,
        user_register,
        user_login,
        user_login_two_factor,
        send_login_two_factor_reset_code,
        reset_login_two_factor,
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
        list_agent_sub_agents,
        get_agent_team_tree,
        list_public_countries,
        get_public_platform_brand,
        user_profile,
        update_username,
        get_user_two_factor_status,
        setup_user_two_factor,
        confirm_user_two_factor,
        update_user_login_two_factor,
        send_user_two_factor_reset_code,
        reset_user_two_factor,
        get_user_third_party_bindings,
        bind_user_third_party_account,
        send_email_bind_code,
        bind_email,
        change_password,
        create_fund_password,
        change_fund_password,
        send_fund_password_reset_code,
        reset_fund_password,
        list_deposit_assets,
        list_withdraw_assets,
        get_or_assign_deposit_address,
        get_user_quick_recharge_config,
        create_user_quick_recharge_order,
        list_user_quick_recharge_orders,
        gmpay_notify,
        create_withdrawal_request,
        get_smtp_config,
        list_smtp_configs,
        create_smtp_config,
        update_smtp_config,
        save_smtp_config,
        save_smtp_delivery_settings,
        send_smtp_test,
        get_admin_platform_brand,
        save_admin_platform_brand,
        list_admin_deposit_address_pool,
        create_admin_deposit_address_pool,
        create_admin_deposit_address_pool_batch,
        get_admin_deposit_address_pool,
        update_admin_deposit_address_pool,
        reclaim_admin_deposit_address_pool,
        get_admin_quick_recharge_config,
        save_admin_quick_recharge_config,
        test_admin_quick_recharge_config,
        list_admin_quick_recharge_orders,
        delete_admin_quick_recharge_order,
        list_admin_countries,
        create_admin_country,
        update_admin_country,
        update_admin_country_status,
        list_admin_agents,
        create_admin_agent,
        get_admin_agent,
        update_admin_agent_status,
        list_admin_agent_users,
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
        get_admin_security_policy,
        update_admin_security_policy,
        reset_admin_user_two_factor,
        list_public_news,
        get_public_news
    ),
    components(schemas(
        ErrorResponse,
        HealthResponse,
        UserAuthRequest,
        RegisterConfigResponse,
        LoginConfigResponse,
        RegisterEmailCodeRequest,
        RegisterEmailCodeResponse,
        AdminAuthRequest,
        AgentAuthRequest,
        RefreshRequest,
        TokenResponse,
        UserProfileResponse,
        UpdateUsernameRequest,
        UpdateUsernameResponse,
        LoginTwoFactorRequest,
        LoginTwoFactorResetCodeRequest,
        LoginTwoFactorResetRequest,
        LoginTwoFactorChallengeResponse,
        LoginTwoFactorSetupChallengeResponse,
        LoginTwoFactorCodeResponse,
        LoginTwoFactorResetResponse,
        UserTwoFactorStatusResponse,
        SetupTwoFactorResponse,
        ConfirmTwoFactorRequest,
        UpdateLoginTwoFactorRequest,
        ResetTwoFactorRequest,
        PaymentPolicy,
        PaymentPolicies,
        ThirdPartyBindingPolicy,
        ThirdPartyBindingResponse,
        ThirdPartyBindingStatusResponse,
        BindThirdPartyAccountRequest,
        UserSecurityPolicy,
        PublicCountryResponse,
        PublicCountriesResponse,
        PlatformBrandResponse,
        SavePlatformBrandRequest,
        AdminCountryResponse,
        AdminCountriesResponse,
        CreateAdminCountryRequest,
        UpdateAdminCountryRequest,
        UpdateAdminCountryStatusRequest,
        BindEmailCodeRequest,
        BindEmailCodeResponse,
        BindEmailRequest,
        BindEmailResponse,
        ChangePasswordRequest,
        CreateFundPasswordRequest,
        ChangeFundPasswordRequest,
        ResetFundPasswordRequest,
        FundPasswordResponse,
        CreateWithdrawalRequest,
        WithdrawalRequestResponse,
        DepositAssetResponse,
        DepositAssetsResponse,
        DepositAddressRequest,
        DepositAddressResponse,
        UserQuickRechargeConfigResponse,
        QuickRechargeReturnTarget,
        CreateQuickRechargeOrderRequest,
        QuickRechargeOrderResponse,
        QuickRechargeOrdersResponse,
        SaveQuickRechargeConfigRequest,
        QuickRechargeConfigResponse,
        TestQuickRechargeConfigRequest,
        TestQuickRechargeConfigResponse,
        DeleteQuickRechargeOrderRequest,
        GmpayNotifyRequest,
        AdminDepositAddressPoolResponse,
        AdminDepositAddressPoolResponseList,
        AdminDepositAddressPoolBatchResponse,
        CreateDepositAddressPoolRequest,
        CreateDepositAddressPoolBatchRequest,
        CreateDepositAddressPoolEntryRequest,
        UpdateDepositAddressPoolRequest,
        ReclaimDepositAddressPoolRequest,
        UpdateSecurityPolicyRequest,
        ResetUserTwoFactorRequest,
        AdminUserTwoFactorResetResponse,
        SaveSmtpConfigRequest,
        SmtpConfigResponse,
        SmtpDeliverySettingsResponse,
        SmtpConfigListResponse,
        SaveSmtpDeliverySettingsRequest,
        SendSmtpTestRequest,
        SendSmtpTestResponse,
        AdminAgentResponse,
        AdminAgentsResponse,
        AdminAgentUserResponse,
        AdminAgentUsersResponse,
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
        AgentSubAgentResponse,
        AgentSubAgentsResponse,
        AgentTeamTreeNodeResponse,
        AgentTeamTreeResponse
    )),
    tags(
        (name = "health", description = "服务健康检查"),
        (name = "auth", description = "用户、管理员和代理认证"),
        (name = "countries", description = "用户端可注册国家和默认语言配置"),
        (name = "platform", description = "用户端公开平台品牌配置"),
        (name = "user-security", description = "用户邮箱、登录密码、资金密码和 2FA"),
        (name = "wallet", description = "用户钱包账户、流水和提现"),
        (name = "admin-platform", description = "后台平台品牌配置"),
        (name = "admin-wallet", description = "后台钱包、充值地址池和流水配置"),
        (name = "admin-countries", description = "后台国家、地区和语言配置"),
        (name = "admin-smtp", description = "后台 SMTP 邮件配置"),
        (name = "admin-agent", description = "后台代理、归属和佣金管理"),
        (name = "admin-news", description = "后台新闻中心管理"),
        (name = "admin-security", description = "后台用户安全策略和 2FA 重置"),
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
    username: Option<String>,
    password: Option<String>,
    country_code: Option<String>,
    code: Option<String>,
    invite_code: Option<String>,
}

#[derive(ToSchema)]
struct RegisterConfigResponse {
    email_code_required: bool,
    invite_code_required: bool,
}

#[derive(ToSchema)]
struct LoginConfigResponse {
    username_login_enabled: bool,
}

#[derive(ToSchema)]
struct RegisterEmailCodeRequest {
    email: String,
}

#[derive(ToSchema)]
struct RegisterEmailCodeResponse {
    sent: bool,
    expires_in_seconds: i64,
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
#[schema(rename_all = "snake_case")]
enum LoginTwoFactorMode {
    None,
    UserEnabled,
    Mandatory,
}

#[derive(ToSchema)]
#[schema(rename_all = "snake_case")]
enum SecurityVerificationMethod {
    FundPassword,
    TwoFactor,
    FundPasswordAndTwoFactor,
}

#[derive(ToSchema)]
struct PaymentPolicy {
    enabled: bool,
    method: SecurityVerificationMethod,
}

#[derive(ToSchema)]
struct PaymentPolicies {
    withdraw: PaymentPolicy,
    spot_order: PaymentPolicy,
    convert: PaymentPolicy,
    earn_subscribe: PaymentPolicy,
}

#[derive(ToSchema)]
struct ThirdPartyBindingPolicy {
    coinbase_wallet_enabled: bool,
    telegram_account_enabled: bool,
}

#[derive(ToSchema)]
struct ThirdPartyBindingResponse {
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
struct ThirdPartyBindingStatusResponse {
    policy: ThirdPartyBindingPolicy,
    bindings: Vec<ThirdPartyBindingResponse>,
}

#[derive(ToSchema)]
struct BindThirdPartyAccountRequest {
    #[schema(pattern = "^(coinbase_wallet|telegram_account)$")]
    provider: String,
    account_identifier: String,
    display_name: Option<String>,
}

#[derive(ToSchema)]
struct UserSecurityPolicy {
    login_2fa_mode: LoginTwoFactorMode,
    registration_invite_required: bool,
    username_login_enabled: bool,
    payment_policies: PaymentPolicies,
    third_party_bindings: ThirdPartyBindingPolicy,
}

#[derive(ToSchema)]
struct UserProfileResponse {
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
struct UpdateUsernameRequest {
    username: String,
}

#[derive(ToSchema)]
struct UpdateUsernameResponse {
    username: String,
}

#[derive(ToSchema)]
struct LoginTwoFactorRequest {
    challenge_id: String,
    totp_code: String,
}

#[derive(ToSchema)]
struct LoginTwoFactorResetCodeRequest {
    challenge_id: String,
}

#[derive(ToSchema)]
struct LoginTwoFactorResetRequest {
    challenge_id: String,
    code: String,
}

#[derive(ToSchema)]
struct LoginTwoFactorChallengeResponse {
    requires_2fa: bool,
    challenge_id: String,
    expires_in_seconds: i64,
}

#[derive(ToSchema)]
struct LoginTwoFactorSetupChallengeResponse {
    requires_2fa_setup: bool,
    setup_challenge_id: String,
    expires_in_seconds: i64,
}

#[derive(ToSchema)]
struct LoginTwoFactorCodeResponse {
    sent: bool,
    expires_in_seconds: i64,
}

#[derive(ToSchema)]
struct LoginTwoFactorResetResponse {
    reset: bool,
    requires_relogin: bool,
}

#[derive(ToSchema)]
struct UserTwoFactorStatusResponse {
    totp_enabled: bool,
    login_2fa_enabled: bool,
    login_2fa_mode: LoginTwoFactorMode,
    can_toggle_login_2fa: bool,
    payment_policies: PaymentPolicies,
    third_party_bindings: ThirdPartyBindingPolicy,
}

#[derive(ToSchema)]
struct SetupTwoFactorResponse {
    secret: String,
    otpauth_uri: String,
}

#[derive(ToSchema)]
struct ConfirmTwoFactorRequest {
    totp_code: String,
}

#[derive(ToSchema)]
struct UpdateLoginTwoFactorRequest {
    enabled: bool,
}

#[derive(ToSchema)]
struct ResetTwoFactorRequest {
    code: String,
}

#[derive(ToSchema)]
struct PublicCountryResponse {
    country_code: String,
    country_name: String,
    default_locale: String,
    supported_locales: Vec<String>,
}

#[derive(ToSchema)]
struct PublicCountriesResponse {
    countries: Vec<PublicCountryResponse>,
}

#[derive(ToSchema)]
struct PlatformBrandResponse {
    id: u64,
    name: String,
    platform_name: String,
    logo_url: Option<String>,
    chart_provider: String,
    updated_by: Option<u64>,
    #[schema(format = Int64)]
    created_at: i64,
    #[schema(format = Int64)]
    updated_at: i64,
}

#[derive(ToSchema)]
struct SavePlatformBrandRequest {
    platform_name: String,
    logo_url: Option<String>,
    chart_provider: Option<String>,
    reason: Option<String>,
}

#[derive(ToSchema)]
struct AdminCountryResponse {
    id: u64,
    country_code: String,
    country_name: String,
    remark: String,
    default_locale: String,
    supported_locales: Vec<String>,
    registration_enabled: bool,
    #[schema(pattern = "^(active|disabled)$")]
    status: String,
    sort_order: i32,
    #[schema(format = Int64)]
    created_at: i64,
    #[schema(format = Int64)]
    updated_at: i64,
}

#[derive(ToSchema)]
struct AdminCountriesResponse {
    countries: Vec<AdminCountryResponse>,
}

#[derive(ToSchema)]
struct CreateAdminCountryRequest {
    country_code: String,
    country_name: String,
    remark: String,
    default_locale: String,
    supported_locales: Vec<String>,
    registration_enabled: bool,
    #[schema(pattern = "^(active|disabled)$")]
    status: Option<String>,
    sort_order: Option<i32>,
    reason: Option<String>,
}

#[derive(ToSchema)]
struct UpdateAdminCountryRequest {
    country_name: String,
    remark: String,
    default_locale: String,
    supported_locales: Vec<String>,
    registration_enabled: bool,
    sort_order: Option<i32>,
    reason: Option<String>,
}

#[derive(ToSchema)]
struct UpdateAdminCountryStatusRequest {
    #[schema(pattern = "^(active|disabled)$")]
    status: String,
    reason: Option<String>,
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
struct ResetFundPasswordRequest {
    code: String,
    new_fund_password: String,
}

#[derive(ToSchema)]
struct FundPasswordResponse {
    fund_password_set: bool,
}

#[derive(ToSchema)]
struct CreateWithdrawalRequest {
    asset_symbol: String,
    network: Option<String>,
    address: String,
    amount: String,
    fee: String,
    fund_password: Option<String>,
    totp_code: Option<String>,
}

#[derive(ToSchema)]
struct WithdrawalRequestResponse {
    id: u64,
    status: String,
    security_method: SecurityVerificationMethod,
}

#[derive(ToSchema)]
struct WithdrawFeeTierResponse {
    min_amount: String,
    max_amount: Option<String>,
    fee_rate_percent: String,
}

#[derive(ToSchema)]
struct DepositAssetResponse {
    symbol: String,
    name: String,
    logo_url: Option<String>,
    precision_scale: i32,
    deposit_enabled: bool,
    withdraw_enabled: bool,
    min_deposit_amount: String,
    deposit_fee: String,
    withdraw_fee: String,
    withdraw_fee_tiers: Vec<WithdrawFeeTierResponse>,
}

#[derive(ToSchema)]
struct DepositAssetsResponse {
    assets: Vec<DepositAssetResponse>,
}

#[derive(ToSchema)]
struct DepositAddressRequest {
    asset_symbol: String,
    #[schema(pattern = "^(eth|base|tron|btc|solana)$")]
    network: String,
}

#[derive(ToSchema)]
struct DepositAddressResponse {
    id: u64,
    asset_symbol: String,
    #[schema(pattern = "^(eth|base|tron|btc|solana)$")]
    network: String,
    address: String,
    memo: Option<String>,
    #[schema(format = Int64)]
    assigned_at: i64,
}

#[derive(ToSchema)]
struct UserQuickRechargeConfigResponse {
    enabled: bool,
    currency: String,
    token: String,
    network: String,
    min_amount: String,
    max_amount: Option<String>,
}

#[derive(ToSchema)]
#[schema(rename_all = "snake_case")]
enum QuickRechargeReturnTarget {
    PcApp,
    MacApp,
    IosApp,
    AndroidApp,
    MobileWeb,
    DesktopWeb,
}

#[derive(ToSchema)]
struct CreateQuickRechargeOrderRequest {
    amount: String,
    return_target: Option<QuickRechargeReturnTarget>,
}

#[derive(ToSchema)]
struct QuickRechargeOrderResponse {
    id: u64,
    order_id: String,
    user_id: u64,
    user_email: Option<String>,
    asset_id: u64,
    asset_symbol: String,
    currency: String,
    token: String,
    network: String,
    fiat_amount: String,
    actual_amount: Option<String>,
    provider_trade_id: Option<String>,
    receive_address: Option<String>,
    payment_url: Option<String>,
    return_target: Option<String>,
    redirect_url: Option<String>,
    expiration_time: Option<i64>,
    status: String,
    block_transaction_id: Option<String>,
    #[schema(format = Int64)]
    paid_at: Option<i64>,
    #[schema(format = Int64)]
    created_at: i64,
    #[schema(format = Int64)]
    updated_at: i64,
}

#[derive(ToSchema)]
struct QuickRechargeOrdersResponse {
    orders: Vec<QuickRechargeOrderResponse>,
}

#[derive(ToSchema)]
struct SaveQuickRechargeConfigRequest {
    enabled: bool,
    api_base_url: Option<String>,
    merchant_pid: Option<String>,
    merchant_secret: Option<String>,
    currency: String,
    token: String,
    network: String,
    notify_url: Option<String>,
    redirect_url: Option<String>,
    pc_app_redirect_url: Option<String>,
    mac_app_redirect_url: Option<String>,
    ios_app_redirect_url: Option<String>,
    android_app_redirect_url: Option<String>,
    mobile_web_redirect_url: Option<String>,
    desktop_web_redirect_url: Option<String>,
    min_amount: String,
    max_amount: Option<String>,
    reason: Option<String>,
}

#[derive(ToSchema)]
struct QuickRechargeConfigResponse {
    id: u64,
    name: String,
    provider: String,
    enabled: bool,
    api_base_url: Option<String>,
    merchant_pid: Option<String>,
    merchant_secret_mask: Option<String>,
    merchant_secret_set: bool,
    currency: String,
    token: String,
    network: String,
    notify_url: Option<String>,
    redirect_url: Option<String>,
    pc_app_redirect_url: Option<String>,
    mac_app_redirect_url: Option<String>,
    ios_app_redirect_url: Option<String>,
    android_app_redirect_url: Option<String>,
    mobile_web_redirect_url: Option<String>,
    desktop_web_redirect_url: Option<String>,
    min_amount: String,
    max_amount: Option<String>,
    updated_by: Option<u64>,
    #[schema(format = Int64)]
    created_at: i64,
    #[schema(format = Int64)]
    updated_at: i64,
}

#[derive(ToSchema)]
struct TestQuickRechargeConfigRequest {
    amount: String,
    reason: Option<String>,
}

#[derive(ToSchema)]
struct TestQuickRechargeConfigResponse {
    order_id: String,
    provider_trade_id: String,
    currency: String,
    token: String,
    network: String,
    fiat_amount: String,
    actual_amount: String,
    receive_address: String,
    payment_url: String,
    expiration_time: Option<i64>,
    #[schema(format = Int64)]
    tested_at: i64,
}

#[derive(ToSchema)]
struct DeleteQuickRechargeOrderRequest {
    reason: Option<String>,
}

#[derive(ToSchema)]
struct GmpayNotifyRequest {
    pid: String,
    trade_id: String,
    order_id: String,
    amount: String,
    actual_amount: String,
    receive_address: Option<String>,
    token: String,
    block_transaction_id: Option<String>,
    status: String,
    signature: String,
}

#[derive(ToSchema)]
struct AdminDepositAddressPoolResponse {
    id: u64,
    #[schema(pattern = "^(eth|base|tron|btc|solana)$")]
    network: String,
    address: String,
    asset_symbol: Option<String>,
    asset_symbols: Vec<String>,
    #[schema(pattern = "^(available|assigned|disabled)$")]
    status: String,
    assigned_user_id: Option<u64>,
    assigned_user_email: Option<String>,
    assigned_asset_symbol: Option<String>,
    #[schema(format = Int64)]
    assigned_at: Option<i64>,
    memo: Option<String>,
    remark: Option<String>,
    #[schema(format = Int64)]
    created_at: i64,
    #[schema(format = Int64)]
    updated_at: i64,
}

#[derive(ToSchema)]
struct AdminDepositAddressPoolResponseList {
    addresses: Vec<AdminDepositAddressPoolResponse>,
}

#[derive(ToSchema)]
struct AdminDepositAddressPoolBatchResponse {
    addresses: Vec<AdminDepositAddressPoolResponse>,
}

#[derive(ToSchema)]
struct CreateDepositAddressPoolRequest {
    #[schema(pattern = "^(eth|base|tron|btc|solana)$")]
    network: String,
    address: String,
    asset_symbol: Option<String>,
    asset_symbols: Option<Vec<String>>,
    #[schema(pattern = "^(available|disabled)$")]
    status: Option<String>,
    memo: Option<String>,
    remark: Option<String>,
    reason: Option<String>,
}

#[derive(ToSchema)]
struct CreateDepositAddressPoolEntryRequest {
    address: String,
    memo: Option<String>,
    remark: Option<String>,
}

#[derive(ToSchema)]
struct CreateDepositAddressPoolBatchRequest {
    #[schema(pattern = "^(eth|base|tron|btc|solana)$")]
    network: String,
    asset_symbol: Option<String>,
    asset_symbols: Option<Vec<String>>,
    #[schema(pattern = "^(available|disabled)$")]
    status: Option<String>,
    entries: Vec<CreateDepositAddressPoolEntryRequest>,
    reason: Option<String>,
}

#[derive(ToSchema)]
struct UpdateDepositAddressPoolRequest {
    #[schema(pattern = "^(eth|base|tron|btc|solana)$")]
    network: String,
    address: String,
    asset_symbol: Option<String>,
    asset_symbols: Option<Vec<String>>,
    #[schema(pattern = "^(available|disabled)$")]
    status: String,
    memo: Option<String>,
    remark: Option<String>,
    reason: Option<String>,
}

#[derive(ToSchema)]
struct ReclaimDepositAddressPoolRequest {
    reason: Option<String>,
}

#[derive(ToSchema)]
struct UpdateSecurityPolicyRequest {
    login_2fa_mode: LoginTwoFactorMode,
    registration_invite_required: bool,
    username_login_enabled: bool,
    payment_policies: PaymentPolicies,
    third_party_bindings: ThirdPartyBindingPolicy,
    reason: Option<String>,
}

#[derive(ToSchema)]
struct ResetUserTwoFactorRequest {
    reason: Option<String>,
}

#[derive(ToSchema)]
struct AdminUserTwoFactorResetResponse {
    user_id: u64,
    totp_enabled: bool,
    login_2fa_enabled: bool,
}

#[derive(ToSchema)]
struct SaveSmtpConfigRequest {
    name: Option<String>,
    host: String,
    port: u16,
    #[schema(pattern = "^(none|starttls|tls)$")]
    security: String,
    username: Option<String>,
    #[schema(nullable = true)]
    password: Option<String>,
    from_email: String,
    from_name: Option<String>,
    verification_code_template_html: Option<String>,
    verification_code_templates: Option<Vec<VerificationCodeTemplate>>,
    enabled: bool,
    priority: Option<u32>,
    reason: Option<String>,
}

#[derive(ToSchema)]
struct VerificationCodeTemplate {
    key: String,
    name: String,
    purpose: Option<String>,
    html: String,
    enabled: bool,
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
    verification_code_template_html: Option<String>,
    verification_code_templates: Vec<VerificationCodeTemplate>,
    enabled: bool,
    priority: u32,
}

#[derive(ToSchema)]
struct SmtpDeliverySettingsResponse {
    #[schema(pattern = "^(priority|round_robin)$")]
    strategy: String,
}

#[derive(ToSchema)]
struct SmtpConfigListResponse {
    configs: Vec<SmtpConfigResponse>,
    delivery_settings: SmtpDeliverySettingsResponse,
}

#[derive(ToSchema)]
struct SaveSmtpDeliverySettingsRequest {
    #[schema(pattern = "^(priority|round_robin)$")]
    strategy: String,
    reason: Option<String>,
}

#[derive(ToSchema)]
struct SendSmtpTestRequest {
    recipient: String,
    config_id: Option<u64>,
    reason: Option<String>,
}

#[derive(ToSchema)]
struct SendSmtpTestResponse {
    sent: bool,
    recipient: String,
    config_id: u64,
    config_name: String,
}

#[derive(ToSchema)]
struct AdminAgentResponse {
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
struct AdminAgentsResponse {
    agents: Vec<AdminAgentResponse>,
}

#[derive(ToSchema)]
struct AdminAgentUserResponse {
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
struct AdminAgentUsersResponse {
    users: Vec<AdminAgentUserResponse>,
}

#[derive(ToSchema)]
struct CreateAdminAgentRequest {
    user_id: u64,
    parent_agent_id: Option<u64>,
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
    payout_asset_id: Option<u64>,
    commission_rate: String,
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
struct AdminAgentCommissionRulesResponse {
    rules: Vec<AdminAgentCommissionRuleResponse>,
}

#[derive(ToSchema)]
struct CreateAdminAgentCommissionRuleRequest {
    agent_id: u64,
    #[schema(pattern = "^(convert|prediction|spot|margin|seconds_contract)$")]
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
    banner_url: Option<String>,
    small_logo_url: Option<String>,
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
struct AgentSubAgentResponse {
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
struct AgentSubAgentsResponse {
    agents: Vec<AgentSubAgentResponse>,
}

#[derive(ToSchema)]
struct AgentTeamTreeNodeResponse {
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
struct AgentTeamTreeResponse {
    root_agent_id: u64,
    agents: Vec<AgentSubAgentResponse>,
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

#[utoipa::path(
    get,
    path = "/api/v1/countries",
    tag = "countries",
    summary = "查询可注册国家和默认语言",
    responses(
        (status = 200, description = "查询成功", body = PublicCountriesResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_public_countries() {}

#[utoipa::path(
    get,
    path = "/api/v1/platform/brand",
    tag = "platform",
    summary = "查询 PC 端品牌与 K 线图配置",
    responses(
        (status = 200, description = "查询成功", body = PlatformBrandResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_public_platform_brand() {}

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
    path = "/api/v1/wallet/deposit-assets",
    tag = "wallet",
    summary = "查询当前支持普通充值的资产",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = DepositAssetsResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_deposit_assets() {}

#[utoipa::path(
    get,
    path = "/api/v1/wallet/withdraw-assets",
    tag = "wallet",
    summary = "查询当前支持提现的资产",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = DepositAssetsResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_withdraw_assets() {}

#[utoipa::path(
    post,
    path = "/api/v1/wallet/deposit-address",
    tag = "wallet",
    summary = "从地址池获取或申请充值地址",
    request_body = DepositAddressRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "申请成功，若用户已有绑定则返回原地址", body = DepositAddressResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "资产不存在或地址池无可用地址", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_or_assign_deposit_address() {}

#[utoipa::path(
    get,
    path = "/api/v1/wallet/quick-recharge/config",
    tag = "wallet",
    summary = "查询用户端快速充值配置",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = UserQuickRechargeConfigResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_user_quick_recharge_config() {}

#[utoipa::path(
    post,
    path = "/api/v1/wallet/quick-recharge/orders",
    tag = "wallet",
    summary = "创建 GMPay/Epusdt 快速充值订单",
    request_body = CreateQuickRechargeOrderRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "创建成功并返回 GMPay 收银台链接", body = QuickRechargeOrderResponse),
        (status = 400, description = "参数错误、配置未启用或金额超出限制", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 502, description = "GMPay 创建订单失败", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn create_user_quick_recharge_order() {}

#[utoipa::path(
    get,
    path = "/api/v1/wallet/quick-recharge/orders",
    tag = "wallet",
    summary = "查询当前用户快速充值订单",
    params(
        ("status" = Option<String>, Query, description = "订单状态"),
        ("limit" = Option<u32>, Query, description = "分页数量")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = QuickRechargeOrdersResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_user_quick_recharge_orders() {}

#[utoipa::path(
    post,
    path = "/api/v1/payments/gmpay/notify",
    tag = "wallet",
    summary = "GMPay/Epusdt 快速充值异步回调",
    request_body = GmpayNotifyRequest,
    responses(
        (status = 200, description = "回调验签成功并返回 ok"),
        (status = 400, description = "验签失败或回调参数无效", body = ErrorResponse),
        (status = 404, description = "订单不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn gmpay_notify() {}

#[utoipa::path(
    post,
    path = "/api/v1/wallet/withdrawals",
    tag = "wallet",
    summary = "创建提现申请并按后台策略完成安全校验",
    request_body = CreateWithdrawalRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "创建成功", body = WithdrawalRequestResponse),
        (status = 400, description = "参数错误或安全校验缺失", body = ErrorResponse),
        (status = 401, description = "未登录或资金密码错误", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn create_withdrawal_request() {}

#[utoipa::path(
    get,
    path = "/admin/api/v1/quick-recharge/config",
    tag = "admin-wallet",
    summary = "查询后台快速充值配置",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = QuickRechargeConfigResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_admin_quick_recharge_config() {}

#[utoipa::path(
    patch,
    path = "/admin/api/v1/quick-recharge/config",
    tag = "admin-wallet",
    summary = "保存后台快速充值配置",
    request_body = SaveQuickRechargeConfigRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "保存成功", body = QuickRechargeConfigResponse),
        (status = 400, description = "参数错误或缺少审计原因", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn save_admin_quick_recharge_config() {}

#[utoipa::path(
    post,
    path = "/admin/api/v1/quick-recharge/config/test",
    tag = "admin-wallet",
    summary = "测试后台快速充值配置",
    request_body = TestQuickRechargeConfigRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "测试成功并返回 GMPay 收银台信息", body = TestQuickRechargeConfigResponse),
        (status = 400, description = "参数错误、配置缺失或缺少审计原因", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 502, description = "GMPay 创建测试订单失败", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn test_admin_quick_recharge_config() {}

#[utoipa::path(
    get,
    path = "/admin/api/v1/quick-recharge/orders",
    tag = "admin-wallet",
    summary = "查询快速充值订单",
    params(
        ("user_id" = Option<u64>, Query, description = "用户 ID"),
        ("email" = Option<String>, Query, description = "用户邮箱"),
        ("status" = Option<String>, Query, description = "订单状态"),
        ("order_id" = Option<String>, Query, description = "平台订单号"),
        ("provider_trade_id" = Option<String>, Query, description = "GMPay 交易号"),
        ("limit" = Option<u32>, Query, description = "分页数量")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = QuickRechargeOrdersResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_admin_quick_recharge_orders() {}

#[utoipa::path(
    delete,
    path = "/admin/api/v1/quick-recharge/orders/{order_id}",
    tag = "admin-wallet",
    summary = "删除未入账的快速充值订单",
    params(("order_id" = String, Path, description = "平台订单号")),
    request_body = DeleteQuickRechargeOrderRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 204, description = "删除成功"),
        (status = 400, description = "缺少审计原因", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "订单不存在", body = ErrorResponse),
        (status = 409, description = "订单已入账或存在钱包流水", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn delete_admin_quick_recharge_order() {}

#[utoipa::path(
    get,
    path = "/admin/api/v1/deposit-address-pool",
    tag = "admin-wallet",
    summary = "查询充值地址池",
    params(
        ("network" = Option<String>, Query, description = "网络：eth/base/tron/btc/solana"),
        ("status" = Option<String>, Query, description = "状态：available/assigned/disabled"),
        ("asset_symbol" = Option<String>, Query, description = "限定资产或已分配资产符号"),
        ("assigned_user_id" = Option<u64>, Query, description = "绑定用户 ID"),
        ("email" = Option<String>, Query, description = "绑定用户邮箱"),
        ("address" = Option<String>, Query, description = "地址模糊查询"),
        ("limit" = Option<u32>, Query, description = "分页数量")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AdminDepositAddressPoolResponseList),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_admin_deposit_address_pool() {}

#[utoipa::path(
    post,
    path = "/admin/api/v1/deposit-address-pool",
    tag = "admin-wallet",
    summary = "新增充值地址池地址",
    request_body = CreateDepositAddressPoolRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "新增成功", body = AdminDepositAddressPoolResponse),
        (status = 400, description = "参数错误或缺少审计原因", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "限定资产不存在", body = ErrorResponse),
        (status = 409, description = "同网络地址已存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn create_admin_deposit_address_pool() {}

#[utoipa::path(
    post,
    path = "/admin/api/v1/deposit-address-pool/batch",
    tag = "admin-wallet",
    summary = "批量新增充值地址池地址",
    request_body = CreateDepositAddressPoolBatchRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "新增成功", body = AdminDepositAddressPoolBatchResponse),
        (status = 400, description = "参数错误或缺少审计原因", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "限定资产不存在", body = ErrorResponse),
        (status = 409, description = "同网络地址已存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn create_admin_deposit_address_pool_batch() {}

#[utoipa::path(
    get,
    path = "/admin/api/v1/deposit-address-pool/{id}",
    tag = "admin-wallet",
    summary = "查询充值地址池详情",
    params(("id" = u64, Path, description = "地址池 ID")),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AdminDepositAddressPoolResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "地址不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_admin_deposit_address_pool() {}

#[utoipa::path(
    patch,
    path = "/admin/api/v1/deposit-address-pool/{id}",
    tag = "admin-wallet",
    summary = "修改未分配的充值地址池地址",
    params(("id" = u64, Path, description = "地址池 ID")),
    request_body = UpdateDepositAddressPoolRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "修改成功", body = AdminDepositAddressPoolResponse),
        (status = 400, description = "参数错误、缺少审计原因或地址已分配", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "地址或限定资产不存在", body = ErrorResponse),
        (status = 409, description = "同网络地址已存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn update_admin_deposit_address_pool() {}

#[utoipa::path(
    post,
    path = "/admin/api/v1/deposit-address-pool/{id}/reclaim",
    tag = "admin-wallet",
    summary = "回收已分配充值地址",
    params(("id" = u64, Path, description = "地址池 ID")),
    request_body = ReclaimDepositAddressPoolRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "回收成功", body = AdminDepositAddressPoolResponse),
        (status = 400, description = "缺少审计原因或地址未分配", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "地址不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn reclaim_admin_deposit_address_pool() {}

#[utoipa::path(
    get,
    path = "/admin/api/v1/countries",
    tag = "admin-countries",
    summary = "查询后台国家配置",
    params(
        ("country_code" = Option<String>, Query, description = "国家或地区代码"),
        ("status" = Option<String>, Query, description = "配置状态"),
        ("registration_enabled" = Option<bool>, Query, description = "是否开放注册"),
        ("limit" = Option<u32>, Query, description = "分页数量"),
        ("offset" = Option<u32>, Query, description = "分页偏移")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AdminCountriesResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_admin_countries() {}

#[utoipa::path(
    post,
    path = "/admin/api/v1/countries",
    tag = "admin-countries",
    summary = "创建后台国家配置",
    request_body = CreateAdminCountryRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "创建成功", body = AdminCountryResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 409, description = "国家代码已存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn create_admin_country() {}

#[utoipa::path(
    patch,
    path = "/admin/api/v1/countries/{id}",
    tag = "admin-countries",
    summary = "更新后台国家配置",
    params(("id" = u64, Path, description = "国家配置 ID")),
    request_body = UpdateAdminCountryRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "更新成功", body = AdminCountryResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "国家配置不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn update_admin_country() {}

#[utoipa::path(
    patch,
    path = "/admin/api/v1/countries/{id}/status",
    tag = "admin-countries",
    summary = "更新后台国家配置状态",
    params(("id" = u64, Path, description = "国家配置 ID")),
    request_body = UpdateAdminCountryStatusRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "更新成功", body = AdminCountryResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "国家配置不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn update_admin_country_status() {}

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
        ("limit" = Option<u32>, Query, description = "返回数量")
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
    get,
    path = "/admin/api/v1/smtp/configs",
    tag = "admin-smtp",
    summary = "查询 SMTP 配置列表与发信策略",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = SmtpConfigListResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_smtp_configs() {}

#[utoipa::path(
    post,
    path = "/admin/api/v1/smtp/configs",
    tag = "admin-smtp",
    summary = "新增 SMTP 配置",
    request_body = SaveSmtpConfigRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "新增成功", body = SmtpConfigResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn create_smtp_config() {}

#[utoipa::path(
    patch,
    path = "/admin/api/v1/smtp/configs/{id}",
    tag = "admin-smtp",
    summary = "更新 SMTP 配置",
    params(("id" = u64, Path, description = "配置 ID")),
    request_body = SaveSmtpConfigRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "更新成功", body = SmtpConfigResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "配置不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn update_smtp_config() {}

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
    patch,
    path = "/admin/api/v1/smtp/delivery-settings",
    tag = "admin-smtp",
    summary = "保存 SMTP 发信策略",
    request_body = SaveSmtpDeliverySettingsRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "保存成功", body = SmtpDeliverySettingsResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn save_smtp_delivery_settings() {}

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

#[utoipa::path(
    get,
    path = "/admin/api/v1/platform/brand",
    tag = "admin-platform",
    summary = "查询 PC 品牌与 K 线图配置",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = PlatformBrandResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_admin_platform_brand() {}

#[utoipa::path(
    patch,
    path = "/admin/api/v1/platform/brand",
    tag = "admin-platform",
    summary = "保存 PC 品牌与 K 线图配置",
    request_body = SavePlatformBrandRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "保存成功", body = PlatformBrandResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn save_admin_platform_brand() {}
