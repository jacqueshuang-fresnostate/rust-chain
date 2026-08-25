use anyhow::{Context, Result, ensure};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use url::Url;
use uuid::Uuid;

const FINAL_MIGRATION_VERSION: i64 = 116;
const RECONCILIATION_MIGRATION: &str =
    include_str!("../migrations/0108_withdrawal_broadcast_reconciliation.sql");
const QUOTE_MIGRATION: &str = include_str!("../migrations/0109_withdrawal_quotes.sql");

#[test]
fn reconciliation_migration_persists_acceptance_evidence_without_duplicate_indexes() {
    assert!(RECONCILIATION_MIGRATION.contains("acceptance_evidence_at"));
    assert!(
        !RECONCILIATION_MIGRATION.contains("idx_wallet_withdrawal_unknown"),
        "0087 already creates the same (status, next_attempt_at, id) index"
    );
}

#[test]
fn withdrawal_quote_foreign_key_preserves_the_consumption_check_contract() {
    assert!(
        QUOTE_MIGRATION.contains("chk_wallet_withdrawal_quote_consumption"),
        "0109 must constrain withdrawal_id to consumed quotes"
    );
    assert!(
        !QUOTE_MIGRATION.contains("ON DELETE SET NULL"),
        "MySQL rejects SET NULL on withdrawal_id because the same column participates in a CHECK constraint"
    );
    assert!(
        QUOTE_MIGRATION.contains("ON DELETE RESTRICT"),
        "consumed quote evidence must use an explicit non-mutating delete rule"
    );
}

#[tokio::test]
async fn fresh_database_runs_the_complete_migration_chain_idempotently() {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!("skipping fresh migration-chain MySQL test because DATABASE_URL is not set");
            return;
        }
    };

    let mut server_url = Url::parse(&database_url).expect("DATABASE_URL must be a valid URL");
    server_url.set_path("/");
    let server_pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect(server_url.as_str())
        .await
        .expect("connect to MySQL server for isolated migration-chain test");
    let database_name = format!("exchange_migration_chain_{}", Uuid::now_v7().simple());
    let create_database = format!(
        "CREATE DATABASE `{database_name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
    );
    if let Err(error) = sqlx::query(&create_database).execute(&server_pool).await {
        eprintln!(
            "skipping fresh migration-chain MySQL test because an isolated database cannot be created: {error}"
        );
        server_pool.close().await;
        return;
    }

    let mut test_url = Url::parse(&database_url).expect("DATABASE_URL must be a valid URL");
    test_url.set_path(&format!("/{database_name}"));
    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect(test_url.as_str())
        .await
        .expect("connect to isolated migration-chain database");

    let exercise_result = exercise_complete_migration_chain(&pool).await;
    pool.close().await;
    let cleanup_result = sqlx::query(&format!("DROP DATABASE `{database_name}`"))
        .execute(&server_pool)
        .await
        .context("drop isolated migration-chain database");
    server_pool.close().await;

    exercise_result.expect("fresh 0001-0116 migration-chain contract");
    cleanup_result.expect("migration-chain test database cleanup");
}

async fn exercise_complete_migration_chain(pool: &MySqlPool) -> Result<()> {
    let migrator = sqlx::migrate!("./migrations");
    let final_version = migrator
        .migrations
        .last()
        .context("migration chain is empty")?
        .version;
    ensure!(
        final_version == FINAL_MIGRATION_VERSION,
        "expected migration chain to end at 0116, found {final_version:04}"
    );
    let first_version = migrator
        .migrations
        .first()
        .context("migration chain is empty")?
        .version;
    ensure!(
        first_version == 1,
        "expected migration chain to start at 0001, found {first_version:04}"
    );
    for pair in migrator.migrations.windows(2) {
        ensure!(
            pair[0].version < pair[1].version,
            "migration versions must be strictly increasing: {:04} then {:04}",
            pair[0].version,
            pair[1].version
        );
    }
    ensure!(
        migrator
            .migrations
            .iter()
            .any(|migration| migration.version == 109),
        "complete migration chain must include 0109 withdrawal quotes"
    );

    migrator
        .run(pool)
        .await
        .context("run fresh migrations 0001-0116")?;
    let applied_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations WHERE success = TRUE")
            .fetch_one(pool)
            .await
            .context("count applied migrations")?;
    ensure!(
        applied_count == migrator.migrations.len() as i64,
        "all embedded migrations must be recorded exactly once"
    );

    migrator
        .run(pool)
        .await
        .context("rerun complete migration chain idempotently")?;
    let rerun_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations WHERE success = TRUE")
            .fetch_one(pool)
            .await
            .context("count applied migrations after idempotent rerun")?;
    ensure!(rerun_count == applied_count);

    let delete_rule: String = sqlx::query_scalar(
        r#"SELECT CAST(DELETE_RULE AS CHAR CHARACTER SET utf8mb4)
           FROM information_schema.REFERENTIAL_CONSTRAINTS
           WHERE CONSTRAINT_SCHEMA = DATABASE()
             AND TABLE_NAME = 'wallet_withdrawal_quotes'
             AND CONSTRAINT_NAME = 'fk_wallet_withdrawal_quotes_withdrawal'"#,
    )
    .fetch_one(pool)
    .await
    .context("read withdrawal quote foreign-key delete rule")?;
    ensure!(
        delete_rule == "RESTRICT" || delete_rule == "NO ACTION",
        "quote consumption evidence must not be nulled by referential actions: {delete_rule}"
    );

    let consumption_check_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM information_schema.TABLE_CONSTRAINTS
           WHERE CONSTRAINT_SCHEMA = DATABASE()
             AND TABLE_NAME = 'wallet_withdrawal_quotes'
             AND CONSTRAINT_NAME = 'chk_wallet_withdrawal_quote_consumption'
             AND CONSTRAINT_TYPE = 'CHECK'"#,
    )
    .fetch_one(pool)
    .await
    .context("verify withdrawal quote consumption check")?;
    ensure!(consumption_check_count == 1);

    Ok(())
}
