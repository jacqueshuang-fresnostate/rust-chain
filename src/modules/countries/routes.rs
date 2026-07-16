//! countries 路由层。
//!
//! 负责将 HTTP 路径映射为应用层调用，避免路由层承载业务规则。

use crate::{error::AppResult, state::AppState};
use axum::{Json, Router, extract::State, routing::get};

/// 国家公共信息路由。
pub fn routes() -> Router<AppState> {
    Router::new().route("/countries", get(list_public_countries_route))
}

async fn list_public_countries_route(
    State(state): State<AppState>,
) -> AppResult<Json<super::presentation::PublicCountriesResponse>> {
    Ok(Json(
        super::application::list_public_countries(&super::service::mysql_pool(&state)?).await?,
    ))
}
