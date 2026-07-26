use super::*;

pub(crate) async fn list_admin_convert_pairs(
    pool: Option<Pool<MySql>>,
    query: AdminConvertPairQuery,
) -> AppResult<ConvertPairsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let pairs = list_admin_convert_pairs_from_store(&pool, route_limit(query.limit)).await?;
    Ok(ConvertPairsResponse { pairs })
}

pub(crate) async fn get_admin_convert_pair(
    pool: Option<Pool<MySql>>,
    pair_id: u64,
) -> AppResult<ConvertPairResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_convert_pair_from_store(&pool, pair_id).await
}

pub(crate) async fn list_admin_convert_orders(
    pool: Option<Pool<MySql>>,
    query: AdminConvertOrdersQuery,
) -> AppResult<ConvertOrdersResponse> {
    let pool = admin_mysql_pool(pool)?;
    let orders = list_admin_convert_orders_from_store(
        &pool,
        AdminConvertOrderListFilter {
            user_id: query.user_id,
            email: query.email,
            status: query.status.and_then(optional_string),
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(ConvertOrdersResponse { orders })
}

pub(crate) async fn get_admin_convert_order(
    pool: Option<Pool<MySql>>,
    order_id: u64,
) -> AppResult<ConvertOrderResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_convert_order_from_store(&pool, order_id).await
}

pub(crate) async fn create_admin_convert_pair(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateConvertPairRequest,
) -> AppResult<ConvertPairResponse> {
    validate_create_convert_pair(&request)?;
    let reason = required_admin_audit_reason(request.reason)?;
    let pool = admin_mysql_pool(pool)?;
    let enabled = request.enabled.unwrap_or(true);
    let fee_rate = request
        .fee_rate
        .clone()
        .unwrap_or_else(|| BigDecimal::from(0));
    let target_min_amount = request
        .target_min_amount
        .clone()
        .unwrap_or_else(|| request.min_amount.clone());
    let target_max_amount = request
        .target_max_amount
        .clone()
        .or_else(|| request.max_amount.clone());

    // 换币交易对写入和后台审计同事务提交，避免配置生效但缺少可追溯记录。
    let mut tx = pool.begin().await?;
    let pair_id = insert_admin_convert_pair_in_tx(
        &mut tx,
        AdminConvertPairInsert {
            from_asset_id: request.from_asset_id,
            to_asset_id: request.to_asset_id,
            pricing_mode: request.pricing_mode.trim().to_owned(),
            spread_rate: request.spread_rate,
            fee_rate,
            min_amount: request.min_amount,
            max_amount: request.max_amount,
            target_min_amount,
            target_max_amount,
            enabled,
        },
    )
    .await?;
    let pair = load_admin_convert_pair_in_tx(&mut tx, pair_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "convert_pair.create",
            target_type: "convert_pair",
            target_id: pair.id,
            before_json: None,
            after_json: Some(convert_pair_audit_json(&pair)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(pair)
}

pub(crate) async fn update_admin_convert_pair(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    pair_id: u64,
    request: UpdateConvertPairRequest,
) -> AppResult<ConvertPairResponse> {
    let reason = required_admin_audit_reason(request.reason.clone())?;
    let pool = admin_mysql_pool(pool)?;

    // 先锁定旧配置，再按请求字段合并新配置，确保审计 before/after 对应同一次写入。
    let mut tx = pool.begin().await?;
    let before = lock_admin_convert_pair_in_tx(&mut tx, pair_id).await?;
    let from_asset_id = request.from_asset_id.unwrap_or(before.from_asset_id);
    let to_asset_id = request.to_asset_id.unwrap_or(before.to_asset_id);
    let pricing_mode = request
        .pricing_mode
        .as_deref()
        .unwrap_or(&before.pricing_mode)
        .trim()
        .to_owned();
    let spread_rate = request
        .spread_rate
        .clone()
        .unwrap_or_else(|| before.spread_rate.clone());
    let fee_rate = request
        .fee_rate
        .clone()
        .unwrap_or_else(|| before.fee_rate.clone());
    let min_amount = request
        .min_amount
        .clone()
        .unwrap_or_else(|| before.min_amount.clone());
    let max_amount = request
        .max_amount
        .clone()
        .unwrap_or_else(|| before.max_amount.clone());
    let target_min_amount = request
        .target_min_amount
        .clone()
        .unwrap_or_else(|| before.target_min_amount.clone());
    let target_max_amount = request
        .target_max_amount
        .clone()
        .unwrap_or_else(|| before.target_max_amount.clone());
    let enabled = request.enabled.unwrap_or(before.enabled);
    let updates_config = request.from_asset_id.is_some()
        || request.to_asset_id.is_some()
        || request.pricing_mode.is_some()
        || request.spread_rate.is_some()
        || request.fee_rate.is_some()
        || request.min_amount.is_some()
        || request.max_amount.is_some()
        || request.target_min_amount.is_some()
        || request.target_max_amount.is_some();

    validate_convert_pair_values(
        from_asset_id,
        to_asset_id,
        &pricing_mode,
        &spread_rate,
        &fee_rate,
        &min_amount,
        max_amount.as_ref(),
        &target_min_amount,
        target_max_amount.as_ref(),
    )?;

    update_admin_convert_pair_in_tx(
        &mut tx,
        pair_id,
        AdminConvertPairUpdate {
            from_asset_id,
            to_asset_id,
            pricing_mode,
            spread_rate,
            fee_rate,
            min_amount,
            max_amount,
            target_min_amount,
            target_max_amount,
            enabled,
        },
    )
    .await?;
    let after = load_admin_convert_pair_in_tx(&mut tx, pair_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: if updates_config {
                "convert_pair.update"
            } else {
                "convert_pair.update_status"
            },
            target_type: "convert_pair",
            target_id: pair_id,
            before_json: Some(convert_pair_audit_json(&before)),
            after_json: Some(convert_pair_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn delete_admin_convert_pair(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    pair_id: u64,
    request: DeleteConvertPairRequest,
) -> AppResult<()> {
    let reason = required_admin_audit_reason(request.reason)?;
    let pool = admin_mysql_pool(pool)?;

    // 删除前锁定交易对并确认无报价、订单和新币兑换规则引用，避免悬挂外键语义。
    let mut tx = pool.begin().await?;
    let before = lock_admin_convert_pair_in_tx(&mut tx, pair_id).await?;
    if before.enabled {
        return Err(AppError::Validation(
            "convert pair must be disabled before deletion".to_owned(),
        ));
    }
    ensure_convert_pair_has_no_references_in_tx(&mut tx, pair_id).await?;
    delete_admin_convert_pair_in_tx(&mut tx, pair_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "convert_pair.delete",
            target_type: "convert_pair",
            target_id: pair_id,
            before_json: Some(convert_pair_audit_json(&before)),
            after_json: None,
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
