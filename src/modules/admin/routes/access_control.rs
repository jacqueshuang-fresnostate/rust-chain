//! 管理员当前权限快照路由。

use crate::{
    error::AppResult,
    modules::{
        admin::{
            application::{get_admin_access, list_admin_permission_catalog},
            presentation::{AdminAccessResponse, AdminPermissionCatalogResponse},
        },
        auth::AdminAuth,
    },
    state::AppState,
};
use axum::{Json, Router, extract::State, routing::get};

/// 注册当前管理员身份与权限查询端点，供前端完成导航与操作可见性初始化。
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/access/me", get(me))
        .route("/access/permissions", get(permission_catalog))
}

/// 返回可分配权限码全集；该路径本身需要 `governance.roles.read` 或全局通配权限。
async fn permission_catalog(
    AdminAuth(_): AdminAuth,
) -> AppResult<Json<AdminPermissionCatalogResponse>> {
    Ok(Json(list_admin_permission_catalog()))
}

/// 返回令牌主体当前的角色和权限；`AdminAuth` 已在进入处完成第一次数据库授权回查。
/// 此处再读一次是为了构造响应，两次之间如果角色被收紧，以后一次结果为准且不返回旧快照。
async fn me(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<AdminAccessResponse>> {
    let pool = super::mysql_pool(&state)?;
    Ok(Json(get_admin_access(&pool, &claims.sub).await?))
}
