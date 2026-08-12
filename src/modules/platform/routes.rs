//! platform 路由层。
//!
//! 负责平台品牌接口的 HTTP 路由聚合，仅编排请求参数与应用服务调用。

use crate::{error::AppResult, state::AppState};
use axum::{Json, Router, extract::State, routing::get};

/// 从 HTTP 状态中取得平台品牌查询使用的数据库连接池。
///
/// 本函数只承担传输层依赖装配；连接池缺失时返回明确错误，避免服务层依赖全局状态。
fn mysql_pool(state: &AppState) -> AppResult<crate::state::MySqlPool> {
    state.mysql.clone().ok_or_else(|| {
        crate::error::AppError::Internal(
            "mysql pool is not configured for platform route".to_owned(),
        )
    })
}

/// 平台品牌路由。
pub fn routes() -> Router<AppState> {
    Router::new().route("/platform/brand", get(get_platform_brand_route))
}

async fn get_platform_brand_route(
    State(state): State<AppState>,
) -> AppResult<Json<super::presentation::PlatformBrandResponse>> {
    Ok(Json(
        super::application::load_platform_brand(&mysql_pool(&state)?).await?,
    ))
}
