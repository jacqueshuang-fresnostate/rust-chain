//! countries 路由层。
//!
//! 负责将 HTTP 路径映射为应用层调用，避免路由层承载业务规则。

use crate::{error::AppResult, state::AppState};
use axum::{Json, Router, extract::State, routing::get};

/// 从 HTTP 状态中取得国家列表查询使用的数据库连接池。
///
/// 该函数不执行查询或业务筛选，连接池缺失时返回稳定内部错误供统一错误层处理。
fn mysql_pool(state: &AppState) -> AppResult<crate::state::MySqlPool> {
    state.mysql.clone().ok_or_else(|| {
        crate::error::AppError::Internal(
            "mysql pool is not configured for countries route".to_owned(),
        )
    })
}

/// 国家公共信息路由。
pub fn routes() -> Router<AppState> {
    Router::new().route("/countries", get(list_public_countries_route))
}

async fn list_public_countries_route(
    State(state): State<AppState>,
) -> AppResult<Json<super::presentation::PublicCountriesResponse>> {
    Ok(Json(
        super::application::list_public_countries(&mysql_pool(&state)?).await?,
    ))
}
