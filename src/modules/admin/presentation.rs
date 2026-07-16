//! admin bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务逻辑逐步迁入。

use crate::modules::wallet::WithdrawFeeTier;
use crate::{
    architecture::PresentationLayer,
    infra::email::VerificationCodeTemplate,
    modules::security::{LoginTwoFactorMode, PaymentPolicies, ThirdPartyBindingPolicy},
    time::{option_unix_millis, unix_millis},
    workers::market_feed::MarketFeedRuntimeStatus,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use sqlx::types::Json as SqlxJson;

#[derive(Debug, Deserialize)]
pub(crate) struct AdminCountriesQuery {
    pub(crate) country_code: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) registration_enabled: Option<bool>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminCountriesQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateAdminCountryRequest {
    pub(crate) country_code: String,
    pub(crate) country_name: String,
    pub(crate) remark: String,
    pub(crate) default_locale: String,
    pub(crate) supported_locales: Vec<String>,
    pub(crate) registration_enabled: bool,
    pub(crate) status: Option<String>,
    pub(crate) sort_order: Option<i32>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateAdminCountryRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateAdminCountryRequest {
    pub(crate) country_name: String,
    pub(crate) remark: String,
    pub(crate) default_locale: String,
    pub(crate) supported_locales: Vec<String>,
    pub(crate) registration_enabled: bool,
    pub(crate) sort_order: Option<i32>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateAdminCountryRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateAdminCountryStatusRequest {
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateAdminCountryStatusRequest {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminCountryResponse {
    pub(crate) id: u64,
    pub(crate) country_code: String,
    pub(crate) country_name: String,
    pub(crate) remark: String,
    pub(crate) default_locale: String,
    pub(crate) supported_locales: SqlxJson<Vec<String>>,
    pub(crate) registration_enabled: bool,
    pub(crate) status: String,
    pub(crate) sort_order: i32,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

impl PresentationLayer for AdminCountryResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminCountriesResponse {
    pub(crate) countries: Vec<AdminCountryResponse>,
}

impl PresentationLayer for AdminCountriesResponse {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminNewsQuery {
    pub(crate) status: Option<String>,
    pub(crate) category: Option<String>,
    pub(crate) country_code: Option<String>,
    pub(crate) locale: Option<String>,
    pub(crate) q: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminNewsQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateAdminNewsItemRequest {
    pub(crate) title: String,
    pub(crate) banner_url: Option<String>,
    pub(crate) small_logo_url: Option<String>,
    pub(crate) category: String,
    pub(crate) status: Option<String>,
    pub(crate) country_code: Option<String>,
    pub(crate) default_locale: String,
    pub(crate) content_json: Value,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateAdminNewsItemRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateAdminNewsItemRequest {
    pub(crate) title: String,
    pub(crate) banner_url: Option<String>,
    pub(crate) small_logo_url: Option<String>,
    pub(crate) category: String,
    pub(crate) country_code: Option<String>,
    pub(crate) default_locale: String,
    pub(crate) content_json: Value,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateAdminNewsItemRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateAdminNewsStatusRequest {
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateAdminNewsStatusRequest {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminNewsItemResponse {
    pub(crate) id: u64,
    pub(crate) title: String,
    pub(crate) banner_url: Option<String>,
    pub(crate) small_logo_url: Option<String>,
    pub(crate) category: String,
    pub(crate) status: String,
    pub(crate) country_code: Option<String>,
    pub(crate) default_locale: String,
    pub(crate) content_json: SqlxJson<Value>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) published_at: Option<DateTime<Utc>>,
    pub(crate) created_by_admin_id: Option<u64>,
    pub(crate) updated_by_admin_id: Option<u64>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

impl PresentationLayer for AdminNewsItemResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminNewsItemsResponse {
    pub(crate) news: Vec<AdminNewsItemResponse>,
}

impl PresentationLayer for AdminNewsItemsResponse {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminAssetQuery {
    pub(crate) symbol: Option<String>,
    pub(crate) asset_type: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
}

impl PresentationLayer for AdminAssetQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateAssetRequest {
    pub(crate) symbol: String,
    pub(crate) name: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) precision_scale: i32,
    pub(crate) asset_type: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) deposit_enabled: Option<bool>,
    pub(crate) withdraw_enabled: Option<bool>,
    pub(crate) min_deposit_amount: Option<BigDecimal>,
    pub(crate) deposit_fee: Option<BigDecimal>,
    pub(crate) withdraw_fee: Option<BigDecimal>,
    pub(crate) withdraw_fee_tiers: Option<Vec<WithdrawFeeTier>>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateAssetRequest {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateAssetRequest {
    pub(crate) name: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) precision_scale: i32,
    pub(crate) asset_type: String,
    pub(crate) status: String,
    pub(crate) deposit_enabled: Option<bool>,
    pub(crate) withdraw_enabled: Option<bool>,
    pub(crate) min_deposit_amount: Option<BigDecimal>,
    pub(crate) deposit_fee: Option<BigDecimal>,
    pub(crate) withdraw_fee: Option<BigDecimal>,
    pub(crate) withdraw_fee_tiers: Option<Vec<WithdrawFeeTier>>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateAssetRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct DeleteAssetRequest {
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for DeleteAssetRequest {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminAssetResponse {
    pub(crate) id: u64,
    pub(crate) symbol: String,
    pub(crate) name: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) precision_scale: i32,
    pub(crate) asset_type: String,
    pub(crate) status: String,
    pub(crate) deposit_enabled: bool,
    pub(crate) withdraw_enabled: bool,
    pub(crate) min_deposit_amount: BigDecimal,
    pub(crate) deposit_fee: BigDecimal,
    pub(crate) withdraw_fee: BigDecimal,
    pub(crate) withdraw_fee_tiers: SqlxJson<Vec<WithdrawFeeTier>>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for AdminAssetResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminAssetsResponse {
    pub(crate) assets: Vec<AdminAssetResponse>,
}

impl PresentationLayer for AdminAssetsResponse {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminUserQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) include_internal: Option<bool>,
    pub(crate) limit: Option<u32>,
}

impl PresentationLayer for AdminUserQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateAdminUserRequest {
    pub(crate) email: Option<String>,
    pub(crate) phone: Option<String>,
    pub(crate) password: String,
    pub(crate) status: Option<String>,
    pub(crate) kyc_level: Option<i32>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateAdminUserRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminUserRechargeRequest {
    pub(crate) asset_id: u64,
    pub(crate) amount: BigDecimal,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for AdminUserRechargeRequest {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminUserResponse {
    pub(crate) id: u64,
    pub(crate) email: Option<String>,
    pub(crate) phone: Option<String>,
    pub(crate) invite_code: Option<String>,
    pub(crate) status: String,
    pub(crate) kyc_level: i32,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

impl PresentationLayer for AdminUserResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminUserRechargeResponse {
    pub(crate) recharge_id: String,
    pub(crate) user_id: u64,
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) amount: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) available: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) frozen: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) locked: BigDecimal,
}

impl PresentationLayer for AdminUserRechargeResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminUsersResponse {
    pub(crate) users: Vec<AdminUserResponse>,
}

impl PresentationLayer for AdminUsersResponse {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminWalletAccountQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) asset_id: Option<u64>,
    pub(crate) include_empty: Option<bool>,
    pub(crate) include_internal: Option<bool>,
    pub(crate) limit: Option<u32>,
}

impl PresentationLayer for AdminWalletAccountQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminWalletLedgerQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) asset_id: Option<u64>,
    pub(crate) change_type: Option<String>,
    pub(crate) ref_type: Option<String>,
    pub(crate) include_internal: Option<bool>,
    pub(crate) limit: Option<u32>,
}

impl PresentationLayer for AdminWalletLedgerQuery {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminWalletAccountResponse {
    pub(crate) id: Option<u64>,
    pub(crate) user_id: u64,
    pub(crate) user_email: Option<String>,
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) available: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) frozen: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) locked: BigDecimal,
    pub(crate) account_exists: bool,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

impl PresentationLayer for AdminWalletAccountResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminWalletLedgerResponse {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    pub(crate) user_email: String,
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    pub(crate) change_type: String,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) amount: BigDecimal,
    pub(crate) balance_type: String,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) balance_after: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) available_after: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) frozen_after: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) locked_after: BigDecimal,
    pub(crate) ref_type: String,
    pub(crate) ref_id: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for AdminWalletLedgerResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminWalletAccountsResponse {
    pub(crate) accounts: Vec<AdminWalletAccountResponse>,
}

impl PresentationLayer for AdminWalletAccountsResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminWalletLedgerResponseList {
    pub(crate) ledger: Vec<AdminWalletLedgerResponse>,
}

impl PresentationLayer for AdminWalletLedgerResponseList {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminKycSubmissionQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
}

impl PresentationLayer for AdminKycSubmissionQuery {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateSecurityPolicyRequest {
    pub(crate) login_2fa_mode: LoginTwoFactorMode,
    #[serde(default)]
    pub(crate) registration_invite_required: bool,
    #[serde(default)]
    pub(crate) username_login_enabled: bool,
    pub(crate) payment_policies: PaymentPolicies,
    #[serde(default)]
    pub(crate) third_party_bindings: ThirdPartyBindingPolicy,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateSecurityPolicyRequest {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResetUserTwoFactorRequest {
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for ResetUserTwoFactorRequest {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminUserTwoFactorResetResponse {
    pub(crate) user_id: u64,
    pub(crate) totp_enabled: bool,
    pub(crate) login_2fa_enabled: bool,
}

impl PresentationLayer for AdminUserTwoFactorResetResponse {}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateAgentRequest {
    pub(crate) user_id: u64,
    pub(crate) parent_agent_id: Option<u64>,
    pub(crate) agent_code: String,
    pub(crate) admin_username: String,
    pub(crate) admin_password: Option<String>,
    pub(crate) admin_password_hash: Option<String>,
    pub(crate) level: Option<i32>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateAgentRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateAgentStatusRequest {
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateAgentStatusRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct AssignUserAgentRequest {
    pub(crate) agent_id: u64,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for AssignUserAgentRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminAgentUsersQuery {
    /// 用于限制返回团队成员数量，保持接口分页行为一致。
    pub(crate) limit: Option<u32>,
}

impl PresentationLayer for AdminAgentUsersQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminAgentQuery {
    pub(crate) agent_id: Option<u64>,
    pub(crate) user_id: Option<u64>,
    pub(crate) parent_agent_id: Option<u64>,
    pub(crate) root_agent_id: Option<u64>,
    pub(crate) level: Option<i32>,
    pub(crate) agent_code: Option<String>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminAgentQuery {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminAgentResponse {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    pub(crate) email: Option<String>,
    pub(crate) parent_agent_id: Option<u64>,
    pub(crate) parent_agent_code: Option<String>,
    pub(crate) root_agent_id: u64,
    pub(crate) root_agent_code: String,
    pub(crate) agent_code: String,
    pub(crate) level: i32,
    pub(crate) path: String,
    pub(crate) status: String,
    pub(crate) direct_user_count: i64,
    pub(crate) team_user_count: i64,
    pub(crate) child_agent_count: i64,
    pub(crate) admin_user_id: Option<u64>,
    pub(crate) admin_username: Option<String>,
    pub(crate) admin_status: Option<String>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for AdminAgentResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminAgentUserResponse {
    pub(crate) user_id: u64,
    pub(crate) email: Option<String>,
    pub(crate) phone: Option<String>,
    pub(crate) status: String,
    pub(crate) kyc_level: i32,
    pub(crate) owner_agent_id: u64,
    // 兼容旧客户端：该字段历史上表示直属归属代理，而不是总代理。
    pub(crate) root_agent_id: u64,
    pub(crate) owner_agent_code: String,
    pub(crate) owner_agent_level: i32,
    pub(crate) direct_inviter_id: Option<u64>,
    pub(crate) direct_inviter_type: Option<String>,
    pub(crate) depth: i32,
    pub(crate) path: String,
    #[serde(with = "unix_millis")]
    pub(crate) referred_at: DateTime<Utc>,
}

impl PresentationLayer for AdminAgentUserResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminUserReferralResponse {
    pub(crate) user_id: u64,
    pub(crate) direct_inviter_id: Option<u64>,
    pub(crate) direct_inviter_type: Option<String>,
    pub(crate) root_agent_id: Option<u64>,
    pub(crate) depth: i32,
    pub(crate) path: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for AdminUserReferralResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminAgentsResponse {
    pub(crate) agents: Vec<AdminAgentResponse>,
}

impl PresentationLayer for AdminAgentsResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminAgentUsersResponse {
    pub(crate) users: Vec<AdminAgentUserResponse>,
}

impl PresentationLayer for AdminAgentUsersResponse {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateAgentCommissionStatusRequest {
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateAgentCommissionStatusRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateAgentCommissionRuleRequest {
    pub(crate) agent_id: u64,
    pub(crate) product_type: String,
    pub(crate) commission_rate: BigDecimal,
    pub(crate) status: Option<String>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateAgentCommissionRuleRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateAgentCommissionRuleRequest {
    pub(crate) commission_rate: Option<BigDecimal>,
    pub(crate) status: Option<String>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateAgentCommissionRuleRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminAgentCommissionQuery {
    pub(crate) agent_id: Option<u64>,
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
}

impl PresentationLayer for AdminAgentCommissionQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminAgentCommissionRuleQuery {
    pub(crate) agent_id: Option<u64>,
    pub(crate) product_type: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminAgentCommissionRuleQuery {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminAgentCommissionResponse {
    pub(crate) id: u64,
    pub(crate) agent_id: u64,
    pub(crate) user_id: u64,
    pub(crate) source_type: String,
    pub(crate) source_id: String,
    pub(crate) source_amount: BigDecimal,
    pub(crate) payout_asset_id: Option<u64>,
    pub(crate) commission_rate: BigDecimal,
    pub(crate) commission_amount: BigDecimal,
    pub(crate) status: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for AdminAgentCommissionResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminAgentCommissionRuleResponse {
    pub(crate) id: u64,
    pub(crate) agent_id: u64,
    pub(crate) product_type: String,
    pub(crate) commission_rate: BigDecimal,
    pub(crate) status: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

impl PresentationLayer for AdminAgentCommissionRuleResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminAgentCommissionsResponse {
    pub(crate) commissions: Vec<AdminAgentCommissionResponse>,
}

impl PresentationLayer for AdminAgentCommissionsResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminAgentCommissionRulesResponse {
    pub(crate) rules: Vec<AdminAgentCommissionRuleResponse>,
}

impl PresentationLayer for AdminAgentCommissionRulesResponse {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminRiskRuleQuery {
    pub(crate) rule_type: Option<String>,
    pub(crate) target_type: Option<String>,
    pub(crate) enabled: Option<bool>,
    pub(crate) limit: Option<u32>,
}

impl PresentationLayer for AdminRiskRuleQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminRiskEventQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) decision: Option<String>,
    pub(crate) risk_level: Option<String>,
    pub(crate) limit: Option<u32>,
}

impl PresentationLayer for AdminRiskEventQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateRiskRuleRequest {
    pub(crate) rule_type: String,
    pub(crate) target_type: String,
    pub(crate) target_id: Option<String>,
    pub(crate) config_json: Value,
    pub(crate) enabled: Option<bool>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateRiskRuleRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateRiskRuleStatusRequest {
    pub(crate) enabled: bool,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateRiskRuleStatusRequest {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct RiskRuleResponse {
    pub(crate) id: u64,
    pub(crate) rule_type: String,
    pub(crate) target_type: String,
    pub(crate) target_id: Option<String>,
    pub(crate) config_json: SqlxJson<Value>,
    pub(crate) enabled: bool,
    pub(crate) created_by: Option<u64>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

impl PresentationLayer for RiskRuleResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct RiskEventResponse {
    pub(crate) id: u64,
    pub(crate) user_id: Option<u64>,
    pub(crate) actor_type: String,
    pub(crate) actor_id: Option<u64>,
    pub(crate) event_type: String,
    pub(crate) risk_level: String,
    pub(crate) decision: String,
    pub(crate) reason: Option<String>,
    pub(crate) payload_json: SqlxJson<Value>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for RiskEventResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct RiskRulesResponse {
    pub(crate) rules: Vec<RiskRuleResponse>,
}

impl PresentationLayer for RiskRulesResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct RiskEventsResponse {
    pub(crate) events: Vec<RiskEventResponse>,
}

impl PresentationLayer for RiskEventsResponse {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminDepositNetworkConfigQuery {
    pub(crate) network: Option<String>,
    pub(crate) address_group_code: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) asset_symbol: Option<String>,
    pub(crate) limit: Option<u32>,
}

impl PresentationLayer for AdminDepositNetworkConfigQuery {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateDepositNetworkConfigRequest {
    pub(crate) network: String,
    pub(crate) display_name: String,
    pub(crate) address_group_code: String,
    pub(crate) address_group_name: Option<String>,
    pub(crate) asset_symbols: Option<Vec<String>>,
    pub(crate) status: Option<String>,
    pub(crate) sort_order: Option<i32>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateDepositNetworkConfigRequest {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateDepositNetworkConfigRequest {
    pub(crate) network: String,
    pub(crate) display_name: String,
    pub(crate) address_group_code: String,
    pub(crate) address_group_name: Option<String>,
    pub(crate) asset_symbols: Option<Vec<String>>,
    pub(crate) status: String,
    pub(crate) sort_order: i32,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateDepositNetworkConfigRequest {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminDepositNetworkConfigResponse {
    pub(crate) id: u64,
    pub(crate) network: String,
    pub(crate) display_name: String,
    pub(crate) address_group_code: String,
    pub(crate) address_group_name: Option<String>,
    pub(crate) asset_symbols: SqlxJson<Vec<String>>,
    pub(crate) status: String,
    pub(crate) sort_order: i32,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

impl PresentationLayer for AdminDepositNetworkConfigResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminDepositNetworkConfigResponseList {
    pub(crate) configs: Vec<AdminDepositNetworkConfigResponse>,
}

impl PresentationLayer for AdminDepositNetworkConfigResponseList {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminDepositAddressPoolQuery {
    pub(crate) network: Option<String>,
    pub(crate) address_group_code: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) asset_symbol: Option<String>,
    pub(crate) assigned_user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) address: Option<String>,
    pub(crate) limit: Option<u32>,
}

impl PresentationLayer for AdminDepositAddressPoolQuery {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateDepositAddressPoolRequest {
    pub(crate) network: String,
    pub(crate) address_group_code: Option<String>,
    pub(crate) address: String,
    pub(crate) asset_symbol: Option<String>,
    pub(crate) asset_symbols: Option<Vec<String>>,
    pub(crate) status: Option<String>,
    pub(crate) memo: Option<String>,
    pub(crate) remark: Option<String>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateDepositAddressPoolRequest {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateDepositAddressPoolRequest {
    pub(crate) network: String,
    pub(crate) address_group_code: Option<String>,
    pub(crate) address: String,
    pub(crate) asset_symbol: Option<String>,
    pub(crate) asset_symbols: Option<Vec<String>>,
    pub(crate) status: String,
    pub(crate) memo: Option<String>,
    pub(crate) remark: Option<String>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateDepositAddressPoolRequest {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReclaimDepositAddressPoolRequest {
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for ReclaimDepositAddressPoolRequest {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateDepositAddressPoolBatchRequest {
    pub(crate) network: String,
    pub(crate) address_group_code: Option<String>,
    pub(crate) asset_symbol: Option<String>,
    pub(crate) asset_symbols: Option<Vec<String>>,
    pub(crate) status: Option<String>,
    pub(crate) entries: Vec<CreateDepositAddressPoolEntryRequest>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateDepositAddressPoolBatchRequest {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateDepositAddressPoolEntryRequest {
    pub(crate) address: String,
    pub(crate) memo: Option<String>,
    pub(crate) remark: Option<String>,
}

impl PresentationLayer for CreateDepositAddressPoolEntryRequest {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminDepositAddressPoolResponse {
    pub(crate) id: u64,
    pub(crate) network: String,
    pub(crate) address_group_code: String,
    pub(crate) address: String,
    pub(crate) asset_symbol: Option<String>,
    pub(crate) asset_symbols: SqlxJson<Vec<String>>,
    pub(crate) status: String,
    pub(crate) assigned_user_id: Option<u64>,
    pub(crate) assigned_user_email: Option<String>,
    pub(crate) assigned_asset_symbol: Option<String>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) assigned_at: Option<DateTime<Utc>>,
    pub(crate) memo: Option<String>,
    pub(crate) remark: Option<String>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

impl PresentationLayer for AdminDepositAddressPoolResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminDepositAddressPoolResponseList {
    pub(crate) addresses: Vec<AdminDepositAddressPoolResponse>,
}

impl PresentationLayer for AdminDepositAddressPoolResponseList {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminDepositAddressPoolBatchResponse {
    pub(crate) addresses: Vec<AdminDepositAddressPoolResponse>,
}

impl PresentationLayer for AdminDepositAddressPoolBatchResponse {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminTradingPairQuery {
    pub(crate) symbol: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) market_type: Option<String>,
    pub(crate) limit: Option<u32>,
}

impl PresentationLayer for AdminTradingPairQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateTradingPairRequest {
    pub(crate) base_asset_id: u64,
    pub(crate) quote_asset_id: u64,
    pub(crate) symbol: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) price_precision: i32,
    pub(crate) qty_precision: i32,
    pub(crate) min_order_value: BigDecimal,
    pub(crate) status: Option<String>,
    pub(crate) market_type: Option<String>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateTradingPairRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateTradingPairStatusRequest {
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateTradingPairStatusRequest {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateTradingPairRequest {
    pub(crate) logo_url: Option<String>,
    pub(crate) price_precision: i32,
    pub(crate) qty_precision: i32,
    pub(crate) min_order_value: BigDecimal,
    pub(crate) status: String,
    pub(crate) market_type: String,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateTradingPairRequest {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminTradingPairResponse {
    pub(crate) id: u64,
    pub(crate) base_asset_id: u64,
    pub(crate) quote_asset_id: u64,
    pub(crate) symbol: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) base_asset: String,
    pub(crate) quote_asset: String,
    pub(crate) price_precision: i32,
    pub(crate) qty_precision: i32,
    pub(crate) min_order_value: BigDecimal,
    pub(crate) status: String,
    pub(crate) market_type: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for AdminTradingPairResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminTradingPairsResponse {
    pub(crate) pairs: Vec<AdminTradingPairResponse>,
}

impl PresentationLayer for AdminTradingPairsResponse {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminMarketStrategyQuery {
    pub(crate) pair_id: Option<u64>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
}

impl PresentationLayer for AdminMarketStrategyQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateMarketStrategyRequest {
    pub(crate) pair_id: u64,
    pub(crate) strategy_type: String,
    pub(crate) start_price: BigDecimal,
    pub(crate) target_price: BigDecimal,
    #[serde(with = "unix_millis")]
    pub(crate) start_time: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) end_time: DateTime<Utc>,
    pub(crate) volatility: BigDecimal,
    pub(crate) volume_min: BigDecimal,
    pub(crate) volume_max: BigDecimal,
    pub(crate) status: Option<String>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateMarketStrategyRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateMarketStrategyRequest {
    pub(crate) strategy_type: String,
    pub(crate) start_price: BigDecimal,
    pub(crate) target_price: BigDecimal,
    #[serde(with = "unix_millis")]
    pub(crate) start_time: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) end_time: DateTime<Utc>,
    pub(crate) volatility: BigDecimal,
    pub(crate) volume_min: BigDecimal,
    pub(crate) volume_max: BigDecimal,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateMarketStrategyRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateMarketStrategyStatusRequest {
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateMarketStrategyStatusRequest {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminMarketStrategyResponse {
    pub(crate) id: u64,
    pub(crate) pair_id: u64,
    pub(crate) symbol: String,
    pub(crate) market_type: String,
    pub(crate) strategy_type: String,
    pub(crate) start_price: BigDecimal,
    pub(crate) target_price: BigDecimal,
    #[serde(with = "unix_millis")]
    pub(crate) start_time: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) end_time: DateTime<Utc>,
    pub(crate) volatility: BigDecimal,
    pub(crate) volume_min: BigDecimal,
    pub(crate) volume_max: BigDecimal,
    pub(crate) status: String,
    pub(crate) run_status: Option<String>,
    pub(crate) current_price: Option<BigDecimal>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) last_generated_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) last_kline_open_time: Option<DateTime<Utc>>,
    pub(crate) recovery_status: Option<String>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for AdminMarketStrategyResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminMarketStrategiesResponse {
    pub(crate) strategies: Vec<AdminMarketStrategyResponse>,
}

impl PresentationLayer for AdminMarketStrategiesResponse {}

#[derive(Debug, Deserialize)]
pub struct SaveMarketFeedConfigRequest {
    pub symbols: Vec<String>,
    pub intervals: Vec<String>,
    pub providers: Vec<String>,
    pub enabled: bool,
    pub reason: Option<String>,
}

impl PresentationLayer for SaveMarketFeedConfigRequest {}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct MarketFeedConfigResponse {
    pub id: u64,
    pub name: String,
    pub symbols: Vec<String>,
    pub intervals: Vec<String>,
    pub providers: Vec<String>,
    pub enabled: bool,
    pub version: u64,
    pub applied_version: Option<u64>,
    pub needs_reload: bool,
    pub last_reload_status: Option<String>,
    pub last_reload_error: Option<String>,
    #[serde(default, with = "option_unix_millis")]
    pub last_reloaded_at: Option<DateTime<Utc>>,
}

impl PresentationLayer for MarketFeedConfigResponse {}

#[derive(Debug, Serialize)]
pub struct MarketFeedStatusResponse {
    pub saved_config: Option<MarketFeedConfigResponse>,
    pub runtime: MarketFeedRuntimeStatus,
}

impl PresentationLayer for MarketFeedStatusResponse {}

#[derive(Debug, Deserialize)]
pub struct ReloadMarketFeedRequest {
    pub reason: String,
}

impl PresentationLayer for ReloadMarketFeedRequest {}

#[derive(Debug, Serialize)]
pub struct ReloadMarketFeedResponse {
    pub config: MarketFeedConfigResponse,
    pub runtime: MarketFeedRuntimeStatus,
}

impl PresentationLayer for ReloadMarketFeedResponse {}

#[derive(Debug, Deserialize)]
pub struct UpsertMarketSourceCredentialRequest {
    pub auth_type: String,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub passphrase: Option<String>,
    pub enabled: bool,
    pub reason: String,
}

impl PresentationLayer for UpsertMarketSourceCredentialRequest {}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct MarketSourceCredentialResponse {
    pub provider: String,
    pub auth_type: String,
    pub api_key_mask: Option<String>,
    pub enabled: bool,
}

impl PresentationLayer for MarketSourceCredentialResponse {}

#[derive(Debug, Serialize)]
pub struct MarketSourceCredentialsResponse {
    pub credentials: Vec<MarketSourceCredentialResponse>,
}

impl PresentationLayer for MarketSourceCredentialsResponse {}

#[derive(Debug, Clone)]
pub struct MarketSourceCredentialSecret {
    pub provider: String,
    pub auth_type: String,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub passphrase: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SaveSmtpConfigRequest {
    pub name: Option<String>,
    pub host: String,
    pub port: u16,
    pub security: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub from_email: String,
    pub from_name: Option<String>,
    pub verification_code_template_html: Option<String>,
    pub verification_code_templates: Option<Vec<VerificationCodeTemplate>>,
    pub enabled: bool,
    pub priority: Option<u32>,
    pub reason: Option<String>,
}

impl PresentationLayer for SaveSmtpConfigRequest {}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct SmtpConfigResponse {
    pub id: u64,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub security: String,
    pub username_mask: Option<String>,
    pub password_set: bool,
    pub from_email: String,
    pub from_name: Option<String>,
    pub verification_code_template_html: Option<String>,
    pub verification_code_templates: Vec<VerificationCodeTemplate>,
    pub enabled: bool,
    pub priority: u32,
}

impl PresentationLayer for SmtpConfigResponse {}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct SmtpDeliverySettingsResponse {
    pub strategy: String,
}

impl PresentationLayer for SmtpDeliverySettingsResponse {}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct SmtpConfigListResponse {
    pub configs: Vec<SmtpConfigResponse>,
    pub delivery_settings: SmtpDeliverySettingsResponse,
}

impl PresentationLayer for SmtpConfigListResponse {}

#[derive(Debug, Deserialize)]
pub struct SaveSmtpDeliverySettingsRequest {
    pub strategy: String,
    pub reason: Option<String>,
}

impl PresentationLayer for SaveSmtpDeliverySettingsRequest {}

#[derive(Debug, Deserialize)]
pub struct SendSmtpTestRequest {
    pub recipient: String,
    pub config_id: Option<u64>,
    pub reason: Option<String>,
}

impl PresentationLayer for SendSmtpTestRequest {}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct SendSmtpTestResponse {
    pub sent: bool,
    pub recipient: String,
    pub config_id: u64,
    pub config_name: String,
}

impl PresentationLayer for SendSmtpTestResponse {}

#[derive(Debug, Deserialize)]
pub struct SaveUploadConfigRequest {
    pub provider: String,
    pub endpoint: Option<String>,
    pub file_field: Option<String>,
    pub bearer_token: Option<String>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
    pub bucket: Option<String>,
    pub region: Option<String>,
    pub public_base_url: Option<String>,
    pub local_root: Option<String>,
    pub key_prefix: Option<String>,
    pub max_file_size_bytes: Option<u64>,
    pub allowed_mime_types: Option<Vec<String>>,
    pub enabled: bool,
    pub reason: Option<String>,
}

impl PresentationLayer for SaveUploadConfigRequest {}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct UploadConfigResponse {
    pub id: u64,
    pub name: String,
    pub provider: String,
    pub endpoint: Option<String>,
    pub file_field: Option<String>,
    pub bearer_token_mask: Option<String>,
    pub bearer_token_set: bool,
    pub access_key_mask: Option<String>,
    pub access_key_set: bool,
    pub secret_key_set: bool,
    pub bucket: Option<String>,
    pub region: Option<String>,
    pub public_base_url: Option<String>,
    pub local_root: Option<String>,
    pub key_prefix: Option<String>,
    pub max_file_size_bytes: u64,
    pub allowed_mime_types: Vec<String>,
    pub enabled: bool,
}

impl PresentationLayer for UploadConfigResponse {}

#[derive(Debug, Clone)]
pub struct UploadFileInput {
    pub original_filename: Option<String>,
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

impl PresentationLayer for UploadFileInput {}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct UploadImageResponse {
    pub provider: String,
    pub object_key: String,
    pub download_url: String,
    pub share_url: Option<String>,
    pub delete_url: Option<String>,
    pub mime_type: String,
    pub size_bytes: u64,
}

impl PresentationLayer for UploadImageResponse {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminMarginLiquidationQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) pair_id: Option<u64>,
    pub(crate) position_id: Option<u64>,
    pub(crate) limit: Option<u32>,
}

impl PresentationLayer for AdminMarginLiquidationQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminAuditLogsQuery {
    pub(crate) admin_id: Option<u64>,
    pub(crate) action: Option<String>,
    pub(crate) target_type: Option<String>,
    pub(crate) target_id: Option<String>,
    pub(crate) limit: Option<u32>,
}

impl PresentationLayer for AdminAuditLogsQuery {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminMarginLiquidationResponse {
    pub(crate) id: u64,
    pub(crate) position_id: u64,
    pub(crate) user_id: u64,
    pub(crate) product_id: u64,
    pub(crate) pair_id: u64,
    pub(crate) margin_asset: u64,
    pub(crate) direction: String,
    pub(crate) margin_amount: BigDecimal,
    pub(crate) notional_amount: BigDecimal,
    pub(crate) interest_amount: BigDecimal,
    pub(crate) entry_price: BigDecimal,
    pub(crate) mark_price: BigDecimal,
    pub(crate) maintenance_margin_rate: BigDecimal,
    pub(crate) equity: BigDecimal,
    pub(crate) maintenance_margin: BigDecimal,
    pub(crate) realized_pnl: BigDecimal,
    pub(crate) payout_amount: BigDecimal,
    pub(crate) reason: String,
    #[serde(with = "unix_millis")]
    pub(crate) liquidated_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for AdminMarginLiquidationResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminMarginLiquidationsResponse {
    pub(crate) liquidations: Vec<AdminMarginLiquidationResponse>,
}

impl PresentationLayer for AdminMarginLiquidationsResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminAuditLogResponse {
    pub(crate) id: u64,
    pub(crate) admin_id: u64,
    pub(crate) action: String,
    pub(crate) target_type: String,
    pub(crate) target_id: String,
    pub(crate) before_json: Option<SqlxJson<Value>>,
    pub(crate) after_json: Option<SqlxJson<Value>>,
    pub(crate) reason: Option<String>,
    pub(crate) ip: Option<String>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for AdminAuditLogResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminAuditLogsResponse {
    pub(crate) logs: Vec<AdminAuditLogResponse>,
}

impl PresentationLayer for AdminAuditLogsResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminDashboardResponse {
    #[serde(with = "unix_millis")]
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) users: AdminDashboardUsersSummary,
    pub(crate) wallet: AdminDashboardWalletSummary,
    pub(crate) market: AdminDashboardMarketSummary,
    pub(crate) trading: AdminDashboardTradingSummary,
    pub(crate) products: AdminDashboardProductsSummary,
    pub(crate) risk: AdminDashboardRiskSummary,
    pub(crate) audit: AdminDashboardAuditSummary,
}

impl PresentationLayer for AdminDashboardResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminDashboardUsersSummary {
    pub(crate) total: i64,
    pub(crate) active: i64,
    pub(crate) new_24h: i64,
}

impl PresentationLayer for AdminDashboardUsersSummary {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminDashboardWalletSummary {
    pub(crate) active_assets: i64,
    pub(crate) wallet_accounts: i64,
    pub(crate) non_zero_accounts: i64,
    pub(crate) pending_unlocks: i64,
    pub(crate) pending_deposits: i64,
    pub(crate) pending_withdrawals: i64,
    pub(crate) custody_status: String,
}

impl PresentationLayer for AdminDashboardWalletSummary {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminDashboardMarketSummary {
    pub(crate) active_pairs: i64,
    pub(crate) disabled_pairs: i64,
    pub(crate) external_pairs: i64,
    pub(crate) strategy_pairs: i64,
    pub(crate) feed_runtime_status: String,
    pub(crate) feed_needs_reload: bool,
    pub(crate) feed_symbols: Vec<String>,
    pub(crate) feed_providers: Vec<String>,
}

impl PresentationLayer for AdminDashboardMarketSummary {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminDashboardTradingSummary {
    pub(crate) spot_open_orders: i64,
    pub(crate) spot_trades_24h: i64,
    pub(crate) convert_pending_orders: i64,
    pub(crate) convert_completed_24h: i64,
}

impl PresentationLayer for AdminDashboardTradingSummary {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminDashboardProductsSummary {
    pub(crate) seconds_open_orders: i64,
    pub(crate) margin_open_positions: i64,
    pub(crate) margin_liquidated_24h: i64,
    pub(crate) earn_active_subscriptions: i64,
    pub(crate) earn_maturing_24h: i64,
}

impl PresentationLayer for AdminDashboardProductsSummary {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminDashboardRiskSummary {
    pub(crate) risk_events_24h: i64,
    pub(crate) blocked_events_24h: i64,
    pub(crate) pending_outbox_events: i64,
    pub(crate) retry_inbox_events: i64,
    pub(crate) dead_letter_inbox_events: i64,
}

impl PresentationLayer for AdminDashboardRiskSummary {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminDashboardAuditSummary {
    pub(crate) admin_actions_24h: i64,
    pub(crate) latest_actions: Vec<AdminDashboardAuditAction>,
}

impl PresentationLayer for AdminDashboardAuditSummary {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminDashboardAuditAction {
    pub(crate) id: u64,
    pub(crate) admin_id: u64,
    pub(crate) action: String,
    pub(crate) target_type: String,
    pub(crate) target_id: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for AdminDashboardAuditAction {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminNewCoinProjectQuery {
    pub(crate) limit: Option<u32>,
}

impl PresentationLayer for AdminNewCoinProjectQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminNewCoinScopedListQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
}

impl PresentationLayer for AdminNewCoinScopedListQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminNewCoinFlatListQuery {
    pub(crate) project_id: Option<u64>,
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
}

impl PresentationLayer for AdminNewCoinFlatListQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminNewCoinPurchaseQuery {
    pub(crate) project_id: Option<u64>,
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
}

impl PresentationLayer for AdminNewCoinPurchaseQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminNewCoinLockPositionQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) asset_id: Option<u64>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
}

impl PresentationLayer for AdminNewCoinLockPositionQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminNewCoinUnlockQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) asset_id: Option<u64>,
    pub(crate) status: Option<String>,
    pub(crate) fee_paid_status: Option<String>,
    pub(crate) limit: Option<u32>,
}

impl PresentationLayer for AdminNewCoinUnlockQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateNewCoinProjectRequest {
    pub(crate) asset_id: u64,
    pub(crate) symbol: String,
    pub(crate) lifecycle_status: String,
    pub(crate) total_supply: BigDecimal,
    pub(crate) issue_price: BigDecimal,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) listed_at: Option<DateTime<Utc>>,
    pub(crate) unlock_type: String,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) fixed_unlock_at: Option<DateTime<Utc>>,
    pub(crate) relative_unlock_seconds: Option<u64>,
    pub(crate) unlock_fee_enabled: Option<bool>,
    pub(crate) unlock_fee_rate: Option<BigDecimal>,
    pub(crate) unlock_fee_basis: Option<String>,
    pub(crate) unlock_fee_asset: Option<u64>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateNewCoinProjectRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateNewCoinLifecycleRequest {
    pub(crate) lifecycle_status: String,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) listed_at: Option<DateTime<Utc>>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateNewCoinLifecycleRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct DistributeNewCoinRequest {
    pub(crate) user_id: u64,
    pub(crate) subscription_id: Option<u64>,
    pub(crate) quantity: BigDecimal,
    pub(crate) idempotency_key: String,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for DistributeNewCoinRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateNewCoinUnlockRuleRequest {
    pub(crate) unlock_type: String,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) listed_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) fixed_unlock_at: Option<DateTime<Utc>>,
    pub(crate) relative_unlock_seconds: Option<u64>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateNewCoinUnlockRuleRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateNewCoinUnlockFeeRuleRequest {
    pub(crate) unlock_fee_enabled: bool,
    pub(crate) unlock_fee_rate: Option<BigDecimal>,
    pub(crate) unlock_fee_basis: Option<String>,
    pub(crate) unlock_fee_asset: Option<u64>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateNewCoinUnlockFeeRuleRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateNewCoinPostListingPurchaseRequest {
    pub(crate) enabled: bool,
    pub(crate) pair_id: Option<u64>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateNewCoinPostListingPurchaseRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpsertNewCoinConvertRuleRequest {
    pub(crate) convert_pair_id: u64,
    pub(crate) rate_source: String,
    pub(crate) fixed_rate: Option<BigDecimal>,
    pub(crate) floating_rate_json: Option<Value>,
    pub(crate) status: Option<String>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpsertNewCoinConvertRuleRequest {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct NewCoinProjectResponse {
    pub(crate) id: u64,
    pub(crate) asset_id: u64,
    pub(crate) symbol: String,
    pub(crate) lifecycle_status: String,
    pub(crate) total_supply: BigDecimal,
    pub(crate) issue_price: BigDecimal,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) listed_at: Option<DateTime<Utc>>,
    pub(crate) unlock_type: String,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) fixed_unlock_at: Option<DateTime<Utc>>,
    pub(crate) relative_unlock_seconds: Option<u64>,
    pub(crate) unlock_fee_enabled: bool,
    pub(crate) unlock_fee_rate: Option<BigDecimal>,
    pub(crate) unlock_fee_basis: Option<String>,
    pub(crate) unlock_fee_asset: Option<u64>,
    pub(crate) status: String,
    pub(crate) post_listing_purchase_enabled: bool,
    pub(crate) post_listing_pair_id: Option<u64>,
    pub(crate) post_listing_pair_status: Option<String>,
}

impl PresentationLayer for NewCoinProjectResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinProjectsResponse {
    pub(crate) projects: Vec<NewCoinProjectResponse>,
}

impl PresentationLayer for NewCoinProjectsResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct NewCoinSubscriptionResponse {
    pub(crate) id: u64,
    pub(crate) project_id: u64,
    pub(crate) user_id: u64,
    pub(crate) quote_asset: u64,
    pub(crate) quote_amount: BigDecimal,
    pub(crate) requested_quantity: BigDecimal,
    pub(crate) allocated_quantity: BigDecimal,
    pub(crate) status: String,
    pub(crate) idempotency_key: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for NewCoinSubscriptionResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinSubscriptionsResponse {
    pub(crate) subscriptions: Vec<NewCoinSubscriptionResponse>,
}

impl PresentationLayer for NewCoinSubscriptionsResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct NewCoinDistributionResponse {
    pub(crate) id: u64,
    pub(crate) project_id: u64,
    pub(crate) user_id: u64,
    pub(crate) subscription_id: Option<u64>,
    pub(crate) asset_id: u64,
    pub(crate) quantity: BigDecimal,
    pub(crate) lock_position_id: Option<u64>,
    pub(crate) status: String,
    pub(crate) idempotency_key: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for NewCoinDistributionResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinDistributionsResponse {
    pub(crate) distributions: Vec<NewCoinDistributionResponse>,
}

impl PresentationLayer for NewCoinDistributionsResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct NewCoinPurchaseResponse {
    pub(crate) id: u64,
    pub(crate) project_id: u64,
    pub(crate) user_id: u64,
    pub(crate) pair_id: u64,
    pub(crate) base_asset: u64,
    pub(crate) quote_asset: u64,
    pub(crate) price: BigDecimal,
    pub(crate) quantity: BigDecimal,
    pub(crate) quote_amount: BigDecimal,
    pub(crate) lock_position_id: Option<u64>,
    pub(crate) status: String,
    pub(crate) idempotency_key: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for NewCoinPurchaseResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinPurchasesResponse {
    pub(crate) purchases: Vec<NewCoinPurchaseResponse>,
}

impl PresentationLayer for NewCoinPurchasesResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct NewCoinLockPositionResponse {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    pub(crate) asset_id: u64,
    pub(crate) unlock_type: String,
    #[serde(with = "unix_millis")]
    pub(crate) unlock_at: DateTime<Utc>,
    pub(crate) locked_amount: BigDecimal,
    pub(crate) released_amount: BigDecimal,
    pub(crate) remaining_amount: BigDecimal,
    pub(crate) merge_key: String,
    pub(crate) status: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for NewCoinLockPositionResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinLockPositionsResponse {
    pub(crate) lock_positions: Vec<NewCoinLockPositionResponse>,
}

impl PresentationLayer for NewCoinLockPositionsResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct NewCoinUnlockResponse {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    pub(crate) asset_id: u64,
    pub(crate) lock_position_id: u64,
    pub(crate) unlock_quantity: BigDecimal,
    pub(crate) unlock_price: Option<BigDecimal>,
    pub(crate) unlock_fee_enabled: bool,
    pub(crate) unlock_fee_rate: Option<BigDecimal>,
    pub(crate) unlock_fee_basis: Option<String>,
    pub(crate) unlock_fee_asset: Option<u64>,
    pub(crate) unlock_fee_amount: Option<BigDecimal>,
    pub(crate) fee_paid_status: String,
    pub(crate) status: String,
    pub(crate) idempotency_key: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for NewCoinUnlockResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinUnlocksResponse {
    pub(crate) unlocks: Vec<NewCoinUnlockResponse>,
}

impl PresentationLayer for NewCoinUnlocksResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct NewCoinConvertRuleResponse {
    pub(crate) id: u64,
    pub(crate) convert_pair_id: u64,
    pub(crate) rate_source: String,
    pub(crate) fixed_rate: Option<BigDecimal>,
    pub(crate) floating_rate_json: Option<SqlxJson<Value>>,
    pub(crate) status: String,
    pub(crate) created_by: Option<u64>,
}

impl PresentationLayer for NewCoinConvertRuleResponse {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminConvertPairQuery {
    pub(crate) limit: Option<u32>,
}

impl PresentationLayer for AdminConvertPairQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminConvertOrdersQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
}

impl PresentationLayer for AdminConvertOrdersQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateConvertPairRequest {
    pub(crate) from_asset_id: u64,
    pub(crate) to_asset_id: u64,
    pub(crate) pricing_mode: String,
    pub(crate) spread_rate: BigDecimal,
    pub(crate) fee_rate: Option<BigDecimal>,
    pub(crate) min_amount: BigDecimal,
    pub(crate) max_amount: Option<BigDecimal>,
    pub(crate) target_min_amount: Option<BigDecimal>,
    pub(crate) target_max_amount: Option<BigDecimal>,
    pub(crate) enabled: Option<bool>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateConvertPairRequest {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateConvertPairRequest {
    pub(crate) from_asset_id: Option<u64>,
    pub(crate) to_asset_id: Option<u64>,
    pub(crate) pricing_mode: Option<String>,
    pub(crate) spread_rate: Option<BigDecimal>,
    pub(crate) fee_rate: Option<BigDecimal>,
    pub(crate) min_amount: Option<BigDecimal>,
    #[serde(default)]
    pub(crate) max_amount: Option<Option<BigDecimal>>,
    pub(crate) target_min_amount: Option<BigDecimal>,
    #[serde(default)]
    pub(crate) target_max_amount: Option<Option<BigDecimal>>,
    pub(crate) enabled: Option<bool>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateConvertPairRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct DeleteConvertPairRequest {
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for DeleteConvertPairRequest {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct ConvertPairResponse {
    pub(crate) id: u64,
    pub(crate) from_asset_id: u64,
    pub(crate) from_asset_symbol: String,
    pub(crate) to_asset_id: u64,
    pub(crate) to_asset_symbol: String,
    pub(crate) pricing_mode: String,
    pub(crate) spread_rate: BigDecimal,
    pub(crate) fee_rate: BigDecimal,
    pub(crate) min_amount: BigDecimal,
    pub(crate) max_amount: Option<BigDecimal>,
    pub(crate) target_min_amount: BigDecimal,
    pub(crate) target_max_amount: Option<BigDecimal>,
    pub(crate) enabled: bool,
}

impl PresentationLayer for ConvertPairResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct ConvertPairsResponse {
    pub(crate) pairs: Vec<ConvertPairResponse>,
}

impl PresentationLayer for ConvertPairsResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct ConvertOrderResponse {
    pub(crate) id: u64,
    pub(crate) user_email: String,
    pub(crate) from_asset_symbol: String,
    pub(crate) to_asset_symbol: String,
    pub(crate) from_amount: BigDecimal,
    pub(crate) to_amount: BigDecimal,
    pub(crate) rate: BigDecimal,
    pub(crate) fee_rate: BigDecimal,
    pub(crate) fee_amount: BigDecimal,
    pub(crate) status: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for ConvertOrderResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct ConvertOrdersResponse {
    pub(crate) orders: Vec<ConvertOrderResponse>,
}

impl PresentationLayer for ConvertOrdersResponse {}

fn serialize_decimal_amount<S>(amount: &BigDecimal, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&format!("{amount:.18}"))
}
