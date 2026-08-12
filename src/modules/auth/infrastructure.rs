//! auth bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。

use crate::{
    architecture::InfrastructureLayer,
    error::{AppError, AppResult},
    modules::{
        auth::{
            ActiveCountryConfig, ActorType, AuthActor, NewAdminActor, NewAgentActor, NewUserActor,
            ProjectRefreshTokenRepository, RefreshTokenRecord, StoredActorCredential,
            StoredProjectRefreshToken, StoredRefreshToken, TokenScope,
            domain::{
                LOGIN_FAILURE_LIMIT, LOGIN_FAILURE_WINDOW_SECONDS, LOGIN_LOCKOUT_SECONDS,
                normalize_invite_code, validate_email_code,
            },
            repository::AuthRepository,
            verify_password,
        },
        user::service::generate_user_invite_code,
    },
};
use axum::async_trait;
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{MySql, Pool, Transaction};
use std::time::Duration as StdDuration;

pub(crate) const CF_TURNSTILE_SITEVERIFY_URL: &str =
    "https://challenges.cloudflare.com/turnstile/v0/siteverify";
pub(crate) const TURNSTILE_VERIFY_TIMEOUT: StdDuration = StdDuration::from_secs(5);
const REDIS_REFRESH_PREFIX: &str = "exchange:auth:refresh:";
const REDIS_REFRESH_ACTOR_PREFIX: &str = "exchange:auth:refresh_actor:";

#[derive(Debug, Deserialize)]
struct CfTurnstileVerifyResponse {
    success: bool,
    #[serde(rename = "error-codes", default)]
    error_codes: Vec<String>,
    hostname: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RedisRefreshTokenRecord {
    actor_type: String,
    actor_id: u64,
    user_id: Option<u64>,
    scope: TokenScope,
    expires_at: i64,
}

#[derive(Clone)]
/// Redis 项目刷新令牌适配器，维护令牌摘要键、主体索引集合与各自 TTL。
///
/// 适配器永不持久化原始刷新令牌；主体索引用于密码修改或人工登出时批量撤销会话，
/// 所有网络及序列化错误均向上转换为统一 `AppResult`。
pub struct RedisProjectRefreshTokenRepository {
    manager: redis::aio::ConnectionManager,
}

impl InfrastructureLayer for RedisProjectRefreshTokenRepository {}

impl RedisProjectRefreshTokenRepository {
    /// 使用已经建立的 Redis 连接管理器创建适配器，不在构造期间发起网络请求。
    pub fn new(manager: redis::aio::ConnectionManager) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl ProjectRefreshTokenRepository for RedisProjectRefreshTokenRepository {
    /// 依次写入令牌摘要记录、主体索引集合及两者 TTL，原始刷新令牌不入库。
    ///
    /// 三条 Redis 命令未使用事务或 Lua；中途失败会上抛，但先前已写的令牌键或索引可能保留至 TTL 到期。
    /// 调用方不得在该方法返回错误时向客户端交付刷新令牌。
    async fn store_project_refresh_token(&self, token: StoredProjectRefreshToken) -> AppResult<()> {
        let key = refresh_token_key(&token.refresh_token);
        let actor_key = refresh_actor_key(token.actor_type, token.actor_id);
        let record = RedisRefreshTokenRecord {
            actor_type: token.actor_type.as_str().to_owned(),
            actor_id: token.actor_id,
            user_id: token.user_id,
            scope: token.scope,
            expires_at: token.expires_at.timestamp(),
        };
        let value = serde_json::to_string(&record).map_err(|error| {
            AppError::Internal(format!("failed to encode refresh token: {error}"))
        })?;
        let ttl = (token.expires_at - Utc::now()).num_seconds().max(1) as u64;
        let mut redis = self.manager.clone();

        redis.set_ex::<_, _, ()>(&key, value, ttl).await?;
        redis.sadd::<_, _, ()>(&actor_key, &key).await?;
        redis.expire::<_, ()>(&actor_key, ttl as i64).await?;
        Ok(())
    }

    /// 根据原始令牌计算不可逆摘要键并读取主体快照，过期记录按未命中处理。
    ///
    /// 内容损坏会返回未授权而非内部结构细节，防止异常缓存数据扩大为信息泄露。
    async fn find_project_refresh_token(
        &self,
        refresh_token: &str,
        now: DateTime<Utc>,
    ) -> AppResult<Option<RefreshTokenRecord>> {
        let mut redis = self.manager.clone();
        let Some(value) = redis
            .get::<_, Option<String>>(refresh_token_key(refresh_token))
            .await?
        else {
            return Ok(None);
        };
        let record: RedisRefreshTokenRecord =
            serde_json::from_str(&value).map_err(|_| AppError::Unauthorized)?;
        if record.expires_at <= now.timestamp() {
            return Ok(None);
        }
        let actor_type = ActorType::from_storage(&record.actor_type)?;

        Ok(Some(RefreshTokenRecord {
            actor_type,
            actor_id: record.actor_id,
            user_id: record.user_id,
            scope: record.scope,
        }))
    }

    /// 查询主体索引集合，删除其当前枚举到的令牌键，再删除索引本身。
    ///
    /// 操作未使用 Redis 事务，中途失败可留下部分键并上抛；只覆盖成功登记到该索引的刷新令牌，
    /// 索引缺失时不会扫描全库补偿。空索引的重复撤销返回成功。
    async fn revoke_actor_refresh_tokens(&self, actor: &AuthActor) -> AppResult<()> {
        let mut redis = self.manager.clone();
        let actor_key = refresh_actor_key(actor.actor_type, actor.actor_id);
        let keys = redis.smembers::<_, Vec<String>>(&actor_key).await?;
        if !keys.is_empty() {
            redis.del::<_, ()>(&keys).await?;
        }
        redis.del::<_, ()>(actor_key).await?;
        Ok(())
    }
}

fn refresh_token_digest(refresh_token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(refresh_token.as_bytes());
    hex::encode(hasher.finalize())
}

fn refresh_token_key(refresh_token: &str) -> String {
    format!(
        "{}{}",
        REDIS_REFRESH_PREFIX,
        refresh_token_digest(refresh_token)
    )
}

fn refresh_actor_key(actor_type: ActorType, actor_id: u64) -> String {
    format!(
        "{}{}:{}",
        REDIS_REFRESH_ACTOR_PREFIX,
        actor_type.as_str(),
        actor_id
    )
}

/// 向配置的 Siteverify URL 发起表单 POST，发送服务端密钥、客户端令牌与可选 `remoteip`。
/// 请求固定五秒超时；网络、非成功状态、响应解码及服务方拒绝分别映射安全错误，拒绝信息
/// 会包含服务方错误码。函数不记录表单值，也不校验返回的 hostname；调用方须保护 URL 与 secret。
pub(crate) async fn verify_turnstile_site_response(
    siteverify_url: &str,
    secret: &str,
    token: &str,
    remote_ip: Option<&str>,
) -> AppResult<()> {
    let mut payload = vec![
        ("secret", secret.to_owned()),
        ("response", token.to_owned()),
    ];
    if let Some(remote_ip) = remote_ip {
        payload.push(("remoteip", remote_ip.to_owned()));
    }

    let response = reqwest::Client::new()
        .post(siteverify_url)
        .form(&payload)
        .timeout(TURNSTILE_VERIFY_TIMEOUT)
        .send()
        .await
        .map_err(|error| {
            AppError::security_forbidden(
                "CF_TURNSTILE_REQUEST_FAILED",
                format!("failed to verify Cloudflare challenge: {error}"),
            )
        })?;

    if !response.status().is_success() {
        return Err(AppError::security_forbidden(
            "CF_TURNSTILE_BAD_RESPONSE",
            format!(
                "Cloudflare challenge verification returned {}",
                response.status()
            ),
        ));
    }

    let body = response
        .json::<CfTurnstileVerifyResponse>()
        .await
        .map_err(|error| {
            AppError::security_forbidden(
                "CF_TURNSTILE_PARSE_FAILED",
                format!("invalid Cloudflare verification response: {error}"),
            )
        })?;

    if !body.success {
        let error_text = if body.error_codes.is_empty() {
            "verification failed".to_owned()
        } else {
            body.error_codes.join(", ")
        };
        return Err(AppError::security_forbidden(
            "CF_TURNSTILE_INVALID",
            format!("Cloudflare verification failed: {error_text}"),
        ));
    }

    let _ = body.hostname;
    Ok(())
}

#[derive(Clone)]
pub struct MySqlAuthRepository {
    pool: Pool<MySql>,
}

impl InfrastructureLayer for MySqlAuthRepository {}

impl MySqlAuthRepository {
    /// 用已配置 MySQL 连接池创建认证仓储适配器，构造时不访问数据库。
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

    async fn has_any_admin(&self) -> AppResult<bool> {
        let (exists,): (bool,) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM admin_users)")
            .fetch_one(&self.pool)
            .await?;

        Ok(exists)
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

    async fn find_login_lockout(
        &self,
        actor_type: ActorType,
        identifier: &str,
    ) -> AppResult<Option<DateTime<Utc>>> {
        let locked_until = sqlx::query_scalar::<_, Option<DateTime<Utc>>>(
            r#"SELECT locked_until
               FROM login_failure_counters
               WHERE actor_type = ? AND identifier = ? AND locked_until > ?
               LIMIT 1"#,
        )
        .bind(actor_type.as_str())
        .bind(identifier)
        .bind(Utc::now().naive_utc())
        .fetch_optional(&self.pool)
        .await?
        .flatten();

        Ok(locked_until)
    }

    async fn record_login_failure(
        &self,
        actor_type: ActorType,
        identifier: &str,
    ) -> AppResult<Option<DateTime<Utc>>> {
        let now = Utc::now();
        let first_failure_locked = LOGIN_FAILURE_LIMIT <= 1;
        let first_failure_locked_until =
            first_failure_locked.then(|| now + Duration::seconds(LOGIN_LOCKOUT_SECONDS));
        let first_failure_window = first_failure_locked_until
            .unwrap_or_else(|| now + Duration::seconds(LOGIN_FAILURE_WINDOW_SECONDS));

        // 计数在单条 upsert 内推进：先读后写会在不存在的行上取间隙锁，与插入意向锁互相死锁，
        // 并发失败请求会因此报错且漏计，等于放过一整轮爆破。
        // ON DUPLICATE KEY UPDATE 的赋值自左向右求值，故 failure_count 之后的表达式读到的是新计数。
        let result = sqlx::query(
            r#"INSERT INTO login_failure_counters
                  (actor_type, identifier, failure_count, window_expires_at, locked_until)
               VALUES (?, ?, 1, ?, ?)
               ON DUPLICATE KEY UPDATE
                  failure_count = IF(window_expires_at > ?, failure_count + 1, 1),
                  locked_until = IF(failure_count >= ?, DATE_ADD(?, INTERVAL ? SECOND), NULL),
                  window_expires_at = IF(
                      failure_count >= ?,
                      DATE_ADD(?, INTERVAL ? SECOND),
                      DATE_ADD(?, INTERVAL ? SECOND)
                  )"#,
        )
        .bind(actor_type.as_str())
        .bind(identifier)
        .bind(first_failure_window.naive_utc())
        .bind(first_failure_locked_until.map(|value| value.naive_utc()))
        .bind(now.naive_utc())
        .bind(LOGIN_FAILURE_LIMIT)
        .bind(now.naive_utc())
        .bind(LOGIN_LOCKOUT_SECONDS)
        .bind(LOGIN_FAILURE_LIMIT)
        .bind(now.naive_utc())
        .bind(LOGIN_LOCKOUT_SECONDS)
        .bind(now.naive_utc())
        .bind(LOGIN_FAILURE_WINDOW_SECONDS)
        .execute(&self.pool)
        .await?;

        // upsert 影响 1 行即新增了标识符——表只在这一刻增长，借此做有界清扫，
        // 否则针对随机账号的撞库会留下永不回收的计数行（成功登录才删）。
        if result.rows_affected() == 1 {
            sqlx::query(
                "DELETE FROM login_failure_counters WHERE window_expires_at <= ? LIMIT 200",
            )
            .bind(now.naive_utc())
            .execute(&self.pool)
            .await?;
        }

        let locked_until = sqlx::query_as::<_, (Option<DateTime<Utc>>,)>(
            r#"SELECT locked_until FROM login_failure_counters
               WHERE actor_type = ? AND identifier = ? LIMIT 1"#,
        )
        .bind(actor_type.as_str())
        .bind(identifier)
        .fetch_optional(&self.pool)
        .await?
        .and_then(|(locked_until,)| locked_until)
        .filter(|locked_until| *locked_until > now);

        Ok(locked_until)
    }

    async fn clear_login_failures(&self, actor_type: ActorType, identifier: &str) -> AppResult<()> {
        sqlx::query("DELETE FROM login_failure_counters WHERE actor_type = ? AND identifier = ?")
            .bind(actor_type.as_str())
            .bind(identifier)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

/// 按管理员 ID 读取 TOTP 导入链接的账号标签，记录缺失时按未授权处理。
pub(crate) async fn load_admin_username(pool: &Pool<MySql>, admin_id: u64) -> AppResult<String> {
    sqlx::query_scalar::<_, String>("SELECT username FROM admin_users WHERE id = ? LIMIT 1")
        .bind(admin_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::Unauthorized)
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

/// 在注册事务中锁定已启用且允许注册的国家配置，防止创建用户期间配置漂移。
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

/// 在注册事务中检查邮箱尚未被用户占用，命中时返回冲突而不泄露其他账号字段。
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

/// 检查同一注册邮箱最新待验证码的六十秒发送冷却，冷却内不插入新记录。
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

/// 在发送新注册码前将同邮箱旧待验证记录标记为已取代，重复执行保持幂等。
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

/// 在发送事务中插入注册邮件码哈希、过期与发送时间，不持久化明文验证码。
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

/// 在调用方已开启的注册事务中锁定最新待验证邮件码，拒绝过期或达到五次尝试上限的记录。
/// 错码只递增一次尝试次数，正确码只把同一行标记为 verified；本函数不自行提交事务。
/// 调用方对校验型错误提交当前事务以保留试码计数，其他数据库错误必须回滚，重放不得重复消费旧验证码。
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

/// 在调用方已开启的注册事务中按“邀请码→所有者→上级推荐链”顺序加锁并组装绑定快照。
/// 已耗尽邀请码、停用代理或停用用户必须在创建用户前失败，避免并发注册超用量或写入失效归属。
/// 本函数不写推荐关系、不递增 used_count；这些副作用须与用户创建共用同一事务，失败或重放不得留下半成品。
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

/// 在新用户注册事务中写入已准备的邀请关系，随后原子累加邀请码使用次数。
/// 路径追加新用户 ID；任一插入或计数失败由注册事务整体回滚，不留半绑定。
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

/// 在注册事务中为新用户生成并插入启用邀请码，唯一冲突最多重试十二次。
/// 非唯一键 SQL 错误立即上抛；重试用尽返回内部错误，不自行提交用户记录。
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

/// 在调用方事务中按用户、邮箱和用途插入待验证码哈希，不保存明文。
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

/// 锁定活跃用户已验证邮箱，供验证码发送与后续消费共用一致地址。
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

/// 检查指定用户、邮箱和用途的六十秒发送冷却，冷却内拒绝新验证码。
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

/// 在发送新码前将同用户同用途旧待验证记录标记已取代，防止旧码重放。
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

/// 以 `FOR UPDATE` 锁定用户、邮箱和用途下最新待验证记录，供试码计数与消费原子更新。
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

/// 按已验证邮箱读取活跃用户 ID 供发送重置码，未命中以统一未注册错误处理。
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

/// 在注册事务中插入已验证邮箱用户，同时保存国家、语言与密码哈希。
/// 唯一冲突映射为用户已存在；不自行提交，便于邀请关系和 outbox 同步回滚。
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

/// 在验证事务中累加指定邮件码试错次数，调用方对校验错误仍需提交该计数。
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

/// 将指定邮件码记录标记为已验证并写入时间，供同事务内后续凭证变更。
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

/// 锁定与验证记录一致的活跃已验证邮箱用户，防止重置期间账号状态漂移。
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

/// 在已锁定的重置事务中更新用户密码哈希，不自行消费验证码或提交。
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

/// 在重置密码事务中撤销用户全部未撤销 MySQL 刷新令牌，重复执行保持幂等。
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

/// 将 MySQL 唯一键冲突映射为用户已存在，其他 SQL 错误保留数据库语义。
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

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_auth_infrastructure_tests.rs"]
mod tests;
