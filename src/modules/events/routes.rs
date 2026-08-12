use crate::{
    error::AppResult,
    modules::{
        auth::AdminAuth,
        events::{
            EventOutboxService, PublishedOutboxBatch,
            application::{
                authorize_private_ws, list_inbox_records as list_inbox_records_use_case,
                list_outbox_records as list_outbox_records_use_case,
                requeue_outbox_dead_letter as requeue_outbox_dead_letter_use_case,
            },
            presentation::{
                EventRecordsQuery, EventRecordsResponse, InboxRecordResponse, OutboxRecordResponse,
                PrivateWsQuery, RequeueOutboxRequest,
            },
            public_channel,
            service::{
                public_ws_confirmation_text, run_private_socket, run_public_multi_socket,
                run_public_socket,
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

/// 注册事件运维与 WebSocket 传输端点；运维端点由 `AdminAuth` 限制，业务编排委托应用层。
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

async fn publish_once(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<PublishedOutboxBatch>> {
    let service = EventOutboxService::from_state(&state)?;
    let summary = service.publish_once(Utc::now()).await?;

    Ok(Json(summary))
}

async fn outbox_records(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<EventRecordsQuery>,
) -> AppResult<Json<EventRecordsResponse<OutboxRecordResponse>>> {
    Ok(Json(list_outbox_records_use_case(&state, query).await?))
}

async fn inbox_records(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<EventRecordsQuery>,
) -> AppResult<Json<EventRecordsResponse<InboxRecordResponse>>> {
    Ok(Json(list_inbox_records_use_case(&state, query).await?))
}

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

async fn public_multi_ws(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> AppResult<Response> {
    let hub = state.event_broadcast_hub.clone();
    Ok(ws.on_upgrade(move |socket| run_public_multi_socket(socket, hub)))
}

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

async fn private_ws(
    Query(query): Query<PrivateWsQuery>,
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> AppResult<Response> {
    let auth = authorize_private_ws(&state, query).await?;
    let hub = state.event_broadcast_hub.clone();
    Ok(ws.on_upgrade(move |socket| run_private_socket(socket, auth, hub)))
}
