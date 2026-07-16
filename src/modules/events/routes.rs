use crate::{
    error::AppResult,
    modules::{
        auth::AdminAuth,
        events::{
            EventOutboxService, PublishedOutboxBatch,
            application::authorize_private_ws,
            presentation::PrivateWsQuery,
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
