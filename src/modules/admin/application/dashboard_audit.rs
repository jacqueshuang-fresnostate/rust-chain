//! 运营仪表盘聚合与后台审计日志检索的应用用例层。
//!
//! 两个用例都是只读的：仪表盘把分散在各子域的统计查询与行情监督器运行快照拼成一份总览，
//! 审计日志则提供按管理员、动作和目标检索操作留痕的唯一入口。
//! 二者都不写数据库，因此查看审计本身不会再产生审计，避免留痕数据被查询行为污染。

use super::*;

/// 汇总用户、钱包、行情、交易、产品、风险和后台审计指标，并合并行情监督器运行快照。
/// 各摘要查询不共享事务或快照锁，因此数据是近实时视图；任一 SQL 失败时整份仪表盘返回错误。
/// 行情板块由三处数据拼成：交易对计数来自数据库，订阅符号与提供商来自调用方传入的运行快照，
/// 待重载标记则取自数据库中保存配置的版本比对，因此它反映的是配置差异而非监督器是否存活。
/// 运行状态在快照缺失时回落为未启动，使部署尚未拉起监督器的环境也能正常展示。
/// 生成时间在函数入口一次取定，各分项查询却先后执行，故各板块之间存在毫秒级的时间偏差。
pub(crate) async fn get_admin_dashboard(
    pool: Option<Pool<MySql>>,
    runtime: MarketFeedRuntimeStatus,
    app_env: &str,
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
        environment: normalize_admin_dashboard_environment(app_env).to_owned(),
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

/// 把部署侧 APP_ENV 收敛为 Dashboard 对外稳定的四值合同，避免前端直接解释任意环境文本。
/// 常见缩写与本地/CI 名称分别映射到 production、staging、test、development；
/// 未知值按 development 降级且不会回显原文，避免把内部集群名称带到管理端。
pub(crate) fn normalize_admin_dashboard_environment(app_env: &str) -> &'static str {
    let normalized = app_env.trim().to_ascii_lowercase().replace('_', "-");
    match normalized.as_str() {
        "production" | "prod" => "production",
        "staging" | "stage" | "preprod" | "pre-production" => "staging",
        "test" | "testing" | "ci" => "test",
        "development" | "develop" | "dev" | "local" => "development",
        _ => "development",
    }
}

/// 按管理员、动作、目标类型、目标 ID 和审计时间范围筛选后台审计日志，并返回倒序分页记录与总数。
/// 文本筛选去除空白，时间范围使用包含边界且开始时间不得晚于结束时间；
/// limit 裁剪到 1～100、offset 最大 100000，读取审计日志本身不会再生成审计。
/// 目标编号按字符串筛选而非数值，因为不同资源的目标标识形态不一，其中包含幂等键这类非数字取值。
/// 偏移上限意味着无法靠深翻页遍历全部历史，检索久远记录应改用管理员、动作或目标条件收窄范围。
pub(crate) async fn list_admin_audit_logs(
    pool: Option<Pool<MySql>>,
    query: AdminAuditLogsQuery,
) -> AppResult<AdminAuditLogsResponse> {
    validate_admin_audit_log_time_range(query.created_from, query.created_to)?;
    let pool = admin_mysql_pool(pool)?;
    let (logs, total) = list_admin_audit_logs_from_store(
        &pool,
        AdminAuditLogListFilter {
            admin_id: query.admin_id,
            action: query.action.and_then(optional_string),
            target_type: query.target_type.and_then(optional_string),
            target_id: query.target_id.and_then(optional_string),
            created_from: query.created_from,
            created_to: query.created_to,
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(AdminAuditLogsResponse { logs, total })
}

/// 校验审计检索的可选时间边界；相等边界代表精确到同一毫秒的包含区间。
/// 任一边界缺省时视为开放区间，只有开始时间晚于结束时间才返回可纠正的 400 校验错误。
pub(crate) fn validate_admin_audit_log_time_range(
    created_from: Option<DateTime<Utc>>,
    created_to: Option<DateTime<Utc>>,
) -> AppResult<()> {
    if created_from
        .zip(created_to)
        .is_some_and(|(created_from, created_to)| created_from > created_to)
    {
        return Err(AppError::Validation(
            "created_from must not be later than created_to".to_owned(),
        ));
    }
    Ok(())
}
