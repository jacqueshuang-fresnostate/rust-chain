//! market bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。
//! 对外数值一律以十进制字符串输出，时间统一序列化为毫秒时间戳，避免 JSON 浮点丢失精度。
//! `DepthCachePayload` 一类结构是 Redis 缓存的反序列化形态，`*Response` 才是对外契约，
//! 两者只能通过本文件的转换函数衔接。本层不访问数据库或缓存，也不做上架、新鲜度与精度校验。

use crate::{
    modules::market::{ValidatedMarketSymbol, repository::KlineDocumentRecord},
    time::{option_unix_millis, unix_millis},
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub(crate) struct MarketsResponse {
    pub(crate) markets: Vec<MarketResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct MarketResponse {
    pub(crate) id: u64,
    pub(crate) symbol: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) base_logo_url: Option<String>,
    pub(crate) quote_logo_url: Option<String>,
    pub(crate) base_asset: String,
    pub(crate) quote_asset: String,
    pub(crate) price_precision: i32,
    pub(crate) qty_precision: i32,
    pub(crate) min_order_value: String,
    pub(crate) status: String,
    pub(crate) market_type: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct MarketFavoritesResponse {
    pub(crate) favorites: Vec<MarketFavoriteResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct MarketFavoriteResponse {
    pub(crate) market_id: u64,
    pub(crate) symbol: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) base_logo_url: Option<String>,
    pub(crate) quote_logo_url: Option<String>,
    pub(crate) base_asset: String,
    pub(crate) quote_asset: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct MarketFavoriteMutationResponse {
    pub(crate) favorite: MarketFavoriteResponse,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct TickerResponse {
    pub(crate) symbol: String,
    pub(crate) last_price: String,
    pub(crate) high_24h: Option<String>,
    pub(crate) low_24h: Option<String>,
    pub(crate) volume_24h: String,
    pub(crate) price_change_24h: Option<String>,
    pub(crate) price_change_percent_24h: Option<String>,
    #[serde(with = "unix_millis")]
    pub(crate) observed_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct KlineQueryParams {
    pub(crate) interval: String,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) start: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) end: Option<DateTime<Utc>>,
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Serialize)]
pub(crate) struct KlineResponse {
    pub(crate) symbol: String,
    pub(crate) interval: String,
    #[serde(with = "unix_millis")]
    pub(crate) open_time: DateTime<Utc>,
    pub(crate) open: String,
    pub(crate) high: String,
    pub(crate) low: String,
    pub(crate) close: String,
    pub(crate) volume: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TradesQueryParams {
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DepthResponse {
    pub(crate) symbol: String,
    pub(crate) bids: Vec<DepthLevelResponse>,
    pub(crate) asks: Vec<DepthLevelResponse>,
    #[serde(with = "unix_millis")]
    pub(crate) observed_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DepthLevelResponse {
    pub(crate) price: String,
    pub(crate) amount: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DepthCachePayload {
    pub(crate) symbol: String,
    pub(crate) bids: Vec<DepthCacheLevel>,
    pub(crate) asks: Vec<DepthCacheLevel>,
    #[serde(with = "unix_millis")]
    pub(crate) observed_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DepthCacheLevel {
    pub(crate) price: BigDecimal,
    pub(crate) quantity: BigDecimal,
}

#[derive(Debug, Serialize)]
pub(crate) struct TradesResponse {
    pub(crate) trades: Vec<TradeResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct TradeResponse {
    pub(crate) id: String,
    pub(crate) symbol: String,
    pub(crate) price: String,
    pub(crate) amount: String,
    pub(crate) direction: String,
    #[serde(with = "unix_millis")]
    pub(crate) time: DateTime<Utc>,
}

impl DepthResponse {
    /// 把 Redis 中反序列化出的盘口载荷转为公开响应，买卖两侧逐档转换为字符串价量。
    /// 档位顺序原样保留，不排序、不合并同价档、不截断档数，`observed_at` 沿用缓存里的观察时间。
    /// 本转换不判断数据有效性，空档位、零数量或买卖倒挂都会照原样透出，新鲜度由调用方自行判断。
    pub(crate) fn from_cache(depth: DepthCachePayload) -> Self {
        Self {
            symbol: depth.symbol,
            bids: depth
                .bids
                .into_iter()
                .map(DepthLevelResponse::from)
                .collect(),
            asks: depth
                .asks
                .into_iter()
                .map(DepthLevelResponse::from)
                .collect(),
            observed_at: depth.observed_at,
        }
    }
}

impl From<DepthCacheLevel> for DepthLevelResponse {
    /// 把缓存中的单档盘口转为公开档位，缓存字段 `quantity` 在响应契约里对应 `amount`。
    /// 两个数值都按十进制原样转成字符串，不做四舍五入或单位换算，精度取决于摄取时写入缓存的值。
    fn from(level: DepthCacheLevel) -> Self {
        Self {
            price: level.price.to_string(),
            amount: level.quantity.to_string(),
        }
    }
}

impl MarketResponse {
    /// 构造无数据库部署下的占位交易对条目：`id` 固定为 0，三个 Logo 字段全为空，状态恒为 active。
    /// 价格与数量精度都写死 8 位、最小下单额写死 1，只有交易对、基础资产、计价资产和市场类型由调用方给出。
    /// 这些字段不来自后台交易对配置，仅供公开列表展示，不能作为下单精度、最小金额或风控参数的依据。
    pub(crate) fn fallback(
        symbol: &str,
        base_asset: &str,
        quote_asset: &str,
        market_type: &str,
    ) -> Self {
        Self {
            id: 0,
            symbol: symbol.to_owned(),
            logo_url: None,
            base_logo_url: None,
            quote_logo_url: None,
            base_asset: base_asset.to_owned(),
            quote_asset: quote_asset.to_owned(),
            price_precision: 8,
            qty_precision: 8,
            min_order_value: "1".to_owned(),
            status: "active".to_owned(),
            market_type: market_type.to_owned(),
        }
    }
}

impl KlineResponse {
    /// 把一条 Mongo K 线文档映射为公开响应，交易对由调用方传入而不从文档字段读取。
    /// 开盘时间由 BSON 时间转成 UTC，OHLCV 沿用文档中的十进制字符串，不做精度归一或高低价关系校验。
    pub(crate) fn from_document(symbol: &str, document: KlineDocumentRecord) -> Self {
        Self {
            symbol: symbol.to_owned(),
            interval: document.interval,
            open_time: DateTime::<Utc>::from(document.open_time.to_system_time()),
            open: document.open,
            high: document.high,
            low: document.low,
            close: document.close,
            volume: document.volume,
        }
    }
}

impl TradeResponse {
    /// 把一条平台现货成交记录映射为公开成交项，成交 ID 转为字符串，价格与数量按十进制原样输出。
    /// 交易对先尝试规范化为大写无分隔符形式，规范化失败时退回数据库原值，不让单条脏数据拖垮整个响应。
    /// 方向字段恒为 BUY，说明该来源尚未区分主动买卖方向，前端不能据此展示真实的成交方向。
    pub(crate) fn from_record(row: crate::modules::market::repository::SpotTradeRecord) -> Self {
        Self {
            id: row.id.to_string(),
            symbol: ValidatedMarketSymbol::from_raw(&row.symbol)
                .map(|symbol| symbol.as_str().to_owned())
                .unwrap_or(row.symbol),
            price: row.price.to_string(),
            amount: row.quantity.to_string(),
            direction: "BUY".to_owned(),
            time: row.created_at,
        }
    }
}
