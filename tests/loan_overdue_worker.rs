use bigdecimal::BigDecimal;
use chrono::{DateTime, TimeDelta, Utc};
use exchange_api::workers::loan_overdue::{LoanOverdueWorkerConfig, run_once_with_dependencies};
use serde_json::json;
use sqlx::{MySqlPool, mysql::MySqlPoolOptions, types::Json as SqlxJson};
use std::{error::Error, str::FromStr};
use tokio::sync::Mutex;
use uuid::Uuid;

static TEST_LOCK: Mutex<()> = Mutex::const_new(());

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

async fn mysql_pool() -> Option<MySqlPool> {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!("skipping loan overdue worker test because DATABASE_URL is not set");
            return None;
        }
    };

    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    Some(pool)
}

async fn create_user(pool: &MySqlPool) -> u64 {
    let email = format!("loan-overdue-{}@example.test", Uuid::now_v7().simple());
    sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(email)
        .bind("not-a-real-hash")
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id()
}

async fn create_asset(pool: &MySqlPool) -> u64 {
    let suffix = Uuid::now_v7().simple().to_string();
    let symbol = format!("LO{}", &suffix[suffix.len() - 12..]).to_ascii_uppercase();
    sqlx::query(
        "INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, 18, 'coin', 'active')",
    )
    .bind(&symbol)
    .bind(&symbol)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id()
}

async fn create_product(pool: &MySqlPool, asset_id: u64) -> u64 {
    let name = format!("loan-overdue-{}", Uuid::now_v7().simple());
    sqlx::query(
        r#"INSERT INTO loan_products
           (loan_type, asset_id, name, name_json, term_days, interest_rate,
            interest_calculation_mode, min_kyc_level, min_amount, max_amount, status)
           VALUES ('credit', ?, ?, ?, 30, 0.02, 'full_term', 0, 1, NULL, 'active')"#,
    )
    .bind(asset_id)
    .bind(&name)
    .bind(SqlxJson(json!({
        "version": 1,
        "default_locale": "zh-CN",
        "items": [{ "locale": "zh-CN", "country": "CN", "title": name }]
    })))
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id()
}

async fn seed_loan_order(
    pool: &MySqlPool,
    user_id: u64,
    product_id: u64,
    asset_id: u64,
    status: &str,
    due_at: DateTime<Utc>,
) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        r#"INSERT INTO loan_orders
           (user_id, product_id, loan_type, asset_id, amount, interest_rate,
            interest_calculation_mode, term_days, min_kyc_level, status, idempotency_key,
            request_fingerprint, disbursed_at, due_at)
           VALUES (?, ?, 'credit', ?, ?, 0.02, 'full_term', 30, 0, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(asset_id)
    .bind(decimal("100.000000000000000000"))
    .bind(status)
    .bind(Uuid::now_v7().simple().to_string())
    .bind("0".repeat(64))
    .bind((due_at - TimeDelta::days(30)).naive_utc())
    .bind(due_at.naive_utc())
    .execute(pool)
    .await?
    .last_insert_id())
}

async fn load_order_state(
    pool: &MySqlPool,
    order_id: u64,
) -> Result<(String, Option<DateTime<Utc>>), sqlx::Error> {
    sqlx::query_as("SELECT status, overdue_at FROM loan_orders WHERE id = ?")
        .bind(order_id)
        .fetch_one(pool)
        .await
}

async fn cleanup_fixture(
    pool: &MySqlPool,
    user_id: u64,
    asset_id: u64,
    product_id: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM loan_orders WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM loan_products WHERE id = ?")
        .bind(product_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(asset_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn loan_overdue_worker_marks_past_due_order_and_is_idempotent() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let _guard = TEST_LOCK.lock().await;
    let now = Utc::now();
    let user_id = create_user(&pool).await;
    let asset_id = create_asset(&pool).await;
    let product_id = create_product(&pool, asset_id).await;
    let overdue_order_id = seed_loan_order(
        &pool,
        user_id,
        product_id,
        asset_id,
        "disbursed",
        now - TimeDelta::days(1),
    )
    .await?;
    let pending_due_order_id = seed_loan_order(
        &pool,
        user_id,
        product_id,
        asset_id,
        "disbursed",
        now + TimeDelta::days(1),
    )
    .await?;

    // 扫描覆盖全表，因此只对本用例播种的订单断言，避免其他夹具残留翻转全局计数。
    let outcome: Result<(), Box<dyn Error>> = async {
        let first = run_once_with_dependencies(&pool, now, 100).await?;
        assert!(first.marked >= 1);

        let (status, overdue_at) = load_order_state(&pool, overdue_order_id).await?;
        assert_eq!(status, "overdue");
        let marked_at = overdue_at.expect("overdue_at is recorded");
        let (untouched_status, untouched_overdue_at) =
            load_order_state(&pool, pending_due_order_id).await?;
        assert_eq!(untouched_status, "disbursed");
        assert!(untouched_overdue_at.is_none());

        run_once_with_dependencies(&pool, now, 100).await?;

        let (status, overdue_at) = load_order_state(&pool, overdue_order_id).await?;
        assert_eq!(status, "overdue");
        assert_eq!(overdue_at, Some(marked_at));
        Ok(())
    }
    .await;

    cleanup_fixture(&pool, user_id, asset_id, product_id).await?;
    outcome
}

#[tokio::test]
async fn loan_overdue_worker_skips_orders_that_are_not_disbursed() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let _guard = TEST_LOCK.lock().await;
    let now = Utc::now();
    let user_id = create_user(&pool).await;
    let asset_id = create_asset(&pool).await;
    let product_id = create_product(&pool, asset_id).await;
    let past_due = now - TimeDelta::days(2);
    let pending_order_id =
        seed_loan_order(&pool, user_id, product_id, asset_id, "pending", past_due).await?;
    let repaid_order_id =
        seed_loan_order(&pool, user_id, product_id, asset_id, "repaid", past_due).await?;

    let outcome: Result<(), Box<dyn Error>> = async {
        run_once_with_dependencies(&pool, now, 100).await?;

        let (pending_status, _) = load_order_state(&pool, pending_order_id).await?;
        let (repaid_status, _) = load_order_state(&pool, repaid_order_id).await?;
        assert_eq!(pending_status, "pending");
        assert_eq!(repaid_status, "repaid");
        Ok(())
    }
    .await;

    cleanup_fixture(&pool, user_id, asset_id, product_id).await?;
    outcome
}

#[test]
fn loan_overdue_worker_ships_enabled_for_periodic_health_scans() {
    assert!(LoanOverdueWorkerConfig::from_env().enabled);
}
