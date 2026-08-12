use super::*;

#[derive(Debug)]
pub(crate) struct AdminTradingPairListFilter {
    pub(crate) symbol: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) market_type: Option<String>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug)]
pub(crate) struct AdminTradingPairInsert {
    pub(crate) base_asset_id: u64,
    pub(crate) quote_asset_id: u64,
    pub(crate) symbol: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) price_precision: i32,
    pub(crate) qty_precision: i32,
    pub(crate) min_order_value: BigDecimal,
    pub(crate) status: String,
    pub(crate) market_type: String,
}

#[derive(Debug)]
pub(crate) struct AdminTradingPairUpdate {
    pub(crate) logo_url: Option<String>,
    pub(crate) price_precision: i32,
    pub(crate) qty_precision: i32,
    pub(crate) min_order_value: BigDecimal,
    pub(crate) status: String,
    pub(crate) market_type: String,
}

#[derive(Debug)]
pub(crate) struct AdminMarketStrategyListFilter {
    pub(crate) pair_id: Option<u64>,
    pub(crate) status: Option<String>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug)]
pub(crate) struct AdminMarketStrategyInsert {
    pub(crate) pair_id: u64,
    pub(crate) strategy_type: String,
    pub(crate) start_price: BigDecimal,
    pub(crate) target_price: BigDecimal,
    pub(crate) start_time: DateTime<Utc>,
    pub(crate) end_time: DateTime<Utc>,
    pub(crate) volatility: BigDecimal,
    pub(crate) volume_min: BigDecimal,
    pub(crate) volume_max: BigDecimal,
    pub(crate) status: String,
}

#[derive(Debug)]
pub(crate) struct AdminMarketStrategyUpdate {
    pub(crate) strategy_type: String,
    pub(crate) start_price: BigDecimal,
    pub(crate) target_price: BigDecimal,
    pub(crate) start_time: DateTime<Utc>,
    pub(crate) end_time: DateTime<Utc>,
    pub(crate) volatility: BigDecimal,
    pub(crate) volume_min: BigDecimal,
    pub(crate) volume_max: BigDecimal,
}

#[derive(Debug)]
pub(crate) struct AdminMarketStrategyRecoveryJobListFilter {
    pub(crate) strategy_id: u64,
    pub(crate) status: Option<String>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug)]
pub(crate) struct AdminMarketStrategyRecoveryJobInsert {
    pub(crate) strategy_id: u64,
    pub(crate) requested_by: u64,
    pub(crate) config_version: i32,
    pub(crate) range_start: DateTime<Utc>,
    pub(crate) range_end: DateTime<Utc>,
    pub(crate) preview_token_hash: String,
    pub(crate) reason: String,
    pub(crate) expected_1m_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdminMarketStrategyRecoveryJobClaim {
    Claimed,
    AlreadyFinished,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct AdminSyntheticStrategySnapshot {
    pub(crate) symbol: String,
    pub(crate) price_precision: i32,
    pub(crate) start_price: BigDecimal,
    pub(crate) target_price: BigDecimal,
    pub(crate) start_time: DateTime<Utc>,
    pub(crate) end_time: DateTime<Utc>,
    pub(crate) volatility: BigDecimal,
    pub(crate) volume_min: BigDecimal,
    pub(crate) volume_max: BigDecimal,
    pub(crate) config_version: i32,
    pub(crate) seed: String,
    pub(crate) config_json: SqlxJson<Value>,
}

/// 分页查询交易对，返回符合调用方筛选条件的记录及相同谓词下的总数。
/// 交易对列表与计数通过连接池分别执行且均不加锁；并发写入可能造成页数据与总数快照不同，SQL 或字段映射失败直接返回错误。
pub(crate) async fn list_admin_trading_pairs(
    pool: &Pool<MySql>,
    filter: AdminTradingPairListFilter,
) -> AppResult<(Vec<AdminTradingPairResponse>, i64)> {
    let mut rows = admin_trading_pair_query();
    let mut total = QueryBuilder::<MySql>::new(
        r#"SELECT COUNT(*)
           FROM trading_pairs pairs
           INNER JOIN assets base ON base.id = pairs.base_asset
           INNER JOIN assets quote ON quote.id = pairs.quote_asset"#,
    );
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(symbol) = filter.symbol.clone() {
            builder.push(" AND pairs.symbol = ");
            builder.push_bind(symbol);
        }
        if let Some(status) = filter.status.clone() {
            builder.push(" AND pairs.status = ");
            builder.push_bind(status);
        }
        if let Some(market_type) = filter.market_type.clone() {
            builder.push(" AND pairs.market_type = ");
            builder.push_bind(market_type);
        }
    }

    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY pairs.id DESC",
        filter.limit,
        filter.offset,
    )
    .await
}

/// 按传入主键或筛选条件从连接池读取交易对并映射为应用层所需的完整记录。
/// 交易对不追加行锁，查询不创建事务；记录缺失时返回未找到，SQL 或字段解码失败直接返回错误，不产生审计副作用。
pub(crate) async fn load_admin_trading_pair(
    pool: &Pool<MySql>,
    pair_id: u64,
) -> AppResult<AdminTradingPairResponse> {
    let mut builder = admin_trading_pair_query();
    builder.push(" WHERE pairs.id = ");
    builder.push_bind(pair_id);
    builder
        .build_query_as::<AdminTradingPairResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

/// 在调用方事务中插入交易对并返回或保留数据库写入结果。
/// 交易对数据库唯一键冲突会映射为业务冲突；调用方持有提交边界并负责同事务审计，任一 SQL 失败使所属用例回滚。
pub(crate) async fn insert_admin_trading_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminTradingPairInsert,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, logo_url, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(input.base_asset_id)
    .bind(input.quote_asset_id)
    .bind(&input.symbol)
    .bind(&input.logo_url)
    .bind(input.price_precision)
    .bind(input.qty_precision)
    .bind(&input.min_order_value)
    .bind(&input.status)
    .bind(&input.market_type)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_trading_pair_error)?;
    Ok(result.last_insert_id())
}

/// 在调用方事务中按传入主键或筛选条件更新交易对，写入应用层已决定的目标字段。
/// 覆盖 Logo、价格/数量精度、最小下单额、状态和市场类型且不检查受影响行数；调用方须先锁交易对，并与审计统一提交。
pub(crate) async fn update_admin_trading_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
    input: AdminTradingPairUpdate,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE trading_pairs
           SET logo_url = ?, price_precision = ?, qty_precision = ?, min_order_value = ?, status = ?, market_type = ?
           WHERE id = ?"#,
    )
    .bind(&input.logo_url)
    .bind(input.price_precision)
    .bind(input.qty_precision)
    .bind(&input.min_order_value)
    .bind(&input.status)
    .bind(&input.market_type)
    .bind(pair_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在调用方事务中针对状态字段更新交易对，写入应用层已决定的目标字段。
/// 仅覆盖 status 且不检查受影响行数；调用方须先锁定交易对并验证启停迁移，函数不启动或停止行情 worker。
pub(crate) async fn update_admin_trading_pair_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
    status: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE trading_pairs SET status = ? WHERE id = ?")
        .bind(status)
        .bind(pair_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 按传入主键或筛选条件从调用方事务快照读取交易对并映射为应用层所需的完整记录。
/// 交易对不追加行锁，由调用方持有事务且本读取不提交；记录缺失时返回未找到，SQL 或字段解码失败直接返回错误，不产生审计副作用。
pub(crate) async fn load_admin_trading_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<AdminTradingPairResponse> {
    let mut builder = admin_trading_pair_query();
    builder.push(" WHERE pairs.id = ");
    builder.push_bind(pair_id);
    builder
        .build_query_as::<AdminTradingPairResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

/// 在调用方事务中按传入主键或筛选条件以 `FOR UPDATE` 锁定交易对并返回一致的修改前快照。
/// 交易对锁由调用方事务持有至结束；函数不自行提交，记录缺失返回未找到，SQL 或解码失败交由外层回滚。
pub(crate) async fn lock_admin_trading_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<AdminTradingPairResponse> {
    sqlx::query_as::<_, (u64,)>("SELECT id FROM trading_pairs WHERE id = ? FOR UPDATE")
        .bind(pair_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)?;
    load_admin_trading_pair_in_tx(tx, pair_id).await
}

/// 在调用方事务中按资产 ID 锁定 active 资产，确认其可作为新交易对的一端。
/// `FOR UPDATE` 锁由交易对写事务持有；资产缺失或非 active 均返回未找到，函数不比较 base/quote，也不提交或写审计。
pub(crate) async fn ensure_trading_pair_asset_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<()> {
    sqlx::query_as::<_, (u64,)>(
        "SELECT id FROM assets WHERE id = ? AND status = 'active' LIMIT 1 FOR UPDATE",
    )
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(())
}

/// 分页查询行情策略，返回符合调用方筛选条件的记录及相同谓词下的总数。
/// 行情策略列表与计数通过连接池分别执行且均不加锁；并发写入可能造成页数据与总数快照不同，SQL 或字段映射失败直接返回错误。
pub(crate) async fn list_admin_market_strategies(
    pool: &Pool<MySql>,
    filter: AdminMarketStrategyListFilter,
) -> AppResult<(Vec<AdminMarketStrategyResponse>, i64)> {
    let mut rows = admin_market_strategy_query();
    let mut total = QueryBuilder::<MySql>::new(
        r#"SELECT COUNT(*)
           FROM market_strategies strategies
           INNER JOIN trading_pairs pairs ON pairs.id = strategies.pair_id"#,
    );
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(pair_id) = filter.pair_id {
            builder.push(" AND strategies.pair_id = ");
            builder.push_bind(pair_id);
        }
        if let Some(status) = filter.status.clone() {
            builder.push(" AND strategies.status = ");
            builder.push_bind(status);
        }
    }

    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY strategies.created_at DESC, strategies.id DESC",
        filter.limit,
        filter.offset,
    )
    .await
}

/// 在调用方事务中锁定 active 交易对并返回其 market_type，确认其可绑定行情策略。
/// 交易对不存在或非 active 返回未找到，类型不是 internal/strategy 返回校验错误；锁持有至策略事务结束，函数不修改交易对。
pub(crate) async fn ensure_market_strategy_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<String> {
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT market_type FROM trading_pairs WHERE id = ? AND status = 'active' FOR UPDATE",
    )
    .bind(pair_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if !matches!(row.0.as_str(), "internal" | "strategy") {
        return Err(AppError::Validation(
            "market strategy can only be bound to internal or strategy pairs".to_owned(),
        ));
    }
    Ok(row.0)
}

/// 在调用方事务中为新策略插入运行检查点，初始化当前价格、生成时间、K 线时间和 idle 恢复状态。
/// `active_version` 同步初始化为首版 1；strategy_id 是否唯一由数据库约束决定，函数不更新已存在检查点；
/// 调用方须将本写入与策略、版本、事件及审计统一提交，SQL 失败整体回滚。
pub(crate) async fn insert_admin_market_strategy_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminMarketStrategyInsert,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO market_strategies
           (pair_id, strategy_type, start_price, target_price, start_time, end_time,
            volatility, volume_min, volume_max, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(input.pair_id)
    .bind(&input.strategy_type)
    .bind(&input.start_price)
    .bind(&input.target_price)
    .bind(input.start_time)
    .bind(input.end_time)
    .bind(&input.volatility)
    .bind(&input.volume_min)
    .bind(&input.volume_max)
    .bind(&input.status)
    .execute(&mut **tx)
    .await?;
    Ok(result.last_insert_id())
}

/// 在调用方事务中插入策略运行检查点，并将 `active_version` 绑定到首版。
/// 调用方必须先插入 `(strategy_id, version = 1)` 版本行以满足复合外键；函数不更新旧检查点，并与策略、节点、版本及审计由外层事务原子提交。
pub(crate) async fn insert_market_strategy_run_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    run_status: &str,
    current_price: &BigDecimal,
    start_time: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO strategy_runs
           (strategy_id, active_version, run_status, current_price, last_generated_at,
            last_kline_open_time, recovery_status)
           VALUES (?, 1, ?, ?, ?, ?, 'idle')"#,
    )
    .bind(strategy_id)
    .bind(run_status)
    .bind(current_price)
    .bind(start_time)
    .bind(start_time)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在调用方事务中插入行情策略版本快照并返回或保留数据库写入结果。
/// 行情策略版本快照函数不提供独立幂等保证，约束冲突沿用数据库错误；调用方持有提交边界并负责同事务审计，任一 SQL 失败使所属用例回滚。
pub(crate) async fn insert_market_strategy_version_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    version: i32,
    effective_time: DateTime<Utc>,
    config_json: Value,
    seed: String,
    admin_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO strategy_versions (strategy_id, version, effective_time, config_json, seed, created_by)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(strategy_id)
    .bind(version)
    .bind(effective_time)
    .bind(SqlxJson(config_json))
    .bind(seed)
    .bind(admin_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在调用方策略事务中按请求顺序全量写入目标节点；`sequence_no` 从 0 递增，
/// 不单独提交且不吞掉唯一键/外键错误，确保主策略、关系节点和版本 JSON 快照原子可见。
pub(crate) async fn insert_market_strategy_nodes_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    nodes: &[MarketStrategyNodeRequest],
) -> AppResult<()> {
    for (sequence_no, node) in nodes.iter().enumerate() {
        sqlx::query(
            r#"INSERT INTO market_strategy_nodes
               (strategy_id, sequence_no, target_time, target_type, target_value,
                execution_mode, tolerance, volatility, volume_min, volume_max)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(strategy_id)
        .bind(
            u32::try_from(sequence_no)
                .map_err(|_| AppError::Validation("too many market strategy nodes".to_owned()))?,
        )
        .bind(node.target_time)
        .bind(&node.target_type)
        .bind(&node.target_value)
        .bind(&node.execution_mode)
        .bind(&node.tolerance)
        .bind(&node.volatility)
        .bind(&node.volume_min)
        .bind(&node.volume_max)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// 在已锁定策略的更新事务中用新的完整节点集替换关系快照。
/// 先删后插与主配置、新版本和审计同事务；任一节点失败会回滚已删旧节点，不留部分更新。
pub(crate) async fn replace_market_strategy_nodes_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    nodes: &[MarketStrategyNodeRequest],
) -> AppResult<()> {
    sqlx::query("DELETE FROM market_strategy_nodes WHERE strategy_id = ?")
        .bind(strategy_id)
        .execute(&mut **tx)
        .await?;
    insert_market_strategy_nodes_in_tx(tx, strategy_id, nodes).await
}

/// 按策略 ID 读取关系表节点并以 `sequence_no,id` 稳定排序，旧策略无记录时返回空集合。
/// 读取不加锁也不解析版本 JSON；SQL 解码错误直接返回，不修改策略。
pub(crate) async fn list_market_strategy_nodes_from_store(
    pool: &Pool<MySql>,
    strategy_id: u64,
) -> AppResult<Vec<AdminMarketStrategyNodeResponse>> {
    Ok(sqlx::query_as::<_, AdminMarketStrategyNodeResponse>(
        r#"SELECT id, sequence_no, target_time, target_type, target_value,
                  execution_mode, tolerance, volatility, volume_min, volume_max
           FROM market_strategy_nodes
           WHERE strategy_id = ?
           ORDER BY sequence_no ASC, id ASC"#,
    )
    .bind(strategy_id)
    .fetch_all(pool)
    .await?)
}

/// 在调用方事务中读取节点快照，用于配置更新前的审计 before 数据。
/// 本查询不额外加锁；调用方应先锁主策略，以统一串行化锁顺序并避免节点死锁。
pub(crate) async fn list_market_strategy_nodes_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
) -> AppResult<Vec<AdminMarketStrategyNodeResponse>> {
    Ok(sqlx::query_as::<_, AdminMarketStrategyNodeResponse>(
        r#"SELECT id, sequence_no, target_time, target_type, target_value,
                  execution_mode, tolerance, volatility, volume_min, volume_max
           FROM market_strategy_nodes
           WHERE strategy_id = ?
           ORDER BY sequence_no ASC, id ASC"#,
    )
    .bind(strategy_id)
    .fetch_all(&mut **tx)
    .await?)
}

/// 在调用方事务中按传入主键或筛选条件更新行情策略，写入应用层已决定的目标字段。
/// 行情策略更新不检查受影响行数；调用方须先完成所需锁定和状态校验，并负责提交、回滚及同事务审计。
pub(crate) async fn update_admin_market_strategy_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    input: AdminMarketStrategyUpdate,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE market_strategies
           SET strategy_type = ?, start_price = ?, target_price = ?, start_time = ?, end_time = ?,
               volatility = ?, volume_min = ?, volume_max = ?
           WHERE id = ?"#,
    )
    .bind(&input.strategy_type)
    .bind(&input.start_price)
    .bind(&input.target_price)
    .bind(input.start_time)
    .bind(input.end_time)
    .bind(&input.volatility)
    .bind(&input.volume_min)
    .bind(&input.volume_max)
    .bind(strategy_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在调用方事务中把配置更新后的运行检查点重置到新起点，并同步切换 `active_version`。
/// 同时清空旧租约，避免旧 worker 继续以过期版本提交；受影响行数异常时外层配置、版本和节点更新整体回滚。
pub(crate) async fn update_market_strategy_run_checkpoint_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    run_status: &str,
    current_price: &BigDecimal,
    start_time: DateTime<Utc>,
    active_version: i32,
) -> AppResult<()> {
    let result = sqlx::query(
        r#"UPDATE strategy_runs
           SET active_version = ?, run_status = ?, current_price = ?, last_generated_at = NULL,
               last_kline_open_time = ?, recovery_status = 'idle', error_message = NULL,
               lease_owner = NULL, lease_expires_at = NULL
           WHERE strategy_id = ?"#,
    )
    .bind(active_version)
    .bind(run_status)
    .bind(current_price)
    .bind(start_time)
    .bind(strategy_id)
    .execute(&mut **tx)
    .await?;
    ensure_market_strategy_run_updated(result.rows_affected())
}

/// 在调用方事务快照中计算指定策略的下一个版本号，即当前最大版本加一。
/// 聚合查询不锁版本范围，单独并发调用可能得到同一版本号；调用方须先锁策略并立即插入版本，唯一键或 SQL 失败由整个策略事务回滚。
pub(crate) async fn next_market_strategy_version_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
) -> AppResult<i32> {
    Ok(sqlx::query_scalar(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM strategy_versions WHERE strategy_id = ?",
    )
    .bind(strategy_id)
    .fetch_one(&mut **tx)
    .await?)
}

/// 在调用方事务中按策略 ID 覆盖领域策略状态。
/// 更新不检查受影响行数；调用方须先锁策略、校验状态迁移，并与运行状态、策略事件及后台审计统一提交。
pub(crate) async fn update_market_strategy_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    status: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE market_strategies SET status = ? WHERE id = ?")
        .bind(status)
        .bind(strategy_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 在调用方事务中同步策略运行状态；暂停、禁用或草稿会释放租约，active 则保留当前配置版本等待 worker 竞争。
/// 更新会校验受影响行数；调用方须先锁主策略并与领域状态、事件及审计统一提交。
pub(crate) async fn update_market_strategy_run_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    run_status: &str,
) -> AppResult<()> {
    let result = sqlx::query(
        r#"UPDATE strategy_runs
           SET run_status = ?, recovery_status = 'idle', error_message = NULL,
               lease_owner = CASE WHEN ? = 'running' THEN lease_owner ELSE NULL END,
               lease_expires_at = CASE WHEN ? = 'running' THEN lease_expires_at ELSE NULL END
           WHERE strategy_id = ?"#,
    )
    .bind(run_status)
    .bind(run_status)
    .bind(run_status)
    .bind(strategy_id)
    .execute(&mut **tx)
    .await?;
    ensure_market_strategy_run_updated(result.rows_affected())
}

/// 按传入主键或筛选条件从调用方事务快照读取行情策略并映射为应用层所需的完整记录。
/// 行情策略不追加行锁，由调用方持有事务且本读取不提交；记录缺失时返回未找到，SQL 或字段解码失败直接返回错误，不产生审计副作用。
pub(crate) async fn load_admin_market_strategy_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
) -> AppResult<AdminMarketStrategyResponse> {
    let mut builder = admin_market_strategy_query();
    builder.push(" WHERE strategies.id = ");
    builder.push_bind(strategy_id);
    builder
        .build_query_as::<AdminMarketStrategyResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

/// 按策略 ID 从连接池读取主配置和运行快照，供后台详情组装及缺口范围校验使用。
/// 查询不加锁、不读节点；记录缺失返回未找到，应用层可与独立节点查询组合。
pub(crate) async fn load_admin_market_strategy_from_store(
    pool: &Pool<MySql>,
    strategy_id: u64,
) -> AppResult<AdminMarketStrategyResponse> {
    let mut builder = admin_market_strategy_query();
    builder.push(" WHERE strategies.id = ");
    builder.push_bind(strategy_id);
    builder
        .build_query_as()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

/// 在调用方事务中按传入主键或筛选条件以 `FOR UPDATE` 锁定行情策略并返回一致的修改前快照。
/// 行情策略锁由调用方事务持有至结束；函数不自行提交，记录缺失返回未找到，SQL 或解码失败交由外层回滚。
pub(crate) async fn lock_admin_market_strategy_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
) -> AppResult<AdminMarketStrategyResponse> {
    let mut builder = admin_market_strategy_query();
    builder.push(" WHERE strategies.id = ");
    builder.push_bind(strategy_id);
    builder.push(" FOR UPDATE");
    builder
        .build_query_as::<AdminMarketStrategyResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

/// 在调用方事务中追加策略动作及 JSON 载荷，形成策略生命周期事件记录。
/// 事件插入不按 action 去重且不会向外部消息系统发布；调用方负责与策略状态和后台审计原子提交，JSON 绑定或 SQL 失败时整体回滚。
pub(crate) async fn insert_market_strategy_event_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    action: &str,
    payload_json: Value,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO strategy_events (strategy_id, event_type, payload_json)
           VALUES (?, ?, ?)"#,
    )
    .bind(strategy_id)
    .bind(action)
    .bind(SqlxJson(payload_json))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn ensure_market_strategy_run_updated(rows_affected: u64) -> AppResult<()> {
    if rows_affected != 1 {
        return Err(AppError::Conflict(
            "market strategy run checkpoint is missing".to_owned(),
        ));
    }
    Ok(())
}

/// 读取单策略的生成器权威快照：主配置、交易对精度以及运行检查点绑定的 active version。
/// 查询不加锁；策略、运行行或对应版本缺失返回未找到，不静默切换到未激活的新版本。
pub(crate) async fn load_admin_synthetic_strategy_snapshot(
    pool: &Pool<MySql>,
    strategy_id: u64,
) -> AppResult<AdminSyntheticStrategySnapshot> {
    sqlx::query_as::<_, AdminSyntheticStrategySnapshot>(
        r#"SELECT pairs.symbol,
                  pairs.price_precision,
                  strategies.start_price,
                  strategies.target_price,
                  strategies.start_time,
                  strategies.end_time,
                  strategies.volatility,
                  strategies.volume_min,
                  strategies.volume_max,
                  versions.version AS config_version,
                  versions.seed,
                  versions.config_json
           FROM market_strategies strategies
           INNER JOIN trading_pairs pairs ON pairs.id = strategies.pair_id
           INNER JOIN strategy_runs runs ON runs.strategy_id = strategies.id
           INNER JOIN strategy_versions versions
             ON versions.strategy_id = strategies.id
            AND versions.version = runs.active_version
           WHERE strategies.id = ?"#,
    )
    .bind(strategy_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)
}

/// 查询指定策略 `[range_start,range_end)` 范围内 Mongo 已存在的 1m 开盘时间，用于缺口差集计算。
/// 只读 `interval/open_time` 并按时间升序；不创建集合/索引、不解码 OHLCV，Mongo 错误原样上抛。
pub(crate) async fn list_existing_one_minute_open_times(
    database: &mongodb::Database,
    symbol: &str,
    range_start: DateTime<Utc>,
    range_end: DateTime<Utc>,
) -> AppResult<Vec<DateTime<Utc>>> {
    use futures_util::TryStreamExt;
    use mongodb::bson::{DateTime as BsonDateTime, Document, doc};

    let symbol = crate::modules::market::ValidatedMarketSymbol::from_raw(symbol)
        .map_err(|error| AppError::Validation(error.to_string()))?;
    let collection = database.collection::<Document>(
        &crate::modules::market::infrastructure::kline_collection_name(&symbol),
    );
    let mut cursor = collection
        .find(doc! {
            "interval": "1m",
            "open_time": {
                "$gte": BsonDateTime::from_millis(range_start.timestamp_millis()),
                "$lt": BsonDateTime::from_millis(range_end.timestamp_millis()),
            }
        })
        .projection(doc! { "_id": 0, "open_time": 1 })
        .sort(doc! { "open_time": 1 })
        .await?;
    let mut open_times = Vec::new();
    while let Some(document) = cursor.try_next().await? {
        if let Ok(value) = document.get_datetime("open_time")
            && let Some(open_time) = DateTime::from_timestamp_millis(value.timestamp_millis())
        {
            open_times.push(open_time);
        }
    }
    Ok(open_times)
}

/// 按策略和可选状态分页查询手动补偿任务，数据行与 COUNT 共用完全相同的谓词。
/// 查询不返回 `preview_token_hash`、不加锁或推进状态；并发新任务可使页行与总数短暂分属不同快照。
pub(crate) async fn list_market_strategy_recovery_jobs_from_store(
    pool: &Pool<MySql>,
    filter: AdminMarketStrategyRecoveryJobListFilter,
) -> AppResult<(Vec<MarketStrategyRecoveryJobResponse>, i64)> {
    let mut rows = recovery_job_query();
    let mut total = QueryBuilder::<MySql>::new(
        "SELECT COUNT(*) FROM kline_recovery_jobs jobs WHERE jobs.strategy_id = ",
    );
    rows.push(" WHERE jobs.strategy_id = ");
    rows.push_bind(filter.strategy_id);
    total.push_bind(filter.strategy_id);
    if let Some(status) = filter.status {
        rows.push(" AND jobs.status = ");
        rows.push_bind(status.clone());
        total.push(" AND jobs.status = ");
        total.push_bind(status);
    }
    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY jobs.created_at DESC, jobs.id DESC",
        filter.limit,
        filter.offset,
    )
    .await
}

/// 在已锁定策略的调用方事务中插入 pending 补偿任务，预览令牌仅保存 SHA-256 哈希。
/// 哈希唯一约束使同一预览令牌的并发/重放只能创建一个任务；冲突映射为明确业务冲突并由外层回滚审计。
pub(crate) async fn insert_market_strategy_recovery_job_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminMarketStrategyRecoveryJobInsert,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO kline_recovery_jobs
           (strategy_id, requested_by, config_version, range_start, range_end,
            preview_token_hash, reason, status, expected_1m_count)
           VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', ?)"#,
    )
    .bind(input.strategy_id)
    .bind(input.requested_by)
    .bind(input.config_version)
    .bind(input.range_start)
    .bind(input.range_end)
    .bind(input.preview_token_hash)
    .bind(input.reason)
    .bind(input.expected_1m_count)
    .execute(&mut **tx)
    .await
    .map_err(|error| {
        if is_mysql_duplicate_key(&error) {
            AppError::Conflict("preview_token has already been executed".to_owned())
        } else {
            AppError::Database(error)
        }
    })?;
    Ok(result.last_insert_id())
}

/// 在调用方事务快照中回读一个补偿任务，作为接口提交成功的响应与审计 after 快照。
/// 不加行锁也不执行 K 线写入；任务缺失返回未找到，使外层事务不会提交不可读任务。
pub(crate) async fn load_market_strategy_recovery_job_in_tx(
    tx: &mut Transaction<'_, MySql>,
    job_id: u64,
) -> AppResult<MarketStrategyRecoveryJobResponse> {
    let mut query = recovery_job_query();
    query.push(" WHERE jobs.id = ");
    query.push_bind(job_id);
    query
        .build_query_as()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

/// 在独立短事务内认领补偿任务：`pending` 直接转 `running`，超过显式截止时间的 `running` 可原子重新认领。
/// 已终态返回幂等结果；新鲜 `running` 返回冲突，从而同时避免并发写 Mongo 与崩溃后永久卡住。
pub(crate) async fn claim_market_strategy_recovery_job(
    pool: &Pool<MySql>,
    job_id: u64,
    started_at: DateTime<Utc>,
    stale_before: DateTime<Utc>,
) -> AppResult<AdminMarketStrategyRecoveryJobClaim> {
    let mut tx = pool.begin().await?;
    let (status, previous_started_at) = sqlx::query_as::<_, (String, Option<DateTime<Utc>>)>(
        "SELECT status, started_at FROM kline_recovery_jobs WHERE id = ? FOR UPDATE",
    )
    .bind(job_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;
    let outcome = match status.as_str() {
        "pending" => {
            let result = sqlx::query(
                r#"UPDATE kline_recovery_jobs
                   SET status = 'running', started_at = ?, completed_at = NULL,
                       error_message = NULL
                   WHERE id = ? AND status = 'pending'"#,
            )
            .bind(started_at)
            .bind(job_id)
            .execute(&mut *tx)
            .await?;
            if result.rows_affected() != 1 {
                return Err(AppError::Conflict(
                    "market recovery job could not be claimed".to_owned(),
                ));
            }
            AdminMarketStrategyRecoveryJobClaim::Claimed
        }
        "running" if previous_started_at.is_none_or(|value| value <= stale_before) => {
            let result = sqlx::query(
                r#"UPDATE kline_recovery_jobs
                   SET started_at = ?, completed_at = NULL, error_message = NULL
                   WHERE id = ? AND status = 'running'
                     AND (started_at IS NULL OR started_at <= ?)"#,
            )
            .bind(started_at)
            .bind(job_id)
            .bind(stale_before)
            .execute(&mut *tx)
            .await?;
            if result.rows_affected() != 1 {
                return Err(AppError::Conflict(
                    "market recovery job could not be reclaimed".to_owned(),
                ));
            }
            AdminMarketStrategyRecoveryJobClaim::Claimed
        }
        "completed" | "failed" => AdminMarketStrategyRecoveryJobClaim::AlreadyFinished,
        "running" => {
            return Err(AppError::Conflict(
                "market recovery job is already running".to_owned(),
            ));
        }
        _ => {
            return Err(AppError::Internal(
                "market recovery job has an invalid status".to_owned(),
            ));
        }
    };
    tx.commit().await?;
    Ok(outcome)
}

/// 从连接池回读补偿任务最新状态，用于 HTTP 执行结束后返回 completed/failed 完整读模型。
/// 查询不加锁、不暴露令牌哈希；记录不存在返回未找到，SQL 或解码失败直接上抛。
pub(crate) async fn load_market_strategy_recovery_job_from_store(
    pool: &Pool<MySql>,
    job_id: u64,
) -> AppResult<MarketStrategyRecoveryJobResponse> {
    let mut query = recovery_job_query();
    query.push(" WHERE jobs.id = ");
    query.push_bind(job_id);
    query
        .build_query_as()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

/// 按预览令牌 SHA-256 哈希查找已有补偿任务，供应用层在重做缺口校验前快速收敛重放。
/// 返回的读模型不包含哈希本身；未命中返回 `None`，查询不加锁、不推进任务。
pub(crate) async fn load_market_strategy_recovery_job_by_token_hash(
    pool: &Pool<MySql>,
    preview_token_hash: &str,
) -> AppResult<Option<MarketStrategyRecoveryJobResponse>> {
    let mut query = recovery_job_query();
    query.push(" WHERE jobs.preview_token_hash = ");
    query.push_bind(preview_token_hash.to_owned());
    query
        .build_query_as()
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)
}

/// 在调用方事务内将 `running` 任务收敛为 `completed`，固化实际 1m/聚合根数和完成时间。
/// 以 `status = running` 作乐观条件，受影响行不为 1 则返回冲突；调用方应在同事务内追加 completed 策略事件。
pub(crate) async fn complete_market_strategy_recovery_job_in_tx(
    tx: &mut Transaction<'_, MySql>,
    job_id: u64,
    actual_1m_count: u32,
    actual_aggregate_count: u32,
    completed_at: DateTime<Utc>,
) -> AppResult<()> {
    let result = sqlx::query(
        r#"UPDATE kline_recovery_jobs
           SET status = 'completed', actual_1m_count = ?, actual_aggregate_count = ?,
               error_message = NULL, completed_at = ?
           WHERE id = ? AND status = 'running'"#,
    )
    .bind(actual_1m_count)
    .bind(actual_aggregate_count)
    .bind(completed_at)
    .bind(job_id)
    .execute(&mut **tx)
    .await?;
    if result.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "market recovery job is no longer running".to_owned(),
        ));
    }
    Ok(())
}

/// 在调用方事务内将 `running` 任务收敛为 `failed`，保留已成功 upsert 的实际根数和截断后错误。
/// Mongo 与 MySQL 无跨库事务，因此失败统计是可审计进度而非回滚承诺；同事务必须追加 failed 策略事件。
pub(crate) async fn fail_market_strategy_recovery_job_in_tx(
    tx: &mut Transaction<'_, MySql>,
    job_id: u64,
    actual_1m_count: u32,
    actual_aggregate_count: u32,
    error_message: &str,
    completed_at: DateTime<Utc>,
) -> AppResult<()> {
    let error_message = error_message.chars().take(4_096).collect::<String>();
    let result = sqlx::query(
        r#"UPDATE kline_recovery_jobs
           SET status = 'failed', actual_1m_count = ?, actual_aggregate_count = ?,
               error_message = ?, completed_at = ?
           WHERE id = ? AND status = 'running'"#,
    )
    .bind(actual_1m_count)
    .bind(actual_aggregate_count)
    .bind(error_message)
    .bind(completed_at)
    .bind(job_id)
    .execute(&mut **tx)
    .await?;
    if result.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "market recovery job is no longer running".to_owned(),
        ));
    }
    Ok(())
}

fn recovery_job_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::new(
        r#"SELECT jobs.id, jobs.strategy_id, jobs.requested_by, jobs.config_version,
                  jobs.range_start, jobs.range_end, jobs.reason, jobs.status,
                  jobs.expected_1m_count, jobs.actual_1m_count,
                  jobs.actual_aggregate_count, jobs.error_message, jobs.started_at,
                  jobs.completed_at, jobs.created_at, jobs.updated_at
           FROM kline_recovery_jobs jobs"#,
    )
}

fn admin_market_strategy_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT strategies.id,
                  strategies.pair_id,
                  pairs.symbol,
                  pairs.market_type,
                  strategies.strategy_type,
                  strategies.start_price,
                  strategies.target_price,
                  strategies.start_time,
                  strategies.end_time,
                  strategies.volatility,
                  strategies.volume_min,
                  strategies.volume_max,
                  strategies.status,
                  runs.run_status,
                  runs.active_version,
                  runs.current_price,
                  runs.last_generated_at,
                  runs.last_kline_open_time,
                  runs.recovery_status,
                  strategies.created_at
           FROM market_strategies strategies
           INNER JOIN trading_pairs pairs ON pairs.id = strategies.pair_id
           LEFT JOIN strategy_runs runs ON runs.strategy_id = strategies.id"#,
    )
}

fn admin_trading_pair_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT pairs.id,
                  pairs.base_asset AS base_asset_id,
                  pairs.quote_asset AS quote_asset_id,
                  pairs.symbol,
                  pairs.logo_url,
                  base.symbol AS base_asset,
                  quote.symbol AS quote_asset,
                  pairs.price_precision,
                  pairs.qty_precision,
                  pairs.min_order_value,
                  pairs.status,
                  pairs.market_type,
                  pairs.created_at
           FROM trading_pairs pairs
           INNER JOIN assets base ON base.id = pairs.base_asset
           INNER JOIN assets quote ON quote.id = pairs.quote_asset"#,
    )
}

fn map_duplicate_trading_pair_error(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("trading pair already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}
