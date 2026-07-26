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

#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    summary = "服务健康检查",
    responses((status = 200, description = "服务可用", body = HealthResponse))
)]
fn health() {}

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
