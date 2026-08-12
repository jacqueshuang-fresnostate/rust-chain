use super::*;

/// 构建资产、钱包查询、充值网络与地址池的后台传输路由。
///
/// 路由仅负责管理员鉴权、Path/Query/JSON 解析和响应状态映射；涉及余额、流水、地址占用与
/// 审计的不变量全部由应用用例维护。删除资产成功仍映射为 204，其余错误沿既有统一错误响应返回。
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/assets", get(list_assets).post(create_asset))
        .route(
            "/assets/:id",
            get(get_asset).patch(update_asset).delete(delete_asset),
        )
        .route("/wallet/accounts", get(list_wallet_accounts))
        .route("/wallet/ledger", get(list_wallet_ledger))
        .route(
            "/deposit-network-configs",
            get(list_deposit_network_configs).post(create_deposit_network_config),
        )
        .route(
            "/deposit-network-configs/:id",
            patch(update_deposit_network_config),
        )
        .route(
            "/deposit-address-pool",
            get(list_deposit_address_pool).post(create_deposit_address_pool),
        )
        .route(
            "/deposit-address-pool/batch",
            post(create_deposit_address_pool_batch),
        )
        .route(
            "/deposit-address-pool/:id",
            get(get_deposit_address_pool).patch(update_deposit_address_pool),
        )
        .route(
            "/deposit-address-pool/:id/reclaim",
            post(reclaim_deposit_address_pool),
        )
}

async fn list_assets(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminAssetQuery>,
) -> AppResult<Json<AdminAssetsResponse>> {
    Ok(Json(
        list_assets_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn get_asset(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(asset_id): Path<u64>,
) -> AppResult<Json<AdminAssetResponse>> {
    Ok(Json(
        get_asset_use_case(state.mysql.clone(), asset_id).await?,
    ))
}

async fn update_asset(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(asset_id): Path<u64>,
    Json(request): Json<UpdateAssetRequest>,
) -> AppResult<Json<AdminAssetResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_asset_use_case(state.mysql.clone(), admin_id, asset_id, request).await?,
    ))
}

async fn create_asset(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateAssetRequest>,
) -> AppResult<Json<AdminAssetResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_asset_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn delete_asset(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(asset_id): Path<u64>,
    Json(request): Json<DeleteAssetRequest>,
) -> AppResult<StatusCode> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    delete_asset_use_case(state.mysql.clone(), admin_id, asset_id, request).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_wallet_accounts(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminWalletAccountQuery>,
) -> AppResult<Json<AdminWalletAccountsResponse>> {
    Ok(Json(
        list_wallet_accounts_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn list_wallet_ledger(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminWalletLedgerQuery>,
) -> AppResult<Json<AdminWalletLedgerResponseList>> {
    Ok(Json(
        list_wallet_ledger_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn list_deposit_network_configs(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminDepositNetworkConfigQuery>,
) -> AppResult<Json<AdminDepositNetworkConfigResponseList>> {
    Ok(Json(
        list_deposit_network_configs_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn create_deposit_network_config(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateDepositNetworkConfigRequest>,
) -> AppResult<Json<AdminDepositNetworkConfigResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_deposit_network_config_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn update_deposit_network_config(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(config_id): Path<u64>,
    Json(request): Json<UpdateDepositNetworkConfigRequest>,
) -> AppResult<Json<AdminDepositNetworkConfigResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_deposit_network_config_use_case(state.mysql.clone(), admin_id, config_id, request)
            .await?,
    ))
}

async fn list_deposit_address_pool(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminDepositAddressPoolQuery>,
) -> AppResult<Json<AdminDepositAddressPoolResponseList>> {
    Ok(Json(
        list_deposit_address_pool_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn get_deposit_address_pool(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(address_id): Path<u64>,
) -> AppResult<Json<AdminDepositAddressPoolResponse>> {
    Ok(Json(
        get_deposit_address_pool_use_case(state.mysql.clone(), address_id).await?,
    ))
}

async fn create_deposit_address_pool(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateDepositAddressPoolRequest>,
) -> AppResult<Json<AdminDepositAddressPoolResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_deposit_address_pool_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn create_deposit_address_pool_batch(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateDepositAddressPoolBatchRequest>,
) -> AppResult<Json<AdminDepositAddressPoolBatchResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_deposit_address_pool_batch_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn update_deposit_address_pool(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(address_id): Path<u64>,
    Json(request): Json<UpdateDepositAddressPoolRequest>,
) -> AppResult<Json<AdminDepositAddressPoolResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_deposit_address_pool_use_case(state.mysql.clone(), admin_id, address_id, request)
            .await?,
    ))
}

async fn reclaim_deposit_address_pool(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(address_id): Path<u64>,
    Json(request): Json<ReclaimDepositAddressPoolRequest>,
) -> AppResult<Json<AdminDepositAddressPoolResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        reclaim_deposit_address_pool_use_case(state.mysql.clone(), admin_id, address_id, request)
            .await?,
    ))
}
