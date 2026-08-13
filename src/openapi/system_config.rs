//! 系统配置的 OpenAPI 契约：汇集健康检查、国家与语言配置、平台品牌配置和邮件服务配置四类接口。
//! 国家与品牌两组各有一个无需登录的公开读接口，返回内容仅限前端渲染所需，管理端读写另走后台路径。
//! 邮件服务支持多套配置并存，发信策略与具体服务器配置分开保存，口令类字段一律掩码返回、留空即沿用原值。

use super::*;

#[derive(ToSchema)]
pub(super) struct PublicCountryResponse {
    country_code: String,
    country_name: String,
    default_locale: String,
    supported_locales: Vec<String>,
}

#[derive(ToSchema)]
pub(super) struct PublicCountriesResponse {
    countries: Vec<PublicCountryResponse>,
}

#[derive(ToSchema)]
pub(super) struct PlatformBrandResponse {
    id: u64,
    name: String,
    platform_name: String,
    logo_url: Option<String>,
    chart_provider: String,
    updated_by: Option<u64>,
    #[schema(format = Int64)]
    created_at: i64,
    #[schema(format = Int64)]
    updated_at: i64,
}

#[derive(ToSchema)]
pub(super) struct SavePlatformBrandRequest {
    platform_name: String,
    logo_url: Option<String>,
    chart_provider: Option<String>,
    reason: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct AdminCountryResponse {
    id: u64,
    country_code: String,
    country_name: String,
    remark: String,
    default_locale: String,
    supported_locales: Vec<String>,
    registration_enabled: bool,
    #[schema(pattern = "^(active|disabled)$")]
    status: String,
    sort_order: i32,
    #[schema(format = Int64)]
    created_at: i64,
    #[schema(format = Int64)]
    updated_at: i64,
}

#[derive(ToSchema)]
pub(super) struct AdminCountriesResponse {
    countries: Vec<AdminCountryResponse>,
    total: i64,
}

#[derive(ToSchema)]
pub(super) struct CreateAdminCountryRequest {
    country_code: String,
    country_name: String,
    remark: String,
    default_locale: String,
    supported_locales: Vec<String>,
    registration_enabled: bool,
    #[schema(pattern = "^(active|disabled)$")]
    status: Option<String>,
    sort_order: Option<i32>,
    reason: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct UpdateAdminCountryRequest {
    country_name: String,
    remark: String,
    default_locale: String,
    supported_locales: Vec<String>,
    registration_enabled: bool,
    sort_order: Option<i32>,
    reason: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct UpdateAdminCountryStatusRequest {
    #[schema(pattern = "^(active|disabled)$")]
    status: String,
    reason: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct SaveSmtpConfigRequest {
    name: Option<String>,
    host: String,
    port: u16,
    #[schema(pattern = "^(none|starttls|tls)$")]
    security: String,
    username: Option<String>,
    #[schema(nullable = true)]
    password: Option<String>,
    from_email: String,
    from_name: Option<String>,
    verification_code_template_html: Option<String>,
    verification_code_templates: Option<Vec<VerificationCodeTemplate>>,
    enabled: bool,
    priority: Option<u32>,
    reason: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct VerificationCodeTemplate {
    key: String,
    name: String,
    purpose: Option<String>,
    html: String,
    enabled: bool,
}

#[derive(ToSchema)]
pub(super) struct SmtpConfigResponse {
    id: u64,
    name: String,
    host: String,
    port: u16,
    security: String,
    username_mask: Option<String>,
    password_set: bool,
    from_email: String,
    from_name: Option<String>,
    verification_code_template_html: Option<String>,
    verification_code_templates: Vec<VerificationCodeTemplate>,
    enabled: bool,
    priority: u32,
}

#[derive(ToSchema)]
pub(super) struct SmtpDeliverySettingsResponse {
    #[schema(pattern = "^(priority|round_robin)$")]
    strategy: String,
}

#[derive(ToSchema)]
pub(super) struct SmtpConfigListResponse {
    configs: Vec<SmtpConfigResponse>,
    delivery_settings: SmtpDeliverySettingsResponse,
}

#[derive(ToSchema)]
pub(super) struct SaveSmtpDeliverySettingsRequest {
    #[schema(pattern = "^(priority|round_robin)$")]
    strategy: String,
    reason: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct SendSmtpTestRequest {
    recipient: String,
    config_id: Option<u64>,
    reason: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct SendSmtpTestResponse {
    sent: bool,
    recipient: String,
    config_id: u64,
    config_name: String,
}

/// 在文档中登记根路径上的存活探针，说明它无需任何鉴权即可访问。
/// 真实实现只返回固定的正常状态，不探测数据库与缓存，因此不能用来判断依赖是否可用。
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    summary = "服务健康检查",
    responses((status = 200, description = "服务可用", body = HealthResponse))
)]
fn health() {}

/// 公开返回可注册国家及其默认语言，注册页据此渲染国家下拉与默认语言选项。
/// 只包含后台已启用的条目，无需登录即可访问，也不含任何后台专用字段。
#[utoipa::path(
    get,
    path = "/api/v1/countries",
    tag = "countries",
    summary = "查询可注册国家和默认语言",
    responses(
        (status = 200, description = "查询成功", body = PublicCountriesResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_public_countries() {}

/// 公开返回桌面端品牌与行情图表相关配置，供前端在未登录状态下完成首屏渲染。
/// 内容来自后台品牌配置，只暴露展示所需字段，不含任何管理端设置项。
#[utoipa::path(
    get,
    path = "/api/v1/platform/brand",
    tag = "platform",
    summary = "查询 PC 端品牌与 K 线图配置",
    responses(
        (status = 200, description = "查询成功", body = PlatformBrandResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_public_platform_brand() {}

/// 后台分页查询国家配置，可按代码、名称与状态等条件筛选，结果包含未启用条目。
/// 与用户端接口的差别在于这里能看到停用记录，便于重新启用或继续修改。
#[utoipa::path(
    get,
    path = "/admin/api/v1/countries",
    tag = "admin-countries",
    summary = "查询后台国家配置",
    params(
        ("country_code" = Option<String>, Query, description = "国家或地区代码"),
        ("status" = Option<String>, Query, description = "配置状态"),
        ("registration_enabled" = Option<bool>, Query, description = "是否开放注册"),
        ("limit" = Option<u32>, Query, description = "分页数量"),
        ("offset" = Option<u32>, Query, description = "分页偏移")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AdminCountriesResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_admin_countries() {}

/// 新增一条国家配置，包含国家代码、名称与默认语言等信息。
/// 国家代码在全局唯一，重复提交返回冲突而不是覆盖既有配置。
#[utoipa::path(
    post,
    path = "/admin/api/v1/countries",
    tag = "admin-countries",
    summary = "创建后台国家配置",
    request_body = CreateAdminCountryRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "创建成功", body = AdminCountryResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 409, description = "国家代码已存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn create_admin_country() {}

/// 更新指定国家配置的名称、区号或默认语言等字段。
/// 启停状态不在此处调整，需要走单独的状态接口；记录不存在返回资源不存在。
#[utoipa::path(
    patch,
    path = "/admin/api/v1/countries/{id}",
    tag = "admin-countries",
    summary = "更新后台国家配置",
    params(("id" = u64, Path, description = "国家配置 ID")),
    request_body = UpdateAdminCountryRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "更新成功", body = AdminCountryResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "国家配置不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn update_admin_country() {}

/// 启用或停用某个国家配置，直接决定它是否出现在用户端的可注册国家列表里。
/// 停用不会影响已注册用户，只影响后续注册与前端展示。
#[utoipa::path(
    patch,
    path = "/admin/api/v1/countries/{id}/status",
    tag = "admin-countries",
    summary = "更新后台国家配置状态",
    params(("id" = u64, Path, description = "国家配置 ID")),
    request_body = UpdateAdminCountryStatusRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "更新成功", body = AdminCountryResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "国家配置不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn update_admin_country_status() {}

/// 查询当前生效的单条邮件服务器配置，口令字段以掩码返回，不回显明文。
/// 尚未配置过任何邮件服务器时返回资源不存在，前端应引导先完成新增。
#[utoipa::path(
    get,
    path = "/admin/api/v1/smtp/config",
    tag = "admin-smtp",
    summary = "查询 SMTP 配置",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = SmtpConfigResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "配置不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_smtp_config() {}

/// 查询全部邮件服务器配置及发信策略，支持多套配置并存以便按用途或轮换发信。
/// 与单条查询的区别在于这里一次给出全量列表和策略，用于配置管理页整体展示。
#[utoipa::path(
    get,
    path = "/admin/api/v1/smtp/configs",
    tag = "admin-smtp",
    summary = "查询 SMTP 配置列表与发信策略",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = SmtpConfigListResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_smtp_configs() {}

/// 新增一套邮件服务器配置，包含主机、端口、安全模式、发件人与可选认证凭据。
/// 安全模式取值限于明文、STARTTLS 与 TLS 三种，未知取值会被直接拒绝。
#[utoipa::path(
    post,
    path = "/admin/api/v1/smtp/configs",
    tag = "admin-smtp",
    summary = "新增 SMTP 配置",
    request_body = SaveSmtpConfigRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "新增成功", body = SmtpConfigResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn create_smtp_config() {}

/// 按主键更新某一套邮件服务器配置，口令留空表示沿用原值，不必每次重填。
/// 配置不存在返回资源不存在，参数不合法则返回参数错误。
#[utoipa::path(
    patch,
    path = "/admin/api/v1/smtp/configs/{id}",
    tag = "admin-smtp",
    summary = "更新 SMTP 配置",
    params(("id" = u64, Path, description = "配置 ID")),
    request_body = SaveSmtpConfigRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "更新成功", body = SmtpConfigResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "配置不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn update_smtp_config() {}

/// 保存单套邮件服务器配置的兼容入口，供只维护一套服务器的旧版后台使用。
/// 与按主键更新的区别在于这里不指定标识，直接落到当前那一套配置上。
#[utoipa::path(
    patch,
    path = "/admin/api/v1/smtp/config",
    tag = "admin-smtp",
    summary = "保存 SMTP 配置",
    request_body = SaveSmtpConfigRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "保存成功", body = SmtpConfigResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn save_smtp_config() {}

/// 保存发信策略，例如多套配置之间如何选择与分配，不改动任何一套服务器凭据。
/// 策略与配置分开保存，可以在完全不触碰口令的前提下调整发信行为。
#[utoipa::path(
    patch,
    path = "/admin/api/v1/smtp/delivery-settings",
    tag = "admin-smtp",
    summary = "保存 SMTP 发信策略",
    request_body = SaveSmtpDeliverySettingsRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "保存成功", body = SmtpDeliverySettingsResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn save_smtp_delivery_settings() {}

/// 用当前启用的配置向指定地址发一封测试邮件，验证邮件服务器是否真的可用。
/// 没有启用中的配置时返回资源不存在；发送成功只代表服务器已接收，不保证最终进入收件箱。
#[utoipa::path(
    post,
    path = "/admin/api/v1/smtp/test",
    tag = "admin-smtp",
    summary = "发送 SMTP 测试邮件",
    request_body = SendSmtpTestRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "发送成功", body = SendSmtpTestResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "启用配置不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn send_smtp_test() {}

/// 后台查询桌面端品牌与行情图表配置，返回内容与公开接口同源但需要后台令牌。
/// 主要用于配置页回填，读取过程不会产生任何变更。
#[utoipa::path(
    get,
    path = "/admin/api/v1/platform/brand",
    tag = "admin-platform",
    summary = "查询 PC 品牌与 K 线图配置",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = PlatformBrandResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_admin_platform_brand() {}

/// 保存桌面端品牌与行情图表配置，保存后用户端公开接口会立即读到新值。
/// 参数不合法返回参数错误；本接口不做灰度，变更对所有访客同时生效。
#[utoipa::path(
    patch,
    path = "/admin/api/v1/platform/brand",
    tag = "admin-platform",
    summary = "保存 PC 品牌与 K 线图配置",
    request_body = SavePlatformBrandRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "保存成功", body = PlatformBrandResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn save_admin_platform_brand() {}
