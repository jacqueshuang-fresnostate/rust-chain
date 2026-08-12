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
    pub(crate) offset: u32,
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct AdminDashboardMarketCounts {
    pub(crate) active_pairs: i64,
    pub(crate) disabled_pairs: i64,
    pub(crate) external_pairs: i64,
    pub(crate) strategy_pairs: i64,
}

/// 在调用方事务中写入一条后台管理员操作审计，保存目标、前后 JSON 快照和规范化原因。
/// target_id 以字符串落库，空白原因转为空值；函数本身就是审计副作用但不提交事务，序列化绑定或 SQL 失败使所属业务写入一并回滚。
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

/// 聚合全部用户的总数、active 数量和最近二十四小时新增数，返回仪表盘用户摘要。
/// 单条连接池聚合以数据库 UTC 时间为窗口且不锁用户；SQL 或计数映射失败返回错误，不产生审计或业务写入。
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

/// 汇总活跃资产、钱包账户、非零账户、待解锁/待充值/待提现数量及托管配置状态。
/// 各子查询在同一只读语句中执行且不加业务锁，托管状态仅表示存在 active 网关；数据库失败返回错误，不触发链上或审计副作用。
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
                  (SELECT COUNT(*) FROM wallet_deposit_events
                   WHERE status = 'observed') AS pending_deposits,
                  (SELECT COUNT(*) FROM wallet_withdrawal_requests
                   WHERE status IN ('pending_review', 'approved', 'broadcasting',
                                    'broadcasted', 'manual_review')) AS pending_withdrawals,
                  CASE WHEN EXISTS (SELECT 1 FROM wallet_chain_gateways WHERE status = 'active')
                       THEN 'active' ELSE 'not_configured' END AS custody_status"#,
    )
    .fetch_one(pool)
    .await?)
}

/// 统计交易对中的 active、disabled、external 和 strategy 数量，返回仪表盘市场计数。
/// 各分类按独立条件计数而非互斥分组；查询不锁交易对，SQL 或整数映射失败直接返回错误。
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

/// 汇总现货未终结订单、二十四小时现货成交、待处理闪兑和二十四小时已完成闪兑数量。
/// 时间窗口使用数据库 UTC 时间，各子查询不锁订单或成交记录；任一 SQL 失败返回错误且不改变交易状态。
pub(crate) async fn load_admin_dashboard_trading_summary(
    pool: &Pool<MySql>,
) -> AppResult<AdminDashboardTradingSummary> {
    Ok(sqlx::query_as::<_, AdminDashboardTradingSummary>(
        r#"SELECT (SELECT COUNT(*) FROM spot_orders
                   WHERE status IN ('pending', 'open', 'partially_filled')) AS spot_open_orders,
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

/// 汇总秒合约开仓、保证金持仓/强平及理财有效/即将到期数量，返回产品仪表盘摘要。
/// 二十四小时时间条件由数据库 UTC 时间计算，聚合不锁产品记录或推进结算；查询失败直接返回数据库错误。
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

/// 汇总二十四小时风险事件/拦截数以及 outbox、重试 inbox、死信 inbox 积压数。
/// 拦截数只匹配四种既有 decision 字符串，读取不锁事件队列也不重试消息；SQL 失败返回错误。
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

/// 按固定时间窗口统计后台二十四小时操作数，供仪表盘展示聚合结果而不加载明细行。
/// 该 SQL 只读且不加锁；数据库失败返回错误，不写业务表或审计记录。
pub(crate) async fn count_admin_dashboard_actions_24h(pool: &Pool<MySql>) -> AppResult<i64> {
    Ok(sqlx::query_as::<_, (i64,)>(
        r#"SELECT COUNT(*) FROM admin_audit_logs
           WHERE created_at >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 24 HOUR)"#,
    )
    .fetch_one(pool)
    .await?
    .0)
}

/// 读取按创建时间和 ID 倒序排列的最近五条后台审计动作，供仪表盘展示。
/// 连接池查询没有筛选、总数查询或行锁；并发新增可能改变下一次结果，SQL/映射失败直接返回错误。
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

/// 按管理员、动作、目标类型和目标 ID 筛选后台审计日志，分页返回完整前后快照及总数。
/// 列表与 COUNT 共用精确匹配谓词并按时间、ID 倒序；两次无锁读取可能受并发写入影响，JSON 解码或 SQL 失败返回错误。
pub(crate) async fn list_admin_audit_logs(
    pool: &Pool<MySql>,
    filter: AdminAuditLogListFilter,
) -> AppResult<(Vec<AdminAuditLogResponse>, i64)> {
    let mut rows = QueryBuilder::<MySql>::new(
        r#"SELECT id, admin_id, action, target_type, target_id,
                  before_json, after_json, reason, ip, created_at
           FROM admin_audit_logs"#,
    );
    let mut total = QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM admin_audit_logs");
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(admin_id) = filter.admin_id {
            builder.push(" AND admin_id = ");
            builder.push_bind(admin_id);
        }
        if let Some(action) = filter.action.clone() {
            builder.push(" AND action = ");
            builder.push_bind(action);
        }
        if let Some(target_type) = filter.target_type.clone() {
            builder.push(" AND target_type = ");
            builder.push_bind(target_type);
        }
        if let Some(target_id) = filter.target_id.clone() {
            builder.push(" AND target_id = ");
            builder.push_bind(target_id);
        }
    }

    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY created_at DESC, id DESC",
        filter.limit,
        filter.offset,
    )
    .await
}
