//! 高风险配置变更申请的 MySQL 持久化适配器。

use super::*;
use crate::modules::admin::repository::{AdminConfigChangeRecord, AdminConfigChangeWrite};

#[derive(Debug, sqlx::FromRow)]
struct AdminConfigChangeRow {
    id: u64,
    request_no: String,
    config_domain: String,
    target_type: String,
    target_id: String,
    action: String,
    base_revision: Option<u64>,
    before_json: Option<SqlxJson<Value>>,
    proposed_json: SqlxJson<Value>,
    reason: String,
    risk_level: String,
    status: String,
    created_by: u64,
    reviewed_by: Option<u64>,
    review_reason: Option<String>,
    applied_by: Option<u64>,
    reviewed_at: Option<DateTime<Utc>>,
    applied_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<AdminConfigChangeRow> for AdminConfigChangeRecord {
    fn from(row: AdminConfigChangeRow) -> Self {
        Self {
            id: row.id,
            request_no: row.request_no,
            config_domain: row.config_domain,
            target_type: row.target_type,
            target_id: row.target_id,
            action: row.action,
            base_revision: row.base_revision,
            before_json: row.before_json.map(|value| value.0),
            proposed_json: row.proposed_json.0,
            reason: row.reason,
            risk_level: row.risk_level,
            status: row.status,
            created_by: row.created_by,
            reviewed_by: row.reviewed_by,
            review_reason: row.review_reason,
            applied_by: row.applied_by,
            reviewed_at: row.reviewed_at,
            applied_at: row.applied_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

pub(crate) struct AdminConfigChangeListFilter {
    pub(crate) config_domain: Option<String>,
    pub(crate) target_type: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) created_by: Option<u64>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

const CONFIG_CHANGE_COLUMNS: &str = r#"id, request_no, config_domain, target_type, target_id,
       action, base_revision, before_json, proposed_json, reason, risk_level, status,
       created_by, reviewed_by, review_reason, applied_by, reviewed_at, applied_at,
       created_at, updated_at"#;

/// 在调用方事务中创建 pending 申请并返回完整快照。
/// request_no 的唯一约束保证客户端重试不会产生同号申请；本函数不提交事务。
pub(crate) async fn insert_admin_config_change_in_tx(
    tx: &mut Transaction<'_, MySql>,
    write: AdminConfigChangeWrite,
) -> AppResult<AdminConfigChangeRecord> {
    let result = sqlx::query(
        r#"INSERT INTO admin_config_change_requests
           (request_no, config_domain, target_type, target_id, action, base_revision,
            before_json, proposed_json, reason, risk_level, status, created_by)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?)"#,
    )
    .bind(&write.request_no)
    .bind(&write.config_domain)
    .bind(&write.target_type)
    .bind(&write.target_id)
    .bind(&write.action)
    .bind(write.base_revision)
    .bind(write.before_json.map(SqlxJson))
    .bind(SqlxJson(write.proposed_json))
    .bind(&write.reason)
    .bind(&write.risk_level)
    .bind(write.created_by)
    .execute(&mut **tx)
    .await?;

    load_admin_config_change_in_tx(tx, result.last_insert_id(), false).await
}

/// 按 ID 在事务中读取申请；`for_update` 为真时锁定该行，供状态转换串行化。
pub(crate) async fn load_admin_config_change_in_tx(
    tx: &mut Transaction<'_, MySql>,
    id: u64,
    for_update: bool,
) -> AppResult<AdminConfigChangeRecord> {
    let mut sql =
        format!("SELECT {CONFIG_CHANGE_COLUMNS} FROM admin_config_change_requests WHERE id = ?");
    if for_update {
        sql.push_str(" FOR UPDATE");
    }
    sqlx::query_as::<_, AdminConfigChangeRow>(&sql)
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .map(AdminConfigChangeRecord::from)
        .ok_or(AppError::NotFound)
}

/// 在调用方持有的事务中保存高风险配置申请的复核结论、复核人、原因与时间。
/// `WHERE status = 'pending'` 是行锁之外的第二道并发保护：只有恰好更新一行才算首次复核成功；
/// 若另一请求已推进状态则返回冲突，由调用方整体回滚业务审计和状态变化，本函数不自行提交事务。
pub(crate) async fn update_admin_config_review_in_tx(
    tx: &mut Transaction<'_, MySql>,
    id: u64,
    status: &str,
    reviewer_id: u64,
    reason: &str,
) -> AppResult<()> {
    let affected = sqlx::query(
        r#"UPDATE admin_config_change_requests
           SET status = ?, reviewed_by = ?, review_reason = ?, reviewed_at = UTC_TIMESTAMP(6)
           WHERE id = ? AND status = 'pending'"#,
    )
    .bind(status)
    .bind(reviewer_id)
    .bind(reason)
    .bind(id)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if affected != 1 {
        return Err(AppError::Conflict(
            "config change request review raced with another request".to_owned(),
        ));
    }
    Ok(())
}

/// 在调用方事务中把已通过复核的高风险配置申请原子推进为已应用，并记录实际执行管理员与时间。
/// 条件更新只接受 `approved` 状态且必须影响一行；并发应用、重复执行或非法前置状态统一返回冲突，
/// 具体业务配置写入与对应审计由上层放在同一事务中提交，因此这里不制造“状态已应用但配置未生效”的部分结果。
pub(crate) async fn mark_admin_config_change_applied_in_tx(
    tx: &mut Transaction<'_, MySql>,
    id: u64,
    applied_by: u64,
) -> AppResult<()> {
    let affected = sqlx::query(
        r#"UPDATE admin_config_change_requests
           SET status = 'applied', applied_by = ?, applied_at = UTC_TIMESTAMP(6)
           WHERE id = ? AND status = 'approved'"#,
    )
    .bind(applied_by)
    .bind(id)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if affected != 1 {
        return Err(AppError::Conflict(
            "config change request apply raced with another request".to_owned(),
        ));
    }
    Ok(())
}

/// 分页查询变更申请，筛选项均为精确匹配，结果按创建时间和 ID 倒序。
pub(crate) async fn list_admin_config_changes(
    pool: &Pool<MySql>,
    filter: AdminConfigChangeListFilter,
) -> AppResult<(Vec<AdminConfigChangeRecord>, i64)> {
    let mut rows = QueryBuilder::<MySql>::new(format!(
        "SELECT {CONFIG_CHANGE_COLUMNS} FROM admin_config_change_requests"
    ));
    let mut total = QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM admin_config_change_requests");
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(value) = filter.config_domain.clone() {
            builder.push(" AND config_domain = ").push_bind(value);
        }
        if let Some(value) = filter.target_type.clone() {
            builder.push(" AND target_type = ").push_bind(value);
        }
        if let Some(value) = filter.status.clone() {
            builder.push(" AND status = ").push_bind(value);
        }
        if let Some(value) = filter.created_by {
            builder.push(" AND created_by = ").push_bind(value);
        }
    }

    let (rows, total) = fetch_admin_page::<AdminConfigChangeRow>(
        pool,
        rows,
        total,
        " ORDER BY created_at DESC, id DESC",
        filter.limit,
        filter.offset,
    )
    .await?;
    Ok((
        rows.into_iter()
            .map(AdminConfigChangeRecord::from)
            .collect(),
        total,
    ))
}
