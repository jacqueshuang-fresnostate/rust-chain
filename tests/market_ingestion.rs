use bigdecimal::BigDecimal;
use chrono::{TimeZone, Utc};
use exchange_api::{
    infra::mongo::KLINE_UNIQUE_INDEX_NAME,
    modules::{
        events::{EventBroadcastHub, WebSocketChannel},
        market::{
            MarketDataProvider, MarketKlineSnapshot, MarketKlineValues, MarketTickerSnapshot,
            RedisMarketCache,
            adapters::{MarketIngestionService, MarketKlineMongoWrite, SyntheticIngestionOutcome},
        },
    },
};
use futures_util::TryStreamExt;
use mongodb::{Client, IndexModel, bson::doc, options::ClientOptions};
use redis::AsyncCommands;
use std::{error::Error, str::FromStr};
use tokio::time::{Duration, timeout};
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

fn test_symbol(prefix: &str) -> String {
    let uuid = Uuid::now_v7().simple().to_string();
    // Redis、Mongo 与 WebSocket 入口都会把交易对规范化为大写；测试夹具也使用同一形态，
    // 避免从小写 UUID 拼出的原始 symbol 误读另一个 Redis key 或订阅错误频道。
    format!("{}{}USDT", prefix, &uuid[16..32]).to_ascii_uppercase()
}

#[tokio::test]
async fn market_ingestion_writes_ticker_to_redis_and_kline_to_redis_and_mongo()
-> Result<(), Box<dyn Error>> {
    let Some(redis_url) = env_or_skip("REDIS_URL") else {
        return Ok(());
    };
    let Some(mongodb_uri) = env_or_skip("MONGODB_URI") else {
        return Ok(());
    };
    let mongodb_database =
        std::env::var("MONGODB_DATABASE").unwrap_or_else(|_| "exchange_test".to_owned());
    let redis_client = redis::Client::open(redis_url)?;
    let manager = redis::aio::ConnectionManager::new(redis_client).await?;
    let mongo_client = Client::with_options(ClientOptions::parse(&mongodb_uri).await?)?;
    let database = mongo_client.database(&mongodb_database);
    let cache = RedisMarketCache::new(manager.clone());
    let ingestion = MarketIngestionService::new(cache, database.clone());
    let symbol = test_symbol("INGEST");
    let observed_at = Utc.timestamp_millis_opt(1_710_000_000_000).unwrap();
    let open_time = Utc.timestamp_millis_opt(1_710_000_000_000).unwrap();
    let ticker = MarketTickerSnapshot::new(
        MarketDataProvider::Bitget,
        &symbol,
        decimal("70000.12"),
        decimal("125.50"),
        observed_at,
    )?;
    let kline = MarketKlineSnapshot::new(
        MarketDataProvider::Htx,
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
        observed_at,
    )?;

    ingestion.ingest_ticker(&ticker).await?;
    ingestion.ingest_kline(&kline).await?;
    ingestion.ingest_kline(&kline).await?;

    let mut raw_connection = manager.clone();
    let ticker_payload: String = raw_connection
        .get(format!("market:ticker:{symbol}"))
        .await?;
    let kline_payload: String = raw_connection
        .get(format!("market:kline:{symbol}:1m"))
        .await?;
    let ticker_json: serde_json::Value = serde_json::from_str(&ticker_payload)?;
    let kline_json: serde_json::Value = serde_json::from_str(&kline_payload)?;
    let mongo_write = MarketKlineMongoWrite::from_snapshot(&kline)?;
    let collection = database.collection::<mongodb::bson::Document>(&mongo_write.collection_name());
    let stored_count = collection
        .count_documents(mongo_write.upsert_filter())
        .await?;
    let stored = collection
        .find_one(doc! { "interval": "1m", "open_time": mongodb::bson::DateTime::from_millis(open_time.timestamp_millis()) })
        .await?
        .unwrap();
    let indexes: Vec<IndexModel> = collection.list_indexes().await?.try_collect().await?;

    assert_eq!(ticker_json["symbol"], symbol);
    assert_eq!(ticker_json["last_price"], "70000.12");
    assert_eq!(kline_json["redis_key"], format!("market:kline:{symbol}:1m"));
    assert_eq!(stored_count, 1);
    assert!(indexes.iter().any(|index| {
        index.keys == doc! { "interval": 1, "open_time": 1 }
            && index
                .options
                .as_ref()
                .and_then(|options| options.name.as_deref())
                == Some(KLINE_UNIQUE_INDEX_NAME)
            && index.options.as_ref().and_then(|options| options.unique) == Some(true)
    }));
    assert_eq!(stored.get_str("close")?, "70005.00");
    assert_eq!(stored.get_str("source")?, "htx");

    let _: usize = raw_connection
        .del(&[
            format!("market:ticker:{symbol}"),
            format!("market:kline:{symbol}:1m"),
            format!("market:kline-sequence:{symbol}:1m"),
        ])
        .await?;
    collection.delete_many(doc! {}).await?;
    Ok(())
}

#[tokio::test]
async fn stale_synthetic_ticker_is_rejected_without_websocket_broadcast()
-> Result<(), Box<dyn Error>> {
    let Some(redis_url) = env_or_skip("REDIS_URL") else {
        return Ok(());
    };
    let mongodb_uri = std::env::var("MONGODB_URI")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "mongodb://127.0.0.1:27017".to_owned());
    let mongodb_database =
        std::env::var("MONGODB_DATABASE").unwrap_or_else(|_| "exchange_test".to_owned());
    let manager = redis::aio::ConnectionManager::new(redis::Client::open(redis_url)?).await?;
    let mongo_client = Client::with_options(ClientOptions::parse(&mongodb_uri).await?)?;
    let hub = EventBroadcastHub::new(8);
    let ingestion = MarketIngestionService::new(
        RedisMarketCache::new(manager.clone()),
        mongo_client.database(&mongodb_database),
    )
    .with_broadcast_hub(Some(hub.clone()));
    let symbol = test_symbol("STALE");
    let normalized_symbol = symbol.to_ascii_uppercase();
    let channel = WebSocketChannel::public("ticker", &normalized_symbol)?;
    let mut receiver = hub.subscribe(&channel);
    let base = Utc.with_ymd_and_hms(2026, 8, 12, 20, 0, 0).unwrap();
    let newer = MarketTickerSnapshot::new(
        MarketDataProvider::Strategy,
        &symbol,
        decimal("20"),
        decimal("2"),
        base + chrono::Duration::seconds(2),
    )?;
    let older = MarketTickerSnapshot::new(
        MarketDataProvider::Strategy,
        &symbol,
        decimal("10"),
        decimal("1"),
        base + chrono::Duration::seconds(1),
    )?;

    assert_eq!(
        ingestion
            .ingest_and_publish_synthetic_ticker(&newer)
            .await?,
        SyntheticIngestionOutcome::Accepted
    );
    let accepted = timeout(Duration::from_millis(100), receiver.recv()).await??;
    assert!(accepted.payload().contains("\"last_price\":\"20\""));
    assert_eq!(
        ingestion
            .ingest_and_publish_synthetic_ticker(&older)
            .await?,
        SyntheticIngestionOutcome::RejectedStale
    );
    assert!(
        timeout(Duration::from_millis(25), receiver.recv())
            .await
            .is_err()
    );

    let key = format!("market:ticker:{normalized_symbol}");
    let mut connection = manager.clone();
    let payload: serde_json::Value =
        serde_json::from_str(&connection.get::<_, String>(&key).await?)?;
    assert_eq!(payload["last_price"], "20");
    let _: usize = connection.del(key).await?;
    Ok(())
}

#[tokio::test]
async fn stale_synthetic_kline_is_rejected_without_mongo_overwrite_or_broadcast()
-> Result<(), Box<dyn Error>> {
    let Some(redis_url) = env_or_skip("REDIS_URL") else {
        return Ok(());
    };
    let Some(mongodb_uri) = env_or_skip("MONGODB_URI") else {
        return Ok(());
    };
    let mongodb_database =
        std::env::var("MONGODB_DATABASE").unwrap_or_else(|_| "exchange_test".to_owned());
    let manager = redis::aio::ConnectionManager::new(redis::Client::open(redis_url)?).await?;
    let mongo_client = Client::with_options(ClientOptions::parse(&mongodb_uri).await?)?;
    let database = mongo_client.database(&mongodb_database);
    let hub = EventBroadcastHub::new(8);
    let ingestion =
        MarketIngestionService::new(RedisMarketCache::new(manager.clone()), database.clone())
            .with_broadcast_hub(Some(hub.clone()));
    let symbol = test_symbol("KSTALE");
    let channel = WebSocketChannel::public("kline", format!("{symbol}_1m"))?;
    let mut receiver = hub.subscribe(&channel);
    let open_time = Utc.with_ymd_and_hms(2026, 8, 12, 21, 0, 0).unwrap();
    let snapshot = |close: &str, second: i64| {
        MarketKlineSnapshot::new(
            MarketDataProvider::Strategy,
            &symbol,
            "1m",
            open_time,
            MarketKlineValues {
                open: decimal("10"),
                high: decimal("20"),
                low: decimal("5"),
                close: decimal(close),
                volume: decimal("100"),
            },
            open_time + chrono::Duration::seconds(second),
        )
    };
    let newer = snapshot("18", 20)?;
    let older = snapshot("12", 10)?;

    assert_eq!(
        ingestion.ingest_and_publish_synthetic_kline(&newer).await?,
        SyntheticIngestionOutcome::Accepted
    );
    let accepted = timeout(Duration::from_millis(100), receiver.recv()).await??;
    assert!(accepted.payload().contains("\"close\":\"18\""));
    assert_eq!(
        ingestion.ingest_and_publish_synthetic_kline(&older).await?,
        SyntheticIngestionOutcome::RejectedStale
    );
    assert!(
        timeout(Duration::from_millis(25), receiver.recv())
            .await
            .is_err()
    );

    let mongo_write = MarketKlineMongoWrite::from_snapshot(&newer)?;
    let collection = database.collection::<mongodb::bson::Document>(&mongo_write.collection_name());
    let stored = collection
        .find_one(mongo_write.upsert_filter())
        .await?
        .expect("accepted kline must be stored");
    assert_eq!(stored.get_str("close")?, "18");
    let key = format!("market:kline:{symbol}:1m");
    let sequence_key = format!("market:kline-sequence:{symbol}:1m");
    let mut connection = manager.clone();
    let _: usize = connection.del(&[key, sequence_key]).await?;
    collection.delete_many(doc! {}).await?;
    Ok(())
}
