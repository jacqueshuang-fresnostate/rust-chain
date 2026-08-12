use super::*;

/// 按规范化符号、状态和市场类型筛选交易对，并返回资产展示字段的分页结果和总数。
/// 非空筛选会执行与写入相同的枚举/格式校验，分页统一裁剪；读取不锁交易对，也不查询活动订单。
pub(crate) async fn list_admin_trading_pairs(
    pool: Option<Pool<MySql>>,
    query: AdminTradingPairQuery,
) -> AppResult<AdminTradingPairsResponse> {
    let symbol = query
        .symbol
        .and_then(optional_string)
        .map(|value| normalize_trading_pair_symbol(&value))
        .transpose()?;
    let status = query
        .status
        .and_then(optional_string)
        .map(|value| validate_trading_pair_status(&value))
        .transpose()?;
    let market_type = query
        .market_type
        .and_then(optional_string)
        .map(|value| validate_trading_pair_market_type(&value))
        .transpose()?;
    let pool = admin_mysql_pool(pool)?;
    let (pairs, total) = list_admin_trading_pairs_from_store(
        &pool,
        AdminTradingPairListFilter {
            symbol,
            status,
            market_type,
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(AdminTradingPairsResponse { pairs, total })
}

/// 按交易对 ID 读取资产、符号、Logo、精度、最小订单额、状态和市场类型。
/// 查询不加锁；记录缺失返回未找到，数据库错误直接返回，不启动行情或撮合组件。
pub(crate) async fn get_admin_trading_pair(
    pool: Option<Pool<MySql>>,
    pair_id: u64,
) -> AppResult<AdminTradingPairResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_trading_pair_from_store(&pool, pair_id).await
}

/// 创建现货交易对并返回含资产符号、精度、状态和市场类型的数据库快照。
/// 请求须满足异资产、合法符号、非负精度和正最小订单额；状态/市场类型缺省为 disabled/external，权限由调用方校验。
/// 事务按基准资产 ID 后计价资产 ID 的调用顺序确认两项资产可用，再插入交易对、回读并写审计；唯一键或 SQL 失败整体回滚。
/// 本用例无幂等键，且提交后不启动行情订阅或交易撮合。
pub(crate) async fn create_admin_trading_pair(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateTradingPairRequest,
) -> AppResult<AdminTradingPairResponse> {
    validate_create_trading_pair_request(&request)?;
    let symbol = normalize_trading_pair_symbol(&request.symbol)?;
    let logo_url = validate_optional_image_url(request.logo_url, "trading pair logo_url")?;
    let status = request
        .status
        .as_deref()
        .map(validate_trading_pair_status)
        .transpose()?
        .unwrap_or_else(|| "disabled".to_owned());
    let market_type = request
        .market_type
        .as_deref()
        .map(validate_trading_pair_market_type)
        .transpose()?
        .unwrap_or_else(|| "external".to_owned());
    let pool = admin_mysql_pool(pool)?;

    // 创建交易对前锁定两个启用资产，避免资产状态变更与交易对创建竞态。
    let mut tx = pool.begin().await?;
    ensure_trading_pair_asset_in_tx(&mut tx, request.base_asset_id).await?;
    ensure_trading_pair_asset_in_tx(&mut tx, request.quote_asset_id).await?;
    let pair_id = insert_admin_trading_pair_in_tx(
        &mut tx,
        AdminTradingPairInsert {
            base_asset_id: request.base_asset_id,
            quote_asset_id: request.quote_asset_id,
            symbol,
            logo_url,
            price_precision: request.price_precision,
            qty_precision: request.qty_precision,
            min_order_value: request.min_order_value,
            status,
            market_type,
        },
    )
    .await?;
    let pair = load_admin_trading_pair_in_tx(&mut tx, pair_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "trading_pair.create",
            target_type: "trading_pair",
            target_id: pair.id,
            before_json: None,
            after_json: Some(trading_pair_audit_json(&pair)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(pair)
}

/// 更新交易对 Logo、精度、最小订单额、状态和市场类型，并返回最终配置快照。
/// 调用方须提供审计原因和合法完整配置；基准/计价资产及符号在此用例中不可修改。
/// 事务先锁交易对，再覆盖配置、回读并写 before/after 审计；记录缺失或数据库失败整体回滚。
/// 相同配置重放仍新增审计，提交后不会自动重载行情或处理现有订单。
pub(crate) async fn update_admin_trading_pair(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    pair_id: u64,
    request: UpdateTradingPairRequest,
) -> AppResult<AdminTradingPairResponse> {
    validate_update_trading_pair_request(&request)?;
    let status = validate_trading_pair_status(&request.status)?;
    let market_type = validate_trading_pair_market_type(&request.market_type)?;
    let logo_url = validate_optional_image_url(request.logo_url, "trading pair logo_url")?;
    let reason = required_admin_audit_reason(request.reason)?;
    let pool = admin_mysql_pool(pool)?;

    // 先锁定交易对旧值再更新，确保后台审计 before/after 对应同一次事务。
    let mut tx = pool.begin().await?;
    let before = lock_admin_trading_pair_in_tx(&mut tx, pair_id).await?;
    update_admin_trading_pair_in_tx(
        &mut tx,
        pair_id,
        AdminTradingPairUpdate {
            logo_url,
            price_precision: request.price_precision,
            qty_precision: request.qty_precision,
            min_order_value: request.min_order_value,
            status,
            market_type,
        },
    )
    .await?;
    let after = load_admin_trading_pair_in_tx(&mut tx, pair_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "trading_pair.config.update",
            target_type: "trading_pair",
            target_id: after.id,
            before_json: Some(trading_pair_audit_json(&before)),
            after_json: Some(trading_pair_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

/// 单独切换交易对 active/disabled 状态，并返回状态更新后的完整交易对。
/// 请求须含受支持状态和审计原因；本函数不检查活动订单、持仓或行情源，权限由调用方保证。
/// 事务先锁交易对，再更新状态、回读并写 before/after 审计；缺失或 SQL 失败整体回滚。
/// 重复设置同一状态仍会留下审计，且不发布市场状态事件。
pub(crate) async fn update_admin_trading_pair_status(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    pair_id: u64,
    request: UpdateTradingPairStatusRequest,
) -> AppResult<AdminTradingPairResponse> {
    let status = validate_trading_pair_status(&request.status)?;
    let reason = required_admin_audit_reason(request.reason)?;
    let pool = admin_mysql_pool(pool)?;

    // 先锁定交易对旧值再更新，确保后台审计 before/after 对应同一次事务。
    let mut tx = pool.begin().await?;
    let before = lock_admin_trading_pair_in_tx(&mut tx, pair_id).await?;
    update_admin_trading_pair_status_in_tx(&mut tx, pair_id, &status).await?;
    let after = load_admin_trading_pair_in_tx(&mut tx, pair_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "trading_pair.status.update",
            target_type: "trading_pair",
            target_id: after.id,
            before_json: Some(trading_pair_audit_json(&before)),
            after_json: Some(trading_pair_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

/// 按交易对和状态筛选行情策略，并返回配置、运行检查点和恢复字段的分页结果与总数。
/// 状态筛选只去除空白，分页执行统一裁剪；读取不锁策略或版本，不改变 worker 的运行状态。
pub(crate) async fn list_admin_market_strategies(
    pool: Option<Pool<MySql>>,
    query: AdminMarketStrategyQuery,
) -> AppResult<AdminMarketStrategiesResponse> {
    let pool = admin_mysql_pool(pool)?;
    let (strategies, total) = list_admin_market_strategies_from_store(
        &pool,
        AdminMarketStrategyListFilter {
            pair_id: query.pair_id,
            status: query.status.and_then(optional_string),
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(AdminMarketStrategiesResponse { strategies, total })
}

/// 创建行情策略及其首个运行检查点和版本 1 快照，并返回可供后台展示的策略。
/// 请求须引用有效交易对并满足价格、时间、波动率和成交量约束；初始状态缺省为 draft，管理员 ID用于版本和审计归属。
/// 同一事务依次确认交易对、插入策略、运行行、UUIDv7 版本、策略事件和后台审计；任一步失败整体回滚。
/// 创建无请求幂等键，提交只写数据库，不直接启动策略 worker。
pub(crate) async fn create_admin_market_strategy(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateMarketStrategyRequest,
) -> AppResult<AdminMarketStrategyResponse> {
    validate_create_market_strategy(&request)?;
    let pool = admin_mysql_pool(pool)?;

    // 策略主表、运行检查点、版本快照和审计事件必须同事务提交，避免策略可见但调度状态缺失。
    let mut tx = pool.begin().await?;
    let market_type = ensure_market_strategy_pair_in_tx(&mut tx, request.pair_id).await?;
    let status = request
        .status
        .as_deref()
        .map(validate_market_strategy_status)
        .transpose()?
        .unwrap_or_else(|| "draft".to_owned());
    let strategy_type = optional_string(request.strategy_type.clone()).unwrap();
    let strategy_id = insert_admin_market_strategy_in_tx(
        &mut tx,
        AdminMarketStrategyInsert {
            pair_id: request.pair_id,
            strategy_type,
            start_price: request.start_price.clone(),
            target_price: request.target_price.clone(),
            start_time: request.start_time,
            end_time: request.end_time,
            volatility: request.volatility.clone(),
            volume_min: request.volume_min.clone(),
            volume_max: request.volume_max.clone(),
            status: status.clone(),
        },
    )
    .await?;
    insert_market_strategy_run_in_tx(
        &mut tx,
        strategy_id,
        market_strategy_run_status(&status),
        &request.start_price,
        request.start_time,
    )
    .await?;
    insert_market_strategy_version_in_tx(
        &mut tx,
        strategy_id,
        1,
        request.start_time,
        market_strategy_config_json(&request, &status, &market_type),
        Uuid::now_v7().to_string(),
        admin_id,
    )
    .await?;
    let strategy = load_admin_market_strategy_in_tx(&mut tx, strategy_id).await?;
    record_admin_market_strategy_change_in_tx(
        &mut tx,
        admin_id,
        strategy_id,
        "market_strategy.create",
        None,
        Some(&strategy),
        request.reason,
    )
    .await?;
    tx.commit().await?;
    Ok(strategy)
}

/// 更新非 active 行情策略的配置，重置运行检查点并追加下一版本快照后返回新配置。
/// 请求须通过数值校验和审计原因校验；事务锁定策略后若状态仍为 active 则返回冲突。
/// 锁后按“主配置、运行检查点、计算下一版本、回读、版本记录、策略事件、后台审计”顺序写入，失败整体回滚。
/// 每次成功调用都会生成新 UUIDv7 版本和审计，故相同请求重放不是幂等操作；不会直接唤醒 worker。
pub(crate) async fn update_admin_market_strategy(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    strategy_id: u64,
    request: UpdateMarketStrategyRequest,
) -> AppResult<AdminMarketStrategyResponse> {
    validate_update_market_strategy(&request)?;
    let reason = required_admin_audit_reason(request.reason.clone())?;
    let pool = admin_mysql_pool(pool)?;

    // 更新策略配置时先锁定旧值，再重置运行检查点并追加版本快照，保证审计和调度状态一致。
    let mut tx = pool.begin().await?;
    let before = lock_admin_market_strategy_in_tx(&mut tx, strategy_id).await?;
    if before.status == "active" {
        return Err(AppError::Conflict(
            "active market strategy must be paused or disabled before update".to_owned(),
        ));
    }
    let strategy_type = optional_string(request.strategy_type.clone()).unwrap();
    update_admin_market_strategy_in_tx(
        &mut tx,
        strategy_id,
        AdminMarketStrategyUpdate {
            strategy_type,
            start_price: request.start_price.clone(),
            target_price: request.target_price.clone(),
            start_time: request.start_time,
            end_time: request.end_time,
            volatility: request.volatility.clone(),
            volume_min: request.volume_min.clone(),
            volume_max: request.volume_max.clone(),
        },
    )
    .await?;
    update_market_strategy_run_checkpoint_in_tx(
        &mut tx,
        strategy_id,
        market_strategy_run_status(&before.status),
        &request.start_price,
        request.start_time,
    )
    .await?;
    let next_version = next_market_strategy_version_in_tx(&mut tx, strategy_id).await?;
    let after = load_admin_market_strategy_in_tx(&mut tx, strategy_id).await?;
    insert_market_strategy_version_in_tx(
        &mut tx,
        strategy_id,
        next_version,
        request.start_time,
        market_strategy_update_config_json(&request, &after.status, &after.market_type),
        Uuid::now_v7().to_string(),
        admin_id,
    )
    .await?;
    record_admin_market_strategy_change_in_tx(
        &mut tx,
        admin_id,
        strategy_id,
        "market_strategy.update",
        Some(&before),
        Some(&after),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

/// 同步切换行情策略业务状态和运行状态，并返回更新后的策略快照。
/// 目标状态仅限 draft/active/paused/disabled；本用例不校验显式审计原因，也不执行额外状态迁移图约束。
/// 事务先锁策略，再更新主状态、映射后的运行状态、回读并写策略事件及后台审计；运行行缺失或 SQL 失败整体回滚。
/// 相同状态重放仍写事件和审计，提交后由其他运行组件观察数据库变化。
pub(crate) async fn update_admin_market_strategy_status(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    strategy_id: u64,
    request: UpdateMarketStrategyStatusRequest,
) -> AppResult<AdminMarketStrategyResponse> {
    let status = validate_market_strategy_status(&request.status)?;
    let pool = admin_mysql_pool(pool)?;

    // 状态和运行状态一起更新；如果运行检查点缺失，整个状态变更回滚。
    let mut tx = pool.begin().await?;
    let before = lock_admin_market_strategy_in_tx(&mut tx, strategy_id).await?;
    update_market_strategy_status_in_tx(&mut tx, strategy_id, &status).await?;
    update_market_strategy_run_status_in_tx(
        &mut tx,
        strategy_id,
        market_strategy_run_status(&status),
    )
    .await?;
    let after = load_admin_market_strategy_in_tx(&mut tx, strategy_id).await?;
    record_admin_market_strategy_change_in_tx(
        &mut tx,
        admin_id,
        strategy_id,
        "market_strategy.status.update",
        Some(&before),
        Some(&after),
        request.reason,
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

async fn record_admin_market_strategy_change_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    strategy_id: u64,
    action: &'static str,
    before: Option<&AdminMarketStrategyResponse>,
    after: Option<&AdminMarketStrategyResponse>,
    reason: Option<String>,
) -> AppResult<()> {
    let before_json = before.map(market_strategy_audit_json);
    let after_json = after.map(market_strategy_audit_json);
    insert_market_strategy_event_in_tx(
        tx,
        strategy_id,
        action,
        json!({
            "before": before_json,
            "after": after_json,
        }),
    )
    .await?;
    insert_admin_audit_log_entry_in_tx(
        tx,
        admin_id,
        AdminAuditLogEntry {
            action,
            target_type: "market_strategy",
            target_id: strategy_id,
            before_json,
            after_json,
            reason,
        },
    )
    .await
}
