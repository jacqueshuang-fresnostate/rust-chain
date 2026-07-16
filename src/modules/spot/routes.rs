use crate::{
    error::AppResult,
    modules::spot::service::admin_id_from_subject,
    modules::user::service::user_id_from_subject,
    modules::{
        auth::{AdminAuth, UserAuth},
        spot::{
            application::{
                cancel_admin_spot_order_with_events as cancel_admin_spot_order_with_events_use_case,
                cancel_all_user_spot_orders_with_events as cancel_all_user_spot_orders_with_events_use_case,
                cancel_user_spot_order_with_events as cancel_user_spot_order_with_events_use_case,
                create_spot_order_with_events as create_spot_order_with_events_use_case,
                fill_spot_orders_with_events_with_request as fill_spot_orders_with_events_with_request_use_case,
                get_admin_spot_order as get_admin_spot_order_use_case,
                list_admin_spot_orders as list_admin_spot_orders_use_case,
                list_admin_spot_trades as list_admin_spot_trades_use_case,
                list_user_spot_orders as list_user_spot_orders_use_case,
                list_user_spot_trades as list_user_spot_trades_use_case, mysql_pool,
                validate_admin_cancel_spot_order_request,
            },
            presentation::{
                AdminCancelSpotOrderRequest, AdminSpotOrdersQuery, AdminSpotTradesQuery,
                CancelAllSpotOrdersQuery, CreateSpotOrderRequest, FillSpotOrdersRequest,
                SpotCancelAllResponse, SpotCancelResponse, SpotFillResponse, SpotOrderResponse,
                SpotOrdersQuery, SpotOrdersResponse, SpotTradesQuery, SpotTradesResponse,
            },
        },
    },
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{delete, get, post},
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/spot/orders",
            post(create_order)
                .get(list_orders)
                .delete(cancel_all_orders),
        )
        .route("/spot/orders/:id", delete(cancel_order))
        .route("/spot/trades", get(list_trades))
}

pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/spot/orders", get(list_admin_orders))
        .route("/spot/orders/:id", get(get_admin_order))
        .route("/spot/orders/:id/cancel", post(cancel_admin_order))
        .route("/spot/trades", get(list_admin_trades))
        .route("/spot/fills", post(fill_orders))
}

async fn create_order(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateSpotOrderRequest>,
) -> AppResult<Json<SpotOrderResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let response = create_spot_order_with_events_use_case(
        &mysql_pool(&state)?,
        state.redis.as_ref(),
        state.event_broadcast_hub.as_ref(),
        user_id,
        request,
    )
    .await?;

    Ok(Json(response))
}

async fn list_orders(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<SpotOrdersQuery>,
) -> AppResult<Json<SpotOrdersResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        list_user_spot_orders_use_case(&pool, user_id, query).await?,
    ))
}

async fn list_admin_orders(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminSpotOrdersQuery>,
) -> AppResult<Json<SpotOrdersResponse>> {
    let pool = mysql_pool(&state)?;
    Ok(Json(list_admin_spot_orders_use_case(&pool, query).await?))
}

async fn get_admin_order(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<SpotOrderResponse>> {
    Ok(Json(
        get_admin_spot_order_use_case(&mysql_pool(&state)?, order_id).await?,
    ))
}

async fn cancel_admin_order(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
    Json(request): Json<AdminCancelSpotOrderRequest>,
) -> AppResult<Json<SpotCancelResponse>> {
    let reason = validate_admin_cancel_spot_order_request(request)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let response = cancel_admin_spot_order_with_events_use_case(
        &mysql_pool(&state)?,
        order_id,
        admin_id,
        reason,
        state.event_broadcast_hub.as_ref(),
    )
    .await?;

    Ok(Json(response))
}

async fn cancel_order(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<SpotCancelResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let response = cancel_user_spot_order_with_events_use_case(
        &mysql_pool(&state)?,
        order_id,
        user_id,
        state.event_broadcast_hub.as_ref(),
    )
    .await?;

    Ok(Json(response))
}

async fn cancel_all_orders(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<CancelAllSpotOrdersQuery>,
) -> AppResult<Json<SpotCancelAllResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    Ok(Json(
        cancel_all_user_spot_orders_with_events_use_case(
            &mysql_pool(&state)?,
            user_id,
            query,
            state.event_broadcast_hub.as_ref(),
        )
        .await?,
    ))
}

async fn list_trades(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<SpotTradesQuery>,
) -> AppResult<Json<SpotTradesResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    Ok(Json(
        list_user_spot_trades_use_case(&mysql_pool(&state)?, user_id, query).await?,
    ))
}

async fn list_admin_trades(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminSpotTradesQuery>,
) -> AppResult<Json<SpotTradesResponse>> {
    let pool = mysql_pool(&state)?;
    Ok(Json(list_admin_spot_trades_use_case(&pool, query).await?))
}

async fn fill_orders(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<FillSpotOrdersRequest>,
) -> AppResult<Json<SpotFillResponse>> {
    let response = fill_spot_orders_with_events_with_request_use_case(
        &mysql_pool(&state)?,
        request,
        state.event_broadcast_hub.as_ref(),
    )
    .await?;
    Ok(Json(response))
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_spot_routes_tests.rs"]
mod tests;
