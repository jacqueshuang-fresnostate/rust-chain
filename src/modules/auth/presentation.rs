//! auth bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。
//!
//! 本文件定义认证限界上下文对外的线格式：注册、登录、刷新、二次验证与密码重置的请求体，
//! 以及令牌、挑战、配置类响应体，另外把 HTTP 头里的 Cloudflare 与代理信息归一化为登录传输上下文。
//! 这里只做结构映射，不做任何判定：是否需要人机校验、账号是否存在、验证码是否有效都由应用与领域层决定。
//! 响应结构体刻意只暴露令牌串、挑战标识和过期秒数，不承载密码哈希、Turnstile 服务端密钥或账号存在性线索；
//! 二次验证绑定阶段的 `secret` 与 `otpauth_uri` 是唯一会外发的敏感字段，只允许回给已完成前置校验的本人，
//! 调用链上任何一层都不得把这些结构体整体写入日志。

use crate::{
    architecture::PresentationLayer,
    modules::auth::{IssuedTokens, TokenScope},
};
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoginTransportContext {
    pub(crate) remote_ip: Option<String>,
    pub(crate) has_cf_clearance: bool,
}

impl LoginTransportContext {
    /// 从请求头一次性提取登录风控所需的两项传输信息：客户端来源 IP 与是否已持有 Cloudflare 通行 Cookie。
    /// 两项取值都来自客户端可控的头部，只作为 Turnstile 站点校验的输入线索，不能当作身份或授权依据；
    /// 因此本上下文不参与口令比对，也不会让任何账号跳过密码验证。是否真的发起回源校验由领域策略判定。
    pub(crate) fn from_headers(headers: &HeaderMap) -> Self {
        Self {
            remote_ip: extract_client_ip(headers),
            has_cf_clearance: has_cf_clearance_cookie(headers),
        }
    }
}

/// 按 `cf-connecting-ip` 优先、`x-forwarded-for` 兜底的顺序解析客户端 IP，只取链路中的第一段并去空白。
/// 取第一段是因为 `x-forwarded-for` 会按经过的代理逐跳追加，最左侧才是最初的客户端地址。
/// 非 ASCII 或无法解析为字符串的头部一律视为缺失。该值只会随 `remoteip` 提交给 Turnstile 站点校验，
/// 由于上游代理可以伪造这两个头，调用方不得据此做限流白名单、审计归属或访问控制判断。
fn extract_client_ip(headers: &HeaderMap) -> Option<String> {
    headers
        .get("cf-connecting-ip")
        .or_else(|| headers.get("x-forwarded-for"))
        .and_then(|header| header.to_str().ok())
        .map(|value| value.split(',').next().unwrap_or(value).trim().to_owned())
}

/// 判断 Cookie 头中是否出现名为 `cf_clearance` 的条目，按分号拆分并逐段去空白后做前缀匹配。
/// 只检查名字是否存在，不解析、不校验、也不解密其值，因为该 Cookie 由 Cloudflare 边缘签发和验证。
/// 结果仅用于决定这次登录能否免去一次回源人机校验，属于性能优化而非安全结论；
/// 客户端可以随意伪造这个 Cookie 名，因此在强制校验开启时该判断会被忽略，任何情况下都不影响口令校验。
fn has_cf_clearance_cookie(headers: &HeaderMap) -> bool {
    headers
        .get("cookie")
        .and_then(|value| value.to_str().ok())
        .map(|cookie| {
            cookie
                .split(';')
                .map(str::trim)
                .any(|entry| entry.starts_with("cf_clearance="))
        })
        .unwrap_or(false)
}

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
    pub(crate) cf_turnstile_token: Option<String>,
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
    pub(crate) cf_turnstile_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AgentAuthRequest {
    pub(crate) username: Option<String>,
    pub(crate) password: Option<String>,
    pub(crate) cf_turnstile_token: Option<String>,
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
pub(crate) struct LoginTwoFactorSetupRequest {
    pub(crate) setup_challenge_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LoginTwoFactorSetupConfirmRequest {
    pub(crate) setup_challenge_id: String,
    pub(crate) totp_code: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminTwoFactorCodeRequest {
    pub(crate) totp_code: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminPasswordChangeRequest {
    pub(crate) current_password: Option<String>,
    pub(crate) new_password: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AdminPasswordChangeResponse {
    pub(crate) changed: bool,
    pub(crate) requires_relogin: bool,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum AdminLoginResponse {
    Token(TokenResponse),
    TwoFactorChallenge(LoginTwoFactorChallengeResponse),
}

impl PresentationLayer for AdminLoginResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminTwoFactorStatusResponse {
    pub(crate) totp_enabled: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct AdminTwoFactorSetupResponse {
    pub(crate) otpauth_uri: String,
    pub(crate) secret: String,
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
pub(crate) struct LoginTwoFactorSetupResponse {
    pub(crate) secret: String,
    pub(crate) otpauth_uri: String,
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
    pub(crate) cf_turnstile_enabled: bool,
    pub(crate) cf_turnstile_site_key: Option<String>,
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
    /// 把服务层签发的令牌对搬运为对外响应体，字段一一对应，不重新签名也不改变有效期。
    /// 转换后访问令牌与刷新令牌都以明文出现在响应体中，是整条链路上令牌唯一对外暴露的位置，
    /// 因此该结构体只能直接序列化返回，不得写入日志、事件或审计记录。
    fn from(tokens: IssuedTokens) -> Self {
        Self {
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            token_type: tokens.token_type,
            scope: tokens.scope,
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_auth_presentation_tests.rs"]
mod tests;
