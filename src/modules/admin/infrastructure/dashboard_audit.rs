use super::*;

#[derive(Debug)]
pub(crate) struct AdminAuditLogEntry {
    pub(crate) action: &'static str,
    pub(crate) target_type: &'static str,
    pub(crate) target_id: u64,
    pub(crate) before_json: Option<Value>,
    pub(crate) after_json: Option<Value>,
    pub(crate) reason: Option<String>,
}

#[derive(Debug)]
pub(crate) struct AdminAuditLogListFilter {
    pub(crate) admin_id: Option<u64>,
    pub(crate) action: Option<String>,
    pub(crate) target_type: Option<String>,
    pub(crate) target_id: Option<String>,
    pub(crate) limit: u32,
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct AdminDashboardMarketCounts {
    pub(crate) active_pairs: i64,
    pub(crate) disabled_pairs: i64,
    pub(crate) external_pairs: i64,
    pub(crate) strategy_pairs: i64,
}

pub(crate) async fn insert_admin_audit_log_entry_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    entry: AdminAuditLogEntry,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, before_json, after_json, reason)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(admin_id)
    .bind(entry.action)
    .bind(entry.target_type)
    .bind(entry.target_id.to_string())
    .bind(entry.before_json.map(SqlxJson))
    .bind(entry.after_json.map(SqlxJson))
    .bind(optional_audit_reason(entry.reason))
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub(crate) async fn load_admin_dashboard_users_summary(
    pool: &Pool<MySql>,
) -> AppResult<AdminDashboardUsersSummary> {
    Ok(sqlx::query_as::<_, AdminDashboardUsersSummary>(
        r#"SELECT COUNT(*) AS total,
                  COUNT(CASE WHEN status = 'active' THEN 1 END) AS active,
                  COUNT(CASE WHEN created_at >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)
                             THEN 1 END) AS new_24h
           FROM users"#,
    )
    .fetch_one(pool)
    .await?)
}

pub(crate) async fn load_admin_dashboard_wallet_summary(
    pool: &Pool<MySql>,
) -> AppResult<AdminDashboardWalletSummary> {
    Ok(sqlx::query_as::<_, AdminDashboardWalletSummary>(
        r#"SELECT (SELECT COUNT(*) FROM assets WHERE status = 'active') AS active_assets,
                  (SELECT COUNT(*) FROM wallet_accounts) AS wallet_accounts,
                  (SELECT COUNT(*) FROM wallet_accounts
                   WHERE available <> 0 OR frozen <> 0 OR locked <> 0) AS non_zero_accounts,
                  (SELECT COUNT(*) FROM asset_lock_positions
                   WHERE status = 'active' AND unlock_at <= UTC_TIMESTAMP(6)) AS pending_unlocks,
                  (SELECT COUNT(*) FROM deposit_records WHERE status = 'pending') AS pending_deposits,
                  (SELECT COUNT(*) FROM withdraw_records WHERE status = 'pending') AS pending_withdrawals,
                  'not_configured' AS custody_status"#,
    )
    .fetch_one(pool)
    .await?)
}

pub(crate) async fn load_admin_dashboard_market_counts(
    pool: &Pool<MySql>,
) -> AppResult<AdminDashboardMarketCounts> {
    Ok(sqlx::query_as::<_, AdminDashboardMarketCounts>(
        r#"SELECT COUNT(CASE WHEN status = 'active' THEN 1 END) AS active_pairs,
                  COUNT(CASE WHEN status = 'disabled' THEN 1 END) AS disabled_pairs,
                  COUNT(CASE WHEN market_type = 'external' THEN 1 END) AS external_pairs,
                  COUNT(CASE WHEN market_type = 'strategy' THEN 1 END) AS strategy_pairs
           FROM trading_pairs"#,
    )
    .fetch_one(pool)
    .await?)
}

pub(crate) async fn load_admin_dashboard_trading_summary(
    pool: &Pool<MySql>,
) -> AppResult<AdminDashboardTradingSummary> {
    Ok(sqlx::query_as::<_, AdminDashboardTradingSummary>(
        r#"SELECT (SELECT COUNT(*) FROM spot_orders WHERE status IN ('pending', 'partial')) AS spot_open_orders,
                  (SELECT COUNT(*) FROM spot_trades
                   WHERE created_at >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)) AS spot_trades_24h,
                  (SELECT COUNT(*) FROM convert_orders WHERE status = 'pending') AS convert_pending_orders,
                  (SELECT COUNT(*) FROM convert_orders
                   WHERE status = 'completed'
                     AND updated_at >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)) AS convert_completed_24h"#,
    )
    .fetch_one(pool)
    .await?)
}

pub(crate) async fn load_admin_dashboard_products_summary(
    pool: &Pool<MySql>,
) -> AppResult<AdminDashboardProductsSummary> {
    Ok(sqlx::query_as::<_, AdminDashboardProductsSummary>(
        r#"SELECT (SELECT COUNT(*) FROM seconds_contract_orders WHERE status = 'opened') AS seconds_open_orders,
                  (SELECT COUNT(*) FROM margin_positions WHERE status = 'opened') AS margin_open_positions,
                  (SELECT COUNT(*) FROM margin_liquidation_records
                   WHERE liquidated_at >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)) AS margin_liquidated_24h,
                  (SELECT COUNT(*) FROM earn_subscriptions WHERE status = 'subscribed') AS earn_active_subscriptions,
                  (SELECT COUNT(*) FROM earn_subscriptions
                   WHERE status = 'subscribed'
                     AND matures_at <= DATE_ADD(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)) AS earn_maturing_24h"#,
    )
    .fetch_one(pool)
    .await?)
}

pub(crate) async fn load_admin_dashboard_risk_summary(
    pool: &Pool<MySql>,
) -> AppResult<AdminDashboardRiskSummary> {
    Ok(sqlx::query_as::<_, AdminDashboardRiskSummary>(
        r#"SELECT (SELECT COUNT(*) FROM risk_events
                   WHERE created_at >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)) AS risk_events_24h,
                  (SELECT COUNT(*) FROM risk_events
                   WHERE decision IN ('block', 'blocked', 'reject', 'rejected')
                     AND created_at >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)) AS blocked_events_24h,
                  (SELECT COUNT(*) FROM event_outbox WHERE status = 'pending') AS pending_outbox_events,
                  (SELECT COUNT(*) FROM event_inbox WHERE status = 'retry') AS retry_inbox_events,
                  (SELECT COUNT(*) FROM event_inbox WHERE status = 'dead_letter') AS dead_letter_inbox_events"#,
    )
    .fetch_one(pool)
    .await?)
}

pub(crate) async fn count_admin_dashboard_actions_24h(pool: &Pool<MySql>) -> AppResult<i64> {
    Ok(sqlx::query_as::<_, (i64,)>(
        r#"SELECT COUNT(*) FROM admin_audit_logs
           WHERE created_at >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)"#,
    )
    .fetch_one(pool)
    .await?
    .0)
}

pub(crate) async fn list_admin_dashboard_latest_actions(
    pool: &Pool<MySql>,
) -> AppResult<Vec<AdminDashboardAuditAction>> {
    Ok(sqlx::query_as::<_, AdminDashboardAuditAction>(
        r#"SELECT id, admin_id, action, target_type, target_id, created_at
           FROM admin_audit_logs
           ORDER BY created_at DESC, id DESC
           LIMIT 5"#,
    )
    .fetch_all(pool)
    .await?)
}

pub(crate) async fn list_admin_audit_logs(
    pool: &Pool<MySql>,
    filter: AdminAuditLogListFilter,
) -> AppResult<Vec<AdminAuditLogResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, admin_id, action, target_type, target_id,
                  before_json, after_json, reason, ip, created_at
           FROM admin_audit_logs
           WHERE 1 = 1"#,
    );
    if let Some(admin_id) = filter.admin_id {
        builder.push(" AND admin_id = ");
        builder.push_bind(admin_id);
    }
    if let Some(action) = filter.action {
        builder.push(" AND action = ");
        builder.push_bind(action);
    }
    if let Some(target_type) = filter.target_type {
        builder.push(" AND target_type = ");
        builder.push_bind(target_type);
    }
    if let Some(target_id) = filter.target_id {
        builder.push(" AND target_id = ");
        builder.push_bind(target_id);
    }
    builder.push(" ORDER BY created_at DESC, id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);

    Ok(builder
        .build_query_as::<AdminAuditLogResponse>()
        .fetch_all(pool)
        .await?)
}
