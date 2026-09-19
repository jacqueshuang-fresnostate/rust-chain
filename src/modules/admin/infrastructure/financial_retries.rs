//! 资金异常工作台存储；来源表不写入，仅更新调度或独立跟进元数据。

use crate::{
    error::{AppError, AppResult},
    modules::admin::presentation::{
        FinancialRetriesQuery, FinancialRetryCount, FinancialRetryIncident, FinancialRetryResponse,
        FinancialRetrySchedule,
    },
};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::{MySql, MySqlConnection, QueryBuilder, Transaction};

/// 为下一次只读事务显式指定可重复读，避免服务器默认隔离级别改变导致总数与分页不一致。
/// 只设置连接的下一事务属性，不修改全局隔离级别、业务数据或调度租约。
pub(crate) async fn prepare_financial_retry_snapshot(
    connection: &mut MySqlConnection,
) -> AppResult<()> {
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(connection)
        .await?;
    Ok(())
}

/// 内部租约快照只用于锁后判定与审计投影，token 不返回浏览器或审计日志。
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct FinancialRetryRecord {
    pub task_kind: String,
    pub item_id: u64,
    pub attempt_count: u64,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub next_attempt_at: DateTime<Utc>,
    pub outcome: String,
    pub lease_token: Option<String>,
}

impl FinancialRetryRecord {
    /// 完整调度状态的摘要包含微秒时间和租约身份；旧读、重复命令及 worker 回写均不能冒充当前版本。
    /// 只返回单向摘要，不暴露随机租约 token，也不引入新的调度字段或改变 worker 写合同。
    pub(crate) fn version(&self) -> String {
        let snapshot = serde_json::json!([
            self.task_kind,
            self.item_id,
            self.attempt_count,
            self.last_attempt_at,
            self.next_attempt_at,
            self.outcome,
            self.lease_token
        ]);
        format!("{:x}", Sha256::digest(snapshot.to_string().as_bytes()))
    }

    /// 以锁后时间判断租约；异常的 running/no-token 组合也不能在未来期限前被人工抢占。
    pub(crate) fn lease_status(&self, now: DateTime<Utc>) -> &'static str {
        if self.lease_token.is_some() || self.outcome == "running" {
            if self.next_attempt_at > now {
                "active"
            } else {
                "expired"
            }
        } else {
            "none"
        }
    }

    /// 输出无凭证调度快照；次数和上次尝试时间始终保留 worker 原始值。
    pub(crate) fn response(&self, now: DateTime<Utc>) -> FinancialRetrySchedule {
        FinancialRetrySchedule {
            version: self.version(),
            task_kind: self.task_kind.clone(),
            item_id: self.item_id,
            attempt_count: (self.task_kind != "seconds").then_some(self.attempt_count),
            last_attempt_at: self.last_attempt_at.map(|v| v.timestamp_millis()),
            next_attempt_at: (self.task_kind != "seconds")
                .then_some(self.next_attempt_at.timestamp_millis()),
            outcome: self.outcome.clone(),
            lease_status: self.lease_status(now).to_owned(),
        }
    }
}

#[derive(sqlx::FromRow)]
struct RetrySourceRow {
    #[sqlx(flatten)]
    schedule: FinancialRetryRecord,
    source_type: Option<String>,
    source_order_id: Option<String>,
    user_id: Option<u64>,
    amount: Option<String>,
    asset: Option<String>,
    #[sqlx(flatten)]
    incident: IncidentRow,
    failure_code: Option<String>,
    failure_at: Option<DateTime<Utc>>,
    review_window_start: Option<DateTime<Utc>>,
    review_window_end: Option<DateTime<Utc>>,
}

#[derive(sqlx::FromRow)]
struct IncidentRow {
    incident_version: u64,
    owner_admin_id: Option<u64>,
    owner_name: Option<String>,
    due_at: Option<DateTime<Utc>>,
    updated_by: Option<u64>,
    updated_at: Option<DateTime<Utc>>,
}

impl IncidentRow {
    fn response(self) -> FinancialRetryIncident {
        FinancialRetryIncident {
            version: self.incident_version,
            owner_admin_id: self.owner_admin_id,
            owner_name: self.owner_name,
            due_at: self.due_at.map(|v| v.timestamp_millis()),
            updated_by: self.updated_by,
            updated_at: self.updated_at.map(|v| v.timestamp_millis()),
        }
    }
}

// seconds rows are read-only evidence, never inserted into or claimed by the worker queue.
const EXCEPTION_ROWS: &str = r#"(
    SELECT task_kind, item_id, attempt_count, last_attempt_at, next_attempt_at, outcome,
           lease_token, NULL AS failure_code, NULL AS failure_at,
           NULL AS review_window_start, NULL AS review_window_end
    FROM financial_worker_retries WHERE task_kind IN ('earn','loan','commission')
    UNION ALL
    SELECT 'seconds', id, CAST(0 AS UNSIGNED), NULL, expires_at, 'manual_review', NULL,
           settlement_failure_code, settlement_failed_at, settlement_window_start, settlement_window_end
    FROM seconds_contract_orders WHERE status = 'manual_review'
) r"#;

fn filter_query(builder: &mut QueryBuilder<'_, MySql>, query: &FinancialRetriesQuery) {
    builder.push(" WHERE 1 = 1");
    if let Some(kind) = &query.task_kind {
        builder.push(" AND r.task_kind = ").push_bind(kind.clone());
    }
    if let Some(outcome) = &query.outcome {
        builder.push(" AND r.outcome = ").push_bind(outcome.clone());
    }
}

/// 读取同一只读事务内的分类总数；不写租约、不锁业务行，错误向上返回而非显示零积压。
pub(crate) async fn count_financial_retries(
    tx: &mut Transaction<'_, MySql>,
    query: &FinancialRetriesQuery,
) -> AppResult<Vec<FinancialRetryCount>> {
    let mut builder = QueryBuilder::new("SELECT r.outcome, COUNT(*) FROM ");
    builder.push(EXCEPTION_ROWS);
    filter_query(&mut builder, query);
    builder.push(" GROUP BY r.outcome ORDER BY r.outcome");
    let rows: Vec<(String, i64)> = builder.build_query_as().fetch_all(&mut **tx).await?;
    Ok(rows
        .into_iter()
        .map(|(outcome, count)| FinancialRetryCount { outcome, count })
        .collect())
}

/// 按稳定调度键分页；以任务种类和主键连接唯一来源行，孤立记录仍保留且证据为 null。
/// 金额仅为本金、佣金或押注原额，不计算应付、不合计跨币种金额，也不连接多态来源到猜测的订单表。
pub(crate) async fn list_financial_retries(
    tx: &mut Transaction<'_, MySql>,
    query: &FinancialRetriesQuery,
    now: DateTime<Utc>,
    limit: u32,
    offset: u32,
) -> AppResult<Vec<FinancialRetryResponse>> {
    let mut builder = QueryBuilder::new(
        r#"SELECT r.*,
          CASE WHEN e.id IS NOT NULL THEN 'earn_subscription'
               WHEN l.id IS NOT NULL THEN 'loan_order'
               WHEN s.id IS NOT NULL THEN 'seconds_contract_order' ELSE c.source_type END AS source_type,
          COALESCE(CAST(e.id AS CHAR), CAST(l.id AS CHAR), c.source_id, CAST(s.id AS CHAR)) AS source_order_id,
          COALESCE(e.user_id, l.user_id, c.user_id, s.user_id) AS user_id,
          CAST(COALESCE(e.amount, l.amount, c.commission_amount, s.stake_amount) AS CHAR) AS amount,
          a.symbol AS asset, CAST(COALESCE(i.version, 0) AS UNSIGNED) AS incident_version,
          i.owner_admin_id, owner.username AS owner_name, i.due_at, i.updated_by, i.updated_at
          FROM "#,
    );
    builder.push(EXCEPTION_ROWS).push(r#"
          LEFT JOIN earn_subscriptions e ON r.task_kind = 'earn' AND r.item_id = e.id
          LEFT JOIN loan_orders l ON r.task_kind = 'loan' AND r.item_id = l.id
          LEFT JOIN agent_commission_records c ON r.task_kind = 'commission' AND r.item_id = c.id
          LEFT JOIN seconds_contract_orders s ON r.task_kind = 'seconds' AND r.item_id = s.id
          LEFT JOIN assets a ON a.id = COALESCE(e.asset_id, l.asset_id, c.payout_asset_id, s.stake_asset)
          LEFT JOIN financial_retry_incidents i ON i.task_kind = r.task_kind AND i.item_id = r.item_id
          LEFT JOIN admin_users owner ON owner.id = i.owner_admin_id"#);
    filter_query(&mut builder, query);
    builder
        .push(" ORDER BY r.next_attempt_at, r.task_kind, r.item_id LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);
    let rows: Vec<RetrySourceRow> = builder.build_query_as().fetch_all(&mut **tx).await?;
    Ok(rows
        .into_iter()
        .map(|row| FinancialRetryResponse {
            schedule: row.schedule.response(now),
            source_type: row.source_type,
            source_order_id: row.source_order_id,
            user_id: row.user_id,
            amount: row.amount,
            asset: row.asset,
            incident: row.incident.response(),
            failure_code: row.failure_code,
            failure_at: row.failure_at.map(|v| v.timestamp_millis()),
            review_window_start: row.review_window_start.map(|v| v.timestamp_millis()),
            review_window_end: row.review_window_end.map(|v| v.timestamp_millis()),
        })
        .collect())
}

/// 仅锁目标调度行，与 claim/finish 的写锁串行化；不存在时不插入、不扫描或启动业务任务。
pub(crate) async fn lock_financial_retry(
    tx: &mut Transaction<'_, MySql>,
    kind: &str,
    item_id: u64,
) -> AppResult<FinancialRetryRecord> {
    sqlx::query_as(
        "SELECT * FROM financial_worker_retries WHERE task_kind = ? AND item_id = ? FOR UPDATE",
    )
    .bind(kind)
    .bind(item_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 调用者持有调度行锁后仅推进下次扫描时间并撤销过期 token；再次拒绝有效租约。
/// 不清空次数/上次尝试，不更新来源状态或金额，旧 finish 无法覆盖已撤销 token。
pub(crate) async fn reschedule_financial_retry(
    tx: &mut Transaction<'_, MySql>,
    record: &FinancialRetryRecord,
    now: DateTime<Utc>,
) -> AppResult<()> {
    if record.lease_status(now) == "active" {
        return Err(AppError::Conflict(
            "任务租约仍有效，请等待到期后刷新".to_owned(),
        ));
    }
    sqlx::query(
        "UPDATE financial_worker_retries SET next_attempt_at = ?, lease_token = NULL, outcome = 'ready' WHERE task_kind = ? AND item_id = ?",
    )
    .bind(now.naive_utc()).bind(&record.task_kind).bind(record.item_id)
    .execute(&mut **tx).await?;
    Ok(())
}

/// 元数据更新先锁当前异常来源以串行化首次登记；秒合约仅限仍在人工复核的订单，绝不写业务字段。
/// 与 worker 完成删除或结算终态互斥后，再锁元数据；不存在或已离开复核时拒绝登记。
pub(crate) async fn lock_financial_incident(
    tx: &mut Transaction<'_, MySql>,
    kind: &str,
    item_id: u64,
) -> AppResult<FinancialRetryIncident> {
    if kind == "seconds" {
        let found: Option<u64> = sqlx::query_scalar(
            "SELECT id FROM seconds_contract_orders WHERE id=? AND status='manual_review' FOR UPDATE"
        ).bind(item_id).fetch_optional(&mut **tx).await?;
        if found.is_none() {
            return Err(AppError::NotFound);
        }
    } else {
        lock_financial_retry(tx, kind, item_id).await?;
    }
    let row: Option<IncidentRow> = sqlx::query_as(
        "SELECT i.version AS incident_version, i.owner_admin_id, a.username AS owner_name,
         i.due_at, i.updated_by, i.updated_at FROM financial_retry_incidents i
         LEFT JOIN admin_users a ON a.id=i.owner_admin_id
         WHERE i.task_kind=? AND i.item_id=? FOR UPDATE",
    )
    .bind(kind)
    .bind(item_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.map(IncidentRow::response).unwrap_or_default())
}

/// 验证负责人为有效管理员并仅持久化事件元数据；期限来自显式输入，审计由调用方同事务提交。
/// 单调递增版本保留清空历史，防止删空再建造成旧版本重放；不更新任务租约、次数或资金状态。
pub(crate) async fn write_financial_incident(
    tx: &mut Transaction<'_, MySql>,
    kind: &str,
    item_id: u64,
    admin_id: u64,
    owner_admin_id: Option<u64>,
    due_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> AppResult<()> {
    if let Some(owner) = owner_admin_id {
        let found: Option<u64> = sqlx::query_scalar(
            "SELECT id FROM admin_users WHERE id=? AND status='active' FOR SHARE",
        )
        .bind(owner)
        .fetch_optional(&mut **tx)
        .await?;
        if found.is_none() {
            return Err(AppError::Validation("负责人必须为有效管理员".to_owned()));
        }
    }
    sqlx::query(
        "INSERT INTO financial_retry_incidents(task_kind,item_id,owner_admin_id,due_at,version,updated_by,updated_at)
         VALUES (?,?,?,?,1,?,?) ON DUPLICATE KEY UPDATE owner_admin_id=VALUES(owner_admin_id),
         due_at=VALUES(due_at), version=version+1, updated_by=VALUES(updated_by), updated_at=VALUES(updated_at)"
    ).bind(kind).bind(item_id).bind(owner_admin_id).bind(due_at.map(|v|v.naive_utc()))
        .bind(admin_id).bind(now.naive_utc()).execute(&mut **tx).await?;
    Ok(())
}
