use super::*;

pub(crate) async fn list_admin_new_coin_projects(
    pool: Option<Pool<MySql>>,
    query: AdminNewCoinProjectQuery,
) -> AppResult<NewCoinProjectsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let projects = list_admin_new_coin_projects_from_store(&pool, route_limit(query.limit)).await?;
    Ok(NewCoinProjectsResponse { projects })
}

pub(crate) async fn list_admin_new_coin_subscriptions(
    pool: Option<Pool<MySql>>,
    query: AdminNewCoinFlatListQuery,
) -> AppResult<NewCoinSubscriptionsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let subscriptions = list_admin_new_coin_subscriptions_from_store(
        &pool,
        AdminNewCoinFlatListFilter {
            project_id: query.project_id,
            user_id: query.user_id,
            email: query.email,
            status: query.status.and_then(optional_string),
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(NewCoinSubscriptionsResponse { subscriptions })
}

/// 组装项目过滤列表参数：由路由层传入的子查询参数统一补齐项目ID。
pub(super) fn build_new_coin_scoped_list_query(
    project_id: u64,
    query: AdminNewCoinScopedListQuery,
) -> AdminNewCoinFlatListQuery {
    AdminNewCoinFlatListQuery {
        project_id: Some(project_id),
        user_id: query.user_id,
        email: query.email,
        status: query.status,
        limit: query.limit,
    }
}

/// 查询某个项目的认购列表。
pub(crate) async fn list_admin_new_coin_subscriptions_for_project(
    pool: Option<Pool<MySql>>,
    project_id: u64,
    query: AdminNewCoinScopedListQuery,
) -> AppResult<NewCoinSubscriptionsResponse> {
    let query = build_new_coin_scoped_list_query(project_id, query);
    list_admin_new_coin_subscriptions(pool, query).await
}

pub(crate) async fn list_admin_new_coin_distributions(
    pool: Option<Pool<MySql>>,
    query: AdminNewCoinFlatListQuery,
) -> AppResult<NewCoinDistributionsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let distributions = list_admin_new_coin_distributions_from_store(
        &pool,
        AdminNewCoinFlatListFilter {
            project_id: query.project_id,
            user_id: query.user_id,
            email: query.email,
            status: query.status.and_then(optional_string),
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(NewCoinDistributionsResponse { distributions })
}

/// 查询某个项目的分配列表。
pub(crate) async fn list_admin_new_coin_distributions_for_project(
    pool: Option<Pool<MySql>>,
    project_id: u64,
    query: AdminNewCoinScopedListQuery,
) -> AppResult<NewCoinDistributionsResponse> {
    let query = build_new_coin_scoped_list_query(project_id, query);
    list_admin_new_coin_distributions(pool, query).await
}

pub(crate) async fn list_admin_new_coin_purchases(
    pool: Option<Pool<MySql>>,
    query: AdminNewCoinPurchaseQuery,
) -> AppResult<NewCoinPurchasesResponse> {
    let pool = admin_mysql_pool(pool)?;
    let purchases = list_admin_new_coin_purchases_from_store(
        &pool,
        AdminNewCoinFlatListFilter {
            project_id: query.project_id,
            user_id: query.user_id,
            email: query.email,
            status: query.status.and_then(optional_string),
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(NewCoinPurchasesResponse { purchases })
}

pub(crate) async fn list_admin_new_coin_lock_positions(
    pool: Option<Pool<MySql>>,
    query: AdminNewCoinLockPositionQuery,
) -> AppResult<NewCoinLockPositionsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let lock_positions = list_admin_new_coin_lock_positions_from_store(
        &pool,
        AdminNewCoinLockPositionListFilter {
            user_id: query.user_id,
            email: query.email,
            asset_id: query.asset_id,
            status: query.status.and_then(optional_string),
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(NewCoinLockPositionsResponse { lock_positions })
}

pub(crate) async fn list_admin_new_coin_unlocks(
    pool: Option<Pool<MySql>>,
    query: AdminNewCoinUnlockQuery,
) -> AppResult<NewCoinUnlocksResponse> {
    let pool = admin_mysql_pool(pool)?;
    let unlocks = list_admin_new_coin_unlocks_from_store(
        &pool,
        AdminNewCoinUnlockListFilter {
            user_id: query.user_id,
            email: query.email,
            asset_id: query.asset_id,
            status: query.status.and_then(optional_string),
            fee_paid_status: query.fee_paid_status.and_then(optional_string),
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(NewCoinUnlocksResponse { unlocks })
}

pub(crate) async fn create_admin_new_coin_project(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateNewCoinProjectRequest,
) -> AppResult<NewCoinProjectResponse> {
    validate_create_new_coin_project(&request)?;
    let pool = admin_mysql_pool(pool)?;

    // 新币项目创建、生命周期事件和后台审计必须同事务提交，避免项目已开放但缺少追踪记录。
    let mut tx = pool.begin().await?;
    let project_id = insert_admin_new_coin_project_in_tx(
        &mut tx,
        AdminNewCoinProjectInsert {
            asset_id: request.asset_id,
            symbol: request.symbol.trim().to_owned(),
            lifecycle_status: request.lifecycle_status.trim().to_owned(),
            total_supply: request.total_supply,
            issue_price: request.issue_price,
            listed_at: request.listed_at,
            unlock_type: request.unlock_type.trim().to_owned(),
            fixed_unlock_at: request.fixed_unlock_at,
            relative_unlock_seconds: request.relative_unlock_seconds,
            unlock_fee_enabled: request.unlock_fee_enabled.unwrap_or(false),
            unlock_fee_rate: request.unlock_fee_rate,
            unlock_fee_basis: request
                .unlock_fee_basis
                .as_deref()
                .map(str::trim)
                .map(str::to_owned),
            unlock_fee_asset: request.unlock_fee_asset,
        },
    )
    .await?;
    let project = load_admin_new_coin_project_in_tx(&mut tx, project_id).await?;
    let event_payload = new_coin_project_audit_json(&project);
    insert_admin_new_coin_lifecycle_event_in_tx(
        &mut tx,
        project.id,
        "new_coin_project.create",
        event_payload.clone(),
        admin_id,
    )
    .await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "new_coin_project.create",
            target_type: "new_coin_project",
            target_id: project.id,
            before_json: None,
            after_json: Some(event_payload),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(project)
}

pub(crate) async fn update_admin_new_coin_lifecycle(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    project_id: u64,
    request: UpdateNewCoinLifecycleRequest,
) -> AppResult<NewCoinProjectResponse> {
    let target_status = parse_lifecycle_status_from_request(&request.lifecycle_status)?;
    let pool = admin_mysql_pool(pool)?;

    // 生命周期流转必须先锁定项目行，再校验当前状态到目标状态的单向流转规则。
    let mut tx = pool.begin().await?;
    let before = lock_admin_new_coin_project_in_tx(&mut tx, project_id).await?;
    let current_status = parse_lifecycle_status_from_db(&before.lifecycle_status)?;
    current_status
        .transition_to(target_status)
        .map_err(|_| AppError::Validation("invalid new coin lifecycle transition".to_owned()))?;
    let listed_at = if target_status == LifecycleStatus::Listed {
        Some(request.listed_at.unwrap_or_else(Utc::now))
    } else {
        before.listed_at
    };
    update_admin_new_coin_project_lifecycle_in_tx(
        &mut tx,
        project_id,
        lifecycle_status_value(target_status),
        listed_at,
    )
    .await?;
    let after = load_admin_new_coin_project_in_tx(&mut tx, project_id).await?;
    record_admin_new_coin_project_change_in_tx(
        &mut tx,
        admin_id,
        project_id,
        "new_coin_project.lifecycle.update",
        &before,
        &after,
        request.reason,
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn update_admin_new_coin_unlock_rule(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    project_id: u64,
    request: UpdateNewCoinUnlockRuleRequest,
) -> AppResult<NewCoinProjectResponse> {
    validate_update_new_coin_unlock_rule(&request)?;
    let pool = admin_mysql_pool(pool)?;

    // 锁定项目后再更新规则，避免后台并发修改导致审计 before/after 失真。
    let mut tx = pool.begin().await?;
    let before = lock_admin_new_coin_project_in_tx(&mut tx, project_id).await?;
    let unlock_type = request.unlock_type.trim().to_owned();
    let listed_at = if unlock_type == "immediate_on_listing" {
        request.listed_at
    } else {
        before.listed_at
    };
    update_admin_new_coin_project_unlock_rule_in_tx(
        &mut tx,
        project_id,
        AdminNewCoinUnlockRuleUpdate {
            unlock_type,
            listed_at,
            fixed_unlock_at: request.fixed_unlock_at,
            relative_unlock_seconds: request.relative_unlock_seconds,
        },
    )
    .await?;
    let after = load_admin_new_coin_project_in_tx(&mut tx, project_id).await?;
    record_admin_new_coin_project_change_in_tx(
        &mut tx,
        admin_id,
        project_id,
        "new_coin_project.unlock_rule.update",
        &before,
        &after,
        request.reason,
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn update_admin_new_coin_unlock_fee_rule(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    project_id: u64,
    request: UpdateNewCoinUnlockFeeRuleRequest,
) -> AppResult<NewCoinProjectResponse> {
    validate_update_new_coin_unlock_fee_rule(&request)?;
    let pool = admin_mysql_pool(pool)?;

    // 矿工费关闭时同步清空费率、计费依据和费用资产，避免旧配置被后续解禁误用。
    let mut tx = pool.begin().await?;
    let before = lock_admin_new_coin_project_in_tx(&mut tx, project_id).await?;
    update_admin_new_coin_project_unlock_fee_rule_in_tx(
        &mut tx,
        project_id,
        AdminNewCoinUnlockFeeRuleUpdate {
            unlock_fee_enabled: request.unlock_fee_enabled,
            unlock_fee_rate: request
                .unlock_fee_enabled
                .then_some(request.unlock_fee_rate)
                .flatten(),
            unlock_fee_basis: if request.unlock_fee_enabled {
                request
                    .unlock_fee_basis
                    .as_deref()
                    .map(str::trim)
                    .map(str::to_owned)
            } else {
                None
            },
            unlock_fee_asset: request
                .unlock_fee_enabled
                .then_some(request.unlock_fee_asset)
                .flatten(),
        },
    )
    .await?;
    let after = load_admin_new_coin_project_in_tx(&mut tx, project_id).await?;
    record_admin_new_coin_project_change_in_tx(
        &mut tx,
        admin_id,
        project_id,
        "new_coin_project.unlock_fee_rule.update",
        &before,
        &after,
        request.reason,
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn update_admin_new_coin_post_listing_purchase(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    project_id: u64,
    request: UpdateNewCoinPostListingPurchaseRequest,
) -> AppResult<NewCoinProjectResponse> {
    validate_update_new_coin_post_listing_purchase(&request)?;
    let pool = admin_mysql_pool(pool)?;

    // 锁定新币项目和目标交易对，确保认购开关、交易对启用和审计一致提交。
    let mut tx = pool.begin().await?;
    let before = lock_admin_new_coin_project_in_tx(&mut tx, project_id).await?;
    ensure_post_listing_purchase_lifecycle(&before)?;
    if request.enabled {
        let pair_id = request.pair_id.ok_or_else(|| {
            AppError::Validation(
                "pair_id is required when post-listing purchase is enabled".to_owned(),
            )
        })?;
        ensure_admin_new_coin_post_listing_pair_in_tx(&mut tx, pair_id, before.asset_id).await?;
        activate_admin_new_coin_post_listing_pair_in_tx(&mut tx, pair_id).await?;
        enable_admin_new_coin_post_listing_purchase_in_tx(&mut tx, project_id, pair_id).await?;
    } else {
        disable_admin_new_coin_post_listing_purchase_in_tx(&mut tx, project_id).await?;
    }
    let after = load_admin_new_coin_project_in_tx(&mut tx, project_id).await?;
    record_admin_new_coin_project_change_in_tx(
        &mut tx,
        admin_id,
        project_id,
        "new_coin_project.post_listing_purchase.update",
        &before,
        &after,
        request.reason,
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn distribute_admin_new_coin(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    project_id: u64,
    request: DistributeNewCoinRequest,
) -> AppResult<NewCoinDistributionResponse> {
    validate_distribute_new_coin(&request)?;
    let idempotency_key = request.idempotency_key.trim().to_owned();
    let pool = admin_mysql_pool(pool)?;

    // 派发会同时影响申购单、钱包余额、锁仓明细、生命周期事件和后台审计，必须放入同一事务。
    let mut tx = pool.begin().await?;
    let project = lock_admin_new_coin_project_in_tx(&mut tx, project_id).await?;
    ensure_distribution_lifecycle(&project)?;
    if admin_new_coin_idempotency_key_exists_in_tx(
        &mut tx,
        "new_coin_distributions",
        &idempotency_key,
    )
    .await?
    {
        return Err(AppError::Conflict(
            "new coin distribution has already been created".to_owned(),
        ));
    }
    if let Some(subscription_id) = request.subscription_id {
        apply_admin_new_coin_subscription_distribution_in_tx(
            &mut tx,
            subscription_id,
            project_id,
            request.user_id,
            &request.quantity,
        )
        .await?;
    }

    let source_time = Utc::now();
    let lock_positions = lock_positions_for_distribution(
        &project,
        request.user_id,
        project.asset_id,
        &idempotency_key,
        request.quantity.clone(),
        source_time,
    )?;
    let lock_position_id = apply_admin_new_coin_distribution_allocation_in_tx(
        &mut tx,
        request.user_id,
        project.asset_id,
        &request.quantity,
        &lock_positions,
        AdminNewCoinLedgerWrite {
            change_type: "new_coin_distribution_lock",
            ref_type: "new_coin_distribution",
            ref_id: &idempotency_key,
        },
    )
    .await?;
    let status = if lock_position_id.is_some() {
        "locked"
    } else {
        "completed"
    };
    let distribution_id = insert_admin_new_coin_distribution_in_tx(
        &mut tx,
        project_id,
        request.user_id,
        request.subscription_id,
        project.asset_id,
        &request.quantity,
        lock_position_id,
        status,
        &idempotency_key,
    )
    .await?;
    let distribution = load_admin_new_coin_distribution_in_tx(&mut tx, distribution_id).await?;
    let distribution_json = new_coin_distribution_audit_json(&distribution);
    insert_admin_new_coin_lifecycle_event_in_tx(
        &mut tx,
        project_id,
        "new_coin_distribution.create",
        json!({ "distribution": distribution_json.clone() }),
        admin_id,
    )
    .await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "new_coin_distribution.create",
            target_type: "new_coin_distribution",
            target_id: distribution.id,
            before_json: None,
            after_json: Some(distribution_json),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(distribution)
}

pub(crate) async fn upsert_admin_new_coin_convert_rule(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: UpsertNewCoinConvertRuleRequest,
) -> AppResult<NewCoinConvertRuleResponse> {
    validate_new_coin_convert_rule(&request)?;
    let status = request
        .status
        .clone()
        .and_then(optional_string)
        .unwrap_or_else(|| "active".to_owned());
    let pool = admin_mysql_pool(pool)?;

    // 同一 convert_pair 只允许一条新币兑换规则，先按 pair 锁定旧记录再 upsert。
    let mut tx = pool.begin().await?;
    let before = lock_admin_new_coin_convert_rule_in_tx(&mut tx, request.convert_pair_id).await?;
    let write = AdminNewCoinConvertRuleWrite {
        convert_pair_id: request.convert_pair_id,
        rate_source: request.rate_source.trim().to_owned(),
        fixed_rate: request.fixed_rate,
        floating_rate_json: request.floating_rate_json,
        status,
        admin_id,
    };
    let rule_id = if let Some(before) = before.as_ref() {
        update_admin_new_coin_convert_rule_in_tx(&mut tx, before.id, &write).await?;
        before.id
    } else {
        insert_admin_new_coin_convert_rule_in_tx(&mut tx, &write).await?
    };
    let after = load_admin_new_coin_convert_rule_in_tx(&mut tx, rule_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: if before.is_some() {
                "new_coin_convert_rule.update"
            } else {
                "new_coin_convert_rule.create"
            },
            target_type: "new_coin_convert_rule",
            target_id: after.id,
            before_json: before.as_ref().map(new_coin_convert_rule_audit_json),
            after_json: Some(new_coin_convert_rule_audit_json(&after)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

async fn record_admin_new_coin_project_change_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    project_id: u64,
    action: &'static str,
    before: &NewCoinProjectResponse,
    after: &NewCoinProjectResponse,
    reason: Option<String>,
) -> AppResult<()> {
    let before_json = new_coin_project_audit_json(before);
    let after_json = new_coin_project_audit_json(after);
    insert_admin_new_coin_lifecycle_event_in_tx(
        tx,
        project_id,
        action,
        json!({
            "before": before_json,
            "after": after_json,
        }),
        admin_id,
    )
    .await?;
    insert_admin_audit_log_entry_in_tx(
        tx,
        admin_id,
        AdminAuditLogEntry {
            action,
            target_type: "new_coin_project",
            target_id: project_id,
            before_json: Some(before_json),
            after_json: Some(after_json),
            reason,
        },
    )
    .await
}
