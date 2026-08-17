use std::{error::Error, str::FromStr};

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header::AUTHORIZATION},
};
use bigdecimal::BigDecimal;
use chrono::{Duration, TimeZone, Utc};
use exchange_api::{
    build_router,
    config::Settings,
    infra::mongo::kline_collection_name,
    modules::{
        auth::{TokenScope, issue_token},
        market::{SyntheticMarketConfig, ValidatedMarketSymbol},
    },
    state::AppState,
};
use mongodb::{
    Client, Database,
    bson::{DateTime as BsonDateTime, Document, doc},
};
use secrecy::SecretString;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{MySqlPool, migrate::MigrateError, mysql::MySqlPoolOptions};
use tower::ServiceExt;
use uuid::Uuid;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).expect("valid decimal fixture")
}

fn env_or_skip(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            eprintln!("skipping admin market recovery test because {name} is not set");
            None
        })
}

async fn mysql_pool_or_skip() -> Result<Option<MySqlPool>, Box<dyn Error>> {
    let Some(database_url) = env_or_skip("DATABASE_URL") else {
        return Ok(None);
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    match sqlx::migrate!().run(&pool).await {
        Ok(()) | Err(MigrateError::VersionMismatch(102)) => {}
        Err(error) => return Err(error.into()),
    }
    Ok(Some(pool))
}

async fn mongo_database_or_skip() -> Result<Option<Database>, Box<dyn Error>> {
    let Some(mongodb_uri) = env_or_skip("MONGODB_URI") else {
        return Ok(None);
    };
    let database_name = std::env::var("MONGODB_DATABASE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "exchange_market".to_owned());
    let client = Client::with_uri_str(&mongodb_uri).await?;
    Ok(Some(client.database(&database_name)))
}

fn test_settings() -> Settings {
    Settings {
        app_env: "test".to_owned(),
        app_host: "127.0.0.1".parse().unwrap(),
        app_port: 0,
        database_url: SecretString::new("mysql://test:test@localhost/test".to_owned()),
        mongodb_uri: SecretString::new("mongodb://localhost:27017".to_owned()),
        mongodb_database: "exchange_test".to_owned(),
        redis_url: SecretString::new("redis://127.0.0.1:1".to_owned()),
        rabbitmq_url: SecretString::new("amqp://guest:guest@localhost:5672/%2f".to_owned()),
        jwt_secret: SecretString::new("admin-market-recovery-secret".to_owned()),
        credential_encryption_key: None,
        jwt_access_ttl_seconds: 900,
        jwt_refresh_ttl_seconds: 2_592_000,
        bitget_rest_base_url: "https://bitget.test".to_owned(),
        bitget_ws_url: "wss://bitget.test/ws".to_owned(),
        htx_rest_base_url: "https://htx.test".to_owned(),
        htx_ws_url: "wss://htx.test/ws".to_owned(),
        coinbase_rest_base_url: "https://coinbase.test".to_owned(),
        coinbase_ws_url: "wss://coinbase.test/ws".to_owned(),
        market_feed_symbols: Vec::new(),
        market_feed_intervals: Vec::new(),
        market_feed_providers: Vec::new(),
        market_feed_reconnect_seconds: 5,
        market_feed_rest_fallback_timeout_seconds: 3,
        event_inbox_retry_scan_seconds: 10,
        event_outbox_publisher_enabled: false,
        event_outbox_publisher_interval_seconds: 5,
        unlock_scanner_enabled: false,
        unlock_scanner_interval_seconds: 10,
        unlock_scanner_batch_limit: 100,
        kline_recovery_enabled: false,
        kline_recovery_interval_seconds: 30,
        kline_recovery_batch_limit: 100,
        seconds_contract_settlement_enabled: false,
        seconds_contract_settlement_interval_seconds: 5,
        seconds_contract_settlement_batch_limit: 100,
        earn_auto_redemption_enabled: false,
        earn_auto_redemption_interval_seconds: 60,
        earn_auto_redemption_batch_limit: 100,
        margin_liquidation_enabled: false,
        margin_liquidation_interval_seconds: 5,
        margin_liquidation_batch_limit: 100,
        margin_interest_enabled: false,
        margin_interest_interval_seconds: 60,
        margin_interest_batch_limit: 100,
        agent_commission_auto_settle_enabled: false,
        agent_commission_auto_settle_interval_seconds: 60,
        agent_commission_auto_settle_min_age_seconds: 3600,
        agent_commission_auto_settle_batch_limit: 100,
    }
}

async fn response_json(response: axum::response::Response) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_slice(
        &to_bytes(response.into_body(), 65_536).await?,
    )?)
}

async fn create_asset(pool: &MySqlPool, prefix: &str) -> Result<u64, Box<dyn Error>> {
    let suffix = Uuid::now_v7().simple().to_string();
    let symbol = format!("{prefix}{}", &suffix[20..]).to_ascii_uppercase();
    Ok(sqlx::query(
        "INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, 18, 'coin', 'active')",
    )
    .bind(&symbol)
    .bind(&symbol)
    .execute(pool)
    .await?
    .last_insert_id())
}

struct RecoveryFixture {
    role_id: u64,
    admin_id: u64,
    base_asset_id: u64,
    quote_asset_id: u64,
    pair_id: u64,
    strategy_id: u64,
    symbol: String,
    seed: String,
    start: chrono::DateTime<Utc>,
}

async fn create_recovery_fixture(pool: &MySqlPool) -> Result<RecoveryFixture, Box<dyn Error>> {
    let suffix = Uuid::now_v7().simple().to_string();
    let role_id =
        sqlx::query("INSERT INTO admin_roles (name, permissions) VALUES (?, JSON_OBJECT())")
            .bind(format!("recovery-role-{suffix}"))
            .execute(pool)
            .await?
            .last_insert_id();
    let admin_id = sqlx::query(
        "INSERT INTO admin_users (username, password_hash, role_id) VALUES (?, 'fixture', ?)",
    )
    .bind(format!("recovery-admin-{suffix}"))
    .bind(role_id)
    .execute(pool)
    .await?
    .last_insert_id();
    let base_asset_id = create_asset(pool, "RB").await?;
    let quote_asset_id = create_asset(pool, "RQ").await?;
    let symbol = format!("RC{}-USDT", &suffix[20..]).to_ascii_uppercase();
    let pair_id = sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, 6, 8, 1, 'active', 'strategy')"#,
    )
    .bind(base_asset_id)
    .bind(quote_asset_id)
    .bind(&symbol)
    .execute(pool)
    .await?
    .last_insert_id();
    // 固定在完整 UTC 日内，使 1d 窗口可由预先铺设的 1m 数据重建。
    let start = Utc.with_ymd_and_hms(2000, 1, 2, 0, 0, 0).unwrap();
    let end = start + Duration::days(1);
    let strategy_id = sqlx::query(
        r#"INSERT INTO market_strategies
           (pair_id, strategy_type, start_price, target_price, start_time, end_time,
            volatility, volume_min, volume_max, status)
           VALUES (?, 'price_path', ?, ?, ?, ?, ?, ?, ?, 'paused')"#,
    )
    .bind(pair_id)
    .bind(decimal("1"))
    .bind(decimal("2"))
    .bind(start)
    .bind(end)
    .bind(decimal("0.01"))
    .bind(decimal("10"))
    .bind(decimal("20"))
    .execute(pool)
    .await?
    .last_insert_id();
    let seed = format!("recovery-seed-{suffix}");
    sqlx::query(
        r#"INSERT INTO strategy_versions
           (strategy_id, version, effective_time, config_json, seed, created_by)
           VALUES (?, 1, ?, ?, ?, ?)"#,
    )
    .bind(strategy_id)
    .bind(start)
    .bind(json!({
        "strategy_type": "price_path",
        "start_price": "1",
        "target_price": "2",
        "start_time": start.timestamp_millis(),
        "end_time": end.timestamp_millis(),
        "volatility": "0.01",
        "volume_min": "10",
        "volume_max": "20",
        "nodes": [],
    }))
    .bind(&seed)
    .bind(admin_id)
    .execute(pool)
    .await?;
    sqlx::query(
        r#"INSERT INTO strategy_runs
           (strategy_id, active_version, run_status, current_price, last_generated_at,
            last_kline_open_time, recovery_status)
           VALUES (?, 1, 'paused', ?, ?, ?, 'idle')"#,
    )
    .bind(strategy_id)
    .bind(decimal("1"))
    .bind(start)
    .bind(start)
    .execute(pool)
    .await?;
    Ok(RecoveryFixture {
        role_id,
        admin_id,
        base_asset_id,
        quote_asset_id,
        pair_id,
        strategy_id,
        symbol,
        seed,
        start,
    })
}

async fn seed_complete_day_except_gap(
    mongo: &Database,
    symbol: &str,
    seed: &str,
    start: chrono::DateTime<Utc>,
    gap_start: chrono::DateTime<Utc>,
    gap_end: chrono::DateTime<Utc>,
) -> Result<String, Box<dyn Error>> {
    let validated = ValidatedMarketSymbol::from_raw(symbol)?;
    let collection_name = kline_collection_name(&validated);
    let collection = mongo.collection::<Document>(&collection_name);
    collection.drop().await.ok();
    let config = SyntheticMarketConfig::new(SyntheticMarketConfig {
        symbol: symbol.to_owned(),
        seed: seed.to_owned(),
        version: 1,
        price_precision: 6,
        start_time: start,
        end_time: start + Duration::days(1),
        start_price: decimal("1"),
        target_price: decimal("2"),
        volatility: decimal("0.01"),
        volume_min: decimal("10"),
        volume_max: decimal("20"),
        generator: Default::default(),
        nodes: Vec::new(),
    })?;
    let documents = (0..1_440_i64)
        .filter_map(|minute| {
            let open_time = start + Duration::minutes(minute);
            if (gap_start..=gap_end).contains(&open_time) {
                return None;
            }
            let candle = config.generate_1m(open_time).expect("valid fixture slot");
            Some(doc! {
                "interval": "1m",
                "open_time": BsonDateTime::from_millis(open_time.timestamp_millis()),
                // 非缺口分钟也使用策略生成器，确保聚合所需的前后 1m 开收连续。
                "open": candle.values.open.to_string(),
                "high": candle.values.high.to_string(),
                "low": candle.values.low.to_string(),
                "close": candle.values.close.to_string(),
                "volume": candle.values.volume.to_string(),
                "source": "fixture",
            })
        })
        .collect::<Vec<_>>();
    collection.insert_many(documents).await?;
    Ok(collection_name)
}

async fn cleanup_fixture(
    pool: &MySqlPool,
    mongo: &Database,
    fixture: &RecoveryFixture,
    collection_name: &str,
) -> Result<(), Box<dyn Error>> {
    mongo
        .collection::<Document>(collection_name)
        .drop()
        .await
        .ok();
    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(fixture.admin_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM kline_recovery_jobs WHERE strategy_id = ?")
        .bind(fixture.strategy_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM strategy_events WHERE strategy_id = ?")
        .bind(fixture.strategy_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM strategy_runs WHERE strategy_id = ?")
        .bind(fixture.strategy_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM strategy_versions WHERE strategy_id = ?")
        .bind(fixture.strategy_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM market_strategies WHERE id = ?")
        .bind(fixture.strategy_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM trading_pairs WHERE id = ?")
        .bind(fixture.pair_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id IN (?, ?)")
        .bind(fixture.base_asset_id)
        .bind(fixture.quote_asset_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(fixture.admin_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(fixture.role_id)
        .execute(pool)
        .await?;
    Ok(())
}

async fn insert_recovery_job(
    pool: &MySqlPool,
    fixture: &RecoveryFixture,
    preview_token: &str,
    range_start: chrono::DateTime<Utc>,
    range_end: chrono::DateTime<Utc>,
    status: &str,
    started_at: Option<chrono::DateTime<Utc>>,
) -> Result<u64, Box<dyn Error>> {
    Ok(sqlx::query(
        r#"INSERT INTO kline_recovery_jobs
           (strategy_id, requested_by, config_version, range_start, range_end,
            preview_token_hash, reason, status, expected_1m_count, started_at)
           VALUES (?, ?, 1, ?, ?, ?, 'reliability retry fixture', ?, ?, ?)"#,
    )
    .bind(fixture.strategy_id)
    .bind(fixture.admin_id)
    .bind(range_start)
    .bind(range_end)
    .bind(hex::encode(Sha256::digest(preview_token.as_bytes())))
    .bind(status)
    .bind(u32::try_from((range_end - range_start).num_minutes())?)
    .bind(started_at)
    .execute(pool)
    .await?
    .last_insert_id())
}

async fn insert_partial_one_minute_candles(
    mongo: &Database,
    collection_name: &str,
    open_times: &[chrono::DateTime<Utc>],
) -> Result<(), Box<dyn Error>> {
    let documents = open_times
        .iter()
        .map(|open_time| {
            doc! {
                "interval": "1m",
                "open_time": BsonDateTime::from_millis(open_time.timestamp_millis()),
                "open": "9", "high": "9", "low": "9", "close": "9", "volume": "9",
                "source": "partial-fixture",
            }
        })
        .collect::<Vec<_>>();
    mongo
        .collection::<Document>(collection_name)
        .insert_many(documents)
        .await?;
    Ok(())
}

#[tokio::test]
async fn http_execute_completes_job_rebuilds_aggregates_and_replays_without_second_run()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let Some(mongo) = mongo_database_or_skip().await? else {
        return Ok(());
    };
    let fixture = create_recovery_fixture(&pool).await?;
    let gap_start = fixture.start + Duration::minutes(5);
    let gap_end = fixture.start + Duration::minutes(10);
    let collection_name = seed_complete_day_except_gap(
        &mongo,
        &fixture.symbol,
        &fixture.seed,
        fixture.start,
        gap_start,
        gap_end - Duration::minutes(1),
    )
    .await?;
    let settings = test_settings();
    let token = issue_token(
        &settings,
        format!("admin:{}", fixture.admin_id),
        TokenScope::Admin,
        900,
    )?;
    // 未装配 Redis：若手动补偿误写 ticker，请求将直接因缺失 Redis 依赖失败。
    let app = build_router(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_mongo(mongo.clone()),
    );
    let preview = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/admin/api/v1/market-strategies/{}/kline-recovery/preview",
                    fixture.strategy_id
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "range_start": gap_start.timestamp_millis(),
                        "range_end": gap_end.timestamp_millis(),
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    let preview_status = preview.status();
    let preview = response_json(preview).await?;
    assert_eq!(preview_status, StatusCode::OK, "payload: {preview}");
    assert_eq!(preview["one_minute_count"], 5);
    let preview_token = preview["preview_token"].as_str().unwrap().to_owned();

    let execute_request = || {
        Request::builder()
            .method("POST")
            .uri(format!(
                "/admin/api/v1/market-strategies/{}/kline-recovery/execute",
                fixture.strategy_id
            ))
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(
                json!({ "preview_token": preview_token, "reason": "restore fixture gap" })
                    .to_string(),
            ))
            .unwrap()
    };
    let executed = app.clone().oneshot(execute_request()).await?;
    let execute_status = executed.status();
    let executed = response_json(executed).await?;
    assert_eq!(execute_status, StatusCode::OK, "payload: {executed}");
    assert_eq!(executed["status"], "completed");
    assert_eq!(executed["actual_1m_count"], 5);
    assert_eq!(executed["actual_aggregate_count"], 5);
    assert!(executed["started_at"].is_number());
    assert!(executed["completed_at"].is_number());
    assert!(executed["error_message"].is_null());
    let job_id = executed["id"].as_u64().unwrap();

    let collection = mongo.collection::<Document>(&collection_name);
    assert_eq!(
        collection
            .count_documents(doc! { "interval": "1m" })
            .await?,
        1_440
    );
    for interval in ["5m", "15m", "1h", "4h", "1d"] {
        let aggregate_open_time = if interval == "5m" {
            fixture.start + Duration::minutes(5)
        } else {
            fixture.start
        };
        assert_eq!(
            collection
                .count_documents(doc! {
                    "interval": interval,
                    "open_time": BsonDateTime::from_millis(aggregate_open_time.timestamp_millis()),
                })
                .await?,
            1,
            "{interval} aggregate must be rebuilt from its complete 1m window"
        );
    }
    let event_types: Vec<String> = sqlx::query_scalar(
        "SELECT event_type FROM strategy_events WHERE strategy_id = ? ORDER BY id",
    )
    .bind(fixture.strategy_id)
    .fetch_all(&pool)
    .await?;
    assert_eq!(
        event_types,
        vec![
            "market_strategy.kline_recovery.requested",
            "market_strategy.kline_recovery.completed",
        ]
    );

    let replayed = app.oneshot(execute_request()).await?;
    let replay_status = replayed.status();
    let replayed = response_json(replayed).await?;
    assert_eq!(replay_status, StatusCode::OK, "payload: {replayed}");
    assert_eq!(replayed["id"], job_id);
    assert_eq!(replayed["status"], "completed");
    let job_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM kline_recovery_jobs WHERE strategy_id = ?")
            .bind(fixture.strategy_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(job_count, 1);
    let event_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM strategy_events WHERE strategy_id = ?")
            .bind(fixture.strategy_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(event_count, 2);

    cleanup_fixture(&pool, &mongo, &fixture, &collection_name).await?;
    Ok(())
}

#[tokio::test]
async fn pending_retry_uses_original_range_after_partial_one_minute_writes()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let Some(mongo) = mongo_database_or_skip().await? else {
        return Ok(());
    };
    let fixture = create_recovery_fixture(&pool).await?;
    let range_start = fixture.start + Duration::minutes(5);
    let range_end = fixture.start + Duration::minutes(10);
    let collection_name = seed_complete_day_except_gap(
        &mongo,
        &fixture.symbol,
        &fixture.seed,
        fixture.start,
        range_start,
        range_end - Duration::minutes(1),
    )
    .await?;
    let settings = test_settings();
    let admin_token = issue_token(
        &settings,
        format!("admin:{}", fixture.admin_id),
        TokenScope::Admin,
        900,
    )?;
    let app = build_router(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_mongo(mongo.clone()),
    );
    let preview = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/admin/api/v1/market-strategies/{}/kline-recovery/preview",
                    fixture.strategy_id
                ))
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "range_start": range_start.timestamp_millis(),
                        "range_end": range_end.timestamp_millis(),
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    let preview = response_json(preview).await?;
    let preview_token = preview["preview_token"].as_str().unwrap().to_owned();
    let job_id = insert_recovery_job(
        &pool,
        &fixture,
        &preview_token,
        range_start,
        range_end,
        "pending",
        None,
    )
    .await?;
    insert_partial_one_minute_candles(
        &mongo,
        &collection_name,
        &[range_start, range_start + Duration::minutes(1)],
    )
    .await?;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/admin/api/v1/market-strategies/{}/kline-recovery/execute",
                    fixture.strategy_id
                ))
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "preview_token": preview_token, "reason": "retry pending job" })
                        .to_string(),
                ))?,
        )
        .await?;
    let status = response.status();
    let response = response_json(response).await?;
    assert_eq!(status, StatusCode::OK, "payload: {response}");
    assert_eq!(response["id"], job_id);
    assert_eq!(response["status"], "completed");
    assert_eq!(response["actual_1m_count"], 5);
    assert_eq!(
        mongo
            .collection::<Document>(&collection_name)
            .count_documents(doc! {
                "interval": "1m",
                "open_time": {
                    "$gte": BsonDateTime::from_millis(range_start.timestamp_millis()),
                    "$lt": BsonDateTime::from_millis(range_end.timestamp_millis()),
                },
            })
            .await?,
        5
    );

    cleanup_fixture(&pool, &mongo, &fixture, &collection_name).await?;
    Ok(())
}

#[tokio::test]
async fn stale_running_is_reclaimed_but_fresh_running_conflicts() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let Some(mongo) = mongo_database_or_skip().await? else {
        return Ok(());
    };
    let fixture = create_recovery_fixture(&pool).await?;
    let range_start = fixture.start + Duration::minutes(5);
    let range_end = fixture.start + Duration::minutes(10);
    let collection_name = seed_complete_day_except_gap(
        &mongo,
        &fixture.symbol,
        &fixture.seed,
        fixture.start,
        range_start,
        range_end - Duration::minutes(1),
    )
    .await?;
    let settings = test_settings();
    let admin_token = issue_token(
        &settings,
        format!("admin:{}", fixture.admin_id),
        TokenScope::Admin,
        900,
    )?;
    let app = build_router(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_mongo(mongo.clone()),
    );
    let preview = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/admin/api/v1/market-strategies/{}/kline-recovery/preview",
                    fixture.strategy_id
                ))
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "range_start": range_start.timestamp_millis(),
                        "range_end": range_end.timestamp_millis(),
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    let preview = response_json(preview).await?;
    let preview_token = preview["preview_token"].as_str().unwrap().to_owned();
    let fresh_started_at = Utc::now();
    let job_id = insert_recovery_job(
        &pool,
        &fixture,
        &preview_token,
        range_start,
        range_end,
        "running",
        Some(fresh_started_at),
    )
    .await?;
    let execute_request = || {
        Request::builder()
            .method("POST")
            .uri(format!(
                "/admin/api/v1/market-strategies/{}/kline-recovery/execute",
                fixture.strategy_id
            ))
            .header(AUTHORIZATION, format!("Bearer {admin_token}"))
            .header("content-type", "application/json")
            .body(Body::from(
                json!({ "preview_token": preview_token, "reason": "reclaim crashed job" })
                    .to_string(),
            ))
            .unwrap()
    };

    let fresh = app.clone().oneshot(execute_request()).await?;
    assert_eq!(fresh.status(), StatusCode::CONFLICT);
    sqlx::query("UPDATE kline_recovery_jobs SET started_at = ? WHERE id = ?")
        .bind(fresh_started_at - Duration::minutes(16))
        .bind(job_id)
        .execute(&pool)
        .await?;
    let stale = app.oneshot(execute_request()).await?;
    let stale_status = stale.status();
    let stale = response_json(stale).await?;
    assert_eq!(stale_status, StatusCode::OK, "payload: {stale}");
    assert_eq!(stale["id"], job_id);
    assert_eq!(stale["status"], "completed");

    cleanup_fixture(&pool, &mongo, &fixture, &collection_name).await?;
    Ok(())
}
