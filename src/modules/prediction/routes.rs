//! prediction 路由层。
//!
//! 统一承接预测模块的用户与管理员 HTTP 入口，保持业务逻辑留在 application 层。

use crate::state::AppState;
use axum::{
    Router,
    routing::{get, patch, post},
};

use super::application as app;

/// 用户端预测路由。
pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/prediction/config", get(app::get_user_config))
        .route("/prediction/markets", get(app::list_user_markets))
        .route("/prediction/markets/:id", get(app::get_user_market))
        .route("/prediction/quotes", post(app::create_quote))
        .route(
            "/prediction/orders",
            get(app::list_user_orders).post(app::create_order),
        )
}

/// 管理端预测路由。
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/prediction/settings",
            get(app::get_admin_settings).patch(app::save_admin_settings),
        )
        .route(
            "/prediction/asset-configs",
            get(app::list_admin_asset_configs).post(app::upsert_admin_asset_config),
        )
        .route(
            "/prediction/asset-configs/:asset_id",
            patch(app::update_admin_asset_config),
        )
        .route("/prediction/markets", get(app::list_admin_markets))
        .route(
            "/prediction/markets/:id",
            get(app::get_admin_market).patch(app::update_admin_market),
        )
        .route(
            "/prediction/markets/:id/settle",
            post(app::settle_admin_market),
        )
        .route("/prediction/orders", get(app::list_admin_orders))
        .route("/prediction/orders/:id", get(app::get_admin_order))
        .route("/prediction/sync", post(app::trigger_admin_sync))
        .route("/prediction/sync/logs", get(app::list_admin_sync_logs))
}
