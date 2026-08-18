//! 高风险配置变更双人复核路由。

use crate::{
    error::AppResult,
    modules::{
        admin::{
            application::{
                apply_admin_config_change, create_admin_config_change,
                list_admin_config_change_requests, review_admin_config_change,
            },
            presentation::{
                AdminConfigChangeQuery, AdminConfigChangeResponse, AdminConfigChangesResponse,
                ApplyAdminConfigChangeRequest, CreateAdminConfigChangeRequest,
                ReviewAdminConfigChangeRequest,
            },
            service::admin_id_from_subject,
        },
        auth::AdminAuth,
    },
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/config-change-requests",
            get(list_requests).post(create_request),
        )
        .route("/config-change-requests/{id}/review", post(review_request))
        .route("/config-change-requests/{id}/apply", post(apply_request))
}

async fn list_requests(
    AdminAuth(_): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminConfigChangeQuery>,
) -> AppResult<Json<AdminConfigChangesResponse>> {
    let pool = super::mysql_pool(&state)?;
    Ok(Json(list_admin_config_change_requests(&pool, query).await?))
}

async fn create_request(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateAdminConfigChangeRequest>,
) -> AppResult<Json<AdminConfigChangeResponse>> {
    let pool = super::mysql_pool(&state)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_admin_config_change(&pool, admin_id, request).await?,
    ))
}

async fn review_request(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<u64>,
    Json(request): Json<ReviewAdminConfigChangeRequest>,
) -> AppResult<Json<AdminConfigChangeResponse>> {
    let pool = super::mysql_pool(&state)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        review_admin_config_change(&pool, admin_id, id, request).await?,
    ))
}

async fn apply_request(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<u64>,
    Json(request): Json<ApplyAdminConfigChangeRequest>,
) -> AppResult<Json<AdminConfigChangeResponse>> {
    let pool = super::mysql_pool(&state)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        apply_admin_config_change(&pool, admin_id, id, request).await?,
    ))
}
