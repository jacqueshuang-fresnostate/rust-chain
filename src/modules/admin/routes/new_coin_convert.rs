use super::*;

/// 构建新币项目生命周期、分发/解锁查询与闪兑管理的后台传输路由。
///
/// 每个入口保持管理员鉴权及原有 DTO，写操作仅解析管理员审计主体并转发应用用例；
/// 分发、解锁、购后开放和闪兑对删除等事务与幂等规则不进入路由层。闪兑对删除成功
/// 仍返回 204，领域或持久化失败继续沿既有错误映射传播。
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/new-coins",
            get(list_new_coin_projects).post(create_new_coin_project),
        )
        .route("/new-coins/:id/lifecycle", patch(update_new_coin_lifecycle))
        .route("/new-coins/:id/distribute", post(distribute_new_coin))
        .route(
            "/new-coins/:id/unlock-rule",
            patch(update_new_coin_unlock_rule),
        )
        .route(
            "/new-coins/:id/post-listing-purchase",
            patch(update_new_coin_post_listing_purchase),
        )
        .route(
            "/new-coins/:id/unlock-fee-rule",
            patch(update_new_coin_unlock_fee_rule),
        )
        .route(
            "/new-coins/:id/subscriptions",
            get(list_new_coin_subscriptions),
        )
        .route(
            "/new-coins/:id/distributions",
            get(list_new_coin_distributions),
        )
        .route(
            "/new-coins/subscriptions",
            get(list_all_new_coin_subscriptions),
        )
        .route(
            "/new-coins/distributions",
            get(list_all_new_coin_distributions),
        )
        .route("/new-coins/purchases", get(list_new_coin_purchases))
        .route(
            "/new-coins/lock-positions",
            get(list_new_coin_lock_positions),
        )
        .route("/new-coins/unlocks", get(list_new_coin_unlocks))
        .route(
            "/convert/pairs",
            get(list_convert_pairs).post(create_convert_pair),
        )
        .route(
            "/convert/pairs/:id",
            get(get_convert_pair)
                .patch(update_convert_pair)
                .delete(delete_convert_pair),
        )
        .route(
            "/convert/new-coin-rules",
            post(upsert_new_coin_convert_rule),
        )
        .route("/convert/orders", get(list_convert_orders))
        .route("/convert/orders/:id", get(get_convert_order))
}

async fn list_new_coin_projects(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewCoinProjectQuery>,
) -> AppResult<Json<NewCoinProjectsResponse>> {
    Ok(Json(
        list_new_coin_projects_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn list_convert_pairs(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminConvertPairQuery>,
) -> AppResult<Json<ConvertPairsResponse>> {
    Ok(Json(
        list_convert_pairs_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn get_convert_pair(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(pair_id): Path<u64>,
) -> AppResult<Json<ConvertPairResponse>> {
    Ok(Json(
        get_convert_pair_use_case(state.mysql.clone(), pair_id).await?,
    ))
}

async fn list_convert_orders(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminConvertOrdersQuery>,
) -> AppResult<Json<ConvertOrdersResponse>> {
    Ok(Json(
        list_convert_orders_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn get_convert_order(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<ConvertOrderResponse>> {
    Ok(Json(
        get_convert_order_use_case(state.mysql.clone(), order_id).await?,
    ))
}

async fn list_new_coin_subscriptions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Query(query): Query<AdminNewCoinScopedListQuery>,
) -> AppResult<Json<NewCoinSubscriptionsResponse>> {
    Ok(Json(
        list_new_coin_subscriptions_for_project_use_case(state.mysql.clone(), project_id, query)
            .await?,
    ))
}

async fn list_all_new_coin_subscriptions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewCoinFlatListQuery>,
) -> AppResult<Json<NewCoinSubscriptionsResponse>> {
    Ok(Json(
        list_new_coin_subscriptions_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn list_new_coin_distributions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Query(query): Query<AdminNewCoinScopedListQuery>,
) -> AppResult<Json<NewCoinDistributionsResponse>> {
    Ok(Json(
        list_new_coin_distributions_for_project_use_case(state.mysql.clone(), project_id, query)
            .await?,
    ))
}

async fn list_all_new_coin_distributions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewCoinFlatListQuery>,
) -> AppResult<Json<NewCoinDistributionsResponse>> {
    Ok(Json(
        list_new_coin_distributions_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn list_new_coin_purchases(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewCoinPurchaseQuery>,
) -> AppResult<Json<NewCoinPurchasesResponse>> {
    Ok(Json(
        list_new_coin_purchases_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn list_new_coin_lock_positions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewCoinLockPositionQuery>,
) -> AppResult<Json<NewCoinLockPositionsResponse>> {
    Ok(Json(
        list_new_coin_lock_positions_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn list_new_coin_unlocks(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewCoinUnlockQuery>,
) -> AppResult<Json<NewCoinUnlocksResponse>> {
    Ok(Json(
        list_new_coin_unlocks_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn create_new_coin_project(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateNewCoinProjectRequest>,
) -> AppResult<Json<NewCoinProjectResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_new_coin_project_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn update_new_coin_lifecycle(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Json(request): Json<UpdateNewCoinLifecycleRequest>,
) -> AppResult<Json<NewCoinProjectResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_new_coin_lifecycle_use_case(state.mysql.clone(), admin_id, project_id, request)
            .await?,
    ))
}

async fn update_new_coin_unlock_rule(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Json(request): Json<UpdateNewCoinUnlockRuleRequest>,
) -> AppResult<Json<NewCoinProjectResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_new_coin_unlock_rule_use_case(state.mysql.clone(), admin_id, project_id, request)
            .await?,
    ))
}

async fn update_new_coin_unlock_fee_rule(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Json(request): Json<UpdateNewCoinUnlockFeeRuleRequest>,
) -> AppResult<Json<NewCoinProjectResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_new_coin_unlock_fee_rule_use_case(
            state.mysql.clone(),
            admin_id,
            project_id,
            request,
        )
        .await?,
    ))
}

async fn update_new_coin_post_listing_purchase(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Json(request): Json<UpdateNewCoinPostListingPurchaseRequest>,
) -> AppResult<Json<NewCoinProjectResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_new_coin_post_listing_purchase_use_case(
            state.mysql.clone(),
            admin_id,
            project_id,
            request,
        )
        .await?,
    ))
}

async fn distribute_new_coin(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Json(request): Json<DistributeNewCoinRequest>,
) -> AppResult<Json<NewCoinDistributionResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        distribute_new_coin_use_case(state.mysql.clone(), admin_id, project_id, request).await?,
    ))
}

async fn upsert_new_coin_convert_rule(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<UpsertNewCoinConvertRuleRequest>,
) -> AppResult<Json<NewCoinConvertRuleResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        upsert_new_coin_convert_rule_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn create_convert_pair(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateConvertPairRequest>,
) -> AppResult<Json<ConvertPairResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_convert_pair_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn update_convert_pair(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(pair_id): Path<u64>,
    Json(request): Json<UpdateConvertPairRequest>,
) -> AppResult<Json<ConvertPairResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_convert_pair_use_case(state.mysql.clone(), admin_id, pair_id, request).await?,
    ))
}

async fn delete_convert_pair(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(pair_id): Path<u64>,
    Json(request): Json<DeleteConvertPairRequest>,
) -> AppResult<StatusCode> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    delete_convert_pair_use_case(state.mysql.clone(), admin_id, pair_id, request).await?;
    Ok(StatusCode::NO_CONTENT)
}
