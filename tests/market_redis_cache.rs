use bigdecimal::BigDecimal;
use chrono::{TimeZone, Utc};
use exchange_api::modules::market::{
    MarketDepthCacheEntry, MarketDepthLevel, MarketKlineCacheEntry, MarketKlineValues,
    MarketTickerCacheEntry, RedisMarketCache,
};
use redis::AsyncCommands;
use std::{error::Error, str::FromStr};
use uuid::Uuid;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

fn env_or_skip(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => {
            eprintln!("skipping integration test because {name} is not set");
            None
        }
    }
}

#[tokio::test]
async fn redis_market_cache_stores_ticker_depth_and_kline_json() -> Result<(), Box<dyn Error>> {
    let Some(redis_url) = env_or_skip("REDIS_URL") else {
        return Ok(());
    };
    let client = redis::Client::open(redis_url)?;
    let manager = redis::aio::ConnectionManager::new(client).await?;
    let cache = RedisMarketCache::new(manager.clone());
    let observed_at = Utc.with_ymd_and_hms(2026, 5, 26, 16, 20, 0).unwrap();
    let open_time = Utc.with_ymd_and_hms(2026, 5, 26, 16, 20, 0).unwrap();
    let uuid = Uuid::now_v7().simple().to_string();
    let symbol = format!("TEST{}USDT", &uuid[16..32]);

    let ticker = MarketTickerCacheEntry::new(
        &symbol,
        decimal("70000.120000000000000000"),
        decimal("125.500000000000000000"),
        observed_at,
    )?;
    let depth = MarketDepthCacheEntry::new(
        &symbol,
        vec![MarketDepthLevel::new(decimal("70000.00"), decimal("0.50"))],
        vec![MarketDepthLevel::new(decimal("70001.00"), decimal("0.40"))],
        observed_at,
    )?;
    let kline = MarketKlineCacheEntry::new(
        &symbol,
        "1m",
        open_time,
        MarketKlineValues {
            open: decimal("70000.00"),
            high: decimal("70010.00"),
            low: decimal("69990.00"),
            close: decimal("70005.00"),
            volume: decimal("12.30"),
        },
    )?;

    cache.save_ticker(ticker.clone()).await?;
    cache.save_depth(depth.clone()).await?;
    cache.save_kline(kline.clone()).await?;

    let mut raw_connection = manager.clone();
    let ticker_payload: String = raw_connection.get(ticker.redis_key()).await?;
    let depth_payload: String = raw_connection.get(depth.redis_key()).await?;
    let kline_payload: String = raw_connection.get(kline.redis_key()).await?;
    let ticker_json: serde_json::Value = serde_json::from_str(&ticker_payload)?;
    let depth_json: serde_json::Value = serde_json::from_str(&depth_payload)?;
    let kline_json: serde_json::Value = serde_json::from_str(&kline_payload)?;

    let normalized_symbol = symbol.to_ascii_uppercase();

    assert_eq!(ticker_json["symbol"], normalized_symbol);
    assert_eq!(
        ticker_json["redis_key"],
        format!("market:ticker:{normalized_symbol}")
    );
    assert!(ticker_json["last_price"].is_string());
    assert!(ticker_json["volume_24h"].is_string());
    assert_eq!(depth_json["symbol"], normalized_symbol);
    assert_eq!(
        depth_json["redis_key"],
        format!("market:depth:{normalized_symbol}")
    );
    assert!(depth_json["bids"][0]["price"].is_string());
    assert!(depth_json["asks"][0]["quantity"].is_string());
    assert_eq!(kline_json["symbol"], normalized_symbol);
    assert_eq!(kline_json["interval"], "1m");
    assert_eq!(
        kline_json["redis_key"],
        format!("market:kline:{normalized_symbol}:1m")
    );
    assert!(kline_json["close"].is_string());

    let _: usize = raw_connection
        .del(&[ticker.redis_key(), depth.redis_key(), kline.redis_key()])
        .await?;
    Ok(())
}
