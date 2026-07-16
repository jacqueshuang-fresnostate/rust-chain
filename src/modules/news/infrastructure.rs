//! news bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。

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

fn public_news_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT id, title, banner_url, small_logo_url, category, status, country_code, default_locale, content_json,
                  published_at, created_at, updated_at
           FROM admin_news_items"#,
    )
}

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
