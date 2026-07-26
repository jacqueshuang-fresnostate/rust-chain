use super::*;

pub(crate) async fn list_admin_news_items(
    pool: Option<Pool<MySql>>,
    query: AdminNewsQuery,
) -> AppResult<AdminNewsItemsResponse> {
    let status = query
        .status
        .and_then(optional_string)
        .map(|value| validate_news_status(&value))
        .transpose()?;
    let category = query
        .category
        .and_then(optional_string)
        .map(|value| validate_news_category(&value))
        .transpose()?;
    let country_code = query
        .country_code
        .and_then(optional_string)
        .map(|value| normalize_news_country_code(&value))
        .transpose()?;
    let locale = query
        .locale
        .and_then(optional_string)
        .map(|value| validate_news_locale(&value))
        .transpose()?;
    let keyword = query.q.and_then(optional_string);
    let pool = admin_mysql_pool(pool)?;
    let (news, total) = list_admin_news_items_from_store(
        &pool,
        AdminNewsListFilter {
            status,
            category,
            country_code,
            locale,
            keyword,
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(AdminNewsItemsResponse { news, total })
}

pub(crate) async fn get_admin_news_item(
    pool: Option<Pool<MySql>>,
    news_id: u64,
) -> AppResult<AdminNewsItemResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_news_item_from_store(&pool, news_id).await
}

pub(crate) async fn create_admin_news_item(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateAdminNewsItemRequest,
) -> AppResult<AdminNewsItemResponse> {
    let title = validate_news_title(&request.title)?;
    let banner_url = validate_optional_image_url(request.banner_url, "news banner_url")?;
    let small_logo_url =
        validate_optional_image_url(request.small_logo_url, "news small_logo_url")?;
    let category = validate_news_category(&request.category)?;
    let status = request
        .status
        .as_deref()
        .map(validate_news_status)
        .transpose()?
        .unwrap_or_else(|| "draft".to_owned());
    let country_code = normalize_optional_news_country_code(request.country_code)?;
    let default_locale = validate_news_locale(&request.default_locale)?;
    let content_json = validate_news_content_document(request.content_json, &default_locale)?;
    let published_at = (status == "published").then(Utc::now);
    let pool = admin_mysql_pool(pool)?;

    // 新闻正文、发布状态和审计日志必须同事务提交，避免后台显示与审计记录不一致。
    let mut tx = pool.begin().await?;
    let news_id = insert_admin_news_item_in_tx(
        &mut tx,
        AdminNewsInsert {
            title,
            banner_url,
            small_logo_url,
            category,
            status,
            country_code,
            default_locale,
            content_json,
            published_at,
            admin_id,
        },
    )
    .await?;
    let news = load_admin_news_item_in_tx(&mut tx, news_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "admin_news_item.create",
            target_type: "admin_news_item",
            target_id: news.id,
            before_json: None,
            after_json: Some(admin_news_item_audit_json(&news)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(news)
}

pub(crate) async fn update_admin_news_item(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    news_id: u64,
    request: UpdateAdminNewsItemRequest,
) -> AppResult<AdminNewsItemResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let title = validate_news_title(&request.title)?;
    let banner_url = validate_optional_image_url(request.banner_url, "news banner_url")?;
    let small_logo_url =
        validate_optional_image_url(request.small_logo_url, "news small_logo_url")?;
    let category = validate_news_category(&request.category)?;
    let country_code = normalize_optional_news_country_code(request.country_code)?;
    let default_locale = validate_news_locale(&request.default_locale)?;
    let content_json = validate_news_content_document(request.content_json, &default_locale)?;
    let pool = admin_mysql_pool(pool)?;

    // 先锁定旧新闻再写入，审计 before/after 才能精确反映本次编辑。
    let mut tx = pool.begin().await?;
    let before = lock_admin_news_item_in_tx(&mut tx, news_id).await?;
    update_admin_news_item_in_tx(
        &mut tx,
        news_id,
        AdminNewsUpdate {
            title,
            banner_url,
            small_logo_url,
            category,
            country_code,
            default_locale,
            content_json,
            admin_id,
        },
    )
    .await?;
    let after = load_admin_news_item_in_tx(&mut tx, news_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "admin_news_item.update",
            target_type: "admin_news_item",
            target_id: after.id,
            before_json: Some(admin_news_item_audit_json(&before)),
            after_json: Some(admin_news_item_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn update_admin_news_status(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    news_id: u64,
    request: UpdateAdminNewsStatusRequest,
) -> AppResult<AdminNewsItemResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let status = validate_news_status(&request.status)?;
    let pool = admin_mysql_pool(pool)?;

    // 首次发布才补 published_at；归档或重复发布保留原发布时间。
    let mut tx = pool.begin().await?;
    let before = lock_admin_news_item_in_tx(&mut tx, news_id).await?;
    let published_at = if status == "published" && before.published_at.is_none() {
        Some(Utc::now())
    } else {
        before.published_at
    };
    update_admin_news_status_in_tx(
        &mut tx,
        news_id,
        AdminNewsStatusUpdate {
            status,
            published_at,
            admin_id,
        },
    )
    .await?;
    let after = load_admin_news_item_in_tx(&mut tx, news_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "admin_news_item.status.update",
            target_type: "admin_news_item",
            target_id: after.id,
            before_json: Some(admin_news_item_audit_json(&before)),
            after_json: Some(admin_news_item_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}
