//! auth bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。

use crate::{
    architecture::InfrastructureLayer,
    error::{AppError, AppResult},
    modules::{
        auth::{
            ActiveCountryConfig, ActorType, AuthActor, NewAdminActor, NewAgentActor, NewUserActor,
            RefreshTokenRecord, StoredActorCredential, StoredRefreshToken,
            domain::{normalize_invite_code, validate_email_code},
            repository::AuthRepository,
            verify_password,
        },
        user::service::generate_user_invite_code,
    },
};
use axum::async_trait;
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use sqlx::{MySql, Pool, Transaction};

#[derive(Clone)]
pub struct MySqlAuthRepository {
    pool: Pool<MySql>,
}

impl InfrastructureLayer for MySqlAuthRepository {}

impl MySqlAuthRepository {
    pub fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }

    async fn find_active_user(&self, actor: &AuthActor) -> AppResult<Option<AuthActor>> {
        let actor_id = sqlx::query_scalar::<_, u64>(
            "SELECT id FROM users WHERE id = ? AND status = 'active' LIMIT 1",
        )
        .bind(actor.actor_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(actor_id.map(|actor_id| AuthActor::new(ActorType::User, actor_id, Some(actor_id))))
    }

    async fn find_active_admin(&self, actor: &AuthActor) -> AppResult<Option<AuthActor>> {
        let actor_id = sqlx::query_scalar::<_, u64>(
            "SELECT id FROM admin_users WHERE id = ? AND status = 'active' LIMIT 1",
        )
        .bind(actor.actor_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(actor_id.map(|actor_id| AuthActor::new(ActorType::Admin, actor_id, None)))
    }

    async fn find_active_agent(&self, actor: &AuthActor) -> AppResult<Option<AuthActor>> {
        let actor_id = sqlx::query_scalar::<_, u64>(
            r#"SELECT agent_admin_users.id
               FROM agent_admin_users
               INNER JOIN agents ON agents.id = agent_admin_users.agent_id
               WHERE agent_admin_users.id = ?
                 AND agent_admin_users.status = 'active'
                 AND agents.status = 'active'
                 AND NOT EXISTS (
                     SELECT 1
                     FROM agents ancestors
                     WHERE (ancestors.path = agents.path
                        OR agents.path LIKE CONCAT(ancestors.path, '/%'))
                       AND ancestors.status <> 'active'
                 )
               LIMIT 1"#,
        )
        .bind(actor.actor_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(actor_id.map(|actor_id| AuthActor::new(ActorType::Agent, actor_id, None)))
    }
}

#[async_trait]
impl AuthRepository for MySqlAuthRepository {
    async fn create_user(&self, actor: NewUserActor) -> AppResult<AuthActor> {
        let result = sqlx::query(
            r#"INSERT INTO users (email, phone, country_code, preferred_locale, password_hash)
               VALUES (?, ?, ?, ?, ?)"#,
        )
        .bind(actor.email)
        .bind(actor.phone)
        .bind(actor.country_code)
        .bind(actor.preferred_locale)
        .bind(actor.password_hash)
        .execute(&self.pool)
        .await
        .map_err(|error| map_duplicate_key(error, "user"))?;
        let actor_id = result.last_insert_id();

        Ok(AuthActor::new(ActorType::User, actor_id, Some(actor_id)))
    }

    async fn create_admin(&self, actor: NewAdminActor) -> AppResult<AuthActor> {
        let result = sqlx::query(
            "INSERT INTO admin_users (username, password_hash, role_id) VALUES (?, ?, ?)",
        )
        .bind(actor.username)
        .bind(actor.password_hash)
        .bind(actor.role_id)
        .execute(&self.pool)
        .await
        .map_err(|error| map_duplicate_key(error, "admin"))?;

        Ok(AuthActor::new(
            ActorType::Admin,
            result.last_insert_id(),
            None,
        ))
    }

    async fn create_agent(&self, actor: NewAgentActor) -> AppResult<AuthActor> {
        let result = sqlx::query(
            "INSERT INTO agent_admin_users (agent_id, username, password_hash) VALUES (?, ?, ?)",
        )
        .bind(actor.agent_id)
        .bind(actor.username)
        .bind(actor.password_hash)
        .execute(&self.pool)
        .await
        .map_err(|error| map_duplicate_key(error, "agent"))?;

        Ok(AuthActor::new(
            ActorType::Agent,
            result.last_insert_id(),
            None,
        ))
    }

    async fn find_registration_country(
        &self,
        country_code: &str,
    ) -> AppResult<Option<ActiveCountryConfig>> {
        let row = sqlx::query_as::<_, (String, String)>(
            r#"SELECT country_code, default_locale
               FROM country_configs
               WHERE country_code = ? AND registration_enabled = TRUE AND status = 'active'
               LIMIT 1"#,
        )
        .bind(country_code)
        .fetch_optional(&self.pool)
        .await?;

        Ok(
            row.map(|(country_code, default_locale)| ActiveCountryConfig {
                country_code,
                default_locale,
            }),
        )
    }

    async fn find_user_by_email(&self, email: &str) -> AppResult<Option<StoredActorCredential>> {
        let row = sqlx::query_as::<_, (u64, String, String)>(
            "SELECT id, password_hash, status FROM users WHERE email = ? LIMIT 1",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;

        Ok(
            row.map(|(actor_id, password_hash, status)| StoredActorCredential {
                actor: AuthActor::new(ActorType::User, actor_id, Some(actor_id)),
                password_hash,
                status,
            }),
        )
    }

    async fn find_user_by_phone(&self, phone: &str) -> AppResult<Option<StoredActorCredential>> {
        let row = sqlx::query_as::<_, (u64, String, String)>(
            "SELECT id, password_hash, status FROM users WHERE phone = ? LIMIT 1",
        )
        .bind(phone)
        .fetch_optional(&self.pool)
        .await?;

        Ok(
            row.map(|(actor_id, password_hash, status)| StoredActorCredential {
                actor: AuthActor::new(ActorType::User, actor_id, Some(actor_id)),
                password_hash,
                status,
            }),
        )
    }

    async fn find_user_by_username(
        &self,
        username: &str,
    ) -> AppResult<Option<StoredActorCredential>> {
        let row = sqlx::query_as::<_, (u64, String, String)>(
            "SELECT id, password_hash, status FROM users WHERE username = ? LIMIT 1",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;

        Ok(
            row.map(|(actor_id, password_hash, status)| StoredActorCredential {
                actor: AuthActor::new(ActorType::User, actor_id, Some(actor_id)),
                password_hash,
                status,
            }),
        )
    }

    async fn find_admin_by_username(
        &self,
        username: &str,
    ) -> AppResult<Option<StoredActorCredential>> {
        let row = sqlx::query_as::<_, (u64, String, String)>(
            "SELECT id, password_hash, status FROM admin_users WHERE username = ? LIMIT 1",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;

        Ok(
            row.map(|(actor_id, password_hash, status)| StoredActorCredential {
                actor: AuthActor::new(ActorType::Admin, actor_id, None),
                password_hash,
                status,
            }),
        )
    }

    async fn find_agent_by_username(
        &self,
        username: &str,
    ) -> AppResult<Option<StoredActorCredential>> {
        let row = sqlx::query_as::<_, (u64, String, String)>(
            r#"SELECT agent_admin_users.id, agent_admin_users.password_hash, agent_admin_users.status
               FROM agent_admin_users
               INNER JOIN agents ON agents.id = agent_admin_users.agent_id
               WHERE agent_admin_users.username = ?
                 AND agents.status = 'active'
                 AND NOT EXISTS (
                     SELECT 1
                     FROM agents ancestors
                     WHERE (ancestors.path = agents.path
                        OR agents.path LIKE CONCAT(ancestors.path, '/%'))
                       AND ancestors.status <> 'active'
                 )
               LIMIT 1"#,
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;

        Ok(
            row.map(|(actor_id, password_hash, status)| StoredActorCredential {
                actor: AuthActor::new(ActorType::Agent, actor_id, None),
                password_hash,
                status,
            }),
        )
    }

    async fn find_active_actor(&self, actor: &AuthActor) -> AppResult<Option<AuthActor>> {
        match actor.actor_type {
            ActorType::User => self.find_active_user(actor).await,
            ActorType::Admin => self.find_active_admin(actor).await,
            ActorType::Agent => self.find_active_agent(actor).await,
        }
    }

    async fn record_login(&self, actor: &AuthActor) -> AppResult<()> {
        if actor.actor_type == ActorType::Agent {
            sqlx::query(
                "UPDATE agent_admin_users SET last_login_at = CURRENT_TIMESTAMP(6) WHERE id = ?",
            )
            .bind(actor.actor_id)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    async fn store_refresh_token(&self, token: StoredRefreshToken) -> AppResult<()> {
        sqlx::query(
            r#"INSERT INTO refresh_tokens (user_id, actor_type, actor_id, token_hash, expires_at)
               VALUES (?, ?, ?, ?, ?)
               ON DUPLICATE KEY UPDATE token_hash = token_hash"#,
        )
        .bind(token.user_id)
        .bind(token.actor_type.as_str())
        .bind(token.actor_id)
        .bind(token.token_hash)
        .bind(token.expires_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn find_refresh_token(
        &self,
        token_hash: &str,
        now: NaiveDateTime,
    ) -> AppResult<Option<RefreshTokenRecord>> {
        let row = sqlx::query_as::<_, (String, u64, Option<u64>)>(
            r#"SELECT actor_type, actor_id, user_id
               FROM refresh_tokens
               WHERE token_hash = ? AND revoked_at IS NULL AND expires_at > ?
               LIMIT 1"#,
        )
        .bind(token_hash)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|(actor_type, actor_id, user_id)| {
            let actor_type = ActorType::from_storage(&actor_type)?;
            Ok(RefreshTokenRecord {
                scope: actor_type.scope(),
                actor_type,
                actor_id,
                user_id,
            })
        })
        .transpose()
    }
}

#[derive(Debug)]
pub struct AuthRouteRepository;

impl InfrastructureLayer for AuthRouteRepository {}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct RegistrationCountryRow {
    pub(crate) country_code: String,
    pub(crate) default_locale: String,
}

#[derive(Debug)]
pub(crate) struct PreparedReferralBinding {
    invite_code_id: u64,
    direct_inviter_type: String,
    direct_inviter_id: u64,
    root_agent_id: Option<u64>,
    depth: i32,
    path_prefix: String,
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct EmailVerificationRow {
    pub(crate) id: u64,
    pub(crate) code_hash: String,
    pub(crate) attempt_count: i32,
    pub(crate) expires_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct RegistrationEmailVerificationRow {
    id: u64,
    code_hash: String,
    attempt_count: i32,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct InviteCodeRow {
    id: u64,
    owner_type: String,
    owner_id: u64,
    usage_limit: Option<i32>,
    used_count: i32,
}

#[derive(Debug, sqlx::FromRow)]
struct ReferralLinkRow {
    root_agent_id: Option<u64>,
    depth: i32,
    path: String,
}

pub(crate) async fn lock_registration_country_in_tx(
    tx: &mut Transaction<'_, MySql>,
    country_code: &str,
) -> AppResult<RegistrationCountryRow> {
    sqlx::query_as::<_, RegistrationCountryRow>(
        r#"SELECT country_code, default_locale
           FROM country_configs
           WHERE country_code = ? AND registration_enabled = TRUE AND status = 'active'
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(country_code)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| {
        AppError::Validation("country_code is not available for registration".to_owned())
    })
}

pub(crate) async fn ensure_registration_email_available_in_tx(
    tx: &mut Transaction<'_, MySql>,
    email: &str,
) -> AppResult<()> {
    let existing_user_id: Option<u64> =
        sqlx::query_scalar("SELECT id FROM users WHERE email = ? LIMIT 1")
            .bind(email)
            .fetch_optional(&mut **tx)
            .await?;
    if existing_user_id.is_some() {
        return Err(AppError::Conflict("email already exists".to_owned()));
    }
    Ok(())
}

pub(crate) async fn ensure_registration_email_not_cooling_down_in_tx(
    tx: &mut Transaction<'_, MySql>,
    email: &str,
    now: DateTime<Utc>,
) -> AppResult<()> {
    let sent_at: Option<DateTime<Utc>> = sqlx::query_scalar(
        r#"SELECT sent_at
           FROM user_registration_email_verifications
           WHERE email = ? AND purpose = 'register' AND status = 'pending'
           ORDER BY id DESC
           LIMIT 1"#,
    )
    .bind(email)
    .fetch_optional(&mut **tx)
    .await?;
    if sent_at.is_some_and(|sent_at| sent_at + Duration::seconds(60) > now) {
        return Err(AppError::Validation(
            "email verification code was sent recently".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) async fn supersede_pending_registration_email_codes_in_tx(
    tx: &mut Transaction<'_, MySql>,
    email: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE user_registration_email_verifications
           SET status = 'superseded'
           WHERE email = ? AND purpose = 'register' AND status = 'pending'"#,
    )
    .bind(email)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn insert_registration_email_verification_in_tx(
    tx: &mut Transaction<'_, MySql>,
    email: &str,
    code_hash: &str,
    expires_at: DateTime<Utc>,
    sent_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO user_registration_email_verifications
           (email, purpose, code_hash, status, expires_at, sent_at)
           VALUES (?, 'register', ?, 'pending', ?, ?)"#,
    )
    .bind(email)
    .bind(code_hash)
    .bind(expires_at.naive_utc())
    .bind(sent_at.naive_utc())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn verify_registration_email_code_in_tx(
    tx: &mut Transaction<'_, MySql>,
    email: &str,
    code: &str,
    now: DateTime<Utc>,
) -> AppResult<()> {
    let code = validate_email_code(code)?;
    let verification = sqlx::query_as::<_, RegistrationEmailVerificationRow>(
        r#"SELECT id, code_hash, attempt_count, expires_at
           FROM user_registration_email_verifications
           WHERE email = ? AND purpose = 'register' AND status = 'pending'
           ORDER BY id DESC
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(email)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Validation("email verification code is invalid".to_owned()))?;

    if verification.expires_at <= now || verification.attempt_count >= 5 {
        return Err(AppError::Validation(
            "email verification code is expired".to_owned(),
        ));
    }
    if !verify_password(&verification.code_hash, &code)? {
        sqlx::query(
            r#"UPDATE user_registration_email_verifications
               SET attempt_count = attempt_count + 1
               WHERE id = ?"#,
        )
        .bind(verification.id)
        .execute(&mut **tx)
        .await?;
        return Err(AppError::Validation(
            "email verification code is invalid".to_owned(),
        ));
    }

    sqlx::query(
        r#"UPDATE user_registration_email_verifications
           SET status = 'verified', verified_at = ?
           WHERE id = ?"#,
    )
    .bind(now.naive_utc())
    .bind(verification.id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn prepare_referral_binding_in_tx(
    tx: &mut Transaction<'_, MySql>,
    code: &str,
) -> AppResult<PreparedReferralBinding> {
    let code = normalize_invite_code(code)?;
    let invite = lock_active_invite_code_in_tx(tx, &code).await?;
    if invite
        .usage_limit
        .is_some_and(|usage_limit| invite.used_count >= usage_limit)
    {
        return Err(AppError::Validation("invite code is exhausted".to_owned()));
    }

    let (direct_inviter_type, direct_inviter_id, root_agent_id, depth, path_prefix) =
        match invite.owner_type.as_str() {
            "agent" => {
                ensure_active_agent_in_tx(tx, invite.owner_id).await?;
                (
                    "agent".to_owned(),
                    invite.owner_id,
                    Some(invite.owner_id),
                    1,
                    format!("/agent:{}", invite.owner_id),
                )
            }
            "user" => {
                ensure_active_user_in_tx(tx, invite.owner_id).await?;
                let inviter = load_referral_link_in_tx(tx, invite.owner_id).await?;
                if let Some(owner_agent_id) = inviter.root_agent_id {
                    // 用户邀请码只改变直属邀请人，新用户仍归属邀请人的代理公司。
                    ensure_active_agent_in_tx(tx, owner_agent_id).await?;
                }
                (
                    "user".to_owned(),
                    invite.owner_id,
                    inviter.root_agent_id,
                    inviter.depth + 1,
                    inviter.path,
                )
            }
            _ => {
                return Err(AppError::Validation(
                    "unsupported invite code owner".to_owned(),
                ));
            }
        };

    Ok(PreparedReferralBinding {
        invite_code_id: invite.id,
        direct_inviter_type,
        direct_inviter_id,
        root_agent_id,
        depth,
        path_prefix,
    })
}

pub(crate) async fn bind_registered_user_referral_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    binding: PreparedReferralBinding,
) -> AppResult<()> {
    let path = format!("{}/user:{}", binding.path_prefix, user_id);
    sqlx::query(
        r#"INSERT INTO user_referrals
              (user_id, direct_inviter_id, direct_inviter_type, root_agent_id, depth, path)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(binding.direct_inviter_id)
    .bind(binding.direct_inviter_type)
    .bind(binding.root_agent_id)
    .bind(binding.depth)
    .bind(path)
    .execute(&mut **tx)
    .await?;

    sqlx::query("UPDATE invite_codes SET used_count = used_count + 1 WHERE id = ?")
        .bind(binding.invite_code_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn create_user_invite_code_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<()> {
    for _ in 0..12 {
        let code = generate_user_invite_code()?;
        let result = sqlx::query(
            r#"INSERT INTO invite_codes (owner_type, owner_id, code, status)
               VALUES ('user', ?, ?, 'active')"#,
        )
        .bind(user_id)
        .bind(&code)
        .execute(&mut **tx)
        .await;

        match result {
            Ok(_) => return Ok(()),
            Err(error) if is_duplicate_key(&error) => continue,
            Err(error) => return Err(AppError::from(error)),
        }
    }

    Err(AppError::Internal(
        "failed to create unique user invite code".to_owned(),
    ))
}

pub(crate) async fn insert_user_email_verification_in_tx(
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

pub(crate) async fn ensure_email_purpose_not_cooling_down_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    email: &str,
    purpose: &str,
    now: DateTime<Utc>,
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
    if sent_at.is_some_and(|sent_at| sent_at + Duration::seconds(60) > now) {
        return Err(AppError::Validation(
            "email verification code was sent recently".to_owned(),
        ));
    }
    Ok(())
}

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

pub(crate) async fn lock_latest_pending_email_verification_by_purpose_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    email: &str,
    purpose: &str,
) -> AppResult<Option<EmailVerificationRow>> {
    sqlx::query_as::<_, EmailVerificationRow>(
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
    .await
    .map_err(AppError::from)
}

pub(crate) async fn load_password_reset_user_id(pool: &Pool<MySql>, email: &str) -> AppResult<u64> {
    sqlx::query_scalar(
        r#"SELECT id
           FROM users
           WHERE email = ? AND status = 'active' AND email_verified_at IS NOT NULL
           LIMIT 1"#,
    )
    .bind(email)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Validation("email is not registered".to_owned()))
}

pub(crate) async fn insert_verified_user_in_tx(
    tx: &mut Transaction<'_, MySql>,
    email: &str,
    password_hash: &str,
    country_code: &str,
    preferred_locale: &str,
    verified_at: DateTime<Utc>,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO users
              (email, email_verified_at, password_hash, country_code, preferred_locale)
           VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(email)
    .bind(verified_at.naive_utc())
    .bind(password_hash)
    .bind(country_code)
    .bind(preferred_locale)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_user)?;

    Ok(result.last_insert_id())
}

pub(crate) async fn increment_email_verification_attempt_in_tx(
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

pub(crate) async fn lock_password_reset_user_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    email: &str,
) -> AppResult<u64> {
    sqlx::query_scalar(
        r#"SELECT id
           FROM users
           WHERE id = ? AND email = ? AND status = 'active'
             AND email_verified_at IS NOT NULL
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(email)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::Unauthorized)
}

pub(crate) async fn update_user_password_in_tx(
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

fn map_duplicate_key(error: sqlx::Error, actor: &str) -> AppError {
    if is_duplicate_key(&error) {
        AppError::Conflict(format!("{actor} already exists"))
    } else {
        AppError::Database(error)
    }
}

pub(crate) fn map_duplicate_user(error: sqlx::Error) -> AppError {
    if is_duplicate_key(&error) {
        AppError::Conflict("user already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}

async fn lock_active_invite_code_in_tx(
    tx: &mut Transaction<'_, MySql>,
    code: &str,
) -> AppResult<InviteCodeRow> {
    sqlx::query_as::<_, InviteCodeRow>(
        r#"SELECT id, owner_type, owner_id, usage_limit, used_count
           FROM invite_codes
           WHERE code = ? AND status = 'active'
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(code)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Validation("invite code is inactive or not found".to_owned()))
}

async fn ensure_active_agent_in_tx(
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

    // 邀请码归属代理的任一上级停用时，下级代理也不可继续发展用户。
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

async fn ensure_active_user_in_tx(tx: &mut Transaction<'_, MySql>, user_id: u64) -> AppResult<()> {
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
    .ok_or_else(|| AppError::Validation("inviter is inactive or not found".to_owned()))?;
    Ok(())
}

async fn load_referral_link_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<ReferralLinkRow> {
    sqlx::query_as::<_, ReferralLinkRow>(
        r#"SELECT root_agent_id, depth, path
           FROM user_referrals
           WHERE user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Validation("inviter has not bound an agent".to_owned()))
}

fn is_duplicate_key(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(database_error) if database_error.code().as_deref() == Some("1062"))
}
