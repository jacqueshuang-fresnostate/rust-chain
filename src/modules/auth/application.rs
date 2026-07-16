//! auth bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。

use crate::{
    error::{AppError, AppResult},
    infra::email::verification_code_email_message,
    modules::{
        admin::application::load_enabled_admin_smtp_config,
        auth::presentation::{
            LoginTwoFactorChallengeResponse, LoginTwoFactorSetupChallengeResponse, TokenResponse,
            UserAuthRequest, UserLoginResponse,
        },
        auth::{
            ActorType, AdminCredentials, AdminRegistration, AgentCredentials, AuthActor,
            AuthService, IssuedTokens, MySqlAuthRepository, TokenScope, UserCredentials,
            domain::{
                optional_string, required_string, validate_email_code, validate_registration_email,
                validate_reset_password,
            },
            hash_password,
            infrastructure::{
                bind_registered_user_referral_in_tx, create_user_invite_code_in_tx,
                ensure_email_purpose_not_cooling_down_in_tx,
                ensure_registration_email_available_in_tx,
                ensure_registration_email_not_cooling_down_in_tx,
                increment_email_verification_attempt_in_tx,
                insert_registration_email_verification_in_tx, insert_user_email_verification_in_tx,
                insert_verified_user_in_tx, load_password_reset_user_id,
                lock_latest_pending_email_verification_by_purpose_in_tx,
                lock_password_reset_user_in_tx, lock_registration_country_in_tx,
                lock_verified_user_email_in_tx, mark_email_verification_verified_in_tx,
                prepare_referral_binding_in_tx, revoke_user_refresh_tokens_in_tx,
                supersede_pending_email_verifications_in_tx,
                supersede_pending_registration_email_codes_in_tx, update_user_password_in_tx,
                verify_registration_email_code_in_tx,
            },
            revoke_actor_auth_sessions, verify_password,
        },
        countries::normalize_country_code,
        security::{
            LoginTwoFactorChallengeType, LoginTwoFactorMode, consume_login_two_factor_challenge,
            create_login_two_factor_challenge, ensure_login_challenge_usable,
            load_login_two_factor_challenge, load_security_policy, load_user_two_factor,
            reset_user_two_factor, verify_user_totp,
        },
    },
    state::AppState,
};
use chrono::{DateTime, Duration, Utc};
use ring::rand::{SecureRandom, SystemRandom};
use sqlx::{MySql, Pool};

pub(crate) fn auth_service(state: &AppState) -> AppResult<AuthService<MySqlAuthRepository>> {
    let pool = mysql_pool(state)?;

    Ok(AuthService::new(
        MySqlAuthRepository::new(pool),
        state.settings.clone(),
        state.auth_manager.clone(),
        state.redis.clone(),
    ))
}

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

pub(crate) async fn register_admin_actor(
    state: &AppState,
    registration: AdminRegistration,
) -> AppResult<IssuedTokens> {
    auth_service(state)?.register_admin(registration).await
}

pub(crate) async fn login_admin_actor(
    state: &AppState,
    credentials: AdminCredentials,
) -> AppResult<IssuedTokens> {
    auth_service(state)?.login_admin(credentials).await
}

pub(crate) async fn login_agent_actor(
    state: &AppState,
    credentials: AgentCredentials,
) -> AppResult<IssuedTokens> {
    auth_service(state)?.login_agent(credentials).await
}

pub(crate) async fn refresh_actor_tokens(
    state: &AppState,
    refresh_token: Option<String>,
    expected_scope: TokenScope,
) -> AppResult<IssuedTokens> {
    auth_service(state)?
        .refresh(refresh_token, expected_scope)
        .await
}

pub(crate) fn reject_agent_registration() -> AppResult<IssuedTokens> {
    // 代理账号由后台业务流程创建，公开认证入口只允许登录和刷新，避免用户绕过代理审核。
    Err(AppError::Forbidden)
}

pub(crate) async fn load_register_config(pool: &Pool<MySql>) -> AppResult<RegisterConfig> {
    let policy = load_security_policy(pool).await?;

    Ok(RegisterConfig {
        email_code_required: true,
        invite_code_required: policy.registration_invite_required,
    })
}

pub(crate) async fn load_login_config(pool: &Pool<MySql>) -> AppResult<LoginConfig> {
    let policy = load_security_policy(pool).await?;

    Ok(LoginConfig {
        username_login_enabled: policy.username_login_enabled,
    })
}

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

    tx.commit().await?;

    auth_service(state)?
        .issue_tokens_for_actor(AuthActor::new(ActorType::User, user_id, Some(user_id)))
        .await
}

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

pub(crate) async fn login_user_with_optional_two_factor_response(
    state: &AppState,
    pool: &Pool<MySql>,
    request: UserAuthRequest,
) -> AppResult<UserLoginResponse> {
    // 登录返回值在应用层统一映射，路由层不承担 outcome 分支。
    let outcome = login_user_with_optional_two_factor(
        state,
        pool,
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

pub(crate) async fn send_password_reset_email_code(
    state: &AppState,
    pool: &Pool<MySql>,
    email: String,
) -> AppResult<DateTime<Utc>> {
    let email = validate_registration_email(Some(email))?;
    let user_id = load_password_reset_user_id(pool, &email).await?;

    send_email_code_for_purpose(state, pool, user_id, "password_reset", "重置登录密码验证码").await
}

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

fn user_id_from_actor(actor: &AuthActor) -> AppResult<u64> {
    if actor.actor_type != ActorType::User {
        return Err(AppError::Unauthorized);
    }
    actor.user_id.ok_or(AppError::Unauthorized)
}

fn generate_email_code() -> AppResult<String> {
    let rng = SystemRandom::new();
    let mut bytes = [0_u8; 4];
    rng.fill(&mut bytes)
        .map_err(|_| AppError::Internal("email verification code generation failed".to_owned()))?;
    let value = u32::from_be_bytes(bytes) % 1_000_000;
    Ok(format!("{value:06}"))
}
