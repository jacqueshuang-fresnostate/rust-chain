//! 第三方快速充值的 OpenAPI 契约：覆盖用户端下单与订单查询、支付回调，以及后台的通道配置与订单管理。
//! 下单成功返回上游收银台链接，用户在第三方页面付款后由异步回调推进订单状态并完成入账。
//! 回调端点不要求登录令牌，其安全性完全依赖签名校验，是本组唯一无需鉴权的写接口。
//! 上游返回的失败以 502 表达，用于和本服务自身的参数、配置类错误区分开。

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
pub(super) struct AdminQuickRechargeOrdersResponse {
    orders: Vec<QuickRechargeOrderResponse>,
    total: i64,
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

/// 返回用户端快速充值的可用配置，包括通道是否启用、可选金额区间与支持的支付方式。
/// 前端据此决定充值页是否展示快捷通道，返回内容中不含任何商户密钥。
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

/// 创建第三方支付的快速充值订单，成功后返回可直接跳转的收银台链接。
/// 通道未启用或金额超出限额返回参数错误；上游创建订单失败以 502 区分于本服务自身故障。
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

/// 查询当前用户的快速充值订单，可按状态过滤并限制返回条数，用于充值记录页。
/// 只返回令牌对应用户的订单，平台订单号与上游交易号一并给出，便于用户自助对账。
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

/// 接收第三方支付的异步回调并按签名校验来源，验签通过后推进订单状态并完成入账。
/// 该接口不要求登录令牌，安全性完全依赖验签；验签失败按参数错误拒绝，成功仅回一个约定字符串。
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

/// 后台查询快速充值通道配置，其中的密钥类字段以掩码形式展示，不会回显明文。
/// 主要用于配置页回填，读取本身不影响线上通道的启用状态。
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

/// 保存快速充值通道配置，含商户凭据与金额限制，必须填写审计原因才能提交。
/// 凭据字段留空表示保持原值不变，因此调整其他项时不需要重新输入密钥。
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

/// 用当前或本次提交的配置向上游发起一笔测试下单，验证商户凭据与网络连通性是否正常。
/// 上游失败以 502 呈现，便于与配置缺失、审计原因未填这类本地校验错误区分开。
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

/// 后台分页查询快速充值订单，支持按用户、邮箱、状态、平台订单号与上游交易号检索。
/// 这是核对到账情况与处理用户申诉的主要入口，返回内容比用户端多出运营所需字段。
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
        ("limit" = Option<u32>, Query, description = "分页数量"),
        ("offset" = Option<u32>, Query, description = "分页偏移")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AdminQuickRechargeOrdersResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_admin_quick_recharge_orders() {}

/// 删除一条尚未入账的快速充值订单，用于清理无效或测试产生的脏数据。
/// 已入账或已产生钱包流水的订单一律拒绝删除并返回冲突，保证资金记录不被破坏。
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
