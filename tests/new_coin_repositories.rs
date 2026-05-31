use bigdecimal::BigDecimal;
use chrono::{Duration, Utc};
use exchange_api::modules::new_coin::{
    MySqlNewCoinRepository, NewCoinPurchaseOrderInsert, UnlockFeePaidStatus, UnlockFeePaymentUpdate,
};
use std::{error::Error, str::FromStr};
use uuid::Uuid;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

fn env_or_skip(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => {
            eprintln!("skipping integration test because {name} is not set");
            None
        }
    }
}

#[tokio::test]
async fn mysql_new_coin_purchase_order_and_unlock_fee_status_are_idempotent()
-> Result<(), Box<dyn Error>> {
    let Some(database_url) = env_or_skip("DATABASE_URL") else {
        return Ok(());
    };
    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    let suffix = Uuid::now_v7().simple().to_string();
    let email = format!("new-coin-{suffix}@example.test");
    let base_symbol = format!("N{}", &suffix[..12]);
    let quote_symbol = format!("Q{}", &suffix[12..24]);
    let fee_symbol = format!("U{}", &suffix[20..32]);
    let pair_symbol = format!("{base_symbol}{quote_symbol}");
    let purchase_key = format!("purchase-{suffix}");
    let unlock_key = format!("unlock-{suffix}");
    let now = Utc::now();

    let user_id = sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(&email)
        .bind("test-password-hash")
        .execute(&pool)
        .await?
        .last_insert_id();
    let base_asset_id = insert_asset(&pool, &base_symbol).await?;
    let quote_asset_id = insert_asset(&pool, &quote_symbol).await?;
    let fee_asset_id = insert_asset(&pool, &fee_symbol).await?;
    let pair_id = sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(base_asset_id)
    .bind(quote_asset_id)
    .bind(&pair_symbol)
    .bind(8_i32)
    .bind(8_i32)
    .bind(decimal("0.000000000000000000"))
    .bind("enabled")
    .bind("spot")
    .execute(&pool)
    .await?
    .last_insert_id();
    let project_id = sqlx::query(
        r#"INSERT INTO new_coin_projects
           (asset_id, symbol, lifecycle_status, total_supply, issue_price, listed_at, unlock_type,
            fixed_unlock_at, unlock_fee_enabled, unlock_fee_rate, unlock_fee_basis, unlock_fee_asset, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(base_asset_id)
    .bind(&base_symbol)
    .bind("listed")
    .bind(decimal("1000000.000000000000000000"))
    .bind(decimal("1.000000000000000000"))
    .bind(now.naive_utc())
    .bind("fixed_time")
    .bind((now + Duration::days(7)).naive_utc())
    .bind(true)
    .bind(decimal("0.04000000"))
    .bind("market_value")
    .bind(fee_asset_id)
    .bind("active")
    .execute(&pool)
    .await?
    .last_insert_id();
    let lock_position_id = sqlx::query(
        r#"INSERT INTO asset_lock_positions
           (user_id, asset_id, unlock_type, unlock_at, locked_amount, released_amount,
            remaining_amount, merge_key, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(base_asset_id)
    .bind("fixed_time")
    .bind((now + Duration::days(7)).naive_utc())
    .bind(decimal("25.000000000000000000"))
    .bind(decimal("0.000000000000000000"))
    .bind(decimal("25.000000000000000000"))
    .bind(format!("lock-{suffix}"))
    .bind("active")
    .execute(&pool)
    .await?
    .last_insert_id();
    sqlx::query(
        r#"INSERT INTO asset_unlock_records
           (user_id, asset_id, lock_position_id, unlock_quantity, unlock_price,
            unlock_fee_enabled, unlock_fee_rate, unlock_fee_basis, unlock_fee_asset,
            unlock_fee_amount, fee_paid_status, status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(base_asset_id)
    .bind(lock_position_id)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("5.000000000000000000"))
    .bind(true)
    .bind(decimal("0.04000000"))
    .bind("market_value")
    .bind(fee_asset_id)
    .bind(decimal("2.000000000000000000"))
    .bind("pending")
    .bind("pending")
    .bind(&unlock_key)
    .execute(&pool)
    .await?;

    let repository = MySqlNewCoinRepository::new(pool.clone());
    let first_order = repository
        .insert_purchase_order(NewCoinPurchaseOrderInsert {
            project_id,
            user_id,
            pair_id,
            base_asset_id,
            quote_asset_id,
            price: decimal("2.000000000000000000"),
            quantity: decimal("25.000000000000000000"),
            quote_amount: decimal("50.000000000000000000"),
            lock_position_id: Some(lock_position_id),
            status: "pending".to_owned(),
            idempotency_key: purchase_key.clone(),
        })
        .await
        .unwrap();
    let duplicate_order = repository
        .insert_purchase_order(NewCoinPurchaseOrderInsert {
            project_id,
            user_id,
            pair_id,
            base_asset_id,
            quote_asset_id,
            price: decimal("2.000000000000000000"),
            quantity: decimal("25.000000000000000000"),
            quote_amount: decimal("50.000000000000000000"),
            lock_position_id: Some(lock_position_id),
            status: "pending".to_owned(),
            idempotency_key: purchase_key.clone(),
        })
        .await
        .unwrap();

    assert!(first_order.inserted);
    assert!(!duplicate_order.inserted);
    assert_eq!(first_order.order_id, duplicate_order.order_id);
    assert_eq!(
        repository
            .unlock_fee_paid_status(&unlock_key, user_id)
            .await
            .unwrap(),
        Some(UnlockFeePaidStatus::Pending)
    );

    assert!(
        repository
            .mark_unlock_fee_paid(UnlockFeePaymentUpdate {
                unlock_idempotency_key: unlock_key.clone(),
                user_id,
                payment_asset_id: fee_asset_id,
                amount: decimal("2.000000000000000000"),
            })
            .await
            .unwrap()
    );
    assert_eq!(
        repository
            .unlock_fee_paid_status(&unlock_key, user_id)
            .await
            .unwrap(),
        Some(UnlockFeePaidStatus::Paid)
    );

    cleanup_new_coin_fixture(
        &pool,
        NewCoinFixtureCleanup {
            purchase_key: &purchase_key,
            unlock_key: &unlock_key,
            lock_position_id,
            project_id,
            pair_id,
            asset_ids: [base_asset_id, quote_asset_id, fee_asset_id],
            user_id,
        },
    )
    .await?;
    Ok(())
}

struct NewCoinFixtureCleanup<'a> {
    purchase_key: &'a str,
    unlock_key: &'a str,
    lock_position_id: u64,
    project_id: u64,
    pair_id: u64,
    asset_ids: [u64; 3],
    user_id: u64,
}

async fn insert_asset(pool: &sqlx::Pool<sqlx::MySql>, symbol: &str) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query("INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, ?, ?, ?)")
        .bind(symbol)
        .bind(symbol)
        .bind(18_i32)
        .bind("coin")
        .bind("active")
        .execute(pool)
        .await?
        .last_insert_id())
}

async fn cleanup_new_coin_fixture(
    pool: &sqlx::Pool<sqlx::MySql>,
    fixture: NewCoinFixtureCleanup<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM new_coin_purchase_orders WHERE idempotency_key = ?")
        .bind(fixture.purchase_key)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM asset_unlock_records WHERE idempotency_key = ?")
        .bind(fixture.unlock_key)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM asset_lock_positions WHERE id = ?")
        .bind(fixture.lock_position_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM new_coin_projects WHERE id = ?")
        .bind(fixture.project_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM trading_pairs WHERE id = ?")
        .bind(fixture.pair_id)
        .execute(pool)
        .await?;
    for asset_id in fixture.asset_ids {
        sqlx::query("DELETE FROM assets WHERE id = ?")
            .bind(asset_id)
            .execute(pool)
            .await?;
    }
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(fixture.user_id)
        .execute(pool)
        .await?;
    Ok(())
}
