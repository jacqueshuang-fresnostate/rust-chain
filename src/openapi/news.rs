//! 新闻中心的 OpenAPI 契约：同时覆盖后台的内容管理接口与用户端的公开阅读接口。
//! 正文以结构化富文本块表达而非 HTML 字符串，并按语言分组存放，便于同一篇新闻多语言共存。
//! 后台接口需要后台作用域令牌且能看到草稿与归档，用户端接口无需登录且只返回已发布内容。

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

/// 后台分页查询新闻列表，可按状态、分类、国家与语言过滤，也支持标题或正文关键词检索。
/// 与用户端不同，草稿和已归档的新闻在这里同样可见，便于编辑继续处理尚未发布的内容。
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

/// 创建一条多语言新闻，正文以结构化富文本块数组提交而不是整段 HTML 字符串。
/// 状态可在创建时直接指定，未指定按草稿处理；可附审计原因说明本次新增的缘由。
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

/// 按主键查询后台新闻详情，返回完整的多语言内容文档以及创建人与最后修改人。
/// 这里不做状态过滤，草稿与归档同样可读，主要供编辑页回填表单使用。
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

/// 整体覆盖新闻的标题、分类、归属国家、默认语言与多语言正文，不做字段级增量合并。
/// 状态不在本接口修改，发布与归档需要走单独的状态变更接口。
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

/// 在草稿、已发布与已归档之间切换新闻状态，这是内容对用户端是否可见的唯一开关。
/// 可附审计原因，便于事后追溯某篇新闻为何被下架或者重新发布。
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

/// 用户端公开查询新闻列表，无需登录，只返回已发布内容，草稿与归档不会出现。
/// 支持按分类、国家与语言过滤，响应中附带横幅与小图标地址供列表页直接渲染。
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

/// 用户端公开查询单篇新闻详情，未发布的内容一律按不存在处理，不泄露其是否真的存在。
/// 返回结构与列表项一致并包含完整多语言正文，可直接用于详情页渲染。
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
