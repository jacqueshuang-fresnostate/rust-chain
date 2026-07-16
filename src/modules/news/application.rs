//! news bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。

use crate::{
    architecture::ApplicationLayer,
    error::{AppError, AppResult},
    modules::news::{
        domain::PublicNewsFilter,
        infrastructure::{fetch_public_news_item, fetch_public_news_items},
        presentation::{PublicNewsItemResponse, PublicNewsItemsResponse},
    },
    state::AppState,
};
use sqlx::{MySql, Pool};

#[derive(Debug)]
pub struct PublicNewsUseCase;

impl ApplicationLayer for PublicNewsUseCase {}

/// 统一提供新闻模块数据库连接池，保持路由层无连接池错误拼装逻辑。
pub(crate) fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for public news routes".to_owned())
    })
}

pub async fn list_public_news_items(
    pool: &Pool<MySql>,
    filter: PublicNewsFilter,
) -> AppResult<PublicNewsItemsResponse> {
    // 公开新闻只返回已发布内容，后台草稿和下架内容不能通过公共 API 泄漏。
    let news = fetch_public_news_items(pool, &filter).await?;
    Ok(PublicNewsItemsResponse { news })
}

pub async fn get_public_news_item(
    pool: &Pool<MySql>,
    news_id: u64,
) -> AppResult<PublicNewsItemResponse> {
    fetch_public_news_item(pool, news_id).await
}
