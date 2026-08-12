//! market bounded context infrastructure compatibility façade.
//!
//! 基础设施实现按职责拆分：缓存模块维护权威行情快照，持久化模块读取 MySQL/Mongo，
//! `adapters` 负责第三方 provider 归一化与 ingestion。此文件仅保留稳定路径与可见性。

mod cache;
mod persistence;

pub mod adapters;

pub use cache::{
    MarketCacheError, MarketCacheWriteOutcome, MarketDepthCacheEntry, MarketKlineCacheEntry,
    MarketTickerCacheEntry, RedisMarketCache, market_depth_redis_key, market_kline_redis_key,
    market_ticker_redis_key,
};
pub use persistence::kline_collection_name;
pub(crate) use persistence::{
    add_user_market_favorite, list_active_markets, list_klines, list_recent_trades,
    list_user_market_favorites, load_cached_depth, load_cached_ticker, market_symbol_is_listed,
    remove_user_market_favorite,
};
