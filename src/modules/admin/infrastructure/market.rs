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
/// strategy_id 是否唯一由数据库约束决定，函数不更新已存在检查点；调用方须将本写入与策略、版本、事件及审计统一提交，SQL 失败整体回滚。
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

/// 在调用方事务中插入行情策略并返回或保留数据库写入结果。
/// 行情策略函数不提供独立幂等保证，约束冲突沿用数据库错误；调用方持有提交边界并负责同事务审计，任一 SQL 失败使所属用例回滚。
pub(crate) async fn insert_market_strategy_run_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    run_status: &str,
    current_price: &BigDecimal,
    start_time: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO strategy_runs
           (strategy_id, run_status, current_price, last_generated_at, last_kline_open_time, recovery_status)
           VALUES (?, ?, ?, ?, ?, 'idle')"#,
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

/// 在调用方事务中针对运行检查点更新行情策略运行检查点，写入应用层已决定的目标字段。
/// 行情策略运行检查点更新会校验受影响行数；调用方须先完成所需锁定和状态校验，并负责提交、回滚及同事务审计。
pub(crate) async fn update_market_strategy_run_checkpoint_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    run_status: &str,
    current_price: &BigDecimal,
    start_time: DateTime<Utc>,
) -> AppResult<()> {
    let result = sqlx::query(
        r#"UPDATE strategy_runs
           SET run_status = ?, current_price = ?, last_generated_at = NULL,
               last_kline_open_time = ?, recovery_status = 'idle', error_message = NULL
           WHERE strategy_id = ?"#,
    )
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

/// 在调用方事务中针对状态字段更新行情策略运行状态，写入应用层已决定的目标字段。
/// 行情策略运行状态更新会校验受影响行数；调用方须先完成所需锁定和状态校验，并负责提交、回滚及同事务审计。
pub(crate) async fn update_market_strategy_run_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    run_status: &str,
) -> AppResult<()> {
    let result = sqlx::query(
        "UPDATE strategy_runs SET run_status = ?, recovery_status = 'idle', error_message = NULL WHERE strategy_id = ?",
    )
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
