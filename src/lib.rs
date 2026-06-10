pub mod config;
pub mod error;
pub mod infra;
pub mod modules;
pub mod openapi;
pub mod state;
pub mod time;
pub mod workers;

use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;
use state::AppState;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use utoipa::ToSchema;

pub fn build_router(state: AppState) -> Router {
    let user_api = Router::new()
        .merge(modules::auth::routes::user_routes())
        .merge(modules::user::routes::routes())
        .merge(modules::wallet::routes::routes())
        .merge(modules::market::routes::routes())
        .merge(modules::spot::routes::routes())
        .merge(modules::new_coin::routes::user_routes())
        .merge(modules::convert::routes::user_routes())
        .merge(modules::seconds_contract::routes::user_routes())
        .merge(modules::margin::routes::user_routes())
        .merge(modules::earn::routes::user_routes())
        .merge(modules::news::routes::routes())
        .merge(modules::events::routes::routes());

    let admin_api = Router::new()
        .merge(modules::auth::routes::admin_routes())
        .merge(modules::spot::routes::admin_routes())
        .merge(modules::admin::routes::routes())
        .merge(modules::seconds_contract::routes::admin_routes())
        .merge(modules::margin::routes::admin_routes())
        .merge(modules::earn::routes::admin_routes());

    let agent_api = Router::new()
        .merge(modules::auth::routes::agent_routes())
        .merge(modules::agent::routes::routes());

    Router::new()
        .route("/health", get(health))
        .merge(openapi::routes())
        .merge(modules::events::routes::routes())
        .nest("/api/v1", user_api)
        .nest("/admin/api/v1", admin_api)
        .nest("/agent/api/v1", agent_api)
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    status: &'static str,
}

pub async fn health(State(_state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use secrecy::SecretString;
    use tower::ServiceExt;

    fn test_state() -> AppState {
        AppState::new(crate::config::Settings {
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

    #[tokio::test]
    async fn health_route_returns_ok() {
        let app = build_router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn route_prefixes_are_registered() {
        let app = build_router(test_state());

        for path in [
            "/api/v1/auth/login",
            "/api/v1/user/profile",
            "/admin/api/v1/auth/login",
            "/agent/api/v1/auth/login",
            "/api/v1/wallet/accounts",
            "/api/v1/convert/pairs",
            "/api/v1/seconds-contracts/products",
            "/admin/api/v1/seconds-contracts/products",
            "/api/v1/margin/products",
            "/admin/api/v1/margin/products",
            "/api/v1/earn/products",
            "/admin/api/v1/earn/products",
            "/api/v1/news",
            "/api/v1/events/outbox/publish-once",
        ] {
            let response = app
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();

            assert_ne!(
                response.status(),
                StatusCode::NOT_FOUND,
                "{path} should be registered"
            );
        }
    }
}
