use bigdecimal::BigDecimal;
use chrono::{DateTime, TimeZone, Utc};
use exchange_api::{
    infra::mongo::kline_collection_name, modules::market::ValidatedMarketSymbol,
    workers::kline_recovery::run_once_with_dependencies,
};
use mongodb::{
    Client, Database,
    bson::{DateTime as BsonDateTime, Document, doc},
};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::{error::Error, str::FromStr};
use uuid::Uuid;

fn env_or_skip(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => {
            eprintln!("skipping integration test because {name} is not set");
            None
        }
    }
}

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

fn unique_symbol(prefix: &str) -> String {
    let uuid = Uuid::now_v7().simple().to_string();
    format!("{}{}", prefix, &uuid[22..32]).to_ascii_uppercase()
}

async fn mysql_pool_or_skip() -> Result<Option<MySqlPool>, Box<dyn Error>> {
    let Some(database_url) = env_or_skip("DATABASE_URL") else {
        return Ok(None);
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    Ok(Some(pool))
}

async fn mongo_database_or_skip() -> Result<Option<Database>, Box<dyn Error>> {
    let Some(mongodb_uri) = env_or_skip("MONGODB_URI") else {
        return Ok(None);
    };
    let database = std::env::var("MONGODB_DATABASE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "exchange_market".to_owned());
    let client = Client::with_uri_str(&mongodb_uri).await?;
    Ok(Some(client.database(&database)))
}

async fn create_asset(pool: &MySqlPool, symbol: &str) -> Result<u64, Box<dyn Error>> {
    let result = sqlx::query(
        "INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, 8, 'coin', 'active')",
    )
    .bind(symbol)
    .bind(format!("{symbol} asset"))
    .execute(pool)
    .await?;
    Ok(result.last_insert_id())
}

async fn create_strategy_fixture(
    pool: &MySqlPool,
    checkpoint: DateTime<Utc>,
) -> Result<(u64, String), Box<dyn Error>> {
    let base_symbol = unique_symbol("KB");
    let quote_symbol = unique_symbol("KQ");
    let pair_symbol = format!("{base_symbol}-{quote_symbol}");
    let base_asset_id = create_asset(pool, &base_symbol).await?;
    let quote_asset_id = create_asset(pool, &quote_symbol).await?;
    let pair_id = sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, 8, 8, 1, 'active', 'strategy')"#,
    )
    .bind(base_asset_id)
    .bind(quote_asset_id)
    .bind(&pair_symbol)
    .execute(pool)
    .await?
    .last_insert_id();
    let strategy_id = sqlx::query(
        r#"INSERT INTO market_strategies
           (pair_id, strategy_type, start_price, target_price, start_time, end_time,
            volatility, volume_min, volume_max, status)
           VALUES (?, 'linear', ?, ?, ?, ?, ?, ?, ?, 'active')"#,
    )
    .bind(pair_id)
    .bind(decimal("1.000000000000000000"))
    .bind(decimal("1.060000000000000000"))
    .bind(checkpoint.naive_utc())
    .bind((checkpoint + chrono::TimeDelta::hours(1)).naive_utc())
    .bind(decimal("0.01000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(decimal("200.000000000000000000"))
    .execute(pool)
    .await?
    .last_insert_id();
    sqlx::query(
        r#"INSERT INTO strategy_runs
           (strategy_id, run_status, current_price, last_generated_at, last_kline_open_time, recovery_status)
           VALUES (?, 'running', ?, ?, ?, 'idle')"#,
    )
    .bind(strategy_id)
    .bind(decimal("1.000000000000000000"))
    .bind(checkpoint.naive_utc())
    .bind(checkpoint.naive_utc())
    .execute(pool)
    .await?;
    Ok((strategy_id, pair_symbol))
}

#[tokio::test]
async fn kline_recovery_backfills_mongo_and_updates_checkpoint_idempotently()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let Some(mongo) = mongo_database_or_skip().await? else {
        return Ok(());
    };
    let checkpoint = Utc.with_ymd_and_hms(2000, 1, 1, 10, 0, 0).unwrap();
    let now = checkpoint + chrono::TimeDelta::minutes(3) + chrono::TimeDelta::seconds(30);
    let last_closed = checkpoint + chrono::TimeDelta::minutes(2);
    let (strategy_id, pair_symbol) = create_strategy_fixture(&pool, checkpoint).await?;
    let symbol = ValidatedMarketSymbol::from_raw(&pair_symbol).unwrap();
    let collection_name = kline_collection_name(&symbol);
    mongo
        .collection::<Document>(&collection_name)
        .drop()
        .await
        .ok();

    let summary = run_once_with_dependencies(&pool, &mongo, now, 10).await?;

    assert_eq!(summary.scanned, 1);
    assert_eq!(summary.recovered_candles, 2);
    assert_eq!(summary.skipped, 0);
    assert_eq!(summary.failed, 0);
    let collection = mongo.collection::<Document>(&collection_name);
    assert_eq!(
        collection
            .count_documents(doc! { "interval": "1m" })
            .await?,
        2
    );
    let last_candle = collection
        .find_one(doc! {
            "interval": "1m",
            "open_time": BsonDateTime::from_millis(last_closed.timestamp_millis()),
        })
        .await?
        .expect("last recovered candle exists");
    assert_eq!(last_candle.get_str("close")?, "1.060000000000000000");
    let checkpoint_row = sqlx::query_as::<_, (DateTime<Utc>, String, String)>(
        r#"SELECT last_kline_open_time,
                  CAST(current_price AS CHAR) AS current_price,
                  recovery_status
           FROM strategy_runs
           WHERE strategy_id = ?"#,
    )
    .bind(strategy_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(checkpoint_row.0, last_closed);
    assert_eq!(checkpoint_row.1, "1.060000000000000000");
    assert_eq!(checkpoint_row.2, "live");

    let idempotent = run_once_with_dependencies(&pool, &mongo, now, 10).await?;

    assert_eq!(idempotent.scanned, 0);
    assert_eq!(idempotent.recovered_candles, 0);
    assert_eq!(
        collection
            .count_documents(doc! { "interval": "1m" })
            .await?,
        2
    );
    Ok(())
}
