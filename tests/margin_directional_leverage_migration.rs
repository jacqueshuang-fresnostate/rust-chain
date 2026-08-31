use anyhow::{Context, Result, ensure};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use url::Url;
use uuid::Uuid;

const MIGRATION: &str = include_str!("../migrations/0120_margin_directional_leverage_settings.sql");

#[test]
fn directional_leverage_migration_is_additive_backfilled_and_constrained() {
    let uppercase = MIGRATION.to_ascii_uppercase();
    for destructive in ["DROP TABLE", "DROP COLUMN", "TRUNCATE TABLE", "DELETE FROM"] {
        assert!(
            !uppercase.contains(destructive),
            "0120 must remain additive and immutable: {destructive}"
        );
    }
    for required in [
        "ADD COLUMN long_leverage DECIMAL(18,8) NULL",
        "ADD COLUMN short_leverage DECIMAL(18,8) NULL",
        "SET long_leverage = leverage",
        "short_leverage = leverage",
        "chk_margin_user_settings_long_leverage",
        "CHECK (long_leverage IS NULL OR long_leverage > 0)",
        "chk_margin_user_settings_short_leverage",
        "CHECK (short_leverage IS NULL OR short_leverage > 0)",
    ] {
        assert!(
            MIGRATION.contains(required),
            "missing directional leverage migration contract: {required}"
        );
    }
}

#[tokio::test]
async fn directional_leverage_migration_backfills_legacy_rows_and_enforces_positive_values() {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!(
                "skipping directional leverage migration test because DATABASE_URL is not set"
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
        .expect("connect to MySQL server for isolated directional leverage migration test");
    let database_name = format!("margin_leverage_{}", Uuid::now_v7().simple());
    if let Err(error) = sqlx::query(&format!("CREATE DATABASE {database_name}"))
        .execute(&server_pool)
        .await
    {
        eprintln!(
            "skipping directional leverage migration test because an isolated database cannot be created: {error}"
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
        .expect("connect to isolated directional leverage migration database");

    let exercise_result = exercise_migration_contract(&pool).await;
    pool.close().await;
    let cleanup_result = sqlx::query(&format!("DROP DATABASE {database_name}"))
        .execute(&server_pool)
        .await
        .context("drop isolated directional leverage migration database");
    server_pool.close().await;

    exercise_result.expect("directional leverage migration contract");
    cleanup_result.expect("directional leverage migration database cleanup");
}

async fn exercise_migration_contract(pool: &MySqlPool) -> Result<()> {
    sqlx::raw_sql(
        r#"CREATE TABLE margin_user_settings (
               id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
               user_id BIGINT UNSIGNED NOT NULL,
               product_id BIGINT UNSIGNED NOT NULL,
               margin_mode VARCHAR(16) NULL,
               leverage DECIMAL(18,8) NULL,
               UNIQUE KEY uq_margin_user_settings_user_product (user_id, product_id),
               CONSTRAINT chk_margin_user_settings_leverage
                   CHECK (leverage IS NULL OR leverage > 0)
           );
           INSERT INTO margin_user_settings (user_id, product_id, leverage) VALUES
               (1, 11, 3.00000000),
               (2, 11, NULL);"#,
    )
    .execute(pool)
    .await
    .context("create and seed pre-0120 margin user settings schema")?;

    sqlx::raw_sql(MIGRATION)
        .execute(pool)
        .await
        .context("execute exact 0120 directional leverage migration")?;

    let rows = sqlx::query_as::<_, (u64, Option<String>, Option<String>, Option<String>)>(
        r#"SELECT id,
                  CAST(leverage AS CHAR),
                  CAST(long_leverage AS CHAR),
                  CAST(short_leverage AS CHAR)
           FROM margin_user_settings
           ORDER BY id"#,
    )
    .fetch_all(pool)
    .await?;
    ensure!(
        rows == vec![
            (
                1,
                Some("3.00000000".to_owned()),
                Some("3.00000000".to_owned()),
                Some("3.00000000".to_owned()),
            ),
            (2, None, None, None),
        ],
        "legacy leverage must backfill both directional columns exactly"
    );

    let columns = sqlx::query_as::<_, (String, String, String)>(
        r#"SELECT CAST(COLUMN_NAME AS CHAR), CAST(COLUMN_TYPE AS CHAR), CAST(IS_NULLABLE AS CHAR)
           FROM information_schema.COLUMNS
           WHERE TABLE_SCHEMA = DATABASE()
             AND TABLE_NAME = 'margin_user_settings'
             AND COLUMN_NAME IN ('long_leverage', 'short_leverage')
           ORDER BY ORDINAL_POSITION"#,
    )
    .fetch_all(pool)
    .await?;
    ensure!(
        columns
            == vec![
                (
                    "long_leverage".to_owned(),
                    "decimal(18,8)".to_owned(),
                    "YES".to_owned(),
                ),
                (
                    "short_leverage".to_owned(),
                    "decimal(18,8)".to_owned(),
                    "YES".to_owned(),
                ),
            ]
    );

    sqlx::query(
        "INSERT INTO margin_user_settings (user_id, product_id, margin_mode) VALUES (3, 11, 'isolated')",
    )
    .execute(pool)
    .await
    .context("directional leverage columns must remain nullable")?;

    let invalid_long = sqlx::query(
        "UPDATE margin_user_settings SET leverage = 1, long_leverage = -1, short_leverage = 1 WHERE id = 1",
    )
    .execute(pool)
    .await;
    ensure!(
        invalid_long.is_err(),
        "negative long leverage must be rejected"
    );
    let invalid_short = sqlx::query(
        "UPDATE margin_user_settings SET leverage = 1, long_leverage = 1, short_leverage = -1 WHERE id = 1",
    )
    .execute(pool)
    .await;
    ensure!(
        invalid_short.is_err(),
        "negative short leverage must be rejected"
    );
    let zero_long = sqlx::query(
        "UPDATE margin_user_settings SET leverage = 1, long_leverage = 0, short_leverage = 1 WHERE id = 1",
    )
    .execute(pool)
    .await;
    ensure!(zero_long.is_err(), "zero long leverage must be rejected");
    let zero_short = sqlx::query(
        "UPDATE margin_user_settings SET leverage = 1, long_leverage = 1, short_leverage = 0 WHERE id = 1",
    )
    .execute(pool)
    .await;
    ensure!(zero_short.is_err(), "zero short leverage must be rejected");

    let unchanged = sqlx::query_as::<_, (String, String, String)>(
        r#"SELECT CAST(leverage AS CHAR), CAST(long_leverage AS CHAR), CAST(short_leverage AS CHAR)
           FROM margin_user_settings
           WHERE id = 1"#,
    )
    .fetch_one(pool)
    .await?;
    ensure!(
        unchanged
            == (
                "3.00000000".to_owned(),
                "3.00000000".to_owned(),
                "3.00000000".to_owned(),
            ),
        "failed constraint writes must leave the backfilled row unchanged"
    );
    Ok(())
}
