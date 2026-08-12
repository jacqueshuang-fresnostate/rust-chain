//! security bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。

use crate::{
    architecture::InfrastructureLayer,
    error::AppResult,
    modules::security::domain::{
        AdminLoginTwoFactorChallenge, AdminTwoFactorSettings, CreatedLoginTwoFactorChallenge,
        LoginTwoFactorChallenge, LoginTwoFactorChallengeType, UserSecurityPolicy,
        UserTwoFactorSettings, decode_security_policy_value, login_challenge_expired,
    },
};
use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use sqlx::{FromRow, MySql, Pool, mysql::MySqlRow, types::Json as SqlxJson};
use uuid::Uuid;

pub const USER_SECURITY_POLICY_KEY: &str = "user_security_policy";
pub const LOGIN_CHALLENGE_TTL_SECONDS: i64 = 300;

#[derive(Debug)]
pub struct SecurityRepository;

impl InfrastructureLayer for SecurityRepository {}

#[derive(Debug, sqlx::FromRow)]
struct UserTwoFactorSettingsRow {
    user_id: u64,
    totp_secret_encrypted: Option<String>,
    totp_enabled: bool,
    login_2fa_enabled: bool,
    confirmed_at: Option<DateTime<Utc>>,
    last_verified_at: Option<DateTime<Utc>>,
}

impl From<UserTwoFactorSettingsRow> for UserTwoFactorSettings {
    fn from(row: UserTwoFactorSettingsRow) -> Self {
        Self {
            user_id: row.user_id,
            totp_secret_encrypted: row.totp_secret_encrypted,
            totp_enabled: row.totp_enabled,
            login_2fa_enabled: row.login_2fa_enabled,
            confirmed_at: row.confirmed_at,
            last_verified_at: row.last_verified_at,
        }
    }
}

// 管理后台的锁行查询仍以领域值作为返回类型；SQLx 适配实现留在基础设施层，
// 并统一经过持久化 row 到领域值的显式映射，避免领域模块导入 SQLx。
impl<'r> FromRow<'r, MySqlRow> for UserTwoFactorSettings {
    fn from_row(row: &'r MySqlRow) -> Result<Self, sqlx::Error> {
        UserTwoFactorSettingsRow::from_row(row).map(Into::into)
    }
}

#[derive(Debug, sqlx::FromRow)]
struct AdminTwoFactorSettingsRow {
    admin_id: u64,
    totp_secret_encrypted: Option<String>,
    totp_enabled: bool,
    confirmed_at: Option<DateTime<Utc>>,
    last_verified_at: Option<DateTime<Utc>>,
}

impl From<AdminTwoFactorSettingsRow> for AdminTwoFactorSettings {
    fn from(row: AdminTwoFactorSettingsRow) -> Self {
        Self {
            admin_id: row.admin_id,
            totp_secret_encrypted: row.totp_secret_encrypted,
            totp_enabled: row.totp_enabled,
            confirmed_at: row.confirmed_at,
            last_verified_at: row.last_verified_at,
        }
    }
}

/// 读取用户安全策略 JSON，通过领域解码兼容历史布尔值，缺失时返回安全默认。
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

/// 将规范安全策略序列化为 JSON 并按固定键幂等写入，不保留历史非布尔表示。
/// 本函数使用单语句提交；序列化或 SQL 失败时不返回虚假成功。
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

/// 读取用户二次验证设置，未建立记录时返回未绑定默认快照。
/// 命中记录会包含加密 TOTP 密钥，只可交给安全应用层解密，禁止直接序列化到响应或日志。
pub async fn load_user_two_factor(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<UserTwoFactorSettings> {
    let settings = sqlx::query_as::<_, UserTwoFactorSettingsRow>(
        r#"SELECT user_id, totp_secret_encrypted, totp_enabled, login_2fa_enabled,
                  confirmed_at, last_verified_at
           FROM user_two_factor_settings
           WHERE user_id = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(settings
        .map(Into::into)
        .unwrap_or_else(|| UserTwoFactorSettings::empty(user_id)))
}

/// 以 upsert 保存用户待确认的加密 TOTP 密钥，并清除启用标志、登录 2FA 开关及时间快照。
/// 输入必须已由应用层加密；若对已绑定用户直接调用也会关闭原绑定，故调用方须先执行未绑定策略检查。
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

/// 在动态码已由应用层验证后，以 upsert 写入所给密文并启用用户 TOTP。
/// SQL 不比较数据库中的待确认密钥，也不检查受影响行数；读取后发生并发替换时，后写请求可覆盖新密钥。
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

/// 仅对当前 `totp_enabled = TRUE` 的用户切换登录二次验证开关。
/// SQL 不检查受影响行数；用户不存在或未绑定时返回成功但不修改记录，调用方须先做状态校验。
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

/// 幂等清除用户 TOTP 密钥、启用标志与时间快照，同时关闭登录二次验证。
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

/// 为用户创建 UUIDv7 登录或首次绑定挑战，固定五分钟有效且初始未消费。
/// 每次调用会新增记录；唯一键或 SQL 失败直接上抛，不签发任何会话。
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

/// 按挑战 ID 读取用户、类型、过期与消费状态，未命中按挑战过期处理。
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

/// 条件更新用户登录挑战的消费时间，仅匹配尚未消费的记录。
/// 本函数不检查受影响行数，挑战不存在或重复消费也返回成功；需要严格防重放的调用方须自行判定。
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

/// 读取管理员二次验证设置，无记录时返回未绑定默认快照。
/// 命中记录含加密 TOTP 密钥，仅供当前管理员安全用例使用，不得透传到管理端 DTO。
pub async fn load_admin_two_factor(
    pool: &Pool<MySql>,
    admin_id: u64,
) -> AppResult<AdminTwoFactorSettings> {
    let settings = sqlx::query_as::<_, AdminTwoFactorSettingsRow>(
        r#"SELECT admin_id, totp_secret_encrypted, totp_enabled, confirmed_at, last_verified_at
           FROM admin_two_factor_settings
           WHERE admin_id = ?
           LIMIT 1"#,
    )
    .bind(admin_id)
    .fetch_optional(pool)
    .await?;

    Ok(settings
        .map(Into::into)
        .unwrap_or_else(|| AdminTwoFactorSettings::empty(admin_id)))
}

/// 以 upsert 保存管理员待确认的加密 TOTP 密钥，并清除原启用标志及时间快照。
/// 若绕过应用层未绑定检查直接调用，会关闭已有绑定；输入密文不得写入日志或响应。
pub async fn save_pending_admin_totp_secret(
    pool: &Pool<MySql>,
    admin_id: u64,
    encrypted_secret: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO admin_two_factor_settings
              (admin_id, totp_secret_encrypted, totp_enabled, confirmed_at, last_verified_at)
           VALUES (?, ?, FALSE, NULL, NULL)
           ON DUPLICATE KEY UPDATE
              totp_secret_encrypted = VALUES(totp_secret_encrypted),
              totp_enabled = FALSE,
              confirmed_at = NULL,
              last_verified_at = NULL"#,
    )
    .bind(admin_id)
    .bind(encrypted_secret)
    .execute(pool)
    .await?;

    Ok(())
}

/// 在验证码已由应用层验证后，以 upsert 写入所给密文并启用管理员 TOTP。
/// SQL 不比较当前待确认密钥且不检查受影响行数，不能单独防止读取后的并发覆盖；本函数不签发令牌。
pub async fn confirm_admin_totp(
    pool: &Pool<MySql>,
    admin_id: u64,
    encrypted_secret: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO admin_two_factor_settings
              (admin_id, totp_secret_encrypted, totp_enabled, confirmed_at, last_verified_at)
           VALUES (?, ?, TRUE, CURRENT_TIMESTAMP(6), CURRENT_TIMESTAMP(6))
           ON DUPLICATE KEY UPDATE
              totp_secret_encrypted = VALUES(totp_secret_encrypted),
              totp_enabled = TRUE,
              confirmed_at = CURRENT_TIMESTAMP(6),
              last_verified_at = CURRENT_TIMESTAMP(6)"#,
    )
    .bind(admin_id)
    .bind(encrypted_secret)
    .execute(pool)
    .await?;

    Ok(())
}

/// 幂等清除管理员 TOTP 密钥、启用标志及验证时间，已清除时重复调用不报错。
pub async fn reset_admin_two_factor(pool: &Pool<MySql>, admin_id: u64) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO admin_two_factor_settings
              (admin_id, totp_secret_encrypted, totp_enabled, confirmed_at, last_verified_at)
           VALUES (?, NULL, FALSE, NULL, NULL)
           ON DUPLICATE KEY UPDATE
              totp_secret_encrypted = NULL,
              totp_enabled = FALSE,
              confirmed_at = NULL,
              last_verified_at = NULL"#,
    )
    .bind(admin_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// 记录管理员最后一次 TOTP 校验通过时间，不更改密钥或启用状态。
/// SQL 不检查受影响行数；记录缺失或被并发清除时也返回成功且不补建设置。
pub async fn record_admin_totp_verified(pool: &Pool<MySql>, admin_id: u64) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE admin_two_factor_settings
           SET last_verified_at = CURRENT_TIMESTAMP(6)
           WHERE admin_id = ?"#,
    )
    .bind(admin_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// 为管理员密码验证后创建五分钟 TOTP 挑战，尝试次数初始为零且每次调用新增记录。
pub async fn create_admin_login_two_factor_challenge(
    pool: &Pool<MySql>,
    admin_id: u64,
) -> AppResult<CreatedLoginTwoFactorChallenge> {
    let challenge_id = Uuid::now_v7().to_string();
    let expires_at = Utc::now() + Duration::seconds(LOGIN_CHALLENGE_TTL_SECONDS);
    sqlx::query(
        r#"INSERT INTO admin_login_two_factor_challenges (challenge_id, admin_id, expires_at)
           VALUES (?, ?, ?)"#,
    )
    .bind(&challenge_id)
    .bind(admin_id)
    .bind(expires_at.naive_utc())
    .execute(pool)
    .await?;

    Ok(CreatedLoginTwoFactorChallenge {
        challenge_id,
        expires_at,
        expires_in_seconds: LOGIN_CHALLENGE_TTL_SECONDS,
    })
}

/// 按 ID 读取管理员登录挑战及尝试次数，未命中使用统一挑战过期语义。
pub async fn load_admin_login_two_factor_challenge(
    pool: &Pool<MySql>,
    challenge_id: &str,
) -> AppResult<AdminLoginTwoFactorChallenge> {
    let row = sqlx::query_as::<_, (String, u64, u32, DateTime<Utc>, Option<DateTime<Utc>>)>(
        r#"SELECT challenge_id, admin_id, attempt_count, expires_at, consumed_at
           FROM admin_login_two_factor_challenges
           WHERE challenge_id = ?
           LIMIT 1"#,
    )
    .bind(challenge_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(login_challenge_expired)?;

    Ok(AdminLoginTwoFactorChallenge {
        challenge_id: row.0,
        admin_id: row.1,
        attempt_count: row.2,
        expires_at: row.3,
        consumed_at: row.4,
    })
}

/// 对指定管理员挑战累加一次错误试码；SQL 未限制消费或过期状态且不检查受影响行数。
/// 调用方须先校验挑战快照，缺失记录也会以未修改任何行的成功结果返回。
pub async fn increment_admin_login_two_factor_attempt(
    pool: &Pool<MySql>,
    challenge_id: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE admin_login_two_factor_challenges
           SET attempt_count = attempt_count + 1
           WHERE challenge_id = ?"#,
    )
    .bind(challenge_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// 条件更新管理员挑战的消费时间，只修改尚未消费的记录。
/// 本函数不检查受影响行数，重复或不存在的挑战也返回成功，不能单独保证只有一个并发请求继续签发。
pub async fn consume_admin_login_two_factor_challenge(
    pool: &Pool<MySql>,
    challenge_id: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE admin_login_two_factor_challenges
           SET consumed_at = CURRENT_TIMESTAMP(6)
           WHERE challenge_id = ? AND consumed_at IS NULL"#,
    )
    .bind(challenge_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// 读取用户资金密码哈希，未设置时返回 `None`，不在基础设施层比对明文。
/// 哈希仍属敏感凭据，只可用于内存校验，不得进入响应、审计或普通日志。
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

/// 记录用户最后一次 TOTP 通过时间，不改变登录开关或密钥内容。
/// SQL 不检查受影响行数；设置行缺失时以零行更新的成功结果返回。
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
