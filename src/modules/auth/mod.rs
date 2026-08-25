//! auth bounded context module root.
//!
//! 认证限界上下文的根模块：对外导出各分层子模块，并集中存放跨层共享的认证原语。
//! 这里定义主体类型与令牌作用域枚举及其字符串映射、`AuthActor` 主体标识、基于 Argon2 的口令与刷新令牌散列、
//! JWT 的签发与解码，以及把 `Authorization` 头转换成 `Claims` 的三个 Axum 提取器。
//! 运行时存在两套会话实现：配置了 Sa-Token 管理器时以服务端会话记录为准，未配置时回落到本地签名的 JWT，
//! 令牌校验入口屏蔽了这一差异，上层拿到的始终是同一种 `Claims`。
//! 安全边界上，口令明文只在散列函数内部停留，不落库也不进日志；刷新令牌一律以摘要形式保存；
//! 令牌缺失、过期、被顶替或被强制下线统一折叠成未授权，只有作用域不匹配才返回禁止访问，
//! 使调用方无法通过错误差异反推会话的真实状态。

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod repository;
pub mod service;
use crate::{
    config::Settings,
    error::{AppError, AppResult},
    state::AppState,
};
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use axum::{
    async_trait,
    extract::{FromRequestParts, Request, State},
    http::{Method, header::AUTHORIZATION, request::Parts},
    middleware::Next,
    response::Response,
};
use chrono::{DateTime, NaiveDateTime, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use sa_token_core::{SaTokenError, SaTokenManager, TokenInfo, TokenValue};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod routes;

pub use infrastructure::MySqlAuthRepository;
pub use repository::{AuthRepository, ProjectRefreshTokenRepository};
pub use service::AuthService;

pub(crate) const ACTIVE_STATUS: &str = "active";
const REFRESH_TOKEN_HASH_SALT: &[u8] = b"exchange-refresh-token-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenScope {
    User,
    Admin,
    Agent,
}

impl TokenScope {
    /// 把令牌作用域映射成 Sa-Token 的登录类型字符串，使同一个账号 ID 在三类主体下占据互不干扰的会话空间。
    /// 该字符串会写入服务端会话记录，并参与按登录类型枚举令牌和登出，取值必须与 `ActorType::as_str` 完全一致，
    /// 否则签发与撤销会落在不同命名空间，强制下线时会漏掉一部分已生效的令牌。
    pub fn as_login_type(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Admin => "admin",
            Self::Agent => "agent",
        }
    }

    /// 把服务端会话里记录的登录类型字符串还原为令牌作用域，是 `as_login_type` 的逆映射。
    /// 只接受三个已知取值，任何未知字符串都判为未授权而非内部错误，使被篡改或跨版本遗留的会话
    /// 不会退化成某个默认作用域被放行。返回的错误不带原始取值，避免把存储内容原样回显给调用方。
    fn from_login_type(value: &str) -> AppResult<Self> {
        match value {
            "user" => Ok(Self::User),
            "admin" => Ok(Self::Admin),
            "agent" => Ok(Self::Agent),
            _ => Err(AppError::Unauthorized),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorType {
    User,
    Admin,
    Agent,
}

impl ActorType {
    /// 返回主体类型在持久化和缓存键中使用的稳定字符串，登录失败计数、刷新令牌记录与 Redis 主体索引都依赖它。
    /// 这三个字面量等同于存储格式的一部分：改动取值会让历史行与新写入落进不同键空间，
    /// 既查不到已有的锁定记录，也撤销不掉先前登记的刷新令牌，因此不可随意重命名。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Admin => "admin",
            Self::Agent => "agent",
        }
    }

    /// 由账号主体类型推导出对应的令牌作用域，二者一一对应，保证账号只能拿到与自身身份相符的令牌。
    /// 刷新流程会用它和令牌记录中的作用域交叉比对，从而拦住拿用户刷新令牌去换管理员访问令牌的越权尝试。
    pub fn scope(self) -> TokenScope {
        match self {
            Self::User => TokenScope::User,
            Self::Admin => TokenScope::Admin,
            Self::Agent => TokenScope::Agent,
        }
    }

    /// 把数据库或 Redis 中保存的主体类型字符串解析回枚举，用于还原刷新令牌记录所绑定的身份。
    /// 无法识别的取值一律返回未授权：宁可让这条记录彻底不可用，也不猜测其原意或退回某个默认主体类型，
    /// 避免存储被污染或写入端出错时，把一个来历不明的身份提升成可用会话。
    pub(crate) fn from_storage(value: &str) -> AppResult<Self> {
        match value {
            "user" => Ok(Self::User),
            "admin" => Ok(Self::Admin),
            "agent" => Ok(Self::Agent),
            _ => Err(AppError::Unauthorized),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub scope: TokenScope,
    pub exp: usize,
    pub token_id: String,
}

#[derive(Debug, Clone)]
pub struct UserAuth(pub Claims);

#[derive(Debug, Clone)]
pub struct AdminAuth(pub Claims);

#[derive(Debug, Clone)]
pub struct AgentAuth(pub Claims);

/// 管理端路由树的统一身份与首次改密闸门。
///
/// 个别历史管理路由的 handler 没有显式提取 `AdminAuth`，仅依赖它们被挂载在
/// `/admin/api/v1` 下。因此必须在聚合路由层再统一执行一次鉴权，否则强制改密管理员会从
/// 未挂提取器的读路由绕过闸门。只有登录、登录 2FA、刷新和登录配置是匿名入口；
/// 其他路由全部复用 `AdminAuth` 的作用域、会话代际、强制改密与权限回查语义。
pub async fn admin_auth_gate_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> AppResult<Response> {
    if is_public_admin_auth_route(request.method(), request.uri().path()) {
        return Ok(next.run(request).await);
    }

    let (mut parts, body) = request.into_parts();
    AdminAuth::from_request_parts(&mut parts, &state).await?;
    Ok(next.run(Request::from_parts(parts, body)).await)
}

fn is_public_admin_auth_route(method: &Method, raw_path: &str) -> bool {
    if method == Method::OPTIONS {
        return true;
    }
    let path = raw_path.strip_prefix("/admin/api/v1").unwrap_or(raw_path);
    matches!(
        (method, path),
        (&Method::GET, "/auth/login/config")
            | (&Method::POST, "/auth/login")
            | (&Method::POST, "/auth/login/2fa")
            | (&Method::POST, "/auth/refresh")
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthActor {
    pub actor_type: ActorType,
    pub actor_id: u64,
    pub user_id: Option<u64>,
    /// 管理员凭据代际；用户与代理固定为零。改密后旧代际令牌即使在撤销竞态中晚到，也会被数据库闸门拒绝。
    pub auth_session_version: u64,
}

impl AuthActor {
    /// 组装认证主体三元组，其中 `user_id` 只有普通用户会填写，管理员与代理后台账号固定传 `None`。
    /// 这项区分决定下游能否把该主体当作交易账户使用，只有携带 `user_id` 的主体才允许进入用户资产相关流程。
    /// 本构造函数不校验主体是否真实存在或仍然活跃，调用方必须传入来自仓储查询的权威值，
    /// 不能拿请求体里的 ID 直接构造主体。
    pub fn new(actor_type: ActorType, actor_id: u64, user_id: Option<u64>) -> Self {
        Self {
            actor_type,
            actor_id,
            user_id,
            auth_session_version: 0,
        }
    }

    /// 覆盖从管理员凭据或刷新令牌记录读取的权威会话代际。
    pub(crate) fn with_auth_session_version(mut self, auth_session_version: u64) -> Self {
        self.auth_session_version = auth_session_version;
        self
    }

    /// 生成写入令牌 `sub` 声明的主体标识，格式是主体类型与主体 ID 以冒号连接。
    /// 加上类型前缀是为了让三张账号表中相同的自增 ID 不会碰撞成同一个身份；
    /// 刷新令牌时还会用它与令牌声明中的 `sub` 逐字符比对，确认令牌记录和声明指向同一个账号。
    pub fn subject(&self) -> String {
        format!("{}:{}", self.actor_type.as_str(), self.actor_id)
    }
}

#[derive(Debug, Clone)]
pub struct UserCredentials {
    pub email: Option<String>,
    pub phone: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub country_code: Option<String>,
    pub username_login_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct AdminRegistration {
    pub username: Option<String>,
    pub password: Option<String>,
    pub role_id: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct AdminCredentials {
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AgentRegistration {
    pub username: Option<String>,
    pub password: Option<String>,
    pub agent_id: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct AgentCredentials {
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
    pub scope: TokenScope,
}

#[derive(Debug, Clone)]
pub struct NewUserActor {
    pub email: Option<String>,
    pub phone: Option<String>,
    pub password_hash: String,
    pub country_code: String,
    pub preferred_locale: String,
}

#[derive(Debug, Clone)]
pub struct NewAdminActor {
    pub username: String,
    pub password_hash: String,
    pub role_id: u64,
}

#[derive(Debug, Clone)]
pub struct NewAgentActor {
    pub username: String,
    pub password_hash: String,
    pub agent_id: u64,
}

#[derive(Debug, Clone)]
pub struct StoredActorCredential {
    pub actor: AuthActor,
    pub password_hash: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct ActiveCountryConfig {
    pub country_code: String,
    pub default_locale: String,
}

#[derive(Debug, Clone)]
pub struct StoredRefreshToken {
    pub actor_type: ActorType,
    pub actor_id: u64,
    pub user_id: Option<u64>,
    pub auth_session_version: u64,
    pub token_hash: String,
    pub expires_at: NaiveDateTime,
}

/// Sa-Token 模式写入项目刷新令牌存储的领域无关数据快照。
///
/// `refresh_token` 只在基础设施适配器生成摘要键时短暂使用，持久化值不得包含原始令牌；
/// `expires_at` 同时用于读取校验和存储 TTL 计算。
#[derive(Debug, Clone)]
pub struct StoredProjectRefreshToken {
    pub refresh_token: String,
    pub actor_type: ActorType,
    pub actor_id: u64,
    pub user_id: Option<u64>,
    pub auth_session_version: u64,
    pub scope: TokenScope,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct RefreshTokenRecord {
    pub actor_type: ActorType,
    pub actor_id: u64,
    pub user_id: Option<u64>,
    pub auth_session_version: u64,
    pub scope: TokenScope,
}

/// 用 Argon2 默认参数散列账号口令，每次调用都以新生成的 v7 UUID 作为盐种子。
/// 逐次取新盐意味着同一个口令在不同账号上产生不同哈希，攻击者既无法比对哈希找出共用口令的账号，
/// 也无法用预先算好的表批量还原。返回值是自带算法参数与盐的 PHC 编码字符串，可直接入库。
/// 口令明文只在本次调用期间停留于内存，不写日志、不进错误信息，调用方应在散列完成后尽快丢弃明文。
/// 生成盐或散列失败只返回通用内部错误，不回显被散列的内容。
pub fn hash_password(password: &str) -> AppResult<String> {
    let salt_seed = Uuid::now_v7();
    let salt = SaltString::encode_b64(salt_seed.as_bytes())
        .map_err(|error| AppError::Internal(format!("failed to create password salt: {error}")))?;

    hash_with_salt(password, &salt)
}

/// 用存储的 PHC 编码哈希校验口令是否匹配，比对交由 Argon2 完成，不做会提前返回的逐字节比较。
/// 存储哈希无法解析时返回未匹配而不是错误，使被截断或写坏的历史记录只表现为登录失败，
/// 不会变成能被外部观察到的内部错误，攻击者因此无法据此判断某个账号的凭据记录是否异常。
/// 返回假同时涵盖口令不对与哈希不可用两种情况，调用方须把它们折叠成同一个未授权响应。
/// 本函数不感知账号状态、失败计数和锁定，这些判断由服务层围绕它完成。
pub fn verify_password(password_hash: &str, password: &str) -> AppResult<bool> {
    let parsed_hash = match PasswordHash::new(password_hash) {
        Ok(parsed_hash) => parsed_hash,
        Err(_) => return Ok(false),
    };

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

/// 用固定盐把刷新令牌散列成可作等值查询的摘要，同一枚令牌每次都得到相同结果。
/// 这里刻意不取随机盐：刷新时要拿客户端提交的令牌串直接反查记录，逐条随机盐会让等值查找无从下手。
/// 因此该摘要的目的是让原始令牌不落库，即便数据库内容外泄也无法直接拿去换取新会话，
/// 而不是提供逐条独立的抗离线破解强度。调用方只能持久化本函数的输出，任何情况下都不得存储原始令牌。
pub fn hash_refresh_token(refresh_token: &str) -> AppResult<String> {
    let salt = SaltString::encode_b64(REFRESH_TOKEN_HASH_SALT).map_err(|error| {
        AppError::Internal(format!("failed to create refresh token salt: {error}"))
    })?;

    hash_with_salt(refresh_token, &salt)
}

/// 强制下线一个主体：先逐个登出会话管理器中该主体已签发的访问令牌，再清空 Redis 里登记的刷新令牌。
/// 枚举或任一次登出失败都会立即上抛，调用方不得在无法证明旧访问令牌失效时返回成功；
/// 此时新凭据可能已在数据库生效，但余下会话仍需运维重试撤销并要求主体重新登录。
/// 两个后端各自独立操作，不具备整体原子性；未配置会话管理器或 Redis 时，对应步骤直接跳过并视为成功。
/// 本函数不改动数据库中的账号状态与口令哈希，调用方须先完成凭据变更再撤销会话，否则旧凭据仍可重新登录。
pub async fn revoke_actor_auth_sessions(state: &AppState, actor: &AuthActor) -> AppResult<()> {
    if let Some(manager) = &state.auth_manager {
        let tokens = match manager
            .get_token_value_list_by_login_id(
                actor.actor_type.as_str(),
                &actor.actor_id.to_string(),
                None,
            )
            .await
        {
            Ok(tokens) => tokens,
            // 从未创建过会话与“当前没有旧会话可撤销”等价；真实后端故障仍必须上抛。
            Err(SaTokenError::SessionNotFound) => Vec::new(),
            Err(error) => return Err(map_sa_token_error(error)),
        };
        for token in tokens {
            manager
                .logout(&TokenValue::new(token))
                .await
                .map_err(map_sa_token_error)?;
        }
    }

    if let Some(redis) = &state.redis {
        infrastructure::RedisProjectRefreshTokenRepository::new(redis.clone())
            .revoke_actor_refresh_tokens(actor)
            .await?;
    }

    Ok(())
}

/// 把会话后端错误收敛成两类：一切与令牌状态相关的失败统一变为未授权，其余归为内部错误。
/// 令牌不存在、已过期、格式非法、未登录、尚未激活、为空、长度不足，以及被踢下线或被顶替，
/// 对外都返回同一个响应，使调用方无法通过错误差异分辨会话是自然过期还是被管理员强制下线。
/// 只有真正的后端故障才会带上原始描述作为内部错误，这类细节不应出现在面向终端用户的响应里。
pub(crate) fn map_sa_token_error(error: SaTokenError) -> AppError {
    match error {
        SaTokenError::TokenNotFound
        | SaTokenError::TokenExpired
        | SaTokenError::InvalidToken(_)
        | SaTokenError::NotLogin
        | SaTokenError::TokenInactive
        | SaTokenError::TokenEmpty
        | SaTokenError::TokenTooShort
        | SaTokenError::AccountKickedOut
        | SaTokenError::AccountReplaced => AppError::Unauthorized,
        other => AppError::Internal(format!("sa-token operation failed: {other}")),
    }
}

/// 以调用方给定的盐执行 Argon2 散列并输出 PHC 编码字符串，是口令与刷新令牌两条散列路径的共同实现。
/// 参数沿用 Argon2 默认配置，调整默认值不会自动升级历史哈希，存量记录仍按其自带参数验证。
/// 盐的来源由调用方决定，正是这一点区分了口令的逐次随机盐和刷新令牌的固定盐两种用法。
/// 散列失败只返回通用内部错误，错误信息中不包含被散列的秘密内容。
fn hash_with_salt(secret: &str, salt: &SaltString) -> AppResult<String> {
    Argon2::default()
        .hash_password(secret.as_bytes(), salt)
        .map(|hash| hash.to_string())
        .map_err(|error| AppError::Internal(format!("failed to hash secret: {error}")))
}

/// 校验并规范化用户名：先去首尾空白并转 ASCII 小写，再要求长度为 3 到 32 且只含字母、数字和下划线。
/// 统一小写让用户名在唯一索引和登录查询中折叠成同一个键，避免仅靠大小写差异注册出视觉上重复的账号。
/// 字符集限制排除空格、同形异体字和控制字符，防止用户名被用来冒充他人或干扰日志与前端渲染。
/// 长度按 Unicode 字符计数，但字符集约束实际已将其限定为 ASCII。
/// 本函数只做格式判定，用户名是否已被占用、用户名登录开关是否开启，都由调用方另行检查。
pub fn normalize_username(value: &str) -> AppResult<String> {
    let username = value.trim().to_ascii_lowercase();
    let length = username.chars().count();
    if !(3..=32).contains(&length)
        || !username
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(AppError::Validation(
            "username must be 3-32 characters and contain only letters, numbers, or underscore"
                .to_owned(),
        ));
    }
    Ok(username)
}

/// 签发一枚 HS256 JWT，声明中写入主体标识、作用域、按传入存活秒数算出的过期时刻和一次性令牌 ID。
/// 访问令牌与刷新令牌共用本函数，二者仅靠存活时长区分，因此调用方必须传入正确的 TTL，
/// 否则会签出有效期远超预期的访问令牌。令牌 ID 每次重新生成，使同一主体在同一秒内签发的两枚令牌互不相同。
/// 签名密钥从配置中取出后只用于本次签名，不会进入令牌载荷、日志或错误信息。
/// 本函数不写任何服务端记录，签出的令牌在自然过期前无法被单独撤销，撤销只能依赖主体级的会话清理。
pub fn issue_token(
    settings: &Settings,
    subject: impl Into<String>,
    scope: TokenScope,
    ttl_seconds: u64,
) -> AppResult<String> {
    issue_token_with_session_version(settings, subject, scope, ttl_seconds, 0)
}

/// 为认证服务签发携带会话代际的 JWT。公开测试 helper 仍默认代际零，以兼容既有固定夹具。
fn issue_token_with_session_version(
    settings: &Settings,
    subject: impl Into<String>,
    scope: TokenScope,
    ttl_seconds: u64,
    auth_session_version: u64,
) -> AppResult<String> {
    let claims = Claims {
        sub: subject.into(),
        scope,
        exp: (Utc::now().timestamp() + ttl_seconds as i64) as usize,
        token_id: versioned_token_id(auth_session_version, Uuid::now_v7().to_string()),
    };

    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(settings.jwt_secret.expose_secret().as_bytes()),
    )
    .map_err(|error| AppError::Internal(format!("failed to issue jwt: {error}")))
}

/// 校验 JWT 的 HS256 签名与过期时间并取出声明，用于未启用服务端会话时的本地令牌验证。
/// 签名不符、载荷被篡改、令牌已过期或结构无法解析，都折叠成同一个未授权错误，底层原因不外泄，
/// 攻击者因此无法从响应中分辨自己构造的令牌是签名错误还是仅仅超时。
/// 本函数只验证令牌自身的完整性与时效，既不回查账号是否仍然活跃，也不检查作用域，
/// 这两项必须由调用方补齐，否则停用账号在令牌到期前仍可继续访问。
pub fn decode_claims(settings: &Settings, token: &str) -> AppResult<Claims> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(settings.jwt_secret.expose_secret().as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map(|data| data.claims)
    .map_err(|_| AppError::Unauthorized)
}

/// 从请求头中取出 `Authorization` 里的 Bearer 令牌，缺少头部、前缀不符或令牌为空都返回未授权。
/// 前缀区分大小写并要求恰好一个空格，不接受其他认证方案，避免把 Basic 之类的凭据误当令牌继续处理；
/// 头部含非 ASCII 等无法转成字符串的内容时同样按缺失处理，不做容错解析。
/// 返回的是尚未经过任何验证的原始请求头切片，调用方必须继续完成签名或会话校验才能信任其内容。
fn bearer_token(parts: &Parts) -> AppResult<&str> {
    let value = parts
        .headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(AppError::Unauthorized)?;
    let token = value
        .strip_prefix("Bearer ")
        .ok_or(AppError::Unauthorized)?;

    if token.is_empty() {
        Err(AppError::Unauthorized)
    } else {
        Ok(token)
    }
}

/// 校验一枚访问令牌并确认其作用域符合预期，是所有受保护接口共用的鉴权收口。
/// 配置了会话管理器时以服务端会话为准，强制下线可以立即生效；否则回落到本地 JWT 的签名与过期校验，
/// 此时令牌在到期前无法被单独撤销。两条路径对上层返回同一种声明结构。
/// 令牌本身不可用返回未授权，令牌有效但作用域不是所要求的那一类则返回禁止访问，
/// 这一区分让持用户令牌访问后台接口的请求不会被误报成未登录。
/// 本函数不查询账号表，账号在令牌有效期内被停用仍会通过，对实时性有要求的用例须自行回查主体状态。
pub async fn claims_from_bearer_token(
    state: &AppState,
    token: &str,
    required_scope: TokenScope,
) -> AppResult<Claims> {
    let claims = match &state.auth_manager {
        Some(manager) => claims_from_sa_token(manager, token).await?,
        None => decode_claims(&state.settings, token)?,
    };

    if claims.scope == required_scope {
        Ok(claims)
    } else {
        Err(AppError::Forbidden)
    }
}

/// 把会话管理器返回的会话信息转换成统一的 `Claims`，让两套令牌实现对上层呈现同一种结构。
/// 主体标识按登录类型与登录 ID 重新拼接，令牌 ID 直接取会话令牌串本身，与本地 JWT 的一次性 ID 含义不同。
/// 登录类型无法识别时判为未授权；会话未设置过期时间时过期字段填零，且时间戳会先夹到非负，
/// 因此这个字段只适合展示，不能拿来做本地过期判断，令牌是否仍然有效已经由管理器裁定。
fn claims_from_token_info(token_info: TokenInfo) -> AppResult<Claims> {
    let scope = TokenScope::from_login_type(&token_info.login_type)?;
    let exp = token_info
        .expire_time
        .map(|time| time.timestamp().max(0) as usize)
        .unwrap_or(0);

    let auth_session_version = token_info
        .extra_data
        .as_ref()
        .and_then(|extra| extra.get("auth_session_version"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);

    Ok(Claims {
        sub: format!("{}:{}", scope.as_login_type(), token_info.login_id),
        scope,
        exp,
        token_id: versioned_token_id(auth_session_version, token_info.token.to_string()),
    })
}

/// 从统一令牌 ID 中还原管理员凭据代际；历史令牌没有前缀，按迁移默认值零处理。
pub(crate) fn claims_auth_session_version(claims: &Claims) -> AppResult<u64> {
    let Some(encoded) = claims.token_id.strip_prefix("sv:") else {
        return Ok(0);
    };
    let (version, token_id) = encoded.split_once(':').ok_or(AppError::Unauthorized)?;
    if token_id.is_empty() {
        return Err(AppError::Unauthorized);
    }
    version.parse().map_err(|_| AppError::Unauthorized)
}

fn versioned_token_id(auth_session_version: u64, token_id: String) -> String {
    format!("sv:{auth_session_version}:{token_id}")
}

/// 向会话管理器查询令牌对应的服务端会话并转换成声明，令牌是否有效完全以后端记录为准。
/// 与本地 JWT 校验不同，这条路径能立刻反映登出、被顶替和管理员强制下线，令牌串本身不携带可信状态。
/// 各种会话失效原因经统一映射后都变为未授权，只有后端本身故障才会呈现为内部错误。
/// 每次校验都要访问一次会话存储，因此会话后端不可用会直接表现为受保护接口整体不可访问。
async fn claims_from_sa_token(manager: &SaTokenManager, token: &str) -> AppResult<Claims> {
    let token_info = manager
        .get_token_info(&TokenValue::new(token.to_owned()))
        .await
        .map_err(map_sa_token_error)?;

    claims_from_token_info(token_info)
}

/// 串起请求头解析与作用域校验：先取出 Bearer 令牌，再按所需作用域完成验证并返回声明。
/// 三个身份提取器共用本函数，使用户端、管理后台和代理后台的鉴权口径完全一致，
/// 不会出现某一侧漏掉作用域检查、导致令牌被跨端复用的情况。
/// 请求头缺失或令牌无效返回未授权，令牌有效但作用域不符返回禁止访问。
async fn require_scope(
    parts: &Parts,
    state: &AppState,
    required_scope: TokenScope,
) -> AppResult<Claims> {
    claims_from_bearer_token(state, bearer_token(parts)?, required_scope).await
}

#[async_trait]
impl FromRequestParts<AppState> for UserAuth {
    type Rejection = AppError;

    /// 为用户端接口提取身份，要求 Bearer 令牌的作用域恰好是用户，管理员或代理令牌一律被拒。
    /// 提取失败会直接以 `AppError` 作为拒绝响应，处理函数因此不会在缺少有效用户身份的情况下被执行。
    /// 提取只保证令牌此刻有效，不代表该用户账号仍然活跃，涉及资金的用例须自行回查账号状态。
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        require_scope(parts, state, TokenScope::User)
            .await
            .map(Self)
    }
}

#[async_trait]
impl FromRequestParts<AppState> for AdminAuth {
    type Rejection = AppError;

    /// 为管理后台接口提取身份，先校验 admin 作用域，再回查管理员状态、角色和当前路由权限。
    /// 权限不嵌入 JWT，因此停用账号或收紧角色会在下一次请求立即生效；只有未挂载 MySQL 的轻量路由单测会略过第二步。
    /// 声明中的主体标识形如 `admin:<id>`，下游需要管理员 ID 时应从中解析，不得信任请求体里传来的值。
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let claims = require_scope(parts, state, TokenScope::Admin).await?;
        if let Some(pool) = state.mysql.as_ref() {
            crate::modules::admin::application::authorize_admin_request(
                pool,
                &claims.sub,
                claims_auth_session_version(&claims)?,
                parts.method.as_str(),
                parts.uri.path(),
            )
            .await?;
        }
        Ok(Self(claims))
    }
}

#[async_trait]
impl FromRequestParts<AppState> for AgentAuth {
    type Rejection = AppError;

    /// 为代理后台接口提取身份，只接受代理作用域的令牌，使代理商与平台管理员的权限域彻底分开。
    /// 令牌有效即通过：代理公司及其上级链路是否仍然活跃只在登录和刷新时校验，此处不再回查，
    /// 因此代理被停用后其尚未过期的访问令牌仍能通过提取，需要立即阻断时必须主动撤销该主体的会话。
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        require_scope(parts, state, TokenScope::Agent)
            .await
            .map(Self)
    }
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_auth_mod_tests.rs"]
mod tests;
