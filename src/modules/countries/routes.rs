//! countries 路由层。
//!
//! 负责将 HTTP 路径映射为应用层调用，避免路由层承载业务规则。
//! 本文件只暴露注册页所需的公开国家清单，可见范围由 SQL 中固定的启用与注册开关条件决定。

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

/// 装配国家配置的公开读取端点，只注册一个免登录 GET 路径。
/// 国家配置的增删改由后台管理路由负责，这里不提供任何写方法，也不带鉴权提取器。
pub fn routes() -> Router<AppState> {
    Router::new().route("/countries", get(list_public_countries_route))
}

/// 返回开放注册的国家清单，每项含国家代码、显示名、默认语言与可选语言列表。
/// 供注册页填充国家下拉框并据此决定初始界面语言，因此只包含状态启用且当前接受注册的国家。
/// 无入参、不分页、不缓存，顺序由后台排序值决定，后台调整配置后下一次请求即刻生效。
async fn list_public_countries_route(
    State(state): State<AppState>,
) -> AppResult<Json<super::presentation::PublicCountriesResponse>> {
    Ok(Json(
        super::application::list_public_countries(&mysql_pool(&state)?).await?,
    ))
}
