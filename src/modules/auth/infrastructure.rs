//! auth bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。
//!
//! 本文件是认证限界上下文全部外部 I/O 的落地点：MySQL 认证仓储、Redis 刷新令牌适配器，
//! 以及向 Cloudflare 站点校验接口发起的出站请求。
//! 文件内函数分为两类，后缀带 `_in_tx` 的都运行在调用方开启的事务中，自身既不开启也不提交事务，
//! 其加锁顺序与提交时机全部由上层用例决定；其余函数直接使用连接池，各自独立提交。
//! 安全上这里只接收已经散列的口令与验证码，绝不写入任何明文凭据；刷新令牌在 MySQL 侧存 Argon2 摘要、
//! 在 Redis 侧存 SHA-256 派生键；查询未命中一律返回 `None` 或统一的校验错误，不靠错误差异暴露账号是否存在。

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
    #[serde(default)]
    auth_session_version: u64,
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
    /// 用已建立的 Redis 连接管理器包出刷新令牌适配器，构造期间不发起任何网络请求。
    /// 连接管理器自带重连并可廉价克隆，因此适配器能在多个请求间共享，无需额外加锁或做连接池管理。
    /// 构造成功不代表 Redis 可达，真正的连接故障要到首次执行读写命令时才会暴露。
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
            auth_session_version: token.auth_session_version,
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
            auth_session_version: record.auth_session_version,
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

/// 对原始刷新令牌做一次 SHA-256 并输出十六进制字符串，作为构造存储键的中间结果。
/// 这里用快速摘要而不是口令散列：令牌本身是高熵随机串，不存在被字典穷举的风险，
/// 而每次刷新都要按令牌定位记录，慢哈希会把成本叠加到每一个请求上。
/// 摘要不可逆，因此即便 Redis 键空间被完整列举，也无法从键名反推出可用的刷新令牌。
fn refresh_token_digest(refresh_token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(refresh_token.as_bytes());
    hex::encode(hasher.finalize())
}

/// 把原始刷新令牌拼成 Redis 中的记录键，形式是固定业务前缀加上令牌摘要。
/// 前缀让认证相关的键落在独立命名空间内，便于按前缀排查和批量清理，也避免与其他业务的键相互覆盖。
/// 键名中只出现摘要，原始令牌不会进入 Redis 的键空间、慢查询日志或监控采样。
fn refresh_token_key(refresh_token: &str) -> String {
    format!(
        "{}{}",
        REDIS_REFRESH_PREFIX,
        refresh_token_digest(refresh_token)
    )
}

/// 拼出主体索引键，形式是固定前缀加主体类型与主体 ID，用来收拢该主体名下的全部刷新令牌键。
/// 有了这个索引，改密或强制下线时无需扫描整个键空间即可批量撤销，代价是写入端必须同步维护索引。
/// 键中带上主体类型，可避免三张账号表里相同的自增 ID 碰撞进同一个索引集合。
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
    /// 用已配置好的 MySQL 连接池包出认证仓储适配器，构造过程不建立连接也不执行任何语句。
    /// 连接池句柄可以廉价克隆并在任务间共享，因此适配器本身无额外状态，能随应用状态一起复制。
    /// 数据库不可达或表结构缺失都要等到具体查询执行时才会以数据库错误的形式暴露。
    pub fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }

    /// 确认普通用户记录存在且状态为活跃，命中时用数据库返回的 ID 重建主体。
    /// 用户主体的 `user_id` 与账号 ID 相同，这里一并回填，使下游资产相关流程可以直接使用。
    /// 账号被停用与记录不存在都返回 `None`，调用方须把两者折叠成未授权，不得据此判断账号是否存在。
    /// 这是不加锁的普通读取，返回之后账号仍可能被并发停用。
    async fn find_active_user(&self, actor: &AuthActor) -> AppResult<Option<AuthActor>> {
        let actor_id = sqlx::query_scalar::<_, u64>(
            "SELECT id FROM users WHERE id = ? AND status = 'active' LIMIT 1",
        )
        .bind(actor.actor_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(actor_id.map(|actor_id| AuthActor::new(ActorType::User, actor_id, Some(actor_id))))
    }

    /// 确认平台管理员记录存在且状态为活跃，命中时重建管理员主体，`user_id` 固定留空。
    /// 留空是刻意为之：管理员不对应任何交易账户，一旦回填就可能被下游误当成用户身份去操作资产。
    /// 停用与不存在同样都返回 `None`。本查询不涉及角色与权限，功能级授权由业务侧另行判定。
    async fn find_active_admin(&self, actor: &AuthActor) -> AppResult<Option<AuthActor>> {
        let row = sqlx::query_as::<_, (u64, u64)>(
            r#"SELECT id, auth_session_version
               FROM admin_users
               WHERE id = ? AND status = 'active' AND must_change_password = FALSE
               LIMIT 1"#,
        )
        .bind(actor.actor_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|(actor_id, auth_session_version)| {
            AuthActor::new(ActorType::Admin, actor_id, None)
                .with_auth_session_version(auth_session_version)
        }))
    }

    /// 确认代理后台账号活跃，并要求其所属代理及整条祖先链路上没有任何一级被停用。
    /// 祖先判定用路径前缀匹配完成：凡路径等于自身或为自身前缀的代理，只要有一个状态不是活跃就整体判否。
    /// 这样冻结上级代理即可立刻切断其全部下级后台的会话续期，而不必逐个改写下级账号状态。
    /// 任何一项不满足都返回 `None`，调用方无法区分究竟是账号本身被停用还是某一级上级被冻结。
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
    /// 插入一条新用户记录，写入邮箱、手机号、国家码、默认语言和调用方已经散列好的口令。
    /// 邮箱或手机号命中唯一索引时映射为用户已存在的冲突错误，其余数据库错误按原语义上抛。
    /// 主体 ID 取自自增主键，用户主体的 `user_id` 与之相同。
    /// 本方法直接使用连接池独立提交，不参与注册用例的事务，也不写推荐关系、邀请码和注册事件。
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

    /// 插入一条平台管理员记录，写入用户名、口令哈希与角色 ID，并返回管理员作用域主体。
    /// 角色是否存在完全交给外键约束把关，这里不做预查，从而避免检查与插入之间出现竞态窗口。
    /// 用户名唯一冲突映射为管理员已存在；调用者是否有权创建管理员由服务层在调用之前判定。
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

    /// 插入一条代理后台账号记录，把用户名与口令哈希挂到入参指定的代理节点上。
    /// 只创建可登录的后台账号，不创建代理公司本身，也不生成或调整代理层级路径。
    /// 代理节点是否存在由外键约束保证，用户名冲突映射为代理已存在；该节点当前是否活跃要到登录时才校验。
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

    /// 读取国家配置中同时满足开放注册且状态活跃的那一条，返回规范化国家码与默认语言。
    /// 注册开关关闭或配置被停用一律按未命中返回，不把「配置存在但被禁用」这一差别透给调用方。
    /// 这是无锁读取，返回后配置仍可能被后台改动，需要与创建用户保持一致时应改用事务内的加锁版本。
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

    /// 按邮箱取出用户 ID、口令哈希与账号状态，邮箱须由调用方预先转成小写后传入。
    /// 状态随记录一并返回而不是写进查询条件，使停用账号照样走完口令比对，与密码错误保持一致的响应特征。
    /// 未命中只返回 `None`，不附带任何可用来区分邮箱是否已注册的额外信息。
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

    /// 按手机号取出同一组用户凭据快照，这是邮箱之外的第二条登录标识入口。
    /// 号码按存储中的原样精确匹配，SQL 里不做去分隔符或模糊处理，格式整形必须在调用之前完成。
    /// 与邮箱查询命中的是同一张用户表的同一批字段，未命中同样只返回 `None`。
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

    /// 按用户名取出用户凭据快照，用户名须已完成小写与字符集规范化。
    /// 用户名登录开关的判定在服务层完成，本方法不重复检查，因此绝不能把它接到开关关闭时的登录路径上，
    /// 否则用户名登录会绕过开关被重新启用。
    /// 取的是与邮箱、手机号查询相同的表和字段，未命中返回 `None`。
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

    /// 按用户名取出平台管理员的 ID、口令哈希与状态，构造出的主体不带 `user_id`。
    /// 查询只按用户名过滤而不过滤状态，把停用判定留给服务层，从而与用户端共用一致的失败与锁定语义。
    /// 未命中返回 `None`；口令比对不在仓储内执行，避免同一判断在多处实现出现分歧。
    async fn find_admin_by_username(
        &self,
        username: &str,
    ) -> AppResult<Option<StoredActorCredential>> {
        let row = sqlx::query_as::<_, (u64, String, String, u64)>(
            r#"SELECT id, password_hash, status, auth_session_version
               FROM admin_users WHERE username = ? LIMIT 1"#,
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;

        Ok(
            row.map(|(actor_id, password_hash, status, auth_session_version)| {
                StoredActorCredential {
                    actor: AuthActor::new(ActorType::Admin, actor_id, None)
                        .with_auth_session_version(auth_session_version),
                    password_hash,
                    status,
                }
            }),
        )
    }

    /// 用存在性子查询判断管理员表是否至少有一行，供首次引导注册决定是否还允许匿名创建。
    /// 选择 EXISTS 而非计数，是因为只关心有无，数据库命中第一行即可返回，表变大也不会退化。
    /// 本查询不加锁，与随后的插入之间存在竞态窗口，并发引导的最终结果由用户名唯一约束裁定。
    async fn has_any_admin(&self) -> AppResult<bool> {
        let (exists,): (bool,) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM admin_users)")
            .fetch_one(&self.pool)
            .await?;

        Ok(exists)
    }

    /// 按用户名取出代理后台账号凭据，同时要求所属代理及其整条祖先链路都处于活跃状态。
    /// 祖先判定用路径前缀匹配，任一上级被停用即按未命中处理，因此冻结上级代理会立即阻断其下级后台登录，
    /// 无需逐条改写下级账号。账号自身的状态随记录返回交由服务层判断，保持与其他主体一致的锁定语义。
    /// 未命中不区分是用户名不存在还是代理层级失效。
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

    /// 按主体类型把活跃状态回查分派到用户、管理员或代理各自的私有实现上。
    /// 三条分支查询的是不同的表，活跃判定口径也不同，其中代理分支还要额外校验整条祖先链路。
    /// 令牌刷新与管理员授权都经由这里回查，使账号停用和代理冻结不必等访问令牌自然过期就能生效。
    async fn find_active_actor(&self, actor: &AuthActor) -> AppResult<Option<AuthActor>> {
        match actor.actor_type {
            ActorType::User => self.find_active_user(actor).await,
            ActorType::Admin => self.find_active_admin(actor).await,
            ActorType::Agent => self.find_active_agent(actor).await,
        }
    }

    /// 在口令校验通过后记录本次登录，当前实现只更新代理后台账号的最近登录时间。
    /// 用户与平台管理员两类主体没有对应字段，直接返回成功，这是有意为之的空操作而不是遗漏。
    /// 更新语句真的失败时会上抛，进而让整次登录失败，因此它不能被当成可有可无的旁路埋点。
    /// 本方法不写审计流水，也不记录来源 IP 与设备信息。
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

    /// 插入一条刷新令牌记录，保存主体信息、令牌摘要与到期时间，原始令牌不落库。
    /// 摘要唯一冲突时借 ON DUPLICATE KEY UPDATE 把 token_hash 写回自身，实际不改动任何列，
    /// 使重复登记同一摘要成为幂等操作而不是报错中断登录。
    /// 本方法不清理该主体的历史令牌，同一主体可以同时持有多枚有效刷新令牌。
    async fn store_refresh_token(&self, token: StoredRefreshToken) -> AppResult<()> {
        sqlx::query(
            r#"INSERT INTO refresh_tokens
                  (user_id, actor_type, actor_id, auth_session_version, token_hash, expires_at)
               VALUES (?, ?, ?, ?, ?, ?)
               ON DUPLICATE KEY UPDATE token_hash = token_hash"#,
        )
        .bind(token.user_id)
        .bind(token.actor_type.as_str())
        .bind(token.actor_id)
        .bind(token.auth_session_version)
        .bind(token.token_hash)
        .bind(token.expires_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 按摘要回查刷新令牌绑定的主体，只接受未被撤销且到期时间晚于传入时刻的记录。
    /// 时间由调用方传入，使同一次刷新流程中的多处判断共用一致的时间基准，也便于测试固定时钟。
    /// 存储中的主体类型字符串无法识别时返回未授权而不是内部错误，避免被污染的行被当成某个默认身份放行。
    /// 作用域由主体类型推导而来，并不单独存列，因此不存在两者互相矛盾的可能。
    /// 本方法只读，既不消费也不轮换令牌，重放边界由服务层决定。
    async fn find_refresh_token(
        &self,
        token_hash: &str,
        now: NaiveDateTime,
    ) -> AppResult<Option<RefreshTokenRecord>> {
        let row = sqlx::query_as::<_, (String, u64, Option<u64>, u64)>(
            r#"SELECT actor_type, actor_id, user_id, auth_session_version
               FROM refresh_tokens
               WHERE token_hash = ? AND revoked_at IS NULL AND expires_at > ?
               LIMIT 1"#,
        )
        .bind(token_hash)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|(actor_type, actor_id, user_id, auth_session_version)| {
            let actor_type = ActorType::from_storage(&actor_type)?;
            Ok(RefreshTokenRecord {
                scope: actor_type.scope(),
                actor_type,
                actor_id,
                user_id,
                auth_session_version,
            })
        })
        .transpose()
    }

    /// 查询该主体类型与标识组合当前是否处于锁定期，只返回仍晚于当前时刻的锁定截止时间。
    /// 过期条件直接写在 SQL 里，已到期的锁定天然被排除，因此解锁靠时间自然完成，无需清理任务或人工介入。
    /// 标识必须是领域层规范化后的失败计数键，直接传原始输入会因大小写或空白差异漏掉既有锁定。
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

    /// 在单条 upsert 中推进失败计数，越过阈值时写入锁定截止时间，最后回读判断当前是否真的处于锁定。
    /// 计数刻意不采用先读后写：那样会在尚不存在的行上取间隙锁，与并发插入的意向锁互相死锁，
    /// 使并发失败请求报错并漏计，等于放过一整轮爆破。窗口未过期则累加，已过期则重置为一次。
    /// 实现依赖 ON DUPLICATE KEY UPDATE 赋值自左向右求值这一特性，让后续表达式读到已经更新的新计数。
    /// upsert 只影响一行说明本次新增了标识符，借这一时机顺带删除一批已过期的计数行；
    /// 否则针对随机账号的撞库会留下永不回收的记录，因为常规清理只发生在登录成功时。
    /// 回读结果还会按当前时间再过滤一次，返回 `None` 表示本次失败尚未触发锁定。
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

    /// 删除该主体类型与标识组合的失败计数行，一并清掉累计次数、时间窗口和锁定状态。
    /// 这是计数行在正常流程中唯一的删除时机，因此只能在口令校验确实通过后调用，任何失败分支都不得触发，
    /// 否则攻击者可以靠制造特定失败来抹掉自己的尝试记录。
    /// 目标行不存在时删除零行并返回成功，重复调用幂等。
    async fn clear_login_failures(&self, actor_type: ActorType, identifier: &str) -> AppResult<()> {
        sqlx::query("DELETE FROM login_failure_counters WHERE actor_type = ? AND identifier = ?")
            .bind(actor_type.as_str())
            .bind(identifier)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

/// 读取管理员用户名，只用于拼装 TOTP 导入链接中展示给认证器 App 的账号标签。
/// 记录不存在时返回未授权而不是内部错误：能走到这一步说明令牌里的管理员已被删除，会话本就不该继续。
/// 只取用户名一列，不触碰口令哈希、角色和二次验证密钥，避免绑定流程顺带把敏感字段读进内存。
pub(crate) async fn load_admin_username(pool: &Pool<MySql>, admin_id: u64) -> AppResult<String> {
    sqlx::query_scalar::<_, String>("SELECT username FROM admin_users WHERE id = ? LIMIT 1")
        .bind(admin_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::Unauthorized)
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct AdminPasswordCredentialRecord {
    pub(crate) password_hash: String,
    pub(crate) status: String,
    pub(crate) must_change_password: bool,
}

/// 在管理员改密事务中锁定当前凭证与强制改密标志；不存在时返回未授权，且锁在事务提交前持续持有。
pub(crate) async fn lock_admin_password_credential_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
) -> AppResult<AdminPasswordCredentialRecord> {
    sqlx::query_as::<_, AdminPasswordCredentialRecord>(
        r#"SELECT password_hash, status, must_change_password
           FROM admin_users
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(admin_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::Unauthorized)
}

/// 在已锁定管理员行上同时写入新哈希、清除首次改密标志并记录改密时刻，三项不可拆分提交。
pub(crate) async fn update_admin_password_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    password_hash: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE admin_users
           SET password_hash = ?, must_change_password = FALSE,
               password_changed_at = CURRENT_TIMESTAMP(6),
               auth_session_version = auth_session_version + 1
           WHERE id = ?"#,
    )
    .bind(password_hash)
    .bind(admin_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在同一改密事务内撤销 MySQL 中尚未撤销的管理员刷新令牌；重复执行不改写既有撤销时间。
pub(crate) async fn revoke_admin_refresh_tokens_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE refresh_tokens
           SET revoked_at = CURRENT_TIMESTAMP(6)
           WHERE actor_type = 'admin' AND actor_id = ? AND revoked_at IS NULL"#,
    )
    .bind(admin_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 把管理员自助改密写入后台审计，记录是否由首次强制闸门触发，但不保存任何口令或哈希。
pub(crate) async fn insert_admin_password_change_audit_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    was_forced: bool,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO admin_audit_logs
              (admin_id, action, target_type, target_id, before_json, after_json, reason)
           VALUES (?, 'admin.password.change', 'admin_user', ?,
                   JSON_OBJECT('must_change_password', ?),
                   JSON_OBJECT('must_change_password', FALSE),
                   'administrator self-service password rotation')"#,
    )
    .bind(admin_id)
    .bind(admin_id.to_string())
    .bind(was_forced)
    .execute(&mut **tx)
    .await?;
    Ok(())
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

/// 在调用方已开启的注册事务中以 `FOR UPDATE` 锁定目标国家配置，要求其开放注册且状态活跃。
/// 加锁是为了让配置在整个注册事务期间保持稳定，避免刚校验完就被后台关闭注册，
/// 结果用户带着一个已失效的国家码落库，后续本地化和合规判断全部错位。
/// 锁一直持有到调用方提交或回滚。未命中返回校验错误，不区分国家不存在与国家已关闭注册。
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

/// 在注册事务中确认邮箱尚未被任何用户占用，命中即返回冲突错误。
/// 只取主键做存在性判断，不读出占用方账号的其他任何字段，冲突信息里也不包含对方账号的信息。
/// 这是不加锁的读取，真正的并发防线仍然是邮箱唯一索引；本检查的作用是把常见冲突提前转成清晰的业务错误，
/// 而不是让调用方看到底层的唯一键报错。
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

/// 在事务中取该邮箱最近一条待验证的注册验证码，若其发送时间距今不足六十秒则拒绝再次发送。
/// 冷却按邮箱与用途维度计算，防止匿名调用方靠反复请求把验证码邮件当成骚扰或轰炸手段，
/// 也顺带限制了攻击者刷新验证码以扩大猜测面的速率。
/// 只看最新一条待验证记录，已被取代或已验证的历史记录不参与判断。
/// 时间基准由调用方传入，保证同一次发送流程中多处判断使用同一个时刻。
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

/// 在插入新注册验证码之前，把该邮箱下所有仍处于待验证状态的旧码统一标记为已取代。
/// 这一步保证任意时刻每个邮箱最多只有一枚可用的注册验证码，旧码在新码发出后立即失效，
/// 从而堵住先攒下多枚验证码、再逐个尝试以绕过单枚试错上限的重放路径。
/// 没有待验证记录时更新零行并返回成功，重复执行幂等。
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

/// 在发送事务中插入一条注册验证码记录，保存邮箱、用途、验证码哈希、到期时间与发送时间。
/// 入库的始终是哈希，明文验证码只出现在随后发出的邮件正文里，数据库与日志中都不留存。
/// 记录初始为待验证状态，试错次数从零开始累积；发送时间同时充当下一次发送的冷却基准。
/// 本函数不提交事务，若调用方在提交前失败，这条记录不会存在，冷却也不会被占用。
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

/// 在调用方事务中插入一条面向已注册用户的验证码记录，按用户、邮箱和用途三元组归档。
/// 用途字段把密码重置、二次验证重置等场景彼此隔离，使各流程的验证码无法互相顶用，
/// 攻击者也不能用一个低风险场景领到的码去完成高风险操作。
/// 保存的是验证码哈希，明文只随邮件发出；记录初始为待验证，发送时间用于计算该用途独立的冷却。
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

/// 在事务中以 `FOR UPDATE` 锁定活跃且已完成邮箱验证的用户，并取出其当前邮箱。
/// 加锁让同一事务内后续的验证码写入或消费与这个邮箱严格对应，避免中途邮箱被改导致验证码发往旧地址，
/// 或者反过来用旧地址领到的验证码去操作新地址的账号。
/// 用户不活跃、邮箱未验证或邮箱为空都返回同一条校验错误，调用方无法区分具体是哪一种情况。
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

/// 在事务中检查该用户、邮箱与用途组合最近一条待验证码的发送时间，不足六十秒则拒绝重复发送。
/// 冷却按用途独立计算，因此密码重置的发送不会挡住二次验证重置的发送，两条流程互不牵连。
/// 只看最新一条待验证记录，已取代和已验证的历史记录不计入冷却判断。
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

/// 在发出新验证码之前，把该用户在同一用途下所有待验证的旧码标记为已取代。
/// 与注册验证码的取代不同，这里按用户和用途过滤而不按邮箱，因此换过邮箱后遗留的旧码同样会被作废。
/// 由此保证每个用户在每种用途下最多只有一枚可用验证码，杜绝攒码之后逐个重放。
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

/// 在事务中以 `FOR UPDATE` 锁定该用户、邮箱与用途下最新的待验证码记录，并返回校验所需字段。
/// 加锁使随后的试错计数递增或消费标记与本次读取构成原子操作，并发提交同一验证码不会各自读到旧计数，
/// 从而把五次试错上限真正卡死，而不是被并发请求稀释成远多于五次的实际尝试。
/// 只取最新一条，历史记录即便仍是待验证状态也不参与比对。
/// 未命中返回 `None`，由调用方统一转换成与验证码错误一致的校验错误。
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

/// 按邮箱定位活跃且已完成邮箱验证的用户 ID，供密码重置流程确定验证码的接收方。
/// 未注册、已停用与邮箱未验证都返回同一条邮箱未注册的校验错误，三者在响应上无法区分，
/// 但这仍意味着该入口整体会暴露某个邮箱是否可用于重置，抗枚举须依赖上游限流。
/// 使用连接池直接查询，不加锁；真正的一致性由后续重置事务内的加锁读取保证。
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

/// 在验证事务中把指定验证码记录的试错次数加一，用于逼近五次上限后让该验证码作废。
/// 递增在数据库内完成而非先读后写，配合调用方持有的行锁，保证并发试码不会互相覆盖计数。
/// 关键约束是：调用方即便最终要返回验证码错误，也必须提交这次递增，
/// 否则回滚会把计数一并抹掉，试错上限形同虚设，单枚验证码可被无限次猜测。
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

/// 把指定验证码记录标记为已验证并写入验证时间，使其无法再被第二次消费。
/// 消费与随后的口令更新等凭证变更处在同一事务内，因此要么一起生效，要么一起回滚，
/// 不会出现验证码已作废但密码没改、用户还得重新领码的中间状态。
/// 本函数按主键更新，不重复校验记录是否仍处于待验证状态，该前提由调用方先前的加锁读取保证。
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

/// 在重置事务中以 `FOR UPDATE` 按用户 ID 和邮箱同时锁定活跃且已验证邮箱的用户。
/// 两个条件一起匹配，是为了确认此刻的账号仍与先前消费掉的那枚验证码指向同一邮箱，
/// 避免在领码与提交之间邮箱被改动，出现拿旧邮箱的验证码改掉新邮箱账号口令的越权路径。
/// 任一条件不满足都返回未授权而不是校验错误，不透露究竟是账号状态变了还是邮箱对不上。
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

/// 在已加锁的重置事务中写入新的口令哈希，入参必须是调用方散列之后的结果，本函数不接受明文。
/// 这里不做长度或复杂度校验，也不消费验证码、不撤销会话、不自行提交，
/// 这些步骤由重置用例在同一事务或后续步骤中显式完成，顺序排错就会留下可被利用的窗口。
/// 只更新口令一列，不改动账号状态与邮箱。
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

/// 在重置事务中把该用户名下所有尚未撤销的刷新令牌一次性打上撤销时间。
/// 与口令更新同事务提交，保证新口令生效的同一刻旧刷新令牌全部失效，不给攻击者留下继续续期的缝隙。
/// 只覆盖 MySQL 中登记的刷新令牌，Sa-Token 会话与 Redis 侧的令牌必须在事务提交后另行撤销。
/// 已撤销的记录被查询条件排除，因此重复执行幂等，也不会刷掉既有的撤销时间。
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

/// 把 MySQL 的唯一键冲突翻译成按主体命名的冲突错误，其余数据库错误保持原样上抛。
/// 主体名由调用方传入，使用户、管理员和代理三条创建路径各自给出可读且不混淆的提示。
/// 只识别唯一键冲突这一种情况，外键失败、超时等错误不会被误判成业务冲突而掩盖真实故障。
/// 错误文案不含冲突的具体字段值，避免把已存在的用户名或邮箱原样回显出去。
fn map_duplicate_key(error: sqlx::Error, actor: &str) -> AppError {
    if is_duplicate_key(&error) {
        AppError::Conflict(format!("{actor} already exists"))
    } else {
        AppError::Database(error)
    }
}

/// 把用户插入时的唯一键冲突固定翻译成用户已存在，其余 SQL 错误保留数据库原本语义。
/// 邮箱与手机号共用同一条提示，因此响应不会指明究竟是哪一列冲突，
/// 减少注册接口被用来逐项探测已有账号联系方式的价值。
/// 与按主体命名的通用映射不同，本函数专供用户注册路径使用，主体名固定且不可配置。
pub(crate) fn map_duplicate_user(error: sqlx::Error) -> AppError {
    if is_duplicate_key(&error) {
        AppError::Conflict("user already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}

/// 在注册事务中以 `FOR UPDATE` 锁定处于启用状态的邀请码，并取出归属方与用量字段。
/// 加锁是超发防护的关键：额度检查与随后的使用次数累加必须落在同一把行锁下，
/// 否则并发注册会同时读到尚未超额的旧值，各自通过检查后一起把用量顶破上限。
/// 邀请码不存在与已停用返回同一条校验错误，不透露该码是否曾经存在过。
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

/// 在注册事务中锁定目标代理并逐级锁定其祖先，要求整条链路上每一级都处于活跃状态。
/// 先按 ID 锁定代理本体并取出层级路径，再用路径前缀匹配把所有祖先一并加锁，
/// 且按层级与 ID 的固定顺序读取，使并发注册以相同顺序获取行锁，避免交叉加锁形成死锁。
/// 祖先集合为空或其中任意一级不是活跃状态都判为失败，因为上级被停用时下级不应继续发展新用户。
/// 这些锁一直持有到调用方提交，从而保证代理状态在整个注册事务期间不被并发改动。
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

/// 在注册事务中以 `FOR UPDATE` 锁定作为邀请人的用户，并要求其状态为活跃。
/// 加锁防止邀请人在推荐关系写入之前被并发停用，避免新用户被挂到一个已经失效的邀请人名下。
/// 只做存在性与状态校验，不读取邀请人的任何业务字段；未命中返回邀请人不可用的校验错误。
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

/// 在注册事务中以 `FOR UPDATE` 锁定邀请人的推荐关系记录，取出其归属代理、层级深度和路径。
/// 新用户的深度在邀请人基础上加一、路径以邀请人路径为前缀，因此这条记录必须在整个事务内保持稳定，
/// 否则并发改动会让推荐树出现深度与路径互相矛盾的分叉，后续分佣按树遍历时会算错归属。
/// 邀请人没有推荐关系记录时返回校验错误，即用户邀请码要求邀请人自己已经归属于某条推荐链路。
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

/// 判断数据库错误是否为 MySQL 的唯一键冲突，依据是错误码 1062。
/// 直接匹配错误码而不是解析错误文本，从而不受数据库语言设置与版本间措辞变化的影响。
/// 邀请码生成的重试循环和各创建路径的冲突映射都依赖它来区分「可重试的撞码」与「真正的故障」，
/// 判断放宽会把真实故障吞成静默重试，收紧则会把正常冲突暴露成内部错误。
fn is_duplicate_key(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(database_error) if database_error.code().as_deref() == Some("1062"))
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_auth_infrastructure_tests.rs"]
mod tests;
