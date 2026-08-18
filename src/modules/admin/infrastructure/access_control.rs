//! 管理员角色与权限快照的 MySQL 读取适配器。

use crate::{
    error::{AppError, AppResult},
    modules::admin::repository::AdminAccessRecord,
};
use serde_json::Value;
use sqlx::{MySql, Pool, Row, types::Json as SqlxJson};

/// 按管理员 ID 联表读取账号状态、角色与权限 JSON。
/// 查询不加锁且每次请求重新执行，使停用账号或收紧权限无需等待 JWT 到期；
/// 管理员或角色缺失统一返回未授权，非 active 账号也不暴露实际状态。
pub(crate) async fn load_admin_access_record(
    pool: &Pool<MySql>,
    admin_id: u64,
) -> AppResult<AdminAccessRecord> {
    let row = sqlx::query(
        r#"SELECT admins.id AS admin_id,
                  admins.username,
                  admins.status,
                  roles.id AS role_id,
                  roles.name AS role_name,
                  roles.permissions
           FROM admin_users AS admins
           INNER JOIN admin_roles AS roles ON roles.id = admins.role_id
           WHERE admins.id = ?
           LIMIT 1"#,
    )
    .bind(admin_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    let record = AdminAccessRecord {
        admin_id: row.try_get("admin_id")?,
        username: row.try_get("username")?,
        status: row.try_get("status")?,
        role_id: row.try_get("role_id")?,
        role_name: row.try_get("role_name")?,
        permissions: row.try_get::<SqlxJson<Value>, _>("permissions")?.0,
    };

    if record.status != "active" {
        return Err(AppError::Unauthorized);
    }
    Ok(record)
}
