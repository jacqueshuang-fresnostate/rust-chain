//! news bounded context route layer.
//!
//! 路由层：声明公开新闻的免登录只读端点。
//! 两个端点都不带任何认证提取器，因此可见范围完全依赖 SQL 中固定的已发布状态限定；
//! 路由只负责取连接池并转交用例，过滤条件的归一与校验分别在表现层与领域层完成。

use super::presentation::{PublicNewsItemResponse, PublicNewsItemsResponse, PublicNewsQuery};
use crate::{error::AppResult, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};

/// 装配公开新闻的两个只读端点：集合路径返回分页列表，带数字主键的子路径返回单条详情。
/// 两者均只支持 GET，公共接口不提供任何写入能力，内容维护完全在后台侧完成。
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/news", get(list_public_news_items))
        .route("/news/:id", get(get_public_news_item))
}

/// 返回已发布新闻的分页列表，支持按分类、国家、语言与关键词组合过滤。
/// 查询串在转成过滤条件时已把空白折叠、页大小夹到一百以内、偏移夹到一万以内；
/// 分类与国家、语言若格式非法会返回校验错误，而不是忽略该条件返回全量数据。
async fn list_public_news_items(
    State(state): State<AppState>,
    Query(query): Query<PublicNewsQuery>,
) -> AppResult<Json<PublicNewsItemsResponse>> {
    let pool = super::application::mysql_pool(&state)?;
    Ok(Json(
        super::application::list_public_news_items(&pool, query.into()).await?,
    ))
}

/// 按路径主键返回单条新闻详情，含完整多语言内容文档。
/// 未发布与不存在都返回未找到，因此该端点不能用来探测后台草稿是否存在；无查询参数也不支持按语言裁剪。
async fn get_public_news_item(
    State(state): State<AppState>,
    Path(news_id): Path<u64>,
) -> AppResult<Json<PublicNewsItemResponse>> {
    let pool = super::application::mysql_pool(&state)?;
    Ok(Json(
        super::application::get_public_news_item(&pool, news_id).await?,
    ))
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_news_routes_tests.rs"]
mod tests;
