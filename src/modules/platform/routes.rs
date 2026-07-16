//! platform 路由层。
//!
//! 负责平台品牌接口的 HTTP 路由聚合，仅编排请求参数与应用服务调用。

use crate::{error::AppResult, state::AppState};
use axum::{Json, Router, extract::State, routing::get};

/// 平台品牌路由。
pub fn routes() -> Router<AppState> {
    Router::new().route("/platform/brand", get(get_platform_brand_route))
}

async fn get_platform_brand_route(
    State(state): State<AppState>,
) -> AppResult<Json<super::presentation::PlatformBrandResponse>> {
    Ok(Json(
        super::application::load_platform_brand(&super::service::mysql_pool(&state)?).await?,
    ))
}
