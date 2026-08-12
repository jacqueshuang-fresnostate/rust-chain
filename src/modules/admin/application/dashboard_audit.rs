use super::*;

/// 汇总用户、钱包、行情、交易、产品、风险和后台审计指标，并合并行情监督器运行快照。
/// 各摘要查询不共享事务或快照锁，因此数据是近实时视图；任一 SQL 失败时整份仪表盘返回错误。
pub(crate) async fn get_admin_dashboard(
    pool: Option<Pool<MySql>>,
    runtime: MarketFeedRuntimeStatus,
) -> AppResult<AdminDashboardResponse> {
    let pool = admin_mysql_pool(pool)?;
    let generated_at = Utc::now();
    let users = load_admin_dashboard_users_summary(&pool).await?;
    let wallet = load_admin_dashboard_wallet_summary(&pool).await?;
    let market_counts = load_admin_dashboard_market_counts(&pool).await?;
    let saved_feed_config = load_admin_market_feed_config_from_store(&pool)
        .await?
        .map(market_feed_config_response);
    let feed_runtime_status = runtime
        .last_reload_status
        .clone()
        .unwrap_or_else(|| "not_started".to_owned());
    let market = AdminDashboardMarketSummary {
        active_pairs: market_counts.active_pairs,
        disabled_pairs: market_counts.disabled_pairs,
        external_pairs: market_counts.external_pairs,
        strategy_pairs: market_counts.strategy_pairs,
        feed_runtime_status,
        feed_needs_reload: saved_feed_config
            .as_ref()
            .is_some_and(|config| config.needs_reload),
        feed_symbols: runtime.symbols,
        feed_providers: runtime.providers,
    };
    let trading = load_admin_dashboard_trading_summary(&pool).await?;
    let products = load_admin_dashboard_products_summary(&pool).await?;
    let risk = load_admin_dashboard_risk_summary(&pool).await?;
    let admin_actions_24h = count_admin_dashboard_actions_24h(&pool).await?;
    let latest_actions = list_admin_dashboard_latest_actions(&pool).await?;

    // Dashboard 是跨多个后台子域的只读聚合，应用层负责组装，避免路由层重新耦合 SQL 细节。
    Ok(AdminDashboardResponse {
        generated_at,
        users,
        wallet,
        market,
        trading,
        products,
        risk,
        audit: AdminDashboardAuditSummary {
            admin_actions_24h,
            latest_actions,
        },
    })
}

/// 按管理员、动作、目标类型和目标 ID 筛选后台审计日志，并返回倒序分页记录与总数。
/// 文本筛选去除空白，limit 裁剪到 1～100、offset 最大 100000；读取审计日志本身不会再生成审计。
pub(crate) async fn list_admin_audit_logs(
    pool: Option<Pool<MySql>>,
    query: AdminAuditLogsQuery,
) -> AppResult<AdminAuditLogsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let (logs, total) = list_admin_audit_logs_from_store(
        &pool,
        AdminAuditLogListFilter {
            admin_id: query.admin_id,
            action: query.action.and_then(optional_string),
            target_type: query.target_type.and_then(optional_string),
            target_id: query.target_id.and_then(optional_string),
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(AdminAuditLogsResponse { logs, total })
}
