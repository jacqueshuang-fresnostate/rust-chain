//! seconds_contract bounded context HTTP routing layer.
//!
//! 路由层：把秒合约的用户接口与后台接口挂载到 axum，并完成鉴权提取、参数归一和用例分发。
//! 用户侧只开放产品目录、订单历史和开仓三个入口；产品配置、订单查询与人工结算集中在后台侧，
//! 两组路由分别由 `UserAuth` 与 `AdminAuth` 提取器把守，令牌 subject 再经服务层解析出用户或管理员编号，
//! 因此用户令牌无法命中后台接口，反之亦然。
//! 本层不写业务规则也不直接访问数据库：只读接口取连接池后交给用例函数，写接口把 `AppState` 中的
//! MySQL、Redis 与事件广播句柄一并透传，由应用层在事务内编排并在提交后发布事件。
//! 所有处理函数返回 `AppResult`，错误到状态码的映射由全局 `AppError` 统一负责，此处不做分支处理。

use super::{
    application::mysql_pool,
    application::{
        create_product as create_product_use_case, delete_product as delete_product_use_case,
        get_admin_order as get_admin_order_use_case,
        get_admin_product as get_admin_product_use_case,
        list_active_products as list_active_products_use_case,
        list_admin_orders as list_admin_orders_use_case,
        list_admin_products as list_admin_products_use_case,
        list_user_orders as list_user_orders_use_case,
        open_order_with_events as open_order_with_events_use_case,
        settle_order_with_events as settle_order_with_events_use_case,
        update_product as update_product_use_case,
        update_product_status as update_product_status_use_case,
    },
    presentation::{
        AdminOrdersQuery, AdminProductsQuery, AdminSecondsContractOrdersResponse,
        AdminSecondsContractProductsResponse, CreateSecondsContractProductRequest,
        DeleteSecondsContractProductRequest, ListOrdersQuery, ListQuery,
        OpenSecondsContractOrderRequest, OpenSecondsContractOrderResponse,
        SecondsContractOrderResponse, SecondsContractOrdersResponse,
        SecondsContractProductResponse, SecondsContractProductsResponse,
        SettleSecondsContractOrderRequest, SettleSecondsContractOrderResponse,
        UpdateSecondsContractProductRequest, UpdateSecondsContractProductStatusRequest,
    },
    service::{admin_id_from_subject, route_limit, route_offset, user_id_from_subject},
};
use crate::{
    error::AppResult,
    modules::auth::{AdminAuth, UserAuth},
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, patch, post},
};

/// 注册面向终端用户的秒合约路由，由调用方挂到已带用户鉴权的路由树下。
/// 产品目录为只读列表；订单路径上 GET 查自己的历史订单、POST 提交开仓，两者共用同一路径不同方法。
/// 用户侧没有结算入口，到期结算只能由后台接口或结算 worker 触发。
pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/seconds-contracts/products", get(list_active_products))
        .route(
            "/seconds-contracts/orders",
            get(list_orders).post(open_order),
        )
}

/// 注册后台管理端的秒合约路由，覆盖产品全生命周期与订单查询、人工结算。
/// 产品集合路径支持列表与创建，产品单体路径支持详情、整体更新和删除，启停状态另开子路径以便单独授权。
/// 订单侧只提供列表和详情两个只读入口，唯一的写操作是 `orders/:id/settle` 人工结算，
/// 该入口会实际动用户资金，调用方必须确保其挂载在管理员鉴权与审计中间件之后。
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/seconds-contracts/products",
            get(list_admin_products).post(create_product),
        )
        .route(
            "/seconds-contracts/products/:id",
            get(get_admin_product)
                .patch(update_product)
                .delete(delete_product),
        )
        .route(
            "/seconds-contracts/products/:id/status",
            patch(update_product_status),
        )
        .route("/seconds-contracts/orders", get(list_admin_orders))
        .route("/seconds-contracts/orders/:id", get(get_admin_order))
        .route("/seconds-contracts/orders/:id/settle", post(settle_order))
}

/// 处理用户端秒合约产品目录请求，返回全部可下单产品及其周期档位。
/// 本接口不要求登录，因此不解析任何用户身份；条数经 `route_limit` 归一后默认 50 且封顶 100。
/// 用例内部固定按启用状态过滤，客户端无法通过参数查看已下架产品。
async fn list_active_products(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<SecondsContractProductsResponse>> {
    let pool = mysql_pool(&state)?;
    Ok(Json(
        list_active_products_use_case(&pool, route_limit(query.limit)).await?,
    ))
}

/// 处理后台产品列表请求，返回带总数的分页结果，可按上下架状态筛选。
/// 只校验管理员身份是否成立，不取管理员编号，因为纯查询不写审计。
/// 分页与筛选参数的归一在用例内完成，与用户目录不同，这里能看到已禁用产品。
async fn list_admin_products(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminProductsQuery>,
) -> AppResult<Json<AdminSecondsContractProductsResponse>> {
    let pool = mysql_pool(&state)?;
    Ok(Json(list_admin_products_use_case(&pool, query).await?))
}

/// 处理后台单个产品详情请求，路径参数为产品主键，返回含完整周期集合的配置快照。
/// 走连接池只读查询、不加锁，返回结果仅供后台展示与编辑回填，不能作为下单校验依据。
/// 产品不存在时用例返回 `AppError::NotFound`，由全局错误映射转成 404。
async fn get_admin_product(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
) -> AppResult<Json<SecondsContractProductResponse>> {
    let pool = mysql_pool(&state)?;
    Ok(Json(get_admin_product_use_case(&pool, product_id).await?))
}

/// 处理用户查询自己秒合约订单历史的请求，同时返回持仓中和已结算的订单。
/// 用户编号只从令牌 subject 解析，请求参数无法指定他人编号，从源头杜绝越权查看。
/// 条数与偏移分别经统一规则归一；未传偏移时按零处理，保持旧客户端读取第一页的行为。
async fn list_orders(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListOrdersQuery>,
) -> AppResult<Json<SecondsContractOrdersResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        list_user_orders_use_case(
            &pool,
            user_id,
            route_limit(query.limit),
            route_offset(query.offset),
        )
        .await?,
    ))
}

/// 处理后台秒合约订单列表请求，支持按用户编号、账号邮箱和订单状态筛选并返回匹配总数。
/// 与用户侧列表相比额外回显账号邮箱，且支持偏移分页，供客服核单与风控排查使用。
/// 仅做查询，不触发结算也不改动任何订单状态。
async fn list_admin_orders(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminOrdersQuery>,
) -> AppResult<Json<AdminSecondsContractOrdersResponse>> {
    let pool = mysql_pool(&state)?;
    Ok(Json(list_admin_orders_use_case(&pool, query).await?))
}

/// 处理后台单笔订单详情请求，返回开仓价、结算价、结果与状态等完整字段，供人工结算前核对。
/// 按订单主键定位且不限定归属用户，这是后台接口的预期行为，越权风险由管理员鉴权层控制。
/// 未到期订单的结算价与结果为空，本接口只读不会顺带触发结算。
async fn get_admin_order(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<SecondsContractOrderResponse>> {
    let pool = mysql_pool(&state)?;
    Ok(Json(get_admin_order_use_case(&pool, order_id).await?))
}

/// 处理后台新建秒合约产品请求，解析管理员编号后交由用例在单个事务内写产品、周期与审计。
/// 与只读接口不同，这里传入的是 `state.mysql` 句柄而非取好的连接池，因为用例需要自行开启事务。
/// 管理员编号来自令牌 subject，会作为审计日志的操作人落库；请求体中的原因文本为必填的审计说明。
/// 参数校验失败或交易对、资产不存在时整体回滚，不会留下没有周期配置的半截产品。
async fn create_product(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateSecondsContractProductRequest>,
) -> AppResult<Json<SecondsContractProductResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_product_use_case(state.mysql.as_ref(), admin_id, request).await?,
    ))
}

/// 处理后台整体更新秒合约产品请求，路径给出产品主键，请求体给出更新后的全量配置。
/// 语义是覆盖而非增量：请求体中未列出的周期视为删除，因此前端必须回填完整周期集合再提交。
/// 用例会在事务内锁定产品、替换周期并写 before/after 双镜像审计，存量已开仓订单沿用自身固化的
/// 周期与赔率快照，不受本次改配置影响。
async fn update_product(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
    Json(request): Json<UpdateSecondsContractProductRequest>,
) -> AppResult<Json<SecondsContractProductResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_product_use_case(state.mysql.as_ref(), admin_id, product_id, request).await?,
    ))
}

/// 处理秒合约产品上下架请求，只改启停状态，不触碰交易对、赔率、周期等交易参数。
/// 单独开一个子路径而不复用整体更新，是为了让运营快速下架时无需回填完整配置。
/// 禁用只阻止新订单开仓，既有持仓订单仍会按原规则到期结算，本接口不做任何资金处理。
/// 变更连同管理员编号与原因写入审计，返回更新后的产品快照。
async fn update_product_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
    Json(request): Json<UpdateSecondsContractProductStatusRequest>,
) -> AppResult<Json<SecondsContractProductResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_product_status_use_case(state.mysql.as_ref(), admin_id, product_id, request).await?,
    ))
}

/// 处理秒合约产品物理删除请求，成功时返回 204 且响应体为空。
/// 删除采用 DELETE 携带请求体的形式，因为审计原因是必填项，需要随请求一并提交。
/// 用例会先确认该产品没有任何历史订单，只要存在一笔就拒绝删除以保护订单外键与对账可追溯性，
/// 此时应改用下架而非删除。
async fn delete_product(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
    Json(request): Json<DeleteSecondsContractProductRequest>,
) -> AppResult<StatusCode> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    delete_product_use_case(state.mysql.as_ref(), admin_id, product_id, request).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// 处理用户秒合约开仓请求，是本模块唯一会扣减用户可用余额的用户侧入口。
/// 下单用户只从令牌解析，请求体不含用户维度字段；开仓价由服务端从 Redis 行情缓存取用，
/// 客户端上送的任何价格都不会被采纳，因此这里必须同时透传 MySQL 与 Redis 句柄。
/// 请求体中的幂等键决定重复提交的语义：同键重放会回读原订单而不二次扣款，前提是产品、方向和金额一致。
/// 事件广播句柄一并传入，由用例在资金事务提交成功之后才推送开仓事件；事务失败时既不扣款也不推事件。
/// 并行多单由客户端用不同幂等键分别调用本接口实现，本接口单次只创建一笔订单。
async fn open_order(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<OpenSecondsContractOrderRequest>,
) -> AppResult<Json<OpenSecondsContractOrderResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let response = open_order_with_events_use_case(
        state.mysql.as_ref(),
        state.redis.as_ref(),
        user_id,
        request,
        state.event_broadcast_hub.as_ref(),
    )
    .await?;
    Ok(Json(response))
}

/// 处理后台人工结算秒合约订单请求，会按胜负结果实际向用户钱包派奖，属于高风险资金入口。
/// 胜负结果由请求体给出而非服务端比价推导，因此调用方须先核对开仓价与结算价再提交，
/// 结算价同样取自请求体，用例只负责校验、落库和按订单固化的赔率计算赔付。
/// 结算幂等：订单已结算且结果相同则回读原结果不重复派奖，结果不同则返回冲突并拒绝覆盖。
/// 不需要 Redis，因为不读行情；事件在结算事务提交成功后才推送，管理员编号写入审计作为操作人。
async fn settle_order(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
    Json(request): Json<SettleSecondsContractOrderRequest>,
) -> AppResult<Json<SettleSecondsContractOrderResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let response = settle_order_with_events_use_case(
        state.mysql.as_ref(),
        admin_id,
        order_id,
        request,
        state.event_broadcast_hub.as_ref(),
    )
    .await?;
    Ok(Json(response))
}
