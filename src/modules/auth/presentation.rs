//! auth bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。

use crate::{
    architecture::PresentationLayer,
    modules::auth::{IssuedTokens, TokenScope},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub(crate) struct UserAuthRequest {
    pub(crate) email: Option<String>,
    pub(crate) phone: Option<String>,
    pub(crate) username: Option<String>,
    pub(crate) password: Option<String>,
    pub(crate) country_code: Option<String>,
    pub(crate) code: Option<String>,
    pub(crate) invite_code: Option<String>,
    pub(crate) promotion: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RegisterEmailCodeRequest {
    pub(crate) email: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PasswordResetCodeRequest {
    pub(crate) email: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PasswordResetRequest {
    pub(crate) email: String,
    pub(crate) code: String,
    pub(crate) password: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminAuthRequest {
    pub(crate) username: Option<String>,
    pub(crate) password: Option<String>,
    pub(crate) role_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AgentAuthRequest {
    pub(crate) username: Option<String>,
    pub(crate) password: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RefreshRequest {
    pub(crate) refresh_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LoginTwoFactorRequest {
    pub(crate) challenge_id: String,
    pub(crate) totp_code: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LoginTwoFactorResetCodeRequest {
    pub(crate) challenge_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LoginTwoFactorResetRequest {
    pub(crate) challenge_id: String,
    pub(crate) code: String,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum UserLoginResponse {
    Token(TokenResponse),
    TwoFactorChallenge(LoginTwoFactorChallengeResponse),
    TwoFactorSetupChallenge(LoginTwoFactorSetupChallengeResponse),
}

impl PresentationLayer for UserLoginResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct LoginTwoFactorChallengeResponse {
    pub(crate) requires_2fa: bool,
    pub(crate) challenge_id: String,
    pub(crate) expires_in_seconds: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct LoginTwoFactorSetupChallengeResponse {
    pub(crate) requires_2fa_setup: bool,
    pub(crate) setup_challenge_id: String,
    pub(crate) expires_in_seconds: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct LoginTwoFactorCodeResponse {
    pub(crate) sent: bool,
    pub(crate) expires_in_seconds: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct RegisterConfigResponse {
    pub(crate) email_code_required: bool,
    pub(crate) invite_code_required: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct LoginConfigResponse {
    pub(crate) username_login_enabled: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct RegisterEmailCodeResponse {
    pub(crate) sent: bool,
    pub(crate) expires_in_seconds: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct LoginTwoFactorResetResponse {
    pub(crate) reset: bool,
    pub(crate) requires_relogin: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct PasswordResetCodeResponse {
    pub(crate) sent: bool,
    pub(crate) expires_in_seconds: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct PasswordResetResponse {
    pub(crate) reset: bool,
    pub(crate) requires_relogin: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct TokenResponse {
    access_token: String,
    refresh_token: String,
    token_type: &'static str,
    scope: TokenScope,
}

impl From<IssuedTokens> for TokenResponse {
    fn from(tokens: IssuedTokens) -> Self {
        Self {
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            token_type: tokens.token_type,
            scope: tokens.scope,
        }
    }
}
