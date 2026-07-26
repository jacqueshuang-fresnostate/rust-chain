use super::*;

#[derive(Debug)]
pub(crate) struct AdminNewsListFilter {
    pub(crate) status: Option<String>,
    pub(crate) category: Option<String>,
    pub(crate) country_code: Option<String>,
    pub(crate) locale: Option<String>,
    pub(crate) keyword: Option<String>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug)]
pub(crate) struct AdminNewsInsert {
    pub(crate) title: String,
    pub(crate) banner_url: Option<String>,
    pub(crate) small_logo_url: Option<String>,
    pub(crate) category: String,
    pub(crate) status: String,
    pub(crate) country_code: Option<String>,
    pub(crate) default_locale: String,
    pub(crate) content_json: Value,
    pub(crate) published_at: Option<DateTime<Utc>>,
    pub(crate) admin_id: u64,
}

#[derive(Debug)]
pub(crate) struct AdminNewsUpdate {
    pub(crate) title: String,
    pub(crate) banner_url: Option<String>,
    pub(crate) small_logo_url: Option<String>,
    pub(crate) category: String,
    pub(crate) country_code: Option<String>,
    pub(crate) default_locale: String,
    pub(crate) content_json: Value,
    pub(crate) admin_id: u64,
}

#[derive(Debug)]
pub(crate) struct AdminNewsStatusUpdate {
    pub(crate) status: String,
    pub(crate) published_at: Option<DateTime<Utc>>,
    pub(crate) admin_id: u64,
}

pub(crate) async fn list_admin_news_items(
    pool: &Pool<MySql>,
    filter: AdminNewsListFilter,
) -> AppResult<(Vec<AdminNewsItemResponse>, i64)> {
    let mut rows = admin_news_query();
    let mut total = QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM admin_news_items");
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(status) = filter.status.clone() {
            builder.push(" AND status = ");
            builder.push_bind(status);
        }
        if let Some(category) = filter.category.clone() {
            builder.push(" AND category = ");
            builder.push_bind(category);
        }
        if let Some(country_code) = filter.country_code.clone() {
            builder.push(" AND country_code = ");
            builder.push_bind(country_code);
        }
        if let Some(locale) = filter.locale.clone() {
            builder.push(" AND JSON_SEARCH(content_json, 'one', ");
            builder.push_bind(locale);
            builder.push(", NULL, '$.items[*].locale') IS NOT NULL");
        }
        if let Some(keyword) = filter.keyword.clone() {
            builder.push(" AND (title LIKE ");
            builder.push_bind(format!("%{keyword}%"));
            builder.push(" OR CAST(content_json AS CHAR) LIKE ");
            builder.push_bind(format!("%{keyword}%"));
            builder.push(")");
        }
    }

    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY updated_at DESC, id DESC",
        filter.limit,
        filter.offset,
    )
    .await
}

pub(crate) async fn load_admin_news_item(
    pool: &Pool<MySql>,
    news_id: u64,
) -> AppResult<AdminNewsItemResponse> {
    let mut builder = admin_news_query();
    builder.push(" WHERE id = ");
    builder.push_bind(news_id);
    builder
        .build_query_as::<AdminNewsItemResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn insert_admin_news_item_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminNewsInsert,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO admin_news_items
           (title, banner_url, small_logo_url, category, status, country_code, default_locale, content_json, published_at,
            created_by_admin_id, updated_by_admin_id)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&input.title)
    .bind(&input.banner_url)
    .bind(&input.small_logo_url)
    .bind(&input.category)
    .bind(&input.status)
    .bind(input.country_code.as_deref())
    .bind(&input.default_locale)
    .bind(SqlxJson(input.content_json))
    .bind(input.published_at)
    .bind(input.admin_id)
    .bind(input.admin_id)
    .execute(&mut **tx)
    .await?;
    Ok(result.last_insert_id())
}

pub(crate) async fn update_admin_news_item_in_tx(
    tx: &mut Transaction<'_, MySql>,
    news_id: u64,
    input: AdminNewsUpdate,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE admin_news_items
           SET title = ?, banner_url = ?, small_logo_url = ?, category = ?, country_code = ?, default_locale = ?, content_json = ?, updated_by_admin_id = ?
           WHERE id = ?"#,
    )
    .bind(&input.title)
    .bind(&input.banner_url)
    .bind(&input.small_logo_url)
    .bind(&input.category)
    .bind(input.country_code.as_deref())
    .bind(&input.default_locale)
    .bind(SqlxJson(input.content_json))
    .bind(input.admin_id)
    .bind(news_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn update_admin_news_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    news_id: u64,
    input: AdminNewsStatusUpdate,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE admin_news_items
           SET status = ?, published_at = ?, updated_by_admin_id = ?
           WHERE id = ?"#,
    )
    .bind(&input.status)
    .bind(input.published_at)
    .bind(input.admin_id)
    .bind(news_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn load_admin_news_item_in_tx(
    tx: &mut Transaction<'_, MySql>,
    news_id: u64,
) -> AppResult<AdminNewsItemResponse> {
    let mut builder = admin_news_query();
    builder.push(" WHERE id = ");
    builder.push_bind(news_id);
    builder
        .build_query_as::<AdminNewsItemResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_admin_news_item_in_tx(
    tx: &mut Transaction<'_, MySql>,
    news_id: u64,
) -> AppResult<AdminNewsItemResponse> {
    let mut builder = admin_news_query();
    builder.push(" WHERE id = ");
    builder.push_bind(news_id);
    builder.push(" FOR UPDATE");
    builder
        .build_query_as::<AdminNewsItemResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

fn admin_news_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT id, title, banner_url, small_logo_url, category, status, country_code,
                  default_locale, content_json, published_at, created_by_admin_id,
                  updated_by_admin_id, created_at, updated_at
           FROM admin_news_items"#,
    )
}
