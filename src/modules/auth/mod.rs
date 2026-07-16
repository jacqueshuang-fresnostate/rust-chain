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
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};
use chrono::{NaiveDateTime, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use sa_token_core::{SaTokenError, SaTokenManager, TokenInfo, TokenValue};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod routes;

pub use infrastructure::MySqlAuthRepository;
pub use repository::AuthRepository;
pub use service::AuthService;
use service::revoke_project_refresh_tokens;

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
    pub fn as_login_type(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Admin => "admin",
            Self::Agent => "agent",
        }
    }

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
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Admin => "admin",
            Self::Agent => "agent",
        }
    }

    pub fn scope(self) -> TokenScope {
        match self {
            Self::User => TokenScope::User,
            Self::Admin => TokenScope::Admin,
            Self::Agent => TokenScope::Agent,
        }
    }

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthActor {
    pub actor_type: ActorType,
    pub actor_id: u64,
    pub user_id: Option<u64>,
}

impl AuthActor {
    pub fn new(actor_type: ActorType, actor_id: u64, user_id: Option<u64>) -> Self {
        Self {
            actor_type,
            actor_id,
            user_id,
        }
    }

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
    pub token_hash: String,
    pub expires_at: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct RefreshTokenRecord {
    pub actor_type: ActorType,
    pub actor_id: u64,
    pub user_id: Option<u64>,
    pub scope: TokenScope,
}

pub fn hash_password(password: &str) -> AppResult<String> {
    let salt_seed = Uuid::now_v7();
    let salt = SaltString::encode_b64(salt_seed.as_bytes())
        .map_err(|error| AppError::Internal(format!("failed to create password salt: {error}")))?;

    hash_with_salt(password, &salt)
}

pub fn verify_password(password_hash: &str, password: &str) -> AppResult<bool> {
    let parsed_hash = match PasswordHash::new(password_hash) {
        Ok(parsed_hash) => parsed_hash,
        Err(_) => return Ok(false),
    };

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

pub fn hash_refresh_token(refresh_token: &str) -> AppResult<String> {
    let salt = SaltString::encode_b64(REFRESH_TOKEN_HASH_SALT).map_err(|error| {
        AppError::Internal(format!("failed to create refresh token salt: {error}"))
    })?;

    hash_with_salt(refresh_token, &salt)
}

pub async fn revoke_actor_auth_sessions(state: &AppState, actor: &AuthActor) -> AppResult<()> {
    if let Some(manager) = &state.auth_manager {
        let tokens = manager
            .get_token_value_list_by_login_id(
                actor.actor_type.as_str(),
                &actor.actor_id.to_string(),
                None,
            )
            .await
            .unwrap_or_default();
        for token in tokens {
            manager
                .logout(&TokenValue::new(token))
                .await
                .map_err(map_sa_token_error)?;
        }
    }

    if let Some(redis) = &state.redis {
        revoke_project_refresh_tokens(redis.clone(), actor).await?;
    }

    Ok(())
}

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

fn hash_with_salt(secret: &str, salt: &SaltString) -> AppResult<String> {
    Argon2::default()
        .hash_password(secret.as_bytes(), salt)
        .map(|hash| hash.to_string())
        .map_err(|error| AppError::Internal(format!("failed to hash secret: {error}")))
}

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

pub fn issue_token(
    settings: &Settings,
    subject: impl Into<String>,
    scope: TokenScope,
    ttl_seconds: u64,
) -> AppResult<String> {
    let claims = Claims {
        sub: subject.into(),
        scope,
        exp: (Utc::now().timestamp() + ttl_seconds as i64) as usize,
        token_id: Uuid::now_v7().to_string(),
    };

    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(settings.jwt_secret.expose_secret().as_bytes()),
    )
    .map_err(|error| AppError::Internal(format!("failed to issue jwt: {error}")))
}

pub fn decode_claims(settings: &Settings, token: &str) -> AppResult<Claims> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(settings.jwt_secret.expose_secret().as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map(|data| data.claims)
    .map_err(|_| AppError::Unauthorized)
}

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

fn claims_from_token_info(token_info: TokenInfo) -> AppResult<Claims> {
    let scope = TokenScope::from_login_type(&token_info.login_type)?;
    let exp = token_info
        .expire_time
        .map(|time| time.timestamp().max(0) as usize)
        .unwrap_or(0);

    Ok(Claims {
        sub: format!("{}:{}", scope.as_login_type(), token_info.login_id),
        scope,
        exp,
        token_id: token_info.token.to_string(),
    })
}

async fn claims_from_sa_token(manager: &SaTokenManager, token: &str) -> AppResult<Claims> {
    let token_info = manager
        .get_token_info(&TokenValue::new(token.to_owned()))
        .await
        .map_err(map_sa_token_error)?;

    claims_from_token_info(token_info)
}

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

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        require_scope(parts, state, TokenScope::Admin)
            .await
            .map(Self)
    }
}

#[async_trait]
impl FromRequestParts<AppState> for AgentAuth {
    type Rejection = AppError;

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
