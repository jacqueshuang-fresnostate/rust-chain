use anyhow::{Context, Result, ensure};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::collections::BTreeMap;
use url::Url;
use uuid::Uuid;

const MIGRATION: &str =
    include_str!("../migrations/0102_synthetic_market_and_manual_kline_recovery.sql");

#[derive(Debug, sqlx::FromRow)]
struct TextColumnMetadata {
    table_name: String,
    column_name: String,
    data_type: String,
    character_set_name: Option<String>,
    collation_name: Option<String>,
    is_nullable: String,
    column_default: Option<String>,
    column_comment: String,
}

#[tokio::test]
async fn synthetic_market_migration_backfills_versions_before_binding_runs() {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!("skipping synthetic market migration test because DATABASE_URL is not set");
            return;
        }
    };

    let mut server_url = Url::parse(&database_url).expect("DATABASE_URL must be a valid URL");
    server_url.set_path("/");
    let server_pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect(server_url.as_str())
        .await
        .expect("connect to MySQL server for isolated synthetic market migration test");
    let database_name = format!("synthetic_market_{}", Uuid::now_v7().simple());
    let create_database =
        format!("CREATE DATABASE `{database_name}` CHARACTER SET latin1 COLLATE latin1_swedish_ci");
    if let Err(error) = sqlx::query(&create_database).execute(&server_pool).await {
        eprintln!(
            "skipping synthetic market migration test because an isolated database cannot be created: {error}"
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
        .expect("connect to isolated synthetic market migration database");

    let exercise_result = exercise_migration_contract(&pool).await;
    pool.close().await;
    let drop_database = format!("DROP DATABASE `{database_name}`");
    let cleanup_result = sqlx::query(&drop_database)
        .execute(&server_pool)
        .await
        .context("drop isolated synthetic market migration database");
    server_pool.close().await;

    exercise_result.expect("synthetic market migration contract");
    cleanup_result.expect("synthetic market migration test database cleanup");
}

async fn exercise_migration_contract(pool: &MySqlPool) -> Result<()> {
    create_pre_migration_schema(pool).await?;
    seed_pre_migration_runs(pool).await?;

    sqlx::raw_sql(MIGRATION)
        .execute(pool)
        .await
        .context("execute exact synthetic market migration SQL")?;

    let missing_version_run: (i32, i32, String, String) = sqlx::query_as(
        r#"SELECT runs.active_version,
                  versions.version,
                  CAST(JSON_UNQUOTE(JSON_EXTRACT(versions.config_json, '$.nodes'))
                       AS CHAR CHARACTER SET utf8mb4),
                  versions.seed
           FROM strategy_runs runs
           INNER JOIN strategy_versions versions
             ON versions.strategy_id = runs.strategy_id
            AND versions.version = runs.active_version
           WHERE runs.strategy_id = 101"#,
    )
    .fetch_one(pool)
    .await
    .context("read migrated run that previously had no strategy version")?;
    ensure!(missing_version_run.0 == 1);
    ensure!(missing_version_run.1 == 1);
    ensure!(missing_version_run.2 == "[]");
    ensure!(missing_version_run.3 == "migration-0102-strategy-101-version-1");

    let existing_version_run: (i32, String) = sqlx::query_as(
        r#"SELECT runs.active_version, versions.seed
           FROM strategy_runs runs
           INNER JOIN strategy_versions versions
             ON versions.strategy_id = runs.strategy_id
            AND versions.version = runs.active_version
           WHERE runs.strategy_id = 102"#,
    )
    .fetch_one(pool)
    .await
    .context("read run bound to its pre-existing latest strategy version")?;
    ensure!(existing_version_run.0 == 3);
    ensure!(existing_version_run.1 == "existing-version-3");

    let orphan_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM strategy_runs runs
           LEFT JOIN strategy_versions versions
             ON versions.strategy_id = runs.strategy_id
            AND versions.version = runs.active_version
           WHERE versions.id IS NULL"#,
    )
    .fetch_one(pool)
    .await?;
    ensure!(orphan_count == 0);

    let active_version_nullable: String = sqlx::query_scalar(
        r#"SELECT CAST(IS_NULLABLE AS CHAR(3) CHARACTER SET utf8mb4)
           FROM information_schema.COLUMNS
           WHERE TABLE_SCHEMA = DATABASE()
             AND TABLE_NAME = 'strategy_runs'
             AND COLUMN_NAME = 'active_version'"#,
    )
    .fetch_one(pool)
    .await?;
    ensure!(active_version_nullable == "NO");

    let foreign_key_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM information_schema.REFERENTIAL_CONSTRAINTS
           WHERE CONSTRAINT_SCHEMA = DATABASE()
             AND CONSTRAINT_NAME = 'fk_strategy_runs_active_version'
             AND REFERENCED_TABLE_NAME = 'strategy_versions'"#,
    )
    .fetch_one(pool)
    .await?;
    ensure!(foreign_key_count == 1);

    let invalid_active_version =
        sqlx::query("UPDATE strategy_runs SET active_version = 999 WHERE strategy_id = 101")
            .execute(pool)
            .await;
    ensure!(invalid_active_version.is_err());

    let check_constraints: Vec<String> = sqlx::query_scalar(
        r#"SELECT CAST(CONSTRAINT_NAME AS CHAR(128) CHARACTER SET utf8mb4)
           FROM information_schema.TABLE_CONSTRAINTS
           WHERE CONSTRAINT_SCHEMA = DATABASE()
             AND CONSTRAINT_TYPE = 'CHECK'
             AND TABLE_NAME IN ('market_strategy_nodes', 'kline_recovery_jobs')
           ORDER BY CONSTRAINT_NAME"#,
    )
    .fetch_all(pool)
    .await?;
    for constraint in [
        "chk_market_strategy_nodes_execution_mode",
        "chk_market_strategy_nodes_target_type",
        "chk_market_strategy_nodes_target_value",
        "chk_market_strategy_nodes_tolerance",
        "chk_market_strategy_nodes_volume_pair",
        "chk_market_strategy_nodes_volatility",
        "chk_kline_recovery_jobs_actual_counts",
        "chk_kline_recovery_jobs_expected_count",
        "chk_kline_recovery_jobs_range",
        "chk_kline_recovery_jobs_status",
    ] {
        ensure!(check_constraints.iter().any(|value| value == constraint));
    }

    assert_text_metadata_and_comments(pool).await?;
    Ok(())
}

async fn create_pre_migration_schema(pool: &MySqlPool) -> Result<()> {
    sqlx::raw_sql(
        r#"CREATE TABLE admin_users (
               id BIGINT UNSIGNED PRIMARY KEY
           ) ENGINE=InnoDB;
           CREATE TABLE trading_pairs (
               id BIGINT UNSIGNED PRIMARY KEY,
               market_type VARCHAR(32) NOT NULL
           ) ENGINE=InnoDB;
           CREATE TABLE market_strategies (
               id BIGINT UNSIGNED PRIMARY KEY,
               pair_id BIGINT UNSIGNED NOT NULL,
               strategy_type VARCHAR(32) NOT NULL,
               start_price DECIMAL(38,18) NOT NULL,
               target_price DECIMAL(38,18) NOT NULL,
               start_time TIMESTAMP(6) NOT NULL,
               end_time TIMESTAMP(6) NOT NULL,
               volatility DECIMAL(18,8) NOT NULL,
               volume_min DECIMAL(38,18) NOT NULL,
               volume_max DECIMAL(38,18) NOT NULL,
               status VARCHAR(32) NOT NULL,
               CONSTRAINT fk_fixture_strategy_pair FOREIGN KEY (pair_id) REFERENCES trading_pairs(id)
           ) ENGINE=InnoDB;
           CREATE TABLE strategy_versions (
               id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
               strategy_id BIGINT UNSIGNED NOT NULL,
               version INT NOT NULL,
               effective_time TIMESTAMP(6) NOT NULL,
               config_json JSON NOT NULL,
               seed VARCHAR(128) NOT NULL,
               created_by BIGINT UNSIGNED NULL,
               UNIQUE KEY uq_strategy_versions_strategy_version (strategy_id, version),
               CONSTRAINT fk_fixture_version_strategy FOREIGN KEY (strategy_id) REFERENCES market_strategies(id),
               CONSTRAINT fk_fixture_version_admin FOREIGN KEY (created_by) REFERENCES admin_users(id)
           ) ENGINE=InnoDB;
           CREATE TABLE strategy_runs (
               strategy_id BIGINT UNSIGNED PRIMARY KEY,
               run_status VARCHAR(32) NOT NULL,
               current_price DECIMAL(38,18) NULL,
               last_tick_at TIMESTAMP(6) NULL,
               last_generated_at TIMESTAMP(6) NULL,
               last_kline_open_time TIMESTAMP(6) NULL,
               recovery_status VARCHAR(32) NULL,
               error_message TEXT NULL,
               updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
                   ON UPDATE CURRENT_TIMESTAMP(6),
               CONSTRAINT fk_fixture_run_strategy FOREIGN KEY (strategy_id) REFERENCES market_strategies(id)
           ) ENGINE=InnoDB;"#,
    )
    .execute(pool)
    .await
    .context("create pre-0102 strategy schema")?;
    Ok(())
}

async fn seed_pre_migration_runs(pool: &MySqlPool) -> Result<()> {
    sqlx::raw_sql(
        r#"INSERT INTO trading_pairs (id, market_type)
           VALUES (11, 'strategy'), (12, 'internal');
           INSERT INTO market_strategies
               (id, pair_id, strategy_type, start_price, target_price, start_time, end_time,
                volatility, volume_min, volume_max, status)
           VALUES
               (101, 11, 'price_path', 1, 2, '2026-08-12 10:00:00',
                '2026-08-12 11:00:00', 0.01, 10, 20, 'paused'),
               (102, 12, 'price_path', 5, 8, '2026-08-12 12:00:00',
                '2026-08-12 13:00:00', 0.02, 30, 40, 'active');
           INSERT INTO strategy_versions
               (strategy_id, version, effective_time, config_json, seed, created_by)
           VALUES
               (102, 2, '2026-08-12 12:00:00', JSON_OBJECT('nodes', JSON_ARRAY()),
                'existing-version-2', NULL),
               (102, 3, '2026-08-12 12:00:00', JSON_OBJECT('nodes', JSON_ARRAY()),
                'existing-version-3', NULL);
           INSERT INTO strategy_runs
               (strategy_id, run_status, current_price, last_generated_at,
                last_kline_open_time, recovery_status)
           VALUES
               (101, 'paused', 1, '2026-08-12 10:00:00', '2026-08-12 10:00:00', 'idle'),
               (102, 'running', 5, '2026-08-12 12:00:00', '2026-08-12 12:00:00', 'live');"#,
    )
    .execute(pool)
    .await
    .context("seed pre-0102 strategy runs and partial version history")?;
    Ok(())
}

async fn assert_text_metadata_and_comments(pool: &MySqlPool) -> Result<()> {
    let rows = sqlx::query_as::<_, TextColumnMetadata>(
        r#"SELECT CAST(TABLE_NAME AS CHAR(64) CHARACTER SET utf8mb4) AS table_name,
                  CAST(COLUMN_NAME AS CHAR(64) CHARACTER SET utf8mb4) AS column_name,
                  CAST(DATA_TYPE AS CHAR(64) CHARACTER SET utf8mb4) AS data_type,
                  CAST(CHARACTER_SET_NAME AS CHAR(64) CHARACTER SET utf8mb4)
                      AS character_set_name,
                  CAST(COLLATION_NAME AS CHAR(64) CHARACTER SET utf8mb4) AS collation_name,
                  CAST(IS_NULLABLE AS CHAR(3) CHARACTER SET utf8mb4) AS is_nullable,
                  CAST(COLUMN_DEFAULT AS CHAR(512) CHARACTER SET utf8mb4) AS column_default,
                  CAST(COLUMN_COMMENT AS CHAR(2048) CHARACTER SET utf8mb4) AS column_comment
           FROM information_schema.COLUMNS
           WHERE TABLE_SCHEMA = DATABASE()
             AND ((TABLE_NAME = 'market_strategy_nodes'
                    AND COLUMN_NAME IN ('target_type', 'execution_mode'))
               OR (TABLE_NAME = 'strategy_runs' AND COLUMN_NAME = 'lease_owner')
               OR (TABLE_NAME = 'kline_recovery_jobs'
                    AND COLUMN_NAME IN ('preview_token_hash', 'reason', 'status', 'error_message')))
           ORDER BY TABLE_NAME, ORDINAL_POSITION"#,
    )
    .fetch_all(pool)
    .await
    .context("read new synthetic market text metadata")?
    .into_iter()
    .map(|row| (format!("{}.{}", row.table_name, row.column_name), row))
    .collect::<BTreeMap<_, _>>();

    let expected = [
        ("kline_recovery_jobs.error_message", "text", "YES", None),
        ("kline_recovery_jobs.preview_token_hash", "char", "NO", None),
        ("kline_recovery_jobs.reason", "varchar", "NO", None),
        (
            "kline_recovery_jobs.status",
            "varchar",
            "NO",
            Some("pending"),
        ),
        (
            "market_strategy_nodes.execution_mode",
            "varchar",
            "NO",
            None,
        ),
        ("market_strategy_nodes.target_type", "varchar", "NO", None),
        ("strategy_runs.lease_owner", "varchar", "YES", None),
    ];
    ensure!(rows.len() == expected.len());
    for (name, data_type, nullable, default) in expected {
        let row = rows
            .get(name)
            .with_context(|| format!("missing metadata for {name}"))?;
        ensure!(row.data_type == data_type);
        ensure!(row.character_set_name.as_deref() == Some("utf8mb4"));
        ensure!(row.collation_name.as_deref() == Some("utf8mb4_unicode_ci"));
        ensure!(row.is_nullable == nullable);
        ensure!(row.column_default.as_deref() == default);
        ensure!(!row.column_comment.trim().is_empty());
        ensure!(!row.column_comment.is_ascii());
    }

    let table_metadata: Vec<(String, String, String)> = sqlx::query_as(
        r#"SELECT CAST(TABLE_NAME AS CHAR(64) CHARACTER SET utf8mb4),
                  CAST(TABLE_COLLATION AS CHAR(64) CHARACTER SET utf8mb4),
                  CAST(TABLE_COMMENT AS CHAR(2048) CHARACTER SET utf8mb4)
           FROM information_schema.TABLES
           WHERE TABLE_SCHEMA = DATABASE()
             AND TABLE_NAME IN ('market_strategy_nodes', 'kline_recovery_jobs')
           ORDER BY TABLE_NAME"#,
    )
    .fetch_all(pool)
    .await
    .context("read new synthetic market table metadata")?;
    ensure!(table_metadata.len() == 2);
    for (_, collation, comment) in table_metadata {
        ensure!(collation == "utf8mb4_unicode_ci");
        ensure!(!comment.trim().is_empty());
        ensure!(!comment.is_ascii());
    }
    Ok(())
}
