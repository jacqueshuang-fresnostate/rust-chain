use super::*;

/// 构建国家配置与后台新闻管理的传输路由。
///
/// 路由保持管理员鉴权、原有 Path/Query/JSON DTO 及 HTTP 方法，仅把管理员审计主体和
/// 已解析输入转发给对应应用用例；国家编码、语言内容和新闻状态规则仍由下层校验，
/// 所有失败按既有统一错误响应返回。
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/countries",
            get(list_admin_countries).post(create_admin_country),
        )
        .route("/countries/:id", patch(update_admin_country))
        .route("/countries/:id/status", patch(update_admin_country_status))
        .route(
            "/news",
            get(list_admin_news_items).post(create_admin_news_item),
        )
        .route(
            "/news/:id",
            get(get_admin_news_item).patch(update_admin_news_item),
        )
        .route("/news/:id/status", patch(update_admin_news_status))
}

async fn list_admin_countries(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminCountriesQuery>,
) -> AppResult<Json<AdminCountriesResponse>> {
    Ok(Json(
        list_admin_countries_use_case(&mysql_pool(&state)?, query).await?,
    ))
}

async fn create_admin_country(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateAdminCountryRequest>,
) -> AppResult<Json<AdminCountryResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_admin_country_use_case(&mysql_pool(&state)?, admin_id, request).await?,
    ))
}

async fn update_admin_country(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(country_id): Path<u64>,
    Json(request): Json<UpdateAdminCountryRequest>,
) -> AppResult<Json<AdminCountryResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_admin_country_use_case(&mysql_pool(&state)?, admin_id, country_id, request).await?,
    ))
}

async fn update_admin_country_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(country_id): Path<u64>,
    Json(request): Json<UpdateAdminCountryStatusRequest>,
) -> AppResult<Json<AdminCountryResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_admin_country_status_use_case(&mysql_pool(&state)?, admin_id, country_id, request)
            .await?,
    ))
}

async fn list_admin_news_items(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewsQuery>,
) -> AppResult<Json<AdminNewsItemsResponse>> {
    Ok(Json(
        list_admin_news_items_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn get_admin_news_item(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(news_id): Path<u64>,
) -> AppResult<Json<AdminNewsItemResponse>> {
    Ok(Json(
        get_admin_news_item_use_case(state.mysql.clone(), news_id).await?,
    ))
}

async fn create_admin_news_item(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateAdminNewsItemRequest>,
) -> AppResult<Json<AdminNewsItemResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_admin_news_item_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn update_admin_news_item(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(news_id): Path<u64>,
    Json(request): Json<UpdateAdminNewsItemRequest>,
) -> AppResult<Json<AdminNewsItemResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_admin_news_item_use_case(state.mysql.clone(), admin_id, news_id, request).await?,
    ))
}

async fn update_admin_news_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(news_id): Path<u64>,
    Json(request): Json<UpdateAdminNewsStatusRequest>,
) -> AppResult<Json<AdminNewsItemResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_admin_news_status_use_case(state.mysql.clone(), admin_id, news_id, request).await?,
    ))
}
