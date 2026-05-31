use bigdecimal::BigDecimal;
use chrono::{TimeZone, Utc};
use exchange_api::{
    infra::mongo::KLINE_UNIQUE_INDEX_NAME,
    modules::market::{
        MarketDataProvider, MarketKlineSnapshot, MarketKlineValues, MarketTickerSnapshot,
        RedisMarketCache,
        adapters::{MarketIngestionService, MarketKlineMongoWrite},
    },
};
use futures_util::TryStreamExt;
use mongodb::{Client, IndexModel, bson::doc, options::ClientOptions};
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

fn test_symbol(prefix: &str) -> String {
    let uuid = Uuid::now_v7().simple().to_string();
    format!("{}{}USDT", prefix, &uuid[16..32])
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
        ])
        .await?;
    collection.delete_many(doc! {}).await?;
    Ok(())
}
