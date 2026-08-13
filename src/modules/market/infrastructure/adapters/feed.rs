//! 行情 feed 编排基础设施。
//!
//! 负责 WebSocket/REST 原始帧的有限流处理、失败汇总、持久化后广播和 provider 配置组装；
//! 单帧失败保持隔离，只有 ingestion 成功的行情才进入公开实时事件。
//!
//! 本文件处在 provider 适配与 ingestion 之间：向上游拿到「provider + 频道 + 原始文本」三元组，
//! 向下游交出已落地的领域快照和统一格式的公开事件，自身不直接读写 Redis 或 Mongo。
//! 单帧处理顺序固定为解析、构造事件、按频道分派 sink 持久化、最后广播，
//! 其中 trade 帧只广播不落地，因为逐笔成交没有权威缓存需求。
//! 广播排在持久化之后是硬约束，避免客户端看到尚未落库的行情；
//! 但事件转换失败发生在写入之后，此时会返回错误而不会回滚已经完成的 Redis/Mongo 写入。
//! 失败隔离贯穿两条入口：WebSocket 有限流逐帧计数，任一帧解析或摄取失败只累加 `failed` 并继续；
//! REST 兜底则会连同 provider、频道、交易对、URL 与错误文本一起记录明细，供断线恢复时定位是哪个请求失败。
//! 需要注意汇总结果只表明处理过程，不代表价格源可用，调用方必须自行校验本轮至少有一次有效写入。
//! 配置组装侧统一在建连前完成交易对与周期校验，把非法配置拦截在产生网络副作用之前。

use super::{
    ingestion::{MarketIngestionService, MarketIngestionSink},
    provider::{
        BitgetMarketAdapter, CoinbaseMarketAdapter, HtxMarketAdapter, MarketFeedProvider,
        bitget_rest_kline_payloads, bitget_rest_ticker_payload, coinbase_rest_kline_payloads,
        coinbase_rest_ticker_payload, htx_rest_kline_payloads, htx_rest_ticker_payload,
        market_feed_payload_hash, provider_name, validation_error,
    },
};
use crate::{
    config::Settings,
    error::{AppError, AppResult},
    modules::{
        events::{EventBroadcastHub, EventBroadcastMessage, EventOutboxWriter},
        market::{
            KlineUpsertKey, MarketDepthSnapshot, MarketKlineSnapshot, MarketTickerSnapshot,
            MarketTradeTick, ValidatedMarketSymbol,
        },
    },
    state::AppState,
};
use axum::async_trait;
use chrono::Utc;
use futures_util::{Stream, StreamExt};
use serde_json::{Value, json};
use std::collections::VecDeque;

#[async_trait]
pub trait MarketFeedRestFallbackHttpClient: Clone + Send + Sync + 'static {
    /// 在受控超时内读取 REST 兜底响应正文；非成功状态或网络错误不得伪装为空行情。
    /// 抽象成 trait 是为了让测试注入可控响应，无需真实网络即可覆盖失败隔离与解析分支。
    /// 实现必须自带超时，否则单个挂起的请求会阻塞整个 provider 的兜底队列。
    async fn get_text(&self, url: &str) -> AppResult<String>;
}

const REST_FALLBACK_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

#[derive(Clone)]
pub struct ReqwestMarketFeedRestFallbackHttpClient {
    client: reqwest::Client,
    timeout: std::time::Duration,
}

impl Default for ReqwestMarketFeedRestFallbackHttpClient {
    /// 以平台默认的三秒兜底超时新建客户端，供未接入配置的测试与工具直接使用。
    /// 生产路径应改用从配置构造的入口，以免超时值与运维实际设定脱节。
    fn default() -> Self {
        Self::with_timeout(REST_FALLBACK_REQUEST_TIMEOUT)
    }
}

impl ReqwestMarketFeedRestFallbackHttpClient {
    /// 注入共享 Reqwest client，并使用平台默认的三秒 REST 兜底超时。
    /// 构造阶段不发送请求；DNS、TLS、HTTP 状态和正文读取错误由 `get_text` 在实际调用时返回。
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            timeout: REST_FALLBACK_REQUEST_TIMEOUT,
        }
    }

    /// 新建默认 Reqwest client，并把调用方给定时长用于每次 REST 兜底请求。
    /// 本函数不校验零时长，也不建立连接；超时只在后续 `get_text` 请求中生效。
    pub fn with_timeout(timeout: std::time::Duration) -> Self {
        Self {
            client: reqwest::Client::new(),
            timeout,
        }
    }

    /// 从运行配置读取 REST 兜底超时秒数，并使用默认 Reqwest client 构造适配器。
    /// 这里只转换时长，不解析 provider 响应，也不验证外部地址可达性。
    pub fn from_settings(settings: &Settings) -> Self {
        Self::with_timeout(std::time::Duration::from_secs(
            settings.market_feed_rest_fallback_timeout_seconds,
        ))
    }

    /// 返回本客户端对每次 REST 兜底请求施加的超时时长，主要供配置核对与测试断言使用。
    /// 该超时覆盖建连到读完正文的全过程，且按单次请求计算，兜底队列整体耗时会随请求数量线性放大。
    pub fn timeout(&self) -> std::time::Duration {
        self.timeout
    }
}

#[async_trait]
impl MarketFeedRestFallbackHttpClient for ReqwestMarketFeedRestFallbackHttpClient {
    /// 以配置的超时发起一次 GET 并返回响应正文，三类失败分别包装成可区分的内部错误文本。
    /// 依次是请求阶段失败（DNS、连接、超时）、状态码非 2xx、以及正文读取失败，
    /// 分开措辞是为了让 REST 兜底的失败明细能直接看出卡在哪一步。
    /// 非成功状态一律转为错误，绝不把错误页正文当作行情返回，避免解析器把它当成空数据静默跳过。
    async fn get_text(&self, url: &str) -> AppResult<String> {
        let response = self
            .client
            .get(url)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|error| {
                AppError::Internal(format!("market feed REST fallback request failed: {error}"))
            })?
            .error_for_status()
            .map_err(|error| {
                AppError::Internal(format!("market feed REST fallback status failed: {error}"))
            })?;
        response.text().await.map_err(|error| {
            AppError::Internal(format!("market feed REST fallback body failed: {error}"))
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketFeedChannel {
    Ticker,
    Depth,
    Kline,
    Trade,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketFeedFrame {
    provider: MarketFeedProvider,
    channel: MarketFeedChannel,
    payload: String,
}

impl MarketFeedFrame {
    /// 标记原始行情载荷的 provider 与频道，供统一解析器选择正确适配器。
    /// payload 原样保留且不在构造时解析；非法 JSON 会在 [`MarketFeedEvent::from_frame`] 返回错误。
    pub fn new(
        provider: MarketFeedProvider,
        channel: MarketFeedChannel,
        payload: impl Into<String>,
    ) -> Self {
        Self {
            provider,
            channel,
            payload: payload.into(),
        }
    }

    /// 把 Bitget `ticker` 频道的原始文本打上标记，后续将由 Bitget ticker 解析器读取 `data` 首个对象。
    /// 该频道的最新价、成交量与观察时间是权威价格的来源，缺任一必填项都会在解析阶段被拒绝。
    /// 本函数不解析、不持久化，也不广播，仅仅记录这段文本应该交给哪个适配器。
    pub fn bitget_ticker(payload: impl Into<String>) -> Self {
        Self::new(
            MarketFeedProvider::Bitget,
            MarketFeedChannel::Ticker,
            payload,
        )
    }

    /// 把 Bitget `books50` 盘口文本打上 depth 标记，档位在解析阶段按 `[价格, 数量]` 二元数组读取。
    /// 该频道最多推送 50 档，深度快照采用覆盖写入，因此乱序到达的旧盘口有可能盖掉新盘口。
    /// 盘口格式校验留给 provider 适配器，这里既不检查档位数量也不判断买卖价是否交叉。
    pub fn bitget_depth(payload: impl Into<String>) -> Self {
        Self::new(
            MarketFeedProvider::Bitget,
            MarketFeedChannel::Depth,
            payload,
        )
    }

    /// 把 Bitget `candle*` 文本打上 kline 标记，解析时按位置读取数组首行的开盘毫秒与 OHLCV。
    /// 周期不在载荷字段里，而是从帧级 `arg.channel` 去掉 `candle` 前缀后反解，频道名缺失整帧会被拒绝。
    /// 周期与 OHLC 校验都在后续解析阶段执行，这里不判断该周期是否属于平台白名单。
    pub fn bitget_kline(payload: impl Into<String>) -> Self {
        Self::new(
            MarketFeedProvider::Bitget,
            MarketFeedChannel::Kline,
            payload,
        )
    }

    /// 把 Bitget `trade` 文本打上 trade 标记，解析时只取 `data` 首条成交，同帧其余成交会被丢弃。
    /// 逐笔成交不进入 Redis 或 Mongo，摄取阶段直接跳过存储，只用于对外广播实时成交流。
    /// 成交字段在后续解析阶段转换，方向无法识别的成交会被拒绝而不是默认成买入。
    pub fn bitget_trade(payload: impl Into<String>) -> Self {
        Self::new(
            MarketFeedProvider::Bitget,
            MarketFeedChannel::Trade,
            payload,
        )
    }

    /// 把 HTX `market.{symbol}.detail` 文本打上 ticker 标记，解析时数值全部取自 `tick` 对象。
    /// 交易对不在字段里，只能从频道名按点号切分取第二段，因此频道名缺失会导致整帧无法归属交易对。
    /// 本函数不解析、不持久化，也不广播，只记录该文本归属的 provider 与频道。
    pub fn htx_ticker(payload: impl Into<String>) -> Self {
        Self::new(MarketFeedProvider::Htx, MarketFeedChannel::Ticker, payload)
    }

    /// 把 HTX `market.{symbol}.depth.step0` 文本打上 depth 标记，档位读自 `tick.bids` 与 `tick.asks`。
    /// step0 表示不做价格聚合的原始档位，因此这里拿到的是 HTX 侧最细粒度的盘口。
    /// 盘口格式校验留给 provider 适配器，本函数不检查档位是否为空。
    pub fn htx_depth(payload: impl Into<String>) -> Self {
        Self::new(MarketFeedProvider::Htx, MarketFeedChannel::Depth, payload)
    }

    /// 把 HTX `market.{symbol}.kline.{period}` 文本打上 kline 标记，开盘时间取 `tick.id` 的秒级时间戳。
    /// 周期从频道名末段反解，`60min` 与 `1hour` 都会映射为平台的 `1h`。
    /// 周期与 OHLC 校验在后续解析阶段执行，未识别的末段会让整帧被拒绝。
    pub fn htx_kline(payload: impl Into<String>) -> Self {
        Self::new(MarketFeedProvider::Htx, MarketFeedChannel::Kline, payload)
    }

    /// 把 HTX `market.{symbol}.trade.detail` 文本打上 trade 标记，成交列表嵌在 `tick.data` 中且只取首条。
    /// 与 Bitget 相比多出一层嵌套，但下游语义一致：成交只广播，不写入任何存储。
    /// 成交字段在后续解析阶段转换，成交时间优先取条目内的毫秒时间。
    pub fn htx_trade(payload: impl Into<String>) -> Self {
        Self::new(MarketFeedProvider::Htx, MarketFeedChannel::Trade, payload)
    }

    /// 把 Coinbase Advanced Trade 的 ticker 文本打上标记，解析时需穿过 `events` 层找到 `tickers` 集合。
    /// 交易对以 `BASE-QUOTE` 产品 ID 形式给出，解析阶段会去掉横线并转大写还原成平台写法。
    /// 该接口不提供 24 小时开盘价，涨跌额固定记为 0，只有涨跌幅可从百分比字段取得。
    /// 本函数不解析、不持久化，也不广播。
    pub fn coinbase_ticker(payload: impl Into<String>) -> Self {
        Self::new(
            MarketFeedProvider::Coinbase,
            MarketFeedChannel::Ticker,
            payload,
        )
    }

    /// 把 Coinbase `level2` 文本打上 depth 标记，注意它推送的是增量更新而非完整订单簿。
    /// 买卖两侧混在同一个 updates 列表里，靠每条的 `side` 字段区分，无法归类的档位会被静默跳过。
    /// 若整帧取不到 provider 时间，解析阶段会退化成本机时间，这是全链路唯一允许的时间兜底。
    /// 盘口格式校验留给 provider 适配器，消费端不能把这份数据当作全量深度使用。
    pub fn coinbase_depth(payload: impl Into<String>) -> Self {
        Self::new(
            MarketFeedProvider::Coinbase,
            MarketFeedChannel::Depth,
            payload,
        )
    }

    /// 把 Coinbase `candles` 文本打上 kline 标记，开盘时间取记录里的 `start` 秒级时间戳。
    /// 该频道的推送粒度固定为五分钟，周期字段缺失或无法识别时解析阶段会直接按 `5m` 处理而不报错。
    /// 因此其他周期的 K 线在 Coinbase 上只能依赖 REST 兜底获取，无法通过 WebSocket 实时拿到。
    /// 周期与 OHLC 校验在后续解析阶段执行。
    pub fn coinbase_kline(payload: impl Into<String>) -> Self {
        Self::new(
            MarketFeedProvider::Coinbase,
            MarketFeedChannel::Kline,
            payload,
        )
    }

    /// 把 Coinbase `market_trades` 文本打上 trade 标记，解析时从 `events` 下的 `trades` 集合取首条。
    /// 成交时间优先按 RFC3339 解析，其次退回帧级时间戳，与另外两家的毫秒时间戳格式不同。
    /// 成交字段在后续解析阶段转换，产品 ID 缺失会导致整条成交被拒绝。
    pub fn coinbase_trade(payload: impl Into<String>) -> Self {
        Self::new(
            MarketFeedProvider::Coinbase,
            MarketFeedChannel::Trade,
            payload,
        )
    }

    /// 返回这一帧归属的行情源，它与频道共同决定统一解析入口选用哪个 provider 适配器。
    /// 标记一旦打错就会用错解析器，绝大多数情况下表现为字段缺失错误而不是静默产生错误行情。
    pub fn provider(&self) -> MarketFeedProvider {
        self.provider
    }

    /// 返回这一帧的频道类型，决定解析成 ticker、depth、kline 还是 trade 快照。
    /// `None` 频道没有对应解析器，会在统一解析入口直接返回不支持频道的校验错误。
    pub fn channel(&self) -> MarketFeedChannel {
        self.channel
    }

    /// 返回未经改动的原始 JSON 文本，解析器会在此基础上做一次完整反序列化。
    /// 构造时不校验 JSON 合法性，语法错误要到解析阶段才会以校验错误的形式暴露。
    pub fn payload(&self) -> &str {
        &self.payload
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketFeedConfig {
    provider: MarketFeedProvider,
    url: String,
    subscription_messages: Vec<String>,
    symbols: Vec<String>,
    intervals: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketFeedRestFallbackTickerRequest {
    symbol: String,
    url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketFeedRestFallbackKlineRequest {
    symbol: String,
    interval: String,
    url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketFeedRestFallbackConfig {
    provider: MarketFeedProvider,
    ticker_requests: Vec<MarketFeedRestFallbackTickerRequest>,
    kline_requests: Vec<MarketFeedRestFallbackKlineRequest>,
}

impl MarketFeedRestFallbackTickerRequest {
    /// 记录单个交易对的 ticker REST 兜底地址；请求对象本身不发送 HTTP，也不规范化 URL。
    /// 之所以把交易对和地址一起留存，是因为部分 provider 的 REST 响应不带交易对字段，
    /// 后续把响应包装成 WebSocket 同形载荷时必须从这里取回交易对，否则无法确定行情归属。
    pub fn new(symbol: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            url: url.into(),
        }
    }

    /// 返回发起该兜底请求时使用的交易对，取值是配置校验后的平台规范写法。
    /// 包装响应与记录失败明细都依赖它，因此它必须与 URL 中实际查询的交易对保持一致。
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// 返回该 ticker 兜底请求的完整 URL，已含 provider 基址与交易对查询参数。
    /// 构造时未做可达性或格式校验，错误地址要等真正发起请求时才会暴露。
    pub fn url(&self) -> &str {
        &self.url
    }
}

impl MarketFeedRestFallbackKlineRequest {
    /// 记录单个交易对、周期及其 K 线 REST 兜底地址；实际请求与解析由 worker 执行。
    /// 相比 ticker 多出周期，是因为 REST 返回的通常是裸行数组，包装回 provider 推送格式时
    /// 交易对和周期都得从请求上下文补回来，缺一就无法拼出解析器能识别的频道名。
    pub fn new(
        symbol: impl Into<String>,
        interval: impl Into<String>,
        url: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            interval: interval.into(),
            url: url.into(),
        }
    }

    /// 返回该 K 线兜底请求对应的交易对，包装响应时会用它拼出频道名或产品 ID。
    /// 同一交易对会随周期数量重复出现多次，每条请求各自独立发送。
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// 返回该请求查询的周期，取值是平台内部写法，拼 URL 时才转换成 provider 专有格式。
    /// 包装响应时同样以平台写法补回载荷，解析器的周期映射能同时接受两种形式。
    pub fn interval(&self) -> &str {
        &self.interval
    }

    /// 返回该 K 线兜底请求的完整 URL，已含 provider 基址、交易对与粒度参数。
    /// Coinbase 的地址还包含按当前时刻回推的时间窗口，因此它是一次性的，不能缓存复用。
    pub fn url(&self) -> &str {
        &self.url
    }
}

impl MarketFeedRestFallbackConfig {
    /// 聚合一个 provider 的 ticker 与 K 线兜底请求清单，保持生成顺序供 worker 逐项处理。
    /// 构造时不去重、不请求网络；空清单表示该频道没有可执行的兜底请求。
    pub fn new(
        provider: MarketFeedProvider,
        ticker_requests: Vec<MarketFeedRestFallbackTickerRequest>,
        kline_requests: Vec<MarketFeedRestFallbackKlineRequest>,
    ) -> Self {
        Self {
            provider,
            ticker_requests,
            kline_requests,
        }
    }

    /// 返回本份兜底配置归属的行情源，兜底执行时会把它写进每条失败明细，用于按 provider 归因。
    /// 每个 provider 拥有独立配置，因此一家不可用不会影响另一家的兜底进度。
    pub fn provider(&self) -> MarketFeedProvider {
        self.provider
    }

    /// 返回该 provider 的 ticker 兜底请求清单，顺序与配置展开结果一致，也就是交易对的配置顺序。
    /// 兜底执行会先跑完全部 ticker 再跑 K 线，因此这个顺序直接决定价格恢复的先后。
    pub fn ticker_requests(&self) -> &[MarketFeedRestFallbackTickerRequest] {
        &self.ticker_requests
    }

    /// 返回首个 ticker 兜底地址以兼容旧调用路径；清单为空时返回空字符串。
    /// 多交易对场景下这个值只是任意一条，不能代表整份配置，新代码应改用完整请求清单。
    /// 返回空字符串而非 `Option`，是为了保持旧签名不变，调用方需自行区分空配置与真实空地址。
    pub fn ticker_url(&self) -> &str {
        self.ticker_requests
            .first()
            .map(MarketFeedRestFallbackTickerRequest::url)
            .unwrap_or_default()
    }

    /// 克隆并返回全部 ticker 兜底地址，不发送请求，也不保证 URL 可达。
    /// 丢弃了交易对上下文，因此只适合日志打印与配置核对，不能拿来驱动实际兜底执行。
    pub fn ticker_urls(&self) -> Vec<String> {
        self.ticker_requests
            .iter()
            .map(|request| request.url.clone())
            .collect()
    }

    /// 返回按交易对与周期笛卡尔积展开的 K 线兜底请求清单，条目数等于两个集合长度之积。
    /// 排列上同一交易对的各周期连续出现，与生成时外层遍历交易对、内层遍历周期的顺序一致。
    pub fn kline_requests(&self) -> &[MarketFeedRestFallbackKlineRequest] {
        &self.kline_requests
    }

    /// 克隆并返回全部 K 线兜底地址，不发送请求，也不保证 URL 可达。
    /// 与 ticker 版本一样只保留地址，交易对与周期信息会丢失，因此同样只适合诊断用途。
    pub fn kline_urls(&self) -> Vec<String> {
        self.kline_requests
            .iter()
            .map(|request| request.url.clone())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MarketFeedRestFallbackFrameRequest {
    channel: MarketFeedChannel,
    symbol: String,
    interval: Option<String>,
    url: String,
}

impl MarketFeedRestFallbackFrameRequest {
    /// 把 ticker 兜底请求收敛成带频道标记的统一形态，使两类请求能排进同一个执行队列。
    /// 周期固定为 `None`，ticker 本就没有周期概念，后续按频道分派时也不会去取它。
    fn ticker(request: &MarketFeedRestFallbackTickerRequest) -> Self {
        Self {
            channel: MarketFeedChannel::Ticker,
            symbol: request.symbol().to_owned(),
            interval: None,
            url: request.url().to_owned(),
        }
    }

    /// 把 K 线兜底请求收敛成同一形态，与 ticker 的差别只在于频道标记与必定存在的周期。
    /// 包装响应时会强制要求这个周期存在，缺失将被判为校验错误而不是退化成某个缺省周期。
    fn kline(request: &MarketFeedRestFallbackKlineRequest) -> Self {
        Self {
            channel: MarketFeedChannel::Kline,
            symbol: request.symbol().to_owned(),
            interval: Some(request.interval().to_owned()),
            url: request.url().to_owned(),
        }
    }
}

struct MarketFeedRestFallbackFrameResult {
    request: MarketFeedRestFallbackFrameRequest,
    result: Result<MarketFeedFrame, AppError>,
}

impl MarketFeedRestFallbackFrameResult {
    /// 把一次兜底请求的结果与它的请求上下文绑定，让失败时仍能追溯到具体的交易对、周期与地址。
    /// 请求上下文按值克隆，因为一个 REST 响应可能展开成多帧，每帧都要独立携带同一份来源信息。
    /// 这里只做打包，既不判断成功失败，也不累加任何计数。
    fn new(
        request: &MarketFeedRestFallbackFrameRequest,
        result: Result<MarketFeedFrame, AppError>,
    ) -> Self {
        Self {
            request: request.clone(),
            result,
        }
    }
}

impl MarketFeedConfig {
    /// 组装单个 provider 的 WebSocket 地址、订阅消息和对应交易对/周期元数据。
    /// 订阅消息在这一步就全部生成完毕，重连时可直接复用同一份文本，不必重新推导，
    /// 因此断线恢复只需重放这些消息即可回到断连前的订阅状态。
    /// 交易对与周期同时留存，是为了让上层在重连、补数或诊断时知道这条连接原本覆盖哪些行情。
    /// 输入原样保存；连接建立、订阅发送和断线重连由运行时 worker 负责。
    pub fn new(
        provider: MarketFeedProvider,
        url: impl Into<String>,
        subscription_messages: Vec<String>,
        symbols: Vec<String>,
        intervals: Vec<String>,
    ) -> Self {
        Self {
            provider,
            url: url.into(),
            subscription_messages,
            symbols,
            intervals,
        }
    }

    /// 返回这条连接归属的行情源，worker 用它决定收到的帧该打上哪个 provider 标记。
    /// 每个 provider 独占一条 WebSocket 连接，因此该值在连接生命周期内固定不变。
    pub fn provider(&self) -> MarketFeedProvider {
        self.provider
    }

    /// 返回该 provider 的 WebSocket 地址，取自运行配置且未做协议或可达性校验。
    /// 地址无效只会在 worker 建连时暴露，配置组装阶段不会提前失败。
    pub fn url(&self) -> &str {
        &self.url
    }

    /// 返回按 provider 协议预生成的 WebSocket 订阅消息，调用方需在连接建立后按序发送。
    /// Bitget 与 HTX 是每个交易对若干条，Coinbase 则是一条报文携带全部产品 ID，数量差异很大。
    /// 重连后必须完整重发，遗漏任何一条都会造成对应频道静默无数据而不是报错。
    pub fn subscription_messages(&self) -> &[String] {
        &self.subscription_messages
    }

    /// 返回这条连接覆盖的交易对，均已通过规范化校验，是订阅消息与 REST 兜底的共同输入。
    /// 顺序与配置一致且不去重，重复项会产生重复订阅。
    pub fn symbols(&self) -> &[String] {
        &self.symbols
    }

    /// 返回这条连接订阅的 K 线周期，取值已限定在平台白名单内，写法为平台内部格式。
    /// 周期为空时不会订阅任何 candle 频道，ticker、盘口与成交订阅不受影响。
    pub fn intervals(&self) -> &[String] {
        &self.intervals
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketFeedFailureContext {
    provider: MarketFeedProvider,
    channel: MarketFeedChannel,
    symbol: String,
    interval: Option<String>,
    url: String,
    error: String,
}

impl MarketFeedFailureContext {
    /// 把一次兜底失败的全部定位信息固化成诊断记录：provider、频道、交易对、周期、URL 与错误文本。
    /// 错误在此处被转成字符串，原始类型不再保留，因此这份记录只能用于日志与告警，无法据此分支重试。
    /// 仅在 REST 兜底路径构造；WebSocket 流入口不记录明细，只累加失败计数。
    fn new(
        provider: MarketFeedProvider,
        request: &MarketFeedRestFallbackFrameRequest,
        error: &AppError,
    ) -> Self {
        Self {
            provider,
            channel: request.channel,
            symbol: request.symbol.clone(),
            interval: request.interval.clone(),
            url: request.url.clone(),
            error: error.to_string(),
        }
    }

    /// 返回失败请求所属的行情源，用于把故障归因到具体外部依赖而不是笼统的行情不可用。
    /// 同一轮兜底里多个 provider 的失败会各自成条，便于判断是单家故障还是全网异常。
    pub fn provider(&self) -> MarketFeedProvider {
        self.provider
    }

    /// 返回失败请求的频道，用于区分是价格兜底失败还是 K 线兜底失败。
    /// 兜底路径只会产生 ticker 与 kline 两种取值，其余频道不参与 REST 兜底。
    pub fn channel(&self) -> MarketFeedChannel {
        self.channel
    }

    /// 返回失败请求针对的交易对，可据此判断是个别交易对下架还是整个 provider 不可用。
    /// 取值来自请求上下文而非响应内容，因此即便响应完全拿不到也一定有值。
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// 返回失败请求的 K 线周期，ticker 兜底失败时为 `None`，可据此反推失败发生在哪条链路。
    /// 有值时是平台内部写法，与 URL 里经过映射的 provider 专有写法可能并不相同。
    pub fn interval(&self) -> Option<&str> {
        self.interval.as_deref()
    }

    /// 返回失败请求的完整 URL，包含基址与全部查询参数，可直接复制出来手工复现。
    /// Coinbase 的 K 线地址带有生成时刻的时间窗口，复现时拿到的数据范围会与当时不同。
    pub fn url(&self) -> &str {
        &self.url
    }

    /// 返回失败原因的文本形式，可能来自 HTTP 请求阶段、非 2xx 状态、正文读取，或后续的载荷包装校验。
    /// 这四类在错误文本里措辞不同，可据此判断是网络问题、接口拒绝还是响应结构变化。
    pub fn error(&self) -> &str {
        &self.error
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MarketFeedSummary {
    pub received: u32,
    pub ingested: u32,
    pub failed: u32,
    failure_contexts: Vec<MarketFeedFailureContext>,
}

impl MarketFeedSummary {
    /// 用调用方提供的计数创建行情处理汇总；失败上下文初始为空，后续按请求失败顺序追加。
    /// 三个计数满足「已接收等于已摄取加失败」，因为每帧只会走成功或失败其中一条分支。
    /// 主要供测试构造预期值，实际运行中的汇总由两个入口从零累加得到。
    pub fn new(received: u32, ingested: u32, failed: u32) -> Self {
        Self {
            received,
            ingested,
            failed,
            failure_contexts: Vec::new(),
        }
    }

    /// 记一次不留明细的失败，用于流式入口以及解析、摄取阶段的错误。
    /// 这些阶段的错误不携带请求上下文，只累加计数，因此排查时只能看出失败数量而无法定位到具体帧。
    fn record_failure(&mut self) {
        self.failed += 1;
    }

    /// 记一次带完整定位信息的失败，在累加计数的同时把诊断记录按发生顺序追加进明细列表。
    /// 只有 REST 兜底的请求级错误走这条路径，因此明细数量通常小于失败总数。
    fn record_failure_context(&mut self, context: MarketFeedFailureContext) {
        self.record_failure();
        self.failure_contexts.push(context);
    }

    /// 返回 REST 兜底失败明细，包含 provider、频道、symbol、URL 与错误文本，供日志和监控诊断。
    /// 明细按失败发生顺序排列，可能少于失败总数，因为解析与摄取阶段的失败不记录上下文。
    /// 列表为空不等于本轮全部成功，仍需结合失败计数一并判断。
    pub fn failure_contexts(&self) -> &[MarketFeedFailureContext] {
        &self.failure_contexts
    }
}

#[derive(Clone)]
pub struct MarketFeedWorker<S> {
    sink: S,
    broadcast_hub: Option<EventBroadcastHub>,
}

impl<S> MarketFeedWorker<S> {
    /// 以指定 ingestion sink 构造行情 worker，默认不向 WebSocket 广播公开行情。
    /// sink 以泛型注入，测试可用内存实现替换真实存储，从而在无 Redis/Mongo 的环境下覆盖编排逻辑。
    /// sink 的 Redis/Mongo 连接可用性在实际摄取帧时检查，构造阶段无外部 I/O。
    pub fn new(sink: S) -> Self {
        Self {
            sink,
            broadcast_hub: None,
        }
    }

    /// 注入公开 WebSocket 广播中心；只有 sink 成功持久化的帧才会经该 hub 广播。
    /// 不注入时行情照常落地但订阅端收不到推送，因此纯采集进程可以省略，对外服务进程必须注入。
    pub fn with_broadcast_hub(mut self, hub: EventBroadcastHub) -> Self {
        self.broadcast_hub = Some(hub);
        self
    }

    /// 保留旧版 outbox builder 调用的兼容入口；当前实时行情不写业务 outbox，因此参数不会被保存。
    /// 调用此方法不会新增持久化或投递保证，可靠行情来源仍是 Redis/Mongo 与重连恢复链。
    pub fn with_outbox_writer<N>(self, _outbox_writer: EventOutboxWriter<N>) -> Self {
        self
    }

    /// 校验 symbol/interval 后，为默认 Bitget、HTX 生成 WebSocket 地址与订阅消息。
    /// 配置为空或字段非法时在连接前返回校验错误，不产生网络或缓存副作用。
    pub fn provider_configs(
        settings: &Settings,
        symbols: &[&str],
        intervals: &[&str],
    ) -> AppResult<Vec<MarketFeedConfig>> {
        Self::provider_configs_for(
            settings,
            &MarketFeedProvider::default_providers(),
            symbols,
            intervals,
        )
    }

    /// 为显式 provider 集合生成 WebSocket 配置；provider 为空直接拒绝，symbol/interval 统一校验一次。
    /// 交易对和周期只校验并规范化一次，随后各 provider 复用同一份结果，确保多家订阅覆盖完全相同的行情范围。
    /// 交易对为空视为配置错误，周期为空则被允许，那种配置只订阅 ticker、盘口与成交而不要 K 线。
    /// 任一 provider 展开失败会让整次调用返回错误，不会退化成部分可用的配置集合。
    /// 保持 provider 输入顺序并逐个展开，函数本身不建立连接或发送订阅消息。
    pub fn provider_configs_for(
        settings: &Settings,
        providers: &[MarketFeedProvider],
        symbols: &[&str],
        intervals: &[&str],
    ) -> AppResult<Vec<MarketFeedConfig>> {
        if providers.is_empty() {
            return Err(AppError::Validation(
                "market feed providers are required".to_owned(),
            ));
        }
        let symbols = validate_feed_symbols(symbols)?;
        let intervals = validate_feed_intervals(intervals)?;
        providers
            .iter()
            .map(|provider| provider.feed_config(settings, &symbols, &intervals))
            .collect()
    }

    /// 校验 symbol/interval 后，为默认 Bitget、HTX 生成 ticker 与 K 线 REST 兜底清单。
    /// 与 WebSocket 配置共用同一套默认 provider 顺序，因此兜底覆盖的行情范围与实时订阅保持一致。
    /// 这里只组装 URL；请求失败隔离、解析和持久化由 `run_rest_fallback_config` 负责。
    pub fn provider_rest_fallback_configs(
        settings: &Settings,
        symbols: &[&str],
        intervals: &[&str],
    ) -> AppResult<Vec<MarketFeedRestFallbackConfig>> {
        Self::provider_rest_fallback_configs_for(
            settings,
            &MarketFeedProvider::default_providers(),
            symbols,
            intervals,
        )
    }

    /// 为显式 provider 集合生成 REST 兜底清单；空 provider、非法 symbol 或周期在发请求前失败。
    /// 把校验前置到组装阶段，可确保非法配置不会产生任何网络请求，也不会留下半截兜底记录。
    /// 单个 provider 的请求数是交易对数加上交易对与周期之积，配置规模较大时需留意兜底一轮的总耗时。
    /// 每个 provider 保持独立配置，后续 worker 可按提供方隔离错误和统计有效写入。
    pub fn provider_rest_fallback_configs_for(
        settings: &Settings,
        providers: &[MarketFeedProvider],
        symbols: &[&str],
        intervals: &[&str],
    ) -> AppResult<Vec<MarketFeedRestFallbackConfig>> {
        if providers.is_empty() {
            return Err(AppError::Validation(
                "market feed providers are required".to_owned(),
            ));
        }
        let symbols = validate_feed_symbols(symbols)?;
        let intervals = validate_feed_intervals(intervals)?;
        providers
            .iter()
            .map(|provider| provider.rest_fallback_config(settings, &symbols, &intervals))
            .collect()
    }
}

impl MarketFeedWorker<MarketIngestionService> {
    /// 从应用状态构造生产 ingestion worker；Redis 与 Mongo 缺一不可，MySQL/事件总线作为撮合与广播副作用按配置接入。
    /// 缺少 Redis 或 Mongo 时立刻返回配置错误，避免 worker 带着不完整依赖启动后才在运行期逐帧报错。
    /// 事件总线存在才挂上广播中心，因此纯采集部署可以只落地不推送，两种形态共用同一段构造逻辑。
    /// 构造阶段不探测任何连接可用性，真正的连通性问题要到第一次摄取时才会暴露。
    pub fn from_state(state: &AppState) -> AppResult<Self> {
        let worker = Self::new(MarketIngestionService::from_state(state)?);
        Ok(match state.event_broadcast_hub.clone() {
            Some(hub) => worker.with_broadcast_hub(hub),
            None => worker,
        })
    }
}

impl<S> MarketFeedWorker<S>
where
    S: MarketIngestionSink,
{
    /// 消费有限行情帧流并逐帧解析、持久化和广播；输入流或摄取失败只增加 `failed`，不会终止后续帧。
    /// 流必须是有限的，函数会一直拉取到流结束才返回，对持续不断的 WebSocket 连接应按批切分后调用。
    /// 失败分两类且都被吞掉：流本身产出的错误项，以及某一帧解析或摄取抛出的错误，两者都只累加计数。
    /// 因此本函数的 `Ok` 返回不代表全部成功，唯一可靠的判断依据是 `ingested` 是否大于零。
    /// 本入口不保留逐帧错误文本，调用方只能依据计数判断本轮是否有有效写入。
    pub async fn run_stream<E, St>(&self, frames: St) -> AppResult<MarketFeedSummary>
    where
        E: ToString,
        St: Stream<Item = Result<MarketFeedFrame, E>> + Send,
    {
        futures_util::pin_mut!(frames);
        let mut summary = MarketFeedSummary::default();

        while let Some(frame) = frames.next().await {
            summary.received += 1;
            match frame {
                Ok(frame) => match self.ingest_frame(&frame).await {
                    Ok(()) => summary.ingested += 1,
                    Err(_) => summary.record_failure(),
                },
                Err(_) => summary.record_failure(),
            }
        }

        Ok(summary)
    }

    /// 依次 GET 一个 provider 的 ticker 与 K 线 URL，并把成功正文包装、解析和持久化；单项失败不阻断其他请求。
    /// HTTP/状态码/正文错误会保存 provider、频道、symbol、URL 与错误文本；解析或 sink 错误只增加失败计数。
    /// 返回汇总不代表价格源可用，调用方必须再执行“至少一个有效写入”校验。
    pub async fn run_rest_fallback_config<C>(
        &self,
        config: &MarketFeedRestFallbackConfig,
        http_client: &C,
    ) -> AppResult<MarketFeedSummary>
    where
        C: MarketFeedRestFallbackHttpClient,
    {
        let frames = fetch_rest_fallback_frames(config, http_client).await?;
        let mut summary = MarketFeedSummary::default();
        for frame in frames {
            summary.received += 1;
            match frame.result {
                Ok(frame) => match self.ingest_frame(&frame).await {
                    Ok(()) => summary.ingested += 1,
                    Err(_) => summary.record_failure(),
                },
                Err(error) => summary.record_failure_context(MarketFeedFailureContext::new(
                    config.provider(),
                    &frame.request,
                    &error,
                )),
            }
        }
        Ok(summary)
    }

    /// 将供应商原始帧解析为领域快照，交给对应 sink 持久化，成功后才发布实时市场事件。
    /// trade 帧目前只发布而不写行情存储；解析、事件转换或 sink 失败时不广播。
    /// WS 消息转换发生在持久化之后；转换失败会返回错误，但不会回滚已经完成的 Redis/Mongo 写入。
    pub async fn ingest_frame(&self, frame: &MarketFeedFrame) -> AppResult<()> {
        let parsed = parse_feed_frame(frame)?;
        let event = MarketFeedEvent::from_parsed(&parsed)?;
        match &parsed {
            ParsedMarketFeed::Ticker(snapshot) => self.sink.ingest_ticker(snapshot).await?,
            ParsedMarketFeed::Depth(snapshot) => self.sink.ingest_depth(snapshot).await?,
            ParsedMarketFeed::Kline(snapshot) => self.sink.ingest_kline(snapshot).await?,
            ParsedMarketFeed::Trade(_) => {}
        }
        if let Some(hub) = &self.broadcast_hub {
            hub.publish(EventBroadcastMessage::from_market_feed_event(&event)?);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketFeedEvent {
    aggregate_type: String,
    aggregate_id: String,
    event_type: String,
    routing_key: String,
    idempotency_key: String,
    public_ws_namespace: String,
    public_ws_topic: String,
    payload: Value,
}

impl MarketFeedEvent {
    /// 从已通过领域构造器校验的 ticker 快照创建统一行情事件，供第三方 feed 与内部策略行情共用同一 WebSocket 合同。
    /// 本函数只构造事件元数据和 JSON，不写 Redis/Mongo、不触发现货订单，也不直接广播。
    pub fn from_ticker_snapshot(snapshot: &MarketTickerSnapshot) -> AppResult<Self> {
        Self::from_parsed(&ParsedMarketFeed::Ticker(snapshot.clone()))
    }

    /// 从已通过领域构造器校验的 K 线快照创建统一行情事件，幂等键继续包含槽位和 OHLCV 载荷摘要。
    /// 本函数无存储或网络副作用；调用方必须在 ingestion 成功后再发布，避免客户端看到未落地数据。
    pub fn from_kline_snapshot(snapshot: &MarketKlineSnapshot) -> AppResult<Self> {
        Self::from_parsed(&ParsedMarketFeed::Kline(snapshot.clone()))
    }

    /// 解析 provider 原始帧并生成统一事件元数据、幂等键、公开 WebSocket 主题与 JSON 载荷。
    /// 字段或频道不合法时返回校验错误；该函数不持久化快照，也不实际发布事件。
    pub fn from_frame(frame: &MarketFeedFrame) -> AppResult<Self> {
        let parsed = parse_feed_frame(frame)?;
        Self::from_parsed(&parsed)
    }

    /// 按频道把已解析的领域快照铺开成统一事件：聚合信息、路由键、幂等键、公开 WS 主题与 JSON 载荷。
    /// 四类事件共享一套结构，但各自的标识规则不同，其中幂等键的构成差异最需要注意。
    /// ticker 与 depth 用「provider + 交易对 + 频道 + 观察毫秒」，同一毫秒内的重复推送会得到相同键。
    /// K 线除槽位外还追加 OHLCV 与观察时间的载荷摘要，因为同一根形成中的蜡烛会反复更新，
    /// 只有把数值纳入摘要才能让每一版更新都拥有独立标识而不被误判成重复。
    /// trade 直接用 provider 成交号，天然唯一，无需引入时间或摘要。
    /// 载荷里所有价格与数量都转成十进制字符串，时间统一为毫秒整数，避免 JSON 浮点损失精度。
    /// 公开 WS 主题保持既有订阅合同：ticker、depth、trade 用交易对，K 线用 `交易对_周期`。
    /// 本函数是纯映射，不做任何 I/O，也不校验快照内容，输入必须已通过领域构造器校验。
    fn from_parsed(parsed: &ParsedMarketFeed) -> AppResult<Self> {
        match parsed {
            ParsedMarketFeed::Ticker(snapshot) => Ok(Self {
                aggregate_type: "market_ticker".to_owned(),
                aggregate_id: snapshot.symbol().to_owned(),
                event_type: "ticker_updated".to_owned(),
                routing_key: format!("market.{}.ticker", snapshot.symbol()),
                idempotency_key: format!(
                    "market_feed:{}:{}:ticker:{}",
                    provider_name(snapshot.provider()),
                    snapshot.symbol(),
                    snapshot.observed_at().timestamp_millis()
                ),
                public_ws_namespace: "ticker".to_owned(),
                public_ws_topic: snapshot.symbol().to_owned(),
                payload: json!({
                    "symbol": snapshot.symbol(),
                    "last_price": snapshot.last_price().to_string(),
                    "high_24h": snapshot.high_24h().to_string(),
                    "low_24h": snapshot.low_24h().to_string(),
                    "volume_24h": snapshot.volume_24h().to_string(),
                    "price_change_24h": snapshot.price_change_24h().to_string(),
                    "price_change_percent_24h": snapshot.price_change_percent_24h().to_string(),
                    "observed_at": snapshot.observed_at().timestamp_millis(),
                    "provider": provider_name(snapshot.provider()),
                }),
            }),
            ParsedMarketFeed::Depth(snapshot) => Ok(Self {
                aggregate_type: "market_depth".to_owned(),
                aggregate_id: snapshot.symbol().to_owned(),
                event_type: "depth_updated".to_owned(),
                routing_key: format!("market.{}.depth", snapshot.symbol()),
                idempotency_key: format!(
                    "market_feed:{}:{}:depth:{}",
                    provider_name(snapshot.provider()),
                    snapshot.symbol(),
                    snapshot.observed_at().timestamp_millis()
                ),
                public_ws_namespace: "depth".to_owned(),
                public_ws_topic: snapshot.symbol().to_owned(),
                payload: json!({
                    "symbol": snapshot.symbol(),
                    "bids": snapshot.bids(),
                    "asks": snapshot.asks(),
                    "observed_at": snapshot.observed_at().timestamp_millis(),
                    "provider": provider_name(snapshot.provider()),
                }),
            }),
            ParsedMarketFeed::Kline(snapshot) => Ok(Self {
                aggregate_type: "market_kline".to_owned(),
                aggregate_id: format!("{}:{}", snapshot.symbol(), snapshot.interval()),
                event_type: "kline_updated".to_owned(),
                routing_key: format!("market.{}.kline.{}", snapshot.symbol(), snapshot.interval()),
                idempotency_key: format!(
                    "market_feed:{}:{}:kline:{}:{}:{}",
                    provider_name(snapshot.provider()),
                    snapshot.symbol(),
                    snapshot.interval(),
                    snapshot.open_time().timestamp_millis(),
                    market_feed_payload_hash(&json!({
                        "open": snapshot.open().to_string(),
                        "high": snapshot.high().to_string(),
                        "low": snapshot.low().to_string(),
                        "close": snapshot.close().to_string(),
                        "volume": snapshot.volume().to_string(),
                        "observed_at": snapshot.observed_at().timestamp_millis(),
                    }))
                ),
                public_ws_namespace: "kline".to_owned(),
                public_ws_topic: format!("{}_{}", snapshot.symbol(), snapshot.interval()),
                payload: json!({
                    "symbol": snapshot.symbol(),
                    "interval": snapshot.interval(),
                    "open_time": snapshot.open_time().timestamp_millis(),
                    "open": snapshot.open().to_string(),
                    "high": snapshot.high().to_string(),
                    "low": snapshot.low().to_string(),
                    "close": snapshot.close().to_string(),
                    "volume": snapshot.volume().to_string(),
                    "observed_at": snapshot.observed_at().timestamp_millis(),
                    "provider": provider_name(snapshot.provider()),
                }),
            }),
            ParsedMarketFeed::Trade(tick) => Ok(Self {
                aggregate_type: "market_trade".to_owned(),
                aggregate_id: tick.trade_id().to_owned(),
                event_type: "trade_created".to_owned(),
                routing_key: format!("market.{}.trade", tick.symbol()),
                idempotency_key: format!(
                    "market_feed:{}:{}:trade:{}",
                    provider_name(tick.provider()),
                    tick.symbol(),
                    tick.trade_id()
                ),
                public_ws_namespace: "trade".to_owned(),
                public_ws_topic: tick.symbol().to_owned(),
                payload: json!({
                    "symbol": tick.symbol(),
                    "trade_id": tick.trade_id(),
                    "side": tick.side(),
                    "price": tick.price().to_string(),
                    "quantity": tick.quantity().to_string(),
                    "traded_at": tick.traded_at().timestamp_millis(),
                    "provider": provider_name(tick.provider()),
                }),
            }),
        }
    }

    /// 返回 outbox/指标使用的行情聚合类型，取值为 `market_ticker`、`market_depth`、`market_kline` 或 `market_trade`。
    /// 它带 `market_` 前缀以便与其他限界上下文的聚合区分，是按类型做指标分桶的主要维度。
    pub fn aggregate_type(&self) -> &str {
        &self.aggregate_type
    }

    /// 返回规范化交易对作为事件聚合标识，使同一市场事件归入稳定聚合根。
    /// 三类行情直接用交易对，但 K 线用 `交易对:周期`，而 trade 用的是 provider 成交号而非交易对。
    pub fn aggregate_id(&self) -> &str {
        &self.aggregate_id
    }

    /// 返回行情事件类型，取值为 `ticker_updated`、`depth_updated`、`kline_updated` 或 `trade_created`。
    /// 前三者是覆盖式更新，只有成交用 created，因为逐笔成交是只增不改的独立事实。
    pub fn event_type(&self) -> &str {
        &self.event_type
    }

    /// 返回包含交易对与频道的事件路由键，供内部事件消费者精确订阅。
    /// 格式为 `market.<交易对>.<频道>`，K 线在末尾额外追加周期，因此可以按周期分别订阅。
    pub fn routing_key(&self) -> &str {
        &self.routing_key
    }

    /// 返回该事件的幂等键，统一以 `market_feed:` 开头，随后是 provider、交易对与频道。
    /// 结尾部分按频道而异：ticker 与 depth 用观察毫秒，成交用 provider 成交号，
    /// K 线则用开盘毫秒再加一段 OHLCV 载荷摘要，以便同一根蜡烛的每一版更新都能被区分开。
    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }

    /// 返回公开 WebSocket 命名空间；客户端需与 topic 一起使用，不能据此推导内部路由键。
    /// 取值是不带前缀的 `ticker`、`depth`、`kline`、`trade`，与内部聚合类型有意保持不同的命名。
    pub fn public_ws_namespace(&self) -> &str {
        &self.public_ws_namespace
    }

    /// 返回公开 WebSocket 主题，保持移动端与 PC 端现有订阅合同不变。
    /// 除 K 线用 `交易对_周期` 外都直接是交易对，这个下划线格式属于对外合同，不能随意改动。
    pub fn public_ws_topic(&self) -> &str {
        &self.public_ws_topic
    }

    /// 返回事件的 JSON 载荷，价格与数量均为十进制字符串，时间为毫秒整数，并附带 provider 代码。
    /// 直接以该结构推送给客户端，字段增删属于对外合同变更，需要与前端同步。
    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

enum ParsedMarketFeed {
    Ticker(MarketTickerSnapshot),
    Depth(MarketDepthSnapshot),
    Kline(MarketKlineSnapshot),
    Trade(MarketTradeTick),
}

/// 顺序执行一个 provider 的全部 REST 兜底请求，并把每次结果连同请求上下文一起收集起来。
/// 请求排成一个队列，先全部 ticker 后全部 K 线，逐条串行发出，不做并发，以免同时压垮外部接口。
/// 每条请求有两个失败点：HTTP 阶段失败，以及响应包装成 provider 推送格式时的校验失败，
/// 两者都会被就地捕获成一条失败结果继续处理下一条，因此单条失败绝不会中断整轮兜底。
/// 一次 K 线响应可能展开成多帧，此时每帧都会复制同一份请求上下文，失败明细里会看到重复的 URL。
/// 本函数只负责取回并包装，不解析领域快照，也不写入任何存储。
/// 返回类型虽为 `AppResult`，但实际不会返回错误，全部失败都装在结果列表里。
async fn fetch_rest_fallback_frames<C>(
    config: &MarketFeedRestFallbackConfig,
    http_client: &C,
) -> AppResult<Vec<MarketFeedRestFallbackFrameResult>>
where
    C: MarketFeedRestFallbackHttpClient,
{
    let mut requests =
        VecDeque::with_capacity(config.ticker_requests().len() + config.kline_requests().len());
    requests.extend(
        config
            .ticker_requests()
            .iter()
            .map(MarketFeedRestFallbackFrameRequest::ticker),
    );
    requests.extend(
        config
            .kline_requests()
            .iter()
            .map(MarketFeedRestFallbackFrameRequest::kline),
    );

    let mut frames = Vec::with_capacity(requests.len());
    while let Some(request) = requests.pop_front() {
        match http_client.get_text(&request.url).await {
            Ok(payload) => match rest_fallback_frames(config.provider(), &request, &payload) {
                Ok(payload_frames) => frames.extend(
                    payload_frames
                        .into_iter()
                        .map(|frame| MarketFeedRestFallbackFrameResult::new(&request, Ok(frame))),
                ),
                Err(error) => {
                    frames.push(MarketFeedRestFallbackFrameResult::new(&request, Err(error)))
                }
            },
            Err(error) => frames.push(MarketFeedRestFallbackFrameResult::new(&request, Err(error))),
        }
    }
    Ok(frames)
}

/// 按 provider 与频道把一段 REST 响应改写成若干条与 WebSocket 同形的帧。
/// 这是 REST 与 WebSocket 复用同一套解析器的关键：响应先伪装成推送格式，再走统一解析入口。
/// ticker 恒定产出一帧，K 线则按响应里的行数展开成多帧，每根蜡烛独立走完解析与落地。
/// K 线必须能从请求上下文取到周期，缺失时返回校验错误而不是猜测一个缺省周期。
/// depth、trade 与 `None` 频道不支持 REST 兜底，命中即返回不支持频道的校验错误。
/// 本函数只做结构改写，字段级校验一律留到后续的 provider 解析阶段。
fn rest_fallback_frames(
    provider: MarketFeedProvider,
    request: &MarketFeedRestFallbackFrameRequest,
    payload: &str,
) -> AppResult<Vec<MarketFeedFrame>> {
    let channel = request.channel;
    let payloads = match (provider, channel) {
        (MarketFeedProvider::Bitget, MarketFeedChannel::Ticker) => {
            vec![bitget_rest_ticker_payload(payload, &request.symbol)?]
        }
        (MarketFeedProvider::Bitget, MarketFeedChannel::Kline) => bitget_rest_kline_payloads(
            payload,
            &request.symbol,
            required_rest_fallback_interval(request)?,
        )?,
        (MarketFeedProvider::Htx, MarketFeedChannel::Ticker) => {
            vec![htx_rest_ticker_payload(payload, &request.symbol)?]
        }
        (MarketFeedProvider::Htx, MarketFeedChannel::Kline) => htx_rest_kline_payloads(
            payload,
            &request.symbol,
            required_rest_fallback_interval(request)?,
        )?,
        (MarketFeedProvider::Coinbase, MarketFeedChannel::Ticker) => {
            vec![coinbase_rest_ticker_payload(payload, &request.symbol)?]
        }
        (MarketFeedProvider::Coinbase, MarketFeedChannel::Kline) => coinbase_rest_kline_payloads(
            payload,
            &request.symbol,
            required_rest_fallback_interval(request)?,
        )?,
        (_, MarketFeedChannel::Depth | MarketFeedChannel::Trade | MarketFeedChannel::None) => {
            return Err(AppError::Validation(
                "unsupported market feed REST fallback channel".to_owned(),
            ));
        }
    };
    Ok(payloads
        .into_iter()
        .map(|payload| MarketFeedFrame::new(provider, channel, payload))
        .collect())
}

/// 断言 K 线兜底请求必须携带周期，缺失时返回校验错误而不是退化成某个缺省周期。
/// 周期由请求上下文提供而非响应内容，因为多数 provider 的 REST K 线响应本身并不回显周期。
/// 一旦这里猜错，蜡烛就会被写进错误的时间槽和缓存键，所以宁可整帧失败也不做兜底推断。
fn required_rest_fallback_interval(
    request: &MarketFeedRestFallbackFrameRequest,
) -> AppResult<&str> {
    request.interval.as_deref().ok_or_else(|| {
        AppError::Validation("market feed REST fallback interval is required".to_owned())
    })
}

/// 按 provider 与频道的组合分派到对应适配器，把原始文本解析成领域快照。
/// 这是全模块唯一的解析入口，WebSocket 实时帧与 REST 兜底改写后的帧都在此汇合，
/// 因此两条链路的字段兼容性、时间语义与错误行为天然保持一致。
/// 三家 provider 各支持 ticker、depth、kline、trade 四个频道，共十二种组合逐一显式列出。
/// 只有 `None` 频道没有解析器，命中即返回不支持频道的校验错误。
/// 匹配全部穷举而不设通配兜底，新增 provider 时编译器会强制补齐所有频道分支。
/// 本函数不写存储也不广播，任何字段缺失或格式错误都以校验错误形式上抛，不产生降级行情。
fn parse_feed_frame(frame: &MarketFeedFrame) -> AppResult<ParsedMarketFeed> {
    match (frame.provider(), frame.channel()) {
        (MarketFeedProvider::Bitget, MarketFeedChannel::Ticker) => {
            BitgetMarketAdapter::ticker_from_ws(frame.payload()).map(ParsedMarketFeed::Ticker)
        }
        (MarketFeedProvider::Bitget, MarketFeedChannel::Depth) => {
            BitgetMarketAdapter::depth_from_ws(frame.payload()).map(ParsedMarketFeed::Depth)
        }
        (MarketFeedProvider::Bitget, MarketFeedChannel::Kline) => {
            BitgetMarketAdapter::kline_from_ws(frame.payload()).map(ParsedMarketFeed::Kline)
        }
        (MarketFeedProvider::Bitget, MarketFeedChannel::Trade) => {
            BitgetMarketAdapter::trade_from_ws(frame.payload()).map(ParsedMarketFeed::Trade)
        }
        (MarketFeedProvider::Htx, MarketFeedChannel::Ticker) => {
            HtxMarketAdapter::ticker_from_ws(frame.payload()).map(ParsedMarketFeed::Ticker)
        }
        (MarketFeedProvider::Htx, MarketFeedChannel::Depth) => {
            HtxMarketAdapter::depth_from_ws(frame.payload()).map(ParsedMarketFeed::Depth)
        }
        (MarketFeedProvider::Htx, MarketFeedChannel::Kline) => {
            HtxMarketAdapter::kline_from_ws(frame.payload()).map(ParsedMarketFeed::Kline)
        }
        (MarketFeedProvider::Htx, MarketFeedChannel::Trade) => {
            HtxMarketAdapter::trade_from_ws(frame.payload()).map(ParsedMarketFeed::Trade)
        }
        (MarketFeedProvider::Coinbase, MarketFeedChannel::Ticker) => {
            CoinbaseMarketAdapter::ticker_from_ws(frame.payload()).map(ParsedMarketFeed::Ticker)
        }
        (MarketFeedProvider::Coinbase, MarketFeedChannel::Depth) => {
            CoinbaseMarketAdapter::depth_from_ws(frame.payload()).map(ParsedMarketFeed::Depth)
        }
        (MarketFeedProvider::Coinbase, MarketFeedChannel::Kline) => {
            CoinbaseMarketAdapter::kline_from_ws(frame.payload()).map(ParsedMarketFeed::Kline)
        }
        (MarketFeedProvider::Coinbase, MarketFeedChannel::Trade) => {
            CoinbaseMarketAdapter::trade_from_ws(frame.payload()).map(ParsedMarketFeed::Trade)
        }
        (_, MarketFeedChannel::None) => Err(AppError::Validation(
            "unsupported market feed channel".to_owned(),
        )),
    }
}

/// 校验并规范化订阅交易对，空集合直接判为配置错误，因为没有交易对的行情连接毫无意义。
/// 每个交易对都走领域值对象校验，返回去分隔符大写形式，随后订阅报文与 REST URL 都基于它生成。
/// 任一交易对非法整次调用即失败，不做跳过或部分保留，避免带着残缺配置建连后才发现少订阅了交易对。
/// 不去重也不排序，重复项会一路传导为重复订阅和重复兜底请求。
fn validate_feed_symbols(symbols: &[&str]) -> AppResult<Vec<String>> {
    if symbols.is_empty() {
        return Err(AppError::Validation(
            "market feed symbols are required".to_owned(),
        ));
    }

    symbols
        .iter()
        .map(|symbol| {
            ValidatedMarketSymbol::from_raw(symbol)
                .map(|symbol| symbol.as_str().to_owned())
                .map_err(validation_error)
        })
        .collect()
}

/// 校验 K 线周期是否落在平台白名单内，并返回规范写法供订阅报文与 REST URL 使用。
/// 与交易对不同，空集合被允许，那种配置只订阅 ticker、盘口与成交而不要任何 K 线频道。
/// 校验借用 K 线幂等键构造器完成，传入当前时间只是为了满足其签名，该时间不会被保留或使用。
/// 任一周期非法则整次调用失败，把错误拦截在生成订阅报文和发起网络请求之前。
fn validate_feed_intervals(intervals: &[&str]) -> AppResult<Vec<String>> {
    intervals
        .iter()
        .map(|interval| {
            KlineUpsertKey::new(*interval, Utc::now())
                .map(|key| key.interval().to_owned())
                .map_err(validation_error)
        })
        .collect()
}
