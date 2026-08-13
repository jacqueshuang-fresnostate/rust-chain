//! user bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。
//! 本文件只声明用户自服务接口的入参与出参结构，不含任何业务逻辑、校验或数据库访问。
//! 所有时间字段统一通过 `unix_millis` 与 `option_unix_millis` 序列化为 Unix 毫秒整数，
//! 避免前端受时区与格式化差异影响。
//! 隐私边界：资料类响应会带出邮箱、手机号等本人字段，但结构体中不存在任何密码哈希、
//! 资金密码或 TOTP 密钥字段，唯一例外是二次验证生成响应中的一次性密钥，仅在绑定时下发一次。
//! 敏感凭证只出现在请求结构中（登录密码、资金密码、各类验证码），且这些结构不派生序列化，
//! 因此不会被反向输出到响应或日志。

use crate::modules::{
    admin::presentation::UploadImageResponse,
    auth::TokenScope,
    security::{LoginTwoFactorMode, PaymentPolicies, ThirdPartyBindingPolicy},
};
use crate::time::{option_unix_millis, unix_millis};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub(crate) struct UserProfileResponse {
    pub(crate) id: u64,
    pub(crate) username: Option<String>,
    pub(crate) email: Option<String>,
    pub(crate) phone: Option<String>,
    pub(crate) avatar_url: Option<String>,
    pub(crate) country_code: Option<String>,
    pub(crate) preferred_locale: Option<String>,
    pub(crate) default_locale: Option<String>,
    pub(crate) supported_locales: Option<Vec<String>>,
    pub(crate) status: String,
    pub(crate) kyc_level: i32,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) email_verified_at: Option<DateTime<Utc>>,
    pub(crate) fund_password_set: bool,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct UserAvatarResponse {
    pub(crate) avatar_url: String,
    pub(crate) upload: UploadImageResponse,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BindEmailCodeRequest {
    pub(crate) email: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct BindEmailCodeResponse {
    pub(crate) sent: bool,
    #[serde(with = "unix_millis")]
    pub(crate) expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BindEmailRequest {
    pub(crate) email: String,
    pub(crate) code: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct BindEmailResponse {
    pub(crate) email: String,
    #[serde(with = "unix_millis")]
    pub(crate) email_verified_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChangePasswordRequest {
    pub(crate) old_password: String,
    pub(crate) new_password: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct TokenResponse {
    pub(crate) access_token: String,
    pub(crate) refresh_token: String,
    pub(crate) token_type: &'static str,
    pub(crate) scope: TokenScope,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateFundPasswordRequest {
    pub(crate) login_password: String,
    pub(crate) fund_password: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChangeFundPasswordRequest {
    pub(crate) old_fund_password: String,
    pub(crate) new_fund_password: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ResetFundPasswordRequest {
    pub(crate) code: String,
    pub(crate) new_fund_password: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct FundPasswordResponse {
    pub(crate) fund_password_set: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateUsernameRequest {
    pub(crate) username: String,
}

/// 用户重置两步验证码时使用的入参。
#[derive(Debug, Deserialize)]
pub(crate) struct ResetTwoFactorRequest {
    pub(crate) code: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct UpdateUsernameResponse {
    pub(crate) username: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct UserTwoFactorStatusResponse {
    pub(crate) totp_enabled: bool,
    pub(crate) login_2fa_enabled: bool,
    pub(crate) login_2fa_mode: LoginTwoFactorMode,
    pub(crate) can_toggle_login_2fa: bool,
    pub(crate) payment_policies: PaymentPolicies,
    pub(crate) third_party_bindings: ThirdPartyBindingPolicy,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct ThirdPartyBindingResponse {
    pub(crate) provider: String,
    pub(crate) account_identifier: String,
    pub(crate) display_name: Option<String>,
    pub(crate) status: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ThirdPartyBindingStatusResponse {
    pub(crate) policy: ThirdPartyBindingPolicy,
    pub(crate) bindings: Vec<ThirdPartyBindingResponse>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BindThirdPartyAccountRequest {
    pub(crate) provider: String,
    pub(crate) account_identifier: String,
    pub(crate) display_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SetupTwoFactorResponse {
    pub(crate) secret: String,
    pub(crate) otpauth_uri: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ConfirmTwoFactorRequest {
    pub(crate) totp_code: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateLoginTwoFactorRequest {
    pub(crate) enabled: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BindReferralCodeRequest {
    pub(crate) code: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct ReferralBindingResponse {
    pub(crate) user_id: u64,
    pub(crate) direct_inviter_id: Option<u64>,
    pub(crate) direct_inviter_type: Option<String>,
    pub(crate) root_agent_id: Option<u64>,
    pub(crate) depth: i32,
    pub(crate) path: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) bound: bool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct ReferralCodeResponse {
    pub(crate) id: u64,
    pub(crate) owner_type: String,
    pub(crate) owner_id: u64,
    pub(crate) code: String,
    pub(crate) usage_limit: Option<i32>,
    pub(crate) used_count: i32,
    pub(crate) status: String,
    pub(crate) root_agent_id: Option<u64>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct MyInvitesResponse {
    pub(crate) users: Vec<MyInviteUserResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct MyInviteUserResponse {
    pub(crate) user_id: u64,
    pub(crate) email: Option<String>,
    pub(crate) phone: Option<String>,
    pub(crate) status: String,
    pub(crate) direct_inviter_type: Option<String>,
    pub(crate) direct_inviter_id: Option<u64>,
    pub(crate) root_agent_id: Option<u64>,
    pub(crate) depth: i32,
    pub(crate) path: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}
