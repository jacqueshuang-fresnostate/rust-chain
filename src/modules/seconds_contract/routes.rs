use super::{
    application::mysql_pool,
    application::{
        create_product as create_product_use_case, delete_product as delete_product_use_case,
        get_admin_order as get_admin_order_use_case,
        get_admin_product as get_admin_product_use_case,
        list_active_products as list_active_products_use_case,
        list_admin_orders as list_admin_orders_use_case,
        list_admin_products as list_admin_products_use_case,
        list_user_orders as list_user_orders_use_case,
        open_order_with_events as open_order_with_events_use_case,
        settle_order_with_events as settle_order_with_events_use_case,
        update_product as update_product_use_case,
        update_product_status as update_product_status_use_case,
    },
    presentation::{
        AdminOrdersQuery, CreateSecondsContractProductRequest, DeleteSecondsContractProductRequest,
        ListQuery, OpenSecondsContractOrderRequest, OpenSecondsContractOrderResponse,
        SecondsContractOrderResponse, SecondsContractOrdersResponse,
        SecondsContractProductResponse, SecondsContractProductsResponse,
        SettleSecondsContractOrderRequest, SettleSecondsContractOrderResponse,
        UpdateSecondsContractProductRequest, UpdateSecondsContractProductStatusRequest,
    },
    service::{admin_id_from_subject, route_limit, user_id_from_subject},
};
use crate::{
    error::AppResult,
    modules::auth::{AdminAuth, UserAuth},
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, patch, post},
};

pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/seconds-contracts/products", get(list_active_products))
        .route(
            "/seconds-contracts/orders",
            get(list_orders).post(open_order),
        )
}

pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/seconds-contracts/products",
            get(list_admin_products).post(create_product),
        )
        .route(
            "/seconds-contracts/products/:id",
            get(get_admin_product)
                .patch(update_product)
                .delete(delete_product),
        )
        .route(
            "/seconds-contracts/products/:id/status",
            patch(update_product_status),
        )
        .route("/seconds-contracts/orders", get(list_admin_orders))
        .route("/seconds-contracts/orders/:id", get(get_admin_order))
        .route("/seconds-contracts/orders/:id/settle", post(settle_order))
}

async fn list_active_products(
    UserAuth(_claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<SecondsContractProductsResponse>> {
    let pool = mysql_pool(&state)?;
    Ok(Json(
        list_active_products_use_case(&pool, route_limit(query.limit)).await?,
    ))
}

async fn list_admin_products(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<SecondsContractProductsResponse>> {
    let pool = mysql_pool(&state)?;
    Ok(Json(
        list_admin_products_use_case(&pool, route_limit(query.limit)).await?,
    ))
}

async fn get_admin_product(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
) -> AppResult<Json<SecondsContractProductResponse>> {
    let pool = mysql_pool(&state)?;
    Ok(Json(get_admin_product_use_case(&pool, product_id).await?))
}

async fn list_orders(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<SecondsContractOrdersResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        list_user_orders_use_case(&pool, user_id, route_limit(query.limit)).await?,
    ))
}

async fn list_admin_orders(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminOrdersQuery>,
) -> AppResult<Json<SecondsContractOrdersResponse>> {
    let pool = mysql_pool(&state)?;
    Ok(Json(list_admin_orders_use_case(&pool, query).await?))
}

async fn get_admin_order(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<SecondsContractOrderResponse>> {
    let pool = mysql_pool(&state)?;
    Ok(Json(get_admin_order_use_case(&pool, order_id).await?))
}

async fn create_product(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateSecondsContractProductRequest>,
) -> AppResult<Json<SecondsContractProductResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_product_use_case(state.mysql.as_ref(), admin_id, request).await?,
    ))
}

async fn update_product(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
    Json(request): Json<UpdateSecondsContractProductRequest>,
) -> AppResult<Json<SecondsContractProductResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_product_use_case(state.mysql.as_ref(), admin_id, product_id, request).await?,
    ))
}

async fn update_product_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
    Json(request): Json<UpdateSecondsContractProductStatusRequest>,
) -> AppResult<Json<SecondsContractProductResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_product_status_use_case(state.mysql.as_ref(), admin_id, product_id, request).await?,
    ))
}

async fn delete_product(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
    Json(request): Json<DeleteSecondsContractProductRequest>,
) -> AppResult<StatusCode> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    delete_product_use_case(state.mysql.as_ref(), admin_id, product_id, request).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn open_order(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<OpenSecondsContractOrderRequest>,
) -> AppResult<Json<OpenSecondsContractOrderResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let response = open_order_with_events_use_case(
        state.mysql.as_ref(),
        state.redis.as_ref(),
        user_id,
        request,
        state.event_broadcast_hub.as_ref(),
    )
    .await?;
    Ok(Json(response))
}

async fn settle_order(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
    Json(request): Json<SettleSecondsContractOrderRequest>,
) -> AppResult<Json<SettleSecondsContractOrderResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let response = settle_order_with_events_use_case(
        state.mysql.as_ref(),
        admin_id,
        order_id,
        request,
        state.event_broadcast_hub.as_ref(),
    )
    .await?;
    Ok(Json(response))
}
