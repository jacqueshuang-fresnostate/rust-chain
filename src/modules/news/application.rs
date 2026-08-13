//! news bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。
//! 新闻限界上下文对外只暴露两个免登录只读用例：分页列表与按主键取详情。
//! 两者都只返回已发布内容，不开事务、不写任何表、不做缓存，因此后台改稿或撤稿后下一次请求即刻可见。

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

/// 从全局状态取出新闻模块所需的 MySQL 连接池，未配置时归类为内部错误。
/// 公开新闻没有静态兜底数据，缺少数据库即无法提供任何内容，属于部署配置缺失而非调用方输入问题，
/// 因此不返回校验错误；错误信息集中在此拼装，路由层不再各自重复该分支。
pub(crate) fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for public news routes".to_owned())
    })
}

/// 返回已发布新闻列表，应用已校验的分类、地区、语言族、关键词和分页条件。
/// 查询只读且不缓存；数据库失败不回退到草稿或静态新闻。
pub async fn list_public_news_items(
    pool: &Pool<MySql>,
    filter: PublicNewsFilter,
) -> AppResult<PublicNewsItemsResponse> {
    // 公开新闻只返回已发布内容，后台草稿和下架内容不能通过公共 API 泄漏。
    let news = fetch_public_news_items(pool, &filter).await?;
    Ok(PublicNewsItemsResponse { news })
}

/// 按主键返回单条已发布新闻的完整内容，含横幅图、小图标、分类、默认语言与多语言内容 JSON。
/// 草稿、已归档与不存在的记录统一映射为未找到，调用方无法据此判断该 ID 是否真实存在。
/// 返回的多语言内容保持库中原样，包含当前地区用不到的语言项，由客户端自行按默认语言回退挑选。
/// 只读用例，不计阅读量、不写访问日志、不做缓存。
pub async fn get_public_news_item(
    pool: &Pool<MySql>,
    news_id: u64,
) -> AppResult<PublicNewsItemResponse> {
    fetch_public_news_item(pool, news_id).await
}
