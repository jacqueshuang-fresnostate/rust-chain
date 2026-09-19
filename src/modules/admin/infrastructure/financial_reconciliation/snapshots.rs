//! 不可变人工采集存储；只写侧车证据/跟进表，业务钱包、订单和分录仅由报表读取。

use crate::{
    error::{AppError, AppResult},
    modules::admin::presentation::financial_reconciliation::snapshots::*,
};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::MySqlConnection;

const SUMMARY: &str = "id, asset_id, asset_symbol, precision_scale, schema_version,
 CAST(TIMESTAMPDIFF(MICROSECOND,'1970-01-01',captured_at) DIV 1000 AS SIGNED) AS captured_at, admin_id, reason";
const FOLLOWUP: &str = "id, snapshot_id, version, owner_admin_id,
 CAST(TIMESTAMPDIFF(MICROSECOND,'1970-01-01',due_at) DIV 1000 AS SIGNED) AS due_at, notes, admin_id, reason,
 CAST(TIMESTAMPDIFF(MICROSECOND,'1970-01-01',recorded_at) DIV 1000 AS SIGNED) AS recorded_at,
 CAST(idempotency_key AS CHAR CHARACTER SET ascii) AS idempotency_key";

/// 用规范 JSON 文本摘要关联请求/证据；幂等查重仍必须同时比较原始键，不能只信散列。
pub(crate) fn digest(value: &[u8]) -> Vec<u8> {
    Sha256::digest(value).to_vec()
}

fn report_digest(report: &Value) -> Vec<u8> {
    // MySQL JSON normalizes object order; hash canonical keys, never storage whitespace/order.
    let mut canonical = report.clone();
    canonical.sort_all_objects();
    digest(canonical.to_string().as_bytes())
}

/// 保存记录含原始请求摘要；仅应用层用于重放比较，历史列表不暴露这些内部字段。
#[derive(sqlx::FromRow)]
pub(crate) struct SavedSnapshot {
    #[sqlx(flatten)]
    pub summary: SnapshotSummary,
    pub idempotency_key: Vec<u8>,
    pub request_hash: Vec<u8>,
    report_hash: Vec<u8>,
    report_json: sqlx::types::Json<Value>,
}

impl SavedSnapshot {
    /// 检查版本、资产和摘要后输出原始 JSON，不调用当前报表或用新口径覆盖旧证据。
    pub(crate) fn response(self) -> AppResult<SnapshotDetail> {
        let report = self.report_json.0;
        if self.summary.schema_version != 1
            || report["coverage"] != "partial"
            || report["asset"]["asset_id"] != self.summary.asset_id
            || report["asset"]["symbol"] != self.summary.asset_symbol
            || report["asset"]["precision_scale"] != self.summary.precision_scale
            || report_digest(&report) != self.report_hash
        {
            return Err(AppError::Internal(
                "invalid saved reconciliation evidence".into(),
            ));
        }
        Ok(SnapshotDetail {
            summary: self.summary,
            idempotency_key: String::from_utf8(self.idempotency_key)
                .map_err(|_| AppError::Internal("invalid saved reconciliation key".into()))?,
            report_hash: self
                .report_hash
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
            report,
        })
    }
}

/// 幂等索引仅定位候选；普通读取不对资产、账户或管理员行加锁。
pub(crate) async fn capture_by_key(
    conn: &mut MySqlConnection,
    admin: u64,
    hash: &[u8],
) -> AppResult<Option<SavedSnapshot>> {
    let sql = format!(
        "SELECT {SUMMARY}, idempotency_key, request_hash, report_hash, report_json
        FROM financial_reconciliation_snapshots WHERE admin_id=? AND key_hash=?"
    );
    Ok(sqlx::query_as(&sql)
        .bind(admin)
        .bind(hash)
        .fetch_optional(conn)
        .await?)
}

/// 精确读取保存的证据；不存在返回 404，不生成替代快照。
pub(crate) async fn detail(conn: &mut MySqlConnection, id: u64) -> AppResult<SnapshotDetail> {
    let sql = format!(
        "SELECT {SUMMARY}, idempotency_key, request_hash, report_hash, report_json
        FROM financial_reconciliation_snapshots WHERE id=?"
    );
    let row: SavedSnapshot = sqlx::query_as(&sql)
        .bind(id)
        .fetch_optional(conn)
        .await?
        .ok_or(AppError::NotFound)?;
    row.response()
}

/// 只追加服务端计算的证据，调用者负责一致快照及同事务审计；唯一键冲突由应用层重读核验。
pub(crate) async fn insert_capture(
    conn: &mut MySqlConnection,
    admin: u64,
    request: &CaptureRequest,
    request_hash: &[u8],
    captured_at: DateTime<Utc>,
    report: &Value,
) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "INSERT INTO financial_reconciliation_snapshots
        (asset_id,asset_symbol,precision_scale,schema_version,captured_at,admin_id,reason,
         idempotency_key,key_hash,request_hash,report_hash,report_json)
        VALUES (?,?,?,1,?,?,?,?,?,?,?,?)",
    )
    .bind(request.asset_id)
    .bind(report["asset"]["symbol"].as_str())
    .bind(report["asset"]["precision_scale"].as_i64())
    .bind(captured_at.naive_utc())
    .bind(admin)
    .bind(&request.reason)
    .bind(request.idempotency_key.as_bytes())
    .bind(digest(request.idempotency_key.as_bytes()))
    .bind(request_hash)
    .bind(report_digest(report))
    .bind(sqlx::types::Json(report))
    .execute(conn)
    .await?
    .last_insert_id())
}

/// 页与总数共享调用者只读快照；排序使用不可变 ID，资产筛选不依赖当前资产目录。
pub(crate) async fn history(
    conn: &mut MySqlConnection,
    asset_id: Option<u64>,
    limit: u32,
    offset: u32,
) -> AppResult<SnapshotHistory> {
    let total = sqlx::query_scalar(
        "SELECT COUNT(*) FROM financial_reconciliation_snapshots
        WHERE (? IS NULL OR asset_id=?)",
    )
    .bind(asset_id)
    .bind(asset_id)
    .fetch_one(&mut *conn)
    .await?;
    let sql = format!(
        "SELECT {SUMMARY} FROM financial_reconciliation_snapshots
        WHERE (? IS NULL OR asset_id=?) ORDER BY id DESC LIMIT ? OFFSET ?"
    );
    let snapshots = sqlx::query_as(&sql)
        .bind(asset_id)
        .bind(asset_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(conn)
        .await?;
    Ok(SnapshotHistory {
        snapshots,
        total,
        limit,
        offset,
    })
}

/// 同一快照内按版本倒序返回追加历史和最新版本；空记录也先验证父采集存在。
pub(crate) async fn followup_history(
    conn: &mut MySqlConnection,
    snapshot_id: u64,
    limit: u32,
    offset: u32,
) -> AppResult<FollowupHistory> {
    let exists: Option<u64> =
        sqlx::query_scalar("SELECT id FROM financial_reconciliation_snapshots WHERE id=?")
            .bind(snapshot_id)
            .fetch_optional(&mut *conn)
            .await?;
    exists.ok_or(AppError::NotFound)?;
    let (total, latest_version): (i64, u64) = sqlx::query_as(
        "SELECT COUNT(*), CAST(COALESCE(MAX(version),0) AS UNSIGNED)
         FROM financial_reconciliation_followups WHERE snapshot_id=?",
    )
    .bind(snapshot_id)
    .fetch_one(&mut *conn)
    .await?;
    let sql = format!(
        "SELECT {FOLLOWUP} FROM financial_reconciliation_followups
        WHERE snapshot_id=? ORDER BY version DESC LIMIT ? OFFSET ?"
    );
    let records = sqlx::query_as(&sql)
        .bind(snapshot_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(conn)
        .await?;
    Ok(FollowupHistory {
        snapshot_id,
        latest_version,
        records,
        total,
        limit,
        offset,
    })
}

/// 重放元数据与回执一起读取；按操作者和键全局去重，禁止把同一键挪到另一个采集记录。
#[derive(sqlx::FromRow)]
pub(crate) struct SavedFollowup {
    #[sqlx(flatten)]
    pub record: FollowupRecord,
    pub request_hash: Vec<u8>,
}

/// 跟进重放只读候选，版本检查前比对原始键和请求；已提交操作不重复追加审计。
pub(crate) async fn followup_by_key(
    conn: &mut MySqlConnection,
    admin: u64,
    hash: &[u8],
) -> AppResult<Option<SavedFollowup>> {
    let sql = format!(
        "SELECT {FOLLOWUP}, request_hash FROM financial_reconciliation_followups
        WHERE admin_id=? AND key_hash=?"
    );
    Ok(sqlx::query_as(&sql)
        .bind(admin)
        .bind(hash)
        .fetch_optional(conn)
        .await?)
}

/// 新事务的首次读取先锁唯一父采集，取得锁后才建立一致快照读取不可变跟进版本。
/// 不锁不存在的跟进范围，避免不同父采集的首次登记互相持有间隙锁导致插入死锁。
pub(crate) async fn lock_latest_followup(
    conn: &mut MySqlConnection,
    id: u64,
) -> AppResult<Option<FollowupRecord>> {
    let exists: Option<u64> = sqlx::query_scalar(
        "SELECT id FROM financial_reconciliation_snapshots WHERE id=? FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?;
    exists.ok_or(AppError::NotFound)?;
    let sql = format!(
        "SELECT {FOLLOWUP} FROM financial_reconciliation_followups
        WHERE snapshot_id=? ORDER BY version DESC LIMIT 1"
    );
    Ok(sqlx::query_as(&sql).bind(id).fetch_optional(conn).await?)
}

/// 负责人必须是当前有效管理员；仅共享锁保护身份验证到提交，不写管理员状态。
pub(crate) async fn validate_owner(
    conn: &mut MySqlConnection,
    owner: Option<u64>,
) -> AppResult<()> {
    if let Some(owner) = owner {
        let found: Option<u64> = sqlx::query_scalar(
            "SELECT id FROM admin_users WHERE id=? AND status='active' FOR SHARE",
        )
        .bind(owner)
        .fetch_optional(conn)
        .await?;
        if found.is_none() {
            return Err(AppError::Validation("负责人必须为有效管理员".into()));
        }
    }
    Ok(())
}

/// 父采集锁下追加下一版本，旧版本不可变；只写明确的负责人/期限/备注，不改变差异结论。
pub(crate) async fn insert_followup(
    conn: &mut MySqlConnection,
    admin: u64,
    snapshot_id: u64,
    request: &FollowupRequest,
    hash: &[u8],
    now: DateTime<Utc>,
) -> Result<FollowupRecord, sqlx::Error> {
    let version = request.expected_version + 1;
    let id = sqlx::query(
        "INSERT INTO financial_reconciliation_followups
        (snapshot_id,version,owner_admin_id,due_at,notes,admin_id,reason,recorded_at,
         idempotency_key,key_hash,request_hash) VALUES (?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(snapshot_id)
    .bind(version)
    .bind(request.owner_admin_id)
    .bind(
        request
            .due_at
            .and_then(DateTime::from_timestamp_millis)
            .map(|d| d.naive_utc()),
    )
    .bind(&request.notes)
    .bind(admin)
    .bind(&request.reason)
    .bind(now.naive_utc())
    .bind(request.idempotency_key.as_bytes())
    .bind(digest(request.idempotency_key.as_bytes()))
    .bind(hash)
    .execute(conn)
    .await?
    .last_insert_id();
    Ok(FollowupRecord {
        id,
        snapshot_id,
        version,
        owner_admin_id: request.owner_admin_id,
        due_at: request.due_at,
        notes: request.notes.clone(),
        admin_id: admin,
        reason: request.reason.clone(),
        recorded_at: now.timestamp_millis(),
        idempotency_key: request.idempotency_key.clone(),
    })
}
