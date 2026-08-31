use bigdecimal::BigDecimal;
use chrono::{TimeZone, Utc};
use exchange_api::{
    infra::mongo::KLINE_UNIQUE_INDEX_NAME,
    modules::{
        events::{EventBroadcastHub, WebSocketChannel},
        market::{
            MarketDataProvider, MarketKlineSnapshot, MarketKlineValues, MarketTickerCacheEntry,
            MarketTickerSnapshot, RedisMarketCache,
            adapters::{
                MarketIngestionService, MarketKlineMongoWrite, SyntheticIngestionOutcome,
                SyntheticTickerProvenance,
            },
        },
    },
};
use futures_util::TryStreamExt;
use mongodb::{Client, IndexModel, bson::doc, options::ClientOptions};
use redis::AsyncCommands;
use sha2::Digest;
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::{error::Error, str::FromStr};
use tokio::time::{Duration, timeout};
use uuid::Uuid;

const INGESTION_SOURCE: &str =
    include_str!("../src/modules/market/infrastructure/adapters/ingestion.rs");

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

async fn mysql_pool_or_skip() -> Result<Option<MySqlPool>, Box<dyn Error>> {
    let Some(database_url) = env_or_skip("DATABASE_URL") else {
        return Ok(None);
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    sqlx::migrate!().run(&pool).await?;
    Ok(Some(pool))
}

struct SyntheticStrategyFixture {
    strategy_id: u64,
    owner: String,
}

#[derive(Debug, sqlx::FromRow)]
struct ArchivedSyntheticTicker {
    event_key: String,
    symbol: String,
    price: BigDecimal,
    source: String,
    observed_at: chrono::DateTime<Utc>,
    generation: u64,
    source_version: String,
    strategy_id: Option<u64>,
    strategy_version: Option<i32>,
}

async fn seed_synthetic_strategy(
    pool: &MySqlPool,
    symbol: &str,
    lease_is_current: bool,
) -> Result<SyntheticStrategyFixture, Box<dyn Error>> {
    let suffix = Uuid::now_v7().simple().to_string();
    let base_symbol = format!("SB{}", &suffix[16..24]).to_ascii_uppercase();
    let quote_symbol = format!("SQ{}", &suffix[24..32]).to_ascii_uppercase();
    let mut tx = pool.begin().await?;
    let base_asset = sqlx::query(
        "INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, 18, 'coin', 'active')",
    )
    .bind(&base_symbol)
    .bind(&base_symbol)
    .execute(&mut *tx)
    .await?
    .last_insert_id();
    let quote_asset = sqlx::query(
        "INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, 18, 'coin', 'active')",
    )
    .bind(&quote_symbol)
    .bind(&quote_symbol)
    .execute(&mut *tx)
    .await?
    .last_insert_id();
    let pair_id = sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision,
            min_order_value, status, market_type)
           VALUES (?, ?, ?, 18, 18, 1, 'active', 'strategy')"#,
    )
    .bind(base_asset)
    .bind(quote_asset)
    .bind(symbol)
    .execute(&mut *tx)
    .await?
    .last_insert_id();
    let now = Utc::now();
    let strategy_id = sqlx::query(
        r#"INSERT INTO market_strategies
           (pair_id, strategy_type, start_price, target_price, start_time, end_time,
            volatility, volume_min, volume_max, status)
           VALUES (?, 'linear', 10, 20, ?, ?, 0.1, 1, 100, 'active')"#,
    )
    .bind(pair_id)
    .bind((now - chrono::Duration::hours(1)).naive_utc())
    .bind((now + chrono::Duration::hours(1)).naive_utc())
    .execute(&mut *tx)
    .await?
    .last_insert_id();
    sqlx::query(
        r#"INSERT INTO strategy_versions
           (strategy_id, version, effective_time, config_json, seed)
           VALUES (?, 1, ?, JSON_OBJECT(), ?)"#,
    )
    .bind(strategy_id)
    .bind((now - chrono::Duration::hours(1)).naive_utc())
    .bind(format!("test-seed-{suffix}"))
    .execute(&mut *tx)
    .await?;
    let owner = format!("synthetic-test-{suffix}");
    let lease_expires_at = if lease_is_current {
        now + chrono::Duration::minutes(10)
    } else {
        now - chrono::Duration::minutes(10)
    };
    sqlx::query(
        r#"INSERT INTO strategy_runs
           (strategy_id, active_version, run_status, recovery_status,
            lease_owner, lease_expires_at)
           VALUES (?, 1, 'live', 'live', ?, ?)"#,
    )
    .bind(strategy_id)
    .bind(&owner)
    .bind(lease_expires_at.naive_utc())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(SyntheticStrategyFixture { strategy_id, owner })
}

#[test]
fn accepted_tickers_trigger_margin_limits_while_depth_and_stale_paths_do_not() {
    let ticker = INGESTION_SOURCE
        .split("pub async fn ingest_ticker")
        .nth(1)
        .expect("ticker ingestion function")
        .split("pub async fn ingest_and_publish_synthetic_ticker")
        .next()
        .expect("ticker ingestion body");
    assert!(ticker.contains("if outcome.is_accepted()"));
    assert!(ticker.contains("self.trigger_margin_limit_orders"));

    let synthetic_ticker = INGESTION_SOURCE
        .split("pub async fn ingest_and_publish_synthetic_ticker")
        .nth(1)
        .expect("synthetic ticker ingestion function")
        .split("pub async fn ingest_and_publish_ticker")
        .next()
        .expect("synthetic ticker ingestion body");
    assert!(synthetic_ticker.contains("SyntheticIngestionOutcome::RejectedStale"));
    assert!(synthetic_ticker.contains("archive_synthetic_ticker"));
    assert!(synthetic_ticker.contains("save_ticker_if_fresh"));
    assert!(synthetic_ticker.contains("self.trigger_margin_limit_orders"));
    assert!(
        synthetic_ticker.find("archive_synthetic_ticker")
            < synthetic_ticker.find("save_ticker_if_fresh")
    );
    assert!(
        synthetic_ticker.find("save_ticker_if_fresh")
            < synthetic_ticker.find("self.trigger_margin_limit_orders")
    );

    let depth = INGESTION_SOURCE
        .split("    pub async fn ingest_depth")
        .nth(1)
        .expect("depth ingestion function")
        .split("pub async fn ingest_kline")
        .next()
        .expect("depth ingestion body");
    assert!(!depth.contains("trigger_margin_limit_orders"));
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
async fn synthetic_ticker_archives_replays_repairs_and_rejects_stale_lease()
-> Result<(), Box<dyn Error>> {
    let Some(redis_url) = env_or_skip("REDIS_URL") else {
        return Ok(());
    };
    let Some(pool) = mysql_pool_or_skip().await? else {
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
    let cache = RedisMarketCache::new(manager.clone());
    let ingestion =
        MarketIngestionService::new(cache.clone(), mongo_client.database(&mongodb_database))
            .with_mysql(Some(pool.clone()))
            .with_broadcast_hub(Some(hub.clone()));
    let mut connection = manager.clone();

    let symbol = test_symbol("SYNARCH");
    let fixture = seed_synthetic_strategy(&pool, &symbol, true).await?;
    let provenance = SyntheticTickerProvenance::new(fixture.strategy_id, 1, &fixture.owner);
    let channel = WebSocketChannel::public("ticker", &symbol)?;
    let mut receiver = hub.subscribe(&channel);
    let observed_at = Utc
        .timestamp_millis_opt(Utc::now().timestamp_millis())
        .single()
        .expect("millisecond timestamp");
    let snapshot = MarketTickerSnapshot::new(
        MarketDataProvider::Strategy,
        &symbol,
        decimal("20"),
        decimal("2"),
        observed_at,
    )?;

    assert_eq!(
        ingestion
            .ingest_and_publish_synthetic_ticker(&snapshot, &provenance)
            .await?,
        SyntheticIngestionOutcome::Accepted
    );
    let accepted = timeout(Duration::from_millis(100), receiver.recv()).await??;
    assert!(accepted.payload().contains("\"last_price\":\"20\""));
    let archived = sqlx::query_as::<_, ArchivedSyntheticTicker>(
        r#"SELECT event_key, symbol, price, source, observed_at, generation,
                  source_version, strategy_id, strategy_version
           FROM market_price_ticks
           WHERE strategy_id = ? AND strategy_version = 1 AND observed_at = ?"#,
    )
    .bind(fixture.strategy_id)
    .bind(observed_at.naive_utc())
    .fetch_one(&pool)
    .await?;
    let source_version = format!("strategy:{}:v1", fixture.strategy_id);
    let canonical = format!(
        "strategy|{}|{}|{}|1|{}",
        symbol,
        observed_at.timestamp_micros(),
        snapshot.last_price().normalized(),
        source_version
    );
    assert_eq!(
        archived.event_key,
        hex::encode(sha2::Sha256::digest(canonical.as_bytes()))
    );
    assert_eq!(archived.symbol, symbol);
    assert_eq!(archived.price.normalized(), decimal("20"));
    assert_eq!(archived.source, "strategy");
    assert_eq!(archived.observed_at, observed_at);
    assert_eq!(archived.generation, 1);
    assert_eq!(archived.source_version, source_version);
    assert_eq!(archived.strategy_id, Some(fixture.strategy_id));
    assert_eq!(archived.strategy_version, Some(1));

    assert_eq!(
        ingestion
            .ingest_and_publish_synthetic_ticker(&snapshot, &provenance)
            .await?,
        SyntheticIngestionOutcome::ReplayedIdentical
    );
    assert!(
        timeout(Duration::from_millis(25), receiver.recv())
            .await
            .is_err()
    );
    let archive_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM market_price_ticks WHERE strategy_id = ? AND strategy_version = 1 AND observed_at = ?",
    )
    .bind(fixture.strategy_id)
    .bind(observed_at.naive_utc())
    .fetch_one(&pool)
    .await?;
    assert_eq!(archive_count, 1);

    let conflicting = MarketTickerSnapshot::new(
        MarketDataProvider::Strategy,
        &symbol,
        decimal("21"),
        decimal("2"),
        observed_at,
    )?;
    let conflicting_error = ingestion
        .ingest_and_publish_synthetic_ticker(&conflicting, &provenance)
        .await
        .expect_err("same event identity with a different price must conflict");
    assert!(
        conflicting_error
            .to_string()
            .contains("conflicts with an existing event payload")
    );
    let cached_after_conflict: serde_json::Value = serde_json::from_str(
        &connection
            .get::<_, String>(format!("market:ticker:{symbol}"))
            .await?,
    )?;
    assert_eq!(cached_after_conflict["last_price"], "20");
    assert!(
        timeout(Duration::from_millis(25), receiver.recv())
            .await
            .is_err()
    );

    // Redis 丢失后的倒退事件必须先由 MySQL 权威历史拒绝，不能借缓存空洞重新进入实时层。
    let _: usize = connection.del(format!("market:ticker:{symbol}")).await?;
    let regressed = MarketTickerSnapshot::new(
        MarketDataProvider::Strategy,
        &symbol,
        decimal("19"),
        decimal("2"),
        observed_at - chrono::TimeDelta::milliseconds(1),
    )?;
    let regressed_error = ingestion
        .ingest_and_publish_synthetic_ticker(&regressed, &provenance)
        .await
        .expect_err("MySQL history must reject a regressed event after Redis loss");
    assert!(regressed_error.to_string().contains("event time regressed"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM market_price_ticks WHERE strategy_id = ?",
        )
        .bind(fixture.strategy_id)
        .fetch_one(&pool)
        .await?,
        1
    );
    assert_eq!(
        ingestion
            .ingest_and_publish_synthetic_ticker(&snapshot, &provenance)
            .await?,
        SyntheticIngestionOutcome::ReplayedIdentical
    );
    assert!(
        timeout(Duration::from_millis(25), receiver.recv())
            .await
            .is_err()
    );

    let stale_symbol = test_symbol("SYNLEASE");
    let stale_fixture = seed_synthetic_strategy(&pool, &stale_symbol, false).await?;
    let stale_provenance =
        SyntheticTickerProvenance::new(stale_fixture.strategy_id, 1, &stale_fixture.owner);
    let stale_snapshot = MarketTickerSnapshot::new(
        MarketDataProvider::Strategy,
        &stale_symbol,
        decimal("30"),
        decimal("3"),
        observed_at,
    )?;
    let stale_channel = WebSocketChannel::public("ticker", &stale_symbol)?;
    let mut stale_receiver = hub.subscribe(&stale_channel);
    let stale_error = ingestion
        .ingest_and_publish_synthetic_ticker(&stale_snapshot, &stale_provenance)
        .await
        .expect_err("expired lease must reject archive");
    assert!(
        stale_error
            .to_string()
            .contains("lease changed before archive")
    );
    let stale_archive_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM market_price_ticks WHERE strategy_id = ?")
            .bind(stale_fixture.strategy_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(stale_archive_count, 0);
    assert!(
        !connection
            .exists::<_, bool>(format!("market:ticker:{stale_symbol}"))
            .await?,
        "expired lease must be rejected before Redis CAS"
    );
    assert!(
        timeout(Duration::from_millis(25), stale_receiver.recv())
            .await
            .is_err()
    );

    let old_version_symbol = test_symbol("SYNVERSION");
    let old_version_fixture = seed_synthetic_strategy(&pool, &old_version_symbol, true).await?;
    let old_version_provenance = SyntheticTickerProvenance::new(
        old_version_fixture.strategy_id,
        2,
        &old_version_fixture.owner,
    );
    let old_version_snapshot = MarketTickerSnapshot::new(
        MarketDataProvider::Strategy,
        &old_version_symbol,
        decimal("31"),
        decimal("3"),
        observed_at,
    )?;
    let old_version_error = ingestion
        .ingest_and_publish_synthetic_ticker(&old_version_snapshot, &old_version_provenance)
        .await
        .expect_err("non-active strategy version must reject archive");
    assert!(
        old_version_error
            .to_string()
            .contains("version, status, or lease changed")
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM market_price_ticks WHERE strategy_id = ?",
        )
        .bind(old_version_fixture.strategy_id)
        .fetch_one(&pool)
        .await?,
        0
    );
    assert!(
        !connection
            .exists::<_, bool>(format!("market:ticker:{old_version_symbol}"))
            .await?,
        "inactive strategy version must be rejected before Redis CAS"
    );

    let future_symbol = test_symbol("SYNFUTURE");
    let future_fixture = seed_synthetic_strategy(&pool, &future_symbol, true).await?;
    let future_provenance =
        SyntheticTickerProvenance::new(future_fixture.strategy_id, 1, &future_fixture.owner);
    let future_snapshot = MarketTickerSnapshot::new(
        MarketDataProvider::Strategy,
        &future_symbol,
        decimal("32"),
        decimal("3"),
        observed_at + chrono::TimeDelta::minutes(5),
    )?;
    let future_error = ingestion
        .ingest_and_publish_synthetic_ticker(&future_snapshot, &future_provenance)
        .await
        .expect_err("future synthetic event time must reject archive");
    assert!(
        future_error
            .to_string()
            .contains("event time is outside current strategy bounds")
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM market_price_ticks WHERE strategy_id = ?",
        )
        .bind(future_fixture.strategy_id)
        .fetch_one(&pool)
        .await?,
        0
    );
    assert!(
        !connection
            .exists::<_, bool>(format!("market:ticker:{future_symbol}"))
            .await?,
        "future event time must be rejected before Redis CAS"
    );

    let repair_symbol = test_symbol("SYNREPAIR");
    let repair_fixture = seed_synthetic_strategy(&pool, &repair_symbol, true).await?;
    let repair_provenance =
        SyntheticTickerProvenance::new(repair_fixture.strategy_id, 1, &repair_fixture.owner);
    let repair_snapshot = MarketTickerSnapshot::new(
        MarketDataProvider::Strategy,
        &repair_symbol,
        decimal("40"),
        decimal("4"),
        observed_at,
    )?;
    assert_eq!(
        cache
            .save_ticker_if_fresh(MarketTickerCacheEntry::from_snapshot(&repair_snapshot)?)
            .await?,
        exchange_api::modules::market::MarketCacheWriteOutcome::Accepted
    );
    let repair_channel = WebSocketChannel::public("ticker", &repair_symbol)?;
    let mut repair_receiver = hub.subscribe(&repair_channel);
    assert_eq!(
        ingestion
            .ingest_and_publish_synthetic_ticker(&repair_snapshot, &repair_provenance)
            .await?,
        SyntheticIngestionOutcome::ReplayedIdentical
    );
    let repaired_event = timeout(Duration::from_millis(100), repair_receiver.recv()).await??;
    assert!(repaired_event.payload().contains("\"last_price\":\"40\""));
    let repaired_archive_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM market_price_ticks WHERE strategy_id = ?")
            .bind(repair_fixture.strategy_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(repaired_archive_count, 1);

    let payload: serde_json::Value = serde_json::from_str(
        &connection
            .get::<_, String>(format!("market:ticker:{symbol}"))
            .await?,
    )?;
    assert_eq!(payload["last_price"], "20");
    let _: usize = connection
        .del(&[
            format!("market:ticker:{symbol}"),
            format!("market:ticker:{stale_symbol}"),
            format!("market:ticker:{old_version_symbol}"),
            format!("market:ticker:{future_symbol}"),
            format!("market:ticker:{repair_symbol}"),
        ])
        .await?;
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
