use super::*;

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
    let pairs = list_admin_trading_pairs_from_store(
        &pool,
        AdminTradingPairListFilter {
            symbol,
            status,
            market_type,
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(AdminTradingPairsResponse { pairs })
}

pub(crate) async fn get_admin_trading_pair(
    pool: Option<Pool<MySql>>,
    pair_id: u64,
) -> AppResult<AdminTradingPairResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_trading_pair_from_store(&pool, pair_id).await
}

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

pub(crate) async fn list_admin_market_strategies(
    pool: Option<Pool<MySql>>,
    query: AdminMarketStrategyQuery,
) -> AppResult<AdminMarketStrategiesResponse> {
    let pool = admin_mysql_pool(pool)?;
    let strategies = list_admin_market_strategies_from_store(
        &pool,
        AdminMarketStrategyListFilter {
            pair_id: query.pair_id,
            status: query.status.and_then(optional_string),
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(AdminMarketStrategiesResponse { strategies })
}

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
