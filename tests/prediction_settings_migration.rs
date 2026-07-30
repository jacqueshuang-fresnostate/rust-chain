use anyhow::{Context, Result, ensure};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::collections::BTreeMap;
use url::Url;
use uuid::Uuid;

const TEXT_METADATA_MIGRATION: &str =
    include_str!("../migrations/0097_prediction_settings_text_metadata.sql");

#[derive(Debug, sqlx::FromRow)]
struct ColumnMetadata {
    column_name: String,
    data_type: String,
    character_set_name: Option<String>,
    collation_name: Option<String>,
    is_nullable: String,
    column_default: Option<String>,
    character_maximum_length: Option<i64>,
}

#[tokio::test]
async fn prediction_settings_binary_metadata_drift_is_repaired_for_sqlx_strings() {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!(
                "skipping prediction settings migration test because DATABASE_URL is not set"
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
        .expect("connect to MySQL server for isolated prediction settings migration test");
    let database_name = format!("pred_text_meta_{}", Uuid::now_v7().simple());
    let create_database = format!(
        "CREATE DATABASE `{database_name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
    );
    if let Err(error) = sqlx::query(&create_database).execute(&server_pool).await {
        eprintln!(
            "skipping prediction settings migration test because an isolated database cannot be created: {error}"
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
        .expect("connect to isolated prediction settings migration database");

    let exercise_result = exercise_migration_contract(&pool).await;
    pool.close().await;
    let drop_database = format!("DROP DATABASE `{database_name}`");
    let cleanup_result = sqlx::query(&drop_database)
        .execute(&server_pool)
        .await
        .context("drop isolated prediction settings migration database");
    server_pool.close().await;

    exercise_result.expect("prediction settings text metadata migration contract");
    cleanup_result.expect("prediction settings migration test database cleanup");
}

async fn exercise_migration_contract(pool: &MySqlPool) -> Result<()> {
    sqlx::raw_sql(
        r#"CREATE TABLE prediction_settings (
               id TINYINT UNSIGNED NOT NULL PRIMARY KEY,
               default_settlement_mode VARCHAR(32) NOT NULL DEFAULT 'manual_confirm',
               default_invalid_refund_policy VARCHAR(32) NOT NULL DEFAULT 'refund_stake_and_fee',
               last_sync_status VARCHAR(32) NULL,
               last_sync_error VARCHAR(512) NULL
           ) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"#,
    )
    .execute(pool)
    .await
    .context("create prediction_settings test table")?;

    let expected_values = (
        "auto_settle",
        "refund_stake_only",
        "failed",
        "同步失败：上游返回无效结果",
    );
    sqlx::query(
        r#"INSERT INTO prediction_settings (
               id,
               default_settlement_mode,
               default_invalid_refund_policy,
               last_sync_status,
               last_sync_error
           ) VALUES (1, ?, ?, ?, ?)"#,
    )
    .bind(expected_values.0)
    .bind(expected_values.1)
    .bind(expected_values.2)
    .bind(expected_values.3)
    .execute(pool)
    .await
    .context("insert prediction settings values before drift")?;

    sqlx::raw_sql(
        r#"ALTER TABLE prediction_settings
               MODIFY COLUMN default_settlement_mode VARBINARY(32)
                   NOT NULL DEFAULT 'manual_confirm',
               MODIFY COLUMN default_invalid_refund_policy VARBINARY(32)
                   NOT NULL DEFAULT 'refund_stake_and_fee',
               MODIFY COLUMN last_sync_status VARBINARY(32) NULL DEFAULT NULL,
               MODIFY COLUMN last_sync_error VARBINARY(512) NULL DEFAULT NULL"#,
    )
    .execute(pool)
    .await
    .context("reproduce prediction settings VARBINARY drift")?;

    assert_binary_values(pool, expected_values).await?;
    assert_string_decode_failure(pool, "VARBINARY").await?;

    sqlx::raw_sql(TEXT_METADATA_MIGRATION)
        .execute(pool)
        .await
        .context("execute exact prediction settings text metadata migration SQL")?;

    assert_varchar_metadata(pool, "utf8mb4_unicode_ci").await?;
    assert_preserved_values(pool, expected_values).await?;

    sqlx::query("INSERT INTO prediction_settings (id) VALUES (2)")
        .execute(pool)
        .await
        .context("insert prediction settings row using migrated defaults")?;
    assert_defaults_and_nulls(pool).await?;

    sqlx::raw_sql(
        r#"ALTER TABLE prediction_settings
               MODIFY COLUMN default_settlement_mode VARCHAR(32)
                   CHARACTER SET utf8mb4 COLLATE utf8mb4_bin
                   NOT NULL DEFAULT 'manual_confirm',
               MODIFY COLUMN default_invalid_refund_policy VARCHAR(32)
                   CHARACTER SET utf8mb4 COLLATE utf8mb4_bin
                   NOT NULL DEFAULT 'refund_stake_and_fee',
               MODIFY COLUMN last_sync_status VARCHAR(32)
                   CHARACTER SET utf8mb4 COLLATE utf8mb4_bin
                   NULL DEFAULT NULL,
               MODIFY COLUMN last_sync_error VARCHAR(512)
                   CHARACTER SET utf8mb4 COLLATE utf8mb4_bin
                   NULL DEFAULT NULL"#,
    )
    .execute(pool)
    .await
    .context("reproduce prediction settings binary-collated VARCHAR drift")?;

    assert_varchar_metadata(pool, "utf8mb4_bin").await?;
    assert_binary_values(pool, expected_values).await?;
    assert_string_decode_failure(pool, "binary-collated VARCHAR").await?;

    sqlx::raw_sql(TEXT_METADATA_MIGRATION)
        .execute(pool)
        .await
        .context("repair binary-collated VARCHAR metadata with exact migration SQL")?;
    assert_varchar_metadata(pool, "utf8mb4_unicode_ci").await?;
    assert_preserved_values(pool, expected_values).await?;
    assert_defaults_and_nulls(pool).await?;

    sqlx::raw_sql(TEXT_METADATA_MIGRATION)
        .execute(pool)
        .await
        .context("execute migration SQL against already-correct VARCHAR metadata")?;
    assert_varchar_metadata(pool, "utf8mb4_unicode_ci").await?;
    assert_preserved_values(pool, expected_values).await?;
    assert_defaults_and_nulls(pool).await?;

    Ok(())
}

async fn assert_binary_values(pool: &MySqlPool, expected: (&str, &str, &str, &str)) -> Result<()> {
    let actual: (Vec<u8>, Vec<u8>, Option<Vec<u8>>, Option<Vec<u8>>) = sqlx::query_as(
        r#"SELECT default_settlement_mode,
                  default_invalid_refund_policy,
                  last_sync_status,
                  last_sync_error
           FROM prediction_settings
           WHERE id = 1"#,
    )
    .fetch_one(pool)
    .await
    .context("read drifted prediction settings as binary values")?;
    ensure!(actual.0 == expected.0.as_bytes());
    ensure!(actual.1 == expected.1.as_bytes());
    ensure!(actual.2.as_deref() == Some(expected.2.as_bytes()));
    ensure!(actual.3.as_deref() == Some(expected.3.as_bytes()));
    Ok(())
}

async fn assert_string_decode_failure(pool: &MySqlPool, drift: &str) -> Result<()> {
    let decode_error = sqlx::query_scalar::<_, String>(
        "SELECT default_settlement_mode FROM prediction_settings WHERE id = 1",
    )
    .fetch_one(pool)
    .await
    .expect_err("binary metadata must reproduce SQLx String decoding failure");
    ensure!(
        matches!(decode_error, sqlx::Error::ColumnDecode { .. }),
        "expected SQLx column decode failure for {drift}, got {decode_error:?}"
    );
    Ok(())
}

async fn assert_preserved_values(
    pool: &MySqlPool,
    expected: (&str, &str, &str, &str),
) -> Result<()> {
    let actual: (String, String, Option<String>, Option<String>) = sqlx::query_as(
        r#"SELECT default_settlement_mode,
                  default_invalid_refund_policy,
                  last_sync_status,
                  last_sync_error
           FROM prediction_settings
           WHERE id = 1"#,
    )
    .fetch_one(pool)
    .await
    .context("decode migrated prediction settings as Rust strings")?;

    ensure!(actual.0 == expected.0);
    ensure!(actual.1 == expected.1);
    ensure!(actual.2.as_deref() == Some(expected.2));
    ensure!(actual.3.as_deref() == Some(expected.3));
    Ok(())
}

async fn assert_defaults_and_nulls(pool: &MySqlPool) -> Result<()> {
    let actual: (String, String, Option<String>, Option<String>) = sqlx::query_as(
        r#"SELECT default_settlement_mode,
                  default_invalid_refund_policy,
                  last_sync_status,
                  last_sync_error
           FROM prediction_settings
           WHERE id = 2"#,
    )
    .fetch_one(pool)
    .await
    .context("decode migrated defaults and nullable fields as Rust strings")?;

    ensure!(actual.0 == "manual_confirm");
    ensure!(actual.1 == "refund_stake_and_fee");
    ensure!(actual.2.is_none());
    ensure!(actual.3.is_none());
    Ok(())
}

async fn assert_varchar_metadata(pool: &MySqlPool, expected_collation: &str) -> Result<()> {
    let rows = sqlx::query_as::<_, ColumnMetadata>(
        r#"SELECT CAST(COLUMN_NAME AS CHAR(64) CHARACTER SET utf8mb4) AS column_name,
                  CAST(DATA_TYPE AS CHAR(64) CHARACTER SET utf8mb4) AS data_type,
                  CAST(CHARACTER_SET_NAME AS CHAR(64) CHARACTER SET utf8mb4)
                      AS character_set_name,
                  CAST(COLLATION_NAME AS CHAR(64) CHARACTER SET utf8mb4)
                      AS collation_name,
                  CAST(IS_NULLABLE AS CHAR(3) CHARACTER SET utf8mb4) AS is_nullable,
                  CAST(COLUMN_DEFAULT AS CHAR(512) CHARACTER SET utf8mb4) AS column_default,
                  CAST(CHARACTER_MAXIMUM_LENGTH AS SIGNED) AS character_maximum_length
           FROM information_schema.COLUMNS
           WHERE TABLE_SCHEMA = DATABASE()
             AND TABLE_NAME = 'prediction_settings'
             AND COLUMN_NAME IN (
                 'default_settlement_mode',
                 'default_invalid_refund_policy',
                 'last_sync_status',
                 'last_sync_error'
             )"#,
    )
    .fetch_all(pool)
    .await
    .context("read migrated prediction settings column metadata")?
    .into_iter()
    .map(|row| (row.column_name.clone(), row))
    .collect::<BTreeMap<_, _>>();

    let expected = [
        ("default_settlement_mode", 32, "NO", Some("manual_confirm")),
        (
            "default_invalid_refund_policy",
            32,
            "NO",
            Some("refund_stake_and_fee"),
        ),
        ("last_sync_status", 32, "YES", None),
        ("last_sync_error", 512, "YES", None),
    ];
    ensure!(rows.len() == expected.len());

    for (column_name, length, nullable, default) in expected {
        let row = rows
            .get(column_name)
            .with_context(|| format!("missing metadata for prediction_settings.{column_name}"))?;
        ensure!(row.data_type == "varchar");
        ensure!(row.character_set_name.as_deref() == Some("utf8mb4"));
        ensure!(
            row.collation_name.as_deref() == Some(expected_collation),
            "prediction_settings.{column_name} must use {expected_collation}"
        );
        ensure!(row.is_nullable == nullable);
        ensure!(row.column_default.as_deref() == default);
        ensure!(row.character_maximum_length == Some(length));
    }

    Ok(())
}
