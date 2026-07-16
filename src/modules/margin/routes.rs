use crate::{
    error::AppResult,
    modules::margin::service::admin_id_from_subject,
    modules::user::service::user_id_from_subject,
    modules::{
        auth::{AdminAuth, UserAuth},
        margin::{
            application::{
                cancel_all_margin_positions_with_events as cancel_all_margin_positions_with_events_use_case,
                cancel_margin_position_with_events as cancel_margin_position_with_events_use_case,
                close_all_margin_positions_with_events as close_all_margin_positions_with_events_use_case,
                close_margin_position_with_events as close_margin_position_with_events_use_case,
                create_margin_product as create_margin_product_use_case,
                get_admin_margin_position as get_admin_margin_position_use_case,
                get_admin_margin_product as get_admin_margin_product_use_case,
                get_margin_position_risk_snapshot as get_margin_position_risk_snapshot_use_case,
                get_user_margin_position as get_user_margin_position_use_case,
                get_user_margin_setting as get_user_margin_setting_use_case,
                list_active_margin_products as list_active_margin_products_use_case,
                list_admin_margin_interest_summary as list_admin_margin_interest_summary_use_case,
                list_admin_margin_position_history as list_admin_margin_position_history_use_case,
                list_admin_margin_products as list_admin_margin_products_use_case,
                list_user_margin_positions as list_user_margin_positions_use_case,
                list_user_margin_wallets as list_user_margin_wallets_use_case, mysql_pool,
                open_margin_position_with_events as open_margin_position_with_events_use_case,
                route_limit, transfer_margin_funds as transfer_margin_funds_use_case,
                update_margin_product_config as update_margin_product_config_use_case,
                update_margin_product_status as update_margin_product_status_use_case,
                update_user_leverage as update_user_leverage_use_case,
                update_user_margin_mode as update_user_margin_mode_use_case,
            },
            presentation::{
                AdminInterestSummaryQuery, AdminInterestSummaryResponse, AdminListPositionsQuery,
                AdminMarginPositionResponse, AdminMarginPositionsResponse,
                CancelAllMarginPositionsResponse, CancelMarginPositionResponse,
                CloseAllMarginPositionsResponse, CloseMarginPositionResponse,
                CreateMarginProductRequest, ListPositionsQuery, ListQuery,
                MarginPositionDetailResponse, MarginPositionsResponse, MarginProductResponse,
                MarginProductsResponse, MarginRiskSnapshotResponse, MarginUserSettingResponse,
                MarginWalletsResponse, OpenMarginPositionRequest, OpenMarginPositionResponse,
                ProductActionRequest, TransferMarginFundsRequest, TransferMarginFundsResponse,
                UpdateMarginProductRequest, UpdateMarginProductStatusRequest,
                UpdateUserLeverageRequest, UpdateUserMarginModeRequest,
            },
        },
    },
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, patch, post},
};

pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/margin/products", get(list_active_products))
        .route("/margin/wallets", get(list_margin_wallets))
        .route("/margin/transfers", post(transfer_margin_funds))
        .route("/margin/settings/:product_id", get(get_user_margin_setting))
        .route(
            "/margin/settings/:product_id/leverage",
            patch(update_user_leverage),
        )
        .route(
            "/margin/settings/:product_id/mode",
            patch(update_user_margin_mode),
        )
        .route("/margin/positions", get(list_positions).post(open_position))
        .route("/margin/positions/close-all", post(close_all_positions))
        .route("/margin/positions/cancel-all", post(cancel_all_positions))
        .route("/margin/positions/:id", get(get_position))
        .route("/margin/positions/:id/risk", get(get_position_risk))
        .route("/margin/positions/:id/close", post(close_position))
        .route("/margin/positions/:id/cancel", post(cancel_position))
}

pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/margin/products",
            get(list_admin_products).post(create_product),
        )
        .route(
            "/margin/products/:id",
            get(get_admin_product).patch(update_product),
        )
        .route("/margin/products/:id/status", patch(update_product_status))
        .route("/margin/positions", get(list_admin_positions))
        .route("/margin/positions/:id", get(get_admin_position))
        .route("/margin/interest/summary", get(list_admin_interest_summary))
}
async fn list_active_products(
    UserAuth(_claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<MarginProductsResponse>> {
    Ok(Json(
        list_active_margin_products_use_case(&mysql_pool(&state)?, route_limit(query.limit))
            .await?,
    ))
}

async fn list_admin_products(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<MarginProductsResponse>> {
    Ok(Json(
        list_admin_margin_products_use_case(&mysql_pool(&state)?, route_limit(query.limit)).await?,
    ))
}

async fn get_admin_product(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
) -> AppResult<Json<MarginProductResponse>> {
    Ok(Json(
        get_admin_margin_product_use_case(&mysql_pool(&state)?, product_id).await?,
    ))
}

async fn create_product(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateMarginProductRequest>,
) -> AppResult<Json<MarginProductResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_margin_product_use_case(state.mysql.as_ref(), admin_id, request).await?,
    ))
}

async fn update_product(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
    Json(request): Json<UpdateMarginProductRequest>,
) -> AppResult<Json<MarginProductResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_margin_product_config_use_case(state.mysql.as_ref(), admin_id, product_id, request)
            .await?,
    ))
}

async fn update_product_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
    Json(request): Json<UpdateMarginProductStatusRequest>,
) -> AppResult<Json<MarginProductResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_margin_product_status_use_case(state.mysql.as_ref(), admin_id, product_id, request)
            .await?,
    ))
}

async fn list_positions(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListPositionsQuery>,
) -> AppResult<Json<MarginPositionsResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        list_user_margin_positions_use_case(&pool, user_id, query.status, route_limit(query.limit))
            .await?,
    ))
}

async fn list_margin_wallets(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<MarginWalletsResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        list_user_margin_wallets_use_case(&pool, user_id, route_limit(None)).await?,
    ))
}

async fn transfer_margin_funds(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<TransferMarginFundsRequest>,
) -> AppResult<Json<TransferMarginFundsResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        transfer_margin_funds_use_case(&pool, user_id, request).await?,
    ))
}

async fn update_user_leverage(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
    Json(request): Json<UpdateUserLeverageRequest>,
) -> AppResult<Json<MarginUserSettingResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        update_user_leverage_use_case(&pool, user_id, product_id, request).await?,
    ))
}

async fn get_user_margin_setting(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
) -> AppResult<Json<MarginUserSettingResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    Ok(Json(
        get_user_margin_setting_use_case(&mysql_pool(&state)?, user_id, product_id).await?,
    ))
}

async fn update_user_margin_mode(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
    Json(request): Json<UpdateUserMarginModeRequest>,
) -> AppResult<Json<MarginUserSettingResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        update_user_margin_mode_use_case(&pool, user_id, product_id, request).await?,
    ))
}

async fn list_admin_positions(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminListPositionsQuery>,
) -> AppResult<Json<AdminMarginPositionsResponse>> {
    let pool = mysql_pool(&state)?;
    Ok(Json(
        list_admin_margin_position_history_use_case(
            &pool,
            query.user_id,
            query.email,
            query.pair_id,
            query.status,
            route_limit(query.limit),
        )
        .await?,
    ))
}

async fn get_admin_position(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(position_id): Path<u64>,
) -> AppResult<Json<AdminMarginPositionResponse>> {
    Ok(Json(
        get_admin_margin_position_use_case(&mysql_pool(&state)?, position_id).await?,
    ))
}

async fn list_admin_interest_summary(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminInterestSummaryQuery>,
) -> AppResult<Json<AdminInterestSummaryResponse>> {
    let pool = mysql_pool(&state)?;
    Ok(Json(
        list_admin_margin_interest_summary_use_case(
            &pool,
            query.user_id,
            query.email,
            query.pair_id,
            query.status,
            route_limit(query.limit),
        )
        .await?,
    ))
}

async fn get_position(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(position_id): Path<u64>,
) -> AppResult<Json<MarginPositionDetailResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    Ok(Json(
        get_user_margin_position_use_case(&mysql_pool(&state)?, user_id, position_id).await?,
    ))
}

async fn get_position_risk(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(position_id): Path<u64>,
) -> AppResult<Json<MarginRiskSnapshotResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    Ok(Json(
        get_margin_position_risk_snapshot_use_case(
            &mysql_pool(&state)?,
            state.redis.as_ref(),
            user_id,
            position_id,
        )
        .await?,
    ))
}

async fn open_position(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<OpenMarginPositionRequest>,
) -> AppResult<Json<OpenMarginPositionResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        open_margin_position_with_events_use_case(
            &pool,
            state.redis.as_ref(),
            state.event_broadcast_hub.as_ref(),
            user_id,
            request,
        )
        .await?,
    ))
}

async fn close_position(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(position_id): Path<u64>,
) -> AppResult<Json<CloseMarginPositionResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        close_margin_position_with_events_use_case(
            &pool,
            state.redis.as_ref(),
            state.event_broadcast_hub.as_ref(),
            user_id,
            position_id,
        )
        .await?,
    ))
}

async fn close_all_positions(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<ProductActionRequest>,
) -> AppResult<Json<CloseAllMarginPositionsResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        close_all_margin_positions_with_events_use_case(
            &pool,
            state.redis.as_ref(),
            state.event_broadcast_hub.as_ref(),
            user_id,
            request.product_id,
        )
        .await?,
    ))
}

async fn cancel_position(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(position_id): Path<u64>,
) -> AppResult<Json<CancelMarginPositionResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        cancel_margin_position_with_events_use_case(
            &pool,
            state.event_broadcast_hub.as_ref(),
            user_id,
            position_id,
        )
        .await?,
    ))
}

async fn cancel_all_positions(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<ProductActionRequest>,
) -> AppResult<Json<CancelAllMarginPositionsResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        cancel_all_margin_positions_with_events_use_case(
            &pool,
            state.event_broadcast_hub.as_ref(),
            user_id,
            request.product_id,
        )
        .await?,
    ))
}
