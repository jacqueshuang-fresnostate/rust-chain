use crate::{
    error::AppResult,
    modules::{
        auth::AdminAuth,
        events::{
            EventBroadcastHub, EventBroadcastMultiSubscription, EventBroadcastSubscription,
            EventOutboxService, PrivateWsAuth, PublishedOutboxBatch, WebSocketChannel,
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
use std::collections::HashSet;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/events/outbox/publish-once", post(publish_once))
        .route("/ws/public", get(public_multi_ws))
        .route("/ws/public/:namespace/:topic", get(public_ws))
        .route("/ws/private", get(private_ws))
}

#[derive(Debug, Deserialize)]
struct PrivateWsQuery {
    token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PublicWsCommand {
    op: String,
    channel: String,
    symbol: Option<String>,
    interval: Option<String>,
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
    Ok(ws.on_upgrade(move |socket| public_multi_socket(socket, hub)))
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

fn public_command_channel(command: &PublicWsCommand) -> AppResult<WebSocketChannel> {
    let symbol = command
        .symbol
        .as_deref()
        .ok_or_else(|| crate::error::AppError::Validation("symbol is required".to_owned()))?;
    match command.channel.as_str() {
        "ticker" | "depth" | "trade" => public_channel(command.channel.clone(), symbol.to_owned()),
        "kline" => {
            let interval = command.interval.as_deref().ok_or_else(|| {
                crate::error::AppError::Validation("interval is required".to_owned())
            })?;
            public_channel("kline".to_owned(), format!("{symbol}_{interval}"))
        }
        _ => Err(crate::error::AppError::Validation(
            "unsupported websocket channel".to_owned(),
        )),
    }
}

async fn public_multi_socket(socket: WebSocket, hub: Option<EventBroadcastHub>) {
    let (mut sender, mut receiver) = socket.split();
    let mut subscription = hub.map(|hub| hub.subscribe_multi());
    let mut channels = HashSet::<WebSocketChannel>::new();

    loop {
        tokio::select! {
            message = receiver.next() => {
                if !handle_public_multi_client_message(message, &mut sender, &mut channels).await {
                    break;
                }
            }
            broadcast = recv_multi_broadcast(&mut subscription), if subscription.is_some() => {
                let Ok(message) = broadcast else {
                    break;
                };
                if channels.contains(message.channel())
                    && sender.send(Message::Text(message.payload().to_owned())).await.is_err()
                {
                    break;
                }
            }
        }
    }
}

async fn recv_multi_broadcast(
    subscription: &mut Option<EventBroadcastMultiSubscription>,
) -> AppResult<crate::modules::events::EventBroadcastMessage> {
    let Some(subscription) = subscription else {
        return Err(crate::error::AppError::Internal(
            "event broadcast hub is not configured".to_owned(),
        ));
    };
    subscription.recv().await
}

async fn handle_public_multi_client_message(
    message: Option<Result<Message, axum::Error>>,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    channels: &mut HashSet<WebSocketChannel>,
) -> bool {
    match message {
        Some(Ok(Message::Text(text))) if text == "ping" => {
            sender.send(Message::Text("pong".to_owned())).await.is_ok()
        }
        Some(Ok(Message::Text(text))) => handle_public_ws_command(text, sender, channels).await,
        Some(Ok(Message::Ping(payload))) => sender.send(Message::Pong(payload)).await.is_ok(),
        Some(Ok(Message::Close(_))) | Some(Err(_)) | None => false,
        Some(Ok(_)) => true,
    }
}

async fn handle_public_ws_command(
    text: String,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    channels: &mut HashSet<WebSocketChannel>,
) -> bool {
    let response = match serde_json::from_str::<PublicWsCommand>(&text)
        .map_err(|error| crate::error::AppError::Validation(format!("invalid json: {error}")))
        .and_then(|command| {
            let channel = public_command_channel(&command)?;
            match command.op.as_str() {
                "subscribe" => {
                    channels.insert(channel.clone());
                    Ok(format!(
                        r#"{{"type":"subscribed","channel":"{}"}}"#,
                        channel.as_text()
                    ))
                }
                "unsubscribe" => {
                    channels.remove(&channel);
                    Ok(format!(
                        r#"{{"type":"unsubscribed","channel":"{}"}}"#,
                        channel.as_text()
                    ))
                }
                _ => Err(crate::error::AppError::Validation(
                    "unsupported websocket operation".to_owned(),
                )),
            }
        }) {
        Ok(response) => response,
        Err(error) => serde_json::json!({
            "type": "error",
            "code": "invalid_request",
            "message": error.to_string(),
        })
        .to_string(),
    };
    sender.send(Message::Text(response)).await.is_ok()
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
