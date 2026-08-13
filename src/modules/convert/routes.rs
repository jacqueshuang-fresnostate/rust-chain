//! convert bounded context HTTP 路由层。
//!
//! 只负责把 axum 提取器解包为应用层用例入参并把用例返回值包成 JSON，
//! 不在此层做金额校验、汇率计算、事务控制或事件广播。
//! 全部闪兑接口挂在用户命名空间下，除交易对列表外均要求 `UserAuth` 通过的 JWT 身份。

use super::{
    application::{
        confirm_convert_quote_with_events as confirm_convert_quote_with_events_use_case,
        create_convert_quote, list_convert_orders, list_convert_pairs,
    },
    presentation::{
        ConfirmConvertQuoteRequest, ConfirmConvertQuoteResponse, ConvertOrdersQuery,
        ConvertOrdersResponse, ConvertPairsResponse, ConvertQuoteResponse,
        CreateConvertQuoteRequest, ListQuery,
    },
};
use crate::{error::AppResult, modules::auth::UserAuth, state::AppState};
use axum::{
    Json, Router,
    extract::{Query, State},
    routing::{get, post},
};

/// 注册四个面向终端用户的闪兑接口：交易对列表、创建报价、确认报价和订单查询。
/// 交易对列表为公开只读接口，其余三个由各自处理函数内的 `UserAuth` 提取器强制鉴权。
/// 该函数只装配路由表并返回待合并的 `Router`，不接触数据库、Redis 或事件总线。
pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/convert/pairs", get(list_pairs))
        .route("/convert/quote", post(create_quote))
        .route("/convert/confirm", post(confirm_quote))
        .route("/convert/orders", get(list_orders))
}

/// 返回后台已启用的闪兑交易对配置，包含双侧资产 Logo、计价模式、价差费率和正反向限额。
/// 无需登录，`limit` 缺省或越界时由服务层归一到 1..=100，不接受任意大的分页量。
/// 只做只读查询，不生成报价、不写缓存，也不反映调用者钱包余额。
async fn list_pairs(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<ConvertPairsResponse>> {
    Ok(Json(list_convert_pairs(state.mysql.clone(), query).await?))
}

/// 以 JWT 主体身份为指定资产方向和源金额申请一笔限时报价，同时需要 MySQL 与 Redis 均已配置。
/// 汇率、目标到账额与手续费由服务端按交易对计价模式计算，请求体不能携带客户端汇率。
/// 报价阶段只做提示性余额校验，不冻结 available；真正扣款发生在后续确认接口的结算事务里。
async fn create_quote(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateConvertQuoteRequest>,
) -> AppResult<Json<ConvertQuoteResponse>> {
    Ok(Json(
        create_convert_quote(
            state.mysql.clone(),
            state.redis.clone(),
            &claims.sub,
            request,
        )
        .await?,
    ))
}

/// 用 quote_id 兑现此前生成的报价，成交后返回确认结果并触发用户私有完成事件。
/// 该路由额外把 `event_broadcast_hub` 透传给应用层，事件只在结算事务提交成功后发布。
/// 报价缺失、不属于当前用户或已过期都会在进入结算事务前失败；重复确认由订单唯一键拒绝。
async fn confirm_quote(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<ConfirmConvertQuoteRequest>,
) -> AppResult<Json<ConfirmConvertQuoteResponse>> {
    Ok(Json(
        confirm_convert_quote_with_events_use_case(
            state.mysql.clone(),
            state.redis.clone(),
            state.event_broadcast_hub.as_ref(),
            &claims.sub,
            request,
        )
        .await?,
    ))
}

/// 按 JWT 主体倒序返回该用户的闪兑订单，可用 `status` 过滤 pending 或 completed 等状态。
/// 用户维度由 claims 强制注入而非请求参数，因此不存在越权查看他人订单的入口。
/// 返回的汇率和费用是下单时固化的快照，不随交易对配置或行情变化重算。
async fn list_orders(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ConvertOrdersQuery>,
) -> AppResult<Json<ConvertOrdersResponse>> {
    Ok(Json(
        list_convert_orders(state.mysql.clone(), &claims.sub, query).await?,
    ))
}
