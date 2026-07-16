//! loan 路由层。
//!
//! 只承担“HTTP 入口 -> 应用层用例”的薄适配职责，复用既有的身份解析与参数结构。

use crate::{
    error::AppResult,
    modules::auth::{AdminAuth, UserAuth},
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, patch, post},
};

use super::service::{admin_id_from_subject, mysql_pool, user_id_from_subject};

use super::presentation::{
    AdminLoanOrdersQuery, CreateLoanOrderRequest, CreateLoanProductRequest, ListQuery,
    LoanOrderActionResponse, LoanOrderResponse, LoanOrdersResponse, LoanProductResponse,
    LoanProductsResponse, ReviewLoanOrderRequest, UpdateLoanProductRequest,
    UpdateLoanProductStatusRequest, UserLoanOrdersQuery,
};

use super::application::{
    approve_loan_order_use_case, cancel_loan_order_use_case, create_loan_order_use_case,
    create_loan_product_use_case, get_admin_order_use_case, get_admin_product_use_case,
    get_user_order_use_case, list_active_products_use_case, list_admin_orders_use_case,
    list_admin_products_use_case, list_user_orders_use_case, reject_loan_order_use_case,
    repay_loan_order_use_case, update_loan_product_status_use_case, update_loan_product_use_case,
};

/// 用户端借贷路由。
pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/loan/products", get(list_active_products))
        .route("/loan/orders", get(list_user_orders).post(create_order))
        .route("/loan/orders/:id", get(get_user_order))
        .route("/loan/orders/:id/cancel", post(cancel_order))
        .route("/loan/orders/:id/repay", post(repay_order))
}

/// 管理端借贷路由。
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/loan/products",
            get(list_admin_products).post(create_product),
        )
        .route(
            "/loan/products/:id",
            get(get_admin_product).patch(update_product),
        )
        .route("/loan/products/:id/status", patch(update_product_status))
        .route("/loan/orders", get(list_admin_orders))
        .route("/loan/orders/:id", get(get_admin_order))
        .route("/loan/orders/:id/approve", post(approve_order))
        .route("/loan/orders/:id/reject", post(reject_order))
}

async fn list_active_products(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<LoanProductsResponse>> {
    let products = list_active_products_use_case(&mysql_pool(&state)?, query).await?;
    Ok(Json(products))
}

async fn list_admin_products(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<LoanProductsResponse>> {
    let products = list_admin_products_use_case(&mysql_pool(&state)?, query).await?;
    Ok(Json(products))
}

async fn get_admin_product(
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
) -> AppResult<Json<LoanProductResponse>> {
    Ok(Json(
        get_admin_product_use_case(&mysql_pool(&state)?, product_id).await?,
    ))
}

async fn create_product(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateLoanProductRequest>,
) -> AppResult<Json<LoanProductResponse>> {
    let pool = mysql_pool(&state)?;
    Ok(Json(create_loan_product_use_case(&pool, request).await?))
}

async fn update_product(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
    Json(request): Json<UpdateLoanProductRequest>,
) -> AppResult<Json<LoanProductResponse>> {
    let pool = mysql_pool(&state)?;
    Ok(Json(
        update_loan_product_use_case(&pool, product_id, request).await?,
    ))
}

async fn update_product_status(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
    Json(request): Json<UpdateLoanProductStatusRequest>,
) -> AppResult<Json<LoanProductResponse>> {
    let pool = mysql_pool(&state)?;
    Ok(Json(
        update_loan_product_status_use_case(&pool, product_id, request.status).await?,
    ))
}

async fn create_order(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateLoanOrderRequest>,
) -> AppResult<Json<LoanOrderActionResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let (order, changed) =
        create_loan_order_use_case(&mysql_pool(&state)?, user_id, request).await?;
    Ok(Json(LoanOrderActionResponse { order, changed }))
}

async fn list_user_orders(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<UserLoanOrdersQuery>,
) -> AppResult<Json<LoanOrdersResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    Ok(Json(
        list_user_orders_use_case(&mysql_pool(&state)?, user_id, query).await?,
    ))
}

async fn get_user_order(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<LoanOrderResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    Ok(Json(
        get_user_order_use_case(&mysql_pool(&state)?, user_id, order_id).await?,
    ))
}

async fn cancel_order(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<LoanOrderActionResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let (order, changed) =
        cancel_loan_order_use_case(&mysql_pool(&state)?, user_id, order_id).await?;
    Ok(Json(LoanOrderActionResponse { order, changed }))
}

async fn repay_order(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<LoanOrderActionResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let (order, changed) =
        repay_loan_order_use_case(&mysql_pool(&state)?, user_id, order_id).await?;
    Ok(Json(LoanOrderActionResponse { order, changed }))
}

async fn list_admin_orders(
    State(state): State<AppState>,
    Query(query): Query<AdminLoanOrdersQuery>,
) -> AppResult<Json<LoanOrdersResponse>> {
    Ok(Json(
        list_admin_orders_use_case(&mysql_pool(&state)?, query).await?,
    ))
}

async fn get_admin_order(
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<LoanOrderResponse>> {
    Ok(Json(
        get_admin_order_use_case(&mysql_pool(&state)?, order_id).await?,
    ))
}

async fn approve_order(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<LoanOrderActionResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let (order, changed) =
        approve_loan_order_use_case(&mysql_pool(&state)?, admin_id, order_id).await?;
    Ok(Json(LoanOrderActionResponse { order, changed }))
}

async fn reject_order(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
    Json(request): Json<ReviewLoanOrderRequest>,
) -> AppResult<Json<LoanOrderActionResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let (order, changed) =
        reject_loan_order_use_case(&mysql_pool(&state)?, admin_id, order_id, request.reason)
            .await?;
    Ok(Json(LoanOrderActionResponse { order, changed }))
}
