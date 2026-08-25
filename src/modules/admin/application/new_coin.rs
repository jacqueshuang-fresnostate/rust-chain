//! 新币项目治理、认购派发查询与新币兑换规则的应用用例层。
//!
//! 只读用例分「按项目维度」和「全站扁平维度」两套，前者从路径补齐项目编号后复用同一批实现。
//! 项目配置类写用例共享一套编排：锁定项目、改配置、回读、同时写生命周期事件与后台审计、提交，
//! 因此每次改动都会在项目自身时间线和管理员操作留痕两处各留一条记录。
//! 派发是本文件唯一动用户资产的用例，它以请求幂等键防重复发币，并按项目解锁规则决定直接入账还是转锁仓。

use super::*;
use crate::modules::new_coin::service::{
    ensure_new_coin_amount_precision, ensure_unlock_fee_asset_matches_quote_asset,
};

async fn lock_new_coin_asset_precisions_in_order(
    tx: &mut Transaction<'_, MySql>,
    asset_ids: impl IntoIterator<Item = u64>,
) -> AppResult<Vec<(u64, i32)>> {
    let mut asset_ids: Vec<_> = asset_ids.into_iter().collect();
    asset_ids.sort_unstable();
    asset_ids.dedup();
    let mut precisions = Vec::with_capacity(asset_ids.len());
    for asset_id in asset_ids {
        let precision =
            crate::modules::admin::infrastructure::load_active_new_coin_asset_precision_in_tx(
                tx, asset_id,
            )
            .await?;
        precisions.push((asset_id, precision));
    }
    Ok(precisions)
}

fn new_coin_asset_precision(precisions: &[(u64, i32)], asset_id: u64) -> AppResult<i32> {
    precisions
        .iter()
        .find_map(|(id, precision)| (*id == asset_id).then_some(*precision))
        .ok_or_else(|| AppError::Internal("new coin asset precision lock is missing".to_owned()))
}

fn ensure_admin_new_coin_supply_invariant(project: &NewCoinProjectResponse) -> AppResult<()> {
    let zero = BigDecimal::from(0);
    if project.total_supply <= zero
        || project.reserved_supply < zero
        || project.allocated_supply < zero
        || project.remaining_supply < zero
        || (project.reserved_supply.clone()
            + project.allocated_supply.clone()
            + project.remaining_supply.clone())
        .normalized()
            != project.total_supply.normalized()
    {
        return Err(AppError::Internal(
            "new coin project supply accounting invariant is broken".to_owned(),
        ));
    }
    Ok(())
}

/// 确认新币项目仍处于启用状态，避免后台选择器加载后项目被并发停用仍继续写配置或派发资产。
/// 调用方必须传入事务内锁定或回读的项目快照；非 active 项目统一返回校验错误且不产生事件或审计。
fn ensure_active_new_coin_project(project: &NewCoinProjectResponse) -> AppResult<()> {
    if project.status != "active" {
        return Err(AppError::Validation(
            "new coin project must be active".to_owned(),
        ));
    }
    Ok(())
}

/// 确认派发接收用户仍可参与新操作；暂停、封禁等非 active 用户不得接收后台新币派发。
/// 用户快照在同一事务中于用户行锁之后读取，因此检查结果保持到本次派发提交或回滚。
fn ensure_active_distribution_user(user: &AdminUserResponse) -> AppResult<()> {
    if user.status != "active" {
        return Err(AppError::Validation(
            "new coin distribution user must be active".to_owned(),
        ));
    }
    Ok(())
}

/// 分页读取新币项目的发行、生命周期、解锁、手续费和上市后购买配置，并返回总数。
/// 当前查询不提供业务筛选，只裁剪 limit/offset；读取不锁项目，也不聚合认购或派发金额。
pub(crate) async fn list_admin_new_coin_projects(
    pool: Option<Pool<MySql>>,
    query: AdminNewCoinProjectQuery,
) -> AppResult<NewCoinProjectsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let (projects, total) = list_admin_new_coin_projects_from_store(
        &pool,
        route_limit(query.limit),
        route_offset(query.offset),
    )
    .await?;
    Ok(NewCoinProjectsResponse { projects, total })
}

/// 按项目、用户、邮箱和状态筛选新币认购记录，并返回分页明细和匹配总数。
/// 状态只去除空白，分页统一裁剪；读取不锁认购记录或钱包，也不计算可派发数量。
/// 项目编号在此为可选条件，因此同一实现既服务全站检索也服务按项目检索，后者由上层先行补齐该字段。
/// 认购记录反映用户申购意向与冻结结果，是否已发币要另查派发记录，二者不在此关联。
pub(crate) async fn list_admin_new_coin_subscriptions(
    pool: Option<Pool<MySql>>,
    query: AdminNewCoinFlatListQuery,
) -> AppResult<NewCoinSubscriptionsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let (subscriptions, total) = list_admin_new_coin_subscriptions_from_store(
        &pool,
        AdminNewCoinFlatListFilter {
            project_id: query.project_id,
            user_id: query.user_id,
            email: query.email,
            status: query.status.and_then(optional_string),
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(NewCoinSubscriptionsResponse {
        subscriptions,
        total,
    })
}

/// 把按项目维度的查询参数升格为扁平查询参数，用路径中的项目编号覆盖项目筛选项。
/// 项目编号来自路径而非请求体，因此调用方无法借查询串越权访问其他项目的子资源。
/// 其余用户、邮箱、状态与分页字段原样透传，两套入口据此复用同一批列表实现。
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
        offset: query.offset,
    }
}

/// 按项目维度检索认购记录：先用路径项目编号覆盖筛选条件，再委托全站认购列表实现。
/// 该入口不校验项目是否存在，项目编号无效时得到的是空列表而不是未找到错误。
pub(crate) async fn list_admin_new_coin_subscriptions_for_project(
    pool: Option<Pool<MySql>>,
    project_id: u64,
    query: AdminNewCoinScopedListQuery,
) -> AppResult<NewCoinSubscriptionsResponse> {
    let query = build_new_coin_scoped_list_query(project_id, query);
    list_admin_new_coin_subscriptions(pool, query).await
}

/// 按项目、用户、邮箱和状态筛选新币派发记录，并返回数量、锁仓和幂等键信息及总数。
/// 查询不锁派发、钱包或锁仓行；分页边界统一裁剪，读取失败不会重试派发。
/// 响应里的幂等键是排查重复发币的关键线索，同一键至多对应一条记录，可据此确认某次派发是否已执行。
/// 锁仓头寸编号为空表示该笔已直接入账可用余额，非空则表示资产仍处于锁仓待解禁状态。
pub(crate) async fn list_admin_new_coin_distributions(
    pool: Option<Pool<MySql>>,
    query: AdminNewCoinFlatListQuery,
) -> AppResult<NewCoinDistributionsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let (distributions, total) = list_admin_new_coin_distributions_from_store(
        &pool,
        AdminNewCoinFlatListFilter {
            project_id: query.project_id,
            user_id: query.user_id,
            email: query.email,
            status: query.status.and_then(optional_string),
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(NewCoinDistributionsResponse {
        distributions,
        total,
    })
}

/// 按项目维度检索派发记录：路径项目编号覆盖筛选条件后委托全站派发列表实现。
/// 与认购的按项目入口同构，同样不校验项目存在性，编号无效时返回空列表而非未找到。
pub(crate) async fn list_admin_new_coin_distributions_for_project(
    pool: Option<Pool<MySql>>,
    project_id: u64,
    query: AdminNewCoinScopedListQuery,
) -> AppResult<NewCoinDistributionsResponse> {
    let query = build_new_coin_scoped_list_query(project_id, query);
    list_admin_new_coin_distributions(pool, query).await
}

/// 按项目、用户、邮箱和状态筛选上市后购买记录，并返回分页结果与匹配总数。
/// 状态仅去空白，分页统一裁剪；读取不锁交易对或订单，也不触发兑换结算。
/// 与认购记录属于新币生命周期的不同阶段：认购发生在上市之前，本入口统计的是开放二级购买之后的成交。
/// 只有开启了上市后购买的项目才会产生这类记录，因此未开放的项目查询结果恒为空。
pub(crate) async fn list_admin_new_coin_purchases(
    pool: Option<Pool<MySql>>,
    query: AdminNewCoinPurchaseQuery,
) -> AppResult<NewCoinPurchasesResponse> {
    let pool = admin_mysql_pool(pool)?;
    let (purchases, total) = list_admin_new_coin_purchases_from_store(
        &pool,
        AdminNewCoinFlatListFilter {
            project_id: query.project_id,
            user_id: query.user_id,
            email: query.email,
            status: query.status.and_then(optional_string),
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(NewCoinPurchasesResponse { purchases, total })
}

/// 按用户、邮箱、资产和状态筛选新币锁仓头寸，并返回解锁时间、剩余金额和来源信息及总数。
/// 查询不锁头寸或推进解锁；分页边界统一裁剪，数据库解码失败直接返回错误。
/// 与派发和解锁两类记录的区别在于这里是当前状态视图：剩余金额随解禁推进而减少，而非逐笔流水。
/// 筛选维度按用户与资产而不含项目编号，因为同一资产的锁仓可能来自多次派发并已按合并键归并。
pub(crate) async fn list_admin_new_coin_lock_positions(
    pool: Option<Pool<MySql>>,
    query: AdminNewCoinLockPositionQuery,
) -> AppResult<NewCoinLockPositionsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let (lock_positions, total) = list_admin_new_coin_lock_positions_from_store(
        &pool,
        AdminNewCoinLockPositionListFilter {
            user_id: query.user_id,
            email: query.email,
            asset_id: query.asset_id,
            status: query.status.and_then(optional_string),
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(NewCoinLockPositionsResponse {
        lock_positions,
        total,
    })
}

/// 按用户、邮箱、资产、解锁状态和费用支付状态筛选新币解锁记录，并返回分页结果与总数。
/// 两个状态筛选只去空白，查询不锁钱包或费用记录，也不执行解锁或补扣费用。
/// 比锁仓头寸多出的费用支付状态维度，正是用来定位「已解禁但解禁手续费尚未扣成功」这类需要人工跟进的记录。
/// 两个状态是彼此独立的筛选项，可单独或组合使用；未提供的项不参与过滤。
pub(crate) async fn list_admin_new_coin_unlocks(
    pool: Option<Pool<MySql>>,
    query: AdminNewCoinUnlockQuery,
) -> AppResult<NewCoinUnlocksResponse> {
    let pool = admin_mysql_pool(pool)?;
    let (unlocks, total) = list_admin_new_coin_unlocks_from_store(
        &pool,
        AdminNewCoinUnlockListFilter {
            user_id: query.user_id,
            email: query.email,
            asset_id: query.asset_id,
            status: query.status.and_then(optional_string),
            fee_paid_status: query.fee_paid_status.and_then(optional_string),
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(NewCoinUnlocksResponse { unlocks, total })
}

/// 创建新币项目，并返回含发行、生命周期、解锁和手续费配置的数据库快照。
/// 请求须含合法生命周期、正总量、非负发行价、非空符号及互斥解锁配置；调用方负责管理员权限和资产 ID 来源。
/// 事务插入项目、回读后依次写生命周期事件和后台审计；实现未预先锁资产，唯一键或任一步失败整体回滚。
/// 本用例无幂等键，提交后不自动开放认购、派发资产或发布外部事件。
pub(crate) async fn create_admin_new_coin_project(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateNewCoinProjectRequest,
) -> AppResult<NewCoinProjectResponse> {
    validate_create_new_coin_project(&request)?;
    let pool = admin_mysql_pool(pool)?;

    // 新币项目创建、生命周期事件和后台审计必须同事务提交，避免项目已开放但缺少追踪记录。
    let mut tx = pool.begin().await?;
    let unlock_fee_asset = request
        .unlock_fee_enabled
        .unwrap_or(false)
        .then_some(request.unlock_fee_asset)
        .flatten();
    let mut asset_ids = vec![request.asset_id, request.quote_asset_id];
    asset_ids.extend(unlock_fee_asset);
    let precisions = lock_new_coin_asset_precisions_in_order(&mut tx, asset_ids).await?;
    ensure_new_coin_amount_precision(
        &request.total_supply,
        new_coin_asset_precision(&precisions, request.asset_id)?,
        "total_supply",
    )?;
    ensure_new_coin_amount_precision(
        &request.issue_price,
        new_coin_asset_precision(&precisions, request.quote_asset_id)?,
        "issue_price",
    )?;
    let project_id = insert_admin_new_coin_project_in_tx(
        &mut tx,
        AdminNewCoinProjectInsert {
            asset_id: request.asset_id,
            symbol: request.symbol.trim().to_owned(),
            lifecycle_status: request.lifecycle_status.trim().to_owned(),
            total_supply: request.total_supply,
            issue_price: request.issue_price,
            quote_asset_id: request.quote_asset_id,
            listed_at: request.listed_at,
            unlock_type: request.unlock_type.trim().to_owned(),
            fixed_unlock_at: request.fixed_unlock_at,
            relative_unlock_seconds: request.relative_unlock_seconds,
            unlock_fee_enabled: request.unlock_fee_enabled.unwrap_or(false),
            unlock_fee_rate: unlock_fee_asset
                .is_some()
                .then_some(request.unlock_fee_rate)
                .flatten(),
            unlock_fee_basis: if unlock_fee_asset.is_some() {
                request
                    .unlock_fee_basis
                    .as_deref()
                    .map(str::trim)
                    .map(str::to_owned)
            } else {
                None
            },
            unlock_fee_asset,
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

/// 按领域迁移图推进新币项目生命周期，并返回更新后的项目快照。
/// 请求目标只接受预热、认购、派发或上市；管理员权限由调用方保证，进入上市时缺省使用当前时间作为 listed_at。
/// 事务先锁项目，基于锁后旧状态校验迁移，再更新生命周期、回读并写生命周期事件和后台审计；非法迁移或数据库失败整体回滚。
/// 相同目标重放通常因迁移图返回错误，不会重复推进；本用例不触发自动派发或交易对上线。
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
    ensure_active_new_coin_project(&before)?;
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

/// 替换新币项目的解锁模式与对应时间参数，并返回最终项目配置。
/// 请求须满足上市即解锁、固定时间、相对周期三种字段形状；切换到非上市即解锁时保留项目原 listed_at。
/// 事务先锁项目，再更新规则、回读并写生命周期事件和后台审计；记录缺失或任一步失败整体回滚。
/// 已生成的锁仓头寸不会被重算；相同配置重放仍新增事件和审计。
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
    ensure_active_new_coin_project(&before)?;
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

/// 更新新币项目的解禁费用规则，并返回事务内回读的项目快照。
/// 调用方须已完成管理员鉴权；费率、计费依据和费用资产组合必须先通过规则校验。
/// 事务先锁定项目，再更新规则并写生命周期及后台审计；关闭费用时必须清空全部旧计费字段。
/// 每次成功调用都会产生审计记录；任一步失败均回滚，不留下半启用的收费配置。
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
    ensure_active_new_coin_project(&before)?;
    ensure_unlock_fee_asset_matches_quote_asset(
        request.unlock_fee_enabled,
        request.unlock_fee_asset,
        before.quote_asset_id,
    )?;
    if let Some(unlock_fee_asset) = request
        .unlock_fee_enabled
        .then_some(request.unlock_fee_asset)
        .flatten()
    {
        load_active_asset_symbol_in_tx(&mut tx, unlock_fee_asset).await?;
    }
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

/// 为已上市新币启用指定购买交易对或关闭上市后购买，并返回项目最终配置。
/// 启用时须提供交易对 ID；事务锁项目并确认生命周期为 listed，再校验交易对关联项目资产，随后激活交易对和项目开关。
/// 关闭路径只清除项目购买开关；项目写入、可选交易对激活、生命周期事件和后台审计同事务提交，失败整体回滚。
/// 重放启用/关闭仍会产生审计，关闭不会恢复此前被激活交易对的状态，也不发布外部行情事件。
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
    ensure_active_new_coin_project(&before)?;
    ensure_post_listing_purchase_lifecycle(&before)?;
    if request.enabled {
        let pair_id = request.pair_id.ok_or_else(|| {
            AppError::Validation(
                "pair_id is required when post-listing purchase is enabled".to_owned(),
            )
        })?;
        ensure_admin_new_coin_post_listing_pair_in_tx(
            &mut tx,
            pair_id,
            before.asset_id,
            before.quote_asset_id.ok_or_else(|| {
                AppError::Validation("new coin project quote asset is not configured".to_owned())
            })?,
        )
        .await?;
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

/// 锁定新币项目与派发记录，按认购结果计算直接入账或锁仓分配，并推进项目派发状态。
/// 派发、钱包余额流水、锁仓、生命周期事件及审计共用事务；幂等键或状态冲突阻止重复发币。
/// 事务顺序为：锁项目行、确认生命周期处于派发阶段、按幂等键查重、可选核销认购额度、
/// 按解锁规则算出锁仓计划并完成入账或锁仓、写派发记录、写生命周期事件与后台审计。
/// 幂等键已存在时直接返回冲突而不是回读既有结果，因此调用方需自行查派发记录确认上次是否成功。
/// 认购编号为可选：提供时会核销对应认购的可派额度，不提供则视为不基于认购的直接空投。
/// 派发状态由是否产生锁仓头寸决定，有锁仓记为锁定中，无锁仓记为已完成。
/// 本用例不发布任何外部事件，资产可用性完全由事务内的余额与锁仓写入体现。
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
    ensure_active_new_coin_project(&project)?;
    ensure_distribution_lifecycle(&project)?;
    ensure_admin_new_coin_supply_invariant(&project)?;
    ensure_unlock_fee_asset_matches_quote_asset(
        project.unlock_fee_enabled,
        project.unlock_fee_asset,
        project.quote_asset_id,
    )?;
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
    let mut asset_ids = vec![project.asset_id];
    if project.unlock_fee_enabled {
        asset_ids.extend(project.unlock_fee_asset);
    }
    let precisions = lock_new_coin_asset_precisions_in_order(&mut tx, asset_ids).await?;
    let project_asset_precision = new_coin_asset_precision(&precisions, project.asset_id)?;
    for (field, amount) in [
        ("total_supply", &project.total_supply),
        ("reserved_supply", &project.reserved_supply),
        ("allocated_supply", &project.allocated_supply),
        ("remaining_supply", &project.remaining_supply),
    ] {
        ensure_new_coin_amount_precision(amount, project_asset_precision, field)?;
    }
    ensure_new_coin_amount_precision(&request.quantity, project_asset_precision, "quantity")?;
    let unlock_fee_precision = if project.unlock_fee_enabled {
        project
            .unlock_fee_asset
            .map(|asset_id| new_coin_asset_precision(&precisions, asset_id))
            .transpose()?
    } else {
        None
    };
    reserve_admin_new_coin_supply_in_tx(&mut tx, project_id, &request.quantity).await?;
    ensure_admin_user_exists_in_tx(&mut tx, request.user_id).await?;
    let distribution_user = load_admin_user_in_tx(&mut tx, request.user_id).await?;
    ensure_active_distribution_user(&distribution_user)?;
    let purchase_cost = if let Some(subscription_id) = request.subscription_id {
        apply_admin_new_coin_subscription_distribution_in_tx(
            &mut tx,
            subscription_id,
            project_id,
            request.user_id,
            &request.quantity,
        )
        .await?
    } else {
        BigDecimal::from(0)
    };

    lock_admin_new_coin_distribution_wallet_in_tx(&mut tx, request.user_id, project.asset_id)
        .await?;
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
        &project,
        &purchase_cost,
        unlock_fee_precision,
        AdminNewCoinLedgerWrite {
            change_type: "new_coin_distribution_lock",
            ref_type: "new_coin_distribution",
            ref_id: &idempotency_key,
        },
    )
    .await?;
    finalize_admin_new_coin_supply_in_tx(&mut tx, project_id, &request.quantity).await?;
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

/// 按换币交易对创建或更新唯一的新币兑换规则，并返回最终规则快照。
/// 调用方须已完成管理员鉴权；费率来源、固定/浮动配置和状态必须先通过业务校验。
/// 事务先按交易对锁定现有规则，再选择插入或更新，并将前后值写入后台审计后提交。
/// 数据库唯一关系防止同一交易对出现多条规则；重试会走更新分支，但仍新增一次审计。
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

/// 在调用方事务内把一次项目配置变更同时写进生命周期事件流和后台审计日志。
/// 生命周期推进、解锁规则替换、解禁费规则调整与上市后购买开关四条路径共用本函数，仅传入的动作名不同。
/// 事件载荷把前后值包进一个对象，审计则分列 before 与 after 两字段，内容同源但结构不同，
/// 前者服务于按项目回溯时间线，后者服务于按管理员追溯操作。
/// 本函数不提交也不回滚，失败直接上抛，由调用方统一回滚整笔变更。
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
