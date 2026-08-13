//! quick_recharge 路由层。
//!
//! 负责用户端、管理员端、公共回调的路由适配，不承载业务规则。
//! 三组路由的鉴权级别截然不同：用户路由由 `UserAuth` 把守并把令牌 subject 透传给用例解析用户编号；
//! 管理员路由由 `AdminAuth` 把守，写操作还会把 subject 透传下去写审计操作人；
//! 公共回调路由没有任何鉴权中间件，它的身份认证完全依赖请求体内的 GMPay 签名，
//! 因此该入口必须保持无副作用直至验签通过，任何在此之前的资金动作都会被伪造回调利用。
//! 涉及商户密钥的三个入口会把凭据加密主密钥一并透传，用于配置层解密或加密密钥字段；
//! 本层自身不解密、不签名、不拼接支付参数，也不直接访问数据库。

use crate::{
    error::AppResult,
    modules::auth::{AdminAuth, UserAuth},
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post},
};
use serde_json::Value;

use super::{
    AdminQuickRechargeOrdersResponse, CreateQuickRechargeOrderRequest,
    DeleteQuickRechargeOrderRequest, QuickRechargeConfigResponse, QuickRechargeOrderResponse,
    QuickRechargeOrdersQuery, QuickRechargeOrdersResponse, SaveQuickRechargeConfigRequest,
    TestQuickRechargeConfigRequest, TestQuickRechargeConfigResponse,
    UserQuickRechargeConfigResponse,
};

/// 用户端快速充值相关路由，挂载在已完成用户鉴权的路由树下。
/// 配置路径只读，返回渠道开关与金额区间；订单路径上 GET 查自己的充值订单、POST 发起新的充值下单。
/// 用户侧没有任何入账入口，余额只会因验签通过的支付回调而变化。
pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/wallet/quick-recharge/config",
            get(get_user_quick_recharge_config),
        )
        .route(
            "/wallet/quick-recharge/orders",
            get(list_user_quick_recharge_orders).post(create_user_quick_recharge_order),
        )
}

/// 管理端快速充值路由，覆盖渠道配置读写、连通性测试与订单查询清理。
/// 配置路径 GET 读脱敏配置、PATCH 保存整份配置；测试路径单独设子路由，因为它会向支付方发起真实建单请求。
/// 订单侧只有列表查询和按本地订单号删除两个入口，删除仅限未支付订单。
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/quick-recharge/config",
            get(get_admin_quick_recharge_config).patch(save_admin_quick_recharge_config),
        )
        .route(
            "/quick-recharge/config/test",
            post(test_admin_quick_recharge_config),
        )
        .route(
            "/quick-recharge/orders",
            get(list_admin_quick_recharge_orders),
        )
        .route(
            "/quick-recharge/orders/:order_id",
            delete(delete_admin_quick_recharge_order),
        )
}

/// GMPay 异步回调的公开路由，必须挂在不带用户或管理员鉴权的路由树下，否则支付方无法访问。
/// 该入口是外部可直接触达的资金链路起点，其身份校验完全由请求体中的 MD5 签名承担；
/// 因此路由层不得在此附加任何会改动数据的中间件，所有判定都留给验签之后的用例执行。
pub fn public_routes() -> Router<AppState> {
    Router::new().route("/payments/gmpay/notify", post(handle_gmpay_notify))
}

/// 返回用户可见的充值渠道信息，仅含开关、币种、网络与单笔金额区间。
/// 要求登录但不使用令牌内容，因为配置是全局单例而非按用户区分，提取器在此只起访问控制作用。
/// 不透传加密主密钥，这条路径无需接触商户密钥。
async fn get_user_quick_recharge_config(
    _auth: UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<UserQuickRechargeConfigResponse>> {
    Ok(Json(
        super::application::get_user_quick_recharge_config(state.mysql.clone()).await?,
    ))
}

/// 查询当前用户的充值订单列表，支持按订单状态筛选并限制返回条数。
/// 把令牌 subject 原样透传给用例解析用户编号，路由层不自行解析也不接受请求参数里的用户维度字段，
/// 因此不存在通过改查询串查看他人订单的可能。
async fn list_user_quick_recharge_orders(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<QuickRechargeOrdersQuery>,
) -> AppResult<Json<QuickRechargeOrdersResponse>> {
    Ok(Json(
        super::application::list_user_quick_recharge_orders(
            state.mysql.clone(),
            &claims.sub,
            query,
        )
        .await?,
    ))
}

/// 发起一笔快速充值下单，返回含支付地址与收款信息的订单，供前端跳转或展示收款二维码。
/// 需要透传加密主密钥，因为用例要解密商户密钥来对下单请求签名。
/// 本接口只创建订单并调用支付方，不会增加用户余额；到账由后续的异步回调完成。
/// 没有请求级幂等键，客户端重复提交会生成新的订单号并产生另一笔外部支付请求，前端需自行防抖。
async fn create_user_quick_recharge_order(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateQuickRechargeOrderRequest>,
) -> AppResult<Json<QuickRechargeOrderResponse>> {
    Ok(Json(
        super::application::create_user_quick_recharge_order(
            state.mysql.clone(),
            state.settings.exposed_credential_encryption_key(),
            &claims.sub,
            request,
        )
        .await?,
    ))
}

/// 返回后台可见的渠道完整配置，供管理页展示与编辑表单回填。
/// 只校验管理员身份而不取编号，因为纯读取无需写审计；商户密钥在用例层已被替换为掩码。
/// 同样不透传加密主密钥，避免只读接口无谓地接触密钥材料。
async fn get_admin_quick_recharge_config(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<QuickRechargeConfigResponse>> {
    Ok(Json(
        super::application::get_admin_quick_recharge_config(state.mysql.clone()).await?,
    ))
}

/// 保存整份渠道配置，返回保存后的脱敏配置视图。
/// 透传加密主密钥用于加密新提交的商户密钥；请求体中密钥字段留空表示不更换，旧密文与掩码原样保留。
/// 管理员编号来自令牌，会作为审计操作人与配置的最后修改人落库。
/// 配置更新与审计在同一事务提交，校验或加密失败都不会留下半生效配置。
async fn save_admin_quick_recharge_config(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<SaveQuickRechargeConfigRequest>,
) -> AppResult<Json<QuickRechargeConfigResponse>> {
    Ok(Json(
        super::application::save_admin_quick_recharge_config(
            state.mysql.clone(),
            state.settings.exposed_credential_encryption_key(),
            &claims.sub,
            request,
        )
        .await?,
    ))
}

/// 用当前保存的配置向支付方发起一次真实建单，验证商户号、密钥与签名口径是否可用。
/// 这不是模拟调用：会在支付方侧产生一笔真实测试订单，但不落本地充值订单，也不改动任何用户钱包。
/// 允许在渠道未启用时执行，便于上线前先行验证；测试结果与当时配置快照一并写入管理员审计。
/// 每次调用都会生成新的订单号并产生新的外部请求，不具备幂等重放语义。
async fn test_admin_quick_recharge_config(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<TestQuickRechargeConfigRequest>,
) -> AppResult<Json<TestQuickRechargeConfigResponse>> {
    Ok(Json(
        super::application::test_admin_quick_recharge_config(
            state.mysql.clone(),
            state.settings.exposed_credential_encryption_key(),
            &claims.sub,
            request,
        )
        .await?,
    ))
}

/// 查询后台充值订单列表并返回匹配总数，支持按用户、邮箱、状态、本地订单号与支付方交易号筛选。
/// 后两个筛选项是掉单排查的主要抓手，可从任一侧订单号反查另一侧。
/// 只校验管理员身份不取编号，因为查询不写审计；本接口不触发任何支付方调用或状态变更。
async fn list_admin_quick_recharge_orders(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<QuickRechargeOrdersQuery>,
) -> AppResult<Json<AdminQuickRechargeOrdersResponse>> {
    Ok(Json(
        super::application::list_admin_quick_recharge_orders(state.mysql.clone(), query).await?,
    ))
}

/// 删除一笔未支付的充值订单，成功返回 204 且响应体为空。
/// 路径参数是本地订单号字符串而非自增主键，与用户和支付方看到的订单标识一致。
/// DELETE 携带请求体是因为审计原因必填；用例会拒绝已支付或已产生钱包流水的订单，此时返回冲突。
async fn delete_admin_quick_recharge_order(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<String>,
    Json(request): Json<DeleteQuickRechargeOrderRequest>,
) -> AppResult<StatusCode> {
    super::application::delete_admin_quick_recharge_order(
        state.mysql.clone(),
        &claims.sub,
        &order_id,
        request,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// 接收 GMPay 支付结果异步通知，是充值资金真正入账的唯一入口，处于公开可访问的路径上。
/// 请求体按任意 JSON 反序列化而不绑定强类型结构，因为签名要对报文中实际出现的全部字段计算，
/// 提前投影成固定结构会丢掉参与签名的未知字段而导致验签失败。
/// 透传加密主密钥供用例解密商户密钥后验签；调用方身份完全由签名决定，本层不做任何鉴权。
/// 成功时返回纯文本 `ok`，这是支付方判定通知已被受理、不再重投的约定应答；
/// 任何错误都通过 `AppError` 转成非 200 状态，促使支付方按其重试策略再次投递。
async fn handle_gmpay_notify(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> AppResult<&'static str> {
    super::application::handle_gmpay_notify(
        state.mysql.clone(),
        state.settings.exposed_credential_encryption_key(),
        payload,
    )
    .await?;
    Ok("ok")
}
