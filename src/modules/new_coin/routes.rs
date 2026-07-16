use crate::{
    error::AppResult,
    modules::{
        auth::UserAuth,
        new_coin::{
            application::{
                create_new_coin_purchase_with_events as create_new_coin_purchase_with_events_use_case,
                create_new_coin_subscription_with_events as create_new_coin_subscription_with_events_use_case,
                get_new_coin_project, list_new_coin_distributions, list_new_coin_projects,
                list_new_coin_purchases, list_new_coin_subscriptions, list_new_coin_unlocks,
                pay_new_coin_unlock_fee,
                release_new_coin_unlock_with_events as release_new_coin_unlock_with_events_use_case,
            },
            presentation::{
                CreatePurchaseRequest, CreateSubscriptionRequest, ListQuery,
                NewCoinDistributionsResponse, NewCoinOrderCreationResponse, NewCoinProjectResponse,
                NewCoinProjectsResponse, NewCoinPurchasesResponse, NewCoinSubscriptionsResponse,
                NewCoinUnlocksResponse, PayUnlockFeeRequest, PayUnlockFeeResponse,
                ReleaseUnlockResponse,
            },
        },
    },
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};

pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/new-coins", get(list_projects))
        .route("/new-coins/:symbol", get(project_detail))
        .route(
            "/new-coins/:symbol/subscriptions",
            post(create_subscription),
        )
        .route("/new-coins/subscriptions", get(list_subscriptions))
        .route("/new-coins/distributions", get(list_distributions))
        .route("/new-coins/:symbol/purchase", post(create_purchase))
        .route("/new-coins/purchases", get(list_purchases))
        .route("/new-coins/unlocks", get(list_unlocks))
        .route("/new-coins/unlocks/:id/pay-fee", post(pay_unlock_fee))
        .route("/new-coins/unlocks/:id/release", post(release_unlock))
}

async fn list_projects(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<NewCoinProjectsResponse>> {
    Ok(Json(
        list_new_coin_projects(state.mysql.clone(), query).await?,
    ))
}

async fn project_detail(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> AppResult<Json<NewCoinProjectResponse>> {
    Ok(Json(
        get_new_coin_project(state.mysql.clone(), &symbol).await?,
    ))
}

async fn list_subscriptions(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<NewCoinSubscriptionsResponse>> {
    Ok(Json(
        list_new_coin_subscriptions(state.mysql.clone(), &claims.sub, query).await?,
    ))
}

async fn list_distributions(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<NewCoinDistributionsResponse>> {
    Ok(Json(
        list_new_coin_distributions(state.mysql.clone(), &claims.sub, query).await?,
    ))
}

async fn list_purchases(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<NewCoinPurchasesResponse>> {
    Ok(Json(
        list_new_coin_purchases(state.mysql.clone(), &claims.sub, query).await?,
    ))
}

async fn list_unlocks(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<NewCoinUnlocksResponse>> {
    Ok(Json(
        list_new_coin_unlocks(state.mysql.clone(), &claims.sub, query).await?,
    ))
}

async fn pay_unlock_fee(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<PayUnlockFeeRequest>,
) -> AppResult<Json<PayUnlockFeeResponse>> {
    Ok(Json(
        pay_new_coin_unlock_fee(state.mysql.clone(), &claims.sub, id, request).await?,
    ))
}

async fn release_unlock(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<ReleaseUnlockResponse>> {
    Ok(Json(
        release_new_coin_unlock_with_events_use_case(
            state.mysql.clone(),
            state.event_broadcast_hub.as_ref(),
            &claims.sub,
            id,
        )
        .await?,
    ))
}

async fn create_subscription(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Json(request): Json<CreateSubscriptionRequest>,
) -> AppResult<Json<NewCoinOrderCreationResponse>> {
    Ok(Json(
        create_new_coin_subscription_with_events_use_case(
            state.mysql.clone(),
            state.event_broadcast_hub.as_ref(),
            &claims.sub,
            symbol,
            request,
        )
        .await?,
    ))
}

async fn create_purchase(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Json(request): Json<CreatePurchaseRequest>,
) -> AppResult<Json<NewCoinOrderCreationResponse>> {
    Ok(Json(
        create_new_coin_purchase_with_events_use_case(
            state.mysql.clone(),
            state.event_broadcast_hub.as_ref(),
            &claims.sub,
            symbol,
            request,
        )
        .await?,
    ))
}
