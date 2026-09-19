//! 资金异常治理用例；查询只读，人工动作仅调度或登记跟进并原子审计，不执行任何资金业务。

use super::admin_mysql_pool;
use crate::{
    error::{AppError, AppResult},
    modules::admin::{
        infrastructure::{
            AdminAuditLogEntry, count_financial_retries, insert_admin_audit_log_entry_in_tx,
            list_financial_retries, lock_financial_incident, lock_financial_retry,
            prepare_financial_retry_snapshot, reschedule_financial_retry, write_financial_incident,
        },
        presentation::{
            FinancialRetriesQuery, FinancialRetriesResponse, FinancialRetryIncident,
            FinancialRetrySchedule, RequeueFinancialRetryRequest,
            UpdateFinancialRetryIncidentRequest,
        },
    },
};
use chrono::{DateTime, Datelike, Utc};
use sqlx::{Connection, MySqlPool};

fn validate_kind(kind: &str) -> AppResult<()> {
    if !matches!(kind, "earn" | "loan" | "commission") {
        return Err(AppError::Validation("无效的资金任务类型".to_owned()));
    }
    Ok(())
}

fn validate_query(query: &FinancialRetriesQuery) -> AppResult<(u32, u32)> {
    if let Some(kind) = &query.task_kind
        && kind != "seconds"
    {
        validate_kind(kind)?;
    }
    if query.outcome.as_deref().is_some_and(|outcome| {
        !matches!(
            outcome,
            "ready" | "running" | "waiting_balance" | "waiting_source" | "failed" | "manual_review"
        )
    }) {
        return Err(AppError::Validation("无效的资金任务结果".to_owned()));
    }
    let (limit, offset) = (query.limit.unwrap_or(50), query.offset.unwrap_or(0));
    if !(1..=100).contains(&limit) || offset > 100_000 {
        return Err(AppError::Validation(
            "分页条数须为 1–100，偏移不得超过 100000".to_owned(),
        ));
    }
    Ok((limit, offset))
}

/// 分页与分类计数来自显式只读一致快照；读取不会修复订单、刷新租约、触发结算或追加审计。
pub(crate) async fn get_admin_financial_retries(
    pool: Option<MySqlPool>,
    query: FinancialRetriesQuery,
) -> AppResult<FinancialRetriesResponse> {
    let (limit, offset) = validate_query(&query)?;
    let pool = admin_mysql_pool(pool)?;
    let mut connection = pool.acquire().await?;
    prepare_financial_retry_snapshot(&mut connection).await?;
    let mut tx = connection
        .begin_with("START TRANSACTION WITH CONSISTENT SNAPSHOT, READ ONLY")
        .await?;
    let now = Utc::now();
    let counts = count_financial_retries(&mut tx, &query).await?;
    let retries = list_financial_retries(&mut tx, &query, now, limit, offset).await?;
    tx.commit().await?;
    Ok(FinancialRetriesResponse {
        total: counts.iter().map(|v| v.count).sum(),
        counts,
        retries,
        limit,
        offset,
        checked_at: now.timestamp_millis(),
    })
}

/// 认证管理员必须提供非空原因；锁后检查有效租约，只更新调度并在同事务记录前后快照。
/// 审计失败整体回滚；不读取或调用业务执行入口，过期租约也不赋予付款、退款或业务终态权限。
pub(crate) async fn requeue_admin_financial_retry(
    pool: Option<MySqlPool>,
    admin_id: u64,
    kind: String,
    item_id: u64,
    request: RequeueFinancialRetryRequest,
) -> AppResult<FinancialRetrySchedule> {
    if admin_id == 0 {
        return Err(AppError::Unauthorized);
    }
    validate_kind(&kind)?;
    if item_id == 0 {
        return Err(AppError::Validation("任务 ID 必须为正整数".to_owned()));
    }
    let reason = request.reason.trim();
    if reason.is_empty() || reason.chars().count() > 500 {
        return Err(AppError::Validation("操作原因须为 1–500 字".to_owned()));
    }
    if request.expected_version.len() != 64
        || !request
            .expected_version
            .bytes()
            .all(|c| c.is_ascii_hexdigit())
    {
        return Err(AppError::Validation("必须提供读取时的调度版本".to_owned()));
    }
    let pool = admin_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = lock_financial_retry(&mut tx, &kind, item_id).await?;
    if before.version() != request.expected_version {
        return Err(AppError::Conflict(
            "调度已变化，请刷新后重新确认".to_owned(),
        ));
    }
    let now = Utc::now();
    reschedule_financial_retry(&mut tx, &before, now).await?;
    let after = lock_financial_retry(&mut tx, &kind, item_id)
        .await?
        .response(now);
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "financial_retry.requeue",
            target_type: "financial_retry",
            target_id: item_id,
            before_json: Some(serde_json::json!(before.response(now))),
            after_json: Some(serde_json::json!(&after)),
            reason: Some(reason.to_owned()),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

/// 登记或修改异常负责人/明确期限，独立于资金调度；缺失版本、无效管理员或过时编辑拒绝。
/// 来源锁、元数据版本比较、元数据写入及审计属于同一事务；不改变秒合约判定或任何资金状态。
pub(crate) async fn update_admin_financial_incident(
    pool: Option<MySqlPool>,
    admin_id: u64,
    kind: String,
    item_id: u64,
    request: UpdateFinancialRetryIncidentRequest,
) -> AppResult<FinancialRetryIncident> {
    if admin_id == 0 {
        return Err(AppError::Unauthorized);
    }
    if kind != "seconds" {
        validate_kind(&kind)?;
    }
    if item_id == 0 || request.owner_admin_id == Some(0) {
        return Err(AppError::Validation(
            "记录及负责人 ID 必须为正整数".to_owned(),
        ));
    }
    let reason = request.reason.trim();
    if reason.is_empty() || reason.chars().count() > 500 {
        return Err(AppError::Validation("操作原因须为 1–500 字".to_owned()));
    }
    let due_at = request
        .due_at
        .map(|value| {
            DateTime::from_timestamp_millis(value)
                .filter(|date| (1000..=9999).contains(&date.year()))
                .ok_or_else(|| AppError::Validation("处理期限不是有效时间".to_owned()))
        })
        .transpose()?;
    let pool = admin_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = lock_financial_incident(&mut tx, &kind, item_id).await?;
    if before.version != request.expected_version {
        return Err(AppError::Conflict(
            "负责人或期限已变化，请刷新后重新确认".to_owned(),
        ));
    }
    write_financial_incident(
        &mut tx,
        &kind,
        item_id,
        admin_id,
        request.owner_admin_id,
        due_at,
        Utc::now(),
    )
    .await?;
    let after = lock_financial_incident(&mut tx, &kind, item_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "financial_retry.incident_update",
            target_type: "financial_retry",
            target_id: item_id,
            before_json: Some(serde_json::json!({"task_kind":kind,"incident":before})),
            after_json: Some(serde_json::json!({"task_kind":kind,"incident":after})),
            reason: Some(reason.to_owned()),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

#[cfg(test)]
#[path = "../../../../tests/unit_src/src_modules_admin_financial_retries_tests.rs"]
mod tests;
