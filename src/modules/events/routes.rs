use crate::{
    error::AppResult,
    modules::{
        auth::AdminAuth,
        events::{
            EventOutboxService, PublishedOutboxBatch,
            application::authorize_private_ws,
            infrastructure::{
                OutboxRecordRow, list_inbox_records, list_outbox_records,
                requeue_outbox_dead_letter,
            },
            presentation::PrivateWsQuery,
            presentation::{EventRecordsQuery, RequeueOutboxRequest},
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
) -> AppResult<Json<serde_json::Value>> {
    let (records, total) = list_outbox_records(events_pool(&state)?, query.as_filter()).await?;

    Ok(Json(
        serde_json::json!({ "records": records, "total": total }),
    ))
}

async fn inbox_records(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<EventRecordsQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let (records, total) = list_inbox_records(events_pool(&state)?, query.as_filter()).await?;

    Ok(Json(
        serde_json::json!({ "records": records, "total": total }),
    ))
}

async fn requeue_outbox(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<u64>,
    Json(request): Json<RequeueOutboxRequest>,
) -> AppResult<Json<OutboxRecordRow>> {
    let reason = request.require_reason()?;
    let admin_id = crate::modules::admin::service::admin_id_from_subject(&claims.sub)?;
    let record = requeue_outbox_dead_letter(events_pool(&state)?, admin_id, id, &reason).await?;

    Ok(Json(record))
}

fn events_pool(state: &AppState) -> AppResult<&sqlx::Pool<sqlx::MySql>> {
    state.mysql.as_ref().ok_or_else(|| {
        crate::error::AppError::Internal("mysql pool is not configured for event routes".to_owned())
    })
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
