use super::*;

/// 分页读取全部闪兑交易对及其源/目标资产展示信息，并返回匹配总数。
/// 当前查询对象不提供业务筛选，只裁剪 limit/offset；读取不加锁，连接池缺失或 SQL 映射失败返回错误。
pub(crate) async fn list_admin_convert_pairs(
    pool: Option<Pool<MySql>>,
    query: AdminConvertPairQuery,
) -> AppResult<ConvertPairsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let (pairs, total) = list_admin_convert_pairs_from_store(
        &pool,
        route_limit(query.limit),
        route_offset(query.offset),
    )
    .await?;
    Ok(ConvertPairsResponse { pairs, total })
}

/// 按交易对 ID 读取闪兑资产、定价、费率、源/目标限额和启用状态。
/// 查询不锁交易对；记录缺失返回未找到，数据库错误直接上抛，也不会计算即时兑换报价。
pub(crate) async fn get_admin_convert_pair(
    pool: Option<Pool<MySql>>,
    pair_id: u64,
) -> AppResult<ConvertPairResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_convert_pair_from_store(&pool, pair_id).await
}

/// 按用户、邮箱和状态筛选闪兑订单，并返回资产、金额、汇率、手续费和时间的分页结果。
/// 状态只去除空白，分页限制执行统一裁剪；读取不锁订单或钱包，匹配总数来自同组 SQL 谓词。
pub(crate) async fn list_admin_convert_orders(
    pool: Option<Pool<MySql>>,
    query: AdminConvertOrdersQuery,
) -> AppResult<ConvertOrdersResponse> {
    let pool = admin_mysql_pool(pool)?;
    let (orders, total) = list_admin_convert_orders_from_store(
        &pool,
        AdminConvertOrderListFilter {
            user_id: query.user_id,
            email: query.email,
            status: query.status.and_then(optional_string),
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(ConvertOrdersResponse { orders, total })
}

/// 按订单 ID 读取单笔闪兑订单及关联用户、资产和定价结果。
/// 查询不加订单或钱包锁；不存在返回未找到，SQL/行映射失败返回错误，不重试或改变订单状态。
pub(crate) async fn get_admin_convert_order(
    pool: Option<Pool<MySql>>,
    order_id: u64,
) -> AppResult<ConvertOrderResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_convert_order_from_store(&pool, order_id).await
}

/// 创建后台换币交易对，并返回数据库最终保存的完整配置。
/// 调用方须已完成管理员鉴权并提供审计原因；资产、计价模式、费率及限额须先通过领域校验。
/// 交易对写入、回读和后台审计共用一个事务，任一步失败都会回滚，避免配置与审计分离。
/// 本用例没有幂等键；提交结果不确定时直接重试可能触发唯一约束，而不会静默复用旧记录。
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

/// 在锁定的旧快照上合并换币交易对的局部更新，并保留准确的前后审计值。
/// 调用方须已完成管理员鉴权并提供审计原因；合并后的资产、费率和限额整体重新校验。
/// 事务按“锁定交易对、更新、回读、写审计”执行，配置和审计必须同时提交或同时回滚。
/// 本用例没有幂等键；每次成功调用都会新增审计记录，失败不会留下部分配置。
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

/// 删除已停用且无业务引用的闪兑交易对，成功返回空结果。
/// 调用方提供已鉴权管理员 ID和必填审计原因；仍启用的交易对直接返回参数错误。
/// 事务先锁交易对，再检查报价、订单及新币兑换规则引用，随后删除并写 before 审计；任一引用、缺失或 SQL 失败整体回滚。
/// 删除不具幂等性，成功后重放会得到未找到；本函数不清理外部行情缓存。
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
