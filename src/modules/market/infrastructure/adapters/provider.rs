//! 第三方行情 provider 适配基础设施。
//!
//! 将 Bitget、HTX 与 Coinbase 的订阅、REST 兜底响应和 WebSocket payload 归一化为领域快照；
//! 缺失或非法的交易对、价格、周期与时间戳直接报错，不写入权威缓存。
//!
//! 本文件承担四类职责：识别后台配置的 provider 代码、拼接各家的 WebSocket 地址与订阅报文、
//! 把 REST 兜底响应改写成与 WebSocket 同形的载荷、以及把最终载荷解析成领域快照。
//! 第三条职责让 REST 与 WebSocket 复用同一套解析器：REST 结果先被包装成 provider 的推送格式，
//! 再交给对应的 `*_from_ws` 解析，因此新增字段只需改一处。
//! 交易对在三种写法间转换：平台内部用去分隔符大写形式，HTX 需要小写，Coinbase 需要 `BASE-QUOTE` 产品 ID。
//! 周期同样各家不同，Bitget 订阅用 `1H`/`1D`、REST 用 `1min` 一类，HTX 统一 `60min` 风格，
//! Coinbase 用 `ONE_MINUTE` 这类枚举，映射函数对未知取值一律保持原样透传或落到各自的兼容缺省值。
//! 所有数值一律经十进制字符串解析成 `BigDecimal`，绝不走浮点，缺字段时宁可报错也不填 0，
//! 以免把「没有数据」伪装成「价格为零」而污染下游的下单、结算与强平判断。
//! 时间戳按各家单位区分：毫秒、秒和 RFC3339 分别有独立的取值函数，解析失败即拒绝该帧。

use super::feed::{
    MarketFeedConfig, MarketFeedRestFallbackConfig, MarketFeedRestFallbackKlineRequest,
    MarketFeedRestFallbackTickerRequest,
};
use crate::{
    config::Settings,
    error::{AppError, AppResult},
    modules::market::{
        MarketDataProvider, MarketDepthLevel, MarketDepthSnapshot, MarketKlineSnapshot,
        MarketKlineValues, MarketTickerSnapshot, MarketTickerValues, MarketTradeSide,
        MarketTradeTick, sanitize_symbol,
    },
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use std::str::FromStr;

pub struct BitgetMarketAdapter;
pub struct HtxMarketAdapter;
pub struct CoinbaseMarketAdapter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketFeedProvider {
    Bitget,
    Htx,
    Coinbase,
}

impl MarketFeedProvider {
    /// 将后台配置的 provider 代码或兼容别名规范化为已支持的行情源枚举。
    /// 比较忽略大小写和首尾空白；未知代码返回校验错误，不会静默回退到默认 provider。
    pub fn from_code(code: &str) -> AppResult<Self> {
        let normalized = code.trim().to_ascii_lowercase();
        for provider in Self::available_providers() {
            if provider.aliases().contains(&normalized.as_str()) {
                return Ok(*provider);
            }
        }
        Err(AppError::Validation(format!(
            "unsupported market feed provider: {normalized}"
        )))
    }

    /// 返回该行情源的规范代码，取值为 `bitget`、`htx` 或 `coinbase`，与别名表中的首选写法一致。
    /// 这是回写配置、日志和指标标签时应当使用的稳定标识，不随后台配置里填写的别名而变化。
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Bitget => "bitget",
            Self::Htx => "htx",
            Self::Coinbase => "coinbase",
        }
    }

    /// 返回该 provider 可接受的后台配置别名，用于兼容 HTX/Huobi 与 Coinbase 历史代码。
    /// 表中的第一项就是规范代码，其余是历史遗留写法，例如 HTX 改名前的 `huobi` 和 Coinbase 的长短横线两种形式。
    /// 所有别名都必须是小写，因为匹配前只把输入转小写而不做其他归一化，大写别名将永远匹配不上。
    pub const fn aliases(&self) -> &'static [&'static str] {
        match self {
            Self::Bitget => &["bitget"],
            Self::Htx => &["htx", "huobi"],
            Self::Coinbase => &[
                "coinbase",
                "coinbase_advanced_trade",
                "coinbase-advanced-trade",
            ],
        }
    }

    /// 返回未显式配置时启用的 provider 顺序：Bitget 后 HTX；Coinbase 需明确选择。
    /// 顺序有意义，配置展开和 REST 兜底都会按这个次序逐个处理，因此 Bitget 是事实上的首选行情源。
    /// Coinbase 不入默认集合，是因为它的产品 ID 与增量盘口语义和另外两家差异较大，需要显式确认后启用。
    pub const fn default_providers() -> [Self; 2] {
        [Self::Bitget, Self::Htx]
    }

    /// 返回当前构建支持的全部 provider，供后台校验与运行时配置展开。
    /// 代码识别正是遍历这个清单再逐个比对别名，因此新增行情源时必须同步登记到这里，否则配置永远无法通过校验。
    pub const fn available_providers() -> &'static [Self] {
        &[Self::Bitget, Self::Htx, Self::Coinbase]
    }
}

impl MarketFeedProvider {
    /// 根据 provider 选择 WebSocket 地址并为每个 symbol/interval 生成订阅消息。
    /// 本函数只组装配置，不连接远端；输入集合按调用方顺序保留，重复项不会在此去重。
    pub(super) fn feed_config(
        &self,
        settings: &Settings,
        symbols: &[String],
        intervals: &[String],
    ) -> AppResult<MarketFeedConfig> {
        Ok(MarketFeedConfig::new(
            *self,
            self.feed_url(settings),
            self.subscription_messages(symbols, intervals),
            symbols.to_vec(),
            intervals.to_vec(),
        ))
    }

    /// 为每个交易对生成 ticker 兜底请求，并为 symbol×interval 笛卡尔积生成 K 线兜底请求。
    /// 只拼接 provider REST URL，不发送 HTTP；交易对和周期须由上游配置校验。
    pub(super) fn rest_fallback_config(
        &self,
        settings: &Settings,
        symbols: &[String],
        intervals: &[String],
    ) -> AppResult<MarketFeedRestFallbackConfig> {
        Ok(MarketFeedRestFallbackConfig::new(
            *self,
            self.ticker_fallback_requests(settings, symbols),
            self.kline_fallback_requests(settings, symbols, intervals),
        ))
    }

    /// 从运行配置中取出该 provider 的 WebSocket 地址，三家分别对应各自独立的配置项。
    /// 地址原样克隆，不做协议、路径或结尾斜杠的校验，连不上只会在 worker 建连时暴露。
    fn feed_url(&self, settings: &Settings) -> String {
        match self {
            Self::Bitget => settings.bitget_ws_url.clone(),
            Self::Htx => settings.htx_ws_url.clone(),
            Self::Coinbase => settings.coinbase_ws_url.clone(),
        }
    }

    /// 为每个交易对生成一条 ticker 兜底请求，请求中同时保留原交易对与拼好的 URL。
    /// 之所以随请求携带交易对，是因为部分 provider 的 REST 响应里没有交易对字段，
    /// 后续包装成 WebSocket 同形载荷时必须从请求上下文补回来。
    /// 输出顺序与输入交易对一致，不去重；重复交易对会产生重复请求并被真实发出。
    fn ticker_fallback_requests(
        &self,
        settings: &Settings,
        symbols: &[String],
    ) -> Vec<MarketFeedRestFallbackTickerRequest> {
        symbols
            .iter()
            .map(|symbol| {
                MarketFeedRestFallbackTickerRequest::new(
                    symbol.clone(),
                    self.ticker_fallback_url(settings, symbol),
                )
            })
            .collect()
    }

    /// 按各家 REST 契约拼出单交易对的 ticker 查询地址，基址一律先去掉结尾斜杠再拼接，避免出现双斜杠。
    /// Bitget 走 `/api/v2/spot/market/tickers` 并以查询参数传交易对，交易对保持平台内部的大写写法。
    /// HTX 走 `/market/detail/merged`，其接口只接受小写交易对，因此这里必须转小写。
    /// Coinbase 走 `/api/v3/brokerage/market/products/{product_id}`，交易对要先转成 `BASE-QUOTE` 形式的产品 ID。
    /// 本函数只做字符串拼接，不发请求，也不对交易对做合法性校验，非法输入会拼出一个注定 404 的地址。
    fn ticker_fallback_url(&self, settings: &Settings, symbol: &str) -> String {
        match self {
            Self::Bitget => format!(
                "{}/api/v2/spot/market/tickers?symbol={symbol}",
                settings.bitget_rest_base_url.trim_end_matches('/')
            ),
            Self::Htx => format!(
                "{}/market/detail/merged?symbol={}",
                settings.htx_rest_base_url.trim_end_matches('/'),
                symbol.to_ascii_lowercase()
            ),
            Self::Coinbase => format!(
                "{}/api/v3/brokerage/market/products/{}",
                settings.coinbase_rest_base_url.trim_end_matches('/'),
                coinbase_product_id(symbol)
            ),
        }
    }

    /// 按交易对与周期的笛卡尔积展开 K 线兜底请求，请求数量等于两个集合长度之积。
    /// 外层遍历交易对、内层遍历周期，因此同一交易对的各个周期在结果中连续排列，便于按交易对观察兜底进度。
    /// 每条请求都记下交易对与周期，因为 REST 响应通常只有裸数组，包装回 WebSocket 形状时全靠这两项补齐。
    /// 集合较大时请求数会迅速膨胀，调用方需要自行控制配置规模，本函数不做上限保护也不去重。
    fn kline_fallback_requests(
        &self,
        settings: &Settings,
        symbols: &[String],
        intervals: &[String],
    ) -> Vec<MarketFeedRestFallbackKlineRequest> {
        symbols
            .iter()
            .flat_map(|symbol| {
                intervals.iter().map(move |interval| {
                    MarketFeedRestFallbackKlineRequest::new(
                        symbol.clone(),
                        interval.clone(),
                        self.kline_fallback_url(settings, symbol, interval),
                    )
                })
            })
            .collect()
    }

    /// 按各家 REST 契约拼出单交易对、单周期的 K 线查询地址，周期先映射成该 provider 的专有写法。
    /// Bitget 用 `granularity` 参数并接受 `1min` 一类粒度；HTX 用 `period` 参数且交易对必须小写。
    /// Coinbase 的接口要求显式时间窗口，这里以当前时刻为终点、按周期秒数乘 300 回推起点，
    /// 也就是一次最多请求 300 根蜡烛，起点用饱和减法防止在极端时钟下溢出为负。
    /// 因为终点取的是调用时刻，同一配置每次拼出的 Coinbase 地址都不同，该 URL 不可缓存复用。
    fn kline_fallback_url(&self, settings: &Settings, symbol: &str, interval: &str) -> String {
        match self {
            Self::Bitget => format!(
                "{}/api/v2/spot/market/candles?symbol={symbol}&granularity={}",
                settings.bitget_rest_base_url.trim_end_matches('/'),
                bitget_rest_interval(interval)
            ),
            Self::Htx => format!(
                "{}/market/history/kline?symbol={}&period={}",
                settings.htx_rest_base_url.trim_end_matches('/'),
                symbol.to_ascii_lowercase(),
                htx_subscription_interval(interval)
            ),
            Self::Coinbase => {
                let (granularity, seconds) = coinbase_rest_granularity(interval);
                let end = Utc::now().timestamp();
                let start = end.saturating_sub(seconds * 300);
                format!(
                    "{}/api/v3/brokerage/market/products/{}/candles?start={start}&end={end}&granularity={granularity}",
                    settings.coinbase_rest_base_url.trim_end_matches('/'),
                    coinbase_product_id(symbol)
                )
            }
        }
    }

    /// 按 provider 分派到对应的订阅报文生成函数，返回的字符串需由 worker 在建连成功后依次发送。
    /// 三家的报文格式互不相同，但都在这一步一次性生成完毕，重连时可以直接复用而无需重新计算。
    fn subscription_messages(&self, symbols: &[String], intervals: &[String]) -> Vec<String> {
        match self {
            Self::Bitget => bitget_subscriptions(symbols, intervals),
            Self::Htx => htx_subscriptions(symbols, intervals),
            Self::Coinbase => coinbase_subscriptions(symbols, intervals),
        }
    }
}

/// 生成 Bitget 现货订阅报文：每个交易对固定订阅 ticker、books50 盘口与逐笔成交三条，再按周期追加 candle 频道。
/// 因此单个交易对的报文数是 3 加周期数量，全部使用 `op: subscribe` 且 `instType` 固定为 SPOT。
/// 盘口选择 books50 而非全量档位，是在深度与带宽之间的取舍，解析端也据此按 50 档语义处理。
/// 周期会先转成 Bitget 订阅专用写法，例如 `1h` 变成 `1H`，拼成 `candle1H` 这样的频道名。
/// 本函数只产出文本，不建连、不发送，也不校验交易对在 Bitget 是否真实存在。
fn bitget_subscriptions(symbols: &[String], intervals: &[String]) -> Vec<String> {
    symbols
        .iter()
        .flat_map(|symbol| {
            let mut messages = vec![
                json!({"op":"subscribe","args":[{"instType":"SPOT","channel":"ticker","instId":symbol}]}).to_string(),
                json!({"op":"subscribe","args":[{"instType":"SPOT","channel":"books50","instId":symbol}]}).to_string(),
                json!({"op":"subscribe","args":[{"instType":"SPOT","channel":"trade","instId":symbol}]}).to_string(),
            ];
            messages.extend(intervals.iter().map(|interval| {
                json!({"op":"subscribe","args":[{"instType":"SPOT","channel":format!("candle{}", bitget_subscription_interval(interval)),"instId":symbol}]}).to_string()
            }));
            messages
        })
        .collect()
}

/// 生成 HTX 订阅报文：交易对先转小写，再拼成 `market.{symbol}.detail`、`.depth.step0`、`.trade.detail` 三条主题。
/// 随后按周期追加 `market.{symbol}.kline.{period}`，周期同样先映射成 HTX 写法，例如 `1h` 变成 `60min`。
/// 每条报文都是 `{"sub": "主题"}` 结构，与 Bitget 的批量 args 形式不同，只能逐条发送。
/// 盘口选用 step0 即不做价格聚合的原始档位，解析时直接取 `tick.bids/asks`。
/// 与 Bitget 一样，本函数只负责拼报文，交易对是否被 HTX 支持要等订阅响应才能知道。
fn htx_subscriptions(symbols: &[String], intervals: &[String]) -> Vec<String> {
    symbols
        .iter()
        .flat_map(|symbol| {
            let symbol = symbol.to_ascii_lowercase();
            let mut messages = vec![
                json!({"sub":format!("market.{symbol}.detail")}).to_string(),
                json!({"sub":format!("market.{symbol}.depth.step0")}).to_string(),
                json!({"sub":format!("market.{symbol}.trade.detail")}).to_string(),
            ];
            messages.extend(intervals.iter().map(|interval| {
                json!({"sub":format!("market.{symbol}.kline.{}", htx_subscription_interval(interval))}).to_string()
            }));
            messages
        })
        .collect()
}

/// 生成 Coinbase Advanced Trade 订阅报文，与另外两家最大的差别是一条报文携带全部产品 ID 而非每对一条。
/// 交易对先统一转成 `BASE-QUOTE` 产品 ID，然后固定订阅 ticker、level2、market_trades 与 heartbeats 四个频道。
/// heartbeats 不产出行情，订阅它是为了在市场清淡时维持连接活跃，避免被服务端判定空闲断开。
/// candles 频道仅在配置包含 `5m` 时才追加，因为 Coinbase 的推送粒度固定为五分钟，其他周期只能靠 REST 兜底获取。
/// level2 推送的是增量更新而非完整订单簿，解析出的盘口档位因此只代表本次变更，不能当作全量深度使用。
fn coinbase_subscriptions(symbols: &[String], intervals: &[String]) -> Vec<String> {
    let product_ids: Vec<String> = symbols
        .iter()
        .map(|symbol| coinbase_product_id(symbol))
        .collect();
    let mut messages = vec![
        json!({"type":"subscribe","product_ids": product_ids.clone(), "channel":"ticker"})
            .to_string(),
        json!({"type":"subscribe","product_ids": product_ids.clone(), "channel":"level2"})
            .to_string(),
        json!({"type":"subscribe","product_ids": product_ids.clone(), "channel":"market_trades"})
            .to_string(),
        json!({"type":"subscribe","product_ids": product_ids.clone(), "channel":"heartbeats"})
            .to_string(),
    ];
    if intervals.iter().any(|interval| interval == "5m") {
        messages.push(
            json!({"type":"subscribe","product_ids": product_ids.clone(), "channel":"candles"})
                .to_string(),
        );
    }
    messages
}

/// 把平台周期转成 Bitget WebSocket candle 频道所用的写法，只有小时和日线需要改成大写 `1H`、`1D`。
/// 分钟级周期在两边写法一致，因此原样透传；未知周期同样透传，交由服务端在订阅时拒绝。
/// 反向解析由 `bitget_interval` 负责，它同时接受大小写形式，两个函数必须成对维护。
fn bitget_subscription_interval(interval: &str) -> &str {
    match interval {
        "1h" => "1H",
        "1d" => "1D",
        value => value,
    }
}

/// 把平台周期转成 Bitget REST `granularity` 参数所用的写法，分钟级要展开成 `1min` 一类，日线写作 `1day`。
/// 注意这与 WebSocket 订阅的写法不同，同一个 `1m` 在订阅里保持原样、在 REST 里必须写成 `1min`。
/// `1h` 未在表中列出，会按原样透传给 Bitget，是否被接受取决于该接口当时的取值约定。
fn bitget_rest_interval(interval: &str) -> &str {
    match interval {
        "1m" => "1min",
        "5m" => "5min",
        "15m" => "15min",
        "1d" => "1day",
        value => value,
    }
}

/// 把平台周期转成 HTX 的 period 写法，分钟级展开为 `1min` 一类，小时线写作 `60min`，日线写作 `1day`。
/// WebSocket 订阅主题与 REST 的 `period` 参数共用这一套写法，因此两条链路只需维护这一个映射。
/// 未知周期原样透传，最终会由 HTX 在订阅或查询时报错，本函数不提前拦截。
fn htx_subscription_interval(interval: &str) -> &str {
    match interval {
        "1m" => "1min",
        "5m" => "5min",
        "15m" => "15min",
        "1h" => "60min",
        "1d" => "1day",
        value => value,
    }
}

/// 把平台周期同时转成 Coinbase 的粒度枚举与该周期对应的秒数，两个返回值缺一不可。
/// 枚举用于请求参数，秒数用于回推查询窗口的起点，两者必须匹配，否则窗口跨度与粒度会对不上。
/// 与其他映射不同，这里对未知周期落到 `ONE_MINUTE` 与 60 秒的缺省值而不是透传，
/// 因为 Coinbase 只接受固定枚举，透传非法值会让整个请求直接失败，退化成一分钟至少还能拿到数据。
/// 平台支持的 `4h` 不在表中，走缺省分支会被当作一分钟处理，这是当前已知的口径缺口。
fn coinbase_rest_granularity(interval: &str) -> (&'static str, i64) {
    match interval {
        "1m" => ("ONE_MINUTE", 60),
        "5m" => ("FIVE_MINUTE", 300),
        "15m" => ("FIFTEEN_MINUTE", 900),
        "1h" => ("ONE_HOUR", 3_600),
        "1d" => ("ONE_DAY", 86_400),
        _ => ("ONE_MINUTE", 60),
    }
}

impl BitgetMarketAdapter {
    /// 解析 Bitget ticker 帧为标准快照；last、24h 指标、交易对与观察时间缺失或非法时拒绝进入权威缓存。
    /// 只取 `data` 数组的首个对象，因此同帧内的其余条目会被丢弃，这与逐帧推送单交易对的实际格式一致。
    /// 交易对优先取条目内的 `instId`，缺失时回退到帧级 `arg.instId`；最新价兼容 `lastPr` 与旧字段 `last`。
    /// 成交量按 `baseVolume`、`baseVol`、`vol24h` 的顺序取首个存在的字段，是必填项，缺失即整帧拒绝。
    /// 高低价与开盘价为可选，缺失时由统计换算函数按平盘规则回填；涨跌幅优先用 `change24h` 给出的比率换算。
    /// 观察时间先取条目内 `ts`，再回退到帧级 `ts`，两者都没有则拒绝，绝不用本机时间冒充 provider 时间。
    /// 字段缺失、精度非法或载荷损坏时返回解析错误，不生成可进入交易缓存的虚假行情。
    pub fn ticker_from_ws(payload: &str) -> AppResult<MarketTickerSnapshot> {
        let value = parse_json(payload)?;
        let item = first_data_object(&value)?;
        let symbol = bitget_symbol(&value, item)?;
        let last_price = decimal_field(item, &["lastPr", "last"])?;
        let values = ticker_24h_values(
            last_price,
            optional_decimal_field(item, &["high24h", "high"])?,
            optional_decimal_field(item, &["low24h", "low"])?,
            decimal_field(item, &["baseVolume", "baseVol", "vol24h"])?,
            optional_decimal_field(item, &["open24h", "open"])?,
            optional_decimal_field(item, &["change24h", "changeUtc24h"])?,
        );
        MarketTickerSnapshot::with_24h(
            MarketDataProvider::Bitget,
            symbol,
            values,
            millis_field(item, &["ts"]).or_else(|_| millis_field_from_value(&value, &["ts"]))?,
        )
        .map_err(validation_error)
    }

    /// 解析 Bitget `books50` 的首个 `data` 对象；`bids`、`asks` 及毫秒时间戳缺失或非法时返回校验错误。
    /// 档位顺序按 provider 载荷保留，函数不执行 Redis 写入或 WebSocket 广播。
    pub fn depth_from_ws(payload: &str) -> AppResult<MarketDepthSnapshot> {
        let value = parse_json(payload)?;
        let item = first_data_object(&value)?;
        let symbol = bitget_symbol(&value, item)?;
        MarketDepthSnapshot::new(
            MarketDataProvider::Bitget,
            symbol,
            levels(item.get("bids"))?,
            levels(item.get("asks"))?,
            millis_field(item, &["ts"]).or_else(|_| millis_field_from_value(&value, &["ts"]))?,
        )
        .map_err(validation_error)
    }

    /// 解析 Bitget `candle*` 首行数组，将索引 0～5 映射为开盘毫秒与 OHLCV；频道仅接受平台支持周期。
    /// 与 ticker 不同，K 线的 `data` 元素是数组而非对象，六个字段完全按位置取值，顺序错位无法被察觉。
    /// 交易对与周期都从帧级 `arg` 读取：`instId` 给出交易对，`channel` 去掉 `candle` 前缀后映射回平台周期。
    /// 周期映射同时接受 `1H` 与 `1h` 两种写法，未列入白名单的周期直接返回校验错误而不是猜测。
    /// 顶层 `ts` 缺失时以开盘时间作为观察时间，其他必填字段非法则拒绝该帧。
    /// 用开盘时间顶替观察时间会削弱同一时间槽内的防倒退能力，因为同槽多次推送将得到完全相同的观察时间。
    pub fn kline_from_ws(payload: &str) -> AppResult<MarketKlineSnapshot> {
        let value = parse_json(payload)?;
        let row = value
            .get("data")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(Value::as_array)
            .ok_or_else(|| AppError::Validation("bitget kline data is required".to_owned()))?;
        let arg = value.get("arg").and_then(Value::as_object);
        let symbol = arg
            .and_then(|arg| arg.get("instId"))
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::Validation("bitget instId is required".to_owned()))?;
        let channel = arg
            .and_then(|arg| arg.get("channel"))
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::Validation("bitget kline channel is required".to_owned()))?;
        let open_time = millis_value(row.first())?;
        MarketKlineSnapshot::new(
            MarketDataProvider::Bitget,
            symbol,
            bitget_interval(channel)?,
            open_time,
            MarketKlineValues {
                open: decimal_value(row.get(1))?,
                high: decimal_value(row.get(2))?,
                low: decimal_value(row.get(3))?,
                close: decimal_value(row.get(4))?,
                volume: decimal_value(row.get(5))?,
            },
            millis_field_from_value(&value, &["ts"]).unwrap_or(open_time),
        )
        .map_err(validation_error)
    }

    /// 解析 Bitget `trade` 首条成交，读取成交号、买卖方向、价格、数量和毫秒时间；缺字段即返回校验错误。
    /// 同一帧里可能包含多笔成交，这里只取首条，其余会被丢弃，因此逐笔数据在高频时段并不完整。
    /// 各字段都按候选名依次尝试以兼容不同版本：成交号取 `tradeId` 或 `id`，方向取 `side` 或 `direction`，
    /// 价格取 `price` 或 `px`，数量取 `size`、`qty` 或 `amount`。
    /// 方向只接受 buy/sell 及 bid/ask 写法，无法识别时返回校验错误而不是默认成买入。
    pub fn trade_from_ws(payload: &str) -> AppResult<MarketTradeTick> {
        let value = parse_json(payload)?;
        let item = first_data_object(&value)?;
        let symbol = bitget_symbol(&value, item)?;
        MarketTradeTick::new(
            MarketDataProvider::Bitget,
            symbol,
            string_field(item, &["tradeId", "id"])?,
            trade_side(&string_field(item, &["side", "direction"])?)?,
            decimal_field(item, &["price", "px"])?,
            decimal_field(item, &["size", "qty", "amount"])?,
            millis_field(item, &["ts"]).or_else(|_| millis_field_from_value(&value, &["ts"]))?,
        )
        .map_err(validation_error)
    }
}

impl HtxMarketAdapter {
    /// 解析 HTX ticker 帧为标准快照；使用 tick 时间和 24h OHLC/成交量，非法字段不得降级成零价格。
    /// 交易对不在字段里，而是从频道名 `market.{symbol}.detail` 按点号切分取第二段还原出来。
    /// 最新价取 `close` 或 `last`，成交量取 `amount` 或 `vol`，两者必填；高低价与开盘价可选。
    /// HTX 不提供现成的涨跌比率，因此涨跌幅只能由最新价与开盘价推算，开盘价缺失时记为 0。
    /// 观察时间优先取帧级 `ts`，缺失才回退到 `tick.ts`，与盘口解析的取值顺序恰好相反。
    /// 字段缺失、精度非法或载荷损坏时返回解析错误，不生成可进入交易缓存的虚假行情。
    pub fn ticker_from_ws(payload: &str) -> AppResult<MarketTickerSnapshot> {
        let value = parse_json(payload)?;
        let tick = required_object(value.get("tick"), "htx tick")?;
        let last_price = decimal_field(tick, &["close", "last"])?;
        let values = ticker_24h_values(
            last_price,
            optional_decimal_field(tick, &["high"])?,
            optional_decimal_field(tick, &["low"])?,
            decimal_field(tick, &["amount", "vol"])?,
            optional_decimal_field(tick, &["open"])?,
            None,
        );
        MarketTickerSnapshot::with_24h(
            MarketDataProvider::Htx,
            htx_symbol(&value)?,
            values,
            millis_field_from_value(&value, &["ts"]).or_else(|_| millis_field(tick, &["ts"]))?,
        )
        .map_err(validation_error)
    }

    /// 解析 HTX `market.{symbol}.depth.step0` 的 `tick.bids/asks`，优先使用 tick 毫秒时间，缺失再取顶层 `ts`。
    /// 频道交易对、盘口数组或时间戳非法时返回校验错误，不生成缓存写入。
    pub fn depth_from_ws(payload: &str) -> AppResult<MarketDepthSnapshot> {
        let value = parse_json(payload)?;
        let tick = required_object(value.get("tick"), "htx tick")?;
        MarketDepthSnapshot::new(
            MarketDataProvider::Htx,
            htx_symbol(&value)?,
            levels(tick.get("bids"))?,
            levels(tick.get("asks"))?,
            millis_field(tick, &["ts"]).or_else(|_| millis_field_from_value(&value, &["ts"]))?,
        )
        .map_err(validation_error)
    }

    /// 解析 HTX `market.{symbol}.kline.{period}`，以 `tick.id` 秒时间作为开盘时间并读取 OHLCV。
    /// `tick.id` 的单位是秒而非毫秒，因此走独立的秒级时间解析，误用毫秒解析会把时间放大千倍。
    /// 周期从频道名末段还原，`60min` 与 `1hour` 都映射为 `1h`，未知末段直接返回校验错误。
    /// 成交量取 `amount` 或 `vol`，与 ticker 保持同一套候选字段顺序，确保两条链路口径一致。
    /// 顶层 `ts` 缺失时观察时间回退到开盘时间；未知周期或非法数值返回校验错误。
    pub fn kline_from_ws(payload: &str) -> AppResult<MarketKlineSnapshot> {
        let value = parse_json(payload)?;
        let tick = required_object(value.get("tick"), "htx tick")?;
        let interval = htx_interval(
            value
                .get("ch")
                .and_then(Value::as_str)
                .ok_or_else(|| AppError::Validation("htx channel is required".to_owned()))?,
        )?;
        let open_time = seconds_field(tick, &["id"])?;
        MarketKlineSnapshot::new(
            MarketDataProvider::Htx,
            htx_symbol(&value)?,
            interval,
            open_time,
            MarketKlineValues {
                open: decimal_field(tick, &["open"])?,
                high: decimal_field(tick, &["high"])?,
                low: decimal_field(tick, &["low"])?,
                close: decimal_field(tick, &["close"])?,
                volume: decimal_field(tick, &["amount", "vol"])?,
            },
            millis_field_from_value(&value, &["ts"]).unwrap_or(open_time),
        )
        .map_err(validation_error)
    }

    /// 解析 HTX `trade.detail` 的首条成交，方向仅接受 buy/sell（及兼容 bid/ask），时间优先取成交项 `ts`。
    /// 成交列表嵌在 `tick.data` 里，比 Bitget 多一层，同帧多笔成交同样只取首条，其余丢弃。
    /// 成交号取 `id` 或 `tradeId`，方向取 `direction` 或 `side`，数量取 `amount` 或 `quantity`。
    /// 数据数组缺失、成交字段非法或 item 时间不可用时回退帧时间；仍无法形成快照则返回校验错误。
    pub fn trade_from_ws(payload: &str) -> AppResult<MarketTradeTick> {
        let value = parse_json(payload)?;
        let tick = required_object(value.get("tick"), "htx tick")?;
        let item = tick
            .get("data")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(Value::as_object)
            .ok_or_else(|| AppError::Validation("htx trade data is required".to_owned()))?;
        MarketTradeTick::new(
            MarketDataProvider::Htx,
            htx_symbol(&value)?,
            string_field(item, &["id", "tradeId"])?,
            trade_side(&string_field(item, &["direction", "side"])?)?,
            decimal_field(item, &["price"])?,
            decimal_field(item, &["amount", "quantity"])?,
            millis_field(item, &["ts"]).or_else(|_| millis_field_from_value(&value, &["ts"]))?,
        )
        .map_err(validation_error)
    }
}

impl CoinbaseMarketAdapter {
    /// 解析 Coinbase ticker 帧为标准快照；交易对、最新价、成交量和观察时间是权威价格写入的前置条件。
    /// 载荷结构比另外两家多一层 `events`，需要在事件数组中找到首个含 `tickers` 的事件再取其首条记录。
    /// 交易对来自 `product_id`，要把 `BASE-QUOTE` 的短横线去掉并转大写才能还原成平台内部写法。
    /// 高低价缺失时回退为最新价，涨跌幅缺失时记为 0；涨跌额固定写 0，因为该接口不提供 24 小时开盘价。
    /// 涨跌幅字段可能带百分号后缀，取值时会先剥掉再解析，兼容 `price_percent_chg_24_h` 与另一种旧命名。
    /// 观察时间优先取记录里的 RFC3339 时间，其次取帧级 `timestamp`，最后才尝试毫秒字段。
    /// 字段缺失、精度非法或载荷损坏时返回解析错误，不生成可进入交易缓存的虚假行情。
    pub fn ticker_from_ws(payload: &str) -> AppResult<MarketTickerSnapshot> {
        let value = parse_json(payload)?;
        let item = coinbase_first_collection_object(&value, "tickers", "coinbase ticker")?;
        let symbol = coinbase_symbol_from_item(item)?;
        let last_price = decimal_field(item, &["price"])?;
        let price_change_percent_24h = coinbase_optional_percent_field(
            item,
            &["price_percent_chg_24_h", "price_percentage_change_24h"],
        )?
        .unwrap_or_else(|| BigDecimal::from(0));
        let values = MarketTickerValues::new(
            last_price.clone(),
            optional_decimal_field(item, &["high_24_h", "high_24h"])?
                .unwrap_or_else(|| last_price.clone()),
            optional_decimal_field(item, &["low_24_h", "low_24h"])?
                .unwrap_or_else(|| last_price.clone()),
            decimal_field(item, &["volume_24_h", "volume_24h"])?,
            BigDecimal::from(0),
            price_change_percent_24h,
        );
        MarketTickerSnapshot::with_24h(
            MarketDataProvider::Coinbase,
            &symbol,
            values,
            coinbase_observed_at(&value, item)?,
        )
        .map_err(validation_error)
    }

    /// 解析 Coinbase `level2` 首个事件的 updates，按 side 拆分买卖档；没有 provider 时间时才使用本机当前时间。
    /// 与另外两家把买卖盘放在不同数组不同，Coinbase 的 updates 是一个混合列表，靠每条的 `side` 字段区分方向。
    /// 买方接受 bid/buy，卖方接受 offer/ask/sell，比较忽略大小写；无法归类的档位会被静默跳过。
    /// 产品 ID 优先取事件级 `product_id`，缺失时回退到首条 update 里的同名字段。
    /// 观察时间取自首条 update，整个 updates 为空时退化为本机当前时间，此时新鲜度判定会偏乐观。
    /// 这是本文件中唯一会用本机时间冒充观察时间的地方，取舍原因是增量盘口本就允许丢弃。
    /// 函数保留增量载荷语义，不补全完整订单簿；产品、档位或数值非法时返回校验错误。
    pub fn depth_from_ws(payload: &str) -> AppResult<MarketDepthSnapshot> {
        let value = parse_json(payload)?;
        let event = coinbase_first_event_object(&value)?;
        let updates = required_array(event.get("updates"), "coinbase level2 updates")?;
        let symbol = event
            .get("product_id")
            .and_then(Value::as_str)
            .or_else(|| {
                updates
                    .first()
                    .and_then(|update| update.get("product_id"))
                    .and_then(Value::as_str)
            })
            .map(coinbase_symbol_from_product_id)
            .ok_or_else(|| AppError::Validation("coinbase product_id is required".to_owned()))?;
        let first_update = updates.first().and_then(Value::as_object);
        MarketDepthSnapshot::new(
            MarketDataProvider::Coinbase,
            &symbol,
            coinbase_depth_levels(updates, true)?,
            coinbase_depth_levels(updates, false)?,
            first_update
                .map(|item| coinbase_observed_at(&value, item))
                .transpose()?
                .unwrap_or_else(Utc::now),
        )
        .map_err(validation_error)
    }

    /// 解析 Coinbase `candles` 首条记录，以 `start` 秒时间为开盘并读取 OHLCV。
    /// `start` 是秒级时间戳，走独立的秒级解析函数，与 HTX 的 `tick.id` 处理方式一致。
    /// 周期先看记录里的 `interval`，没有再看 `granularity`，两者都接受枚举写法与平台写法。
    /// 未识别粒度按现有兼容合同映射为 `5m`，观察时间缺失时回退到开盘时间。
    /// 这个缺省来自 Coinbase 推送只有五分钟粒度的事实，但也意味着字段异常时会被静默当成五分钟线写入历史。
    pub fn kline_from_ws(payload: &str) -> AppResult<MarketKlineSnapshot> {
        let value = parse_json(payload)?;
        let candle = coinbase_first_collection_object(&value, "candles", "coinbase candle")?;
        let symbol = coinbase_symbol_from_item(candle)?;
        let open_time = seconds_field(candle, &["start"])?;
        MarketKlineSnapshot::new(
            MarketDataProvider::Coinbase,
            &symbol,
            coinbase_candle_interval(candle),
            open_time,
            MarketKlineValues {
                open: decimal_field(candle, &["open"])?,
                high: decimal_field(candle, &["high"])?,
                low: decimal_field(candle, &["low"])?,
                close: decimal_field(candle, &["close"])?,
                volume: decimal_field(candle, &["volume"])?,
            },
            coinbase_observed_at(&value, candle).unwrap_or(open_time),
        )
        .map_err(validation_error)
    }

    /// 解析 Coinbase `market_trades` 首条成交并保留 product_id、trade_id、side、price、size 与 provider 时间。
    /// 记录同样嵌在 `events` 下的 `trades` 数组里，只取首条，成交号兼容 `trade_id` 与 `tradeId` 两种命名。
    /// 数量字段取 `size` 或 `quantity`，价格取 `price`，交易对由产品 ID 去横线转大写还原。
    /// 成交时间与 ticker 共用同一套取值顺序，优先 RFC3339，其次帧级时间戳，最后毫秒字段。
    /// 集合为空、方向未知、数值或时间非法时返回校验错误，不生成供缓存或 WebSocket 发布的成交事件。
    pub fn trade_from_ws(payload: &str) -> AppResult<MarketTradeTick> {
        let value = parse_json(payload)?;
        let trade = coinbase_first_collection_object(&value, "trades", "coinbase trade")?;
        let symbol = coinbase_symbol_from_item(trade)?;
        MarketTradeTick::new(
            MarketDataProvider::Coinbase,
            &symbol,
            string_field(trade, &["trade_id", "tradeId"])?,
            trade_side(&string_field(trade, &["side"])?)?,
            decimal_field(trade, &["price"])?,
            decimal_field(trade, &["size", "quantity"])?,
            coinbase_observed_at(&value, trade)?,
        )
        .map_err(validation_error)
    }
}

/// 将 Bitget REST ticker 包装为与 WS `ticker` 相同的 `arg/data/ts` 形状；无上游 `ts` 时使用本机毫秒时间。
/// 本函数只验证外层 JSON，`data` 必填字段由后续 Bitget ticker 解析器检查。
pub(super) fn bitget_rest_ticker_payload(payload: &str, symbol: &str) -> AppResult<String> {
    let value = parse_json(payload)?;
    Ok(json!({
        "arg": {"channel": "ticker", "instId": symbol},
        "data": value.get("data").cloned().unwrap_or(Value::Null),
        "ts": rest_payload_observed_millis(&value),
    })
    .to_string())
}

/// 将 Bitget REST candle 数组逐行包装为 WS `candle*` 帧；symbol、interval 来自已校验请求上下文。
/// 每一行独立成帧，因此一次 REST 响应会展开成多个帧，各自经过完整解析后逐根写入缓存与历史。
/// 频道名按订阅写法拼成 `candle1H` 这类形式，好让复用的 WebSocket 解析器能反推回平台周期。
/// 各行共用同一个包装时刻的时间戳，所以同批蜡烛的观察时间完全相同，无法据此区分它们的先后。
/// 缺少 `data` 数组返回校验错误，观察时间取上游 `ts` 或包装时本机时间。
pub(super) fn bitget_rest_kline_payloads(
    payload: &str,
    symbol: &str,
    interval: &str,
) -> AppResult<Vec<String>> {
    let value = parse_json(payload)?;
    let rows = required_array(value.get("data"), "bitget REST kline data")?;
    Ok(rows
        .iter()
        .map(|row| {
            json!({
                "arg": {"channel": format!("candle{}", bitget_subscription_interval(interval)), "instId": symbol},
                "data": [row.clone()],
                "ts": rest_payload_observed_millis(&value),
            })
            .to_string()
        })
        .collect())
}

/// 将 HTX REST merged ticker 包装为 `market.{symbol}.detail` WS 形状；无 `tick` 时保留 null 供后续解析器报错。
/// 频道名里的交易对必须转小写，否则复用的解析器从频道切分出的交易对将与 HTX 推送格式不一致。
/// 缺失 `tick` 时故意写入 null 而不在这里报错，是为了让「字段缺失」统一由下游解析器给出一致的错误信息。
/// 观察时间取响应里的 `ts`，缺失才用包装时的本机时间兜底。
pub(super) fn htx_rest_ticker_payload(payload: &str, symbol: &str) -> AppResult<String> {
    let value = parse_json(payload)?;
    let tick = value.get("tick").cloned().unwrap_or(Value::Null);
    Ok(json!({
        "ch": format!("market.{}.detail", symbol.to_ascii_lowercase()),
        "tick": tick,
        "ts": rest_payload_observed_millis(&value),
    })
    .to_string())
}

/// 将 HTX REST K 线数组逐条包装为 `market.{symbol}.kline.{period}` 帧；缺少数组时立即返回校验错误。
/// 每行整体放进 `tick` 字段，因为 HTX 的 REST 行本身就是对象，结构与推送里的 `tick` 完全相同。
/// 频道名同时需要小写交易对和 HTX 写法的周期，解析器正是从这两段还原出交易对与平台周期。
/// 保持 REST 返回顺序与每行数值不变，并统一补入 provider 时间，字段细节由后续 HTX K 线解析器校验。
pub(super) fn htx_rest_kline_payloads(
    payload: &str,
    symbol: &str,
    interval: &str,
) -> AppResult<Vec<String>> {
    let value = parse_json(payload)?;
    let rows = required_array(value.get("data"), "htx REST kline data")?;
    Ok(rows
        .iter()
        .map(|row| {
            json!({
                "ch": format!("market.{}.kline.{}", symbol.to_ascii_lowercase(), htx_subscription_interval(interval)),
                "tick": row.clone(),
                "ts": rest_payload_observed_millis(&value),
            })
            .to_string()
        })
        .collect())
}

/// 将 Coinbase REST product 对象包装为 Advanced Trade `ticker` snapshot 事件。
/// REST 返回的是裸 product 对象，这里整体搬进 `events[0].tickers[0]`，补齐推送特有的三层结构。
/// 由于产品对象被原样复用，其字段名必须与推送一致，解析器才能取到最新价与成交量等必填项。
/// `product_id` 缺失时从请求 symbol 推导，事件时间取包装时本机时间；非对象 JSON 返回校验错误。
/// 事件时间用本机时间意味着 REST 兜底得到的 ticker 观察时间总是「现在」，它在防倒退比较中必然胜过缓存旧值。
pub(super) fn coinbase_rest_ticker_payload(payload: &str, symbol: &str) -> AppResult<String> {
    let value = parse_json(payload)?;
    let product_id = value
        .get("product_id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| coinbase_product_id(symbol));
    let mut ticker = value
        .as_object()
        .cloned()
        .ok_or_else(|| AppError::Validation("coinbase REST product is required".to_owned()))?;
    ticker.insert("product_id".to_owned(), Value::String(product_id));
    Ok(json!({
        "channel": "ticker",
        "timestamp": Utc::now().to_rfc3339(),
        "events": [{
            "type": "snapshot",
            "tickers": [Value::Object(ticker)]
        }]
    })
    .to_string())
}

/// 将 Coinbase REST candles 逐条包装为 `candles` snapshot，并补入请求 product_id 与平台周期。
/// 每根蜡烛独立成帧，并就地写入 `product_id` 与 `interval` 两个字段，因为 REST 行本身不带这些信息。
/// 补入的 `interval` 是平台写法而非 Coinbase 枚举，解析端的周期映射同时接受两种形式，因此可以直接透传。
/// 若某一行不是 JSON 对象则跳过补字段，该行随后会在解析阶段因缺少产品 ID 而被拒绝。
/// 事件观察时间取包装时本机时间；缺少 candles 数组返回校验错误。
pub(super) fn coinbase_rest_kline_payloads(
    payload: &str,
    symbol: &str,
    interval: &str,
) -> AppResult<Vec<String>> {
    let value = parse_json(payload)?;
    let rows = required_array(value.get("candles"), "coinbase REST candle data")?;
    let product_id = coinbase_product_id(symbol);
    Ok(rows
        .iter()
        .map(|row| {
            let mut candle = row.clone();
            if let Some(object) = candle.as_object_mut() {
                object.insert("product_id".to_owned(), Value::String(product_id.clone()));
                object.insert("interval".to_owned(), Value::String(interval.to_owned()));
            }
            json!({
                "channel": "candles",
                "timestamp": Utc::now().to_rfc3339(),
                "events": [{
                    "type": "snapshot",
                    "candles": [candle],
                }]
            })
            .to_string()
        })
        .collect())
}

/// 取 Coinbase 推送里 `events` 数组的首个事件对象，用于那些不按集合名查找的场景，例如增量盘口。
/// 一帧内可能带多个事件，这里只处理第一个，其余会被丢弃；数组缺失或首元素不是对象时返回校验错误。
fn coinbase_first_event_object(value: &Value) -> AppResult<&serde_json::Map<String, Value>> {
    value
        .get("events")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(Value::as_object)
        .ok_or_else(|| AppError::Validation("coinbase event is required".to_owned()))
}

/// 在 `events` 数组里找出第一个包含指定集合字段的事件，再返回该集合的首个对象元素。
/// 与只看首个事件的做法不同，这里会跳过不含目标集合的事件，因此 heartbeats 等噪声事件不会干扰取值。
/// `collection` 是集合字段名，如 `tickers`、`candles`、`trades`；`name` 只用于拼装错误文本。
/// 找不到任何匹配事件、集合为空或首元素不是对象时，都返回带上 `name` 的校验错误。
fn coinbase_first_collection_object<'a>(
    value: &'a Value,
    collection: &str,
    name: &str,
) -> AppResult<&'a serde_json::Map<String, Value>> {
    value
        .get("events")
        .and_then(Value::as_array)
        .and_then(|events| {
            events
                .iter()
                .filter_map(Value::as_object)
                .find_map(|event| event.get(collection).and_then(Value::as_array))
        })
        .and_then(|items| items.first())
        .and_then(Value::as_object)
        .ok_or_else(|| AppError::Validation(format!("{name} is required")))
}

/// 从 Coinbase 记录中读出 `product_id` 并还原成平台内部交易对写法，缺失该字段直接返回校验错误。
/// 之所以不允许回退到帧级信息，是因为一帧内可能混有多个产品，猜错交易对会把行情写到错误的缓存键上。
fn coinbase_symbol_from_item(item: &serde_json::Map<String, Value>) -> AppResult<String> {
    item.get("product_id")
        .and_then(Value::as_str)
        .map(coinbase_symbol_from_product_id)
        .ok_or_else(|| AppError::Validation("coinbase product_id is required".to_owned()))
}

/// 把平台交易对转成 Coinbase 的 `BASE-QUOTE` 产品 ID，做法是按已知计价资产列表匹配后缀再插入横线。
/// 计价资产按列表顺序逐个尝试，`USDT` 排在 `USD` 之前，否则 `BTCUSDT` 会被错误地拆成 `BTCUS-DT`。
/// 匹配要求剩余的基础资产非空，因此像 `USDT` 这样整体等于计价资产的输入不会被拆分。
/// 没有命中任何后缀时原样返回规范化交易对，这种产品 ID 大概率无效，会在请求阶段以 404 暴露。
fn coinbase_product_id(symbol: &str) -> String {
    let normalized = sanitize_symbol(symbol);
    const QUOTES: &[&str] = &[
        "USDT", "USDC", "USD", "EUR", "GBP", "BTC", "ETH", "SOL", "DAI",
    ];
    for quote in QUOTES {
        if normalized.len() > quote.len() && normalized.ends_with(quote) {
            let base = &normalized[..normalized.len() - quote.len()];
            return format!("{base}-{quote}");
        }
    }
    normalized
}

/// 把 Coinbase 的 `BASE-QUOTE` 产品 ID 还原成平台内部交易对：去掉所有横线并转大写。
/// 这是产品 ID 拼装的逆操作，但不校验结果是否属于已配置的交易对，白名单判断由领域层负责。
fn coinbase_symbol_from_product_id(product_id: &str) -> String {
    product_id.replace('-', "").to_ascii_uppercase()
}

/// 判定一根 Coinbase 蜡烛的周期：先看 REST 包装补入的 `interval`，再看推送自带的 `granularity`。
/// 同时接受 `ONE_MINUTE` 这类枚举和 `1m` 这类平台写法，使 REST 与 WebSocket 两条链路能共用解析器。
/// 与其他周期映射不同，这里对任何无法识别的取值都返回 `5m` 而不是报错，
/// 因为 Coinbase 推送的 candles 频道固定就是五分钟粒度，缺字段属于常态。
/// 代价是字段异常时会被静默当成五分钟线写入历史，排查数据错位时需要留意这个缺省。
fn coinbase_candle_interval(candle: &serde_json::Map<String, Value>) -> &str {
    match candle
        .get("interval")
        .and_then(Value::as_str)
        .or_else(|| candle.get("granularity").and_then(Value::as_str))
    {
        Some("ONE_MINUTE") => "1m",
        Some("FIVE_MINUTE") => "5m",
        Some("FIFTEEN_MINUTE") => "15m",
        Some("ONE_HOUR") => "1h",
        Some("ONE_DAY") => "1d",
        Some(value @ ("1m" | "5m" | "15m" | "1h" | "1d")) => value,
        _ => "5m",
    }
}

/// 从 Coinbase level2 的混合 updates 列表中筛出一侧档位，`bids` 为真取买盘，为假取卖盘。
/// 买方接受 bid 与 buy，卖方接受 offer、ask 与 sell，比较忽略大小写；缺少 `side` 的条目按空串处理必然落空。
/// 因此方向拼写异常的档位会被静默丢弃而不是报错，盘口只会变薄，不会混入方向错误的价位。
/// 价格取 `price_level` 或 `price`，数量取 `new_quantity`、`quantity` 或 `size`，两者必填。
/// 数量取的是更新后的绝对值而非增量，数量为 0 表示该价位被撤单，这里照样保留，由消费端决定如何处理。
/// 只要有一个入选档位的价格或数量无法解析，整次调用就返回校验错误，不做部分成功。
fn coinbase_depth_levels(updates: &[Value], bids: bool) -> AppResult<Vec<MarketDepthLevel>> {
    updates
        .iter()
        .filter_map(Value::as_object)
        .filter(|item| {
            let side = item.get("side").and_then(Value::as_str).unwrap_or_default();
            if bids {
                side.eq_ignore_ascii_case("bid") || side.eq_ignore_ascii_case("buy")
            } else {
                side.eq_ignore_ascii_case("offer")
                    || side.eq_ignore_ascii_case("ask")
                    || side.eq_ignore_ascii_case("sell")
            }
        })
        .map(|item| {
            Ok(MarketDepthLevel::new(
                decimal_field(item, &["price_level", "price"])?,
                decimal_field(item, &["new_quantity", "quantity", "size"])?,
            ))
        })
        .collect()
}

/// 按三级优先级取 Coinbase 记录的观察时间：记录内的 RFC3339 `time`/`event_time`、帧级 `timestamp`、毫秒字段。
/// 前两级都要求 RFC3339 文本，解析失败会被当作缺失继续向下回退，而不是立刻报错。
/// 只有走到最后一级仍取不到合法毫秒时间时才返回校验错误，因此错误信息指向的是毫秒字段而非最初的 RFC3339 字段。
/// 记录级时间优先于帧级，是为了在一帧包含多条记录时保留各自的真实时间。
fn coinbase_observed_at(
    value: &Value,
    item: &serde_json::Map<String, Value>,
) -> AppResult<DateTime<Utc>> {
    coinbase_optional_rfc3339_field(item, &["time", "event_time"])
        .or_else(|| value.get("timestamp").and_then(coinbase_rfc3339_value))
        .map(Ok)
        .unwrap_or_else(|| millis_field(item, &["ts", "time"]))
}

/// 按候选名顺序找出第一个存在的字段，并尝试按 RFC3339 解析成时间，失败与缺失都返回 `None`。
/// 注意只会尝试首个命中的字段，它若存在但格式非法，后面的候选名不会再被尝试。
fn coinbase_optional_rfc3339_field(
    item: &serde_json::Map<String, Value>,
    names: &[&str],
) -> Option<DateTime<Utc>> {
    names
        .iter()
        .find_map(|name| item.get(*name))
        .and_then(coinbase_rfc3339_value)
}

/// 把一个 JSON 值按 RFC3339 解析为 UTC 时间，解析后统一转换时区，消除原始文本里的偏移量差异。
/// 数字型取值会先转成字符串再尝试解析，实际上必然失败，因此本函数只对时间文本有效。
fn coinbase_rfc3339_value(value: &Value) -> Option<DateTime<Utc>> {
    value_as_string(value)
        .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())
        .map(|value| value.with_timezone(&Utc))
}

/// 读取 Coinbase 的百分比字段并解析为十进制数，取值前会剥掉结尾可能存在的百分号。
/// 字段缺失返回 `Ok(None)` 属于正常情况，但字段存在却不是字符串或数字、或剥号后无法解析时返回校验错误。
/// 剥号只处理结尾，所以形如 `1.5%%` 之类的异常文本仍会解析失败，这是刻意不做过度容错。
/// 返回值已经是百分数本身，调用方直接使用，不需要再乘以 100。
fn coinbase_optional_percent_field(
    item: &serde_json::Map<String, Value>,
    names: &[&str],
) -> AppResult<Option<BigDecimal>> {
    names
        .iter()
        .find_map(|name| item.get(*name))
        .map(|value| {
            let value = value_as_string(value).ok_or_else(|| {
                AppError::Validation("coinbase percent value is required".to_owned())
            })?;
            BigDecimal::from_str(value.trim_end_matches('%')).map_err(|error| {
                AppError::Validation(format!("coinbase percent value is invalid: {error}"))
            })
        })
        .transpose()
}

/// 取 REST 响应顶层的 `ts` 作为包装后帧的观察时间，缺失或类型不符时退回本机当前毫秒。
/// 退回本机时间意味着这批数据在防倒退比较中总会显得最新，能覆盖缓存旧值，这正是兜底路径需要的行为，
/// 但也说明 REST 兜底得到的观察时间不能用来衡量行情实际延迟。
fn rest_payload_observed_millis(value: &Value) -> i64 {
    value
        .get("ts")
        .and_then(value_as_i64)
        .unwrap_or_else(|| Utc::now().timestamp_millis())
}

/// 把原始行情文本解析成 JSON 值，语法错误一律转成携带底层原因的校验错误。
/// 归为校验错误而非内部错误，是因为损坏的载荷来自外部行情源，重试同一份文本不会有不同结果。
fn parse_json(payload: &str) -> AppResult<Value> {
    serde_json::from_str(payload)
        .map_err(|error| AppError::Validation(format!("invalid market payload json: {error}")))
}

/// 取 `data` 数组的首个对象元素，这是 Bitget 各频道推送的统一取值方式。
/// 数组为空、字段缺失或首元素不是对象都返回同一条校验错误，因此错误文本无法区分具体是哪种情况。
/// 只取首条意味着同帧内的其余条目被丢弃，K 线因为元素是数组而不能走这个入口。
fn first_data_object(value: &Value) -> AppResult<&serde_json::Map<String, Value>> {
    value
        .get("data")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(Value::as_object)
        .ok_or_else(|| AppError::Validation("market data item is required".to_owned()))
}

/// 断言某个可选 JSON 值必须是对象并借出其内容，缺失与类型不符都返回以 `name` 命名的校验错误。
/// `name` 只影响错误文本，调用方应传入面向排查的名称，例如 `htx tick`，以便日志能定位到具体字段。
fn required_object<'a>(
    value: Option<&'a Value>,
    name: &str,
) -> AppResult<&'a serde_json::Map<String, Value>> {
    value
        .and_then(Value::as_object)
        .ok_or_else(|| AppError::Validation(format!("{name} is required")))
}

/// 断言某个可选 JSON 值必须是数组并借出其内容，主要用于 REST 兜底响应的行集合与盘口档位。
/// 空数组视为合法并原样返回，调用方需要自行判断是否可以接受零条记录。
fn required_array<'a>(value: Option<&'a Value>, name: &str) -> AppResult<&'a Vec<Value>> {
    value
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::Validation(format!("{name} is required")))
}

/// 取 Bitget 帧的交易对：优先用数据条目自带的 `instId`，缺失时回退到帧级 `arg.instId`。
/// 优先条目内字段是因为它与该条数据严格对应，而帧级 `arg` 只反映订阅参数。
/// 两处都没有则返回校验错误，绝不从频道名或请求上下文猜测交易对。
fn bitget_symbol<'a>(
    value: &'a Value,
    item: &'a serde_json::Map<String, Value>,
) -> AppResult<&'a str> {
    item.get("instId")
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("arg")
                .and_then(|arg| arg.get("instId"))
                .and_then(Value::as_str)
        })
        .ok_or_else(|| AppError::Validation("bitget instId is required".to_owned()))
}

/// 从 HTX 频道名中切出交易对：按点号分割 `market.{symbol}.detail` 这类主题并取第二段。
/// HTX 的载荷体里没有交易对字段，频道名是唯一来源，因此频道格式变化会直接导致整条链路解析失败。
/// 返回的是 HTX 原始小写写法，规范化成平台大写形式由后续领域构造器完成。
fn htx_symbol(value: &Value) -> AppResult<&str> {
    value
        .get("ch")
        .and_then(Value::as_str)
        .and_then(|channel| channel.split('.').nth(1))
        .ok_or_else(|| AppError::Validation("htx channel symbol is required".to_owned()))
}

/// 把 Bitget 的 candle 频道名反解成平台周期，先去掉 `candle` 前缀再逐一匹配白名单。
/// 大小时与日线同时接受 `1H`/`1h` 与 `1D`/`1d`，因为订阅用大写而部分响应会回显小写。
/// 未列入白名单的周期返回校验错误而不是透传，避免把无法写入缓存的周期带进领域层。
/// 平台支持的 `4h` 不在此表中，意味着当前不接受来自 Bitget 的四小时线。
fn bitget_interval(channel: &str) -> AppResult<&str> {
    match channel.strip_prefix("candle").unwrap_or(channel) {
        "1m" => Ok("1m"),
        "5m" => Ok("5m"),
        "15m" => Ok("15m"),
        "1H" | "1h" => Ok("1h"),
        "1D" | "1d" => Ok("1d"),
        _ => Err(AppError::Validation(
            "bitget kline interval is invalid".to_owned(),
        )),
    }
}

/// 把 HTX K 线频道名反解成平台周期，取最后一个点号之后的片段再匹配白名单。
/// 小时线同时接受 `60min` 与 `1hour` 两种历史写法，其余按分钟或日粒度一一对应。
/// 未识别的片段返回校验错误；与 Bitget 一样，这里也不支持四小时线。
fn htx_interval(channel: &str) -> AppResult<&str> {
    match channel.rsplit('.').next().unwrap_or_default() {
        "1min" => Ok("1m"),
        "5min" => Ok("5m"),
        "15min" => Ok("15m"),
        "60min" | "1hour" => Ok("1h"),
        "1day" => Ok("1d"),
        _ => Err(AppError::Validation(
            "htx kline interval is invalid".to_owned(),
        )),
    }
}

/// 将领域行情提供方映射为日志、指标和事件载荷使用的稳定小写代码。
/// 该代码同时写入 Mongo K 线文档的 `source` 字段并构成事件幂等键的一段，因此取值一旦发布就不能再改，
/// 否则同一根蜡烛会因为幂等键变化而被当成新事件重复投递。
/// 与配置侧的 provider 代码不同，这里多出内部策略行情对应的 `strategy`。
pub(super) fn provider_name(provider: MarketDataProvider) -> &'static str {
    match provider {
        MarketDataProvider::Bitget => "bitget",
        MarketDataProvider::Htx => "htx",
        MarketDataProvider::Strategy => "strategy",
        MarketDataProvider::Coinbase => "coinbase",
    }
}

/// 对规范化 JSON 文本计算稳定 FNV-1a 摘要，用于失败上下文和去重标识而非密码学签名。
/// 字段顺序由 `serde_json::Value` 序列化结果决定；不得把该 64 位摘要当作安全认证凭据。
pub(super) fn market_feed_payload_hash(payload: &Value) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in payload.to_string().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

/// 解析 Bitget 与 HTX 共用的盘口档位格式：外层是数组，每档又是 `[价格, 数量]` 形式的二元数组。
/// 严格按位置取值，索引 0 是价格、索引 1 是数量，多余元素被忽略，两家的成交笔数等附加位因此不会进入领域模型。
/// 外层不是数组、某档不是数组、或价格数量无法解析成十进制数时，整次调用返回校验错误，不做部分成功。
/// 档位顺序原样保留，不排序也不过滤零量档位，最优价的判断留给消费端。
fn levels(value: Option<&Value>) -> AppResult<Vec<MarketDepthLevel>> {
    value
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::Validation("depth levels are required".to_owned()))?
        .iter()
        .map(|level| {
            let values = level
                .as_array()
                .ok_or_else(|| AppError::Validation("depth level must be an array".to_owned()))?;
            Ok(MarketDepthLevel::new(
                decimal_value(values.first())?,
                decimal_value(values.get(1))?,
            ))
        })
        .collect()
}

/// 按候选名顺序取出首个存在的字段并转成字符串，数字型取值会被转成其十进制文本。
/// 这层兼容让成交号既能接受字符串 ID 也能接受数字 ID，无需在各适配器里分别判断类型。
/// 所有候选名都不存在，或首个命中的字段既非字符串也非数字时，返回以首个候选名命名的校验错误。
fn string_field(item: &serde_json::Map<String, Value>, names: &[&str]) -> AppResult<String> {
    names
        .iter()
        .find_map(|name| item.get(*name))
        .and_then(value_as_string)
        .ok_or_else(|| AppError::Validation(format!("market field {} is required", names[0])))
}

/// 按候选名顺序取出首个存在的字段并解析成 `BigDecimal`，用于价格、数量这类必填数值。
/// 只尝试首个命中的字段，它若解析失败就直接报错，不会继续尝试后面的候选名。
/// 数值一律经十进制文本解析，不走浮点，避免价格在解析阶段就损失精度。
/// 字段全部缺失时返回以首个候选名命名的校验错误；这里绝不用 0 兜底，以免把缺数据伪装成零价。
fn decimal_field(item: &serde_json::Map<String, Value>, names: &[&str]) -> AppResult<BigDecimal> {
    names
        .iter()
        .find_map(|name| item.get(*name))
        .map(|value| decimal_value(Some(value)))
        .transpose()?
        .ok_or_else(|| AppError::Validation(format!("market decimal {} is required", names[0])))
}

/// 与必填版本的区别只在于字段全部缺失时返回 `Ok(None)` 而不是报错，用于高低价、开盘价等可选统计。
/// 字段一旦存在就必须能解析成十进制数，格式非法仍返回校验错误，不会退化成 `None`。
/// 调用方拿到 `None` 后应按各自的回填规则处理，例如用最新价补齐高低价。
fn optional_decimal_field(
    item: &serde_json::Map<String, Value>,
    names: &[&str],
) -> AppResult<Option<BigDecimal>> {
    names
        .iter()
        .find_map(|name| item.get(*name))
        .map(|value| decimal_value(Some(value)))
        .transpose()
}

/// 把各 provider 给出的零散 24 小时字段补齐成完整的 ticker 统计，是 Bitget 与 HTX 共用的换算入口。
/// 高低价缺失时回填为最新价，这样区间会退化成一个点，而不是出现 0 这种会误导展示的值。
/// 涨跌额等于最新价减去开盘价，开盘价缺失时记为 0，表示信息不足而非真实持平。
/// 涨跌幅优先采用 provider 直接给出的比率并乘以 100 换算成百分数；只有没有比率时才用涨跌额除以开盘价推算。
/// 推算路径对开盘价为 0 的情况显式返回 0，避免除零；这两条路径的精度口径不同，同一交易对切换来源时会有细微差异。
/// 本函数不校验最新价是否落在高低区间内，也不修正负成交量，只做缺失字段的补齐与单位换算。
fn ticker_24h_values(
    last_price: BigDecimal,
    high_24h: Option<BigDecimal>,
    low_24h: Option<BigDecimal>,
    volume_24h: BigDecimal,
    open_24h: Option<BigDecimal>,
    change_ratio_24h: Option<BigDecimal>,
) -> MarketTickerValues {
    let high_24h = high_24h.unwrap_or_else(|| last_price.clone());
    let low_24h = low_24h.unwrap_or_else(|| last_price.clone());
    let price_change_24h = open_24h
        .as_ref()
        .map(|open| last_price.clone() - open.clone())
        .unwrap_or_else(|| BigDecimal::from(0));
    let price_change_percent_24h = change_ratio_24h
        .map(|change| change * BigDecimal::from(100))
        .unwrap_or_else(|| {
            let Some(open) = open_24h else {
                return BigDecimal::from(0);
            };
            if open == 0 {
                return BigDecimal::from(0);
            }
            price_change_24h.clone() / open * BigDecimal::from(100)
        });

    MarketTickerValues::new(
        last_price,
        high_24h,
        low_24h,
        volume_24h,
        price_change_24h,
        price_change_percent_24h,
    )
}

/// 按候选名取出首个存在的字段并按毫秒时间戳解析成 UTC 时间，用于三家推送的帧级与条目级时间。
/// 取值兼容数字与数字字符串两种形式，因为各家在不同频道里对时间戳的 JSON 类型并不一致。
/// 字段全部缺失时返回以首个候选名命名的校验错误，绝不用本机时间冒充 provider 观察时间。
fn millis_field(item: &serde_json::Map<String, Value>, names: &[&str]) -> AppResult<DateTime<Utc>> {
    names
        .iter()
        .find_map(|name| item.get(*name))
        .map(|value| millis_value(Some(value)))
        .transpose()?
        .ok_or_else(|| AppError::Validation(format!("market timestamp {} is required", names[0])))
}

/// 在整帧顶层按毫秒解析时间字段，先断言载荷本身是 JSON 对象再复用条目级取值逻辑。
/// 各解析器常用它与条目级取值组成回退链，因此调用方通常会忽略这里的错误并继续尝试另一处时间来源。
fn millis_field_from_value(value: &Value, names: &[&str]) -> AppResult<DateTime<Utc>> {
    let item = value
        .as_object()
        .ok_or_else(|| AppError::Validation("market payload object is required".to_owned()))?;
    millis_field(item, names)
}

/// 按候选名取出首个存在的字段并按秒级时间戳解析，专用于 HTX 的 `tick.id` 和 Coinbase 的 `start`。
/// 与毫秒版本共用数字或数字字符串的兼容取值，但单位不同，误用会让时间偏差达到千倍量级。
/// 纳秒部分固定填 0，因此秒级来源的开盘时间天然对齐到整秒。
/// 字段缺失或超出可表示范围时统一返回校验错误，不做任何近似或截断。
fn seconds_field(
    item: &serde_json::Map<String, Value>,
    names: &[&str],
) -> AppResult<DateTime<Utc>> {
    names
        .iter()
        .find_map(|name| item.get(*name))
        .and_then(value_as_i64)
        .and_then(|seconds| DateTime::<Utc>::from_timestamp(seconds, 0))
        .ok_or_else(|| AppError::Validation(format!("market timestamp {} is invalid", names[0])))
}

/// 把单个 JSON 值解析成 `BigDecimal`，是本文件所有金额与数量解析的最终落点。
/// 先统一转成字符串再按十进制解析，因此数字与字符串两种 JSON 表示得到完全一致的结果，且全程不经过浮点。
/// 布尔、数组等其他类型按缺值处理，与真正缺失字段共用同一条错误信息。
/// 解析失败时错误里会带上底层原因，便于定位是空串、多余符号还是超长小数位。
fn decimal_value(value: Option<&Value>) -> AppResult<BigDecimal> {
    value
        .and_then(value_as_string)
        .ok_or_else(|| AppError::Validation("market decimal value is required".to_owned()))
        .and_then(|value| {
            BigDecimal::from_str(&value).map_err(|error| {
                AppError::Validation(format!("market decimal is invalid: {error}"))
            })
        })
}

/// 把单个 JSON 值按毫秒时间戳解析成 UTC 时间，是各处毫秒取值的最终落点。
/// 数字与数字字符串都接受，超出可表示范围的毫秒数会被判为非法而不是回绕成错误时间。
/// K 线解析直接用它读取 Bitget 数组首位的开盘毫秒，那里没有字段名可用，只能按位置取值。
fn millis_value(value: Option<&Value>) -> AppResult<DateTime<Utc>> {
    value
        .and_then(value_as_i64)
        .and_then(DateTime::<Utc>::from_timestamp_millis)
        .ok_or_else(|| AppError::Validation("market timestamp millis is invalid".to_owned()))
}

/// 把字符串或数字型 JSON 值统一取成字符串，其余类型返回 `None`。
/// 数字走 `serde_json` 自身的文本表示，保留原始字面精度，不会因为中转 `f64` 而丢位。
/// 这是各家在同一字段上混用字符串与数字时的兼容层，也是十进制解析前的必经一步。
fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

/// 把 JSON 值取成 `i64`，数字要求本身可无损表示为整数，字符串则按十进制整数解析。
/// 带小数点的数字与文本都会取不到值，因此时间戳字段一旦被写成浮点形式就会被判为缺失。
/// 时间戳解析与 REST 响应的 `ts` 提取都依赖它，它是本文件唯一的整数取值入口。
fn value_as_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(value) => value.parse::<i64>().ok(),
        _ => None,
    }
}

/// 把各家的成交方向写法归一成领域枚举：buy 与 bid 视为买入，sell 与 ask 视为卖出。
/// 比较忽略大小写，覆盖 Bitget 的 `side`/`direction`、HTX 的 `direction` 与 Coinbase 的 `side`。
/// 无法识别的取值返回校验错误而不是默认成买入，因为方向错误会直接把成交流的多空判断带偏。
fn trade_side(value: &str) -> AppResult<MarketTradeSide> {
    if value.eq_ignore_ascii_case("buy") || value.eq_ignore_ascii_case("bid") {
        Ok(MarketTradeSide::Buy)
    } else if value.eq_ignore_ascii_case("sell") || value.eq_ignore_ascii_case("ask") {
        Ok(MarketTradeSide::Sell)
    } else {
        Err(AppError::Validation(
            "market trade side is invalid".to_owned(),
        ))
    }
}

/// 将领域/解析错误统一映射为对外行情校验错误，保留原错误文本供调用链定位具体字段。
/// 归为校验类而非内部故障，是因为这类错误来自外部行情源的载荷本身，重投同一帧不会有不同结果。
/// 转换过程只保留错误文本，原始错误类型会丢失，因此调用方无法再按错误种类分支处理。
pub(super) fn validation_error(error: impl ToString) -> AppError {
    AppError::Validation(error.to_string())
}
