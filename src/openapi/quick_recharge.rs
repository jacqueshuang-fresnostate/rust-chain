use super::*;

#[derive(ToSchema)]
pub(super) struct UserQuickRechargeConfigResponse {
    enabled: bool,
    currency: String,
    token: String,
    network: String,
    min_amount: String,
    max_amount: Option<String>,
}

#[derive(ToSchema)]
#[schema(rename_all = "snake_case")]
pub(super) enum QuickRechargeReturnTarget {
    PcApp,
    MacApp,
    IosApp,
    AndroidApp,
    MobileWeb,
    DesktopWeb,
}

#[derive(ToSchema)]
pub(super) struct CreateQuickRechargeOrderRequest {
    amount: String,
    return_target: Option<QuickRechargeReturnTarget>,
}

#[derive(ToSchema)]
pub(super) struct QuickRechargeOrderResponse {
    id: u64,
    order_id: String,
    user_id: u64,
    user_email: Option<String>,
    asset_id: u64,
    asset_symbol: String,
    currency: String,
    token: String,
    network: String,
    fiat_amount: String,
    actual_amount: Option<String>,
    provider_trade_id: Option<String>,
    receive_address: Option<String>,
    payment_url: Option<String>,
    return_target: Option<String>,
    redirect_url: Option<String>,
    expiration_time: Option<i64>,
    status: String,
    block_transaction_id: Option<String>,
    #[schema(format = Int64)]
    paid_at: Option<i64>,
    #[schema(format = Int64)]
    created_at: i64,
    #[schema(format = Int64)]
    updated_at: i64,
}

#[derive(ToSchema)]
pub(super) struct QuickRechargeOrdersResponse {
    orders: Vec<QuickRechargeOrderResponse>,
}

#[derive(ToSchema)]
pub(super) struct SaveQuickRechargeConfigRequest {
    enabled: bool,
    api_base_url: Option<String>,
    merchant_pid: Option<String>,
    merchant_secret: Option<String>,
    currency: String,
    token: String,
    network: String,
    notify_url: Option<String>,
    redirect_url: Option<String>,
    pc_app_redirect_url: Option<String>,
    mac_app_redirect_url: Option<String>,
    ios_app_redirect_url: Option<String>,
    android_app_redirect_url: Option<String>,
    mobile_web_redirect_url: Option<String>,
    desktop_web_redirect_url: Option<String>,
    min_amount: String,
    max_amount: Option<String>,
    reason: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct QuickRechargeConfigResponse {
    id: u64,
    name: String,
    provider: String,
    enabled: bool,
    api_base_url: Option<String>,
    merchant_pid: Option<String>,
    merchant_secret_mask: Option<String>,
    merchant_secret_set: bool,
    currency: String,
    token: String,
    network: String,
    notify_url: Option<String>,
    redirect_url: Option<String>,
    pc_app_redirect_url: Option<String>,
    mac_app_redirect_url: Option<String>,
    ios_app_redirect_url: Option<String>,
    android_app_redirect_url: Option<String>,
    mobile_web_redirect_url: Option<String>,
    desktop_web_redirect_url: Option<String>,
    min_amount: String,
    max_amount: Option<String>,
    updated_by: Option<u64>,
    #[schema(format = Int64)]
    created_at: i64,
    #[schema(format = Int64)]
    updated_at: i64,
}

#[derive(ToSchema)]
pub(super) struct TestQuickRechargeConfigRequest {
    amount: String,
    reason: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct TestQuickRechargeConfigResponse {
    order_id: String,
    provider_trade_id: String,
    currency: String,
    token: String,
    network: String,
    fiat_amount: String,
    actual_amount: String,
    receive_address: String,
    payment_url: String,
    expiration_time: Option<i64>,
    #[schema(format = Int64)]
    tested_at: i64,
}

#[derive(ToSchema)]
pub(super) struct DeleteQuickRechargeOrderRequest {
    reason: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct GmpayNotifyRequest {
    pid: String,
    trade_id: String,
    order_id: String,
    amount: String,
    actual_amount: String,
    receive_address: Option<String>,
    token: String,
    block_transaction_id: Option<String>,
    status: String,
    signature: String,
}

#[utoipa::path(
    get,
    path = "/api/v1/wallet/quick-recharge/config",
    tag = "wallet",
    summary = "查询用户端快速充值配置",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = UserQuickRechargeConfigResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_user_quick_recharge_config() {}

#[utoipa::path(
    post,
    path = "/api/v1/wallet/quick-recharge/orders",
    tag = "wallet",
    summary = "创建 GMPay/Epusdt 快速充值订单",
    request_body = CreateQuickRechargeOrderRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "创建成功并返回 GMPay 收银台链接", body = QuickRechargeOrderResponse),
        (status = 400, description = "参数错误、配置未启用或金额超出限制", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 502, description = "GMPay 创建订单失败", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn create_user_quick_recharge_order() {}

#[utoipa::path(
    get,
    path = "/api/v1/wallet/quick-recharge/orders",
    tag = "wallet",
    summary = "查询当前用户快速充值订单",
    params(
        ("status" = Option<String>, Query, description = "订单状态"),
        ("limit" = Option<u32>, Query, description = "分页数量")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = QuickRechargeOrdersResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_user_quick_recharge_orders() {}

#[utoipa::path(
    post,
    path = "/api/v1/payments/gmpay/notify",
    tag = "wallet",
    summary = "GMPay/Epusdt 快速充值异步回调",
    request_body = GmpayNotifyRequest,
    responses(
        (status = 200, description = "回调验签成功并返回 ok"),
        (status = 400, description = "验签失败或回调参数无效", body = ErrorResponse),
        (status = 404, description = "订单不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn gmpay_notify() {}

#[utoipa::path(
    get,
    path = "/admin/api/v1/quick-recharge/config",
    tag = "admin-wallet",
    summary = "查询后台快速充值配置",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = QuickRechargeConfigResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_admin_quick_recharge_config() {}

#[utoipa::path(
    patch,
    path = "/admin/api/v1/quick-recharge/config",
    tag = "admin-wallet",
    summary = "保存后台快速充值配置",
    request_body = SaveQuickRechargeConfigRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "保存成功", body = QuickRechargeConfigResponse),
        (status = 400, description = "参数错误或缺少审计原因", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn save_admin_quick_recharge_config() {}

#[utoipa::path(
    post,
    path = "/admin/api/v1/quick-recharge/config/test",
    tag = "admin-wallet",
    summary = "测试后台快速充值配置",
    request_body = TestQuickRechargeConfigRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "测试成功并返回 GMPay 收银台信息", body = TestQuickRechargeConfigResponse),
        (status = 400, description = "参数错误、配置缺失或缺少审计原因", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 502, description = "GMPay 创建测试订单失败", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn test_admin_quick_recharge_config() {}

#[utoipa::path(
    get,
    path = "/admin/api/v1/quick-recharge/orders",
    tag = "admin-wallet",
    summary = "查询快速充值订单",
    params(
        ("user_id" = Option<u64>, Query, description = "用户 ID"),
        ("email" = Option<String>, Query, description = "用户邮箱"),
        ("status" = Option<String>, Query, description = "订单状态"),
        ("order_id" = Option<String>, Query, description = "平台订单号"),
        ("provider_trade_id" = Option<String>, Query, description = "GMPay 交易号"),
        ("limit" = Option<u32>, Query, description = "分页数量")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = QuickRechargeOrdersResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_admin_quick_recharge_orders() {}

#[utoipa::path(
    delete,
    path = "/admin/api/v1/quick-recharge/orders/{order_id}",
    tag = "admin-wallet",
    summary = "删除未入账的快速充值订单",
    params(("order_id" = String, Path, description = "平台订单号")),
    request_body = DeleteQuickRechargeOrderRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 204, description = "删除成功"),
        (status = 400, description = "缺少审计原因", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "订单不存在", body = ErrorResponse),
        (status = 409, description = "订单已入账或存在钱包流水", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn delete_admin_quick_recharge_order() {}
