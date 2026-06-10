use crate::{
    error::{AppError, AppResult},
    state::AppState,
    time::{option_unix_millis, unix_millis},
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{MySql, Pool, QueryBuilder, types::Json as SqlxJson};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/news", get(list_public_news_items))
        .route("/news/:id", get(get_public_news_item))
}

#[derive(Debug, Deserialize)]
struct PublicNewsQuery {
    category: Option<String>,
    country_code: Option<String>,
    locale: Option<String>,
    q: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct PublicNewsItemResponse {
    id: u64,
    title: String,
    category: String,
    status: String,
    country_code: Option<String>,
    default_locale: String,
    content_json: SqlxJson<Value>,
    #[serde(default, with = "option_unix_millis")]
    published_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
    #[serde(with = "unix_millis")]
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
struct PublicNewsItemsResponse {
    news: Vec<PublicNewsItemResponse>,
}

async fn list_public_news_items(
    State(state): State<AppState>,
    Query(query): Query<PublicNewsQuery>,
) -> AppResult<Json<PublicNewsItemsResponse>> {
    let pool = mysql_pool(&state)?;
    let mut builder = public_news_query();
    builder.push(" WHERE status = 'published'");
    apply_public_news_filters(&mut builder, &query)?;
    builder.push(" ORDER BY published_at DESC, updated_at DESC, id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);
    builder.push(" OFFSET ");
    builder.push_bind(route_offset(query.offset) as i64);

    let news = builder
        .build_query_as::<PublicNewsItemResponse>()
        .fetch_all(&pool)
        .await?;
    Ok(Json(PublicNewsItemsResponse { news }))
}

async fn get_public_news_item(
    State(state): State<AppState>,
    Path(news_id): Path<u64>,
) -> AppResult<Json<PublicNewsItemResponse>> {
    let pool = mysql_pool(&state)?;
    let mut builder = public_news_query();
    builder.push(" WHERE id = ");
    builder.push_bind(news_id);
    builder.push(" AND status = 'published'");
    let item = builder
        .build_query_as::<PublicNewsItemResponse>()
        .fetch_optional(&pool)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(item))
}

fn public_news_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT id, title, category, status, country_code, default_locale, content_json,
                  published_at, created_at, updated_at
           FROM admin_news_items"#,
    )
}

fn apply_public_news_filters(
    builder: &mut QueryBuilder<'_, MySql>,
    query: &PublicNewsQuery,
) -> AppResult<()> {
    if let Some(category) = optional_string(query.category.clone()) {
        builder.push(" AND category = ");
        builder.push_bind(validate_news_category(&category)?);
    }
    if let Some(country_code) = optional_string(query.country_code.clone()) {
        let country_code = normalize_news_country_code(&country_code)?;
        builder.push(" AND (country_code IS NULL OR country_code = 'GLOBAL' OR country_code = ");
        builder.push_bind(country_code);
        builder.push(")");
    }
    if let Some(locale) = optional_string(query.locale.clone()) {
        builder.push(" AND JSON_SEARCH(content_json, 'one', ");
        builder.push_bind(validate_news_locale(&locale)?);
        builder.push(", NULL, '$.items[*].locale') IS NOT NULL");
    }
    if let Some(keyword) = optional_string(query.q.clone()) {
        builder.push(" AND (title LIKE ");
        builder.push_bind(format!("%{keyword}%"));
        builder.push(" OR CAST(content_json AS CHAR) LIKE ");
        builder.push_bind(format!("%{keyword}%"));
        builder.push(")");
    }
    Ok(())
}

fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for public news routes".to_owned())
    })
}

fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

fn route_offset(offset: Option<u32>) -> u32 {
    offset.unwrap_or(0).min(10_000)
}

fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn validate_news_category(value: &str) -> AppResult<String> {
    match value {
        "general" | "market" | "product" | "system" | "promotion" => Ok(value.to_owned()),
        _ => Err(AppError::Validation("unsupported news category".to_owned())),
    }
}

fn normalize_news_country_code(value: &str) -> AppResult<String> {
    let country_code = value.to_ascii_uppercase();
    if country_code == "GLOBAL" {
        return Ok(country_code);
    }
    if country_code.len() < 2
        || country_code.len() > 16
        || !country_code.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    {
        return Err(AppError::Validation(
            "news country_code format is invalid".to_owned(),
        ));
    }
    Ok(country_code)
}

fn validate_news_locale(value: &str) -> AppResult<String> {
    if value.len() < 2
        || value.len() > 16
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err(AppError::Validation(
            "news locale format is invalid".to_owned(),
        ));
    }
    Ok(value.to_owned())
}
