//! auth bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。
//!
//! 本文件把认证限界上下文的对外用例串成完整流程：注册、登录、二次验证、邮件验证码与密码重置，
//! 并在此界定事务边界，逐个说明哪些步骤必须同事务提交、哪些只能分步执行、失败后会残留哪些已生效的写入。
//! 人机校验统一排在所有口令校验之前，Turnstile 的服务端密钥与校验地址只从运行时环境读取，不入库也不外发。
//! 这里也是 TOTP 密钥唯一被解密并以明文外发的位置，明文只回给已通过前置校验的本人，任何一层都不得记录。
//! 需要特别注意的是，二次验证挑战的可用性检查与消费大多不是同一个原子操作，
//! 各函数注释分别写明了自己的并发与重放边界，调用方不得默认这些用例具备严格的一次性语义。

use crate::{
    error::{AppError, AppResult},
    infra::{
        email::verification_code_email_message,
        secrets::{decrypt_secret, encrypt_secret},
    },
    modules::{
        admin::{
            application::{authorize_admin_permission, load_enabled_admin_smtp_config},
            service::admin_id_from_subject,
        },
        auth::presentation::{
            AdminLoginResponse, AdminPasswordChangeRequest, AdminPasswordChangeResponse,
            AdminTwoFactorSetupResponse, AdminTwoFactorStatusResponse, LoginTransportContext,
            LoginTwoFactorChallengeResponse, LoginTwoFactorSetupChallengeResponse,
            LoginTwoFactorSetupResponse, TokenResponse, UserAuthRequest, UserLoginResponse,
        },
        auth::{
            ActorType, AdminCredentials, AdminRegistration, AgentCredentials, AuthActor,
            AuthService, IssuedTokens, MySqlAuthRepository, TokenScope, UserCredentials,
            claims_from_bearer_token,
            domain::{
                LoginTurnstilePolicy, optional_string, required_string, validate_email_code,
                validate_registration_email, validate_reset_password,
            },
            hash_password,
            infrastructure::{
                CF_TURNSTILE_SITEVERIFY_URL, RedisProjectRefreshTokenRepository,
                bind_registered_user_referral_in_tx, create_user_invite_code_in_tx,
                ensure_email_purpose_not_cooling_down_in_tx,
                ensure_registration_email_available_in_tx,
                ensure_registration_email_not_cooling_down_in_tx,
                increment_email_verification_attempt_in_tx,
                insert_admin_password_change_audit_in_tx,
                insert_registration_email_verification_in_tx, insert_user_email_verification_in_tx,
                insert_verified_user_in_tx, load_admin_username, load_password_reset_user_id,
                lock_admin_password_credential_in_tx,
                lock_latest_pending_email_verification_by_purpose_in_tx,
                lock_password_reset_user_in_tx, lock_registration_country_in_tx,
                lock_verified_user_email_in_tx, mark_email_verification_verified_in_tx,
                prepare_referral_binding_in_tx, revoke_admin_refresh_tokens_in_tx,
                revoke_user_refresh_tokens_in_tx, supersede_pending_email_verifications_in_tx,
                supersede_pending_registration_email_codes_in_tx, update_admin_password_in_tx,
                update_user_password_in_tx, verify_registration_email_code_in_tx,
                verify_turnstile_site_response,
            },
            revoke_actor_auth_sessions, verify_password,
        },
        countries::normalize_country_code,
        events::{infrastructure::insert_event_in_tx, user_created_outbox_event},
        security::domain::login_challenge_expired,
        security::{
            LoginTwoFactorChallengeType, LoginTwoFactorMode, confirm_admin_totp, confirm_user_totp,
            consume_admin_login_two_factor_challenge, consume_login_two_factor_challenge,
            create_admin_login_two_factor_challenge, create_login_two_factor_challenge,
            credential_encryption_key, ensure_admin_login_challenge_usable,
            ensure_login_challenge_usable, generate_totp_secret,
            increment_admin_login_two_factor_attempt, load_admin_login_two_factor_challenge,
            load_admin_two_factor, load_login_two_factor_challenge, load_security_policy,
            load_user_two_factor, reset_admin_two_factor, reset_user_two_factor,
            save_pending_admin_totp_secret, save_pending_totp_secret, totp_otpauth_uri,
            verify_admin_totp, verify_totp_code, verify_user_totp,
        },
        user::infrastructure::load_user_account_label,
    },
    state::AppState,
};
use axum::http::{HeaderMap, header::AUTHORIZATION};
use chrono::{DateTime, Duration, Utc};
use ring::rand::{SecureRandom, SystemRandom};
use sqlx::{MySql, Pool};
use std::sync::Arc;

#[derive(Debug, Clone)]
struct TurnstileRuntimeConfig {
    secret: Option<String>,
    site_key: Option<String>,
    enforce_token: bool,
    siteverify_url: String,
}

impl TurnstileRuntimeConfig {
    /// 从进程环境读取登录前置人机校验配置，密钥变量名兼容旧的 `CF_TURNSTILE_SECRET_KEY` 写法。
    /// 密钥与站点公钥都会先去首尾空白，整形后为空按未配置处理，避免部署时遗留的空变量被当成已启用。
    /// 强制标志只认几个明确的真值写法，其余取值以及读取失败一律视为关闭，保持默认宽松不误伤登录。
    /// 校验接口地址缺省回落到内置常量。本函数每次调用都重新读取环境，既不缓存也不检查密钥是否真的可用。
    fn from_env() -> Self {
        let secret = std::env::var("CF_TURNSTILE_SECRET")
            .ok()
            .or_else(|| std::env::var("CF_TURNSTILE_SECRET_KEY").ok())
            .and_then(normalized_env_value);
        let site_key = std::env::var("CF_TURNSTILE_SITE_KEY")
            .ok()
            .and_then(normalized_env_value);
        let enforce_token = std::env::var("CF_TURNSTILE_ENFORCE_TOKEN")
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false);
        let siteverify_url = std::env::var("CF_TURNSTILE_SITEVERIFY_URL")
            .unwrap_or_else(|_| CF_TURNSTILE_SITEVERIFY_URL.to_owned());

        Self {
            secret,
            site_key,
            enforce_token,
            siteverify_url,
        }
    }
}

/// 从应用状态组装 MySQL 认证仓储、可选 Sa-Token 管理器与 Redis 刷新令牌端口。
/// 缺少 MySQL 配置立即失败；构造过程不访问数据库或 Redis，连接错误在具体用例中上抛。
pub(crate) fn auth_service(state: &AppState) -> AppResult<AuthService<MySqlAuthRepository>> {
    let pool = mysql_pool(state)?;
    let project_refresh_tokens = state.redis.clone().map(|manager| {
        Arc::new(RedisProjectRefreshTokenRepository::new(manager))
            as Arc<dyn crate::modules::auth::ProjectRefreshTokenRepository>
    });

    Ok(AuthService::new(
        MySqlAuthRepository::new(pool),
        state.settings.clone(),
        state.auth_manager.clone(),
        project_refresh_tokens,
    ))
}

/// 从应用状态取出认证持久化所用的 MySQL 连接池副本，未配置时返回语义明确的内部错误。
/// 克隆的是连接池句柄而不是新建连接，开销很低，各用例按需现取即可，无须在上层缓存或长期持有。
/// 返回错误代表服务缺少必需的数据库配置，属于部署问题而非业务失败，不应被折算成面向用户的校验错误。
pub(crate) fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for auth persistence".to_owned())
    })
}

pub(crate) struct RegisterConfig {
    pub(crate) email_code_required: bool,
    pub(crate) invite_code_required: bool,
}

pub(crate) struct LoginConfig {
    pub(crate) username_login_enabled: bool,
    pub(crate) cf_turnstile_enabled: bool,
    pub(crate) cf_turnstile_site_key: Option<String>,
}

pub(crate) struct RegisterUserWithEmailCodeInput {
    pub(crate) email: Option<String>,
    pub(crate) password: Option<String>,
    pub(crate) code: Option<String>,
    pub(crate) country_code: Option<String>,
    pub(crate) invite_code: Option<String>,
    pub(crate) promotion: Option<String>,
}

pub(crate) struct UserLoginInput {
    pub(crate) email: Option<String>,
    pub(crate) phone: Option<String>,
    pub(crate) username: Option<String>,
    pub(crate) password: Option<String>,
}

pub(crate) enum UserLoginOutcome {
    Tokens(IssuedTokens),
    TwoFactorChallenge {
        challenge_id: String,
        expires_in_seconds: i64,
    },
    TwoFactorSetupChallenge {
        setup_challenge_id: String,
        expires_in_seconds: i64,
    },
}

/// 编排管理员创建：必须先验证管理员作用域与账号写权限；匿名首管理员创建已由显式 migrator 引导取代。
/// 凭证解析或账号写入失败不签发令牌；插入与令牌签发不是同一事务，会话失败后账号可能已经存在。
pub(crate) async fn register_admin_actor(
    state: &AppState,
    headers: &HeaderMap,
    registration: AdminRegistration,
) -> AppResult<IssuedTokens> {
    let token = admin_bearer_token(headers).ok_or(AppError::Unauthorized)?;
    let claims = claims_from_bearer_token(state, token, TokenScope::Admin).await?;
    let requester_subject = claims.sub.clone();
    authorize_admin_permission(
        &mysql_pool(state)?,
        &requester_subject,
        super::claims_auth_session_version(&claims)?,
        "admin.accounts.write",
    )
    .await?;

    auth_service(state)?
        .register_admin(Some(&requester_subject), registration)
        .await
}

/// 从请求头中取出管理员注册请求必须携带的 Bearer 令牌，缺失、前缀不符或令牌为空串都返回未提供。
/// 本函数只做前缀剥离，不验证签名、不检查作用域，返回值必须再经过完整的令牌校验和管理员写权限校验才能当作身份使用。
fn admin_bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|token| !token.is_empty())
}

/// 验证管理员密码后读取二次验证状态；未绑定直接签发令牌，已绑定只创建挑战。
/// 凭证验证会清除失败计数并记录适配器支持的登录时间，这些写入不因后续 2FA 查询、
/// 挑战创建或令牌后端失败而回滚；重复密码登录会新增五分钟挑战且不撤销旧挑战。
pub(crate) async fn login_admin_actor(
    state: &AppState,
    pool: &Pool<MySql>,
    credentials: AdminCredentials,
) -> AppResult<AdminLoginResponse> {
    let service = auth_service(state)?;
    let actor = service.verify_admin_credentials(credentials).await?;
    // 管理员 2FA 按账号自愿绑定，未绑定的账号维持原有登录行为，避免上线即锁死存量管理员。
    if !load_admin_two_factor(pool, actor.actor_id)
        .await?
        .totp_enabled
    {
        return Ok(AdminLoginResponse::Token(
            service.issue_tokens_for_actor(actor).await?.into(),
        ));
    }

    let challenge =
        create_admin_login_two_factor_challenge(pool, actor.actor_id, actor.auth_session_version)
            .await?;

    Ok(AdminLoginResponse::TwoFactorChallenge(
        LoginTwoFactorChallengeResponse {
            requires_2fa: true,
            challenge_id: challenge.challenge_id,
            expires_in_seconds: challenge.expires_in_seconds,
        },
    ))
}

/// 在管理员密码与 2FA 流程前按运行时策略调用 Turnstile；校验时会把服务端 secret、
/// 客户端 token 与可选远端 IP 发往配置 URL，失败时不查账号也不累计账号登录失败次数。
pub(crate) async fn login_admin_with_turnstile(
    state: &AppState,
    credentials: AdminCredentials,
    turnstile_token: Option<String>,
    transport: LoginTransportContext,
) -> AppResult<AdminLoginResponse> {
    verify_login_turnstile(turnstile_token, transport).await?;
    let pool = mysql_pool(state)?;
    login_admin_actor(state, &pool, credentials).await
}

/// 读取管理员挑战后检查过期、消费状态和试码上限，再校验 TOTP、写消费时间并签发令牌。
/// 错码会另发 SQL 累加次数；检查与消费不是带行数判定的同一原子操作，并发请求可能都越过
/// 前置检查。消费写入后令牌后端失败不会恢复挑战，调用方需重新完成密码登录。
pub(crate) async fn verify_admin_login_two_factor(
    state: &AppState,
    pool: &Pool<MySql>,
    challenge_id: String,
    totp_code: String,
) -> AppResult<TokenResponse> {
    let challenge = load_admin_login_two_factor_challenge(pool, &challenge_id).await?;
    ensure_admin_login_challenge_usable(&challenge)?;
    if let Err(error) = verify_admin_totp(
        pool,
        state.settings.as_ref(),
        challenge.admin_id,
        &totp_code,
    )
    .await
    {
        // 试码失败计入挑战次数，用尽后挑战作废，攻击者必须重新通过密码登录。
        increment_admin_login_two_factor_attempt(pool, &challenge.challenge_id).await?;
        return Err(error);
    }
    consume_admin_login_two_factor_challenge(pool, &challenge.challenge_id).await?;

    let tokens = auth_service(state)?
        .issue_tokens_for_actor(
            AuthActor::new(ActorType::Admin, challenge.admin_id, None)
                .with_auth_session_version(challenge.auth_session_version),
        )
        .await?;

    Ok(tokens.into())
}

/// 校验当前管理员旧口令后，在同一 MySQL 事务中更新哈希、清除首次强制改密标志、撤销数据库刷新令牌并写审计。
/// 提交事务前必须先成功撤销 Sa-Token/Redis 会话；任一撤销失败就回滚口令变更，不返回“已改密但旧会话仍有效”的成功响应。
/// 成功响应不签发替代令牌，调用方必须使用新口令重新登录。
pub(crate) async fn change_admin_password(
    state: &AppState,
    pool: &Pool<MySql>,
    subject: &str,
    request: AdminPasswordChangeRequest,
) -> AppResult<AdminPasswordChangeResponse> {
    let admin_id = admin_id_from_subject(subject)?;
    let current_password = required_string(request.current_password, "current_password")?;
    let new_password =
        validate_reset_password(&required_string(request.new_password, "new_password")?)?;
    if current_password == new_password {
        return Err(AppError::Validation(
            "new_password must be different from current_password".to_owned(),
        ));
    }
    let password_hash = hash_password(&new_password)?;

    let mut tx = pool.begin().await?;
    let credential = lock_admin_password_credential_in_tx(&mut tx, admin_id).await?;
    if credential.status != "active"
        || !verify_password(&credential.password_hash, &current_password)?
    {
        return Err(AppError::Unauthorized);
    }
    update_admin_password_in_tx(&mut tx, admin_id, &password_hash).await?;
    revoke_admin_refresh_tokens_in_tx(&mut tx, admin_id).await?;
    insert_admin_password_change_audit_in_tx(&mut tx, admin_id, credential.must_change_password)
        .await?;
    // 不允许“数据库改密成功但旧访问会话仍有效”的成功响应。
    // 先将可撤销的服务端会话全部下线，任一后端失败则不提交口令与闸门事务。
    revoke_actor_auth_sessions(state, &AuthActor::new(ActorType::Admin, admin_id, None)).await?;
    tx.commit().await?;
    Ok(AdminPasswordChangeResponse {
        changed: true,
        requires_relogin: true,
    })
}

/// 按 `admin:<id>` 形式的主体标识读取该管理员的二次验证启用状态，主体格式非法时按未授权处理。
/// 只返回是否启用的布尔值，既不返回也不解密已保存的密钥或待确认密钥，避免状态查询变成密钥泄露通道。
/// 主体标识取自令牌声明而非请求参数，因此调用方无法借这个接口窥探其他管理员的绑定情况。
pub(crate) async fn get_admin_two_factor_status(
    pool: &Pool<MySql>,
    subject: &str,
) -> AppResult<AdminTwoFactorStatusResponse> {
    let two_factor = load_admin_two_factor(pool, admin_id_from_subject(subject)?).await?;

    Ok(AdminTwoFactorStatusResponse {
        totp_enabled: two_factor.totp_enabled,
    })
}

/// 读取未绑定状态后生成 TOTP 密钥，加密保存为待确认值，并返回含明文 secret 的导入信息。
/// 状态检查与 upsert 分离，并发启用可能被后写待确认值关闭；调用方须把响应限定给当前管理员，
/// 不得记录 URI/secret。本步骤不签发或撤销会话。
pub(crate) async fn setup_admin_two_factor(
    state: &AppState,
    pool: &Pool<MySql>,
    subject: &str,
) -> AppResult<AdminTwoFactorSetupResponse> {
    let admin_id = admin_id_from_subject(subject)?;
    if load_admin_two_factor(pool, admin_id).await?.totp_enabled {
        return Err(AppError::security_validation(
            "2fa_already_enabled",
            "2FA 已绑定",
        ));
    }
    let key = credential_encryption_key(state.settings.as_ref())?;
    let account = load_admin_username(pool, admin_id).await?;
    let secret = generate_totp_secret()?;
    save_pending_admin_totp_secret(pool, admin_id, &encrypt_secret(&secret, key)?).await?;

    Ok(AdminTwoFactorSetupResponse {
        otpauth_uri: totp_otpauth_uri("Exchange Admin", &account, &secret),
        secret,
    })
}

/// 解密当前读取到的管理员待确认密钥并校验 TOTP，随后以该密文启用绑定。
/// 已绑定、未生成密钥或动态码错误不改状态；确认写采用 upsert，不比较读取后的并发替换，
/// 也不签发或撤销管理员会话。
pub(crate) async fn confirm_admin_two_factor(
    state: &AppState,
    pool: &Pool<MySql>,
    subject: &str,
    totp_code: String,
) -> AppResult<AdminTwoFactorStatusResponse> {
    let admin_id = admin_id_from_subject(subject)?;
    let two_factor = load_admin_two_factor(pool, admin_id).await?;
    if two_factor.totp_enabled {
        return Err(AppError::security_validation(
            "2fa_already_enabled",
            "2FA 已绑定",
        ));
    }
    let encrypted_secret = two_factor.totp_secret_encrypted.ok_or_else(|| {
        AppError::security_validation("security_verification_required", "请先生成 2FA 密钥")
    })?;
    let secret = decrypt_secret(
        &encrypted_secret,
        credential_encryption_key(state.settings.as_ref())?,
    )?;
    if !verify_totp_code(&secret, &totp_code, Utc::now())? {
        return Err(AppError::security_validation(
            "invalid_2fa_code",
            "2FA 验证码错误",
        ));
    }
    confirm_admin_totp(pool, admin_id, &encrypted_secret).await?;

    Ok(AdminTwoFactorStatusResponse { totp_enabled: true })
}

/// 要求管理员先通过当前 TOTP 再清除绑定，防止会话被劫持后直接关闭二次验证。
/// 动态码错误时不改状态；清除采用幂等 upsert，不在此处撤销已有管理员会话。
pub(crate) async fn disable_admin_two_factor(
    state: &AppState,
    pool: &Pool<MySql>,
    subject: &str,
    totp_code: String,
) -> AppResult<AdminTwoFactorStatusResponse> {
    let admin_id = admin_id_from_subject(subject)?;
    // 解绑必须先通过一次有效验证码，避免会话被劫持后直接摘掉第二因子。
    verify_admin_totp(pool, state.settings.as_ref(), admin_id, &totp_code).await?;
    reset_admin_two_factor(pool, admin_id).await?;

    Ok(AdminTwoFactorStatusResponse {
        totp_enabled: false,
    })
}

/// 验证代理管理员凭证、账号和整条祖先状态，成功后签发独立代理作用域会话。
/// 错误密码累计统一锁定计数；成功后的登录记录与令牌存储不是同一事务，后者失败不回滚前者。
pub(crate) async fn login_agent_actor(
    state: &AppState,
    credentials: AgentCredentials,
) -> AppResult<IssuedTokens> {
    auth_service(state)?.login_agent(credentials).await
}

/// 在代理账号认证前按 clearance/强制策略执行 Turnstile；需校验时把服务端 secret、客户端 token
/// 与可选远端 IP 发往配置 URL。失败不查代理凭据，服务方或网络错误直接上抛。
pub(crate) async fn login_agent_with_turnstile(
    state: &AppState,
    credentials: AgentCredentials,
    turnstile_token: Option<String>,
    transport: LoginTransportContext,
) -> AppResult<IssuedTokens> {
    verify_login_turnstile(turnstile_token, transport).await?;
    login_agent_actor(state, credentials).await
}

/// 校验刷新令牌、预期作用域与主体活跃状态后另签一组会话。
/// 当前实现不消费传入令牌，故同一有效刷新令牌可重复调用；主体级撤销或到期后才失效。
pub(crate) async fn refresh_actor_tokens(
    state: &AppState,
    refresh_token: Option<String>,
    expected_scope: TokenScope,
) -> AppResult<IssuedTokens> {
    auth_service(state)?
        .refresh(refresh_token, expected_scope)
        .await
}

/// 直接返回禁止访问，把公开的代理注册入口彻底封死，既不解析参数也不访问任何存储。
/// 代理账号只能经后台审核流程创建，其层级路径与归属关系由业务侧派生，自助注册会整体绕过这些约束。
/// 由于不查询任何存储，本函数不会泄露用户名是否已被占用，也不产生失败计数或审计记录。
pub(crate) fn reject_agent_registration() -> AppResult<IssuedTokens> {
    // 代理账号由后台业务流程创建，公开认证入口只允许登录和刷新，避免用户绕过代理审核。
    Err(AppError::Forbidden)
}

/// 组装注册页所需的开关：邮件验证码在当前实现中恒为必填，邀请码则取自可后台调整的安全策略。
/// 邮件验证码写死为必填，是因为注册用例本身无条件消费验证码，这个标志只服务于前端展示，
/// 不能被理解成一个可以反向关闭后端校验的开关。
/// 读取策略失败会直接上抛，此时前端拿不到配置，不应退化成放开其中任何一项要求。
pub(crate) async fn load_register_config(pool: &Pool<MySql>) -> AppResult<RegisterConfig> {
    let policy = load_security_policy(pool).await?;

    Ok(RegisterConfig {
        email_code_required: true,
        invite_code_required: policy.registration_invite_required,
    })
}

/// 查询数据库登录策略并读取 Turnstile 环境变量，对外只返回用户名开关、enabled 和公开 site_key。
/// 服务端 secret 与 Siteverify URL 不进入响应；MySQL 查询失败会上抛。
pub(crate) async fn load_login_config(state: &AppState) -> AppResult<LoginConfig> {
    let policy = load_security_policy(&mysql_pool(state)?).await?;
    let runtime = TurnstileRuntimeConfig::from_env();
    let (cf_turnstile_enabled, cf_turnstile_site_key) = turnstile_login_config(&runtime);

    Ok(LoginConfig {
        username_login_enabled: policy.username_login_enabled,
        cf_turnstile_enabled,
        cf_turnstile_site_key,
    })
}

/// 编排邮件码注册：同事务锁国家配置、消费验证码、写用户、邀请关系与 outbox。
/// 错码仅提交试错计数；其他事务内失败整体回滚。提交后才签发令牌，令牌后端失败时
/// 已注册用户、邀请关系和 outbox 仍然保留；重放注册受邮箱唯一约束阻断。
pub(crate) async fn register_user_with_email_code(
    state: &AppState,
    pool: &Pool<MySql>,
    input: RegisterUserWithEmailCodeInput,
) -> AppResult<IssuedTokens> {
    let policy = load_security_policy(pool).await?;
    let email = validate_registration_email(input.email)?;
    let password = required_string(input.password, "password")?;
    let password_hash = hash_password(&password)?;
    let code = required_string(input.code, "code")?;
    let country_code =
        normalize_country_code(&required_string(input.country_code, "country_code")?)?;
    let invite_code =
        optional_string(input.invite_code).or_else(|| optional_string(input.promotion));

    // 注册邀请码开关属于安全策略，应用层统一读取，避免 HTTP 层复制业务判断。
    if policy.registration_invite_required && invite_code.is_none() {
        return Err(AppError::Validation("invite_code is required".to_owned()));
    }

    let now = Utc::now();
    let mut tx = pool.begin().await?;
    let country = lock_registration_country_in_tx(&mut tx, &country_code).await?;
    match verify_registration_email_code_in_tx(&mut tx, &email, &code, now).await {
        Ok(()) => {}
        Err(error) if matches!(error, AppError::Validation(_)) => {
            tx.commit().await?;
            return Err(error);
        }
        Err(error) => return Err(error),
    }
    let referral_binding = match invite_code {
        Some(code) => Some(prepare_referral_binding_in_tx(&mut tx, &code).await?),
        None => None,
    };

    let user_id = insert_verified_user_in_tx(
        &mut tx,
        &email,
        &password_hash,
        &country.country_code,
        &country.default_locale,
        now,
    )
    .await?;

    create_user_invite_code_in_tx(&mut tx, user_id).await?;
    if let Some(binding) = referral_binding {
        bind_registered_user_referral_in_tx(&mut tx, user_id, binding).await?;
    }
    insert_event_in_tx(&mut tx, &user_created_outbox_event(user_id, now)).await?;

    tx.commit().await?;

    auth_service(state)?
        .issue_tokens_for_actor(AuthActor::new(ActorType::User, user_id, Some(user_id)))
        .await
}

/// 将传输请求字段映射为邮件码注册用例，成功后保持历史令牌响应结构。
/// 所有事务、邀请幂等与 outbox 副作用均由内层用例负责，映射失败不会额外签发令牌。
pub(crate) async fn register_user_with_email_code_response(
    state: &AppState,
    pool: &Pool<MySql>,
    request: UserAuthRequest,
) -> AppResult<TokenResponse> {
    // 统一在应用层完成请求字段映射，路由层仅保留请求提取。
    let tokens = register_user_with_email_code(
        state,
        pool,
        RegisterUserWithEmailCodeInput {
            email: request.email,
            password: request.password,
            code: request.code,
            country_code: request.country_code,
            invite_code: request.invite_code,
            promotion: request.promotion,
        },
    )
    .await?;

    Ok(tokens.into())
}

/// 验证用户口令后统一执行登录 2FA 策略；调用方须先完成 Turnstile，并传入当前 MySQL 连接池。
/// 未命中 2FA 时签发会话；命中时新增五分钟挑战且不提前签发 token。重复登录会新增挑战，
/// 本函数不撤销旧挑战；口令成功已清除失败计数并记录登录，后续 SQL/令牌失败不回滚这些写入。
pub(crate) async fn login_user_with_optional_two_factor(
    state: &AppState,
    pool: &Pool<MySql>,
    input: UserLoginInput,
) -> AppResult<UserLoginOutcome> {
    let policy = load_security_policy(pool).await?;
    let service = auth_service(state)?;
    let actor = service
        .verify_user_credentials(UserCredentials {
            email: input.email,
            phone: input.phone,
            username: input.username,
            password: input.password,
            country_code: None,
            username_login_enabled: policy.username_login_enabled,
        })
        .await?;
    let user_id = user_id_from_actor(&actor)?;
    let two_factor = load_user_two_factor(pool, user_id).await?;

    let requires_challenge = match policy.login_2fa_mode {
        LoginTwoFactorMode::None => false,
        LoginTwoFactorMode::UserEnabled => two_factor.totp_enabled && two_factor.login_2fa_enabled,
        LoginTwoFactorMode::Mandatory => true,
    };

    if !requires_challenge {
        let tokens = service.issue_tokens_for_actor(actor).await?;
        return Ok(UserLoginOutcome::Tokens(tokens));
    }

    let challenge_type = if two_factor.totp_enabled {
        LoginTwoFactorChallengeType::LoginTwoFactor
    } else {
        LoginTwoFactorChallengeType::SetupTwoFactor
    };
    let challenge = create_login_two_factor_challenge(pool, user_id, challenge_type).await?;

    match challenge_type {
        LoginTwoFactorChallengeType::LoginTwoFactor => Ok(UserLoginOutcome::TwoFactorChallenge {
            challenge_id: challenge.challenge_id,
            expires_in_seconds: challenge.expires_in_seconds,
        }),
        LoginTwoFactorChallengeType::SetupTwoFactor => {
            Ok(UserLoginOutcome::TwoFactorSetupChallenge {
                setup_challenge_id: challenge.challenge_id,
                expires_in_seconds: challenge.expires_in_seconds,
            })
        }
    }
}

/// 用户登录入口：按运行时策略先调用 Turnstile Siteverify，再进入口令与 2FA 编排。
/// 需校验时，挑战 token、服务端 secret 与可选远端 IP 会发送至配置 URL；失败不查账号。
/// 成功返回一组令牌或一个新挑战，响应中不暴露 TOTP 密钥、密码哈希或 Turnstile secret。
pub(crate) async fn login_user_with_optional_two_factor_response(
    state: &AppState,
    request: UserAuthRequest,
    transport: LoginTransportContext,
) -> AppResult<UserLoginResponse> {
    verify_login_turnstile(request.cf_turnstile_token, transport).await?;
    let pool = mysql_pool(state)?;
    // 登录返回值在应用层统一映射，路由层不承担 outcome 分支。
    let outcome = login_user_with_optional_two_factor(
        state,
        &pool,
        UserLoginInput {
            email: request.email,
            phone: request.phone,
            username: request.username,
            password: request.password,
        },
    )
    .await?;

    Ok(match outcome {
        UserLoginOutcome::Tokens(tokens) => UserLoginResponse::Token(tokens.into()),
        UserLoginOutcome::TwoFactorChallenge {
            challenge_id,
            expires_in_seconds,
        } => UserLoginResponse::TwoFactorChallenge(LoginTwoFactorChallengeResponse {
            requires_2fa: true,
            challenge_id,
            expires_in_seconds,
        }),
        UserLoginOutcome::TwoFactorSetupChallenge {
            setup_challenge_id,
            expires_in_seconds,
        } => UserLoginResponse::TwoFactorSetupChallenge(LoginTwoFactorSetupChallengeResponse {
            requires_2fa_setup: true,
            setup_challenge_id,
            expires_in_seconds,
        }),
    })
}

/// 登录前置人机校验的统一入口，每次调用都重新读取一遍运行时配置再执行判定。
/// 每次重读意味着运维改动环境变量后无需重启即可生效，代价是每次登录都要访问一次进程环境。
/// 未启用或本次无需校验时直接放行；需要校验而令牌缺失则返回安全校验错误，此时完全不会去查账号。
async fn verify_login_turnstile(
    turnstile_token: Option<String>,
    transport: LoginTransportContext,
) -> AppResult<()> {
    let runtime = TurnstileRuntimeConfig::from_env();
    verify_login_turnstile_with_runtime(turnstile_token, transport, &runtime).await
}

/// 用显式传入的运行时配置执行人机校验判定，把环境读取与判定逻辑分离，便于覆盖各种配置组合的测试。
/// 先由领域策略判断本次是否需要校验，不需要就直接放行；即便需要校验，缺少服务端密钥时同样放行，
/// 因为此时根本无法完成核验，拒绝全部登录的代价远大于放行。
/// 确定要校验后，纯空白令牌等同于缺失并返回专门的错误码，随后携带密钥、令牌和可选来源 IP 请求站点校验。
/// 本函数不接触任何账号数据，校验失败也不会累计该账号的登录失败次数，因此不能替代口令侧的爆破防护。
async fn verify_login_turnstile_with_runtime(
    turnstile_token: Option<String>,
    transport: LoginTransportContext,
    runtime: &TurnstileRuntimeConfig,
) -> AppResult<()> {
    let policy = login_turnstile_policy(runtime);
    if !policy.requires_verification(transport.has_cf_clearance) {
        return Ok(());
    }
    let Some(secret) = runtime.secret.as_deref() else {
        return Ok(());
    };
    let token = turnstile_token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| {
            AppError::security_validation(
                "CF_TURNSTILE_TOKEN_MISSING",
                "cf_turnstile_token is required",
            )
        })?;

    verify_turnstile_site_response(
        &runtime.siteverify_url,
        secret,
        token,
        transport.remote_ip.as_deref(),
    )
    .await
}

/// 把运行时配置压缩成领域策略对象，只传递密钥与站点公钥是否存在，不把密钥内容带进领域层。
/// 这样领域层可以在完全不接触敏感值的前提下判定是否需要校验，密钥始终只停留在应用层和请求发送处。
fn login_turnstile_policy(runtime: &TurnstileRuntimeConfig) -> LoginTurnstilePolicy {
    LoginTurnstilePolicy::new(
        runtime.secret.is_some(),
        runtime.site_key.is_some(),
        runtime.enforce_token,
    )
}

/// 从运行时配置中挑出登录配置接口可以公开的两项：是否启用人机校验，以及供前端渲染组件的站点公钥。
/// 站点公钥本就要嵌入页面，公开无妨；服务端密钥与站点校验接口地址刻意不出现在返回值里，
/// 使调用方即便直接把结果序列化进响应，也不会顺手把敏感配置一起带出去。
fn turnstile_login_config(runtime: &TurnstileRuntimeConfig) -> (bool, Option<String>) {
    (
        login_turnstile_policy(runtime).enabled(),
        runtime.site_key.clone(),
    )
}

/// 把环境变量取值整形成有意义的配置项：去掉首尾空白，整形后为空则判定为未配置。
/// 部署中常见的空赋值和多余空格会因此被当成缺失，而不是变成一把空密钥去调用站点校验并必然失败。
fn normalized_env_value(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

/// 为未注册邮箱生成十分钟验证码，同事务校验占用与冷却并取代旧码。
/// 先提交哈希记录后调用 SMTP；发送失败时记录仍存在且受冷却约束，明文不入库。
pub(crate) async fn send_registration_email_code(
    state: &AppState,
    pool: &Pool<MySql>,
    email: String,
) -> AppResult<DateTime<Utc>> {
    let email = validate_registration_email(Some(email))?;
    let now = Utc::now();
    let expires_at = now + Duration::minutes(10);
    let code = generate_email_code()?;
    let code_hash = hash_password(&code)?;
    let sender = state
        .email_sender
        .clone()
        .ok_or_else(|| AppError::Internal("email sender is not configured".to_owned()))?;
    let smtp_config = load_enabled_admin_smtp_config(
        pool,
        state.settings.as_ref().exposed_credential_encryption_key(),
    )
    .await?
    .ok_or_else(|| AppError::Internal("enabled smtp config is not configured".to_owned()))?;

    let mut tx = pool.begin().await?;
    ensure_registration_email_available_in_tx(&mut tx, &email).await?;
    ensure_registration_email_not_cooling_down_in_tx(&mut tx, &email, now).await?;
    supersede_pending_registration_email_codes_in_tx(&mut tx, &email).await?;
    insert_registration_email_verification_in_tx(&mut tx, &email, &code_hash, expires_at, now)
        .await?;
    tx.commit().await?;

    let message = verification_code_email_message(
        email.to_owned(),
        "注册验证码",
        &code,
        10,
        smtp_config.verification_code_template_html_for_purpose("register"),
    );
    sender.send(smtp_config, message).await?;

    Ok(expires_at)
}

/// 为活跃用户的已验证邮箱发送指定用途验证码，用途同时隔离冷却和消费。
/// 事务中锁邮箱、取代旧码并存哈希；提交后发信，SMTP 失败不回滚已保存记录。
pub(crate) async fn send_email_code_for_purpose(
    state: &AppState,
    pool: &Pool<MySql>,
    user_id: u64,
    purpose: &'static str,
    subject: &'static str,
) -> AppResult<DateTime<Utc>> {
    let now = Utc::now();
    let expires_at = now + Duration::minutes(10);
    let code = generate_email_code()?;
    let code_hash = hash_password(&code)?;
    let sender = state
        .email_sender
        .clone()
        .ok_or_else(|| AppError::Internal("email sender is not configured".to_owned()))?;
    let smtp_config = load_enabled_admin_smtp_config(
        pool,
        state.settings.as_ref().exposed_credential_encryption_key(),
    )
    .await?
    .ok_or_else(|| AppError::Internal("enabled smtp config is not configured".to_owned()))?;

    let mut tx = pool.begin().await?;
    let email = lock_verified_user_email_in_tx(&mut tx, user_id).await?;
    ensure_email_purpose_not_cooling_down_in_tx(&mut tx, user_id, &email, purpose, now).await?;
    supersede_pending_email_verifications_in_tx(&mut tx, user_id, purpose).await?;
    insert_user_email_verification_in_tx(
        &mut tx, user_id, &email, purpose, &code_hash, expires_at, now,
    )
    .await?;
    tx.commit().await?;

    let message = verification_code_email_message(
        email,
        subject,
        &code,
        10,
        smtp_config.verification_code_template_html_for_purpose(purpose),
    );
    sender.send(smtp_config, message).await?;

    Ok(expires_at)
}

/// 校验邮箱格式后定位对应的活跃且已完成邮箱验证的用户，再走通用发送流程下发密码重置验证码。
/// 未注册、已停用或邮箱未验证的地址会返回校验错误而不是静默成功，因此该入口可被用来判断邮箱是否已注册。
/// 用途固定为密码重置，与注册和二次验证重置的验证码彼此隔离，发送冷却与消费也各自独立计算。
/// 本函数不校验调用方身份，任何人都能为他人邮箱触发一封重置邮件，真正的授权发生在提交验证码那一步。
pub(crate) async fn send_password_reset_email_code(
    state: &AppState,
    pool: &Pool<MySql>,
    email: String,
) -> AppResult<DateTime<Utc>> {
    let email = validate_registration_email(Some(email))?;
    let user_id = load_password_reset_user_id(pool, &email).await?;

    send_email_code_for_purpose(state, pool, user_id, "password_reset", "重置登录密码验证码").await
}

/// 读取并校验用户登录挑战及 TOTP，记录 TOTP 验证时间、写消费时间后再签发令牌。
/// 动态码错误不消费挑战；检查与消费是独立 SQL，且消费更新不检查受影响行数，
/// 因而本函数不提供并发请求的原子防重放保证。消费后令牌失败不会恢复挑战。
pub(crate) async fn verify_login_two_factor_and_issue_tokens(
    state: &AppState,
    pool: &Pool<MySql>,
    challenge_id: String,
    totp_code: String,
) -> AppResult<IssuedTokens> {
    let challenge = load_login_two_factor_challenge(pool, &challenge_id).await?;
    ensure_login_challenge_usable(&challenge, LoginTwoFactorChallengeType::LoginTwoFactor)?;
    verify_user_totp(pool, state.settings.as_ref(), challenge.user_id, &totp_code).await?;
    consume_login_two_factor_challenge(pool, &challenge.challenge_id).await?;

    auth_service(state)?
        .issue_tokens_for_actor(AuthActor::new(
            ActorType::User,
            challenge.user_id,
            Some(challenge.user_id),
        ))
        .await
}

/// 验证首次绑定挑战后生成 TOTP 密钥，加密保存待确认值并返回剩余有效期。
/// 已绑定用户不覆盖密钥；本步骤不消费挑战、不签发令牌，可在挑战期内重新生成。
pub(crate) async fn setup_login_two_factor_challenge(
    state: &AppState,
    pool: &Pool<MySql>,
    challenge_id: String,
) -> AppResult<LoginTwoFactorSetupResponse> {
    let challenge = load_login_two_factor_challenge(pool, &challenge_id).await?;
    ensure_login_challenge_usable(&challenge, LoginTwoFactorChallengeType::SetupTwoFactor)?;
    if load_user_two_factor(pool, challenge.user_id)
        .await?
        .totp_enabled
    {
        return Err(AppError::security_validation(
            "2fa_already_enabled",
            "2FA 已绑定",
        ));
    }

    let key = credential_encryption_key(state.settings.as_ref())?;
    let secret = generate_totp_secret()?;
    save_pending_totp_secret(pool, challenge.user_id, &encrypt_secret(&secret, key)?).await?;
    let account = load_user_account_label(pool, challenge.user_id)
        .await?
        .unwrap_or_else(|| format!("user:{}", challenge.user_id));

    Ok(LoginTwoFactorSetupResponse {
        secret: secret.clone(),
        otpauth_uri: totp_otpauth_uri("Exchange", &account, &secret),
        expires_in_seconds: (challenge.expires_at - Utc::now()).num_seconds().max(0),
    })
}

/// 校验首次绑定挑战与当前待确认 TOTP，先启用密钥，再以条件更新消费挑战并签发令牌。
/// 启用、消费和令牌签发不在同一事务：消费竞争失败时 TOTP 可能已经启用；
/// 消费成功后的令牌后端失败不会恢复挑战，调用方需重新登录。
pub(crate) async fn confirm_login_two_factor_setup_and_issue_tokens(
    state: &AppState,
    pool: &Pool<MySql>,
    challenge_id: String,
    totp_code: String,
) -> AppResult<IssuedTokens> {
    let challenge = load_login_two_factor_challenge(pool, &challenge_id).await?;
    ensure_login_challenge_usable(&challenge, LoginTwoFactorChallengeType::SetupTwoFactor)?;
    let two_factor = load_user_two_factor(pool, challenge.user_id).await?;
    if two_factor.totp_enabled {
        return Err(AppError::security_validation(
            "2fa_already_enabled",
            "2FA 已绑定",
        ));
    }
    let encrypted_secret = two_factor.totp_secret_encrypted.ok_or_else(|| {
        AppError::security_validation("security_verification_required", "请先生成 2FA 密钥")
    })?;
    let secret = decrypt_secret(
        &encrypted_secret,
        credential_encryption_key(state.settings.as_ref())?,
    )?;
    if !verify_totp_code(&secret, &totp_code, Utc::now())? {
        return Err(AppError::security_validation(
            "invalid_2fa_code",
            "2FA 验证码错误",
        ));
    }

    confirm_user_totp(pool, challenge.user_id, &encrypted_secret).await?;
    consume_setup_login_two_factor_challenge(pool, &challenge.challenge_id).await?;

    auth_service(state)?
        .issue_tokens_for_actor(AuthActor::new(
            ActorType::User,
            challenge.user_id,
            Some(challenge.user_id),
        ))
        .await
}

/// 以带条件的单条更新消费首次绑定挑战，只有仍未被消费且尚未过期的挑战才会被打上消费时间。
/// 通过检查受影响行数判定是否真的抢到了这次消费，因此并发提交同一挑战时只有一个请求成功，
/// 其余会得到挑战失效错误，这是本文件中少数具备原子防重放能力的消费路径。
/// 判定完全交给数据库条件完成，本函数不预读挑战状态，也不区分挑战是已被消费还是已经过期。
async fn consume_setup_login_two_factor_challenge(
    pool: &Pool<MySql>,
    challenge_id: &str,
) -> AppResult<()> {
    let result = sqlx::query(
        r#"UPDATE login_two_factor_challenges
           SET consumed_at = CURRENT_TIMESTAMP(6)
           WHERE challenge_id = ?
             AND challenge_type = ?
             AND consumed_at IS NULL
             AND expires_at > CURRENT_TIMESTAMP(6)"#,
    )
    .bind(challenge_id)
    .bind(LoginTwoFactorChallengeType::SetupTwoFactor.as_str())
    .execute(pool)
    .await?;

    if result.rows_affected() != 1 {
        return Err(login_challenge_expired());
    }

    Ok(())
}

/// 校验登录挑战后向挑战用户已验证邮箱发送二次验证重置码。
/// 挑战过期或类型不符时不创建邮件码；发送冷却、事务与 SMTP 失败语义复用通用发送用例。
pub(crate) async fn send_login_two_factor_reset_email_code(
    state: &AppState,
    pool: &Pool<MySql>,
    challenge_id: String,
) -> AppResult<DateTime<Utc>> {
    let challenge = load_login_two_factor_challenge(pool, &challenge_id).await?;
    ensure_login_challenge_usable(&challenge, LoginTwoFactorChallengeType::LoginTwoFactor)?;

    send_email_code_for_purpose(
        state,
        pool,
        challenge.user_id,
        "login_2fa_reset",
        "重置登录 2FA 验证码",
    )
    .await
}

/// 在有效登录挑战下消费专用邮件码，成功后清除用户 TOTP 并消费挑战。
/// 邮件码在自己的事务中提交消费，重置 TOTP 与挑战消费随后分别执行；任一后续 SQL 失败
/// 不回滚先前步骤。错码只在邮件码事务中累计次数，不清除 TOTP。
pub(crate) async fn reset_login_two_factor_with_email_code(
    pool: &Pool<MySql>,
    challenge_id: String,
    code: String,
) -> AppResult<()> {
    let challenge = load_login_two_factor_challenge(pool, &challenge_id).await?;
    ensure_login_challenge_usable(&challenge, LoginTwoFactorChallengeType::LoginTwoFactor)?;
    verify_email_code_for_purpose(pool, challenge.user_id, &code, "login_2fa_reset").await?;
    reset_user_two_factor(pool, challenge.user_id).await?;
    consume_login_two_factor_challenge(pool, &challenge.challenge_id).await
}

/// 在事务中锁定已验证邮箱与指定用途最新待验证码，成功时标记已验证。
/// 错码只累加尝试次数并提交；过期、超限或缺失不消费其他用途验证码。
pub(crate) async fn verify_email_code_for_purpose(
    pool: &Pool<MySql>,
    user_id: u64,
    code: &str,
    purpose: &'static str,
) -> AppResult<()> {
    let code = validate_email_code(code)?;
    let now = Utc::now();
    let mut tx = pool.begin().await?;
    let email = lock_verified_user_email_in_tx(&mut tx, user_id).await?;
    let verification =
        lock_latest_pending_email_verification_by_purpose_in_tx(&mut tx, user_id, &email, purpose)
            .await?
            .ok_or_else(|| AppError::Validation("email verification code is invalid".to_owned()))?;
    if verification.expires_at <= now || verification.attempt_count >= 5 {
        return Err(AppError::Validation(
            "email verification code is expired".to_owned(),
        ));
    }
    if !verify_password(&verification.code_hash, &code)? {
        increment_email_verification_attempt_in_tx(&mut tx, verification.id).await?;
        tx.commit().await?;
        return Err(AppError::Validation(
            "email verification code is invalid".to_owned(),
        ));
    }

    mark_email_verification_verified_in_tx(&mut tx, verification.id, now).await?;
    tx.commit().await?;
    Ok(())
}

/// 先在独立事务消费密码重置码，再锁定用户并于第二个事务更新哈希、撤销 MySQL 刷新令牌。
/// 提交后尝试撤销 Sa-Token/Redis 会话但不签发新令牌；外部会话撤销失败会上抛，
/// 此时新密码与数据库撤销已生效，验证码也已消费，调用方应重新登录。
pub(crate) async fn reset_password_with_email_code(
    state: &AppState,
    pool: &Pool<MySql>,
    email: String,
    code: String,
    password: String,
) -> AppResult<()> {
    let email = validate_registration_email(Some(email))?;
    let code = validate_email_code(&code)?;
    let password = validate_reset_password(&password)?;
    let user_id = load_password_reset_user_id(pool, &email).await?;

    verify_email_code_for_purpose(pool, user_id, &code, "password_reset").await?;

    let password_hash = hash_password(&password)?;
    let mut tx = pool.begin().await?;
    let locked_user_id = lock_password_reset_user_in_tx(&mut tx, user_id, &email).await?;
    update_user_password_in_tx(&mut tx, locked_user_id, &password_hash).await?;
    revoke_user_refresh_tokens_in_tx(&mut tx, locked_user_id).await?;
    tx.commit().await?;

    revoke_actor_auth_sessions(
        state,
        &AuthActor::new(ActorType::User, locked_user_id, Some(locked_user_id)),
    )
    .await
}

/// 从认证主体中取出用户 ID，同时要求该主体的类型必须是普通用户。
/// 管理员与代理主体不携带用户 ID，若放行它们，后续二次验证和邮件码流程会误用另一张表的自增 ID，
/// 从而落到一个毫不相干的用户身上，因此这里把类型不符与 ID 缺失都判为未授权。
fn user_id_from_actor(actor: &AuthActor) -> AppResult<u64> {
    if actor.actor_type != ActorType::User {
        return Err(AppError::Unauthorized);
    }
    actor.user_id.ok_or(AppError::Unauthorized)
}

/// 用系统密码学安全随机源生成六位数字邮件验证码，不足六位时左侧补零。
/// 取四字节随机数对一百万取模会让偏小的数值概率略高，但偏差相对六位空间可以忽略，
/// 配合十分钟有效期、五次试错上限与发送冷却后不构成可利用的偏置。
/// 随机源失败按内部错误上抛，绝不退化成时间戳一类可预测的取值。
/// 返回的明文只用于组装邮件正文，落库的始终是它的哈希，因此明文不得写入日志或事件。
fn generate_email_code() -> AppResult<String> {
    let rng = SystemRandom::new();
    let mut bytes = [0_u8; 4];
    rng.fill(&mut bytes)
        .map_err(|_| AppError::Internal("email verification code generation failed".to_owned()))?;
    let value = u32::from_be_bytes(bytes) % 1_000_000;
    Ok(format!("{value:06}"))
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_auth_application_tests.rs"]
mod tests;
