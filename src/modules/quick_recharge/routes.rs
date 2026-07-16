//! quick_recharge 路由层。
//!
//! 负责用户端、管理员端、公共回调的路由适配，不承载业务规则。

use crate::{
    error::AppResult,
    modules::auth::{AdminAuth, UserAuth},
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post},
};
use serde_json::Value;

use super::{
    CreateQuickRechargeOrderRequest, DeleteQuickRechargeOrderRequest, QuickRechargeConfigResponse,
    QuickRechargeOrderResponse, QuickRechargeOrdersQuery, QuickRechargeOrdersResponse,
    SaveQuickRechargeConfigRequest, TestQuickRechargeConfigRequest,
    TestQuickRechargeConfigResponse, UserQuickRechargeConfigResponse,
};

/// 用户端快速充值相关路由。
pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/wallet/quick-recharge/config",
            get(get_user_quick_recharge_config),
        )
        .route(
            "/wallet/quick-recharge/orders",
            get(list_user_quick_recharge_orders).post(create_user_quick_recharge_order),
        )
}

/// 管理端快速充值相关路由。
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/quick-recharge/config",
            get(get_admin_quick_recharge_config).patch(save_admin_quick_recharge_config),
        )
        .route(
            "/quick-recharge/config/test",
            post(test_admin_quick_recharge_config),
        )
        .route(
            "/quick-recharge/orders",
            get(list_admin_quick_recharge_orders),
        )
        .route(
            "/quick-recharge/orders/:order_id",
            delete(delete_admin_quick_recharge_order),
        )
}

/// GMPay 回调公开路由。
pub fn public_routes() -> Router<AppState> {
    Router::new().route("/payments/gmpay/notify", post(handle_gmpay_notify))
}

async fn get_user_quick_recharge_config(
    _auth: UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<UserQuickRechargeConfigResponse>> {
    Ok(Json(
        super::application::get_user_quick_recharge_config(state.mysql.clone()).await?,
    ))
}

async fn list_user_quick_recharge_orders(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<QuickRechargeOrdersQuery>,
) -> AppResult<Json<QuickRechargeOrdersResponse>> {
    Ok(Json(
        super::application::list_user_quick_recharge_orders(
            state.mysql.clone(),
            &claims.sub,
            query,
        )
        .await?,
    ))
}

async fn create_user_quick_recharge_order(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateQuickRechargeOrderRequest>,
) -> AppResult<Json<QuickRechargeOrderResponse>> {
    Ok(Json(
        super::application::create_user_quick_recharge_order(
            state.mysql.clone(),
            state.settings.exposed_credential_encryption_key(),
            &claims.sub,
            request,
        )
        .await?,
    ))
}

async fn get_admin_quick_recharge_config(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<QuickRechargeConfigResponse>> {
    Ok(Json(
        super::application::get_admin_quick_recharge_config(state.mysql.clone()).await?,
    ))
}

async fn save_admin_quick_recharge_config(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<SaveQuickRechargeConfigRequest>,
) -> AppResult<Json<QuickRechargeConfigResponse>> {
    Ok(Json(
        super::application::save_admin_quick_recharge_config(
            state.mysql.clone(),
            state.settings.exposed_credential_encryption_key(),
            &claims.sub,
            request,
        )
        .await?,
    ))
}

async fn test_admin_quick_recharge_config(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<TestQuickRechargeConfigRequest>,
) -> AppResult<Json<TestQuickRechargeConfigResponse>> {
    Ok(Json(
        super::application::test_admin_quick_recharge_config(
            state.mysql.clone(),
            state.settings.exposed_credential_encryption_key(),
            &claims.sub,
            request,
        )
        .await?,
    ))
}

async fn list_admin_quick_recharge_orders(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<QuickRechargeOrdersQuery>,
) -> AppResult<Json<QuickRechargeOrdersResponse>> {
    Ok(Json(
        super::application::list_admin_quick_recharge_orders(state.mysql.clone(), query).await?,
    ))
}

async fn delete_admin_quick_recharge_order(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<String>,
    Json(request): Json<DeleteQuickRechargeOrderRequest>,
) -> AppResult<StatusCode> {
    super::application::delete_admin_quick_recharge_order(
        state.mysql.clone(),
        &claims.sub,
        &order_id,
        request,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn handle_gmpay_notify(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> AppResult<&'static str> {
    super::application::handle_gmpay_notify(
        state.mysql.clone(),
        state.settings.exposed_credential_encryption_key(),
        payload,
    )
    .await?;
    Ok("ok")
}
