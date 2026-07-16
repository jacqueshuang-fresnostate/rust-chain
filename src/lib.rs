pub mod architecture;
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
        .merge(modules::countries::routes())
        .merge(modules::platform::routes())
        .merge(modules::user::routes::routes())
        .merge(modules::wallet::routes::routes())
        .merge(modules::quick_recharge::user_routes())
        .merge(modules::quick_recharge::public_routes())
        .merge(modules::market::routes::routes())
        .merge(modules::spot::routes::routes())
        .merge(modules::new_coin::routes::user_routes())
        .merge(modules::convert::routes::user_routes())
        .merge(modules::seconds_contract::routes::user_routes())
        .merge(modules::margin::routes::user_routes())
        .merge(modules::earn::routes::user_routes())
        .merge(modules::loan::user_routes())
        .merge(modules::prediction::user_routes())
        .merge(modules::news::routes::routes())
        .merge(modules::events::routes::routes());

    let admin_api = Router::new()
        .merge(modules::auth::routes::admin_routes())
        .merge(modules::spot::routes::admin_routes())
        .merge(modules::admin::routes::routes())
        .merge(modules::quick_recharge::admin_routes())
        .merge(modules::seconds_contract::routes::admin_routes())
        .merge(modules::margin::routes::admin_routes())
        .merge(modules::earn::routes::admin_routes())
        .merge(modules::loan::admin_routes())
        .merge(modules::prediction::admin_routes());

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
#[path = "../tests/unit_src/src_lib_tests.rs"]
mod tests;
