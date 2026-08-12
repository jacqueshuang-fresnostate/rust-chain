use super::*;

/// 按状态、分类、国家、locale 和关键字筛选后台新闻，并返回分页摘要和匹配总数。
/// 枚举与地域筛选沿用写入校验，关键字仅去空白；查询不锁新闻且不返回富文本写事务快照。
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

/// 按新闻 ID 读取标题、图片、分类、状态、地域、locale、富文本和发布时间的后台详情。
/// 查询不加锁；记录缺失返回未找到，JSON/SQL 解码失败返回错误，也不改变阅读量或发布状态。
pub(crate) async fn get_admin_news_item(
    pool: Option<Pool<MySql>>,
    news_id: u64,
) -> AppResult<AdminNewsItemResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_news_item_from_store(&pool, news_id).await
}

/// 创建后台新闻并返回包含状态、地域、默认语言和发布时间的完整新闻响应。
/// 标题、图片长度、分类、locale 与版本 1 富文本文档须合法；状态缺省 draft，直接创建为 published 时设置当前发布时间。
/// 事务不锁其他业务行，依次插入新闻、回读和写 after 审计；数据库或审计失败整体回滚。
/// 创建无幂等键，也不发送站内信或发布事件；重复请求会创建另一条新闻。
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

/// 更新新闻正文、标题、图片、分类、地域和默认语言，并返回锁后最终快照。
/// 请求须提供审计原因且完整富文本文档通过校验；该用例不修改状态或 published_at。
/// 事务先锁新闻，再覆盖可编辑字段、回读并写 before/after 审计；记录缺失或任一步失败整体回滚。
/// 相同内容重放仍新增审计，不发送发布通知或清理图片对象。
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

/// 切换新闻 draft/published/archived 状态，并返回保留首发时间的最终新闻快照。
/// 请求须提供审计原因；事务锁新闻后仅在首次进入 published 且旧值无时间时写入当前时间，重复发布或归档保留原值。
/// 状态更新、管理员更新人、回读和 before/after 审计同事务提交；记录缺失或数据库失败整体回滚。
/// 相同状态重放仍写审计，且不发送通知、事件或外部缓存失效请求。
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
