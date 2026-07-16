//! security bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。

use crate::{
    architecture::InfrastructureLayer,
    error::AppResult,
    modules::security::domain::{
        CreatedLoginTwoFactorChallenge, LoginTwoFactorChallenge, LoginTwoFactorChallengeType,
        UserSecurityPolicy, UserTwoFactorSettings, decode_security_policy_value,
        login_challenge_expired,
    },
};
use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use sqlx::{MySql, Pool, types::Json as SqlxJson};
use uuid::Uuid;

pub const USER_SECURITY_POLICY_KEY: &str = "user_security_policy";
pub const LOGIN_CHALLENGE_TTL_SECONDS: i64 = 300;

#[derive(Debug)]
pub struct SecurityRepository;

impl InfrastructureLayer for SecurityRepository {}

pub async fn load_security_policy(pool: &Pool<MySql>) -> AppResult<UserSecurityPolicy> {
    let policy = sqlx::query_scalar::<_, SqlxJson<Value>>(
        r#"SELECT policy_value
           FROM security_policy_configs
           WHERE policy_key = ?
           LIMIT 1"#,
    )
    .bind(USER_SECURITY_POLICY_KEY)
    .fetch_optional(pool)
    .await?;

    policy
        .map(|value| decode_security_policy_value(value.0))
        .transpose()
        .map(|policy| policy.unwrap_or_default())
}

pub async fn save_security_policy(
    pool: &Pool<MySql>,
    policy: &UserSecurityPolicy,
    admin_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO security_policy_configs (policy_key, policy_value, updated_by)
           VALUES (?, ?, ?)
           ON DUPLICATE KEY UPDATE
               policy_value = VALUES(policy_value),
               updated_by = VALUES(updated_by)"#,
    )
    .bind(USER_SECURITY_POLICY_KEY)
    .bind(SqlxJson(policy.clone()))
    .bind(admin_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn load_user_two_factor(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<UserTwoFactorSettings> {
    let settings = sqlx::query_as::<_, UserTwoFactorSettings>(
        r#"SELECT user_id, totp_secret_encrypted, totp_enabled, login_2fa_enabled,
                  confirmed_at, last_verified_at
           FROM user_two_factor_settings
           WHERE user_id = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(settings.unwrap_or_else(|| UserTwoFactorSettings::empty(user_id)))
}

pub async fn save_pending_totp_secret(
    pool: &Pool<MySql>,
    user_id: u64,
    encrypted_secret: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO user_two_factor_settings
              (user_id, totp_secret_encrypted, totp_enabled, login_2fa_enabled, confirmed_at, last_verified_at)
           VALUES (?, ?, FALSE, FALSE, NULL, NULL)
           ON DUPLICATE KEY UPDATE
              totp_secret_encrypted = VALUES(totp_secret_encrypted),
              totp_enabled = FALSE,
              login_2fa_enabled = FALSE,
              confirmed_at = NULL,
              last_verified_at = NULL"#,
    )
    .bind(user_id)
    .bind(encrypted_secret)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn confirm_user_totp(
    pool: &Pool<MySql>,
    user_id: u64,
    encrypted_secret: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO user_two_factor_settings
              (user_id, totp_secret_encrypted, totp_enabled, login_2fa_enabled, confirmed_at, last_verified_at)
           VALUES (?, ?, TRUE, FALSE, CURRENT_TIMESTAMP(6), CURRENT_TIMESTAMP(6))
           ON DUPLICATE KEY UPDATE
              totp_secret_encrypted = VALUES(totp_secret_encrypted),
              totp_enabled = TRUE,
              confirmed_at = CURRENT_TIMESTAMP(6),
              last_verified_at = CURRENT_TIMESTAMP(6)"#,
    )
    .bind(user_id)
    .bind(encrypted_secret)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn set_user_login_two_factor(
    pool: &Pool<MySql>,
    user_id: u64,
    enabled: bool,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE user_two_factor_settings
           SET login_2fa_enabled = ?
           WHERE user_id = ? AND totp_enabled = TRUE"#,
    )
    .bind(enabled)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn reset_user_two_factor(pool: &Pool<MySql>, user_id: u64) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO user_two_factor_settings
              (user_id, totp_secret_encrypted, totp_enabled, login_2fa_enabled, confirmed_at, last_verified_at)
           VALUES (?, NULL, FALSE, FALSE, NULL, NULL)
           ON DUPLICATE KEY UPDATE
              totp_secret_encrypted = NULL,
              totp_enabled = FALSE,
              login_2fa_enabled = FALSE,
              confirmed_at = NULL,
              last_verified_at = NULL"#,
    )
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn create_login_two_factor_challenge(
    pool: &Pool<MySql>,
    user_id: u64,
    challenge_type: LoginTwoFactorChallengeType,
) -> AppResult<CreatedLoginTwoFactorChallenge> {
    let challenge_id = Uuid::now_v7().to_string();
    let expires_at = Utc::now() + Duration::seconds(LOGIN_CHALLENGE_TTL_SECONDS);
    sqlx::query(
        r#"INSERT INTO login_two_factor_challenges
              (challenge_id, user_id, challenge_type, expires_at)
           VALUES (?, ?, ?, ?)"#,
    )
    .bind(&challenge_id)
    .bind(user_id)
    .bind(challenge_type.as_str())
    .bind(expires_at.naive_utc())
    .execute(pool)
    .await?;

    Ok(CreatedLoginTwoFactorChallenge {
        challenge_id,
        expires_at,
        expires_in_seconds: LOGIN_CHALLENGE_TTL_SECONDS,
    })
}

pub async fn load_login_two_factor_challenge(
    pool: &Pool<MySql>,
    challenge_id: &str,
) -> AppResult<LoginTwoFactorChallenge> {
    let row = sqlx::query_as::<_, (String, u64, String, DateTime<Utc>, Option<DateTime<Utc>>)>(
        r#"SELECT challenge_id, user_id, challenge_type, expires_at, consumed_at
           FROM login_two_factor_challenges
           WHERE challenge_id = ?
           LIMIT 1"#,
    )
    .bind(challenge_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(login_challenge_expired)?;

    Ok(LoginTwoFactorChallenge {
        challenge_id: row.0,
        user_id: row.1,
        challenge_type: LoginTwoFactorChallengeType::from_storage(&row.2)?,
        expires_at: row.3,
        consumed_at: row.4,
    })
}

pub async fn consume_login_two_factor_challenge(
    pool: &Pool<MySql>,
    challenge_id: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE login_two_factor_challenges
           SET consumed_at = CURRENT_TIMESTAMP(6)
           WHERE challenge_id = ? AND consumed_at IS NULL"#,
    )
    .bind(challenge_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn load_user_fund_password_hash(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Option<String>> {
    let hash: Option<String> = sqlx::query_scalar(
        r#"SELECT fund_password_hash
           FROM user_security
           WHERE user_id = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .flatten();

    Ok(hash)
}

pub async fn record_user_totp_verified(pool: &Pool<MySql>, user_id: u64) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE user_two_factor_settings
           SET last_verified_at = CURRENT_TIMESTAMP(6)
           WHERE user_id = ?"#,
    )
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}
