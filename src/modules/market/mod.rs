pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod repository;
pub mod service;
pub mod synthetic;

pub mod routes;

pub use domain::{
    KlineKeyError, KlineQuery, KlineUpsertKey, MarketCacheEntryError, MarketDataProvider,
    MarketDepthLevel, MarketDepthSnapshot, MarketEvent, MarketEventType, MarketKlineSnapshot,
    MarketKlineValues, MarketSymbolError, MarketTickerSnapshot, MarketTickerValues,
    MarketTradeSide, MarketTradeTick, ValidatedMarketSymbol, sanitize_symbol,
};
pub use infrastructure::{
    MarketCacheError, MarketCacheWriteOutcome, MarketDepthCacheEntry, MarketKlineCacheEntry,
    MarketTickerCacheEntry, RedisMarketCache, kline_collection_name, market_depth_redis_key,
    market_kline_redis_key, market_ticker_redis_key,
};
pub use synthetic::{
    SyntheticAggregateCandle, SyntheticCandle, SyntheticExecutionMode, SyntheticKlineInterval,
    SyntheticMarketConfig, SyntheticMarketError, SyntheticMarketNode, SyntheticTargetType,
    aggregate_1m_candles,
};

pub use infrastructure::adapters;

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_market_mod_tests.rs"]
mod tests;
