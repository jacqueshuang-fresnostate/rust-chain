//! OpenAPI 文档聚合层：集中声明对外契约，只描述公开接口，内部数据行、密文字段与测试辅助类型一律不进 schema。
//! 本文件里的类型和函数都不参与真实请求处理，路径条目仅作为文档骨架存在，具体逻辑由各业务模块的处理器实现。
//! 因此路径、请求体与响应体必须与实际路由手工保持一致，改接口时若漏改这里，文档会与线上行为悄悄脱节。
//! 子模块按业务域拆分：认证、用户安全、钱包、快捷充值、代理、代理门户、新闻与系统配置，各自持有对应的 DTO 与路径。
//! 需要登录的接口统一声明 bearer 令牌安全要求，未声明的即为公开接口，可以不带令牌直接访问。

#![allow(dead_code)]

use axum::Router;
use utoipa::{
    Modify, OpenApi, ToSchema,
    openapi::security::{Http, HttpAuthScheme, SecurityScheme},
};
use utoipa_swagger_ui::SwaggerUi;

use crate::{HealthResponse, error::ErrorResponse, state::AppState};

mod agent_portal;
mod agents;
mod auth;
mod news;
mod quick_recharge;
mod system_config;
mod user_security;
mod wallet;

use self::agent_portal::*;
use self::agents::*;
use self::auth::*;
use self::news::*;
use self::quick_recharge::*;
use self::system_config::*;
use self::user_security::*;
use self::wallet::*;

/// 构建 Swagger UI 与 OpenAPI JSON 的只读文档路由，供联调和前端生成客户端时直接访问。
/// 同一份文档被挂载在两处：根路径下的 `/docs` 与 `/openapi.json`，以及带 API 前缀的 `/api/docs` 与 `/api/openapi.json`。
/// 保留两套地址是为了兼容只把 `/api` 前缀转发给本服务的网关配置，两者内容完全相同，不存在版本差异。
/// 文档在每次请求时由派生实现生成，不做鉴权也不读取共享状态，因此生产环境需要在网关侧决定是否对外开放。
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
        get_admin_login_config,
        send_register_email_code,
        user_register,
        user_login,
        user_login_two_factor,
        user_login_two_factor_setup,
        user_login_two_factor_setup_confirm,
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
        list_user_withdrawals,
        list_admin_wallet_withdrawals,
        approve_admin_wallet_withdrawal,
        reject_admin_wallet_withdrawal,
        broadcast_admin_wallet_withdrawal,
        confirm_admin_wallet_withdrawal,
        fail_admin_wallet_withdrawal,
        list_admin_wallet_deposits,
        observe_admin_wallet_deposit,
        reverse_admin_wallet_deposit,
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
        update_admin_agent_commission_statuses,
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
        LoginTwoFactorSetupRequest,
        LoginTwoFactorSetupConfirmRequest,
        LoginTwoFactorResetCodeRequest,
        LoginTwoFactorResetRequest,
        LoginTwoFactorChallengeResponse,
        LoginTwoFactorSetupChallengeResponse,
        LoginTwoFactorSetupResponse,
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
        WalletWithdrawalResponse,
        WalletWithdrawalsResponse,
        ReviewWithdrawalRequest,
        BroadcastWithdrawalRequest,
        ConfirmWithdrawalRequest,
        FailWithdrawalRequest,
        ObserveDepositRequest,
        ReverseDepositRequest,
        WalletDepositEventResponse,
        WalletDepositsResponse,
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
        BatchUpdateAdminAgentCommissionStatusRequest,
        AdminAgentCommissionBatchStatusItemResponse,
        AdminAgentCommissionBatchStatusResponse,
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
        AgentDashboardAssetSummaryResponse,
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
    /// 在生成的文档中补写名为 `bearerAuth` 的 HTTP Bearer 安全方案，让各路径的安全声明能够解析到具体定义。
    /// 组件区缺失时先补一个空的再写入，避免只声明了引用却没有对应方案而使 Swagger UI 无法弹出授权输入框。
    /// 这里只登记认证方式，不设置默认全局要求，因此接口是否需要令牌仍由各自路径上的安全声明决定。
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearerAuth",
            SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
        );
    }
}
