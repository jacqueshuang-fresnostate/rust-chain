use crate::{
    error::{AppError, AppResult},
    modules::auth::{
        AdminCredentials, AdminRegistration, AgentCredentials, AuthService, IssuedTokens,
        MySqlAuthRepository, TokenScope, UserCredentials,
    },
    state::AppState,
};
use axum::{Json, Router, extract::State, routing::post};
use serde::{Deserialize, Serialize};

pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(user_register))
        .route("/auth/login", post(user_login))
        .route("/auth/refresh", post(user_refresh))
}

pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(admin_register))
        .route("/auth/login", post(admin_login))
        .route("/auth/refresh", post(admin_refresh))
}

pub fn agent_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(agent_register))
        .route("/auth/login", post(agent_login))
        .route("/auth/refresh", post(agent_refresh))
}

#[derive(Debug, Deserialize)]
struct UserAuthRequest {
    email: Option<String>,
    phone: Option<String>,
    password: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AdminAuthRequest {
    username: Option<String>,
    password: Option<String>,
    role_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct AgentAuthRequest {
    username: Option<String>,
    password: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RefreshRequest {
    refresh_token: Option<String>,
}

#[derive(Debug, Serialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    token_type: &'static str,
    scope: TokenScope,
}

async fn user_register(
    State(state): State<AppState>,
    Json(request): Json<UserAuthRequest>,
) -> AppResult<Json<TokenResponse>> {
    let tokens = auth_service(&state)?
        .register_user(UserCredentials {
            email: request.email,
            phone: request.phone,
            password: request.password,
        })
        .await?;

    Ok(Json(tokens.into()))
}

async fn user_login(
    State(state): State<AppState>,
    Json(request): Json<UserAuthRequest>,
) -> AppResult<Json<TokenResponse>> {
    let tokens = auth_service(&state)?
        .login_user(UserCredentials {
            email: request.email,
            phone: request.phone,
            password: request.password,
        })
        .await?;

    Ok(Json(tokens.into()))
}

async fn user_refresh(
    State(state): State<AppState>,
    Json(request): Json<RefreshRequest>,
) -> AppResult<Json<TokenResponse>> {
    refresh(&state, request.refresh_token, TokenScope::User).await
}

async fn admin_register(
    State(state): State<AppState>,
    Json(request): Json<AdminAuthRequest>,
) -> AppResult<Json<TokenResponse>> {
    let tokens = auth_service(&state)?
        .register_admin(AdminRegistration {
            username: request.username,
            password: request.password,
            role_id: request.role_id,
        })
        .await?;

    Ok(Json(tokens.into()))
}

async fn admin_login(
    State(state): State<AppState>,
    Json(request): Json<AdminAuthRequest>,
) -> AppResult<Json<TokenResponse>> {
    let tokens = auth_service(&state)?
        .login_admin(AdminCredentials {
            username: request.username,
            password: request.password,
        })
        .await?;

    Ok(Json(tokens.into()))
}

async fn admin_refresh(
    State(state): State<AppState>,
    Json(request): Json<RefreshRequest>,
) -> AppResult<Json<TokenResponse>> {
    refresh(&state, request.refresh_token, TokenScope::Admin).await
}

async fn agent_register(
    State(_state): State<AppState>,
    Json(_request): Json<AgentAuthRequest>,
) -> AppResult<Json<TokenResponse>> {
    Err(AppError::Forbidden)
}

async fn agent_login(
    State(state): State<AppState>,
    Json(request): Json<AgentAuthRequest>,
) -> AppResult<Json<TokenResponse>> {
    let tokens = auth_service(&state)?
        .login_agent(AgentCredentials {
            username: request.username,
            password: request.password,
        })
        .await?;

    Ok(Json(tokens.into()))
}

async fn agent_refresh(
    State(state): State<AppState>,
    Json(request): Json<RefreshRequest>,
) -> AppResult<Json<TokenResponse>> {
    refresh(&state, request.refresh_token, TokenScope::Agent).await
}

async fn refresh(
    state: &AppState,
    refresh_token: Option<String>,
    expected_scope: TokenScope,
) -> AppResult<Json<TokenResponse>> {
    let tokens = auth_service(state)?
        .refresh(refresh_token, expected_scope)
        .await?;

    Ok(Json(tokens.into()))
}

fn auth_service(state: &AppState) -> AppResult<AuthService<MySqlAuthRepository>> {
    let pool = state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for auth persistence".to_owned())
    })?;

    Ok(AuthService::new(
        MySqlAuthRepository::new(pool),
        state.settings.clone(),
    ))
}

impl From<IssuedTokens> for TokenResponse {
    fn from(tokens: IssuedTokens) -> Self {
        Self {
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            token_type: tokens.token_type,
            scope: tokens.scope,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use secrecy::SecretString;
    use serde_json::Value;
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

    async fn request_auth_route(
        app: Router,
        path: &str,
        body: &'static str,
    ) -> (StatusCode, Value) {
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();

        (status, payload)
    }

    async fn assert_auth_route_requires_mysql(app: Router, path: &str, body: &'static str) {
        let (status, payload) = request_auth_route(app, path, body).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{path}");
        assert_eq!(payload["code"], "INTERNAL_ERROR");
        assert!(
            payload["message"]
                .as_str()
                .unwrap()
                .contains("mysql pool is not configured for auth persistence")
        );
    }

    async fn assert_auth_route_forbidden(app: Router, path: &str, body: &'static str) {
        let (status, payload) = request_auth_route(app, path, body).await;

        assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
        assert_eq!(payload["code"], "FORBIDDEN");
    }

    #[tokio::test]
    async fn user_auth_routes_return_clear_error_without_mysql() {
        let app = user_routes().with_state(test_state());

        assert_auth_route_requires_mysql(
            app.clone(),
            "/auth/register",
            r#"{"email":"user@example.com","password":"password-1"}"#,
        )
        .await;
        assert_auth_route_requires_mysql(
            app.clone(),
            "/auth/login",
            r#"{"email":"user@example.com","password":"password-1"}"#,
        )
        .await;
        assert_auth_route_requires_mysql(
            app,
            "/auth/refresh",
            r#"{"refresh_token":"refresh-token-1"}"#,
        )
        .await;
    }

    #[tokio::test]
    async fn admin_auth_routes_return_clear_error_without_mysql() {
        let app = admin_routes().with_state(test_state());

        assert_auth_route_requires_mysql(
            app.clone(),
            "/auth/register",
            r#"{"username":"admin","password":"password-1"}"#,
        )
        .await;
        assert_auth_route_requires_mysql(
            app.clone(),
            "/auth/login",
            r#"{"username":"admin","password":"password-1"}"#,
        )
        .await;
        assert_auth_route_requires_mysql(
            app,
            "/auth/refresh",
            r#"{"refresh_token":"refresh-token-1"}"#,
        )
        .await;
    }

    #[tokio::test]
    async fn agent_auth_routes_return_clear_error_without_mysql() {
        let app = agent_routes().with_state(test_state());

        assert_auth_route_forbidden(
            app.clone(),
            "/auth/register",
            r#"{"username":"agent","password":"password-1"}"#,
        )
        .await;
        assert_auth_route_requires_mysql(
            app.clone(),
            "/auth/login",
            r#"{"username":"agent","password":"password-1"}"#,
        )
        .await;
        assert_auth_route_requires_mysql(
            app,
            "/auth/refresh",
            r#"{"refresh_token":"refresh-token-1"}"#,
        )
        .await;
    }
}
