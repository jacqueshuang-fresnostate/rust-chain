//! 默认行情与人工策略跨分钟接管的真实存储验证；仅使用独立库和真实当前时间，不制造未来行情。

use std::{error::Error, str::FromStr, time::Duration};

use bigdecimal::BigDecimal;
use chrono::{DateTime, TimeDelta, Timelike, Utc};
use exchange_api::{
    modules::market::{RedisMarketCache, adapters::MarketIngestionService, sanitize_symbol},
    workers::synthetic_market::run_once_with_dependencies,
};
use mongodb::bson::{Document, doc};
use redis::AsyncCommands;
use serde_json::{Value, json};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use uuid::Uuid;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}
fn minute(time: DateTime<Utc>) -> DateTime<Utc> {
    time.with_second(0).unwrap().with_nanosecond(0).unwrap()
}

async fn next_real_minute(after: DateTime<Utc>) -> DateTime<Utc> {
    let target = minute(after) + TimeDelta::minutes(1) + TimeDelta::milliseconds(200);
    let delay = (target - Utc::now()).num_milliseconds().max(0) as u64;
    tokio::time::sleep(Duration::from_millis(delay)).await;
    let now = Utc::now();
    assert!(now >= target, "wait must reach a real UTC minute boundary");
    now
}

async fn strategy(
    pool: &MySqlPool,
    pair: u64,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    seed: &str,
) -> Result<u64, sqlx::Error> {
    let id=sqlx::query("INSERT INTO market_strategies(pair_id,strategy_type,start_price,target_price,start_time,end_time,volatility,volume_min,volume_max,status) VALUES(?,'price_path',0.12,0.13,?,?,0.01,60,60,'active')")
        .bind(pair).bind(start.naive_utc()).bind(end.naive_utc()).execute(pool).await?.last_insert_id();
    sqlx::query("INSERT INTO strategy_versions(strategy_id,version,effective_time,config_json,seed) VALUES(?,1,?,JSON_OBJECT(),?)")
        .bind(id).bind(start.naive_utc()).bind(seed).execute(pool).await?;
    sqlx::query(
        "INSERT INTO strategy_runs(strategy_id,active_version,run_status) VALUES(?,1,'running')",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(id)
}

async fn archive_count(pool: &MySqlPool, symbol: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM market_price_ticks WHERE symbol=?")
        .bind(symbol)
        .fetch_one(pool)
        .await
}

async fn active_source(pool: &MySqlPool, pair: u64) -> Result<(String, u64), sqlx::Error> {
    sqlx::query_as(
        "SELECT active_source,generation FROM market_pair_generation_runs WHERE pair_id=?",
    )
    .bind(pair)
    .fetch_one(pool)
    .await
}

async fn root(
    collection: &mongodb::Collection<Document>,
    open: DateTime<Utc>,
) -> Result<Document, mongodb::error::Error> {
    Ok(collection
        .find_one(doc! {"interval":"1m","open_time":mongodb::bson::DateTime::from_millis(open.timestamp_millis())})
        .await?
        .expect("published authoritative one-minute candle"))
}

#[tokio::test]
async fn default_manual_default_transition_waits_for_real_minutes_and_preserves_history()
-> Result<(), Box<dyn Error>> {
    let (Ok(mysql_url), Ok(mongo_url), Ok(redis_url)) = (
        std::env::var("DATABASE_URL"),
        std::env::var("DEFAULT_MARKET_TEST_MONGO_URL"),
        std::env::var("DEFAULT_MARKET_TEST_REDIS_URL"),
    ) else {
        eprintln!(
            "skipping transition integration: isolated MySQL/Mongo/Redis environment required"
        );
        return Ok(());
    };
    assert!(mongo_url.starts_with("mongodb://127.0.0.1:"));
    assert!(redis_url.starts_with("redis://127.0.0.1:"));
    let pool = MySqlPoolOptions::new()
        .max_connections(8)
        .connect(&mysql_url)
        .await?;
    let database: String = sqlx::query_scalar("SELECT DATABASE()")
        .fetch_one(&pool)
        .await?;
    assert!(
        database.starts_with("codex_default_runtime_transition_"),
        "requires a dedicated transition-only MySQL database"
    );
    sqlx::migrate!().run(&pool).await?;
    let existing: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM trading_pairs")
        .fetch_one(&pool)
        .await?;
    assert_eq!(
        existing, 0,
        "transition database must not contain another worker candidate"
    );
    let suffix = &Uuid::now_v7().simple().to_string()[16..24];
    let raw_symbol = format!("DT{suffix}-USDT");
    let symbol = sanitize_symbol(&raw_symbol);
    let mut assets = Vec::new();
    for prefix in ["DTB", "DTQ"] {
        assets.push(sqlx::query("INSERT INTO assets(symbol,name,precision_scale,asset_type,status) VALUES(?,?,6,'coin','active')")
            .bind(format!("{prefix}{suffix}")).bind("Transition fixture asset").execute(&pool).await?.last_insert_id());
    }
    let pair=sqlx::query("INSERT INTO trading_pairs(base_asset,quote_asset,symbol,price_precision,qty_precision,min_order_value,status,market_type) VALUES(?,?,?,6,2,1,'active','strategy')")
        .bind(assets[0]).bind(assets[1]).bind(&raw_symbol).execute(&pool).await?.last_insert_id();
    sqlx::query("INSERT INTO market_default_generators(pair_id,enabled,initial_price,config_json,version,seed) VALUES(?,TRUE,0.1,?,1,'transition-default')")
        .bind(pair).bind(sqlx::types::Json(json!({"volume_min":"60","volume_max":"60"}))).execute(&pool).await?;
    sqlx::query("INSERT INTO market_default_generator_versions(pair_id,version,initial_price,config_json,seed) SELECT pair_id,version,initial_price,config_json,seed FROM market_default_generators WHERE pair_id=?")
        .bind(pair).execute(&pool).await?;
    let mongo = mongodb::Client::with_uri_str(mongo_url)
        .await?
        .database(&format!("codex_default_transition_{suffix}"));
    let mut redis = redis::Client::open(redis_url)?
        .get_connection_manager()
        .await?;
    let ingestion =
        MarketIngestionService::new(RedisMarketCache::new(redis.clone()), mongo.clone())
            .with_mysql(Some(pool.clone()));
    let collection = mongo.collection::<Document>(&format!("market_klines_{symbol}"));
    let ticker_key = format!("market:ticker:{symbol}");
    let candle_key = format!("market:kline:{symbol}:1m");
    let setup_now = Utc::now();
    let expired = strategy(
        &pool,
        pair,
        minute(setup_now) - TimeDelta::minutes(10),
        minute(setup_now) - TimeDelta::minutes(1),
        "transition-expired",
    )
    .await?;

    // 留出创建人工排期与同分钟断言的时间，避免测试自身正好跨分钟而得到非确定性结果。
    if Utc::now().second() >= 55 {
        next_real_minute(Utc::now()).await;
    }
    let default_time = Utc::now();
    let first = run_once_with_dependencies(
        &pool,
        &mongo,
        &ingestion,
        default_time,
        10,
        "transition-default-a",
    )
    .await?;
    assert_eq!(
        (first.scanned, first.published, first.failed),
        (1, 1, 0),
        "{first:?}"
    );
    let (source, default_generation) = active_source(&pool, pair).await?;
    assert_eq!(source, "default");
    let first_root = root(&collection, minute(default_time)).await?;
    let first_ticker: String = redis.get(&ticker_key).await?;
    assert_eq!(archive_count(&pool, &symbol).await?, 1);
    let manual = strategy(
        &pool,
        pair,
        minute(default_time),
        minute(default_time) + TimeDelta::minutes(6),
        "transition-manual",
    )
    .await?;
    let same_minute = Utc::now();
    assert_eq!(minute(same_minute), minute(default_time));
    let deferred = run_once_with_dependencies(
        &pool,
        &mongo,
        &ingestion,
        same_minute,
        10,
        "transition-manual-a",
    )
    .await?;
    assert_eq!(
        (
            deferred.scanned,
            deferred.published,
            deferred.skipped,
            deferred.failed
        ),
        (1, 0, 1, 0),
        "{deferred:?}"
    );
    assert_eq!(
        active_source(&pool, pair).await?,
        ("default".to_owned(), default_generation)
    );
    assert_eq!(redis.get::<_, String>(&ticker_key).await?, first_ticker);
    assert_eq!(root(&collection, minute(default_time)).await?, first_root);
    assert_eq!(archive_count(&pool, &symbol).await?, 1);
    eprintln!(
        "Default source committed at {default_time}; same-minute manual takeover correctly deferred"
    );

    let manual_time = next_real_minute(default_time).await;
    let takeover = run_once_with_dependencies(
        &pool,
        &mongo,
        &ingestion,
        manual_time,
        10,
        "transition-manual-a",
    )
    .await?;
    assert_eq!(
        (takeover.published, takeover.failed),
        (1, 0),
        "{takeover:?}"
    );
    let (source, manual_generation) = active_source(&pool, pair).await?;
    assert_eq!(source, "strategy");
    assert!(manual_generation > default_generation);
    let manual_ticker: String = redis.get(&ticker_key).await?;
    let manual_price = decimal(
        serde_json::from_str::<Value>(&manual_ticker)?["last_price"]
            .as_str()
            .unwrap(),
    );
    let manual_root = root(&collection, minute(manual_time)).await?;
    assert_eq!(
        root(&collection, minute(default_time)).await?,
        first_root,
        "new manual algorithm must not replace the preceding default candle"
    );
    let identity:(String,Option<u64>,Option<i32>,BigDecimal)=sqlx::query_as("SELECT source,strategy_id,strategy_version,price FROM market_price_ticks WHERE symbol=? ORDER BY observed_at DESC,id DESC LIMIT 1")
        .bind(&symbol).fetch_one(&pool).await?;
    assert_eq!(
        identity,
        (
            "strategy".to_owned(),
            Some(manual),
            Some(1),
            manual_price.clone()
        )
    );

    // 只暂停人工策略，不关闭默认开关；本分钟已属于人工，默认不能接着重画这根蜡烛。
    sqlx::query("UPDATE market_strategies SET status='paused' WHERE id=?")
        .bind(manual)
        .execute(&pool)
        .await?;
    sqlx::query("UPDATE strategy_runs SET run_status='paused',lease_owner=NULL,lease_expires_at=NULL WHERE strategy_id=?").bind(manual).execute(&pool).await?;
    let paused_at = Utc::now();
    assert_eq!(minute(paused_at), minute(manual_time));
    let default_deferred = run_once_with_dependencies(
        &pool,
        &mongo,
        &ingestion,
        paused_at,
        10,
        "transition-default-b",
    )
    .await?;
    assert_eq!(
        (
            default_deferred.published,
            default_deferred.skipped,
            default_deferred.failed
        ),
        (0, 1, 0),
        "{default_deferred:?}"
    );
    assert_eq!(
        active_source(&pool, pair).await?,
        ("strategy".to_owned(), manual_generation)
    );
    assert_eq!(redis.get::<_, String>(&ticker_key).await?, manual_ticker);
    assert_eq!(root(&collection, minute(manual_time)).await?, manual_root);
    assert_eq!(archive_count(&pool, &symbol).await?, 2);
    eprintln!(
        "Manual source committed at {manual_time}; same-minute default resumption correctly deferred"
    );

    let resumed_time = next_real_minute(manual_time).await;
    let resumed = run_once_with_dependencies(
        &pool,
        &mongo,
        &ingestion,
        resumed_time,
        10,
        "transition-default-b",
    )
    .await?;
    assert_eq!((resumed.published, resumed.failed), (1, 0), "{resumed:?}");
    let (source, resumed_generation) = active_source(&pool, pair).await?;
    assert_eq!(source, "default");
    assert!(resumed_generation > manual_generation);
    let resumed_candle: Value = serde_json::from_str(&redis.get::<_, String>(&candle_key).await?)?;
    assert_eq!(
        decimal(resumed_candle["open"].as_str().unwrap()),
        manual_price,
        "default resumes from actual last manual archive rather than its configured 0.1 start"
    );
    assert_eq!(root(&collection, minute(default_time)).await?, first_root);
    assert_eq!(root(&collection, minute(manual_time)).await?, manual_root);
    assert_eq!(collection.count_documents(doc! {"interval":"1m"}).await?, 3);
    assert_eq!(archive_count(&pool, &symbol).await?, 3);
    let latest:(String,Option<u64>,Option<i32>,u64)=sqlx::query_as("SELECT source,strategy_id,strategy_version,generation FROM market_price_ticks WHERE symbol=? ORDER BY observed_at DESC,id DESC LIMIT 1")
        .bind(&symbol).fetch_one(&pool).await?;
    assert_eq!(
        latest,
        ("default".to_owned(), None, None, resumed_generation)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM spot_trades")
            .fetch_one(&pool)
            .await?,
        0
    );
    eprintln!(
        "Default resumed at {resumed_time}; archive identity, price continuity and historical preservation verified"
    );

    let before_pause: String = redis.get(&ticker_key).await?;
    sqlx::query("UPDATE market_default_generators SET all_market_paused=TRUE WHERE pair_id=?")
        .bind(pair)
        .execute(&pool)
        .await?;
    let stopped = run_once_with_dependencies(
        &pool,
        &mongo,
        &ingestion,
        Utc::now(),
        10,
        "transition-default-b",
    )
    .await?;
    assert_eq!(
        (stopped.scanned, stopped.published, stopped.failed),
        (0, 0, 0),
        "{stopped:?}"
    );
    assert_eq!(redis.get::<_, String>(&ticker_key).await?, before_pause);
    assert_eq!(archive_count(&pool, &symbol).await?, 3);

    let keys: Vec<String> = redis.keys(format!("market:*{symbol}*")).await?;
    if !keys.is_empty() {
        let _: i64 = redis.del(keys).await?;
    }
    mongo.drop().await?;
    sqlx::query("DELETE FROM market_price_ticks WHERE symbol=?")
        .bind(&symbol)
        .execute(&pool)
        .await?;
    for id in [expired, manual] {
        sqlx::query("DELETE FROM strategy_runs WHERE strategy_id=?")
            .bind(id)
            .execute(&pool)
            .await?;
        sqlx::query("DELETE FROM strategy_versions WHERE strategy_id=?")
            .bind(id)
            .execute(&pool)
            .await?;
        sqlx::query("DELETE FROM market_strategies WHERE id=?")
            .bind(id)
            .execute(&pool)
            .await?;
    }
    for table in [
        "market_pair_generation_runs",
        "market_default_generator_versions",
        "market_default_generators",
    ] {
        sqlx::query(&format!("DELETE FROM {table} WHERE pair_id=?"))
            .bind(pair)
            .execute(&pool)
            .await?;
    }
    sqlx::query("DELETE FROM trading_pairs WHERE id=?")
        .bind(pair)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id IN (?,?)")
        .bind(assets[0])
        .bind(assets[1])
        .execute(&pool)
        .await?;
    pool.close().await;
    Ok(())
}
