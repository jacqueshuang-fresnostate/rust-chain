use super::{
    application::{
        create_earn_category, create_earn_product, get_admin_earn_category, get_admin_earn_product,
        get_admin_earn_subscription, list_active_earn_products, list_admin_earn_categories,
        list_admin_earn_products, list_admin_earn_subscriptions, list_earn_subscriptions,
        redeem_earn_subscription_with_events as redeem_earn_subscription_with_events_use_case,
        subscribe_earn_product_with_events as subscribe_earn_product_with_events_use_case,
        update_earn_category, update_earn_category_status, update_earn_product,
        update_earn_product_status,
    },
    presentation::{
        AdminCategoriesQuery, AdminSubscriptionsQuery, CreateEarnCategoryRequest,
        CreateEarnProductRequest, EarnCategoriesResponse, EarnCategoryResponse,
        EarnProductResponse, EarnProductsResponse, EarnSubscriptionResponse,
        EarnSubscriptionsResponse, ListQuery, RedeemEarnResponse, SubscribeEarnRequest,
        SubscribeEarnResponse, UpdateEarnCategoryRequest, UpdateEarnCategoryStatusRequest,
        UpdateEarnProductRequest, UpdateEarnProductStatusRequest,
    },
};
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

pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/earn/products", get(list_active_products))
        .route(
            "/earn/subscriptions",
            get(list_subscriptions).post(subscribe),
        )
        .route("/earn/subscriptions/:id/redeem", post(redeem_subscription))
}

pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/earn/categories",
            get(list_admin_categories).post(create_category),
        )
        .route(
            "/earn/categories/:id",
            get(get_admin_category).patch(update_category),
        )
        .route("/earn/categories/:id/status", patch(update_category_status))
        .route(
            "/earn/products",
            get(list_admin_products).post(create_product),
        )
        .route(
            "/earn/products/:id",
            get(get_admin_product).patch(update_product),
        )
        .route("/earn/products/:id/status", patch(update_product_status))
        .route("/earn/subscriptions", get(list_admin_subscriptions))
        .route("/earn/subscriptions/:id", get(get_admin_subscription))
}

async fn list_active_products(
    UserAuth(_claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<EarnProductsResponse>> {
    Ok(Json(
        list_active_earn_products(state.mysql.clone(), query).await?,
    ))
}

async fn list_admin_products(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<EarnProductsResponse>> {
    Ok(Json(
        list_admin_earn_products(state.mysql.clone(), query).await?,
    ))
}

async fn get_admin_product(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
) -> AppResult<Json<EarnProductResponse>> {
    Ok(Json(
        get_admin_earn_product(state.mysql.clone(), product_id).await?,
    ))
}

async fn list_subscriptions(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<EarnSubscriptionsResponse>> {
    Ok(Json(
        list_earn_subscriptions(state.mysql.clone(), &claims.sub, query).await?,
    ))
}

async fn list_admin_subscriptions(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminSubscriptionsQuery>,
) -> AppResult<Json<EarnSubscriptionsResponse>> {
    Ok(Json(
        list_admin_earn_subscriptions(state.mysql.clone(), query).await?,
    ))
}

async fn get_admin_subscription(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(subscription_id): Path<u64>,
) -> AppResult<Json<EarnSubscriptionResponse>> {
    Ok(Json(
        get_admin_earn_subscription(state.mysql.clone(), subscription_id).await?,
    ))
}

async fn list_admin_categories(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminCategoriesQuery>,
) -> AppResult<Json<EarnCategoriesResponse>> {
    Ok(Json(
        list_admin_earn_categories(state.mysql.clone(), query).await?,
    ))
}

async fn get_admin_category(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(category_id): Path<u64>,
) -> AppResult<Json<EarnCategoryResponse>> {
    Ok(Json(
        get_admin_earn_category(state.mysql.clone(), category_id).await?,
    ))
}

async fn create_category(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateEarnCategoryRequest>,
) -> AppResult<Json<EarnCategoryResponse>> {
    Ok(Json(
        create_earn_category(state.mysql.clone(), &claims.sub, request).await?,
    ))
}

async fn update_category(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(category_id): Path<u64>,
    Json(request): Json<UpdateEarnCategoryRequest>,
) -> AppResult<Json<EarnCategoryResponse>> {
    Ok(Json(
        update_earn_category(state.mysql.clone(), &claims.sub, category_id, request).await?,
    ))
}

async fn update_category_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(category_id): Path<u64>,
    Json(request): Json<UpdateEarnCategoryStatusRequest>,
) -> AppResult<Json<EarnCategoryResponse>> {
    Ok(Json(
        update_earn_category_status(state.mysql.clone(), &claims.sub, category_id, request).await?,
    ))
}

async fn create_product(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateEarnProductRequest>,
) -> AppResult<Json<EarnProductResponse>> {
    Ok(Json(
        create_earn_product(state.mysql.clone(), &claims.sub, request).await?,
    ))
}

async fn update_product(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
    Json(request): Json<UpdateEarnProductRequest>,
) -> AppResult<Json<EarnProductResponse>> {
    Ok(Json(
        update_earn_product(state.mysql.clone(), &claims.sub, product_id, request).await?,
    ))
}

async fn update_product_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
    Json(request): Json<UpdateEarnProductStatusRequest>,
) -> AppResult<Json<EarnProductResponse>> {
    Ok(Json(
        update_earn_product_status(state.mysql.clone(), &claims.sub, product_id, request).await?,
    ))
}

async fn subscribe(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<SubscribeEarnRequest>,
) -> AppResult<Json<SubscribeEarnResponse>> {
    Ok(Json(
        subscribe_earn_product_with_events_use_case(
            state.mysql.clone(),
            state.event_broadcast_hub.as_ref(),
            &claims.sub,
            request,
        )
        .await?,
    ))
}

async fn redeem_subscription(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(subscription_id): Path<u64>,
) -> AppResult<Json<RedeemEarnResponse>> {
    Ok(Json(
        redeem_earn_subscription_with_events_use_case(
            state.mysql.clone(),
            state.event_broadcast_hub.as_ref(),
            &claims.sub,
            subscription_id,
        )
        .await?,
    ))
}
