//! WebSocket 频道规范化、鉴权、连接循环与进程内广播。

use crate::{
    config::Settings,
    error::{AppError, AppResult},
    modules::{
        auth::{TokenScope, decode_claims},
        events::presentation::PublicWsCommand,
        market::{KlineUpsertKey, ValidatedMarketSymbol, adapters::MarketFeedEvent},
    },
};
use axum::extract::ws::{Message, WebSocket};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashSet;
use tokio::sync::broadcast::{self, error::RecvError};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WebSocketChannel {
    pub namespace: String,
    pub topic: String,
}

impl WebSocketChannel {
    /// 构造并校验公共频道；namespace/topic 只能包含安全 ASCII 段且长度不超过 64。
    /// 该纯函数不持有连接、不产生广播副作用，非法输入返回 validation error，重复调用结果一致。
    pub fn public(namespace: impl Into<String>, topic: impl Into<String>) -> AppResult<Self> {
        let namespace = validate_ws_segment(namespace.into(), "websocket namespace")?;
        let topic = validate_ws_segment(topic.into(), "websocket topic")?;
        Ok(Self { namespace, topic })
    }

    /// 构造用户私有频道；调用方必须传入已鉴权的用户 ID，本函数不再次访问会话或数据库。
    /// 相同用户始终映射到同一频道文本，不产生 I/O 或持久化副作用。
    pub fn private_user(user_id: u64) -> Self {
        Self {
            namespace: "private".to_owned(),
            topic: format!("user:{user_id}"),
        }
    }

    /// 序列化稳定频道路径；私有频道保持 `private:user:<id>`，公共频道保持既有三段格式。
    /// 仅分配字符串，不校验权限、不读写外部状态。
    pub fn as_text(&self) -> String {
        if self.namespace == "private" {
            return format!("private:{}", self.topic);
        }
        format!("public:{}:{}", self.namespace, self.topic)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateWsAuth {
    pub user_id: u64,
}

impl PrivateWsAuth {
    /// 兼容旧 JWT 测试路径，从原始查询串提取 token 并仅接受 user scope。
    /// 缺失/非法 token 返回未授权，非用户 scope 返回禁止；无数据库事务与外部副作用。
    pub fn from_query(query: Option<&str>, settings: &Settings) -> AppResult<Self> {
        let token = query
            .and_then(|query| {
                query.split('&').find_map(|pair| {
                    let (key, value) = pair.split_once('=')?;
                    (key == "token" && !value.is_empty()).then_some(value)
                })
            })
            .ok_or(AppError::Unauthorized)?;
        let claims = decode_claims(settings, token)?;
        if claims.scope != TokenScope::User {
            return Err(AppError::Forbidden);
        }
        let user_id = claims
            .sub
            .strip_prefix("user:")
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or(AppError::Unauthorized)?;

        Ok(Self { user_id })
    }

    /// 从已验证的用户 subject 构造私有 WebSocket 身份；subject 必须严格为 `user:<u64>`。
    /// 该纯函数不访问会话后端、不消费 token；非法 subject 返回未授权且无任何副作用。
    pub(crate) fn from_user_subject(subject: &str) -> AppResult<Self> {
        Ok(Self {
            user_id: user_id_from_subject(subject)?,
        })
    }
}

/// 把兼容路径参数映射为 `public:<namespace>:<topic>`：ticker/depth/trade 规范化交易对，kline 同时规范化交易对与周期。
/// 未识别 namespace 仍走安全段校验以保留旧单频道路由；非法参数在升级 socket 前失败，本函数不创建订阅或广播。
pub(crate) fn public_channel(namespace: String, topic: String) -> AppResult<WebSocketChannel> {
    match namespace.as_str() {
        "ticker" | "depth" | "trade" => Ok(WebSocketChannel::public(
            namespace,
            ValidatedMarketSymbol::from_raw(&topic)
                .map_err(|error| AppError::Validation(error.to_string()))?
                .as_str(),
        )?),
        "kline" => {
            let Some((symbol, interval)) = topic.rsplit_once('_') else {
                return WebSocketChannel::public(namespace, topic);
            };
            let symbol = ValidatedMarketSymbol::from_raw(symbol)
                .map_err(|error| AppError::Validation(error.to_string()))?;
            let interval = KlineUpsertKey::new(interval, Utc::now())
                .map_err(|error| AppError::Validation(error.to_string()))?
                .interval()
                .to_owned();
            WebSocketChannel::public(namespace, format!("{}_{}", symbol.as_str(), interval))
        }
        _ => WebSocketChannel::public(namespace, topic),
    }
}

/// 把连接内命令限制到 ticker/depth/trade/kline 四类公共频道，并复用路径入口的交易对和周期规范化规则。
/// 所有频道都要求 symbol，kline 额外要求 interval；解析失败不改变当前连接的订阅集合，也不触及其他 WebSocket 会话。
pub(crate) fn public_command_channel(command: &PublicWsCommand) -> AppResult<WebSocketChannel> {
    let symbol = command
        .symbol
        .as_deref()
        .ok_or_else(|| AppError::Validation("symbol is required".to_owned()))?;
    match command.channel.as_str() {
        "ticker" | "depth" | "trade" => public_channel(command.channel.clone(), symbol.to_owned()),
        "kline" => {
            let interval = command
                .interval
                .as_deref()
                .ok_or_else(|| AppError::Validation("interval is required".to_owned()))?;
            public_channel("kline".to_owned(), format!("{symbol}_{interval}"))
        }
        _ => Err(AppError::Validation(
            "unsupported websocket channel".to_owned(),
        )),
    }
}

fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 将已校验频道编码为稳定 `{"type":"subscribed","channel":"..."}` 确认帧；只构造文本，不代表 hub 已持久化订阅或存在历史消息。
pub(crate) fn public_ws_confirmation_text(channel: &WebSocketChannel) -> String {
    serde_json::json!({
        "type": "subscribed",
        "channel": channel.as_text(),
    })
    .to_string()
}

fn validate_ws_segment(value: String, field: &str) -> AppResult<String> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(AppError::Validation(format!("invalid {field}")));
    }

    Ok(value)
}

/// 运行公共多频道会话：仅把当前连接显式订阅的 ticker/depth/trade/kline 广播转发给客户端，并在连接内处理订阅增删和保活。
/// hub 订阅发生在 socket 升级后且只接收此后消息；未配置 hub 时仅处理命令/保活，慢客户端 lag、hub 关闭、读写错误或断连都会结束会话，无持久游标或补发。
pub(crate) async fn run_public_multi_socket(socket: WebSocket, hub: Option<EventBroadcastHub>) {
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

/// 运行兼容公共单频道会话：先发送订阅确认，再仅转发与路径频道完全匹配的进程内广播。
/// 订阅不持久化且只覆盖连接建立后的消息；hub 缺失时保留保活，断线重连不会补发错过的行情。
pub(crate) async fn run_public_socket(
    socket: WebSocket,
    channel: WebSocketChannel,
    hub: Option<EventBroadcastHub>,
    confirmation: String,
) {
    let subscription = hub.map(|hub| hub.subscribe(&channel));
    run_subscription_socket(socket, confirmation, subscription).await;
}

/// 为已鉴权用户订阅唯一 `private:user:<id>` 频道并转发提交后产生的进程内私有事件。
/// 身份必须在升级前完成校验；订阅无历史持久化或重放保证，hub 缺失时只保留保活，断线不会影响业务事务。
pub(crate) async fn run_private_socket(
    socket: WebSocket,
    auth: PrivateWsAuth,
    hub: Option<EventBroadcastHub>,
) {
    let channel = WebSocketChannel::private_user(auth.user_id);
    let subscription = hub.map(|hub| hub.subscribe(&channel));
    run_subscription_socket(socket, public_ws_confirmation_text(&channel), subscription).await;
}

/// 运行单频道 socket 生命周期；先发送确认，再在客户端帧与广播之间并发转发。
/// 断连、发送失败或广播关闭都会结束循环；无持久化事务，重连与消息重放由客户端负责。
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

/// 处理多频道连接的一帧输入；保活帧直接响应，文本命令只更新当前连接内订阅集合。
/// 坏帧或断连返回 false 结束连接，不修改全局状态或持久化数据。
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

/// 解析 subscribe/unsubscribe JSON 命令；错误转换为既有 `invalid_request` 文本响应。
/// 更新仅作用于当前连接集合，重复订阅/取消天然幂等，不进行数据库或消息发布。
async fn handle_public_ws_command(
    text: String,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    channels: &mut HashSet<WebSocketChannel>,
) -> bool {
    let response = match serde_json::from_str::<PublicWsCommand>(&text)
        .map_err(|error| AppError::Validation(format!("invalid json: {error}")))
        .and_then(|command| {
            let channel = public_command_channel(&command)?;
            match command.op.as_str() {
                "subscribe" => {
                    channels.insert(channel.clone());
                    Ok(public_ws_subscription_response(
                        "subscribed",
                        &channel.as_text(),
                    ))
                }
                "unsubscribe" => {
                    channels.remove(&channel);
                    Ok(public_ws_subscription_response(
                        "unsubscribed",
                        &channel.as_text(),
                    ))
                }
                _ => Err(AppError::Validation(
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

fn public_ws_subscription_response(message_type: &str, channel: &str) -> String {
    serde_json::json!({
        "type": message_type,
        "channel": channel,
    })
    .to_string()
}

/// 处理单频道连接保活帧；文本 `ping` 与协议 Ping 分别返回 pong，其余业务帧忽略。
/// 发送失败、关闭或读取错误返回 false；不产生持久化或广播副作用。
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

/// 从可选多频道订阅接收下一条广播；未配置 hub 时返回明确内部错误。
/// 本入口不重放历史事件，也不修改外部状态。
async fn recv_multi_broadcast(
    subscription: &mut Option<EventBroadcastMultiSubscription>,
) -> AppResult<EventBroadcastMessage> {
    let Some(subscription) = subscription else {
        return Err(AppError::Internal(
            "event broadcast hub is not configured".to_owned(),
        ));
    };
    subscription.recv().await
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventBroadcastMessage {
    channel: WebSocketChannel,
    payload: String,
}

impl EventBroadcastMessage {
    /// 构造公共广播消息；频道必须已通过校验，payload 原样保留以维持 JSON 合同。
    /// 本函数只构造值，不实际发布，因此无事务、重试或外部副作用。
    pub fn public(channel: WebSocketChannel, payload: impl Into<String>) -> Self {
        Self {
            channel,
            payload: payload.into(),
        }
    }

    /// 构造单用户私有广播消息；调用方负责用户授权与 payload 脱敏。
    /// 相同输入生成相同路由，不直接发送、不持久化，也不保证客户端在线。
    pub fn private_user(user_id: u64, payload: impl Into<String>) -> Self {
        Self {
            channel: WebSocketChannel::private_user(user_id),
            payload: payload.into(),
        }
    }

    /// 将市场 feed 事件映射为公共广播，保持 provider payload 和频道规则不变。
    /// 频道不合法时失败；成功仅构造消息，不写 outbox、不发布网络消息。
    pub fn from_market_feed_event(event: &MarketFeedEvent) -> AppResult<Self> {
        Ok(Self::public(
            WebSocketChannel::public(event.public_ws_namespace(), event.public_ws_topic())?,
            event.payload().to_string(),
        ))
    }

    /// 返回消息的只读频道引用；不重新校验、不分配，也不改变广播状态。
    pub fn channel(&self) -> &WebSocketChannel {
        &self.channel
    }

    /// 返回待广播 payload 原文；调用方不得据此假定 JSON 已解析或事件已持久化。
    pub fn payload(&self) -> &str {
        &self.payload
    }
}

#[derive(Clone)]
pub struct EventBroadcastHub {
    sender: broadcast::Sender<EventBroadcastMessage>,
}

impl EventBroadcastHub {
    /// 创建进程内广播 hub；capacity 最小收敛为 1，历史消息不持久化且重启后丢失。
    /// 构造不启动任务；慢订阅者可能 lag，由 recv 语义跳过陈旧消息。
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity.max(1));
        Self { sender }
    }

    /// 创建单频道订阅；只接收创建后的消息，并在 recv 时过滤其他频道。
    /// 重复调用产生独立 receiver，不修改 hub，也不承诺历史重放。
    pub fn subscribe(&self, channel: &WebSocketChannel) -> EventBroadcastSubscription {
        EventBroadcastSubscription {
            channel: channel.clone(),
            receiver: self.sender.subscribe(),
        }
    }

    /// 创建未过滤的多频道订阅；调用方负责按当前连接集合筛选消息。
    /// 订阅只覆盖创建后的进程内广播，无数据库事务或历史重放。
    pub fn subscribe_multi(&self) -> EventBroadcastMultiSubscription {
        EventBroadcastMultiSubscription {
            receiver: self.sender.subscribe(),
        }
    }

    /// 向进程内订阅者尽力发布；无接收者或通道关闭时静默丢弃以保持非阻塞语义。
    /// 不写 outbox、无重试/事务保证，关键事件必须由调用方另行持久化。
    pub fn publish(&self, message: EventBroadcastMessage) {
        let _ = self.sender.send(message);
    }
}

pub struct EventBroadcastMultiSubscription {
    receiver: broadcast::Receiver<EventBroadcastMessage>,
}

impl EventBroadcastMultiSubscription {
    /// 接收下一条多频道广播；lagged 时跳过缺失历史，通道关闭返回内部错误。
    /// 不持久化消费游标，取消任务后没有重放保证。
    pub async fn recv(&mut self) -> AppResult<EventBroadcastMessage> {
        loop {
            match self.receiver.recv().await {
                Ok(message) => return Ok(message),
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => {
                    return Err(AppError::Internal(
                        "event broadcast channel is closed".to_owned(),
                    ));
                }
            }
        }
    }
}

pub struct EventBroadcastSubscription {
    channel: WebSocketChannel,
    receiver: broadcast::Receiver<EventBroadcastMessage>,
}

impl EventBroadcastSubscription {
    /// 接收下一条匹配频道的广播；其他频道和 lagged 通知被跳过，关闭时返回内部错误。
    /// 不持久化消费游标，断线重连不会补发历史消息。
    pub async fn recv(&mut self) -> AppResult<EventBroadcastMessage> {
        loop {
            match self.receiver.recv().await {
                Ok(message) if message.channel() == &self.channel => return Ok(message),
                Ok(_) | Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => {
                    return Err(AppError::Internal(
                        "event broadcast channel is closed".to_owned(),
                    ));
                }
            }
        }
    }
}
