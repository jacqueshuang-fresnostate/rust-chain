use super::*;

pub(crate) async fn list_admin_assets(
    pool: Option<Pool<MySql>>,
    query: AdminAssetQuery,
) -> AppResult<AdminAssetsResponse> {
    let symbol = query
        .symbol
        .and_then(optional_string)
        .map(|value| normalize_asset_symbol(&value))
        .transpose()?;
    let asset_type = query
        .asset_type
        .and_then(optional_string)
        .map(|value| validate_asset_type(&value))
        .transpose()?;
    let status = query
        .status
        .and_then(optional_string)
        .map(|value| validate_asset_status(&value))
        .transpose()?;
    let pool = admin_mysql_pool(pool)?;
    let assets = list_admin_assets_from_store(
        &pool,
        AdminAssetListFilter {
            symbol,
            asset_type,
            status,
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(AdminAssetsResponse { assets })
}

pub(crate) async fn get_admin_asset(
    pool: Option<Pool<MySql>>,
    asset_id: u64,
) -> AppResult<AdminAssetResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_asset_from_store(&pool, asset_id).await
}

pub(crate) async fn create_admin_asset(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateAssetRequest,
) -> AppResult<AdminAssetResponse> {
    validate_create_asset_request(&request)?;
    let symbol = normalize_asset_symbol(&request.symbol)?;
    let name = validate_asset_name(&request.name)?;
    let logo_url = validate_optional_image_url(request.logo_url, "asset logo_url")?;
    let asset_type = request
        .asset_type
        .as_deref()
        .map(validate_asset_type)
        .transpose()?
        .unwrap_or_else(|| "coin".to_owned());
    let status = request
        .status
        .as_deref()
        .map(validate_asset_status)
        .transpose()?
        .unwrap_or_else(|| "active".to_owned());
    let deposit_enabled = request.deposit_enabled.unwrap_or(true);
    let withdraw_enabled = request.withdraw_enabled.unwrap_or(true);
    let min_deposit_amount = request
        .min_deposit_amount
        .unwrap_or_else(|| BigDecimal::from(0));
    let deposit_fee = request.deposit_fee.unwrap_or_else(|| BigDecimal::from(0));
    let withdraw_fee = request.withdraw_fee.unwrap_or_else(|| BigDecimal::from(0));
    let withdraw_fee_tiers =
        normalize_asset_withdraw_fee_tiers(request.withdraw_fee_tiers.unwrap_or_default())?;
    validate_asset_fee_settings(&min_deposit_amount, &deposit_fee, &withdraw_fee)?;
    let pool = admin_mysql_pool(pool)?;

    // 资产创建和钱包账户初始化必须同事务提交，避免用户缺少新资产账户。
    let mut tx = pool.begin().await?;
    let asset_id = insert_admin_asset_in_tx(
        &mut tx,
        AdminAssetInsert {
            symbol,
            name,
            logo_url,
            precision_scale: request.precision_scale,
            asset_type,
            status,
            deposit_enabled,
            withdraw_enabled,
            min_deposit_amount,
            deposit_fee,
            withdraw_fee,
            withdraw_fee_tiers,
        },
    )
    .await?;
    let asset = load_admin_asset_in_tx(&mut tx, asset_id).await?;
    create_wallet_accounts_for_asset_in_tx(&mut tx, asset.id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "asset.create",
            target_type: "asset",
            target_id: asset.id,
            before_json: None,
            after_json: Some(asset_audit_json(&asset)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(asset)
}

pub(crate) async fn update_admin_asset(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    asset_id: u64,
    request: UpdateAssetRequest,
) -> AppResult<AdminAssetResponse> {
    validate_update_asset_request(&request)?;
    let name = validate_asset_name(&request.name)?;
    let asset_type = validate_asset_type(&request.asset_type)?;
    let status = validate_asset_status(&request.status)?;
    let logo_url = validate_optional_image_url(request.logo_url, "asset logo_url")?;
    let reason = required_admin_audit_reason(request.reason)?;
    let pool = admin_mysql_pool(pool)?;

    // 先锁定资产旧值再更新，确保资产配置和审计 before/after 对应同一次事务。
    let mut tx = pool.begin().await?;
    let before = lock_admin_asset_in_tx(&mut tx, asset_id).await?;
    let deposit_enabled = request.deposit_enabled.unwrap_or(before.deposit_enabled);
    let withdraw_enabled = request.withdraw_enabled.unwrap_or(before.withdraw_enabled);
    let min_deposit_amount = request
        .min_deposit_amount
        .unwrap_or_else(|| before.min_deposit_amount.clone());
    let deposit_fee = request
        .deposit_fee
        .unwrap_or_else(|| before.deposit_fee.clone());
    let withdraw_fee = request
        .withdraw_fee
        .unwrap_or_else(|| before.withdraw_fee.clone());
    let withdraw_fee_tiers = match request.withdraw_fee_tiers {
        Some(tiers) => normalize_asset_withdraw_fee_tiers(tiers)?,
        None => before.withdraw_fee_tiers.0.clone(),
    };
    validate_asset_fee_settings(&min_deposit_amount, &deposit_fee, &withdraw_fee)?;
    update_admin_asset_in_tx(
        &mut tx,
        asset_id,
        AdminAssetUpdate {
            name,
            logo_url,
            precision_scale: request.precision_scale,
            asset_type,
            status,
            deposit_enabled,
            withdraw_enabled,
            min_deposit_amount,
            deposit_fee,
            withdraw_fee,
            withdraw_fee_tiers,
        },
    )
    .await?;
    let after = load_admin_asset_in_tx(&mut tx, asset_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "asset.config.update",
            target_type: "asset",
            target_id: after.id,
            before_json: Some(asset_audit_json(&before)),
            after_json: Some(asset_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn delete_admin_asset(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    asset_id: u64,
    request: DeleteAssetRequest,
) -> AppResult<()> {
    let reason = required_admin_audit_reason(request.reason)?;
    let pool = admin_mysql_pool(pool)?;

    // 删除前先清理零余额钱包账户，再检查引用，避免仅由空钱包账户阻止资产退场。
    let mut tx = pool.begin().await?;
    let before = lock_admin_asset_in_tx(&mut tx, asset_id).await?;
    if before.status != "disabled" {
        return Err(AppError::Validation(
            "asset must be disabled before deletion".to_owned(),
        ));
    }
    delete_zero_balance_wallet_accounts_for_asset_in_tx(&mut tx, asset_id).await?;
    ensure_asset_has_no_references_in_tx(&mut tx, asset_id).await?;
    delete_admin_asset_in_tx(&mut tx, asset_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "asset.delete",
            target_type: "asset",
            target_id: asset_id,
            before_json: Some(asset_audit_json(&before)),
            after_json: None,
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

pub(crate) async fn list_admin_wallet_accounts(
    pool: Option<Pool<MySql>>,
    query: AdminWalletAccountQuery,
) -> AppResult<AdminWalletAccountsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let accounts = list_admin_wallet_accounts_from_store(
        &pool,
        AdminWalletAccountListFilter {
            user_id: query.user_id,
            email: query.email,
            asset_id: query.asset_id,
            include_empty: query.include_empty.unwrap_or(false),
            include_internal: query.include_internal.unwrap_or(false),
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(AdminWalletAccountsResponse { accounts })
}

pub(crate) async fn list_admin_wallet_ledger(
    pool: Option<Pool<MySql>>,
    query: AdminWalletLedgerQuery,
) -> AppResult<AdminWalletLedgerResponseList> {
    let pool = admin_mysql_pool(pool)?;
    let ledger = list_admin_wallet_ledger_from_store(
        &pool,
        AdminWalletLedgerListFilter {
            user_id: query.user_id,
            email: query.email,
            asset_id: query.asset_id,
            change_type: query.change_type,
            ref_type: query.ref_type,
            include_internal: query.include_internal.unwrap_or(false),
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(AdminWalletLedgerResponseList { ledger })
}

pub(crate) async fn list_admin_deposit_network_configs(
    pool: Option<Pool<MySql>>,
    query: AdminDepositNetworkConfigQuery,
) -> AppResult<AdminDepositNetworkConfigResponseList> {
    let network = query
        .network
        .and_then(optional_string)
        .map(|value| normalize_deposit_network(&value))
        .transpose()?;
    let address_group_code = query
        .address_group_code
        .and_then(optional_string)
        .map(|value| validate_address_group_code(&value))
        .transpose()?;
    let status = query
        .status
        .and_then(optional_string)
        .map(|value| validate_deposit_network_config_status(&value))
        .transpose()?;
    let asset_symbol = query
        .asset_symbol
        .and_then(optional_string)
        .map(|value| normalize_asset_symbol(&value))
        .transpose()?;
    let pool = admin_mysql_pool(pool)?;
    let configs = list_admin_deposit_network_configs_from_store(
        &pool,
        AdminDepositNetworkConfigListFilter {
            network,
            address_group_code,
            status,
            asset_symbol,
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(AdminDepositNetworkConfigResponseList { configs })
}

pub(crate) async fn create_admin_deposit_network_config(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateDepositNetworkConfigRequest,
) -> AppResult<AdminDepositNetworkConfigResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let network = normalize_deposit_network(&request.network)?;
    let display_name = validate_deposit_network_display_name(&request.display_name)?;
    let address_group_code = validate_address_group_code(&request.address_group_code)?;
    let address_group_name =
        validate_optional_length(request.address_group_name, "address_group_name", 128)?;
    let asset_symbols = normalize_deposit_asset_symbols(None, request.asset_symbols)?;
    let status = request
        .status
        .as_deref()
        .map(validate_deposit_network_config_status)
        .transpose()?
        .unwrap_or_else(|| "active".to_owned());
    let sort_order = request.sort_order.unwrap_or(0);
    let pool = admin_mysql_pool(pool)?;
    ensure_asset_symbols_exist(&pool, &asset_symbols).await?;

    // 网络配置写入与审计同事务提交，避免充值地址池读取到无审计的配置变更。
    let mut tx = pool.begin().await?;
    let config_id = insert_admin_deposit_network_config_in_tx(
        &mut tx,
        AdminDepositNetworkConfigWrite {
            network,
            display_name,
            address_group_code,
            address_group_name,
            asset_symbols,
            status,
            sort_order,
        },
    )
    .await?;
    let created = load_deposit_network_config_in_tx(&mut tx, config_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "deposit_network_config.create",
            target_type: "deposit_network_config",
            target_id: created.id,
            before_json: None,
            after_json: Some(deposit_network_config_audit_json(&created)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(created)
}

pub(crate) async fn update_admin_deposit_network_config(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    config_id: u64,
    request: UpdateDepositNetworkConfigRequest,
) -> AppResult<AdminDepositNetworkConfigResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let network = normalize_deposit_network(&request.network)?;
    let display_name = validate_deposit_network_display_name(&request.display_name)?;
    let address_group_code = validate_address_group_code(&request.address_group_code)?;
    let address_group_name =
        validate_optional_length(request.address_group_name, "address_group_name", 128)?;
    let asset_symbols = normalize_deposit_asset_symbols(None, request.asset_symbols)?;
    let status = validate_deposit_network_config_status(&request.status)?;
    let pool = admin_mysql_pool(pool)?;
    ensure_asset_symbols_exist(&pool, &asset_symbols).await?;

    // 先锁定旧网络配置再更新，确保充值网络配置审计 before/after 对应同一次事务。
    let mut tx = pool.begin().await?;
    let before = lock_deposit_network_config_in_tx(&mut tx, config_id).await?;
    update_admin_deposit_network_config_in_tx(
        &mut tx,
        config_id,
        AdminDepositNetworkConfigWrite {
            network,
            display_name,
            address_group_code,
            address_group_name,
            asset_symbols,
            status,
            sort_order: request.sort_order,
        },
    )
    .await?;
    let after = load_deposit_network_config_in_tx(&mut tx, config_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "deposit_network_config.update",
            target_type: "deposit_network_config",
            target_id: after.id,
            before_json: Some(deposit_network_config_audit_json(&before)),
            after_json: Some(deposit_network_config_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn list_admin_deposit_address_pool(
    pool: Option<Pool<MySql>>,
    query: AdminDepositAddressPoolQuery,
) -> AppResult<AdminDepositAddressPoolResponseList> {
    let network = query
        .network
        .and_then(optional_string)
        .map(|value| normalize_deposit_network(&value))
        .transpose()?;
    let address_group_code = query
        .address_group_code
        .and_then(optional_string)
        .map(|value| validate_address_group_code(&value))
        .transpose()?;
    let status = query
        .status
        .and_then(optional_string)
        .map(|value| validate_deposit_address_status(&value))
        .transpose()?;
    let asset_symbol = query
        .asset_symbol
        .and_then(optional_string)
        .map(|value| normalize_asset_symbol(&value))
        .transpose()?;
    let address = query.address.and_then(optional_string);
    let pool = admin_mysql_pool(pool)?;
    let addresses = list_admin_deposit_address_pool_from_store(
        &pool,
        AdminDepositAddressPoolListFilter {
            network,
            address_group_code,
            status,
            asset_symbol,
            assigned_user_id: query.assigned_user_id,
            email: query.email,
            address,
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(AdminDepositAddressPoolResponseList { addresses })
}

pub(crate) async fn get_admin_deposit_address_pool(
    pool: Option<Pool<MySql>>,
    address_id: u64,
) -> AppResult<AdminDepositAddressPoolResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_deposit_address_pool(&pool, address_id).await
}

pub(crate) async fn create_admin_deposit_address_pool(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateDepositAddressPoolRequest,
) -> AppResult<AdminDepositAddressPoolResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let network = normalize_deposit_network(&request.network)?;
    let address = validate_deposit_address(&request.address)?;
    let asset_symbols =
        normalize_deposit_asset_symbols(request.asset_symbol, request.asset_symbols)?;
    let status = request
        .status
        .as_deref()
        .map(validate_deposit_address_assignable_status)
        .transpose()?
        .unwrap_or_else(|| "available".to_owned());
    let memo = validate_optional_length(request.memo, "memo", 255)?;
    let remark = validate_optional_length(request.remark, "remark", 512)?;
    let pool = admin_mysql_pool(pool)?;
    ensure_asset_symbols_exist(&pool, &asset_symbols).await?;
    let network_config = load_deposit_network_config_by_network(&pool, &network).await?;
    ensure_deposit_asset_symbols_allowed_by_network(&asset_symbols, &network_config)?;
    let address_group_code =
        resolve_deposit_address_group_code(request.address_group_code, &network_config)?;

    // 地址入池和审计同事务提交，确保后台地址池变更可追踪。
    let mut tx = pool.begin().await?;
    let created = insert_deposit_address_pool_in_tx(
        &mut tx,
        AdminDepositAddressPoolWrite {
            network,
            address_group_code,
            address,
            asset_symbols,
            status,
            memo,
            remark,
        },
    )
    .await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "deposit_address_pool.create",
            target_type: "deposit_address_pool",
            target_id: created.id,
            before_json: None,
            after_json: Some(deposit_address_pool_audit_json(&created)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(created)
}

pub(crate) async fn create_admin_deposit_address_pool_batch(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateDepositAddressPoolBatchRequest,
) -> AppResult<AdminDepositAddressPoolBatchResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let network = normalize_deposit_network(&request.network)?;
    let asset_symbols =
        normalize_deposit_asset_symbols(request.asset_symbol, request.asset_symbols)?;
    let status = request
        .status
        .as_deref()
        .map(validate_deposit_address_assignable_status)
        .transpose()?
        .unwrap_or_else(|| "available".to_owned());
    let entries = normalize_deposit_address_batch_entries(request.entries)?;
    let pool = admin_mysql_pool(pool)?;
    ensure_asset_symbols_exist(&pool, &asset_symbols).await?;
    let network_config = load_deposit_network_config_by_network(&pool, &network).await?;
    ensure_deposit_asset_symbols_allowed_by_network(&asset_symbols, &network_config)?;
    let address_group_code =
        resolve_deposit_address_group_code(request.address_group_code, &network_config)?;

    // 批量入池逐条写审计，保持每个地址都有独立后台操作轨迹。
    let mut tx = pool.begin().await?;
    let mut addresses = Vec::with_capacity(entries.len());
    for entry in entries {
        let created = insert_deposit_address_pool_in_tx(
            &mut tx,
            AdminDepositAddressPoolWrite {
                network: network.clone(),
                address_group_code: address_group_code.clone(),
                address: entry.address,
                asset_symbols: asset_symbols.clone(),
                status: status.clone(),
                memo: entry.memo,
                remark: entry.remark,
            },
        )
        .await?;
        insert_admin_audit_log_entry_in_tx(
            &mut tx,
            admin_id,
            AdminAuditLogEntry {
                action: "deposit_address_pool.create",
                target_type: "deposit_address_pool",
                target_id: created.id,
                before_json: None,
                after_json: Some(deposit_address_pool_audit_json(&created)),
                reason: Some(reason.clone()),
            },
        )
        .await?;
        addresses.push(created);
    }
    tx.commit().await?;
    Ok(AdminDepositAddressPoolBatchResponse { addresses })
}

pub(crate) async fn update_admin_deposit_address_pool(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    address_id: u64,
    request: UpdateDepositAddressPoolRequest,
) -> AppResult<AdminDepositAddressPoolResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let network = normalize_deposit_network(&request.network)?;
    let address = validate_deposit_address(&request.address)?;
    let asset_symbols =
        normalize_deposit_asset_symbols(request.asset_symbol, request.asset_symbols)?;
    let status = validate_deposit_address_assignable_status(&request.status)?;
    let memo = validate_optional_length(request.memo, "memo", 255)?;
    let remark = validate_optional_length(request.remark, "remark", 512)?;
    let pool = admin_mysql_pool(pool)?;
    ensure_asset_symbols_exist(&pool, &asset_symbols).await?;
    let network_config = load_deposit_network_config_by_network(&pool, &network).await?;
    ensure_deposit_asset_symbols_allowed_by_network(&asset_symbols, &network_config)?;
    let address_group_code =
        resolve_deposit_address_group_code(request.address_group_code, &network_config)?;

    // 已分配地址必须先回收再编辑，避免用户充值地址被后台直接改写。
    let mut tx = pool.begin().await?;
    let before = lock_deposit_address_pool_in_tx(&mut tx, address_id).await?;
    if before.status == "assigned" {
        return Err(AppError::Validation(
            "assigned deposit address must be reclaimed before editing".to_owned(),
        ));
    }
    update_deposit_address_pool_in_tx(
        &mut tx,
        address_id,
        AdminDepositAddressPoolWrite {
            network,
            address_group_code,
            address,
            asset_symbols,
            status,
            memo,
            remark,
        },
    )
    .await?;
    let after = load_deposit_address_pool_in_tx(&mut tx, address_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "deposit_address_pool.update",
            target_type: "deposit_address_pool",
            target_id: after.id,
            before_json: Some(deposit_address_pool_audit_json(&before)),
            after_json: Some(deposit_address_pool_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn reclaim_admin_deposit_address_pool(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    address_id: u64,
    request: ReclaimDepositAddressPoolRequest,
) -> AppResult<AdminDepositAddressPoolResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let pool = admin_mysql_pool(pool)?;

    // 回收操作只清分配字段，不改地址自身配置，保证地址可重新进入可分配池。
    let mut tx = pool.begin().await?;
    let before = lock_deposit_address_pool_in_tx(&mut tx, address_id).await?;
    if before.status != "assigned" {
        return Err(AppError::Validation(
            "only assigned deposit address can be reclaimed".to_owned(),
        ));
    }
    reclaim_deposit_address_pool_in_tx(&mut tx, address_id).await?;
    let after = load_deposit_address_pool_in_tx(&mut tx, address_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "deposit_address_pool.reclaim",
            target_type: "deposit_address_pool",
            target_id: after.id,
            before_json: Some(deposit_address_pool_audit_json(&before)),
            after_json: Some(deposit_address_pool_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}
