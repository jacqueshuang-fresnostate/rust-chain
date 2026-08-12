//! user bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。

use crate::{
    error::{AppError, AppResult},
    modules::user::{
        presentation::{
            MyInviteUserResponse, ReferralBindingResponse, ReferralCodeResponse,
            ThirdPartyBindingResponse, UserProfileResponse,
        },
        repository::{
            EmailVerificationRecord, InviteCodeRecord, ReferralLinkRecord, UserPasswordRecord,
        },
    },
};
use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use sqlx::{MySql, Pool, Transaction, types::Json as SqlxJson};

#[derive(Debug, sqlx::FromRow)]
struct UserProfileRow {
    id: u64,
    username: Option<String>,
    email: Option<String>,
    phone: Option<String>,
    avatar_url: Option<String>,
    country_code: Option<String>,
    preferred_locale: Option<String>,
    default_locale: Option<String>,
    supported_locales: Option<SqlxJson<Vec<String>>>,
    status: String,
    kyc_level: i32,
    email_verified_at: Option<DateTime<Utc>>,
    fund_password_set: bool,
    created_at: DateTime<Utc>,
}

impl From<UserProfileRow> for UserProfileResponse {
    fn from(row: UserProfileRow) -> Self {
        Self {
            id: row.id,
            username: row.username,
            email: row.email,
            phone: row.phone,
            avatar_url: row.avatar_url,
            country_code: row.country_code,
            preferred_locale: row.preferred_locale,
            default_locale: row.default_locale,
            supported_locales: row.supported_locales.map(|value| value.0),
            status: row.status,
            kyc_level: row.kyc_level,
            email_verified_at: row.email_verified_at,
            fund_password_set: row.fund_password_set,
            created_at: row.created_at,
        }
    }
}

/// 读取用户基本资料、国家本地化配置与资金密码设置标志，不返回任何哈希。
/// 用户不存在时按未授权处理；左连缺失的国家或安全记录保留空值，查询只读。
pub(crate) async fn load_user_profile(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<UserProfileResponse> {
    let profile = sqlx::query_as::<_, UserProfileRow>(
        r#"SELECT users.id, users.username, users.email, users.phone, users.avatar_url,
                  users.country_code, users.preferred_locale,
                  countries.default_locale, countries.supported_locales,
                  users.status, users.kyc_level, users.email_verified_at,
                  CASE WHEN security.fund_password_hash IS NULL THEN FALSE ELSE TRUE END AS fund_password_set,
                  users.created_at
           FROM users
           LEFT JOIN user_security security ON security.user_id = users.id
           LEFT JOIN country_configs countries ON countries.country_code = users.country_code
           WHERE users.id = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    Ok(profile.into())
}

/// 以不加锁查询确认用户主键存在，缺失时返回未授权且无写入。
pub(crate) async fn ensure_user_exists(pool: &Pool<MySql>, user_id: u64) -> AppResult<()> {
    sqlx::query_as::<_, (u64,)>("SELECT id FROM users WHERE id = ? LIMIT 1")
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::Unauthorized)?;
    Ok(())
}

/// 在调用方事务中锁定用户主键，防止后续用户副作用期间并发变更。
pub(crate) async fn ensure_user_exists_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<()> {
    sqlx::query_as::<_, (u64,)>("SELECT id FROM users WHERE id = ? LIMIT 1 FOR UPDATE")
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::Unauthorized)?;
    Ok(())
}

/// 将已上传头像的 URL 写入用户资料，零行受影响按未授权处理。
pub(crate) async fn update_user_avatar_url(
    pool: &Pool<MySql>,
    user_id: u64,
    avatar_url: &str,
) -> AppResult<()> {
    let result = sqlx::query("UPDATE users SET avatar_url = ? WHERE id = ?")
        .bind(avatar_url)
        .bind(user_id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::Unauthorized);
    }
    Ok(())
}

/// 锁定活跃用户当前用户名，供唯一更新与审计快照共用。
pub(crate) async fn lock_active_user_username_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<Option<String>> {
    sqlx::query_scalar::<_, Option<String>>(
        "SELECT username FROM users WHERE id = ? AND status = 'active' LIMIT 1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::Unauthorized)
}

/// 锁定用户登录密码哈希与状态，供改密时防止并发覆盖凭证。
pub(crate) async fn lock_user_password_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<UserPasswordRecord> {
    let row = sqlx::query_as::<_, (u64, String, String)>(
        r#"SELECT id, password_hash, status
           FROM users
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::Unauthorized)?;
    Ok(UserPasswordRecord {
        id: row.0,
        password_hash: row.1,
        status: row.2,
    })
}

/// 在邀请或安全事务中锁定活跃用户，停用或缺失均按未授权处理。
pub(crate) async fn ensure_active_user_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<()> {
    sqlx::query_as::<_, (u64,)>(
        r#"SELECT id
           FROM users
           WHERE id = ? AND status = 'active'
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::Unauthorized)?;
    Ok(())
}

/// 在已锁定用户的改密事务中更新登录密码哈希，不自行提交。
pub(crate) async fn update_user_password_hash_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    password_hash: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(password_hash)
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 在改密事务中撤销用户全部未撤销 MySQL 刷新令牌，重复执行保持幂等。
pub(crate) async fn revoke_user_refresh_tokens_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE refresh_tokens
           SET revoked_at = CURRENT_TIMESTAMP(6)
           WHERE actor_type = 'user' AND actor_id = ? AND revoked_at IS NULL"#,
    )
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 按用户列出第三方账号绑定，以最近更新时间排序且不读取外部 provider。
pub(crate) async fn list_user_third_party_bindings(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Vec<ThirdPartyBindingResponse>> {
    sqlx::query_as::<_, ThirdPartyBindingResponse>(
        r#"SELECT provider, account_identifier, display_name, status, created_at, updated_at
           FROM user_third_party_bindings
           WHERE user_id = ?
           ORDER BY updated_at DESC, id DESC"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

/// 按用户读取最早创建的邀请码，并附带当前代理归属，无记录时返回 `None`。
pub(crate) async fn load_user_invite_code(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Option<ReferralCodeResponse>> {
    sqlx::query_as::<_, ReferralCodeResponse>(
        r#"SELECT codes.id, codes.owner_type, codes.owner_id, codes.code,
                  codes.usage_limit, codes.used_count, codes.status,
                  referrals.root_agent_id, codes.created_at
           FROM invite_codes codes
           LEFT JOIN user_referrals referrals ON referrals.user_id = codes.owner_id
           WHERE codes.owner_type = 'user' AND codes.owner_id = ?
           ORDER BY codes.id ASC
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

/// 更新旧用户邀请码或插入新记录，唯一键冲突返回 `false` 供应用层重试。
/// 指定旧主键但所有权不匹配时返回内部错误；本操作不改使用次数和邀请关系。
pub(crate) async fn write_user_invite_code(
    pool: &Pool<MySql>,
    user_id: u64,
    existing_code_id: Option<u64>,
    code: &str,
) -> AppResult<bool> {
    let result = if let Some(existing_code_id) = existing_code_id {
        sqlx::query(
            r#"UPDATE invite_codes
               SET code = ?
               WHERE id = ? AND owner_type = 'user' AND owner_id = ?"#,
        )
        .bind(code)
        .bind(existing_code_id)
        .bind(user_id)
        .execute(pool)
        .await
    } else {
        sqlx::query(
            r#"INSERT INTO invite_codes (owner_type, owner_id, code, status)
               VALUES ('user', ?, ?, 'active')"#,
        )
        .bind(user_id)
        .bind(code)
        .execute(pool)
        .await
    };

    match result {
        Ok(result) if result.rows_affected() > 0 => Ok(true),
        Ok(_) => Err(AppError::Internal(
            "failed to update user invite code".to_owned(),
        )),
        Err(error) if is_duplicate_key(&error) => Ok(false),
        Err(error) => Err(AppError::from(error)),
    }
}

/// 锁定用户现有邀请绑定，供首次绑定用例幂等返回且防止并发双绑。
pub(crate) async fn lock_user_referral_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<Option<ReferralBindingResponse>> {
    sqlx::query_as::<_, ReferralBindingResponse>(
        r#"SELECT user_id, direct_inviter_id, direct_inviter_type,
                  root_agent_id, depth, path, created_at,
                  true AS bound
           FROM user_referrals
           WHERE user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

/// 在当前事务快照中读取用户邀请绑定，缺失时返回未找到。
pub(crate) async fn load_user_referral_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<ReferralBindingResponse> {
    sqlx::query_as::<_, ReferralBindingResponse>(
        r#"SELECT user_id, direct_inviter_id, direct_inviter_type,
                  root_agent_id, depth, path, created_at,
                  true AS bound
           FROM user_referrals
           WHERE user_id = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 按物化路径依次锁定直属代理及所有祖先，任一停用都拒绝新用户归属。
/// 锁顺序固定为层级与 ID 升序；结构缺失或 SQL 失败由邀请事务整体回滚。
pub(crate) async fn ensure_active_agent_in_tx(
    tx: &mut Transaction<'_, MySql>,
    agent_id: u64,
) -> AppResult<()> {
    let (path,) = sqlx::query_as::<_, (String,)>(
        r#"SELECT path
           FROM agents
           WHERE id = ? AND status = 'active'
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(agent_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Validation("agent is inactive or not found".to_owned()))?;

    // 直属代理仍为 active 也不够，任一上级停用后整家下级公司都不能继续发展用户。
    let ancestor_statuses = sqlx::query_scalar::<_, String>(
        r#"SELECT status
           FROM agents
           WHERE path = ? OR ? LIKE CONCAT(path, '/%')
           ORDER BY level ASC, id ASC
           FOR UPDATE"#,
    )
    .bind(&path)
    .bind(&path)
    .fetch_all(&mut **tx)
    .await?;
    if ancestor_statuses.is_empty() || ancestor_statuses.iter().any(|status| status != "active") {
        return Err(AppError::Validation(
            "agent hierarchy is inactive or invalid".to_owned(),
        ));
    }
    Ok(())
}

/// 锁定邀请人的代理归属、深度和路径，供用户邀请时派生下一层关系。
pub(crate) async fn load_referral_link_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<ReferralLinkRecord> {
    let row = sqlx::query_as::<_, (Option<u64>, i32, String)>(
        r#"SELECT root_agent_id, depth, path
           FROM user_referrals
           WHERE user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Validation("inviter has not bound an agent".to_owned()))?;
    Ok(ReferralLinkRecord {
        root_agent_id: row.0,
        depth: row.1,
        path: row.2,
    })
}

/// 以 `FOR UPDATE` 锁定启用邀请码的所有者和使用计数，防止并发突破用量。
pub(crate) async fn lock_active_invite_code_in_tx(
    tx: &mut Transaction<'_, MySql>,
    code: &str,
) -> AppResult<InviteCodeRecord> {
    let row = sqlx::query_as::<_, (u64, String, u64, Option<i32>, i32)>(
        r#"SELECT id, owner_type, owner_id, usage_limit, used_count
           FROM invite_codes
           WHERE code = ? AND status = 'active'
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(code)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Validation("invite code is inactive or not found".to_owned()))?;
    Ok(InviteCodeRecord {
        id: row.0,
        owner_type: row.1,
        owner_id: row.2,
        usage_limit: row.3,
        used_count: row.4,
    })
}

/// 在调用方事务中写入用户的直属邀请人、公司归属、深度和物化路径。
/// 用户主键唯一性阻止重复绑定；本函数不自行累加邀请码计数或提交。
pub(crate) async fn insert_user_referral_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    direct_inviter_id: u64,
    direct_inviter_type: &str,
    root_agent_id: Option<u64>,
    depth: i32,
    path: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO user_referrals
              (user_id, direct_inviter_id, direct_inviter_type, root_agent_id, depth, path)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(direct_inviter_id)
    .bind(direct_inviter_type)
    .bind(root_agent_id)
    .bind(depth)
    .bind(path)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在邀请绑定同一事务内累加邀请码已用次数，失败须与绑定一起回滚。
pub(crate) async fn increment_invite_code_used_count_in_tx(
    tx: &mut Transaction<'_, MySql>,
    invite_code_id: u64,
) -> AppResult<()> {
    sqlx::query("UPDATE invite_codes SET used_count = used_count + 1 WHERE id = ?")
        .bind(invite_code_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 最多读取一百个由当前用户直接邀请的成员，不展开间接后代。
pub(crate) async fn list_direct_invited_users(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Vec<MyInviteUserResponse>> {
    sqlx::query_as::<_, MyInviteUserResponse>(
        r#"SELECT referrals.user_id, users.email, users.phone, users.status,
                  referrals.direct_inviter_type, referrals.direct_inviter_id,
                  referrals.root_agent_id, referrals.depth, referrals.path,
                  referrals.created_at
           FROM user_referrals referrals
           INNER JOIN users ON users.id = referrals.user_id
           WHERE referrals.direct_inviter_type = 'user'
             AND referrals.direct_inviter_id = ?
           ORDER BY referrals.created_at ASC, referrals.user_id ASC
           LIMIT 100"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

/// 按用户与 provider 幂等插入或更新第三方绑定，每次成功均恢复为 `bound`。
/// 本函数不校验外部账号归属；持久化与审计必须由应用层放在同一事务。
pub(crate) async fn upsert_user_third_party_binding_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    provider: &str,
    account_identifier: &str,
    display_name: &Option<String>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO user_third_party_bindings
              (user_id, provider, account_identifier, display_name, status)
           VALUES (?, ?, ?, ?, 'bound')
           ON DUPLICATE KEY UPDATE
              account_identifier = VALUES(account_identifier),
              display_name = VALUES(display_name),
              status = 'bound'"#,
    )
    .bind(user_id)
    .bind(provider)
    .bind(account_identifier)
    .bind(display_name)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在已锁定用户的事务中更新登录用户名，唯一键冲突映射为明确业务冲突。
pub(crate) async fn update_user_username_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    username: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE users SET username = ? WHERE id = ?")
        .bind(username)
        .bind(user_id)
        .execute(&mut **tx)
        .await
        .map_err(map_duplicate_username)?;
    Ok(())
}

/// 锁定用户安全行并返回可选资金密码哈希，无安全行或未设置均返回 `None`。
pub(crate) async fn lock_fund_password_hash_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<Option<String>> {
    sqlx::query_scalar(
        r#"SELECT fund_password_hash
           FROM user_security
           WHERE user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await
    .map(|value: Option<Option<String>>| value.flatten())
    .map_err(AppError::from)
}

/// 复用安全行锁确认资金密码已设置，缺失时返回未找到。
pub(crate) async fn ensure_fund_password_exists_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<()> {
    lock_fund_password_hash_in_tx(tx, user_id)
        .await?
        .map(|_| ())
        .ok_or(AppError::NotFound)
}

/// 为用户幂等创建或替换资金密码哈希，适用于首次设置与邮件码重置。
pub(crate) async fn upsert_fund_password_hash_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    fund_password_hash: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO user_security (user_id, fund_password_hash)
           VALUES (?, ?)
           ON DUPLICATE KEY UPDATE fund_password_hash = VALUES(fund_password_hash)"#,
    )
    .bind(user_id)
    .bind(fund_password_hash)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 仅更新已存用户安全行的资金密码哈希，不创建缺失记录或自行提交。
pub(crate) async fn update_fund_password_hash_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    fund_password_hash: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE user_security SET fund_password_hash = ? WHERE user_id = ?")
        .bind(fund_password_hash)
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 检查待绑定邮箱未被其他用户占用，当前用户的相同邮箱不视为冲突。
pub(crate) async fn ensure_email_available_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    email: &str,
) -> AppResult<()> {
    let existing_user_id: Option<u64> = sqlx::query_scalar(
        r#"SELECT id
           FROM users
           WHERE email = ? AND id <> ?
           LIMIT 1"#,
    )
    .bind(email)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?;
    if existing_user_id.is_some() {
        return Err(AppError::Conflict("email already exists".to_owned()));
    }
    Ok(())
}

/// 检查用户、邮箱和用途下最新待验证码的可配置发送冷却期。
/// 冷却期内返回校验错误且不插入新码；本查询在调用方事务中执行。
pub(crate) async fn ensure_email_verification_not_cooling_down_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    email: &str,
    purpose: &str,
    now: DateTime<Utc>,
    cooldown_seconds: i64,
) -> AppResult<()> {
    let sent_at: Option<DateTime<Utc>> = sqlx::query_scalar(
        r#"SELECT sent_at
           FROM user_email_verifications
           WHERE user_id = ? AND email = ? AND purpose = ? AND status = 'pending'
           ORDER BY id DESC
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(email)
    .bind(purpose)
    .fetch_optional(&mut **tx)
    .await?;
    if sent_at.is_some_and(|sent_at| sent_at + Duration::seconds(cooldown_seconds) > now) {
        return Err(AppError::Validation(
            "email verification code was sent recently".to_owned(),
        ));
    }
    Ok(())
}

/// 将同用户同用途的所有待验证邮件码标记为已取代，防止发新码后旧码重放。
pub(crate) async fn supersede_pending_email_verifications_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    purpose: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE user_email_verifications
           SET status = 'superseded'
           WHERE user_id = ? AND purpose = ? AND status = 'pending'"#,
    )
    .bind(user_id)
    .bind(purpose)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 插入待验证邮件码的哈希、用途、过期与发送时间，明文验证码不入库。
pub(crate) async fn insert_pending_email_verification_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    email: &str,
    purpose: &str,
    code_hash: &str,
    expires_at: DateTime<Utc>,
    sent_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO user_email_verifications
           (user_id, email, purpose, code_hash, status, expires_at, sent_at)
           VALUES (?, ?, ?, ?, 'pending', ?, ?)"#,
    )
    .bind(user_id)
    .bind(email)
    .bind(purpose)
    .bind(code_hash)
    .bind(expires_at.naive_utc())
    .bind(sent_at.naive_utc())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 以 `FOR UPDATE` 锁定最新待验证邮件码，供试错计数、过期判断与消费共用。
pub(crate) async fn lock_latest_pending_email_verification_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    email: &str,
    purpose: &str,
) -> AppResult<Option<EmailVerificationRecord>> {
    let row = sqlx::query_as::<_, (u64, String, i32, DateTime<Utc>)>(
        r#"SELECT id, code_hash, attempt_count, expires_at
           FROM user_email_verifications
           WHERE user_id = ? AND email = ? AND purpose = ? AND status = 'pending'
           ORDER BY id DESC
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(email)
    .bind(purpose)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.map(
        |(id, code_hash, attempt_count, expires_at)| EmailVerificationRecord {
            id,
            code_hash,
            attempt_count,
            expires_at,
        },
    ))
}

/// 在调用方事务中锁定活跃用户已经验证的邮箱，防止安全重置码发送期间账号资料并发变化。
/// 用户停用、邮箱缺失或尚未验证时返回校验错误；本函数不发送邮件，也不提交事务。
pub(crate) async fn lock_verified_user_email_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<String> {
    let email: Option<String> = sqlx::query_scalar(
        r#"SELECT email
           FROM users
           WHERE id = ? AND status = 'active' AND email_verified_at IS NOT NULL
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .flatten();
    email.ok_or_else(|| AppError::Validation("verified email is required".to_owned()))
}

/// 在错码分支累加验证尝试次数，调用方对该校验错误仍需提交计数。
pub(crate) async fn increment_email_verification_attempt_count_in_tx(
    tx: &mut Transaction<'_, MySql>,
    verification_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE user_email_verifications
           SET attempt_count = attempt_count + 1
           WHERE id = ?"#,
    )
    .bind(verification_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在验证成功事务中更新用户邮箱与已验证时间，唯一冲突映射为邮箱已存在。
pub(crate) async fn update_user_bound_email_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    email: &str,
    verified_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE users
           SET email = ?, email_verified_at = ?
           WHERE id = ?"#,
    )
    .bind(email)
    .bind(verified_at.naive_utc())
    .bind(user_id)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_email)?;
    Ok(())
}

/// 将指定邮件码标记为已验证并保存消费时间，供同事务中完成凭证变更。
pub(crate) async fn mark_email_verification_verified_in_tx(
    tx: &mut Transaction<'_, MySql>,
    verification_id: u64,
    verified_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE user_email_verifications
           SET status = 'verified', verified_at = ?
           WHERE id = ?"#,
    )
    .bind(verified_at.naive_utc())
    .bind(verification_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在业务事务中写入用户作为主体的前后 JSON 审计快照，不独立提交。
/// 调用方必须先移除密码、密钥和证件原文；审计写入失败须使业务变更一起回滚。
pub(crate) async fn insert_user_audit_event_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    action: &'static str,
    target_type: &'static str,
    target_id: String,
    before_json: Option<Value>,
    after_json: Option<Value>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO audit_events
           (actor_type, actor_id, action, target_type, target_id, before_json, after_json)
           VALUES ('user', ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(before_json.map(SqlxJson))
    .bind(after_json.map(SqlxJson))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 按用户名、邮箱、手机的优先级读取 TOTP 账号标签，均缺失时返回 `None`。
pub(crate) async fn load_user_account_label(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Option<String>> {
    let label = sqlx::query_as::<_, (Option<String>, Option<String>, Option<String>)>(
        "SELECT username, email, phone FROM users WHERE id = ? LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .and_then(|(username, email, phone)| username.or(email).or(phone));
    Ok(label)
}

fn map_duplicate_username(error: sqlx::Error) -> AppError {
    if is_duplicate_key(&error) {
        AppError::Conflict("username already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}

fn map_duplicate_email(error: sqlx::Error) -> AppError {
    if is_duplicate_key(&error) {
        AppError::Conflict("email already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}

fn is_duplicate_key(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(database_error) if database_error.code().as_deref() == Some("1062"))
}
