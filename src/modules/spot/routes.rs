//! 现货上下文的 HTTP 路由装配层。
//!
//! 本文件只做鉴权、身份解析、依赖注入和结果包装：从 `UserAuth` 或 `AdminAuth` 取出 JWT，
//! 解析成用户或管理员标识，从 `AppState` 取 MySQL 连接池、Redis 与事件广播中心，转发给用例。
//! 所有业务校验、事务边界、钱包锁顺序、幂等判定与事件发布时机都在 application 层内部完成。
//! 用户路由把下单、撤单、批量撤单和查询挂在 `/spot/orders` 与 `/spot/trades` 两条路径上，
//! 通过 HTTP 方法区分动作；后台路由额外提供订单详情、强制撤单和撮合成交录入。
//! 需要注意 `/spot/fills` 是后台入口但会真正结算双边资金，是本文件里风险最高的转发点。

use crate::{
    error::AppResult,
    modules::spot::service::admin_id_from_subject,
    modules::user::service::user_id_from_subject,
    modules::{
        auth::{AdminAuth, UserAuth},
        spot::{
            application::{
                cancel_admin_spot_order_with_events as cancel_admin_spot_order_with_events_use_case,
                cancel_all_user_spot_orders_with_events as cancel_all_user_spot_orders_with_events_use_case,
                cancel_user_spot_order_with_events as cancel_user_spot_order_with_events_use_case,
                create_spot_order_with_events as create_spot_order_with_events_use_case,
                fill_spot_orders_with_events_with_request as fill_spot_orders_with_events_with_request_use_case,
                get_admin_spot_order as get_admin_spot_order_use_case,
                list_admin_spot_orders as list_admin_spot_orders_use_case,
                list_admin_spot_trades as list_admin_spot_trades_use_case,
                list_user_spot_orders as list_user_spot_orders_use_case,
                list_user_spot_trades as list_user_spot_trades_use_case, mysql_pool,
                validate_admin_cancel_spot_order_request,
            },
            presentation::{
                AdminCancelSpotOrderRequest, AdminSpotOrdersQuery, AdminSpotOrdersResponse,
                AdminSpotTradesQuery, AdminSpotTradesResponse, CancelAllSpotOrdersQuery,
                CreateSpotOrderRequest, FillSpotOrdersRequest, SpotCancelAllResponse,
                SpotCancelResponse, SpotFillResponse, SpotOrderResponse, SpotOrdersQuery,
                SpotOrdersResponse, SpotTradesQuery, SpotTradesResponse,
            },
        },
    },
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{delete, get, post},
};

/// 装配用户侧现货路由，只用两条路径承载五个动作，靠 HTTP 方法区分语义。
/// `/spot/orders` 上 POST 下单、GET 查列表、DELETE 批量撤单，批量撤单的交易对过滤走查询串而非请求体。
/// 单笔撤单挂在 `/spot/orders/:id` 的 DELETE 上；`/spot/trades` 只提供成交查询，没有写入口。
/// 各 handler 自行声明 `UserAuth`，本函数不挂载任何鉴权或限流中间件。
pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/spot/orders",
            post(create_order)
                .get(list_orders)
                .delete(cancel_all_orders),
        )
        .route("/spot/orders/:id", delete(cancel_order))
        .route("/spot/trades", get(list_trades))
}

/// 装配后台现货路由，包含跨用户订单与成交检索、订单详情、强制撤单和撮合成交录入。
/// 强制撤单用 POST `/spot/orders/:id/cancel` 而非 DELETE，因为它需要请求体携带审计原因。
/// `/spot/fills` 是唯一会真正结算双边资金的后台入口，其余四个都是只读或仅退款。
/// 全部 handler 走 `AdminAuth`，其中只有强制撤单需要把管理员标识透传给用例用于审计归属。
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/spot/orders", get(list_admin_orders))
        .route("/spot/orders/:id", get(get_admin_order))
        .route("/spot/orders/:id/cancel", post(cancel_admin_order))
        .route("/spot/trades", get(list_admin_trades))
        .route("/spot/fills", post(fill_orders))
}

/// 现货下单入口，支持限价、市价与止损限价三种类型，是用户侧唯一冻结资金的写路径。
/// 同时注入 Redis 和事件广播中心：前者用于市价单取服务端参考价，后者用于提交后推送订单事件。
/// 请求可带幂等键，用例据此做同键同参重放，返回原订单且不重复冻结资金。
async fn create_order(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateSpotOrderRequest>,
) -> AppResult<Json<SpotOrderResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let response = create_spot_order_with_events_use_case(
        &mysql_pool(&state)?,
        state.redis.as_ref(),
        state.event_broadcast_hub.as_ref(),
        user_id,
        request,
    )
    .await?;

    Ok(Json(response))
}

/// 查询当前用户自己的现货订单列表，可按交易对和状态筛选并限制返回条数。
/// 用户标识只取自 JWT，查询串无法指定他人；未传 `limit` 时由用例夹到默认上限。
async fn list_orders(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<SpotOrdersQuery>,
) -> AppResult<Json<SpotOrdersResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        list_user_spot_orders_use_case(&pool, user_id, query).await?,
    ))
}

/// 后台跨用户检索现货订单并返回同筛选口径的总数，支持交易对、状态、用户标识和邮箱组合过滤。
/// 比用户侧多出 `include_internal` 开关，用于决定是否把平台内部做市订单纳入结果。
/// 只读路径不解析管理员标识，也不写任何审计记录。
async fn list_admin_orders(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminSpotOrdersQuery>,
) -> AppResult<Json<AdminSpotOrdersResponse>> {
    let pool = mysql_pool(&state)?;
    Ok(Json(list_admin_spot_orders_use_case(&pool, query).await?))
}

async fn get_admin_order(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<SpotOrderResponse>> {
    Ok(Json(
        get_admin_spot_order_use_case(&mysql_pool(&state)?, order_id).await?,
    ))
}

async fn cancel_admin_order(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
    Json(request): Json<AdminCancelSpotOrderRequest>,
) -> AppResult<Json<SpotCancelResponse>> {
    let reason = validate_admin_cancel_spot_order_request(request)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let response = cancel_admin_spot_order_with_events_use_case(
        &mysql_pool(&state)?,
        order_id,
        admin_id,
        reason,
        state.event_broadcast_hub.as_ref(),
    )
    .await?;

    Ok(Json(response))
}

async fn cancel_order(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<SpotCancelResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let response = cancel_user_spot_order_with_events_use_case(
        &mysql_pool(&state)?,
        order_id,
        user_id,
        state.event_broadcast_hub.as_ref(),
    )
    .await?;

    Ok(Json(response))
}

async fn cancel_all_orders(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<CancelAllSpotOrdersQuery>,
) -> AppResult<Json<SpotCancelAllResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    Ok(Json(
        cancel_all_user_spot_orders_with_events_use_case(
            &mysql_pool(&state)?,
            user_id,
            query,
            state.event_broadcast_hub.as_ref(),
        )
        .await?,
    ))
}

async fn list_trades(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<SpotTradesQuery>,
) -> AppResult<Json<SpotTradesResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    Ok(Json(
        list_user_spot_trades_use_case(&mysql_pool(&state)?, user_id, query).await?,
    ))
}

async fn list_admin_trades(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminSpotTradesQuery>,
) -> AppResult<Json<AdminSpotTradesResponse>> {
    let pool = mysql_pool(&state)?;
    Ok(Json(list_admin_spot_trades_use_case(&pool, query).await?))
}

async fn fill_orders(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<FillSpotOrdersRequest>,
) -> AppResult<Json<SpotFillResponse>> {
    let response = fill_spot_orders_with_events_with_request_use_case(
        &mysql_pool(&state)?,
        request,
        state.event_broadcast_hub.as_ref(),
    )
    .await?;
    Ok(Json(response))
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_spot_routes_tests.rs"]
mod tests;
