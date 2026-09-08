use std::{error::Error, str::FromStr};

use bigdecimal::BigDecimal;
use chrono::{Timelike, Utc};
use exchange_api::{
    modules::market::{
        MarketDataProvider, MarketTickerSnapshot, RedisMarketCache,
        adapters::{DefaultTickerProvenance, MarketIngestionService, PairTickerFence},
        infrastructure::default_runtime::PairGenerationLock,
        sanitize_symbol,
    },
    workers::synthetic_market::run_once_with_dependencies,
};
use mongodb::bson::{Document, doc};
use redis::AsyncCommands;
use serde_json::{Value, json};
use sqlx::mysql::MySqlPoolOptions;
use uuid::Uuid;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

#[tokio::test]
async fn default_runtime_publishes_without_strategy_replays_restart_and_fences_pause()
-> Result<(), Box<dyn Error>> {
    let (Ok(mysql_url), Ok(mongo_url), Ok(redis_url)) = (
        std::env::var("DATABASE_URL"),
        std::env::var("DEFAULT_MARKET_TEST_MONGO_URL"),
        std::env::var("DEFAULT_MARKET_TEST_REDIS_URL"),
    ) else {
        eprintln!(
            "skipping default runtime integration: isolated DATABASE_URL and DEFAULT_MARKET_TEST_MONGO_URL/REDIS_URL are required"
        );
        return Ok(());
    };
    assert!(mongo_url.starts_with("mongodb://127.0.0.1:"));
    assert!(redis_url.starts_with("redis://127.0.0.1:"));
    let pool = MySqlPoolOptions::new()
        .max_connections(12)
        .connect(&mysql_url)
        .await?;
    let database: String = sqlx::query_scalar("SELECT DATABASE()")
        .fetch_one(&pool)
        .await?;
    assert!(
        database.starts_with("codex_default_runtime_"),
        "requires disposable runtime-only database"
    );
    sqlx::migrate!().run(&pool).await?;
    let suffix = &Uuid::now_v7().simple().to_string()[16..24];
    let mut assets = Vec::new();
    for prefix in ["DRB", "DRQ"] {
        assets.push(sqlx::query("INSERT INTO assets(symbol,name,precision_scale,asset_type,status) VALUES(?,?,6,'coin','active')")
            .bind(format!("{prefix}{suffix}")).bind(format!("{prefix} runtime fixture")).execute(&pool).await?.last_insert_id());
    }
    let raw_symbol = format!("DR{suffix}-USDT");
    let symbol = sanitize_symbol(&raw_symbol);
    let pair=sqlx::query("INSERT INTO trading_pairs(base_asset,quote_asset,symbol,price_precision,qty_precision,min_order_value,status,market_type) VALUES(?,?,?,6,2,1,'active','strategy')")
        .bind(assets[0]).bind(assets[1]).bind(&raw_symbol).execute(&pool).await?.last_insert_id();
    sqlx::query("INSERT INTO market_default_generators(pair_id,enabled,initial_price,config_json,version,seed) VALUES(?,TRUE,0.1,?,1,'runtime-seed')")
        .bind(pair).bind(sqlx::types::Json(json!({"volume_min":"60","volume_max":"60"}))).execute(&pool).await?;
    sqlx::query("INSERT INTO market_default_generator_versions(pair_id,version,initial_price,config_json,seed) SELECT pair_id,version,initial_price,config_json,seed FROM market_default_generators WHERE pair_id=?")
        .bind(pair).execute(&pool).await?;
    let mongo_client = mongodb::Client::with_uri_str(mongo_url).await?;
    let mongo = mongo_client.database(&format!("codex_default_runtime_{suffix}"));
    let redis = redis::Client::open(redis_url)?
        .get_connection_manager()
        .await?;
    let mut connection = redis.clone();
    let ingestion = MarketIngestionService::new(RedisMarketCache::new(redis), mongo.clone())
        .with_mysql(Some(pool.clone()));
    let now = Utc::now().with_nanosecond(0).unwrap();
    let first =
        run_once_with_dependencies(&pool, &mongo, &ingestion, now, 10, "runtime-owner-a").await?;
    assert_eq!(
        (first.scanned, first.published, first.failed),
        (1, 1, 0),
        "{first:?}"
    );
    let ticker_key = format!("market:ticker:{symbol}");
    let ticker_raw: String = connection.get(&ticker_key).await?;
    let ticker: Value = serde_json::from_str(&ticker_raw)?;
    assert!(decimal(ticker["last_price"].as_str().unwrap()) > decimal("0"));
    let candle_key = format!("market:kline:{symbol}:1m");
    let candle: Value = serde_json::from_str(&connection.get::<_, String>(&candle_key).await?)?;
    assert_eq!(ticker["last_price"], candle["close"]);
    assert_eq!(candle["open"].as_str().map(decimal), Some(decimal("0.1")));
    let trades_key = format!("market:synthetic-trades:{symbol}");
    let trade_count: i64 = connection.llen(&trades_key).await?;
    assert_eq!(trade_count, 1);
    let trade: Value =
        serde_json::from_str(&connection.lindex::<_, String>(&trades_key, 0).await?)?;
    assert!(
        trade["trade_id"]
            .as_str()
            .unwrap()
            .starts_with(&format!("default:{pair}:v1:"))
    );
    let depth: Value = serde_json::from_str(
        &connection
            .get::<_, String>(format!("market:depth:{symbol}"))
            .await?,
    )?;
    assert!(
        decimal(depth["bids"][0]["price"].as_str().unwrap())
            < decimal(depth["asks"][0]["price"].as_str().unwrap())
    );
    for interval in ["5m", "15m", "1h", "4h", "1d"] {
        assert!(
            connection
                .exists::<_, bool>(format!("market:kline:{symbol}:{interval}"))
                .await?
        );
    }
    let collection = mongo.collection::<Document>(&format!("market_klines_{symbol}"));
    assert_eq!(collection.count_documents(doc! {"interval":"1m"}).await?, 1);
    let count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM market_price_ticks WHERE symbol=? AND source='default' AND strategy_id IS NULL AND strategy_version IS NULL").bind(&symbol).fetch_one(&pool).await?;
    assert_eq!(count, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM spot_trades")
            .fetch_one(&pool)
            .await?,
        0
    );

    // 同一事件跨进程 owner 重放沿用固定分钟和代际，不制造第二笔成交/历史。
    let state_before: Value = sqlx::query_scalar::<_, sqlx::types::Json<Value>>(
        "SELECT state_json FROM market_pair_generation_runs WHERE pair_id=?",
    )
    .bind(pair)
    .fetch_one(&pool)
    .await?
    .0;
    let restart =
        run_once_with_dependencies(&pool, &mongo, &ingestion, now, 10, "runtime-owner-b").await?;
    assert_eq!(restart.published, 1, "{restart:?}");
    assert_eq!(connection.get::<_, String>(&ticker_key).await?, ticker_raw);
    assert_eq!(connection.llen::<_, i64>(&trades_key).await?, trade_count);
    assert_eq!(collection.count_documents(doc! {"interval":"1m"}).await?, 1);
    assert_eq!(
        sqlx::query_scalar::<_, sqlx::types::Json<Value>>(
            "SELECT state_json FROM market_pair_generation_runs WHERE pair_id=?"
        )
        .bind(pair)
        .fetch_one(&pool)
        .await?
        .0,
        state_before
    );

    let lock = PairGenerationLock::acquire(&pool, pair, 0).await?.unwrap();
    assert!(PairGenerationLock::acquire(&pool, pair, 0).await?.is_none());
    let locked =
        run_once_with_dependencies(&pool, &mongo, &ingestion, now, 10, "runtime-owner-c").await?;
    assert_eq!((locked.skipped, locked.published), (1, 0));
    lock.release().await?;

    let generation: u64 =
        sqlx::query_scalar("SELECT generation FROM market_pair_generation_runs WHERE pair_id=?")
            .bind(pair)
            .fetch_one(&pool)
            .await?;
    let forged = DefaultTickerProvenance {
        fence: PairTickerFence {
            pair_id: pair,
            generation,
            lock_name: "not-owned".into(),
            connection_id: 0,
            owner: "runtime-owner-b".into(),
        },
        config_version: 1,
    };
    let fake = MarketTickerSnapshot::new(
        MarketDataProvider::Strategy,
        &symbol,
        decimal("0.2"),
        decimal("1"),
        now,
    )?;
    assert!(
        ingestion
            .ingest_and_publish_default_ticker(&fake, &forged)
            .await
            .is_err()
    );
    assert_eq!(connection.get::<_, String>(&ticker_key).await?, ticker_raw);

    // 参数升级保留当前分钟旧版本；新版本不会在一分钟内重置开盘或重新抽随机数。
    sqlx::query("UPDATE market_default_generators SET version=2,config_json=? WHERE pair_id=?")
        .bind(sqlx::types::Json(
            json!({"volatility":"0.05","volume_min":"60","volume_max":"60"}),
        ))
        .bind(pair)
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO market_default_generator_versions(pair_id,version,initial_price,config_json,seed) SELECT pair_id,version,initial_price,config_json,seed FROM market_default_generators WHERE pair_id=?").bind(pair).execute(&pool).await?;
    let updated =
        run_once_with_dependencies(&pool, &mongo, &ingestion, now, 10, "runtime-owner-b").await?;
    assert_eq!(updated.published, 1, "{updated:?}");
    assert_eq!(
        sqlx::query_scalar::<_, u32>(
            "SELECT default_version FROM market_pair_generation_runs WHERE pair_id=?"
        )
        .bind(pair)
        .fetch_one(&pool)
        .await?,
        1
    );
    assert_eq!(connection.get::<_, String>(&ticker_key).await?, ticker_raw);

    sqlx::query("UPDATE market_default_generators SET all_market_paused=TRUE WHERE pair_id=?")
        .bind(pair)
        .execute(&pool)
        .await?;
    let paused =
        run_once_with_dependencies(&pool, &mongo, &ingestion, Utc::now(), 10, "runtime-owner-b")
            .await?;
    assert_eq!((paused.scanned, paused.published), (0, 0));
    assert_eq!(connection.get::<_, String>(&ticker_key).await?, ticker_raw);
    sqlx::query("UPDATE market_default_generators SET all_market_paused=FALSE WHERE pair_id=?")
        .bind(pair)
        .execute(&pool)
        .await?;

    // 停机后的当前分钟续接最后接受价，仅生成当前一根，不自动补造缺口。
    let current_minute = now.timestamp().div_euclid(60) * 60;
    let past = chrono::DateTime::from_timestamp(current_minute - 600, 0).unwrap();
    let mut old_state = state_before.clone();
    old_state["closed"]["open_time"] = json!(past.timestamp_millis());
    old_state["closed"]["observed_at"] =
        json!((past + chrono::TimeDelta::seconds(59)).timestamp_millis());
    sqlx::query("UPDATE market_pair_generation_runs SET state_json=?,last_tick_at=?,lease_owner='old-process' WHERE pair_id=?")
        .bind(sqlx::types::Json(old_state)).bind(past.naive_utc()).bind(pair).execute(&pool).await?;
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let new_now = Utc::now().with_nanosecond(0).unwrap();
    let continued =
        run_once_with_dependencies(&pool, &mongo, &ingestion, new_now, 10, "runtime-owner-d")
            .await?;
    assert_eq!(continued.published, 1, "{continued:?}");
    let continued_candle: Value =
        serde_json::from_str(&connection.get::<_, String>(&candle_key).await?)?;
    assert_eq!(
        continued_candle["open"].as_str().map(decimal),
        ticker["last_price"].as_str().map(decimal)
    );
    assert!(collection.count_documents(doc! {"interval":"1m"}).await? <= 2);
    assert_eq!(collection.count_documents(doc!{"interval":"1m","open_time":{"$lt":mongodb::bson::DateTime::from_millis(current_minute*1000)}}).await?,0);

    // 删除仅属于本 fixture 的缓存、集合和关系数据，整个测试库由调用方最终清理。
    let keys: Vec<String> = connection.keys(format!("market:*{symbol}*")).await?;
    if !keys.is_empty() {
        let _: i64 = connection.del(keys).await?;
    }
    mongo.drop().await?;
    sqlx::query("DELETE FROM market_price_ticks WHERE symbol=?")
        .bind(&symbol)
        .execute(&pool)
        .await?;
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
    Ok(())
}
