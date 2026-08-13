//! news bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。
//! 公开新闻的两条读路径都直接查后台新闻表，并在 SQL 中固定限定已发布状态，
//! 因此草稿与已归档内容不存在从公共接口泄漏的可能。
//! 过滤条件用 QueryBuilder 动态拼装，但所有取值都以绑定参数传入且事先经领域层校验；
//! 查询返回完整多语言内容 JSON，不在服务端按语言裁剪，历史语言版本因此不会在读取环节丢失。

use crate::{
    architecture::InfrastructureLayer,
    error::{AppError, AppResult},
    modules::news::{
        domain::{
            PublicNewsFilter, news_locale_search_patterns, normalize_news_country_code,
            validate_news_category,
        },
        presentation::PublicNewsItemResponse,
    },
};
use sqlx::{MySql, Pool, QueryBuilder};

#[derive(Debug)]
pub struct PublicNewsRepository;

impl InfrastructureLayer for PublicNewsRepository {}

/// 从 `admin_news_items` 查询 published 新闻；分类精确匹配，地区包含 GLOBAL/空值，语言按 JSON locale 族匹配。
/// 关键词同时搜索标题和 `content_json` 文本，结果按发布时间/更新时间/主键倒序分页；数据库失败直接返回。
/// 三级倒序排序保证同一发布时刻的多条新闻次序稳定，翻页时不会重复或漏掉记录。
/// 状态限定写在过滤条件之前，任何过滤组合都无法把草稿或已归档内容带出。
pub async fn fetch_public_news_items(
    pool: &Pool<MySql>,
    filter: &PublicNewsFilter,
) -> AppResult<Vec<PublicNewsItemResponse>> {
    let mut builder = public_news_query();
    builder.push(" WHERE status = 'published'");
    apply_public_news_filters(&mut builder, filter)?;
    builder.push(" ORDER BY published_at DESC, updated_at DESC, id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);
    builder.push(" OFFSET ");
    builder.push_bind(filter.offset as i64);

    builder
        .build_query_as::<PublicNewsItemResponse>()
        .fetch_all(pool)
        .await
        .map_err(AppError::from)
}

/// 按主键读取单条新闻详情，同时强制状态必须为 published。
/// 状态条件写进 SQL 而非查回后再判断，草稿与已归档记录因此在数据库层就被排除，
/// 命中不到统一返回未找到，调用方无法区分新闻不存在与尚未发布，避免通过 ID 探测未上线的公告。
/// 返回的多语言内容为完整 JSON 原文，不在此按语言裁剪，由前端按默认语言与用户偏好挑选展示项。
pub async fn fetch_public_news_item(
    pool: &Pool<MySql>,
    news_id: u64,
) -> AppResult<PublicNewsItemResponse> {
    let mut builder = public_news_query();
    builder.push(" WHERE id = ");
    builder.push_bind(news_id);
    builder.push(" AND status = 'published'");
    builder
        .build_query_as::<PublicNewsItemResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

/// 构造列表与详情共用的查询骨架，只含 SELECT 列与来源表，不带任何过滤或排序。
/// 两处入口共享同一份字段清单，保证详情页与列表页返回的结构完全一致，前端可复用同一渲染逻辑。
/// 内容列取的是完整多语言 JSON 原文，未按语言展开，因此新增语言项不会改变查询本身。
fn public_news_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT id, title, banner_url, small_logo_url, category, status, country_code, default_locale, content_json,
                  published_at, created_at, updated_at
           FROM admin_news_items"#,
    )
}

/// 按已给定的过滤条件逐段追加 WHERE 子句，调用方须保证前面已写入状态限定，这里只做 AND 追加。
/// 分类、国家代码与语言在拼装前分别经过白名单和格式校验，任一非法立即返回校验错误并放弃整条查询，
/// 校验后的值一律以绑定参数入句，不做字符串插值。
/// 地区条件放行三种情况：无地区限定、标记为 GLOBAL 的全球公告、以及精确匹配目标国家的内容。
/// 语言条件把目标语言展开成语言族模式集合，对多语言内容项的 locale 字段做 JSON_SEARCH，任一命中即可。
/// 关键词是唯一未做字符白名单的条件，它以两侧通配的绑定参数同时匹配标题与内容 JSON 的文本形式，
/// 因此用户输入的百分号和下划线会被当作通配符生效，这是既有行为而非疏漏。
fn apply_public_news_filters(
    builder: &mut QueryBuilder<'_, MySql>,
    filter: &PublicNewsFilter,
) -> AppResult<()> {
    if let Some(category) = &filter.category {
        builder.push(" AND category = ");
        builder.push_bind(validate_news_category(category)?);
    }
    if let Some(country_code) = &filter.country_code {
        let country_code = normalize_news_country_code(country_code)?;
        builder.push(" AND (country_code IS NULL OR country_code = 'GLOBAL' OR country_code = ");
        builder.push_bind(country_code);
        builder.push(")");
    }
    if let Some(locale) = &filter.locale {
        let patterns = news_locale_search_patterns(locale)?;
        builder.push(" AND (");
        for (index, pattern) in patterns.iter().enumerate() {
            if index > 0 {
                builder.push(" OR ");
            }
            builder.push("JSON_SEARCH(content_json, 'one', ");
            builder.push_bind(pattern.clone());
            builder.push(", NULL, '$.items[*].locale') IS NOT NULL");
        }
        builder.push(")");
    }
    if let Some(keyword) = &filter.keyword {
        builder.push(" AND (title LIKE ");
        builder.push_bind(format!("%{keyword}%"));
        builder.push(" OR CAST(content_json AS CHAR) LIKE ");
        builder.push_bind(format!("%{keyword}%"));
        builder.push(")");
    }
    Ok(())
}
