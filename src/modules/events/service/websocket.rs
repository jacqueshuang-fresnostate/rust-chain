//! WebSocket 频道规范化、鉴权、连接循环与进程内广播。
//!
//! 这里的广播是纯进程内的尽力投递，与 outbox/inbox 那条持久化链路完全独立：
//! 消息不落库、不重试、不补发，进程重启即全部丢失，订阅也只覆盖建立之后产生的消息，
//! 因此断线重连不会拿到期间错过的内容，客户端必须靠查询接口补齐状态。
//! 慢消费者会因缓冲区溢出被跳过若干条而不是阻塞发送方，这是为了保证一个卡住的连接不拖垮整个广播。
//!
//! 由此引出一条全局约束：任何业务事件都必须在数据库事务提交成功之后才广播。
//! 广播无法回滚，若在提交前推送，事务一旦回滚客户端就会看到实际不存在的数据。
//!
//! 频道命名分公共与私有两类，私有频道由已鉴权的用户编号唯一确定，客户端无法通过订阅命令切换收听对象，
//! 公共频道则要求命名空间与主题都通过安全字符校验后才能构造。

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

/// 一个广播频道的标识，由命名空间与主题两段构成，是订阅匹配与消息路由的键。
/// 实现了哈希与相等，因此可直接放进集合表示某条连接当前的订阅集。
/// 公共频道的两段都经过安全字符校验；私有频道的命名空间固定，主题由用户编号生成。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WebSocketChannel {
    /// 命名空间，公共频道为行情类别，私有频道固定为 `private`。
    pub namespace: String,
    /// 主题，公共频道为交易对或交易对加周期，私有频道为 `user:<编号>`。
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

    /// 构造代理私有频道；调用方必须传入由 agent token 与服务端账号关系解析的精确代理 ID。
    /// 频道主题固定为 `agent:<id>`，不接受物化路径或顶级代理 ID，因此父代订阅不会收到子代的客服刷新提示。
    pub fn private_agent(agent_id: u64) -> Self {
        Self {
            namespace: "private".to_owned(),
            topic: format!("agent:{agent_id}"),
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

/// 私有 WebSocket 的鉴权结论，只保留一个用户编号。
/// 收敛成单一编号是刻意的：连接建立后不再持有令牌，也无从获得除该用户以外任何频道的访问能力。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateWsAuth {
    /// 已通过校验的用户编号，直接决定本连接绑定的私有频道。
    pub user_id: u64,
}

/// 代理私有 WebSocket 的鉴权结论，只保留服务端解析的精确代理 ID。
/// 不保留代理管理员 ID、root ID 或 path，连接建立后因此无法切换到父级或子级频道。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentPrivateWsAuth {
    /// 已验证的精确代理主键，直接决定订阅的 `private:agent:<id>` 频道。
    pub agent_id: u64,
}

impl PrivateWsAuth {
    /// 兼容旧 JWT 测试路径，从原始查询串提取 token 并仅接受 user scope。
    /// 与生产路径的关键差别是这里只做本地 JWT 签名校验，不查询会话存储，
    /// 因此无法感知已被服务端撤销的令牌，生产连接必须走带会话校验的那条路径。
    /// 查询串按 `&` 与 `=` 手工切分并取首个非空 token 参数，不做 URL 解码。
    /// 令牌缺失或签名非法返回未授权，签名有效但作用域不是用户则返回禁止，两者刻意区分以便排障。
    /// subject 必须严格形如 `user:{数字}`，解析失败同样按未授权处理。
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
/// 规范化的意义在于让同一交易对的不同书写形式落到同一频道，否则订阅方与广播方会因大小写或分隔符差异而错过消息。
/// K 线主题按最后一个下划线切分成交易对与周期两段，两段各自规范化后再用下划线重新拼回；
/// 主题中不含下划线时不做切分，直接按普通频道走安全字符校验，以兼容历史路径。
/// 未识别的命名空间同样只走安全字符校验而不报错，保留旧单频道路由继续可用。
/// 任何非法参数都在 socket 升级之前失败，本函数不创建订阅、不注册连接、不广播任何消息。
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
/// 与路径入口共用同一套规范化，保证经命令订阅与经路径订阅得到的频道名逐字相同，不会出现只差写法的两个频道。
/// 四类频道都必须给出交易对，K 线额外要求周期，缺任一项返回校验错误；
/// 命令里的频道名不在白名单内也直接拒绝，这与路径入口放行未知命名空间的宽松策略不同，
/// 因为连接内命令是新接口无需兼容历史路径。
/// 解析失败不改变当前连接的订阅集合，也不影响其他连接。
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

/// 从令牌 subject 解析用户编号，要求严格形如 `user:{数字}`。
/// 前缀不符或数字解析失败一律返回未授权而非参数错误，避免向调用方泄露 subject 的具体格式问题。
/// 解析结果直接决定该连接能收到谁的私有事件，因此不接受任何宽松匹配。
fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 将已校验频道编码为稳定 `{"type":"subscribed","channel":"..."}` 确认帧。
/// 单频道公共连接与私有连接都在握手后立即发送它，作为客户端可以开始收数据的信号。
/// 只构造文本：既不代表 hub 已注册订阅，也不代表该频道存在可补发的历史消息。
pub(crate) fn public_ws_confirmation_text(channel: &WebSocketChannel) -> String {
    serde_json::json!({
        "type": "subscribed",
        "channel": channel.as_text(),
    })
    .to_string()
}

/// 校验频道名的单个片段：非空、长度不超过 64 字节，且只含 ASCII 字母数字与短横线下划线。
/// 字符集收紧是必要的，频道名会被拼进冒号分隔的频道文本并参与匹配，
/// 放行冒号或空白会让不同片段拼出相同的频道文本，从而造成订阅串台。
/// 校验通过后原样返回，不做大小写折叠也不裁剪空白。
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

/// 为已在升级前鉴权的代理订阅唯一 `private:agent:<id>` 频道。
/// 该频道只传送不含正文的客服刷新提示，不存储、不重放，掉线后必须以 REST 重新对齐。
/// hub 缺失时连接仍保留 ping/pong，不得把是否收到提示当成消息提交成功的依据。
pub(crate) async fn run_agent_private_socket(
    socket: WebSocket,
    auth: AgentPrivateWsAuth,
    hub: Option<EventBroadcastHub>,
) {
    let channel = WebSocketChannel::private_agent(auth.agent_id);
    let subscription = hub.map(|hub| hub.subscribe(&channel));
    run_subscription_socket(socket, public_ws_confirmation_text(&channel), subscription).await;
}

/// 运行单频道 socket 生命周期；先发送确认，再在客户端帧与广播之间并发转发。
/// 确认帧发送失败即直接返回，不进入循环，因为连接已不可用。
/// 有订阅时用二选一等待同时照看客户端输入与广播输出，两侧任一出错即结束会话；
/// 未配置广播 hub 时退化为只处理保活帧的循环，连接仍能维持但永远收不到数据。
/// 该实现被公共单频道与私有连接共用，两者仅在订阅目标与确认文案上不同。
/// 全程无持久化，断线期间的消息不会缓存，重连后的状态补齐由客户端自行完成。
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

/// 处理多频道连接的一帧输入，返回值表示会话是否继续。
/// 纯文本 `ping` 与协议层 Ping 分别回以文本 pong 和协议 Pong，两种保活形式都支持是为了兼容不同客户端库。
/// 其余文本帧一律当作订阅命令解析，命令只影响当前连接的订阅集合，不触及其他连接。
/// 收到关闭帧、读取出错或流结束都返回假以终止会话；二进制等其他帧型被忽略但保持连接。
/// 本函数不写数据库、不改全局状态。
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

/// 解析订阅或退订命令并更新当前连接的订阅集合，返回值表示会话是否继续。
/// JSON 非法、频道参数不合规或操作名不在白名单，都会被折叠成同一种 `invalid_request` 错误响应发回客户端，
/// 而不是断开连接，这样客户端拼错一条命令不会导致整个会话中断。
/// 由于订阅集合是哈希集合，重复订阅与重复退订天然幂等，客户端无需自行去重。
/// 只有发送响应失败才返回假终止会话。
/// 命令只作用于本连接的内存集合，不写数据库、不发布任何消息、不影响其他连接。
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

/// 构造订阅或退订的应答文本，两种操作共用同一形状只是类型字段不同。
/// 应答只表示服务端已更新本连接的订阅集合，不代表该频道当前有数据，也不代表存在可补发的历史消息。
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

/// 一条待广播的消息，由目标频道与已序列化的载荷文本组成。
/// 载荷以字符串形态携带而非结构化值，因为 hub 不解释内容，只按频道转发给匹配的订阅者。
/// 构造出实例并不等于已发送，必须显式交给 hub 发布。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventBroadcastMessage {
    /// 目标频道，订阅者据此决定是否接收本条消息。
    channel: WebSocketChannel,
    /// 待发送的载荷原文，通常是 JSON 文本，hub 原样转发不做校验。
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

    /// 构造单代理私有广播；代理 ID 必须来自服务端会话归属，payload 应只作刷新提示。
    /// 相同输入只构造频道与文本，不实际发送、不落库也不保证代理当时在线。
    pub fn private_agent(agent_id: u64, payload: impl Into<String>) -> Self {
        Self {
            channel: WebSocketChannel::private_agent(agent_id),
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

    /// 返回消息目标频道的只读引用，供订阅端判断是否应接收本条消息。
    /// 不重新校验频道合法性、不分配内存，也不改变任何广播状态。
    pub fn channel(&self) -> &WebSocketChannel {
        &self.channel
    }

    /// 返回待广播的载荷原文，将被原样作为文本帧发给客户端。
    /// hub 不解析该内容，因此调用方不得据此假定它是合法 JSON，也不代表对应事件已持久化。
    pub fn payload(&self) -> &str {
        &self.payload
    }
}

/// 进程内广播中枢，所有 WebSocket 连接都从它派生订阅。
/// 基于容量有限的广播通道实现，因此消息只在内存中存在：不落库、不重试、不补发，进程重启即清空。
/// 可自由克隆，克隆出的句柄共享同一条通道，业务各处因此可以各持一份用于发布。
#[derive(Clone)]
pub struct EventBroadcastHub {
    /// 广播发送端，容量满时最慢的订阅者会丢失若干条而非阻塞发送方。
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

/// 不做频道过滤的订阅，收取 hub 上的全部消息。
/// 供多频道连接使用：该连接的订阅集合在运行期动态变化，因此过滤放在连接循环里按当前集合判断，
/// 而不是固化在订阅上。
pub struct EventBroadcastMultiSubscription {
    /// 广播接收端，只能收到订阅创建之后产生的消息。
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

/// 绑定单一频道的订阅，接收时自动丢弃其他频道的消息。
/// 供单频道公共连接与私有连接使用，二者的订阅目标在握手时就已固定，运行期不可更改。
/// 注意过滤发生在接收端而非发送端，因此其他频道的高频消息仍会占用本订阅的缓冲区容量。
pub struct EventBroadcastSubscription {
    /// 本订阅关心的唯一频道。
    channel: WebSocketChannel,
    /// 广播接收端，只能收到订阅创建之后产生的消息。
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
