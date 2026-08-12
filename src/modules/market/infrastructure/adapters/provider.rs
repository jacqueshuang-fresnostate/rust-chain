//! 第三方行情 provider 适配基础设施。
//!
//! 将 Bitget、HTX 与 Coinbase 的订阅、REST 兜底响应和 WebSocket payload 归一化为领域快照；
//! 缺失或非法的交易对、价格、周期与时间戳直接报错，不写入权威缓存。

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

    /// 返回代码。
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Bitget => "bitget",
            Self::Htx => "htx",
            Self::Coinbase => "coinbase",
        }
    }

    /// 返回该 provider 可接受的后台配置别名，用于兼容 HTX/Huobi 与 Coinbase 历史代码。
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
    pub const fn default_providers() -> [Self; 2] {
        [Self::Bitget, Self::Htx]
    }

    /// 返回当前构建支持的全部 provider，供后台校验与运行时配置展开。
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

    fn feed_url(&self, settings: &Settings) -> String {
        match self {
            Self::Bitget => settings.bitget_ws_url.clone(),
            Self::Htx => settings.htx_ws_url.clone(),
            Self::Coinbase => settings.coinbase_ws_url.clone(),
        }
    }

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

    fn subscription_messages(&self, symbols: &[String], intervals: &[String]) -> Vec<String> {
        match self {
            Self::Bitget => bitget_subscriptions(symbols, intervals),
            Self::Htx => htx_subscriptions(symbols, intervals),
            Self::Coinbase => coinbase_subscriptions(symbols, intervals),
        }
    }
}

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

fn bitget_subscription_interval(interval: &str) -> &str {
    match interval {
        "1h" => "1H",
        "1d" => "1D",
        value => value,
    }
}

fn bitget_rest_interval(interval: &str) -> &str {
    match interval {
        "1m" => "1min",
        "5m" => "5min",
        "15m" => "15min",
        "1d" => "1day",
        value => value,
    }
}

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
    /// 顶层 `ts` 缺失时以开盘时间作为观察时间，其他必填字段非法则拒绝该帧。
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
    /// 未识别粒度按现有兼容合同映射为 `5m`，观察时间缺失时回退到开盘时间。
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
/// `product_id` 缺失时从请求 symbol 推导，事件时间取包装时本机时间；非对象 JSON 返回校验错误。
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

fn coinbase_first_event_object(value: &Value) -> AppResult<&serde_json::Map<String, Value>> {
    value
        .get("events")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(Value::as_object)
        .ok_or_else(|| AppError::Validation("coinbase event is required".to_owned()))
}

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

fn coinbase_symbol_from_item(item: &serde_json::Map<String, Value>) -> AppResult<String> {
    item.get("product_id")
        .and_then(Value::as_str)
        .map(coinbase_symbol_from_product_id)
        .ok_or_else(|| AppError::Validation("coinbase product_id is required".to_owned()))
}

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

fn coinbase_symbol_from_product_id(product_id: &str) -> String {
    product_id.replace('-', "").to_ascii_uppercase()
}

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

fn coinbase_observed_at(
    value: &Value,
    item: &serde_json::Map<String, Value>,
) -> AppResult<DateTime<Utc>> {
    coinbase_optional_rfc3339_field(item, &["time", "event_time"])
        .or_else(|| value.get("timestamp").and_then(coinbase_rfc3339_value))
        .map(Ok)
        .unwrap_or_else(|| millis_field(item, &["ts", "time"]))
}

fn coinbase_optional_rfc3339_field(
    item: &serde_json::Map<String, Value>,
    names: &[&str],
) -> Option<DateTime<Utc>> {
    names
        .iter()
        .find_map(|name| item.get(*name))
        .and_then(coinbase_rfc3339_value)
}

fn coinbase_rfc3339_value(value: &Value) -> Option<DateTime<Utc>> {
    value_as_string(value)
        .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())
        .map(|value| value.with_timezone(&Utc))
}

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

fn rest_payload_observed_millis(value: &Value) -> i64 {
    value
        .get("ts")
        .and_then(value_as_i64)
        .unwrap_or_else(|| Utc::now().timestamp_millis())
}

fn parse_json(payload: &str) -> AppResult<Value> {
    serde_json::from_str(payload)
        .map_err(|error| AppError::Validation(format!("invalid market payload json: {error}")))
}

fn first_data_object(value: &Value) -> AppResult<&serde_json::Map<String, Value>> {
    value
        .get("data")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(Value::as_object)
        .ok_or_else(|| AppError::Validation("market data item is required".to_owned()))
}

fn required_object<'a>(
    value: Option<&'a Value>,
    name: &str,
) -> AppResult<&'a serde_json::Map<String, Value>> {
    value
        .and_then(Value::as_object)
        .ok_or_else(|| AppError::Validation(format!("{name} is required")))
}

fn required_array<'a>(value: Option<&'a Value>, name: &str) -> AppResult<&'a Vec<Value>> {
    value
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::Validation(format!("{name} is required")))
}

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

fn htx_symbol(value: &Value) -> AppResult<&str> {
    value
        .get("ch")
        .and_then(Value::as_str)
        .and_then(|channel| channel.split('.').nth(1))
        .ok_or_else(|| AppError::Validation("htx channel symbol is required".to_owned()))
}

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

fn string_field(item: &serde_json::Map<String, Value>, names: &[&str]) -> AppResult<String> {
    names
        .iter()
        .find_map(|name| item.get(*name))
        .and_then(value_as_string)
        .ok_or_else(|| AppError::Validation(format!("market field {} is required", names[0])))
}

fn decimal_field(item: &serde_json::Map<String, Value>, names: &[&str]) -> AppResult<BigDecimal> {
    names
        .iter()
        .find_map(|name| item.get(*name))
        .map(|value| decimal_value(Some(value)))
        .transpose()?
        .ok_or_else(|| AppError::Validation(format!("market decimal {} is required", names[0])))
}

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

fn millis_field(item: &serde_json::Map<String, Value>, names: &[&str]) -> AppResult<DateTime<Utc>> {
    names
        .iter()
        .find_map(|name| item.get(*name))
        .map(|value| millis_value(Some(value)))
        .transpose()?
        .ok_or_else(|| AppError::Validation(format!("market timestamp {} is required", names[0])))
}

fn millis_field_from_value(value: &Value, names: &[&str]) -> AppResult<DateTime<Utc>> {
    let item = value
        .as_object()
        .ok_or_else(|| AppError::Validation("market payload object is required".to_owned()))?;
    millis_field(item, names)
}

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

fn millis_value(value: Option<&Value>) -> AppResult<DateTime<Utc>> {
    value
        .and_then(value_as_i64)
        .and_then(DateTime::<Utc>::from_timestamp_millis)
        .ok_or_else(|| AppError::Validation("market timestamp millis is invalid".to_owned()))
}

fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn value_as_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(value) => value.parse::<i64>().ok(),
        _ => None,
    }
}

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
pub(super) fn validation_error(error: impl ToString) -> AppError {
    AppError::Validation(error.to_string())
}
