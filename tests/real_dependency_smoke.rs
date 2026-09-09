use mongodb::bson::doc;
use redis::AsyncCommands;
use sqlx::mysql::MySqlPoolOptions;
use std::env;
use uuid::Uuid;

/// 在 CI 真实依赖服务上验证迁移、MySQL 查询、Redis 读写和 Mongo 读写。
/// 本地未提供 DATABASE_URL 时明确跳过；一旦进入 CI 模式，其他依赖环境变量
/// 缺失或服务不可用都必须失败，避免把“只跑了迁移”当成完整集成验证。
#[tokio::test]
async fn real_mysql_redis_and_mongo_are_ready_after_migrations()
-> Result<(), Box<dyn std::error::Error>> {
    if env::var("RUN_REAL_DEPENDENCY_SMOKE").as_deref() != Ok("1") {
        eprintln!("skipping real dependency smoke test because RUN_REAL_DEPENDENCY_SMOKE is not 1");
        return Ok(());
    }
    let database_url = env::var("DATABASE_URL")?;
    let redis_url = env::var("REDIS_URL")?;
    let mongodb_uri = env::var("MONGODB_URI")?;
    let mongodb_database = env::var("MONGODB_DATABASE")?;

    let mysql = MySqlPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await?;
    let migrator = sqlx::migrate!("./migrations");
    let expected_migrations = migrator.iter().count() as i64;
    let expected_latest_version = migrator
        .iter()
        .map(|migration| migration.version)
        .max()
        .expect("at least one embedded migration");
    let applied_migrations: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations WHERE success = TRUE")
            .fetch_one(&mysql)
            .await?;
    let latest_applied_version: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations WHERE success = TRUE",
    )
    .fetch_one(&mysql)
    .await?;
    let failed_migrations: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations WHERE success = FALSE")
            .fetch_one(&mysql)
            .await?;
    assert_eq!(
        applied_migrations, expected_migrations,
        "migration count drift"
    );
    assert_eq!(
        latest_applied_version, expected_latest_version,
        "latest migration was not applied"
    );
    assert_eq!(
        failed_migrations, 0,
        "failed SQLx migrations remain recorded"
    );

    let required_tables: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM information_schema.tables
           WHERE table_schema = DATABASE()
             AND table_name IN (
               'wallet_ledger', 'market_price_ticks', 'kline_recovery_jobs',
               'new_coin_distributions', 'admin_audit_logs'
             )"#,
    )
    .fetch_one(&mysql)
    .await?;
    assert_eq!(required_tables, 5, "required release tables are missing");
    mysql.close().await;

    let mut redis = redis::Client::open(redis_url)?
        .get_connection_manager()
        .await?;
    let redis_key = format!("ci:dependency-smoke:{}", Uuid::now_v7());
    let pong: String = redis::cmd("PING").query_async(&mut redis).await?;
    assert_eq!(pong, "PONG");
    let _: () = redis.set_ex(&redis_key, "ready", 30).await?;
    let redis_value: Option<String> = redis.get(&redis_key).await?;
    assert_eq!(redis_value.as_deref(), Some("ready"));
    let _: usize = redis.del(&redis_key).await?;

    let mongo = mongodb::Client::with_uri_str(&mongodb_uri).await?;
    let database = mongo.database(&mongodb_database);
    database.run_command(doc! { "ping": 1 }).await?;
    let collection_name = format!("ci_dependency_smoke_{}", Uuid::now_v7().simple());
    let collection = database.collection::<mongodb::bson::Document>(&collection_name);
    let token = Uuid::now_v7().to_string();
    collection
        .insert_one(doc! { "token": &token, "status": "ready" })
        .await?;
    let stored = collection.find_one(doc! { "token": &token }).await?;
    assert_eq!(
        stored
            .as_ref()
            .and_then(|document| document.get_str("status").ok()),
        Some("ready")
    );
    collection.drop().await?;

    Ok(())
}
