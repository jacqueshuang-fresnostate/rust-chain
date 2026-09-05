use anyhow::{Result, ensure};
use chrono::{DateTime, Utc};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use url::Url;
use uuid::Uuid;

#[tokio::test]
async fn listing_migration_uses_only_recorded_events_and_keeps_legacy_locks() -> Result<()> {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping listing migration test: DATABASE_URL is not set");
        return Ok(());
    };
    let mut url = Url::parse(&database_url)?;
    url.set_path("/");
    let server = MySqlPoolOptions::new()
        .max_connections(1)
        .connect(url.as_str())
        .await?;
    let name = format!("new_coin_listing_{}", Uuid::now_v7().simple());
    sqlx::query(&format!(
        "CREATE DATABASE `{name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
    ))
    .execute(&server)
    .await?;
    url.set_path(&format!("/{name}"));
    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect(url.as_str())
        .await?;
    let result = exercise_migration(&pool).await;
    pool.close().await;
    let cleanup = sqlx::query(&format!("DROP DATABASE `{name}`"))
        .execute(&server)
        .await;
    server.close().await;
    result?;
    cleanup?;
    Ok(())
}

async fn exercise_migration(pool: &MySqlPool) -> Result<()> {
    // Minimal pre-0123 schema: execute the exact immutable migration, not a copied UPDATE.
    sqlx::raw_sql(r#"
        CREATE TABLE new_coin_projects (id BIGINT UNSIGNED PRIMARY KEY, asset_id BIGINT UNSIGNED NOT NULL, lifecycle_status VARCHAR(32) NOT NULL, listed_at TIMESTAMP(6) NULL);
        CREATE TABLE new_coin_lifecycle_events (project_id BIGINT UNSIGNED NOT NULL, event_type VARCHAR(128), payload_json JSON NOT NULL, created_at TIMESTAMP(6) NOT NULL);
        CREATE TABLE asset_lock_positions (id BIGINT UNSIGNED PRIMARY KEY, unlock_at TIMESTAMP(6) NOT NULL, unlock_type VARCHAR(32) NOT NULL, remaining_amount DECIMAL(36,18) NOT NULL);
        INSERT INTO new_coin_projects VALUES (1,1,'listed','2001-01-01'),(2,2,'listed','2001-01-01'),(3,3,'listed','2001-01-01'),(4,4,'distribution','2001-01-01');
        INSERT INTO new_coin_lifecycle_events VALUES
          (1,'new_coin_project.lifecycle.update','{"before":{"lifecycle_status":"distribution"},"after":{"lifecycle_status":"listed"}}','2000-01-02 00:00:00.123456'),
          (1,'new_coin_project.unlock_rule.update','{"after":{"lifecycle_status":"listed"}}','1999-01-01'),
          (2,'new_coin_project.create','{"lifecycle_status":"listed"}','2000-01-03'),
          (3,'new_coin_project.unlock_rule.update','{"after":{"lifecycle_status":"listed"}}','2000-01-04'),
          (4,'new_coin_project.lifecycle.update','{"before":{"lifecycle_status":"distribution"},"after":{"lifecycle_status":"listed"}}','2000-01-05');
        INSERT INTO asset_lock_positions VALUES (1,'2000-01-01','immediate_on_listing',10),(2,'2000-01-02','fixed_time',20),(3,'2000-01-03','relative_period',30);
    "#).execute(pool).await?;
    let before:Vec<(u64,DateTime<Utc>,String,String)>=sqlx::query_as("SELECT id,unlock_at,unlock_type,CAST(remaining_amount AS CHAR) FROM asset_lock_positions ORDER BY id").fetch_all(pool).await?;
    sqlx::raw_sql(include_str!(
        "../migrations/0123_new_coin_actual_listing.sql"
    ))
    .execute(pool)
    .await?;
    let events: Vec<(u64, DateTime<Utc>, Option<DateTime<Utc>>)> =
        sqlx::query_as("SELECT id,listed_at,actual_listed_at FROM new_coin_projects ORDER BY id")
            .fetch_all(pool)
            .await?;
    ensure!(
        events.iter().all(|p| p.1.timestamp() == 978307200),
        "plans changed"
    );
    ensure!(
        events[0].2.unwrap().timestamp_micros() == 946771200123456,
        "listing audit microseconds lost"
    );
    ensure!(
        events[1].2.unwrap().timestamp() == 946857600,
        "listed creation not backfilled"
    );
    ensure!(
        events[2].2.is_none() && events[3].2.is_none(),
        "unknown/nonlisted project inferred an event"
    );
    let after:Vec<(u64,DateTime<Utc>,String,String)>=sqlx::query_as("SELECT id,unlock_at,unlock_type,CAST(remaining_amount AS CHAR) FROM asset_lock_positions ORDER BY id").fetch_all(pool).await?;
    ensure!(before == after, "historic contract rewritten");
    let gates: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM asset_lock_positions WHERE listing_project_id IS NOT NULL",
    )
    .fetch_one(pool)
    .await?;
    ensure!(gates == 0, "legacy lock opted into new gate");
    Ok(())
}
