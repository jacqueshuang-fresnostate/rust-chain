//! 常驻行情 HTTP 入口；沿用 market-pairs 路径权限与 AdminAuth，状态切换在应用层统一协调。

use super::*;
use crate::modules::admin::{
    application::{
        get_admin_default_market, pause_admin_default_market, preview_admin_default_market,
        save_admin_default_market,
    },
    presentation::{
        DefaultMarketPreviewResponse, DefaultMarketResponse, PauseDefaultMarketRequest,
        PreviewDefaultMarketRequest, SaveDefaultMarketRequest,
    },
};

/// 注册交易对默认行情配置、无副作用预览及独立全部暂停入口；权限继承 market.pairs，不新增隐式开关。
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/market-pairs/:id/default-generator",
            get(get_default_market).patch(save_default_market),
        )
        .route(
            "/market-pairs/:id/default-generator/preview",
            post(preview_default_market),
        )
        .route(
            "/market-pairs/:id/default-generator/pause-all",
            patch(pause_default_market),
        )
}

async fn get_default_market(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> AppResult<Json<DefaultMarketResponse>> {
    Ok(Json(
        get_admin_default_market(state.mysql.clone(), id).await?,
    ))
}

async fn save_default_market(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<u64>,
    Json(request): Json<SaveDefaultMarketRequest>,
) -> AppResult<Json<DefaultMarketResponse>> {
    Ok(Json(
        save_admin_default_market(
            state.mysql.clone(),
            admin_id_from_subject(&claims.sub)?,
            id,
            request,
        )
        .await?,
    ))
}

async fn pause_default_market(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<u64>,
    Json(request): Json<PauseDefaultMarketRequest>,
) -> AppResult<Json<DefaultMarketResponse>> {
    Ok(Json(
        pause_admin_default_market(
            state.mysql.clone(),
            admin_id_from_subject(&claims.sub)?,
            id,
            request,
        )
        .await?,
    ))
}

async fn preview_default_market(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<u64>,
    Json(request): Json<PreviewDefaultMarketRequest>,
) -> AppResult<Json<DefaultMarketPreviewResponse>> {
    Ok(Json(
        preview_admin_default_market(state.mysql.clone(), id, request).await?,
    ))
}
