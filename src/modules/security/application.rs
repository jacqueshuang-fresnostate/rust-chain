//! security bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。

use crate::{
    architecture::ApplicationLayer,
    config::Settings,
    error::{AppError, AppResult},
    infra::secrets::decrypt_secret,
    modules::{
        auth::verify_password,
        security::{
            domain::{
                ADMIN_LOGIN_TWO_FACTOR_ATTEMPT_LIMIT, AdminLoginTwoFactorChallenge,
                LoginTwoFactorChallenge, LoginTwoFactorChallengeType, SecurityAction,
                SecurityVerificationInput, SecurityVerificationMethod, login_challenge_expired,
                required_security_field, verify_totp_code,
            },
            infrastructure::{
                load_admin_two_factor, load_security_policy, load_user_fund_password_hash,
                load_user_two_factor, record_admin_totp_verified, record_user_totp_verified,
            },
        },
    },
};
use chrono::Utc;
use sqlx::{MySql, Pool};

#[derive(Debug)]
pub struct SecurityVerificationUseCase;

impl ApplicationLayer for SecurityVerificationUseCase {}

/// 检查用户挑战快照类型匹配、未消费且尚未过期，失效时返回统一重新登录语义。
/// 本函数不锁行或消费挑战；后续写入前存在并发竞争，调用方须另行完成防重放消费。
pub fn ensure_login_challenge_usable(
    challenge: &LoginTwoFactorChallenge,
    expected_type: LoginTwoFactorChallengeType,
) -> AppResult<()> {
    if challenge.challenge_type != expected_type
        || challenge.consumed_at.is_some()
        || challenge.expires_at <= Utc::now()
    {
        return Err(login_challenge_expired());
    }
    Ok(())
}

/// 检查管理员挑战快照尚未消费、未过期且未达到单挑战试码上限。
/// 该纯检查不锁行也不消费记录；调用方仍须持久化消费，并自行承担检查后的并发竞争。
pub fn ensure_admin_login_challenge_usable(
    challenge: &AdminLoginTwoFactorChallenge,
) -> AppResult<()> {
    if challenge.consumed_at.is_some()
        || challenge.attempt_count >= ADMIN_LOGIN_TWO_FACTOR_ATTEMPT_LIMIT
        || challenge.expires_at <= Utc::now()
    {
        return Err(login_challenge_expired());
    }
    Ok(())
}

/// 读取管理员已启用的加密 TOTP 密钥，解密后校验当前时间窗口。
/// 加密密钥与解密明文仅在当前调用内使用，不得记录；校验成功另发 SQL 更新最后验证时间。
/// 未绑定、密钥配置或动态码错误均不降级放行，时间写入不与调用方后续动作共用事务。
pub async fn verify_admin_totp(
    pool: &Pool<MySql>,
    settings: &Settings,
    admin_id: u64,
    code: &str,
) -> AppResult<()> {
    let two_factor = load_admin_two_factor(pool, admin_id).await?;
    let encrypted_secret = two_factor
        .totp_secret_encrypted
        .filter(|_| two_factor.totp_enabled)
        .ok_or_else(|| AppError::security_validation("2fa_required_not_bound", "请先绑定 2FA"))?;
    let secret = decrypt_secret(&encrypted_secret, credential_encryption_key(settings)?)?;
    if !verify_totp_code(&secret, code, Utc::now())? {
        return Err(AppError::security_validation(
            "invalid_2fa_code",
            "2FA 验证码错误",
        ));
    }

    record_admin_totp_verified(pool, admin_id).await?;

    Ok(())
}

/// 返回进程配置中的凭证加密密钥引用；缺失时返回服务端配置错误，调用方不得记录或响应该值。
pub fn credential_encryption_key(settings: &Settings) -> AppResult<&str> {
    settings
        .exposed_credential_encryption_key()
        .ok_or_else(|| AppError::Internal("credential encryption key is not configured".to_owned()))
}

/// 按安全策略编排资金密码与 TOTP 校验，策略关闭时返回配置方式且不读凭证。
/// 策略读取会访问 MySQL；凭证缺失、未绑定或验证失败立即拒绝，TOTP 成功另写验证时间。
/// 结果不是可携带或一次性消费的授权证明，也不与后续资金事务绑定，调用方须对每次动作现场调用。
pub async fn verify_user_security_action(
    pool: &Pool<MySql>,
    settings: &Settings,
    user_id: u64,
    action: SecurityAction,
    input: SecurityVerificationInput<'_>,
) -> AppResult<SecurityVerificationMethod> {
    let policy = load_security_policy(pool).await?;
    let action_policy = policy.payment_policies.policy_for(action);
    if !action_policy.enabled {
        return Ok(action_policy.method);
    }

    if action_policy.method.requires_fund_password() {
        let password = required_security_field(input.fund_password)?;
        let hash = load_user_fund_password_hash(pool, user_id)
            .await?
            .ok_or_else(|| {
                AppError::security_validation("fund_password_required_not_set", "请先设置资金密码")
            })?;
        if !verify_password(&hash, password)? {
            return Err(AppError::Unauthorized);
        }
    }

    if action_policy.method.requires_two_factor() {
        let code = required_security_field(input.totp_code)?;
        verify_user_totp(pool, settings, user_id, code).await?;
    }

    Ok(action_policy.method)
}

/// 读取用户已启用的加密 TOTP 密钥，解密并校验六位动态码。
/// 成功后另发 SQL 更新最后验证时间；缺少绑定、密钥或动态码错误不更新该时间。
/// 解密明文不出当前调用，时间写入不与后续资金或登录动作共用事务。
pub async fn verify_user_totp(
    pool: &Pool<MySql>,
    settings: &Settings,
    user_id: u64,
    code: &str,
) -> AppResult<()> {
    let two_factor = load_user_two_factor(pool, user_id).await?;
    let encrypted_secret = two_factor
        .totp_secret_encrypted
        .filter(|_| two_factor.totp_enabled)
        .ok_or_else(|| AppError::security_validation("2fa_required_not_bound", "请先绑定 2FA"))?;
    let secret = decrypt_secret(&encrypted_secret, credential_encryption_key(settings)?)?;
    if !verify_totp_code(&secret, code, Utc::now())? {
        return Err(AppError::security_validation(
            "invalid_2fa_code",
            "2FA 验证码错误",
        ));
    }

    // TOTP 通过后只记录最后验证时间，不在应用层直接拼 SQL。
    record_user_totp_verified(pool, user_id).await?;

    Ok(())
}
