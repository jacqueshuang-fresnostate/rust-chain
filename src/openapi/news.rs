use super::*;

#[derive(ToSchema)]
pub(super) struct NewsRichTextLeaf {
    text: String,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
}

#[derive(ToSchema)]
pub(super) struct NewsRichTextBlock {
    #[schema(pattern = "^(p|h1|h2|h3|blockquote)$")]
    r#type: String,
    children: Vec<NewsRichTextLeaf>,
}

#[derive(ToSchema)]
pub(super) struct NewsContentTranslation {
    locale: String,
    country_code: String,
    title: String,
    summary: Option<String>,
    content: Vec<NewsRichTextBlock>,
}

#[derive(ToSchema)]
pub(super) struct NewsContentDocument {
    version: u8,
    default_locale: String,
    items: Vec<NewsContentTranslation>,
}

#[derive(ToSchema)]
pub(super) struct AdminNewsItemResponse {
    id: u64,
    title: String,
    #[schema(pattern = "^(general|market|product|system|promotion)$")]
    category: String,
    #[schema(pattern = "^(draft|published|archived)$")]
    status: String,
    country_code: Option<String>,
    default_locale: String,
    content_json: NewsContentDocument,
    #[schema(format = Int64)]
    published_at: Option<i64>,
    created_by_admin_id: Option<u64>,
    updated_by_admin_id: Option<u64>,
    #[schema(format = Int64)]
    created_at: i64,
    #[schema(format = Int64)]
    updated_at: i64,
}

#[derive(ToSchema)]
pub(super) struct AdminNewsItemsResponse {
    news: Vec<AdminNewsItemResponse>,
    total: i64,
}

#[derive(ToSchema)]
pub(super) struct PublicNewsItemResponse {
    id: u64,
    title: String,
    banner_url: Option<String>,
    small_logo_url: Option<String>,
    #[schema(pattern = "^(general|market|product|system|promotion)$")]
    category: String,
    #[schema(pattern = "^published$")]
    status: String,
    country_code: Option<String>,
    default_locale: String,
    content_json: NewsContentDocument,
    #[schema(format = Int64)]
    published_at: Option<i64>,
    #[schema(format = Int64)]
    created_at: i64,
    #[schema(format = Int64)]
    updated_at: i64,
}

#[derive(ToSchema)]
pub(super) struct PublicNewsItemsResponse {
    news: Vec<PublicNewsItemResponse>,
}

#[derive(ToSchema)]
pub(super) struct CreateAdminNewsItemRequest {
    title: String,
    #[schema(pattern = "^(general|market|product|system|promotion)$")]
    category: String,
    #[schema(pattern = "^(draft|published|archived)$")]
    status: Option<String>,
    country_code: Option<String>,
    default_locale: String,
    content_json: NewsContentDocument,
    reason: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct UpdateAdminNewsItemRequest {
    title: String,
    #[schema(pattern = "^(general|market|product|system|promotion)$")]
    category: String,
    country_code: Option<String>,
    default_locale: String,
    content_json: NewsContentDocument,
    reason: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct UpdateAdminNewsStatusRequest {
    #[schema(pattern = "^(draft|published|archived)$")]
    status: String,
    reason: Option<String>,
}

#[utoipa::path(
    get,
    path = "/admin/api/v1/news",
    tag = "admin-news",
    summary = "查询后台新闻列表",
    params(
        ("status" = Option<String>, Query, description = "新闻状态"),
        ("category" = Option<String>, Query, description = "新闻分类"),
        ("country_code" = Option<String>, Query, description = "国家或地区代码"),
        ("locale" = Option<String>, Query, description = "语言代码"),
        ("q" = Option<String>, Query, description = "标题或内容关键词"),
        ("limit" = Option<u32>, Query, description = "分页数量"),
        ("offset" = Option<u32>, Query, description = "分页偏移")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AdminNewsItemsResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_admin_news() {}

#[utoipa::path(
    post,
    path = "/admin/api/v1/news",
    tag = "admin-news",
    summary = "创建后台新闻",
    request_body = CreateAdminNewsItemRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "创建成功", body = AdminNewsItemResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn create_admin_news() {}

#[utoipa::path(
    get,
    path = "/admin/api/v1/news/{id}",
    tag = "admin-news",
    summary = "查询后台新闻详情",
    params(("id" = u64, Path, description = "新闻 ID")),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AdminNewsItemResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "新闻不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_admin_news() {}

#[utoipa::path(
    patch,
    path = "/admin/api/v1/news/{id}",
    tag = "admin-news",
    summary = "更新后台新闻",
    params(("id" = u64, Path, description = "新闻 ID")),
    request_body = UpdateAdminNewsItemRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "更新成功", body = AdminNewsItemResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "新闻不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn update_admin_news() {}

#[utoipa::path(
    patch,
    path = "/admin/api/v1/news/{id}/status",
    tag = "admin-news",
    summary = "更新后台新闻状态",
    params(("id" = u64, Path, description = "新闻 ID")),
    request_body = UpdateAdminNewsStatusRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "更新成功", body = AdminNewsItemResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "新闻不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn update_admin_news_status() {}

#[utoipa::path(
    get,
    path = "/api/v1/news",
    tag = "news",
    summary = "查询用户端公开新闻列表",
    params(
        ("category" = Option<String>, Query, description = "新闻分类"),
        ("country_code" = Option<String>, Query, description = "国家或地区代码"),
        ("locale" = Option<String>, Query, description = "语言代码"),
        ("q" = Option<String>, Query, description = "标题或内容关键词"),
        ("limit" = Option<u32>, Query, description = "分页数量"),
        ("offset" = Option<u32>, Query, description = "分页偏移")
    ),
    responses(
        (status = 200, description = "查询成功", body = PublicNewsItemsResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_public_news() {}

#[utoipa::path(
    get,
    path = "/api/v1/news/{id}",
    tag = "news",
    summary = "查询用户端公开新闻详情",
    params(("id" = u64, Path, description = "新闻 ID")),
    responses(
        (status = 200, description = "查询成功", body = PublicNewsItemResponse),
        (status = 404, description = "新闻不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_public_news() {}
