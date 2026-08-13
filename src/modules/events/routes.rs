//! events bounded context HTTP 与 WebSocket 路由层。
//!
//! 本文件挂载两类形态完全不同的端点：一类是管理员运维用的普通 HTTP 接口，覆盖手动触发一轮 outbox 发布、
//! 查询 outbox 与 inbox 记录、重排死信；另一类是长连接的 WebSocket 端点，分公共频道与用户私有频道。
//!
//! 两类端点的鉴权方式不同。HTTP 运维接口由 `AdminAuth` 提取器把守；
//! 公共 WebSocket 不需要身份，任何人都可订阅行情一类的公开数据；
//! 私有 WebSocket 无法使用请求头携带令牌，因此改由查询串传令牌，并且必须在协议升级之前完成校验，
//! 校验失败直接返回错误响应而不建立连接。
//!
//! `/ws/spot`、`/ws/margin`、`/ws/seconds` 只是 `/ws/public` 的历史兼容别名，
//! 三者复用同一处理函数与同一套多频道订阅规则，路径前缀不构成任何数据隔离。
//! 路由层自身不落库、不发布事件，也不持有订阅状态，连接建立后全部交给广播 hub 与套接字循环处理。

use crate::{
    error::AppResult,
    modules::{
        auth::AdminAuth,
        events::{
            application::{
                authorize_private_ws, list_inbox_records as list_inbox_records_use_case,
                list_outbox_records as list_outbox_records_use_case, publish_outbox_once,
                requeue_outbox_dead_letter as requeue_outbox_dead_letter_use_case,
            },
            presentation::{
                EventRecordsQuery, EventRecordsResponse, InboxRecordResponse, OutboxRecordResponse,
                PrivateWsQuery, RequeueOutboxRequest,
            },
            public_channel,
            service::{
                PublishedOutboxBatch, public_ws_confirmation_text, run_private_socket,
                run_public_multi_socket, run_public_socket,
            },
        },
    },
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State, WebSocketUpgrade},
    response::Response,
    routing::{get, post},
};
use chrono::Utc;

/// 注册管理员 outbox 发布/查询/死信重排、inbox 查询，以及公共兼容别名、公共单频道和私有 WebSocket 端点。
/// 运维请求必须先通过 `AdminAuth`；公共别名共享同一多频道订阅规则，私有连接在升级前校验 user token，路由层不直接持久化事件或广播。
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/events/outbox/publish-once", post(publish_once))
        .route("/events/outbox", get(outbox_records))
        .route("/events/outbox/:id/requeue", post(requeue_outbox))
        .route("/events/inbox", get(inbox_records))
        .route("/ws/public", get(public_multi_ws))
        .route("/ws/public/:namespace/:topic", get(public_ws))
        .route("/ws/spot", get(public_multi_ws))
        .route("/ws/spot/:namespace/:topic", get(public_ws))
        .route("/ws/margin", get(public_multi_ws))
        .route("/ws/margin/:namespace/:topic", get(public_ws))
        .route("/ws/seconds", get(public_multi_ws))
        .route("/ws/seconds/:namespace/:topic", get(public_ws))
        .route("/ws/private", get(private_ws))
}

/// 手动触发一轮 outbox 发布并返回本轮批次摘要，供运维在积压时立即推进而不必等待定时轮询。
/// 以调用时刻作为到期判定基准，因此只会发出当下已到重试时间的消息。
/// 与定时发布走同一条用例，因此并发触发不会破坏语义，最多造成同一消息被重复投递一次，
/// 这在 at-least-once 语义下由下游 inbox 去重吸收。
async fn publish_once(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<PublishedOutboxBatch>> {
    let summary = publish_outbox_once(&state, Utc::now()).await?;

    Ok(Json(summary))
}

/// 分页查询 outbox 记录并返回匹配总数，用于观察待发布积压与死信堆积。
/// 支持按状态筛选，不传状态即返回全部；只校验管理员身份而不取编号，因为查询不写审计。
/// 返回项不含事件载荷，只有路由信息与状态时间，避免业务数据进入运维面板。
async fn outbox_records(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<EventRecordsQuery>,
) -> AppResult<Json<EventRecordsResponse<OutboxRecordResponse>>> {
    Ok(Json(list_outbox_records_use_case(&state, query).await?))
}

/// 分页查询 inbox 记录并返回匹配总数，用于排查某个消费者为何反复失败。
/// 与 outbox 查询共用同一份查询串结构，但返回项带消费者名、错误摘要与消费完成时刻。
/// 纯读取：不领取消息、不推进重试、不改动任何消费状态。
async fn inbox_records(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<EventRecordsQuery>,
) -> AppResult<Json<EventRecordsResponse<InboxRecordResponse>>> {
    Ok(Json(list_inbox_records_use_case(&state, query).await?))
}

/// 把一条死信事件重新投入待发布队列，返回重排后的记录快照。
/// 这里保留完整的鉴权对象而非丢弃，因为用例需要管理员身份来写审计操作人。
/// 只有处于死信状态的事件可被重排，重复重排返回冲突；重排会清零失败次数，相当于重置整轮重试预算。
/// 本接口只改数据库状态，实际投递仍由后续的发布轮次完成。
async fn requeue_outbox(
    auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<u64>,
    Json(request): Json<RequeueOutboxRequest>,
) -> AppResult<Json<OutboxRecordResponse>> {
    Ok(Json(
        requeue_outbox_dead_letter_use_case(&state, auth, id, request).await?,
    ))
}

/// 升级为公共多频道 WebSocket 连接，客户端连上后再通过消息动态订阅或退订多个频道。
/// 不做任何鉴权，因为该通道只承载行情等公开数据；私有数据一律走私有端点。
/// 升级前只克隆广播 hub 句柄，真正的订阅管理与消息推送都在套接字循环内进行。
/// 多个历史路径别名共用本处理函数，路径前缀不影响可订阅的频道范围。
async fn public_multi_ws(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> AppResult<Response> {
    let hub = state.event_broadcast_hub.clone();
    Ok(ws.on_upgrade(move |socket| run_public_multi_socket(socket, hub)))
}

/// 升级为单频道公共 WebSocket 连接，频道由路径上的命名空间与主题两段确定。
/// 频道名在升级之前先经校验构造，非法命名空间或主题会直接返回错误而不建立连接。
/// 与多频道端点的差别是订阅在握手时就固定下来，连接期间不能再改订阅。
/// 升级前预先算好订阅确认文案，避免每条连接重复拼接；同样不做鉴权，只承载公开数据。
async fn public_ws(
    Path((namespace, topic)): Path<(String, String)>,
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> AppResult<Response> {
    let channel = public_channel(namespace, topic)?;
    let hub = state.event_broadcast_hub.clone();
    let confirmation = public_ws_confirmation_text(&channel);
    Ok(ws.on_upgrade(move |socket| run_public_socket(socket, channel, hub, confirmation.clone())))
}

/// 升级为用户私有 WebSocket 连接，用于推送订单、成交、资金变动等仅本人可见的事件。
/// 令牌通过查询串传递，因为浏览器的 WebSocket API 无法自定义请求头。
/// 校验严格发生在协议升级之前：未通过时直接返回错误响应，连接根本不会建立，
/// 因此不存在先连上再鉴权的中间窗口。
/// 鉴权结果被移交给套接字循环，用于把该连接绑定到对应用户的私有频道，客户端无法自行切换收听对象。
async fn private_ws(
    Query(query): Query<PrivateWsQuery>,
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> AppResult<Response> {
    let auth = authorize_private_ws(&state, query).await?;
    let hub = state.event_broadcast_hub.clone();
    Ok(ws.on_upgrade(move |socket| run_private_socket(socket, auth, hub)))
}
