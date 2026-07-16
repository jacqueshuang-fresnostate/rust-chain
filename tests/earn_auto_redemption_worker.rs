use bigdecimal::BigDecimal;
use chrono::{TimeZone, Utc};
use exchange_api::{
    config::Settings,
    modules::events::{EventBroadcastHub, WebSocketChannel},
    state::AppState,
    workers::earn_auto_redemption::{
        EarnAutoRedemptionWorker, run_once_with_broadcast, run_once_with_dependencies,
    },
};
use secrecy::SecretString;
use serde_json::{Value, json};
use sqlx::{MySql, MySqlPool, Transaction, mysql::MySqlPoolOptions};
use std::{error::Error, str::FromStr, time::Duration};
use tokio::{sync::Mutex, time::timeout};
use uuid::Uuid;

static TEST_LOCK: Mutex<()> = Mutex::const_new(());

#[derive(Debug)]
struct EarnFixture {
    user_id: u64,
    asset_id: u64,
    product_id: u64,
    subscription_id: u64,
}

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

fn test_settings() -> Settings {
    Settings {
        app_env: "test".to_owned(),
        app_host: "127.0.0.1".parse().unwrap(),
        app_port: 0,
        database_url: SecretString::new("mysql://test:test@localhost/test".to_owned()),
        mongodb_uri: SecretString::new("mongodb://localhost:27017".to_owned()),
        mongodb_database: "exchange_test".to_owned(),
        redis_url: SecretString::new("redis://localhost:6379".to_owned()),
        rabbitmq_url: SecretString::new("amqp://guest:guest@localhost:5672/%2f".to_owned()),
        jwt_secret: SecretString::new("test-secret".to_owned()),
        credential_encryption_key: Some(SecretString::new(
            "0123456789abcdef0123456789abcdef".to_owned(),
        )),
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
        event_outbox_publisher_enabled: true,
        event_outbox_publisher_interval_seconds: 5,
        unlock_scanner_enabled: true,
        unlock_scanner_interval_seconds: 10,
        unlock_scanner_batch_limit: 100,
        kline_recovery_enabled: true,
        kline_recovery_interval_seconds: 30,
        kline_recovery_batch_limit: 100,
        seconds_contract_settlement_enabled: true,
        seconds_contract_settlement_interval_seconds: 5,
        seconds_contract_settlement_batch_limit: 100,
        earn_auto_redemption_enabled: true,
        earn_auto_redemption_interval_seconds: 60,
        earn_auto_redemption_batch_limit: 100,
        margin_liquidation_enabled: true,
        margin_liquidation_interval_seconds: 5,
        margin_liquidation_batch_limit: 100,
        margin_interest_enabled: true,
        margin_interest_interval_seconds: 60,
        margin_interest_batch_limit: 100,
    }
}

fn env_or_skip(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => {
            eprintln!("skipping earn auto redemption worker test because {name} is not set");
            None
        }
    }
}

async fn mysql_pool_or_skip() -> Result<Option<MySqlPool>, Box<dyn Error>> {
    let Some(database_url) = env_or_skip("DATABASE_URL") else {
        return Ok(None);
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(Some(pool))
}

async fn close_previous_earn_worker_subscriptions(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE earn_subscriptions SET status = 'redeemed' WHERE idempotency_key LIKE 'earn-worker-%' AND status = 'subscribed'",
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn create_user(tx: &mut Transaction<'_, MySql>) -> Result<u64, sqlx::Error> {
    let email = format!("earn-worker-{}@example.test", Uuid::now_v7().simple());
    Ok(
        sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
            .bind(email)
            .bind("not-a-real-hash")
            .execute(&mut **tx)
            .await?
            .last_insert_id(),
    )
}

async fn create_asset(tx: &mut Transaction<'_, MySql>) -> Result<u64, sqlx::Error> {
    let suffix = Uuid::now_v7().simple().to_string();
    let symbol = format!("EW{}", &suffix[20..32]).to_ascii_uppercase();
    Ok(sqlx::query(
        "INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, 18, 'coin', 'active')",
    )
    .bind(&symbol)
    .bind(format!("{symbol} asset"))
    .execute(&mut **tx)
    .await?
    .last_insert_id())
}

fn default_introduction_json(name: &str) -> Value {
    json!({
        "version": 1,
        "default_locale": "zh-CN",
        "items": [
            {
                "locale": "zh-CN",
                "country": "CN",
                "title": name,
                "content": [
                    { "type": "p", "children": [{ "text": name }] }
                ]
            }
        ]
    })
}

async fn seed_matured_subscription(
    pool: &MySqlPool,
    now: chrono::DateTime<Utc>,
    matures_at: chrono::DateTime<Utc>,
) -> Result<EarnFixture, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let user_id = create_user(&mut tx).await?;
    let asset_id = create_asset(&mut tx).await?;
    let product_name = format!("Earn Worker {}", Uuid::now_v7().simple());
    let product_id = sqlx::query(
        r#"INSERT INTO earn_products
           (asset_id, name, category, introduction_json, term_days, apr_rate, min_subscribe, max_subscribe, status)
           VALUES (?, ?, 'fixed_term', ?, 365, ?, ?, NULL, 'active')"#,
    )
    .bind(asset_id)
    .bind(&product_name)
    .bind(sqlx::types::Json(default_introduction_json(&product_name)))
    .bind(decimal("0.10000000"))
    .bind(decimal("10.000000000000000000"))
    .execute(&mut *tx)
    .await?
    .last_insert_id();
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(asset_id)
        .bind(decimal("80.000000000000000000"))
        .execute(&mut *tx)
        .await?;
    let idempotency_key = format!("earn-worker-{}", Uuid::now_v7().simple());
    let amount = decimal("20.000000000000000000");
    let subscription_id = sqlx::query(
        r#"INSERT INTO earn_subscriptions
           (user_id, product_id, asset_id, amount, apr_rate, term_days, status,
            idempotency_key, subscribed_at, matures_at)
           VALUES (?, ?, ?, ?, ?, 365, 'subscribed', ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(asset_id)
    .bind(&amount)
    .bind(decimal("0.10000000"))
    .bind(&idempotency_key)
    .bind((now - chrono::TimeDelta::days(365)).naive_utc())
    .bind(matures_at.naive_utc())
    .execute(&mut *tx)
    .await?
    .last_insert_id();
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, 'earn_subscribe', ?, 'available', ?, ?, 0, 0, 'earn_subscription', ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(-amount)
    .bind(decimal("80.000000000000000000"))
    .bind(decimal("80.000000000000000000"))
    .bind(subscription_id.to_string())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(EarnFixture {
        user_id,
        asset_id,
        product_id,
        subscription_id,
    })
}

async fn cleanup_fixture(pool: &MySqlPool, fixture: EarnFixture) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM wallet_ledger WHERE ref_type = 'earn_subscription' AND ref_id = ?")
        .bind(fixture.subscription_id.to_string())
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM earn_subscriptions WHERE id = ?")
        .bind(fixture.subscription_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM earn_products WHERE id = ?")
        .bind(fixture.product_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
        .bind(fixture.user_id)
        .bind(fixture.asset_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(fixture.asset_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(fixture.user_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn earn_auto_redemption_worker_redeems_matured_subscription_idempotently()
-> Result<(), Box<dyn Error>> {
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    close_previous_earn_worker_subscriptions(&pool).await?;
    let now = Utc.with_ymd_and_hms(1990, 1, 1, 12, 0, 0).unwrap();
    let fixture =
        seed_matured_subscription(&pool, now, now - chrono::TimeDelta::seconds(1)).await?;
    sqlx::query("UPDATE earn_subscriptions SET maturity_profit_fee_rate = ? WHERE id = ?")
        .bind(decimal("0.10000000"))
        .bind(fixture.subscription_id)
        .execute(&pool)
        .await?;

    let hub = EventBroadcastHub::new(16);
    let _keepalive_hub = hub.clone();
    let mut private_events = hub.subscribe(&WebSocketChannel::private_user(fixture.user_id));

    let summary = run_once_with_broadcast(&pool, Some(&hub), now, 10).await?;

    assert_eq!(summary.scanned, 1);
    assert_eq!(summary.redeemed, 1);
    assert_eq!(summary.skipped, 0);
    assert_eq!(summary.failed, 0);
    let event_message = timeout(Duration::from_millis(100), private_events.recv()).await??;
    let event: serde_json::Value = serde_json::from_str(event_message.payload())?;
    assert_eq!(event["type"], "earn.subscription.redeemed");
    assert_eq!(event["subscription_id"], fixture.subscription_id);
    assert_eq!(event["asset_id"], fixture.asset_id);
    assert_eq!(event["principal_amount"], "20.000000000000000000");
    assert_eq!(event["gross_yield_amount"], "2.000000000000000000");
    assert_eq!(event["yield_amount"], "1.800000000000000000");
    assert_eq!(event["maturity_profit_fee_amount"], "0.200000000000000000");
    assert_eq!(event["fee_amount"], "0.200000000000000000");
    assert_eq!(event["redeem_amount"], "21.800000000000000000");
    assert_eq!(event["status"], "redeemed");
    let (status, redeemed_at): (String, Option<chrono::DateTime<Utc>>) =
        sqlx::query_as("SELECT status, redeemed_at FROM earn_subscriptions WHERE id = ?")
            .bind(fixture.subscription_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(status, "redeemed");
    assert!(redeemed_at.is_some());
    let (available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(fixture.user_id)
            .bind(fixture.asset_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(available, decimal("101.800000000000000000"));
    let (redeem_amount, ledger_count): (BigDecimal, i64) = sqlx::query_as(
        r#"SELECT COALESCE(SUM(amount), 0), COUNT(*)
           FROM wallet_ledger
           WHERE ref_type = 'earn_subscription'
             AND ref_id = ?
             AND change_type = 'earn_redeem'"#,
    )
    .bind(fixture.subscription_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(redeem_amount, decimal("21.800000000000000000"));
    assert_eq!(ledger_count, 1);

    let idempotent = run_once_with_broadcast(&pool, Some(&hub), now, 10).await?;

    assert_eq!(idempotent.scanned, 0);
    assert_eq!(idempotent.redeemed, 0);
    assert!(
        timeout(Duration::from_millis(25), private_events.recv())
            .await
            .is_err(),
        "idempotent earn auto redemption replay must not publish duplicate private event"
    );
    let (ledger_count_after,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'earn_subscription' AND ref_id = ? AND change_type = 'earn_redeem'",
    )
    .bind(fixture.subscription_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count_after, 1);
    cleanup_fixture(&pool, fixture).await?;
    Ok(())
}

#[tokio::test]
async fn earn_auto_redemption_worker_scans_past_broken_rows() -> Result<(), Box<dyn Error>> {
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    close_previous_earn_worker_subscriptions(&pool).await?;
    let now = Utc.with_ymd_and_hms(1990, 1, 2, 12, 0, 0).unwrap();
    let broken = seed_matured_subscription(&pool, now, now - chrono::TimeDelta::seconds(2)).await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
        .bind(broken.user_id)
        .bind(broken.asset_id)
        .execute(&pool)
        .await?;
    let healthy =
        seed_matured_subscription(&pool, now, now - chrono::TimeDelta::seconds(1)).await?;

    let summary = run_once_with_dependencies(&pool, now, 1).await?;

    assert_eq!(summary.scanned, 2);
    assert_eq!(summary.redeemed, 1);
    assert_eq!(summary.failed, 1);
    let (broken_status,): (String,) =
        sqlx::query_as("SELECT status FROM earn_subscriptions WHERE id = ?")
            .bind(broken.subscription_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(broken_status, "subscribed");
    let (healthy_status,): (String,) =
        sqlx::query_as("SELECT status FROM earn_subscriptions WHERE id = ?")
            .bind(healthy.subscription_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(healthy_status, "redeemed");

    cleanup_fixture(&pool, broken).await?;
    cleanup_fixture(&pool, healthy).await?;
    Ok(())
}

#[tokio::test]
async fn earn_auto_redemption_worker_run_once_uses_pool_and_limit() -> Result<(), Box<dyn Error>> {
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    close_previous_earn_worker_subscriptions(&pool).await?;
    let now = Utc.with_ymd_and_hms(1990, 1, 3, 12, 0, 0).unwrap();
    let first = seed_matured_subscription(&pool, now, now - chrono::TimeDelta::seconds(2)).await?;
    let second = seed_matured_subscription(&pool, now, now - chrono::TimeDelta::seconds(1)).await?;
    let worker = EarnAutoRedemptionWorker;
    let state = AppState::new(test_settings()).with_mysql(pool.clone());

    let summary = worker.run_once(&state, now, 1).await?;

    assert_eq!(summary.scanned, 1);
    assert_eq!(summary.redeemed, 1);
    let (remaining_subscribed,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM earn_subscriptions WHERE id IN (?, ?) AND status = 'subscribed'",
    )
    .bind(first.subscription_id)
    .bind(second.subscription_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(remaining_subscribed, 1);

    cleanup_fixture(&pool, first).await?;
    cleanup_fixture(&pool, second).await?;
    Ok(())
}
