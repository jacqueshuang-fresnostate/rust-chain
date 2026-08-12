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

/// 按状态、分类、国家、内容 locale 和标题/正文关键字筛选后台新闻，分页返回完整内容及总数。
/// locale 通过 JSON_SEARCH 匹配、关键字通过 LIKE 匹配；列表与 COUNT 分别无锁执行，并发编辑可能导致两者快照不同。
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

/// 按新闻 ID 读取标题、图片、适用区域、多语言内容、发布信息和管理员字段。
/// 连接池查询不加锁；记录缺失返回未找到，内容 JSON 或时间字段映射失败返回错误，不改变发布状态。
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

/// 在调用方事务中插入新闻内容、初始状态、可选发布时间及创建/更新管理员，并返回新闻 ID。
/// 函数不按内容去重且不发布通知；调用方负责校验文档并与创建审计原子提交，SQL 失败整体回滚。
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

/// 在调用方事务中按 ID 覆盖新闻标题、图片、分类、适用区域、locale 内容及更新管理员。
/// 不修改状态和发布时间，也不检查受影响行数；调用方须先锁新闻，并与前后快照审计统一提交。
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

/// 在调用方事务中按新闻 ID 同时覆盖状态、发布时间和更新管理员。
/// 函数不推导 published_at 或检查状态迁移/受影响行数；调用方锁定旧新闻后决定目标值，并负责审计，不触发消息推送。
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

/// 在调用方事务快照中按 ID 回读新闻完整内容，供写后响应和审计使用。
/// 查询不追加锁；记录缺失返回未找到，SQL/JSON 映射失败由外层回滚，函数不提交或发送通知。
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

/// 在调用方事务中按新闻 ID 以 `FOR UPDATE` 锁定记录并返回修改前完整快照。
/// 锁持有至编辑/状态事务结束；记录缺失返回未找到，函数不判断状态迁移，也不提交、审计或发布内容。
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
