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
                LoginTwoFactorChallenge, LoginTwoFactorChallengeType, SecurityAction,
                SecurityVerificationInput, SecurityVerificationMethod, login_challenge_expired,
                required_security_field, verify_totp_code,
            },
            infrastructure::{
                load_security_policy, load_user_fund_password_hash, load_user_two_factor,
                record_user_totp_verified,
            },
        },
    },
};
use chrono::Utc;
use sqlx::{MySql, Pool};

#[derive(Debug)]
pub struct SecurityVerificationUseCase;

impl ApplicationLayer for SecurityVerificationUseCase {}

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
    let key = settings
        .exposed_credential_encryption_key()
        .ok_or_else(|| {
            AppError::Internal("credential encryption key is not configured".to_owned())
        })?;
    let secret = decrypt_secret(&encrypted_secret, key)?;
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
