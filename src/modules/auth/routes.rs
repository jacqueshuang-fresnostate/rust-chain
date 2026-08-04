use crate::{
    error::{AppError, AppResult},
    modules::auth::{
        AdminAuth, AdminCredentials, AdminRegistration, AgentCredentials, TokenScope,
        application::{
            confirm_admin_two_factor, confirm_login_two_factor_setup_and_issue_tokens,
            disable_admin_two_factor, get_admin_two_factor_status, load_login_config,
            load_register_config, login_admin_actor, login_agent_actor,
            login_user_with_optional_two_factor_response, mysql_pool, refresh_actor_tokens,
            register_admin_actor, register_user_with_email_code_response,
            reject_agent_registration, reset_login_two_factor_with_email_code,
            reset_password_with_email_code, send_login_two_factor_reset_email_code,
            send_password_reset_email_code, send_registration_email_code, setup_admin_two_factor,
            setup_login_two_factor_challenge, verify_admin_login_two_factor,
            verify_login_two_factor_and_issue_tokens,
        },
        presentation::{
            AdminAuthRequest, AdminLoginResponse, AdminTwoFactorCodeRequest,
            AdminTwoFactorSetupResponse, AdminTwoFactorStatusResponse, AgentAuthRequest,
            LoginConfigResponse, LoginTwoFactorCodeResponse, LoginTwoFactorRequest,
            LoginTwoFactorResetCodeRequest, LoginTwoFactorResetRequest,
            LoginTwoFactorResetResponse, LoginTwoFactorSetupConfirmRequest,
            LoginTwoFactorSetupRequest, LoginTwoFactorSetupResponse, PasswordResetCodeRequest,
            PasswordResetCodeResponse, PasswordResetRequest, PasswordResetResponse, RefreshRequest,
            RegisterConfigResponse, RegisterEmailCodeRequest, RegisterEmailCodeResponse,
            TokenResponse, UserAuthRequest, UserLoginResponse,
        },
    },
    state::AppState,
};
use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    routing::{get, post},
};
use chrono::Utc;
use serde::Deserialize;
use std::time::Duration;

const CF_TURNSTILE_SITEVERIFY: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";

pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/register/config", get(get_register_config))
        .route("/auth/login/config", get(get_login_config))
        .route("/auth/register/email-code", post(send_register_email_code))
        .route("/auth/register", post(user_register))
        .route("/auth/password/reset-code", post(send_password_reset_code))
        .route("/auth/password/reset", post(reset_password))
        .route("/auth/login", post(user_login))
        .route("/auth/login/2fa", post(user_login_two_factor))
        .route("/auth/login/2fa/setup", post(user_login_two_factor_setup))
        .route(
            "/auth/login/2fa/setup/confirm",
            post(user_login_two_factor_setup_confirm),
        )
        .route(
            "/auth/login/2fa/reset-code",
            post(send_login_two_factor_reset_code),
        )
        .route("/auth/login/2fa/reset", post(reset_login_two_factor))
        .route("/auth/refresh", post(user_refresh))
}

pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(admin_register))
        .route("/auth/login/config", get(get_login_config))
        .route("/auth/login", post(admin_login))
        .route("/auth/login/2fa", post(admin_login_two_factor))
        .route("/auth/2fa", get(admin_two_factor_status))
        .route("/auth/2fa/setup", post(admin_two_factor_setup))
        .route("/auth/2fa/confirm", post(admin_two_factor_confirm))
        .route("/auth/2fa/disable", post(admin_two_factor_disable))
        .route("/auth/refresh", post(admin_refresh))
}

pub fn agent_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(agent_register))
        .route("/auth/login", post(agent_login))
        .route("/auth/refresh", post(agent_refresh))
}

async fn get_register_config(
    State(state): State<AppState>,
) -> AppResult<Json<RegisterConfigResponse>> {
    let config = load_register_config(&mysql_pool(&state)?).await?;

    Ok(Json(RegisterConfigResponse {
        email_code_required: config.email_code_required,
        invite_code_required: config.invite_code_required,
    }))
}

async fn get_login_config(State(state): State<AppState>) -> AppResult<Json<LoginConfigResponse>> {
    let config = load_login_config(&mysql_pool(&state)?).await?;
    let (cf_turnstile_enabled, cf_turnstile_site_key) = get_login_turnstile_policy();

    Ok(Json(LoginConfigResponse {
        username_login_enabled: config.username_login_enabled,
        cf_turnstile_enabled,
        cf_turnstile_site_key,
    }))
}

async fn send_register_email_code(
    State(state): State<AppState>,
    Json(request): Json<RegisterEmailCodeRequest>,
) -> AppResult<Json<RegisterEmailCodeResponse>> {
    let pool = mysql_pool(&state)?;
    let expires_at = send_registration_email_code(&state, &pool, request.email).await?;

    Ok(Json(RegisterEmailCodeResponse {
        sent: true,
        expires_in_seconds: (expires_at - Utc::now()).num_seconds().max(0),
    }))
}

async fn send_password_reset_code(
    State(state): State<AppState>,
    Json(request): Json<PasswordResetCodeRequest>,
) -> AppResult<Json<PasswordResetCodeResponse>> {
    let pool = mysql_pool(&state)?;
    let expires_at = send_password_reset_email_code(&state, &pool, request.email).await?;

    Ok(Json(PasswordResetCodeResponse {
        sent: true,
        expires_in_seconds: (expires_at - Utc::now()).num_seconds().max(0),
    }))
}

async fn reset_password(
    State(state): State<AppState>,
    Json(request): Json<PasswordResetRequest>,
) -> AppResult<Json<PasswordResetResponse>> {
    let pool = mysql_pool(&state)?;
    reset_password_with_email_code(&state, &pool, request.email, request.code, request.password)
        .await?;

    Ok(Json(PasswordResetResponse {
        reset: true,
        requires_relogin: true,
    }))
}

async fn user_register(
    State(state): State<AppState>,
    Json(request): Json<UserAuthRequest>,
) -> AppResult<Json<TokenResponse>> {
    let pool = mysql_pool(&state)?;
    let tokens = register_user_with_email_code_response(&state, &pool, request).await?;

    Ok(Json(tokens))
}

async fn user_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UserAuthRequest>,
) -> AppResult<Json<UserLoginResponse>> {
    verify_cf_turnstile_token(request.cf_turnstile_token.as_deref(), &headers).await?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        login_user_with_optional_two_factor_response(&state, &pool, request).await?,
    ))
}

async fn user_refresh(
    State(state): State<AppState>,
    Json(request): Json<RefreshRequest>,
) -> AppResult<Json<TokenResponse>> {
    let tokens = refresh_actor_tokens(&state, request.refresh_token, TokenScope::User).await?;

    Ok(Json(tokens.into()))
}

async fn user_login_two_factor(
    State(state): State<AppState>,
    Json(request): Json<LoginTwoFactorRequest>,
) -> AppResult<Json<TokenResponse>> {
    let pool = mysql_pool(&state)?;
    let tokens = verify_login_two_factor_and_issue_tokens(
        &state,
        &pool,
        request.challenge_id,
        request.totp_code,
    )
    .await?;

    Ok(Json(tokens.into()))
}

async fn user_login_two_factor_setup(
    State(state): State<AppState>,
    Json(request): Json<LoginTwoFactorSetupRequest>,
) -> AppResult<Json<LoginTwoFactorSetupResponse>> {
    let pool = mysql_pool(&state)?;
    Ok(Json(
        setup_login_two_factor_challenge(&state, &pool, request.setup_challenge_id).await?,
    ))
}

async fn user_login_two_factor_setup_confirm(
    State(state): State<AppState>,
    Json(request): Json<LoginTwoFactorSetupConfirmRequest>,
) -> AppResult<Json<TokenResponse>> {
    let pool = mysql_pool(&state)?;
    let tokens = confirm_login_two_factor_setup_and_issue_tokens(
        &state,
        &pool,
        request.setup_challenge_id,
        request.totp_code,
    )
    .await?;

    Ok(Json(tokens.into()))
}

async fn send_login_two_factor_reset_code(
    State(state): State<AppState>,
    Json(request): Json<LoginTwoFactorResetCodeRequest>,
) -> AppResult<Json<LoginTwoFactorCodeResponse>> {
    let pool = mysql_pool(&state)?;
    let expires_at =
        send_login_two_factor_reset_email_code(&state, &pool, request.challenge_id).await?;

    Ok(Json(LoginTwoFactorCodeResponse {
        sent: true,
        expires_in_seconds: (expires_at - Utc::now()).num_seconds().max(0),
    }))
}

async fn reset_login_two_factor(
    State(state): State<AppState>,
    Json(request): Json<LoginTwoFactorResetRequest>,
) -> AppResult<Json<LoginTwoFactorResetResponse>> {
    let pool = mysql_pool(&state)?;
    reset_login_two_factor_with_email_code(&pool, request.challenge_id, request.code).await?;

    Ok(Json(LoginTwoFactorResetResponse {
        reset: true,
        requires_relogin: true,
    }))
}

async fn admin_register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AdminAuthRequest>,
) -> AppResult<Json<TokenResponse>> {
    let tokens = register_admin_actor(
        &state,
        &headers,
        AdminRegistration {
            username: request.username,
            password: request.password,
            role_id: request.role_id,
        },
    )
    .await?;

    Ok(Json(tokens.into()))
}

async fn admin_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AdminAuthRequest>,
) -> AppResult<Json<AdminLoginResponse>> {
    verify_cf_turnstile_token(request.cf_turnstile_token.as_deref(), &headers).await?;
    let pool = mysql_pool(&state)?;
    let response = login_admin_actor(
        &state,
        &pool,
        AdminCredentials {
            username: request.username,
            password: request.password,
        },
    )
    .await?;

    Ok(Json(response))
}

async fn admin_login_two_factor(
    State(state): State<AppState>,
    Json(request): Json<LoginTwoFactorRequest>,
) -> AppResult<Json<TokenResponse>> {
    let pool = mysql_pool(&state)?;
    let tokens =
        verify_admin_login_two_factor(&state, &pool, request.challenge_id, request.totp_code)
            .await?;

    Ok(Json(tokens))
}

async fn admin_two_factor_status(
    State(state): State<AppState>,
    AdminAuth(claims): AdminAuth,
) -> AppResult<Json<AdminTwoFactorStatusResponse>> {
    let pool = mysql_pool(&state)?;
    let status = get_admin_two_factor_status(&pool, &claims.sub).await?;

    Ok(Json(status))
}

async fn admin_two_factor_setup(
    State(state): State<AppState>,
    AdminAuth(claims): AdminAuth,
) -> AppResult<Json<AdminTwoFactorSetupResponse>> {
    let pool = mysql_pool(&state)?;
    let setup = setup_admin_two_factor(&state, &pool, &claims.sub).await?;

    Ok(Json(setup))
}

async fn admin_two_factor_confirm(
    State(state): State<AppState>,
    AdminAuth(claims): AdminAuth,
    Json(request): Json<AdminTwoFactorCodeRequest>,
) -> AppResult<Json<AdminTwoFactorStatusResponse>> {
    let pool = mysql_pool(&state)?;
    let status = confirm_admin_two_factor(&state, &pool, &claims.sub, request.totp_code).await?;

    Ok(Json(status))
}

async fn admin_two_factor_disable(
    State(state): State<AppState>,
    AdminAuth(claims): AdminAuth,
    Json(request): Json<AdminTwoFactorCodeRequest>,
) -> AppResult<Json<AdminTwoFactorStatusResponse>> {
    let pool = mysql_pool(&state)?;
    let status = disable_admin_two_factor(&state, &pool, &claims.sub, request.totp_code).await?;

    Ok(Json(status))
}

async fn admin_refresh(
    State(state): State<AppState>,
    Json(request): Json<RefreshRequest>,
) -> AppResult<Json<TokenResponse>> {
    let tokens = refresh_actor_tokens(&state, request.refresh_token, TokenScope::Admin).await?;

    Ok(Json(tokens.into()))
}

async fn agent_register(
    State(_state): State<AppState>,
    Json(_request): Json<AgentAuthRequest>,
) -> AppResult<Json<TokenResponse>> {
    let tokens = reject_agent_registration()?;

    Ok(Json(tokens.into()))
}

async fn agent_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AgentAuthRequest>,
) -> AppResult<Json<TokenResponse>> {
    verify_cf_turnstile_token(request.cf_turnstile_token.as_deref(), &headers).await?;
    let tokens = login_agent_actor(
        &state,
        AgentCredentials {
            username: request.username,
            password: request.password,
        },
    )
    .await?;

    Ok(Json(tokens.into()))
}

async fn agent_refresh(
    State(state): State<AppState>,
    Json(request): Json<RefreshRequest>,
) -> AppResult<Json<TokenResponse>> {
    let tokens = refresh_actor_tokens(&state, request.refresh_token, TokenScope::Agent).await?;

    Ok(Json(tokens.into()))
}

#[derive(Debug, Deserialize)]
struct CfTurnstileVerifyResponse {
    success: bool,
    #[serde(rename = "error-codes", default)]
    error_codes: Vec<String>,
    hostname: Option<String>,
}

fn extract_client_ip(headers: &HeaderMap) -> Option<String> {
    headers
        .get("cf-connecting-ip")
        .or_else(|| headers.get("x-forwarded-for"))
        .and_then(|header| header.to_str().ok())
        .map(|value: &str| value.split(',').next().unwrap_or(value).trim().to_owned())
}

fn has_cf_clearance_cookie(headers: &HeaderMap) -> bool {
    headers
        .get("cookie")
        .and_then(|value| value.to_str().ok())
        .map(|cookie| {
            cookie
                .split(';')
                .map(|entry| entry.trim())
                .any(|entry| entry.starts_with("cf_clearance="))
        })
        .unwrap_or(false)
}

async fn verify_cf_turnstile_token(token: Option<&str>, headers: &HeaderMap) -> AppResult<()> {
    let (enabled, _) = get_login_turnstile_policy();
    if !enabled {
        return Ok(());
    }

    let secret = match env_cf_turnstile_secret() {
        Some(secret) => secret,
        None => return Ok(()),
    };

    if !should_require_turnstile_token(
        enabled,
        env_cf_turnstile_enforce_token(),
        has_cf_clearance_cookie(headers),
    ) {
        return Ok(());
    }

    let token = match token {
        Some(token) if !token.trim().is_empty() => token.trim(),
        _ => {
            return Err(AppError::security_validation(
                "CF_TURNSTILE_TOKEN_MISSING",
                "cf_turnstile_token is required",
            ));
        }
    };

    let siteverify_url = std::env::var("CF_TURNSTILE_SITEVERIFY_URL")
        .unwrap_or_else(|_| CF_TURNSTILE_SITEVERIFY.to_owned());
    let mut payload = vec![("secret", secret), ("response", token.to_owned())];

    if let Some(ip) = extract_client_ip(headers) {
        payload.push(("remoteip", ip));
    }

    let response = reqwest::Client::new()
        .post(siteverify_url)
        .form(&payload)
        .timeout(Duration::from_secs(5))
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

fn get_login_turnstile_policy() -> (bool, Option<String>) {
    login_turnstile_policy(env_cf_turnstile_secret(), env_cf_turnstile_site_key())
}

fn login_turnstile_policy(
    secret: Option<String>,
    site_key: Option<String>,
) -> (bool, Option<String>) {
    let enabled = secret.is_some() && site_key.is_some();

    (enabled, site_key)
}

fn should_require_turnstile_token(
    enabled: bool,
    enforce_token: bool,
    has_cf_clearance: bool,
) -> bool {
    enabled && (enforce_token || !has_cf_clearance)
}

fn env_cf_turnstile_secret() -> Option<String> {
    let secret = match std::env::var("CF_TURNSTILE_SECRET") {
        Ok(secret) => secret,
        Err(_) => match std::env::var("CF_TURNSTILE_SECRET_KEY") {
            Ok(secret) => secret,
            Err(_) => return None,
        },
    };

    let secret = secret.trim();
    if secret.is_empty() {
        None
    } else {
        Some(secret.to_owned())
    }
}

fn env_cf_turnstile_site_key() -> Option<String> {
    let site_key = match std::env::var("CF_TURNSTILE_SITE_KEY") {
        Ok(value) => value,
        Err(_) => return None,
    };

    let site_key = site_key.trim();
    if site_key.is_empty() {
        None
    } else {
        Some(site_key.to_owned())
    }
}

fn env_cf_turnstile_enforce_token() -> bool {
    std::env::var("CF_TURNSTILE_ENFORCE_TOKEN")
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            normalized == "1" || normalized == "true" || normalized == "yes" || normalized == "on"
        })
        .unwrap_or(false)
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_auth_routes_tests.rs"]
mod tests;
