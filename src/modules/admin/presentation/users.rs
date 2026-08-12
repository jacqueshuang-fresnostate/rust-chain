//! 后台用户、钱包账户流水、KYC 与用户安全操作 DTO。

use super::*;

#[derive(Debug, Deserialize)]
pub(crate) struct AdminUserQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) include_internal: Option<bool>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
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
    pub(crate) total: i64,
}

impl PresentationLayer for AdminUsersResponse {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminKycSubmissionQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
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
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateUserStatusRequest {
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateUserStatusRequest {}
