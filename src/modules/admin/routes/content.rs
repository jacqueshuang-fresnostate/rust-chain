//! 承载国家地区配置与站内新闻内容管理的 HTTP 传输入口。
//!
//! 两组资源都属于运营内容而非资金链路，因此没有任何余额或锁仓副作用，但都会写后台审计。
//! 国家配置决定注册准入与可选语言，新闻则带有富文本正文和 draft、published、archived 三态。
//! 值得注意的是这两组入口获取连接池的方式不同：国家侧用 `mysql_pool(&state)` 在传输层先解析出池引用再传引用，
//! 新闻侧沿用 `state.mysql.clone()` 把可选池交给用例自行解析；二者只是参数形态差异，鉴权与错误映射完全一致。

use super::*;

/// 构建国家配置与后台新闻管理的传输路由。
///
/// 路由保持管理员鉴权、原有 Path/Query/JSON DTO 及 HTTP 方法，仅把管理员审计主体和
/// 已解析输入转发给对应应用用例；国家编码、语言内容和新闻状态规则仍由下层校验，
/// 所有失败按既有统一错误响应返回。国家与新闻都把「改内容」和「改状态」拆成了两个入口，
/// 避免一次请求同时改动展示内容与对外可见性。
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

/// 处理 GET /countries，按国家代码、状态和注册开关筛选国家配置。
/// 代码与状态按写入口径先规范化再匹配，响应含各国家的支持语言集合；查询不加锁，也不改变注册策略。
async fn list_admin_countries(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminCountriesQuery>,
) -> AppResult<Json<AdminCountriesResponse>> {
    Ok(Json(
        list_admin_countries_use_case(&mysql_pool(&state)?, query).await?,
    ))
}

/// 处理 POST /countries，新增国家的注册准入、默认语言与支持语言配置。
/// 状态缺省为 active、排序缺省为 0；国家代码撞唯一约束会整体回滚，该接口没有幂等键。
/// 新增国家只影响后续注册判定，不会迁移已注册用户，也不会刷新任何外部地域服务。
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

/// 处理 PATCH /countries/:id，修改国家名称、备注、语言集合、注册开关与排序。
/// 请求必须携带审计原因；国家代码与启用状态不在此入口修改，状态需走专用的状态入口。
/// 调整支持语言不会改动已注册用户此前选定的语言，仅影响后续的可选范围。
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

/// 处理 PATCH /countries/:id/status，单独切换国家的启用与停用状态。
/// 请求必须携带审计原因；停用不会检查该国已有多少注册用户，也不会迁移或封禁这些账号。
/// 状态提交后由注册读取端在后续请求中自行观察，本入口不主动失效任何缓存。
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

/// 处理 GET /news，按状态、分类、国家、语言和关键字检索站内新闻。
/// 返回的是不含完整富文本的摘要列表，正文需另行按编号读取详情；关键字仅做去空白后匹配。
async fn list_admin_news_items(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewsQuery>,
) -> AppResult<Json<AdminNewsItemsResponse>> {
    Ok(Json(
        list_admin_news_items_use_case(state.mysql.clone(), query).await?,
    ))
}

/// 处理 GET /news/:id，读取单条新闻的标题、封面、分类、地域、语言、富文本正文与发布时间。
/// 与列表入口相比这里才会返回完整正文文档；读取不加锁，也不会累计阅读量或改变发布状态。
async fn get_admin_news_item(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(news_id): Path<u64>,
) -> AppResult<Json<AdminNewsItemResponse>> {
    Ok(Json(
        get_admin_news_item_use_case(state.mysql.clone(), news_id).await?,
    ))
}

/// 处理 POST /news，新建一条站内新闻及其结构化富文本正文。
/// 状态缺省为 draft，若创建时就直接指定为已发布，应用层会把当前时间写作首次发布时间。
/// 创建不发送站内信也不广播事件，且没有幂等键，重复提交会产生内容相同的多条新闻。
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

/// 处理 PATCH /news/:id，修改新闻的标题、封面、分类、地域、默认语言与富文本正文。
/// 请求必须携带审计原因；该入口刻意不改动发布状态与首次发布时间，上下线需走状态入口。
/// 替换封面不会清理对象存储中此前上传的旧图片，历史图片需另行治理。
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

/// 处理 PATCH /news/:id/status，在草稿、已发布与已归档之间切换新闻对外可见性。
/// 请求必须携带审计原因；只有首次进入已发布且原本没有发布时间时才写入当前时间，
/// 因此归档后重新发布会保留最初的首发时间。状态变更不发送任何通知，也不触发外部缓存失效。
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
