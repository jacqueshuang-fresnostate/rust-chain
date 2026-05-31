use crate::{
    error::AppResult,
    modules::{
        auth::AdminAuth,
        events::{
            EventBroadcastHub, EventBroadcastSubscription, EventOutboxService, PrivateWsAuth,
            PublishedOutboxBatch, WebSocketChannel,
        },
        market::{KlineUpsertKey, ValidatedMarketSymbol},
    },
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{
        Path, Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::Response,
    routing::{get, post},
};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/events/outbox/publish-once", post(publish_once))
        .route("/ws/public/:namespace/:topic", get(public_ws))
        .route("/ws/private", get(private_ws))
}

#[derive(Debug, Deserialize)]
struct PrivateWsQuery {
    token: Option<String>,
}

async fn publish_once(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<PublishedOutboxBatch>> {
    let service = EventOutboxService::from_state(&state)?;
    let summary = service.publish_once(Utc::now()).await?;

    Ok(Json(summary))
}

async fn public_ws(
    Path((namespace, topic)): Path<(String, String)>,
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> AppResult<Response> {
    let channel = public_channel(namespace, topic)?;
    let hub = state.event_broadcast_hub.clone();
    Ok(ws.on_upgrade(move |socket| public_socket(socket, channel, hub)))
}

async fn private_ws(
    Query(query): Query<PrivateWsQuery>,
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> AppResult<Response> {
    let token_query = query.token.as_deref().map(|token| format!("token={token}"));
    let auth = PrivateWsAuth::from_query(token_query.as_deref(), &state.settings)?;
    let hub = state.event_broadcast_hub.clone();
    Ok(ws.on_upgrade(move |socket| private_socket(socket, auth, hub)))
}

fn public_channel(namespace: String, topic: String) -> AppResult<WebSocketChannel> {
    match namespace.as_str() {
        "ticker" | "depth" | "trade" => Ok(WebSocketChannel::public(
            namespace,
            ValidatedMarketSymbol::from_raw(&topic)
                .map_err(|error| crate::error::AppError::Validation(error.to_string()))?
                .as_str(),
        )?),
        "kline" => {
            let Some((symbol, interval)) = topic.rsplit_once('_') else {
                return WebSocketChannel::public(namespace, topic);
            };
            let symbol = ValidatedMarketSymbol::from_raw(symbol)
                .map_err(|error| crate::error::AppError::Validation(error.to_string()))?;
            let interval = KlineUpsertKey::new(interval, Utc::now())
                .map_err(|error| crate::error::AppError::Validation(error.to_string()))?
                .interval()
                .to_owned();
            WebSocketChannel::public(namespace, format!("{}_{}", symbol.as_str(), interval))
        }
        _ => WebSocketChannel::public(namespace, topic),
    }
}

async fn public_socket(
    socket: WebSocket,
    channel: WebSocketChannel,
    hub: Option<EventBroadcastHub>,
) {
    let subscription = hub.map(|hub| hub.subscribe(&channel));
    run_subscription_socket(
        socket,
        format!(
            r#"{{"type":"subscribed","channel":"{}"}}"#,
            channel.as_text()
        ),
        subscription,
    )
    .await;
}

async fn private_socket(socket: WebSocket, auth: PrivateWsAuth, hub: Option<EventBroadcastHub>) {
    let channel = WebSocketChannel::private_user(auth.user_id);
    let subscription = hub.map(|hub| hub.subscribe(&channel));
    run_subscription_socket(
        socket,
        format!(
            r#"{{"type":"subscribed","channel":"{}"}}"#,
            channel.as_text()
        ),
        subscription,
    )
    .await;
}

async fn run_subscription_socket(
    socket: WebSocket,
    confirmation: String,
    subscription: Option<EventBroadcastSubscription>,
) {
    let (mut sender, mut receiver) = socket.split();
    if sender.send(Message::Text(confirmation)).await.is_err() {
        return;
    }

    match subscription {
        Some(mut subscription) => loop {
            tokio::select! {
                message = receiver.next() => {
                    if !handle_client_message(message, &mut sender).await {
                        break;
                    }
                }
                broadcast = subscription.recv() => {
                    let Ok(message) = broadcast else {
                        break;
                    };
                    if sender
                        .send(Message::Text(message.payload().to_owned()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            }
        },
        None => while handle_client_message(receiver.next().await, &mut sender).await {},
    }
}

async fn handle_client_message(
    message: Option<Result<Message, axum::Error>>,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
) -> bool {
    match message {
        Some(Ok(Message::Text(text))) if text == "ping" => {
            sender.send(Message::Text("pong".to_owned())).await.is_ok()
        }
        Some(Ok(Message::Ping(payload))) => sender.send(Message::Pong(payload)).await.is_ok(),
        Some(Ok(Message::Close(_))) | Some(Err(_)) | None => false,
        Some(Ok(_)) => true,
    }
}
