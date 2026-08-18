use anyhow::{Context, Result, ensure};
use serde_json::Value;
use sqlx::{MySqlPool, mysql::MySqlPoolOptions, types::Json as SqlxJson};
use url::Url;
use uuid::Uuid;

const MIGRATION: &str = include_str!("../migrations/0104_admin_configuration_governance.sql");

#[test]
fn governance_migration_declares_rbac_audit_revision_and_maker_checker_contracts() {
    for required in [
        "ADD COLUMN request_id VARCHAR(64)",
        "SET permissions = JSON_ARRAY('*')",
        "ALTER TABLE prediction_settings",
        "ALTER TABLE prediction_asset_configs",
        "ALTER TABLE loan_products",
        "CREATE TABLE admin_config_change_requests",
        "applied_by BIGINT UNSIGNED",
        "created_by BIGINT UNSIGNED NOT NULL",
        "reviewed_by BIGINT UNSIGNED NULL",
    ] {
        assert!(
            MIGRATION.contains(required),
            "missing migration contract: {required}"
        );
    }
}

#[tokio::test]
async fn governance_migration_builds_revision_and_two_person_storage() {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!("skipping governance migration test because DATABASE_URL is not set");
            return;
        }
    };

    let mut server_url = Url::parse(&database_url).expect("DATABASE_URL must be a valid URL");
    server_url.set_path("/");
    let server_pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect(server_url.as_str())
        .await
        .expect("connect to MySQL server for isolated governance migration test");
    let database_name = format!("admin_governance_{}", Uuid::now_v7().simple());
    if let Err(error) = sqlx::query(&format!("CREATE DATABASE `{database_name}`"))
        .execute(&server_pool)
        .await
    {
        eprintln!(
            "skipping governance migration test because an isolated database cannot be created: {error}"
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
        .expect("connect to isolated governance migration database");

    let exercise_result = exercise_migration(&pool).await;
    pool.close().await;
    let cleanup_result = sqlx::query(&format!("DROP DATABASE `{database_name}`"))
        .execute(&server_pool)
        .await
        .context("drop isolated governance migration database");
    server_pool.close().await;

    exercise_result.expect("governance migration contract");
    cleanup_result.expect("governance migration database cleanup");
}

async fn exercise_migration(pool: &MySqlPool) -> Result<()> {
    sqlx::raw_sql(
        r#"CREATE TABLE admin_roles (
               id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
               name VARCHAR(64) NOT NULL UNIQUE,
               permissions JSON NOT NULL
           );
           CREATE TABLE admin_users (
               id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
               username VARCHAR(64) NOT NULL,
               role_id BIGINT UNSIGNED NOT NULL
           );
           CREATE TABLE admin_audit_logs (
               id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
               admin_id BIGINT UNSIGNED NOT NULL,
               action VARCHAR(128) NOT NULL,
               target_type VARCHAR(64) NOT NULL,
               target_id VARCHAR(64) NOT NULL,
               before_json JSON NULL,
               after_json JSON NULL,
               reason VARCHAR(512) NULL,
               ip VARCHAR(64) NULL,
               created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
           );
           CREATE TABLE prediction_settings (
               id BIGINT UNSIGNED PRIMARY KEY,
               quote_ttl_seconds INT UNSIGNED NOT NULL
           );
           CREATE TABLE prediction_asset_configs (
               id BIGINT UNSIGNED PRIMARY KEY,
               max_payout_amount DECIMAL(36, 18) NULL
           );
           CREATE TABLE loan_products (
               id BIGINT UNSIGNED PRIMARY KEY,
               status VARCHAR(32) NOT NULL
           );
           INSERT INTO admin_roles (name, permissions) VALUES ('super_admin', JSON_ARRAY());
           INSERT INTO admin_users (username, role_id) VALUES ('maker', 1), ('reviewer', 1);
           INSERT INTO prediction_settings (id, quote_ttl_seconds) VALUES (1, 30);
           INSERT INTO prediction_asset_configs (id, max_payout_amount) VALUES (1, 100);
           INSERT INTO loan_products (id, status) VALUES (1, 'active');"#,
    )
    .execute(pool)
    .await?;

    sqlx::raw_sql(MIGRATION).execute(pool).await?;

    let permissions: SqlxJson<Value> =
        sqlx::query_scalar("SELECT permissions FROM admin_roles WHERE name = 'super_admin'")
            .fetch_one(pool)
            .await?;
    ensure!(permissions.0 == serde_json::json!(["*"]));

    let revisions = sqlx::query_as::<_, (u64, u64, u64)>(
        r#"SELECT
             (SELECT revision FROM prediction_settings WHERE id = 1),
             (SELECT revision FROM prediction_asset_configs WHERE id = 1),
             (SELECT revision FROM loan_products WHERE id = 1)"#,
    )
    .fetch_one(pool)
    .await?;
    ensure!(revisions == (1, 1, 1));

    sqlx::query(
        r#"INSERT INTO admin_config_change_requests
           (request_no, config_domain, target_type, target_id, action, proposed_json,
            reason, risk_level, status, created_by, reviewed_by)
           VALUES ('ACR-test', 'prediction', 'settings', 'default', 'update',
                   JSON_OBJECT('enabled', TRUE), 'test', 'high', 'approved', 1, 2)"#,
    )
    .execute(pool)
    .await?;
    let row: (String, u64, Option<u64>) = sqlx::query_as(
        "SELECT status, created_by, reviewed_by FROM admin_config_change_requests WHERE request_no = 'ACR-test'",
    )
    .fetch_one(pool)
    .await?;
    ensure!(row == ("approved".to_owned(), 1, Some(2)));
    Ok(())
}
