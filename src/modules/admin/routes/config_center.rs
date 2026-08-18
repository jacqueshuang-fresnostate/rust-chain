//! 后台配置中心聚合查询路由。

use crate::{
    error::AppResult,
    modules::{
        admin::{
            application::list_admin_config_center,
            presentation::{AdminConfigCenterQuery, AdminConfigCenterResponse},
        },
        auth::AdminAuth,
    },
    state::AppState,
};
use axum::{Json, Router, extract::Query, extract::State, routing::get};

/// 注册配置中心唯一的只读聚合入口；具体权限由 `AdminAuth` 按 `config_center.read` 实时回查。
pub(super) fn routes() -> Router<AppState> {
    Router::new().route("/config-center", get(list_config_center))
}

/// 返回后端权威配置状态，不读取凭据明文，也不会因列表查询触发测试、重载或发布副作用。
async fn list_config_center(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminConfigCenterQuery>,
) -> AppResult<Json<AdminConfigCenterResponse>> {
    Ok(Json(
        list_admin_config_center(state.mysql.clone(), query).await?,
    ))
}
