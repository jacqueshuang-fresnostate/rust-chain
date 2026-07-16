use super::*;
use crate::{config::Settings, state::AppState};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header::AUTHORIZATION},
    routing::get,
};
use secrecy::SecretString;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
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
        coinbase_rest_base_url: "https://coinbase.test".to_owned(),
        coinbase_ws_url: "wss://coinbase.test/ws".to_owned(),
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

#[derive(Clone, Default)]
struct TestAuthRepository {
    refresh_tokens: Arc<Mutex<HashMap<String, StoredRefreshToken>>>,
    users_by_username: Arc<Mutex<HashMap<String, StoredActorCredential>>>,
}

#[async_trait]
impl AuthRepository for TestAuthRepository {
    async fn create_user(&self, _actor: NewUserActor) -> AppResult<AuthActor> {
        Err(AppError::Internal("not used".to_owned()))
    }

    async fn create_admin(&self, _actor: NewAdminActor) -> AppResult<AuthActor> {
        Err(AppError::Internal("not used".to_owned()))
    }

    async fn create_agent(&self, _actor: NewAgentActor) -> AppResult<AuthActor> {
        Err(AppError::Internal("not used".to_owned()))
    }

    async fn find_registration_country(
        &self,
        _country_code: &str,
    ) -> AppResult<Option<ActiveCountryConfig>> {
        Ok(None)
    }

    async fn find_user_by_email(&self, _email: &str) -> AppResult<Option<StoredActorCredential>> {
        Ok(None)
    }

    async fn find_user_by_phone(&self, _phone: &str) -> AppResult<Option<StoredActorCredential>> {
        Ok(None)
    }

    async fn find_user_by_username(
        &self,
        username: &str,
    ) -> AppResult<Option<StoredActorCredential>> {
        Ok(self
            .users_by_username
            .lock()
            .unwrap()
            .get(username)
            .cloned())
    }

    async fn find_admin_by_username(
        &self,
        _username: &str,
    ) -> AppResult<Option<StoredActorCredential>> {
        Ok(None)
    }

    async fn find_agent_by_username(
        &self,
        _username: &str,
    ) -> AppResult<Option<StoredActorCredential>> {
        Ok(None)
    }

    async fn find_active_actor(&self, actor: &AuthActor) -> AppResult<Option<AuthActor>> {
        Ok(Some(actor.clone()))
    }

    async fn record_login(&self, _actor: &AuthActor) -> AppResult<()> {
        Ok(())
    }

    async fn store_refresh_token(&self, token: StoredRefreshToken) -> AppResult<()> {
        self.refresh_tokens
            .lock()
            .unwrap()
            .insert(token.token_hash.clone(), token);
        Ok(())
    }

    async fn find_refresh_token(
        &self,
        token_hash: &str,
        now: NaiveDateTime,
    ) -> AppResult<Option<RefreshTokenRecord>> {
        Ok(self
            .refresh_tokens
            .lock()
            .unwrap()
            .get(token_hash)
            .filter(|token| token.expires_at > now)
            .map(|token| RefreshTokenRecord {
                actor_type: token.actor_type,
                actor_id: token.actor_id,
                user_id: token.user_id,
                scope: token.actor_type.scope(),
            }))
    }
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

#[test]
fn username_normalization_rejects_ambiguous_values() {
    assert_eq!(normalize_username(" Moon_1024 ").unwrap(), "moon_1024");
    assert!(normalize_username("ab").is_err());
    assert!(normalize_username("moon-light").is_err());
    assert!(normalize_username("用户名").is_err());
}

#[tokio::test]
async fn username_login_requires_policy_toggle() {
    let repository = TestAuthRepository::default();
    repository.users_by_username.lock().unwrap().insert(
        "moon_1024".to_owned(),
        StoredActorCredential {
            actor: AuthActor::new(ActorType::User, 42, Some(42)),
            password_hash: hash_password("CorrectPassword123!").unwrap(),
            status: ACTIVE_STATUS.to_owned(),
        },
    );
    let state = test_state();
    let service = AuthService::new(repository, state.settings.clone(), None, None);

    let disabled = service
        .verify_user_credentials(UserCredentials {
            email: None,
            phone: None,
            username: Some("Moon_1024".to_owned()),
            password: Some("CorrectPassword123!".to_owned()),
            country_code: None,
            username_login_enabled: false,
        })
        .await;
    assert!(matches!(disabled, Err(AppError::Validation(_))));

    let actor = service
        .verify_user_credentials(UserCredentials {
            email: None,
            phone: None,
            username: Some("Moon_1024".to_owned()),
            password: Some("CorrectPassword123!".to_owned()),
            country_code: None,
            username_login_enabled: true,
        })
        .await
        .unwrap();
    assert_eq!(actor, AuthActor::new(ActorType::User, 42, Some(42)));
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
async fn sa_token_user_session_satisfies_existing_extractors() {
    let mut state = test_state();
    let auth_manager = crate::infra::auth::memory_manager(&state.settings);
    let token = auth_manager
        .login_with_options(
            "42",
            Some("user".to_owned()),
            Some("api".to_owned()),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    state = state.with_auth_manager(auth_manager);
    let app = scoped_app(state);

    assert_eq!(
        status_for(app.clone(), "/user", token.as_str()).await,
        StatusCode::OK
    );
    assert_eq!(
        status_for(app, "/admin", token.as_str()).await,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn sa_token_refresh_preserves_scope_and_legacy_subject_shape() {
    let state = test_state();
    let auth_manager = crate::infra::auth::memory_manager(&state.settings);
    let state = state.with_auth_manager(auth_manager);
    let repository = TestAuthRepository::default();
    let service = AuthService::new(
        repository,
        state.settings.clone(),
        state.auth_manager.clone(),
        None,
    );
    let tokens = service
        .issue_tokens_for_actor(AuthActor::new(ActorType::User, 42, Some(42)))
        .await
        .unwrap();
    let claims = claims_from_bearer_token(&state, &tokens.access_token, TokenScope::User)
        .await
        .unwrap();

    assert_eq!(claims.sub, "user:42");
    assert_eq!(claims.scope, TokenScope::User);

    let refreshed = service
        .refresh(Some(tokens.refresh_token.clone()), TokenScope::User)
        .await
        .unwrap();
    let refreshed_claims =
        claims_from_bearer_token(&state, &refreshed.access_token, TokenScope::User)
            .await
            .unwrap();

    assert_eq!(refreshed_claims.sub, "user:42");
    assert!(
        service
            .refresh(Some(tokens.refresh_token), TokenScope::Admin)
            .await
            .is_err()
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
