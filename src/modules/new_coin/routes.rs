//! new_coin bounded context HTTP routing layer.
//!
//! 路由层：声明新币发行面向普通用户的 HTTP 端点，并把请求转交给应用层用例。
//! 本文件只做三件事：绑定路径与方法、通过提取器完成鉴权与参数解析、把用例返回值包成 JSON。
//! 所有业务判定、事务边界、幂等控制与事件广播都在应用层完成，
//! 路由处理器内不出现任何 SQL、状态机判断或金额计算。
//! 项目列表与详情是公开端点，其余端点一律要求 `UserAuth`，
//! 用户身份只从会话声明的 `sub` 取得，绝不接受请求体或查询参数传入的用户标识。
//! 错误统一由 `AppResult` 透传给全局错误处理，本层不吞异常也不改写状态码。

use crate::{
    error::AppResult,
    modules::{
        auth::UserAuth,
        new_coin::{
            application::{
                create_new_coin_purchase_with_events as create_new_coin_purchase_with_events_use_case,
                create_new_coin_subscription_with_events as create_new_coin_subscription_with_events_use_case,
                get_new_coin_project, list_new_coin_distributions, list_new_coin_projects,
                list_new_coin_purchases, list_new_coin_subscriptions, list_new_coin_unlocks,
                pay_new_coin_unlock_fee,
                release_new_coin_unlock_with_events as release_new_coin_unlock_with_events_use_case,
            },
            presentation::{
                CreatePurchaseRequest, CreateSubscriptionRequest, ListQuery,
                NewCoinDistributionsResponse, NewCoinOrderCreationResponse, NewCoinProjectResponse,
                NewCoinProjectsResponse, NewCoinPurchasesResponse, NewCoinSubscriptionsResponse,
                NewCoinUnlocksResponse, PayUnlockFeeRequest, PayUnlockFeeResponse,
                ReleaseUnlockResponse,
            },
        },
    },
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};

/// 装配新币模块面向普通用户的全部端点，返回待挂载到上层 `AppState` 路由树的子路由。
/// 端点分两类：`/new-coins` 与 `/new-coins/:symbol` 是无需登录的项目公告读接口；
/// 其余读写接口都在处理器内经 `UserAuth` 提取器鉴权，未登录直接被提取器拒绝。
/// 写操作共四个，分别是按符号申购、按符号二级市场买入、缴纳解禁手续费和申请释放锁仓，
/// 全部使用 POST 且由应用层通过幂等键控制重复提交，路由层本身不做去重。
/// 路径中的 `:symbol` 是项目符号，`:id` 是解禁记录的幂等键而非自增主键。
/// 本函数只声明路由表，不注册中间件、不设限流、也不在此处绑定任何数据库连接。
pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/new-coins", get(list_projects))
        .route("/new-coins/:symbol", get(project_detail))
        .route(
            "/new-coins/:symbol/subscriptions",
            post(create_subscription),
        )
        .route("/new-coins/subscriptions", get(list_subscriptions))
        .route("/new-coins/distributions", get(list_distributions))
        .route("/new-coins/:symbol/purchase", post(create_purchase))
        .route("/new-coins/purchases", get(list_purchases))
        .route("/new-coins/unlocks", get(list_unlocks))
        .route("/new-coins/unlocks/:id/pay-fee", post(pay_unlock_fee))
        .route("/new-coins/unlocks/:id/release", post(release_unlock))
}

/// 处理新币项目列表的公开查询，无需登录即可访问，返回内容对所有访客一致。
/// 仅把可选的条数参数透传给用例，上限裁剪与停用项目过滤都在下游完成。
/// MySQL 未配置时由用例返回内部错误，本处理器不做降级也不返回空列表。
async fn list_projects(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<NewCoinProjectsResponse>> {
    Ok(Json(
        list_new_coin_projects(state.mysql.clone(), query).await?,
    ))
}

/// 处理单个新币项目详情的公开查询，路径段 `:symbol` 原样作为项目符号传给用例。
/// 符号不存在或项目已被后台停用时由用例返回 `NotFound`，本处理器不区分两者也不回退查草稿。
/// 与列表接口共用同一份项目视图字段，因此详情页与列表页展示的口径完全一致。
async fn project_detail(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> AppResult<Json<NewCoinProjectResponse>> {
    Ok(Json(
        get_new_coin_project(state.mysql.clone(), &symbol).await?,
    ))
}

/// 处理当前登录用户的新币申购单查询，用户范围来自会话声明的 `sub`，请求参数无法覆盖。
/// 只透传条数参数，返回的是申请数量与实际配额数量并存的历史快照，不触发配额重算。
/// 未携带有效令牌时请求在 `UserAuth` 提取阶段即被拒绝，不会进入本处理器。
async fn list_subscriptions(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<NewCoinSubscriptionsResponse>> {
    Ok(Json(
        list_new_coin_subscriptions(state.mysql.clone(), &claims.sub, query).await?,
    ))
}

/// 处理当前登录用户的新币分发记录查询，展示每次认购结果实际落到钱包的明细。
/// 与申购单查询相区别：申购单描述「申请与中签」，分发记录描述「资产如何到账及是否锁仓」。
/// 纯读路径，不会补发遗漏的分发，也不会推进任何分发状态。
async fn list_distributions(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<NewCoinDistributionsResponse>> {
    Ok(Json(
        list_new_coin_distributions(state.mysql.clone(), &claims.sub, query).await?,
    ))
}

/// 处理当前登录用户的二级市场买入记录查询，返回下单时固化的价格、数量与计价总额。
/// 该三元快照不随行情变动而重算，可直接用于对账，与公开项目列表的实时配置无关。
/// 结果按用户隔离并受条数上限约束，本处理器不额外过滤状态或交易对。
async fn list_purchases(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<NewCoinPurchasesResponse>> {
    Ok(Json(
        list_new_coin_purchases(state.mysql.clone(), &claims.sub, query).await?,
    ))
}

/// 处理当前登录用户的解禁记录查询，一并返回解禁数量、到期信息与手续费口径和缴费状态。
/// 这是缴费与释放两个写接口的前置读接口，前端据此判断某条记录该走缴费还是可直接释放。
/// 只呈现状态，既不缴费也不释放锁仓，更不会因为已到期就自动推进记录状态。
async fn list_unlocks(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<NewCoinUnlocksResponse>> {
    Ok(Json(
        list_new_coin_unlocks(state.mysql.clone(), &claims.sub, query).await?,
    ))
}

/// 处理解禁手续费缴纳请求，路径段 `:id` 是解禁记录的幂等键，用于与登录用户共同定位唯一记录。
/// 请求体携带支付资产与金额，用例会要求两者与记录固化的收费快照完全一致，不符即拒绝。
/// 重复缴费不会报错而是返回 `paid=false`，因此前端应据响应字段而非 HTTP 状态码判断是否本次生效。
/// 该端点只推进记录的缴费状态，不在此处扣减钱包余额。
async fn pay_unlock_fee(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<PayUnlockFeeRequest>,
) -> AppResult<Json<PayUnlockFeeResponse>> {
    Ok(Json(
        pay_new_coin_unlock_fee(state.mysql.clone(), &claims.sub, id, request).await?,
    ))
}

/// 处理锁仓释放请求，把已到期且满足缴费要求的锁仓额度转为可用余额，是本模块的资金出账入口之一。
/// 路径段 `:id` 是解禁记录的幂等键；未到期、未缴费或记录不存在时由下游返回校验或 `NotFound` 错误。
/// 除状态与连接池外还额外传入事件广播中心，由应用层在事务提交成功后再发布用户私有解锁事件，
/// 因此路由层既不感知事件格式，也不会在失败路径上误发通知。
async fn release_unlock(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<ReleaseUnlockResponse>> {
    Ok(Json(
        release_new_coin_unlock_with_events_use_case(
            state.mysql.clone(),
            state.event_broadcast_hub.as_ref(),
            &claims.sub,
            id,
        )
        .await?,
    ))
}

/// 处理新币申购下单，路径段 `:symbol` 指定项目，请求体携带计价资产、支付金额、申购数量与幂等键。
/// 下游要求项目正处于申购阶段，金额与数量均为正，幂等键非空，任一不满足都在扣款前被拒。
/// 资金动作是扣减计价资产的可用余额并按解禁规则分配新币，全部在仓储的单个事务内完成。
/// 重复幂等键返回冲突错误，既不二次扣款也不重复广播事件。
/// 事件广播中心一并传入，由应用层在事务提交后发布申购创建的私有事件。
async fn create_subscription(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Json(request): Json<CreateSubscriptionRequest>,
) -> AppResult<Json<NewCoinOrderCreationResponse>> {
    Ok(Json(
        create_new_coin_subscription_with_events_use_case(
            state.mysql.clone(),
            state.event_broadcast_hub.as_ref(),
            &claims.sub,
            symbol,
            request,
        )
        .await?,
    ))
}

/// 处理上市后二级市场买入下单，请求体携带交易对、价格、数量与幂等键，计价总额由下游按价格乘数量算出。
/// 与申购路径的关键差异在于要求项目已上市、后台购买开关已开启，且交易对必须等于后台批准的那一个。
/// 下单事务会重新锁定项目与交易对再扣款锁仓，因此后台在请求途中改配置不会导致按旧规则成交。
/// 重复幂等键返回冲突错误，不产生第二笔资金变更；事件同样由应用层在提交成功后才广播。
async fn create_purchase(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Json(request): Json<CreatePurchaseRequest>,
) -> AppResult<Json<NewCoinOrderCreationResponse>> {
    Ok(Json(
        create_new_coin_purchase_with_events_use_case(
            state.mysql.clone(),
            state.event_broadcast_hub.as_ref(),
            &claims.sub,
            symbol,
            request,
        )
        .await?,
    ))
}
