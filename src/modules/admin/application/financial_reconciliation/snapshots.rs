//! 人工采集和差异跟进用例；证据与审计原子提交，重放不重新采集，不触发资金操作。

use crate::{
    error::{AppError, AppResult},
    modules::admin::{
        application::admin_mysql_pool,
        infrastructure::{
            AdminAuditLogEntry,
            financial_reconciliation::{self as reader, snapshots as store},
            insert_admin_audit_log_entry_in_tx,
        },
        presentation::financial_reconciliation::snapshots::*,
    },
};
use chrono::{DateTime, Datelike, Utc};
use serde_json::json;
use sqlx::{Connection, MySqlPool};

fn pagination(limit: Option<u32>, offset: Option<u32>) -> AppResult<(u32, u32)> {
    let (limit, offset) = (limit.unwrap_or(20), offset.unwrap_or(0));
    if !(1..=100).contains(&limit) || offset > 100_000 {
        return Err(AppError::Validation(
            "分页条数须为 1–100，偏移不得超过 100000".into(),
        ));
    }
    Ok((limit, offset))
}

fn positive(id: u64) -> AppResult<()> {
    if id == 0 {
        return Err(AppError::Validation("记录和资产 ID 必须为正整数".into()));
    }
    Ok(())
}

fn validate_command(admin: u64, key: &str, reason: &mut String) -> AppResult<()> {
    if admin == 0 {
        return Err(AppError::Unauthorized);
    }
    if key.is_empty() || key.len() > 128 || !key.bytes().all(|b| (33..=126).contains(&b)) {
        return Err(AppError::Validation(
            "幂等键须为 1–128 位可见 ASCII 字符".into(),
        ));
    }
    *reason = reason.trim().to_owned();
    if reason.is_empty() || reason.chars().count() > 500 {
        return Err(AppError::Validation("操作原因须为 1–500 字".into()));
    }
    Ok(())
}

fn conflict() -> AppError {
    AppError::Conflict("幂等键已用于不同请求，请核对原始采集或跟进记录".into())
}

fn capture_replay(
    saved: store::SavedSnapshot,
    key: &str,
    hash: &[u8],
) -> AppResult<SnapshotDetail> {
    if saved.idempotency_key != key.as_bytes() || saved.request_hash != hash {
        return Err(conflict());
    }
    saved.response()
}

fn followup_replay(
    saved: store::SavedFollowup,
    key: &str,
    hash: &[u8],
) -> AppResult<FollowupRecord> {
    if saved.record.idempotency_key != key || saved.request_hash != hash {
        return Err(conflict());
    }
    Ok(saved.record)
}

fn duplicate(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .is_some_and(|e| e.is_unique_violation())
}

/// 人工请求建立可重复读一致快照，复用连接读取器后只追加证据与审计。
/// 原 GET 的 READ ONLY 合同不变；唯一键竞态先回滚再新读核验原键/请求，绝不重采已提交证据。
pub(crate) async fn capture(
    pool: Option<MySqlPool>,
    admin: u64,
    mut request: CaptureRequest,
) -> AppResult<SnapshotDetail> {
    positive(request.asset_id)?;
    validate_command(admin, &request.idempotency_key, &mut request.reason)?;
    let hash = store::digest(
        json!([request.asset_id, request.reason])
            .to_string()
            .as_bytes(),
    );
    let key_hash = store::digest(request.idempotency_key.as_bytes());
    let pool = admin_mysql_pool(pool)?;
    let mut conn = pool.acquire().await?;
    reader::prepare_snapshot(&mut conn).await?;
    let mut tx = conn
        .begin_with("START TRANSACTION WITH CONSISTENT SNAPSHOT, READ WRITE")
        .await?;
    if let Some(saved) = store::capture_by_key(&mut tx, admin, &key_hash).await? {
        let result = capture_replay(saved, &request.idempotency_key, &hash)?;
        tx.commit().await?;
        return Ok(result);
    }
    let captured_at = Utc::now();
    let report = serde_json::to_value(reader::report(&mut tx, request.asset_id).await?)
        .map_err(|_| AppError::Internal("cannot serialize reconciliation evidence".into()))?;
    let id =
        match store::insert_capture(&mut tx, admin, &request, &hash, captured_at, &report).await {
            Ok(id) => id,
            Err(error) if duplicate(&error) => {
                tx.rollback().await?;
                let saved = store::capture_by_key(&mut conn, admin, &key_hash)
                    .await?
                    .ok_or_else(conflict)?;
                return capture_replay(saved, &request.idempotency_key, &hash);
            }
            Err(error) => return Err(error.into()),
        };
    let result = store::detail(&mut tx, id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin,
        AdminAuditLogEntry {
            action: "financial_reconciliation.capture",
            target_type: "financial_reconciliation_snapshot",
            target_id: id,
            before_json: None,
            after_json: Some(
                json!({"snapshot": result.summary, "report_hash":result.report_hash,
            "capture_mode":"manual", "coverage":"partial"}),
            ),
            reason: Some(request.reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(result)
}

/// 历史页的总数与条目来自只读一致快照；不重新读取钱包或改写采集口径。
pub(crate) async fn history(
    pool: Option<MySqlPool>,
    query: SnapshotQuery,
) -> AppResult<SnapshotHistory> {
    let (limit, offset) = pagination(query.limit, query.offset)?;
    if let Some(id) = query.asset_id {
        positive(id)?;
    }
    let pool = admin_mysql_pool(pool)?;
    let mut conn = pool.acquire().await?;
    reader::prepare_snapshot(&mut conn).await?;
    let mut tx = conn
        .begin_with("START TRANSACTION WITH CONSISTENT SNAPSHOT, READ ONLY")
        .await?;
    let result = store::history(&mut tx, query.asset_id, limit, offset).await?;
    tx.commit().await?;
    Ok(result)
}

/// 精确历史证据只读回放，即使实时余额、资产名称或报表实现变化也不重新计算。
pub(crate) async fn detail(pool: Option<MySqlPool>, id: u64) -> AppResult<SnapshotDetail> {
    positive(id)?;
    let pool = admin_mysql_pool(pool)?;
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin_with("START TRANSACTION READ ONLY").await?;
    let result = store::detail(&mut tx, id).await?;
    tx.commit().await?;
    Ok(result)
}

/// 追加历史与最新版本共享只读快照；分页不代表完整处理历史或财务差异解决结论。
pub(crate) async fn followups(
    pool: Option<MySqlPool>,
    id: u64,
    query: FollowupQuery,
) -> AppResult<FollowupHistory> {
    positive(id)?;
    let (limit, offset) = pagination(query.limit, query.offset)?;
    let pool = admin_mysql_pool(pool)?;
    let mut conn = pool.acquire().await?;
    reader::prepare_snapshot(&mut conn).await?;
    let mut tx = conn
        .begin_with("START TRANSACTION WITH CONSISTENT SNAPSHOT, READ ONLY")
        .await?;
    let result = store::followup_history(&mut tx, id, limit, offset).await?;
    tx.commit().await?;
    Ok(result)
}

/// 只锁采集/跟进元数据后比较版本；负责人、明确期限、备注及操作者与审计原子追加。
/// 同键重放先于版本检查，冲突保留旧版本；不存在资金、订单、调度或差异“已解决”状态写入。
pub(crate) async fn append_followup(
    pool: Option<MySqlPool>,
    admin: u64,
    id: u64,
    mut request: FollowupRequest,
) -> AppResult<FollowupRecord> {
    positive(id)?;
    validate_command(admin, &request.idempotency_key, &mut request.reason)?;
    if request.owner_admin_id == Some(0) || request.expected_version == u64::MAX {
        return Err(AppError::Validation("无效负责人或跟进版本".into()));
    }
    request.notes = request.notes.trim().to_owned();
    if request.notes.is_empty() || request.notes.chars().count() > 2000 {
        return Err(AppError::Validation("跟进备注须为 1–2000 字".into()));
    }
    if let Some(due) = request.due_at {
        DateTime::from_timestamp_millis(due)
            .filter(|d| (1000..=9999).contains(&d.year()))
            .ok_or_else(|| AppError::Validation("处理期限不是有效时间".into()))?;
    }
    let hash = store::digest(
        json!([
            id,
            request.expected_version,
            request.owner_admin_id,
            request.due_at,
            request.notes,
            request.reason
        ])
        .to_string()
        .as_bytes(),
    );
    let key_hash = store::digest(request.idempotency_key.as_bytes());
    let pool = admin_mysql_pool(pool)?;
    let mut conn = pool.acquire().await?;
    reader::prepare_snapshot(&mut conn).await?;
    let mut tx = conn.begin().await?;
    // No consistent read precedes the parent lock: waiters see the last committed version.
    let before = store::lock_latest_followup(&mut tx, id).await?;
    if let Some(saved) = store::followup_by_key(&mut tx, admin, &key_hash).await? {
        let result = followup_replay(saved, &request.idempotency_key, &hash)?;
        tx.commit().await?;
        return Ok(result);
    }
    if before.as_ref().map_or(0, |r| r.version) != request.expected_version {
        return Err(AppError::Conflict(
            "跟进版本已变化，请重新读取后确认；原备注未提交".into(),
        ));
    }
    store::validate_owner(&mut tx, request.owner_admin_id).await?;
    // Match DATETIME(3) exactly so the first response and persisted replay are identical.
    let now = DateTime::from_timestamp_millis(Utc::now().timestamp_millis())
        .ok_or_else(|| AppError::Internal("invalid reconciliation clock".into()))?;
    let result = match store::insert_followup(&mut tx, admin, id, &request, &hash, now).await {
        Ok(result) => result,
        Err(error) if duplicate(&error) => {
            tx.rollback().await?;
            let saved = store::followup_by_key(&mut conn, admin, &key_hash)
                .await?
                .ok_or_else(conflict)?;
            return followup_replay(saved, &request.idempotency_key, &hash);
        }
        Err(error) => return Err(error.into()),
    };
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin,
        AdminAuditLogEntry {
            action: "financial_reconciliation.followup",
            target_type: "financial_reconciliation_snapshot",
            target_id: id,
            before_json: Some(json!(before)),
            after_json: Some(json!(&result)),
            reason: Some(request.reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(result)
}

#[cfg(test)]
#[path = "../../../../../tests/unit_src/src_modules_admin_financial_reconciliation_snapshot_tests.rs"]
mod tests;
