use bigdecimal::BigDecimal;
use chrono::{TimeZone, Utc};
use exchange_api::modules::market::{
    MarketCacheWriteOutcome, MarketDepthCacheEntry, MarketDepthLevel, MarketKlineCacheEntry,
    MarketKlineValues, MarketTickerCacheEntry, RedisMarketCache,
};
use redis::AsyncCommands;
use std::{error::Error, str::FromStr, sync::Arc};
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
    assert!(ticker_json["high_24h"].is_string());
    assert!(ticker_json["low_24h"].is_string());
    assert!(ticker_json["volume_24h"].is_string());
    assert!(ticker_json["price_change_24h"].is_string());
    assert!(ticker_json["price_change_percent_24h"].is_string());
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

    let kline_sequence_key = format!("market:kline-sequence:{normalized_symbol}:1m");
    let cleanup_keys = vec![
        ticker.redis_key().to_owned(),
        depth.redis_key().to_owned(),
        kline.redis_key().to_owned(),
        kline_sequence_key,
    ];
    let _: usize = raw_connection.del(cleanup_keys).await?;
    Ok(())
}

#[tokio::test]
async fn redis_ticker_compare_and_set_replays_identical_and_rejects_conflicting_equal_observations()
-> Result<(), Box<dyn Error>> {
    let Some(redis_url) = env_or_skip("REDIS_URL") else {
        return Ok(());
    };
    let client = redis::Client::open(redis_url)?;
    let manager = redis::aio::ConnectionManager::new(client).await?;
    let cache = RedisMarketCache::new(manager.clone());
    let uuid = Uuid::now_v7().simple().to_string();
    let symbol = format!("TICKCAS{}USDT", &uuid[16..24]);
    let base = Utc.with_ymd_and_hms(2026, 8, 12, 17, 0, 0).unwrap();
    let newer = MarketTickerCacheEntry::new(
        &symbol,
        decimal("12.50"),
        decimal("100"),
        base + chrono::Duration::seconds(2),
    )?;
    let older = MarketTickerCacheEntry::new(
        &symbol,
        decimal("10.00"),
        decimal("90"),
        base + chrono::Duration::seconds(1),
    )?;
    let equal = MarketTickerCacheEntry::new(
        &symbol,
        decimal("9.00"),
        decimal("80"),
        base + chrono::Duration::seconds(2),
    )?;
    let key = newer.redis_key().to_owned();

    assert_eq!(
        cache
            .save_ticker_if_fresh(newer.clone())
            .await
            .map_err(|error| format!("newer ticker CAS failed: {error}"))?,
        MarketCacheWriteOutcome::Accepted
    );
    assert_eq!(
        cache
            .save_ticker_if_fresh(newer)
            .await
            .map_err(|error| format!("identical ticker replay failed: {error}"))?,
        MarketCacheWriteOutcome::ReplayedIdentical
    );
    assert_eq!(
        cache
            .save_ticker_if_fresh(older)
            .await
            .map_err(|error| format!("older ticker CAS failed: {error}"))?,
        MarketCacheWriteOutcome::RejectedStale
    );
    assert_eq!(
        cache
            .save_ticker_if_fresh(equal)
            .await
            .map_err(|error| format!("equal ticker CAS failed: {error}"))?,
        MarketCacheWriteOutcome::RejectedStale
    );

    let mut connection = manager.clone();
    let raw_payload = connection
        .get::<_, Option<String>>(&key)
        .await?
        .ok_or("ticker CAS did not leave a cached payload")?;
    let payload: serde_json::Value = serde_json::from_str(&raw_payload)?;
    assert_eq!(payload["last_price"], "12.50");
    assert_eq!(
        payload["observed_at"],
        (base + chrono::Duration::seconds(2)).timestamp_millis()
    );
    let _: usize = connection.del(&key).await?;
    Ok(())
}

#[tokio::test]
async fn concurrent_ticker_writers_converge_to_newest_observed_at() -> Result<(), Box<dyn Error>> {
    let Some(redis_url) = env_or_skip("REDIS_URL") else {
        return Ok(());
    };
    let client = redis::Client::open(redis_url)?;
    let manager = redis::aio::ConnectionManager::new(client).await?;
    let cache = Arc::new(RedisMarketCache::new(manager.clone()));
    let uuid = Uuid::now_v7().simple().to_string();
    let symbol = format!("TICKRACE{}USDT", &uuid[16..24]);
    let base = Utc.with_ymd_and_hms(2026, 8, 12, 18, 0, 0).unwrap();
    let key = MarketTickerCacheEntry::new(&symbol, decimal("1"), decimal("1"), base)?
        .redis_key()
        .to_owned();
    let mut writers = Vec::new();
    for second in 0_i64..32 {
        let cache = cache.clone();
        let symbol = symbol.clone();
        writers.push(tokio::spawn(async move {
            let entry = MarketTickerCacheEntry::new(
                &symbol,
                BigDecimal::from(second + 1),
                BigDecimal::from(100),
                base + chrono::Duration::seconds(second),
            )
            .expect("valid ticker fixture");
            cache.save_ticker_if_fresh(entry).await
        }));
    }
    for writer in writers {
        writer.await??;
    }

    let mut connection = manager.clone();
    let payload: serde_json::Value =
        serde_json::from_str(&connection.get::<_, String>(&key).await?)?;
    assert_eq!(payload["last_price"], "32");
    assert_eq!(
        payload["observed_at"],
        (base + chrono::Duration::seconds(31)).timestamp_millis()
    );
    let _: usize = connection.del(&key).await?;
    Ok(())
}

#[tokio::test]
async fn redis_kline_compare_and_set_rejects_older_slot_and_older_forming_snapshot()
-> Result<(), Box<dyn Error>> {
    let Some(redis_url) = env_or_skip("REDIS_URL") else {
        return Ok(());
    };
    let client = redis::Client::open(redis_url)?;
    let manager = redis::aio::ConnectionManager::new(client).await?;
    let cache = RedisMarketCache::new(manager.clone());
    let uuid = Uuid::now_v7().simple().to_string();
    let symbol = format!("KCAS{}USDT", &uuid[16..24]);
    let first_open = Utc.with_ymd_and_hms(2026, 8, 12, 19, 0, 0).unwrap();
    let current_open = first_open + chrono::Duration::minutes(1);
    let values = |close: &str| MarketKlineValues {
        open: decimal("10"),
        high: decimal("20"),
        low: decimal("5"),
        close: decimal(close),
        volume: decimal("100"),
    };
    let first = MarketKlineCacheEntry::with_observed_at(
        &symbol,
        "1m",
        current_open,
        values("13"),
        current_open + chrono::Duration::seconds(15),
    )?;
    let newer = MarketKlineCacheEntry::with_observed_at(
        &symbol,
        "1m",
        current_open,
        values("15"),
        current_open + chrono::Duration::seconds(20),
    )?;
    let key = first.redis_key().to_owned();
    let normalized_symbol = first.symbol().to_owned();
    let sequence_key = format!("market:kline-sequence:{normalized_symbol}:1m");
    let equal = MarketKlineCacheEntry::with_observed_at(
        &symbol,
        "1m",
        current_open,
        values("14"),
        current_open + chrono::Duration::seconds(20),
    )?;
    let old_forming = MarketKlineCacheEntry::with_observed_at(
        &symbol,
        "1m",
        current_open,
        values("11"),
        current_open + chrono::Duration::seconds(10),
    )?;
    let old_slot = MarketKlineCacheEntry::with_observed_at(
        &symbol,
        "1m",
        first_open,
        values("9"),
        current_open + chrono::Duration::seconds(30),
    )?;

    assert_eq!(
        cache.save_kline_if_fresh(first).await?,
        MarketCacheWriteOutcome::Accepted
    );
    assert_eq!(
        cache.save_kline_if_fresh(newer).await?,
        MarketCacheWriteOutcome::Accepted
    );
    assert_eq!(
        cache.save_kline_if_fresh(equal).await?,
        MarketCacheWriteOutcome::RejectedStale
    );
    assert_eq!(
        cache.save_kline_if_fresh(old_forming).await?,
        MarketCacheWriteOutcome::RejectedStale
    );
    assert_eq!(
        cache.save_kline_if_fresh(old_slot).await?,
        MarketCacheWriteOutcome::RejectedStale
    );

    let mut connection = manager.clone();
    let payload: serde_json::Value =
        serde_json::from_str(&connection.get::<_, String>(&key).await?)?;
    assert_eq!(payload["open_time"], current_open.timestamp_millis());
    assert_eq!(payload["close"], "15");
    assert!(payload.get("observed_at").is_none());
    let _: usize = connection.del(&[key, sequence_key]).await?;
    Ok(())
}
