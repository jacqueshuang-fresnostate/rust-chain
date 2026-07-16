use bigdecimal::BigDecimal;
use chrono::{TimeDelta, Utc};
use exchange_api::modules::convert::{
    ConvertConfirmationInsert, ConvertQuoteCacheEntry, ConvertQuoteInsert, MySqlConvertRepository,
    QuoteId, RedisConvertQuoteCache,
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
async fn redis_quote_ttl_cache_stores_expected_json_shape() -> Result<(), Box<dyn Error>> {
    let Some(redis_url) = env_or_skip("REDIS_URL") else {
        return Ok(());
    };
    let client = redis::Client::open(redis_url)?;
    let manager = redis::aio::ConnectionManager::new(client).await?;
    let repository = RedisConvertQuoteCache::new(manager.clone());
    let quote_id = QuoteId(Uuid::now_v7());
    let redis_key = format!("convert:quote:{}", quote_id.0);
    let expires_at = Utc::now() + TimeDelta::seconds(30);
    let entry = ConvertQuoteCacheEntry {
        quote_id: quote_id.clone(),
        user_id: "1001".to_owned(),
        from_asset: "11".to_owned(),
        to_asset: "12".to_owned(),
        from_amount: decimal("25.500000000000000000"),
        to_amount: decimal("51.000000000000000000"),
        fee_rate: decimal("0.00000000"),
        fee_amount: decimal("0.000000000000000000"),
        expires_at: chrono::DateTime::from_timestamp_millis(expires_at.timestamp_millis()).unwrap(),
        redis_key: redis_key.clone(),
        ttl_seconds: 30,
    };

    repository.save_quote_ttl(entry.clone()).await.unwrap();

    let mut raw_connection = manager.clone();
    let payload: String = raw_connection.get(&redis_key).await?;
    let payload_json: serde_json::Value = serde_json::from_str(&payload)?;

    assert_eq!(payload_json["quote_id"], quote_id.0.to_string());
    assert_eq!(payload_json["user_id"], "1001");
    assert_eq!(payload_json["from_asset"], "11");
    assert_eq!(payload_json["to_asset"], "12");
    assert!(payload_json["from_amount"].is_string());
    assert!(payload_json["to_amount"].is_string());
    assert_eq!(
        payload_json["expires_at"],
        entry.expires_at.timestamp_millis()
    );
    assert_eq!(payload_json["redis_key"], redis_key);
    assert_eq!(payload_json["ttl_seconds"], 30);

    let ttl: i64 = redis::cmd("TTL")
        .arg(&redis_key)
        .query_async(&mut raw_connection)
        .await?;
    assert!((1..=30).contains(&ttl), "unexpected Redis TTL: {ttl}");
    assert_eq!(
        repository.get_quote_ttl(&quote_id).await.unwrap(),
        Some(entry)
    );

    let _: usize = raw_connection.del(&redis_key).await?;
    Ok(())
}

#[tokio::test]
async fn mysql_convert_order_insert_is_idempotent_by_quote_id() -> Result<(), Box<dyn Error>> {
    let Some(database_url) = env_or_skip("DATABASE_URL") else {
        return Ok(());
    };
    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    let suffix = Uuid::now_v7().simple().to_string();
    let email = format!("convert-{suffix}@example.test");
    let from_symbol = format!("F{}", &suffix[..12]);
    let to_symbol = format!("T{}", &suffix[12..24]);
    let quote_id = QuoteId(Uuid::now_v7());
    let quote_id_value = quote_id.0.to_string();

    let user_id = sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(&email)
        .bind("test-password-hash")
        .execute(&pool)
        .await?
        .last_insert_id();
    let from_asset_id = insert_asset(&pool, &from_symbol).await?;
    let to_asset_id = insert_asset(&pool, &to_symbol).await?;
    let convert_pair_id = sqlx::query(
        r#"INSERT INTO convert_pairs
           (from_asset, to_asset, pricing_mode, spread_rate, min_amount, max_amount, enabled)
           VALUES (?, ?, ?, ?, ?, NULL, ?)"#,
    )
    .bind(from_asset_id)
    .bind(to_asset_id)
    .bind("fixed")
    .bind(decimal("0.00000000"))
    .bind(decimal("0.000000000000000000"))
    .bind(true)
    .execute(&pool)
    .await?
    .last_insert_id();

    let repository = MySqlConvertRepository::new(pool.clone());
    repository
        .insert_quote(ConvertQuoteInsert {
            quote_id: quote_id.clone(),
            convert_pair_id,
            user_id,
            from_asset_id,
            to_asset_id,
            from_amount: decimal("25.500000000000000000"),
            to_amount: decimal("51.000000000000000000"),
            rate: decimal("2.000000000000000000"),
            spread_rate: decimal("0.00000000"),
            fee_rate: decimal("0.00000000"),
            fee_amount: decimal("0.000000000000000000"),
            expires_at: Utc::now() + TimeDelta::seconds(60),
        })
        .await
        .unwrap();

    let first_insert = repository.insert_order_for_quote(&quote_id).await.unwrap();
    let duplicate_insert = repository.insert_order_for_quote(&quote_id).await.unwrap();

    assert_eq!(first_insert, ConvertConfirmationInsert::Inserted);
    assert_eq!(duplicate_insert, ConvertConfirmationInsert::Duplicate);
    let (order_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM convert_orders WHERE quote_id = ?")
            .bind(&quote_id_value)
            .fetch_one(&pool)
            .await?;
    assert_eq!(order_count, 1);

    cleanup_convert_fixture(
        &pool,
        &quote_id_value,
        convert_pair_id,
        from_asset_id,
        to_asset_id,
        user_id,
    )
    .await?;
    Ok(())
}

async fn insert_asset(pool: &sqlx::Pool<sqlx::MySql>, symbol: &str) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query("INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, ?, ?, ?)")
        .bind(symbol)
        .bind(symbol)
        .bind(18_i32)
        .bind("coin")
        .bind("active")
        .execute(pool)
        .await?
        .last_insert_id())
}

async fn cleanup_convert_fixture(
    pool: &sqlx::Pool<sqlx::MySql>,
    quote_id: &str,
    convert_pair_id: u64,
    from_asset_id: u64,
    to_asset_id: u64,
    user_id: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM convert_orders WHERE quote_id = ?")
        .bind(quote_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM convert_quotes WHERE quote_id = ?")
        .bind(quote_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM convert_pairs WHERE id = ?")
        .bind(convert_pair_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(from_asset_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(to_asset_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
