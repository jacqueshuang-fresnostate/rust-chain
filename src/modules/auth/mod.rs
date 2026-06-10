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
use chrono::{Duration, NaiveDateTime, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use sqlx::{MySql, Pool};
use std::sync::Arc;
use uuid::Uuid;

pub mod routes;

const ACTIVE_STATUS: &str = "active";
const REFRESH_TOKEN_HASH_SALT: &[u8] = b"exchange-refresh-token-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenScope {
    User,
    Admin,
    Agent,
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

    fn from_storage(value: &str) -> AppResult<Self> {
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
    pub password: Option<String>,
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
}

#[async_trait]
pub trait AuthRepository: Clone + Send + Sync + 'static {
    async fn create_user(&self, actor: NewUserActor) -> AppResult<AuthActor>;
    async fn create_admin(&self, actor: NewAdminActor) -> AppResult<AuthActor>;
    async fn create_agent(&self, actor: NewAgentActor) -> AppResult<AuthActor>;
    async fn find_user_by_email(&self, email: &str) -> AppResult<Option<StoredActorCredential>>;
    async fn find_user_by_phone(&self, phone: &str) -> AppResult<Option<StoredActorCredential>>;
    async fn find_admin_by_username(
        &self,
        username: &str,
    ) -> AppResult<Option<StoredActorCredential>>;
    async fn find_agent_by_username(
        &self,
        username: &str,
    ) -> AppResult<Option<StoredActorCredential>>;
    async fn find_active_actor(&self, actor: &AuthActor) -> AppResult<Option<AuthActor>>;
    async fn record_login(&self, actor: &AuthActor) -> AppResult<()>;
    async fn store_refresh_token(&self, token: StoredRefreshToken) -> AppResult<()>;
    async fn find_refresh_token(
        &self,
        token_hash: &str,
        now: NaiveDateTime,
    ) -> AppResult<Option<RefreshTokenRecord>>;
}

#[derive(Clone)]
pub struct MySqlAuthRepository {
    pool: Pool<MySql>,
}

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
        let result =
            sqlx::query("INSERT INTO users (email, phone, password_hash) VALUES (?, ?, ?)")
                .bind(actor.email)
                .bind(actor.phone)
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
               WHERE agent_admin_users.username = ? AND agents.status = 'active'
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
            Ok(RefreshTokenRecord {
                actor_type: ActorType::from_storage(&actor_type)?,
                actor_id,
                user_id,
            })
        })
        .transpose()
    }
}

#[derive(Clone)]
pub struct AuthService<R> {
    repository: R,
    settings: Arc<Settings>,
}

impl<R: AuthRepository> AuthService<R> {
    pub fn new(repository: R, settings: Arc<Settings>) -> Self {
        Self {
            repository,
            settings,
        }
    }

    pub async fn register_user(&self, credentials: UserCredentials) -> AppResult<IssuedTokens> {
        let password = required_string(credentials.password, "password")?;
        let (email, phone) = user_identifier(credentials.email, credentials.phone)?;
        let actor = self
            .repository
            .create_user(NewUserActor {
                email,
                phone,
                password_hash: hash_password(&password)?,
            })
            .await?;

        self.issue_tokens(actor).await
    }

    pub async fn login_user(&self, credentials: UserCredentials) -> AppResult<IssuedTokens> {
        let password = required_string(credentials.password, "password")?;
        let (email, phone) = user_identifier(credentials.email, credentials.phone)?;
        let stored = match (email, phone) {
            (Some(email), _) => self.repository.find_user_by_email(&email).await?,
            (None, Some(phone)) => self.repository.find_user_by_phone(&phone).await?,
            (None, None) => {
                return Err(AppError::Validation(
                    "email or phone is required".to_owned(),
                ));
            }
        }
        .ok_or(AppError::Unauthorized)?;

        self.verify_and_issue(stored, &password).await
    }

    pub async fn register_admin(&self, registration: AdminRegistration) -> AppResult<IssuedTokens> {
        let username = required_string(registration.username, "username")?;
        let password = required_string(registration.password, "password")?;
        let role_id = registration
            .role_id
            .ok_or_else(|| AppError::Validation("role_id is required".to_owned()))?;
        let actor = self
            .repository
            .create_admin(NewAdminActor {
                username,
                password_hash: hash_password(&password)?,
                role_id,
            })
            .await?;

        self.issue_tokens(actor).await
    }

    pub async fn login_admin(&self, credentials: AdminCredentials) -> AppResult<IssuedTokens> {
        let username = required_string(credentials.username, "username")?;
        let password = required_string(credentials.password, "password")?;
        let stored = self
            .repository
            .find_admin_by_username(&username)
            .await?
            .ok_or(AppError::Unauthorized)?;

        self.verify_and_issue(stored, &password).await
    }

    pub async fn register_agent(&self, registration: AgentRegistration) -> AppResult<IssuedTokens> {
        let username = required_string(registration.username, "username")?;
        let password = required_string(registration.password, "password")?;
        let agent_id = registration
            .agent_id
            .ok_or_else(|| AppError::Validation("agent_id is required".to_owned()))?;
        let actor = self
            .repository
            .create_agent(NewAgentActor {
                username,
                password_hash: hash_password(&password)?,
                agent_id,
            })
            .await?;

        self.issue_tokens(actor).await
    }

    pub async fn login_agent(&self, credentials: AgentCredentials) -> AppResult<IssuedTokens> {
        let username = required_string(credentials.username, "username")?;
        let password = required_string(credentials.password, "password")?;
        let stored = self
            .repository
            .find_agent_by_username(&username)
            .await?
            .ok_or(AppError::Unauthorized)?;

        self.verify_and_issue(stored, &password).await
    }

    pub async fn refresh(
        &self,
        refresh_token: Option<String>,
        expected_scope: TokenScope,
    ) -> AppResult<IssuedTokens> {
        let refresh_token = required_string(refresh_token, "refresh_token")?;
        let claims = decode_claims(&self.settings, &refresh_token)?;
        if claims.scope != expected_scope {
            return Err(AppError::Unauthorized);
        }

        let token_hash = hash_refresh_token(&refresh_token)?;
        let stored = self
            .repository
            .find_refresh_token(&token_hash, Utc::now().naive_utc())
            .await?
            .ok_or(AppError::Unauthorized)?;
        let actor = AuthActor::new(stored.actor_type, stored.actor_id, stored.user_id);

        if stored.actor_type.scope() != claims.scope || actor.subject() != claims.sub {
            return Err(AppError::Unauthorized);
        }

        let actor = self
            .repository
            .find_active_actor(&actor)
            .await?
            .ok_or(AppError::Unauthorized)?;

        self.issue_tokens(actor).await
    }

    async fn verify_and_issue(
        &self,
        stored: StoredActorCredential,
        password: &str,
    ) -> AppResult<IssuedTokens> {
        if stored.status != ACTIVE_STATUS || !verify_password(&stored.password_hash, password)? {
            return Err(AppError::Unauthorized);
        }

        self.repository.record_login(&stored.actor).await?;
        self.issue_tokens(stored.actor).await
    }

    async fn issue_tokens(&self, actor: AuthActor) -> AppResult<IssuedTokens> {
        let scope = actor.actor_type.scope();
        let subject = actor.subject();
        let access_token = issue_token(
            &self.settings,
            subject.clone(),
            scope,
            self.settings.jwt_access_ttl_seconds,
        )?;
        let refresh_token = issue_token(
            &self.settings,
            subject,
            scope,
            self.settings.jwt_refresh_ttl_seconds,
        )?;
        let token_hash = hash_refresh_token(&refresh_token)?;
        let expires_at = Utc::now().naive_utc()
            + Duration::seconds(self.settings.jwt_refresh_ttl_seconds as i64);

        self.repository
            .store_refresh_token(StoredRefreshToken {
                actor_type: actor.actor_type,
                actor_id: actor.actor_id,
                user_id: actor.user_id,
                token_hash,
                expires_at,
            })
            .await?;

        Ok(IssuedTokens {
            access_token,
            refresh_token,
            token_type: "Bearer",
            scope,
        })
    }
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

fn hash_with_salt(secret: &str, salt: &SaltString) -> AppResult<String> {
    Argon2::default()
        .hash_password(secret.as_bytes(), salt)
        .map(|hash| hash.to_string())
        .map_err(|error| AppError::Internal(format!("failed to hash secret: {error}")))
}

fn user_identifier(
    email: Option<String>,
    phone: Option<String>,
) -> AppResult<(Option<String>, Option<String>)> {
    let email = optional_string(email);
    let phone = optional_string(phone);

    if email.is_none() && phone.is_none() {
        Err(AppError::Validation(
            "email or phone is required".to_owned(),
        ))
    } else {
        Ok((email, phone))
    }
}

fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn required_string(value: Option<String>, field: &str) -> AppResult<String> {
    optional_string(value).ok_or_else(|| AppError::Validation(format!("{field} is required")))
}

fn map_duplicate_key(error: sqlx::Error, actor: &str) -> AppError {
    if is_duplicate_key(&error) {
        AppError::Conflict(format!("{actor} already exists"))
    } else {
        AppError::Database(error)
    }
}

fn is_duplicate_key(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(database_error) if database_error.code().as_deref() == Some("1062"))
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

fn require_scope(parts: &Parts, state: &AppState, required_scope: TokenScope) -> AppResult<Claims> {
    let claims = decode_claims(&state.settings, bearer_token(parts)?)?;

    if claims.scope == required_scope {
        Ok(claims)
    } else {
        Err(AppError::Forbidden)
    }
}

#[async_trait]
impl FromRequestParts<AppState> for UserAuth {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        require_scope(parts, state, TokenScope::User).map(Self)
    }
}

#[async_trait]
impl FromRequestParts<AppState> for AdminAuth {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        require_scope(parts, state, TokenScope::Admin).map(Self)
    }
}

#[async_trait]
impl FromRequestParts<AppState> for AgentAuth {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        require_scope(parts, state, TokenScope::Agent).map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Settings, state::AppState};
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode, header::AUTHORIZATION},
        routing::get,
    };
    use secrecy::SecretString;
    use tower::ServiceExt;

    fn test_state() -> AppState {
        AppState::new(Settings {
            app_env: "test".to_owned(),
            app_host: "127.0.0.1".parse().unwrap(),
            app_port: 0,
            database_url: SecretString::new("mysql://test:test@localhost/test".to_owned()),
            mongodb_uri: SecretString::new("mongodb://localhost:27017".to_owned()),
            mongodb_database: "exchange_test".to_owned(),
            redis_url: SecretString::new("redis://localhost:6379".to_owned()),
            rabbitmq_url: SecretString::new("amqp://guest:guest@localhost:5672/%2f".to_owned()),
            jwt_secret: SecretString::new("test-secret".to_owned()),
            credential_encryption_key: Some(SecretString::new(
                "0123456789abcdef0123456789abcdef".to_owned(),
            )),
            jwt_access_ttl_seconds: 900,
            jwt_refresh_ttl_seconds: 2_592_000,
            bitget_rest_base_url: "https://bitget.test".to_owned(),
            bitget_ws_url: "wss://bitget.test/ws".to_owned(),
            htx_rest_base_url: "https://htx.test".to_owned(),
            htx_ws_url: "wss://htx.test/ws".to_owned(),
            market_feed_symbols: Vec::new(),
            market_feed_intervals: Vec::new(),
            market_feed_providers: Vec::new(),
            market_feed_reconnect_seconds: 5,
            market_feed_rest_fallback_timeout_seconds: 3,
            event_inbox_retry_scan_seconds: 10,
            event_outbox_publisher_enabled: true,
            event_outbox_publisher_interval_seconds: 5,
            unlock_scanner_enabled: true,
            unlock_scanner_interval_seconds: 10,
            unlock_scanner_batch_limit: 100,
            kline_recovery_enabled: true,
            kline_recovery_interval_seconds: 30,
            kline_recovery_batch_limit: 100,
            seconds_contract_settlement_enabled: true,
            seconds_contract_settlement_interval_seconds: 5,
            seconds_contract_settlement_batch_limit: 100,
            earn_auto_redemption_enabled: true,
            earn_auto_redemption_interval_seconds: 60,
            earn_auto_redemption_batch_limit: 100,
            margin_liquidation_enabled: true,
            margin_liquidation_interval_seconds: 5,
            margin_liquidation_batch_limit: 100,
            margin_interest_enabled: true,
            margin_interest_interval_seconds: 60,
            margin_interest_batch_limit: 100,
        })
    }

    fn scoped_app(state: AppState) -> Router {
        Router::new()
            .route("/user", get(|_auth: UserAuth| async { "user" }))
            .route("/admin", get(|_auth: AdminAuth| async { "admin" }))
            .route("/agent", get(|_auth: AgentAuth| async { "agent" }))
            .with_state(state)
    }

    #[test]
    fn password_hashes_verify_without_storing_plaintext() {
        let password_hash = hash_password("correct horse battery staple").unwrap();

        assert_ne!(password_hash, "correct horse battery staple");
        assert!(verify_password(&password_hash, "correct horse battery staple").unwrap());
        assert!(!verify_password(&password_hash, "wrong password").unwrap());
    }

    #[test]
    fn actor_type_maps_to_token_scope_and_storage_value() {
        assert_eq!(ActorType::User.scope(), TokenScope::User);
        assert_eq!(ActorType::Admin.scope(), TokenScope::Admin);
        assert_eq!(ActorType::Agent.scope(), TokenScope::Agent);
        assert_eq!(ActorType::User.as_str(), "user");
        assert_eq!(ActorType::Admin.as_str(), "admin");
        assert_eq!(ActorType::Agent.as_str(), "agent");
    }

    #[test]
    fn refresh_token_hash_is_deterministic_and_not_plaintext() {
        let first = hash_refresh_token("refresh-token-1").unwrap();
        let second = hash_refresh_token("refresh-token-1").unwrap();
        let different = hash_refresh_token("refresh-token-2").unwrap();

        assert_eq!(first, second);
        assert_ne!(first, different);
        assert_ne!(first, "refresh-token-1");
        assert!(first.starts_with("$argon2id$"));
    }

    async fn status_for(app: Router, path: &str, token: &str) -> StatusCode {
        app.oneshot(
            Request::builder()
                .uri(path)
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
    }

    #[tokio::test]
    async fn user_token_cannot_access_admin_or_agent_extractors() {
        let state = test_state();
        let token = issue_token(
            &state.settings,
            "user-1",
            TokenScope::User,
            state.settings.jwt_access_ttl_seconds,
        )
        .unwrap();
        let app = scoped_app(state);

        assert_eq!(
            status_for(app.clone(), "/user", &token).await,
            StatusCode::OK
        );
        assert_eq!(
            status_for(app.clone(), "/admin", &token).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            status_for(app, "/agent", &token).await,
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn admin_token_cannot_satisfy_user_scope() {
        let state = test_state();
        let token = issue_token(
            &state.settings,
            "admin-1",
            TokenScope::Admin,
            state.settings.jwt_access_ttl_seconds,
        )
        .unwrap();
        let app = scoped_app(state);

        assert_eq!(
            status_for(app.clone(), "/admin", &token).await,
            StatusCode::OK
        );
        assert_eq!(
            status_for(app, "/user", &token).await,
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn agent_token_only_satisfies_agent_scope() {
        let state = test_state();
        let token = issue_token(
            &state.settings,
            "agent-1",
            TokenScope::Agent,
            state.settings.jwt_access_ttl_seconds,
        )
        .unwrap();
        let app = scoped_app(state);

        assert_eq!(
            status_for(app.clone(), "/agent", &token).await,
            StatusCode::OK
        );
        assert_eq!(
            status_for(app.clone(), "/user", &token).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            status_for(app, "/admin", &token).await,
            StatusCode::FORBIDDEN
        );
    }
}
