use anyhow::{Context, Result, ensure};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use url::Url;
use uuid::Uuid;

const MIGRATION: &str = include_str!("../migrations/0103_margin_transfer_asset_config.sql");

#[test]
fn margin_transfer_asset_migration_declares_a_safe_default_and_targeted_backfill() {
    assert!(MIGRATION.contains("margin_transfer_enabled BOOLEAN NOT NULL DEFAULT FALSE"));
    assert!(MIGRATION.contains("FROM margin_products AS product"));
    assert!(MIGRATION.contains("FROM margin_wallet_accounts AS wallet"));
}

#[tokio::test]
async fn margin_transfer_asset_migration_backfills_only_existing_margin_usage() {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!(
                "skipping margin transfer asset migration test because DATABASE_URL is not set"
            );
            return;
        }
    };

    let mut server_url = Url::parse(&database_url).expect("DATABASE_URL must be a valid URL");
    server_url.set_path("/");
    let server_pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect(server_url.as_str())
        .await
        .expect("connect to MySQL server for isolated margin transfer migration test");
    let database_name = format!("margin_transfer_{}", Uuid::now_v7().simple());
    if let Err(error) = sqlx::query(&format!("CREATE DATABASE {database_name}"))
        .execute(&server_pool)
        .await
    {
        eprintln!(
            "skipping margin transfer asset migration test because an isolated database cannot be created: {error}"
        );
        server_pool.close().await;
        return;
    }

    let mut test_url = Url::parse(&database_url).expect("DATABASE_URL must be a valid URL");
    test_url.set_path(&format!("/{database_name}"));
    let pool = MySqlPoolOptions::new()
        .max_connections(2)
        .connect(test_url.as_str())
        .await
        .expect("connect to isolated margin transfer migration database");

    let exercise_result = exercise_migration_contract(&pool).await;
    pool.close().await;
    let cleanup_result = sqlx::query(&format!("DROP DATABASE {database_name}"))
        .execute(&server_pool)
        .await
        .context("drop isolated margin transfer migration database");
    server_pool.close().await;

    exercise_result.expect("margin transfer asset migration contract");
    cleanup_result.expect("margin transfer asset migration database cleanup");
}

async fn exercise_migration_contract(pool: &MySqlPool) -> Result<()> {
    sqlx::raw_sql(
        r#"CREATE TABLE assets (
               id BIGINT UNSIGNED PRIMARY KEY,
               symbol VARCHAR(32) NOT NULL,
               withdraw_enabled BOOLEAN NOT NULL DEFAULT TRUE
           );
           CREATE TABLE margin_products (
               id BIGINT UNSIGNED PRIMARY KEY,
               margin_asset BIGINT UNSIGNED NOT NULL
           );
           CREATE TABLE margin_wallet_accounts (
               id BIGINT UNSIGNED PRIMARY KEY,
               asset_id BIGINT UNSIGNED NOT NULL
           );
           INSERT INTO assets (id, symbol) VALUES
               (1, 'PRODUCT'), (2, 'WALLET'), (3, 'UNUSED');
           INSERT INTO margin_products (id, margin_asset) VALUES (1, 1);
           INSERT INTO margin_wallet_accounts (id, asset_id) VALUES (1, 2);"#,
    )
    .execute(pool)
    .await
    .context("create and seed pre-migration margin asset schema")?;

    sqlx::raw_sql(MIGRATION)
        .execute(pool)
        .await
        .context("execute exact margin transfer asset migration")?;

    let values = sqlx::query_as::<_, (u64, bool)>(
        "SELECT id, margin_transfer_enabled FROM assets ORDER BY id",
    )
    .fetch_all(pool)
    .await?;
    ensure!(values == vec![(1, true), (2, true), (3, false)]);

    sqlx::query("INSERT INTO assets (id, symbol) VALUES (4, 'NEW')")
        .execute(pool)
        .await?;
    let default_value: bool =
        sqlx::query_scalar("SELECT margin_transfer_enabled FROM assets WHERE id = 4")
            .fetch_one(pool)
            .await?;
    ensure!(!default_value);

    let metadata: (String, String) = sqlx::query_as(
        r#"SELECT CAST(COLUMN_DEFAULT AS CHAR), COLUMN_COMMENT
           FROM information_schema.COLUMNS
           WHERE TABLE_SCHEMA = DATABASE()
             AND TABLE_NAME = 'assets'
             AND COLUMN_NAME = 'margin_transfer_enabled'"#,
    )
    .fetch_one(pool)
    .await?;
    ensure!(metadata.0 == "0");
    ensure!(metadata.1 == "是否允许用户从现货账户转入杠杆账户");
    Ok(())
}
