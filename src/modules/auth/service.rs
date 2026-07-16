//! auth bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。

use crate::{
    architecture::ServiceLayer,
    config::Settings,
    error::{AppError, AppResult},
    modules::{
        auth::{
            ACTIVE_STATUS, ActorType, AdminCredentials, AdminRegistration, AgentCredentials,
            AgentRegistration, AuthActor, IssuedTokens, NewAdminActor, NewAgentActor, NewUserActor,
            RefreshTokenRecord, StoredActorCredential, StoredRefreshToken, TokenScope,
            UserCredentials, decode_claims, hash_password, hash_refresh_token, issue_token,
            map_sa_token_error, normalize_username, repository::AuthRepository, verify_password,
        },
        countries::normalize_country_code,
    },
};
use chrono::{Duration, Utc};
use redis::AsyncCommands;
use sa_token_core::SaTokenManager;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

const REDIS_REFRESH_PREFIX: &str = "exchange:auth:refresh:";
const REDIS_REFRESH_ACTOR_PREFIX: &str = "exchange:auth:refresh_actor:";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RedisRefreshTokenRecord {
    actor_type: String,
    actor_id: u64,
    user_id: Option<u64>,
    scope: TokenScope,
    expires_at: i64,
}

#[derive(Clone)]
pub struct AuthService<R> {
    repository: R,
    settings: Arc<Settings>,
    auth_manager: Option<Arc<SaTokenManager>>,
    redis: Option<redis::aio::ConnectionManager>,
}

impl<R> ServiceLayer for AuthService<R> {}

impl<R: AuthRepository> AuthService<R> {
    pub fn new(
        repository: R,
        settings: Arc<Settings>,
        auth_manager: Option<Arc<SaTokenManager>>,
        redis: Option<redis::aio::ConnectionManager>,
    ) -> Self {
        Self {
            repository,
            settings,
            auth_manager,
            redis,
        }
    }

    pub async fn register_user(&self, credentials: UserCredentials) -> AppResult<IssuedTokens> {
        let password = required_string(credentials.password, "password")?;
        let country_code = required_string(credentials.country_code, "country_code")?;
        let country_code = normalize_country_code(&country_code)?;
        let country = self
            .repository
            .find_registration_country(&country_code)
            .await?
            .ok_or_else(|| {
                AppError::Validation("country_code is not available for registration".to_owned())
            })?;
        let (email, phone) = user_identifier(credentials.email, credentials.phone)?;
        let actor = self
            .repository
            .create_user(NewUserActor {
                email,
                phone,
                password_hash: hash_password(&password)?,
                country_code: country.country_code,
                preferred_locale: country.default_locale,
            })
            .await?;

        self.issue_tokens(actor).await
    }

    pub async fn login_user(&self, credentials: UserCredentials) -> AppResult<IssuedTokens> {
        let actor = self.verify_user_credentials(credentials).await?;
        self.issue_tokens(actor).await
    }

    pub async fn verify_user_credentials(
        &self,
        credentials: UserCredentials,
    ) -> AppResult<AuthActor> {
        let password = required_string(credentials.password, "password")?;
        let identifier = user_login_identifier(
            credentials.email,
            credentials.phone,
            credentials.username,
            credentials.username_login_enabled,
        )?;
        let stored = match identifier {
            UserLoginIdentifier::Email(email) => self.repository.find_user_by_email(&email).await?,
            UserLoginIdentifier::Phone(phone) => self.repository.find_user_by_phone(&phone).await?,
            UserLoginIdentifier::Username(username) => {
                self.repository.find_user_by_username(&username).await?
            }
        }
        .ok_or(AppError::Unauthorized)?;

        self.verify_actor_credentials(stored, &password).await
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
        if self.auth_manager.is_some() {
            return self.refresh_sa_token(&refresh_token, expected_scope).await;
        }

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

    async fn refresh_sa_token(
        &self,
        refresh_token: &str,
        expected_scope: TokenScope,
    ) -> AppResult<IssuedTokens> {
        let stored = self
            .find_project_refresh_token(refresh_token)
            .await?
            .ok_or(AppError::Unauthorized)?;

        if stored.scope != expected_scope || stored.actor_type.scope() != expected_scope {
            return Err(AppError::Unauthorized);
        }

        let actor = AuthActor::new(stored.actor_type, stored.actor_id, stored.user_id);
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
        let actor = self.verify_actor_credentials(stored, password).await?;
        self.issue_tokens(actor).await
    }

    async fn verify_actor_credentials(
        &self,
        stored: StoredActorCredential,
        password: &str,
    ) -> AppResult<AuthActor> {
        if stored.status != ACTIVE_STATUS || !verify_password(&stored.password_hash, password)? {
            return Err(AppError::Unauthorized);
        }

        self.repository.record_login(&stored.actor).await?;
        Ok(stored.actor)
    }

    pub async fn issue_tokens_for_actor(&self, actor: AuthActor) -> AppResult<IssuedTokens> {
        self.issue_tokens(actor).await
    }

    async fn issue_tokens(&self, actor: AuthActor) -> AppResult<IssuedTokens> {
        if let Some(manager) = &self.auth_manager {
            return self.issue_sa_tokens(manager, actor).await;
        }

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

    async fn issue_sa_tokens(
        &self,
        manager: &SaTokenManager,
        actor: AuthActor,
    ) -> AppResult<IssuedTokens> {
        let scope = actor.actor_type.scope();
        let access_token = manager
            .login_with_options(
                actor.actor_id.to_string(),
                Some(scope.as_login_type().to_owned()),
                Some("api".to_owned()),
                Some(json!({
                    "actor_type": actor.actor_type.as_str(),
                    "actor_id": actor.actor_id,
                    "user_id": actor.user_id,
                })),
                None,
                None,
            )
            .await
            .map_err(map_sa_token_error)?;
        let refresh_token = generate_refresh_token();
        let expires_at = Utc::now().timestamp() + self.settings.jwt_refresh_ttl_seconds as i64;
        let record = RedisRefreshTokenRecord {
            actor_type: actor.actor_type.as_str().to_owned(),
            actor_id: actor.actor_id,
            user_id: actor.user_id,
            scope,
            expires_at,
        };

        if let Some(redis) = &self.redis {
            store_project_refresh_token(
                redis.clone(),
                &refresh_token,
                &record,
                self.settings.jwt_refresh_ttl_seconds,
            )
            .await?;
        } else {
            self.repository
                .store_refresh_token(StoredRefreshToken {
                    actor_type: actor.actor_type,
                    actor_id: actor.actor_id,
                    user_id: actor.user_id,
                    token_hash: hash_refresh_token(&refresh_token)?,
                    expires_at: Utc::now().naive_utc()
                        + Duration::seconds(self.settings.jwt_refresh_ttl_seconds as i64),
                })
                .await?;
        }

        Ok(IssuedTokens {
            access_token: access_token.to_string(),
            refresh_token,
            token_type: "Bearer",
            scope,
        })
    }

    async fn find_project_refresh_token(
        &self,
        refresh_token: &str,
    ) -> AppResult<Option<RefreshTokenRecord>> {
        if let Some(redis) = &self.redis {
            return load_project_refresh_token(redis.clone(), refresh_token).await;
        }

        self.repository
            .find_refresh_token(&hash_refresh_token(refresh_token)?, Utc::now().naive_utc())
            .await
    }
}

pub(crate) async fn revoke_project_refresh_tokens(
    mut redis: redis::aio::ConnectionManager,
    actor: &AuthActor,
) -> AppResult<()> {
    let actor_key = refresh_actor_key(actor.actor_type, actor.actor_id);
    let keys = redis.smembers::<_, Vec<String>>(&actor_key).await?;
    if !keys.is_empty() {
        redis.del::<_, ()>(&keys).await?;
    }
    redis.del::<_, ()>(actor_key).await?;

    Ok(())
}

fn generate_refresh_token() -> String {
    format!("refresh_{}", Uuid::now_v7().simple())
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

async fn store_project_refresh_token(
    mut redis: redis::aio::ConnectionManager,
    refresh_token: &str,
    record: &RedisRefreshTokenRecord,
    ttl_seconds: u64,
) -> AppResult<()> {
    let key = refresh_token_key(refresh_token);
    let actor_type = ActorType::from_storage(&record.actor_type)?;
    let actor_key = refresh_actor_key(actor_type, record.actor_id);
    let value = serde_json::to_string(record)
        .map_err(|error| AppError::Internal(format!("failed to encode refresh token: {error}")))?;
    let ttl = ttl_seconds.max(1);

    redis.set_ex::<_, _, ()>(&key, value, ttl).await?;
    redis.sadd::<_, _, ()>(&actor_key, &key).await?;
    redis.expire::<_, ()>(&actor_key, ttl as i64).await?;

    Ok(())
}

async fn load_project_refresh_token(
    mut redis: redis::aio::ConnectionManager,
    refresh_token: &str,
) -> AppResult<Option<RefreshTokenRecord>> {
    let key = refresh_token_key(refresh_token);
    let Some(value) = redis.get::<_, Option<String>>(key).await? else {
        return Ok(None);
    };
    let record: RedisRefreshTokenRecord =
        serde_json::from_str(&value).map_err(|_| AppError::Unauthorized)?;
    if record.expires_at <= Utc::now().timestamp() {
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

enum UserLoginIdentifier {
    Email(String),
    Phone(String),
    Username(String),
}

fn user_login_identifier(
    email: Option<String>,
    phone: Option<String>,
    username: Option<String>,
    username_login_enabled: bool,
) -> AppResult<UserLoginIdentifier> {
    if let Some(email) = optional_string(email) {
        return Ok(UserLoginIdentifier::Email(email));
    }
    if let Some(phone) = optional_string(phone) {
        return Ok(UserLoginIdentifier::Phone(phone));
    }
    if let Some(username) = optional_string(username) {
        if !username_login_enabled {
            return Err(AppError::Validation(
                "username login is disabled".to_owned(),
            ));
        }
        return Ok(UserLoginIdentifier::Username(normalize_username(
            &username,
        )?));
    }

    Err(AppError::Validation(
        "email, phone or username is required".to_owned(),
    ))
}

fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn required_string(value: Option<String>, field: &str) -> AppResult<String> {
    optional_string(value).ok_or_else(|| AppError::Validation(format!("{field} is required")))
}
