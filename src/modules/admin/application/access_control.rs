//! 管理员请求授权用例。

use crate::{
    error::{AppError, AppResult},
    modules::admin::{
        domain::AdminScope,
        infrastructure::load_admin_access_record,
        presentation::{AdminAccessResponse, AdminPermissionCatalogResponse},
        service::{
            admin_id_from_subject, admin_permission_catalog, admin_scope_from_record,
            required_admin_permission,
        },
    },
};
use sqlx::{MySql, Pool};

/// 回查管理员当前角色并强制校验路由权限。
/// 调用方应先完成 Bearer 令牌和 admin scope 验证；本用例再把 subject 解析为数字 ID，
/// 通过 MySQL 快照即时反映账号停用和权限收紧。未映射业务路由需要 `*`，不会默认放行。
pub(crate) async fn authorize_admin_request(
    pool: &Pool<MySql>,
    subject: &str,
    method: &str,
    path: &str,
) -> AppResult<AdminScope> {
    let scope = load_admin_scope(pool, subject).await?;
    if let Some(permission) = required_admin_permission(method, path)
        && !scope.allows(&permission)
    {
        return Err(AppError::Forbidden);
    }
    Ok(scope)
}

/// 返回后端授权器实际采用的权限码目录，供角色配置页展示和校验候选值。
pub(crate) fn list_admin_permission_catalog() -> AdminPermissionCatalogResponse {
    AdminPermissionCatalogResponse {
        permissions: admin_permission_catalog(),
    }
}

/// 强制校验一个显式权限码，供不经过 `AdminAuth` 提取器的兼容入口复用。
/// 该用例与路由授权使用同一份数据库快照与通配符语义，避免注册等特殊路径绕过 RBAC。
pub(crate) async fn authorize_admin_permission(
    pool: &Pool<MySql>,
    subject: &str,
    permission: &str,
) -> AppResult<AdminScope> {
    let scope = load_admin_scope(pool, subject).await?;
    if scope.allows(permission) {
        Ok(scope)
    } else {
        Err(AppError::Forbidden)
    }
}

/// 读取当前管理员的身份与权限响应，供后台导航和页内操作做可见性管理。
/// 返回值是即时数据库快照而不是 JWT 声明；前端隐藏仅改善体验，后续业务请求仍会再经后端授权。
pub(crate) async fn get_admin_access(
    pool: &Pool<MySql>,
    subject: &str,
) -> AppResult<AdminAccessResponse> {
    Ok(load_admin_scope(pool, subject).await?.into())
}

async fn load_admin_scope(pool: &Pool<MySql>, subject: &str) -> AppResult<AdminScope> {
    let admin_id = admin_id_from_subject(subject)?;
    admin_scope_from_record(load_admin_access_record(pool, admin_id).await?)
}
