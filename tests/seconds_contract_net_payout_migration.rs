use std::str::FromStr;

use anyhow::{Result, ensure};
use bigdecimal::BigDecimal;
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use url::Url;
use uuid::Uuid;

const MIGRATION: &str = include_str!("../migrations/0125_seconds_contract_net_payout_rates.sql");

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).expect("valid decimal fixture")
}

#[test]
fn net_payout_migration_is_explicit_and_never_rewrites_order_snapshots() {
    let normalized = MIGRATION.to_ascii_lowercase();

    assert_eq!(
        normalized
            .matches("update seconds_contract_products")
            .count(),
        1
    );
    assert_eq!(
        normalized
            .matches("update seconds_contract_product_cycles")
            .count(),
        1
    );
    assert!(normalized.contains("_seconds_net_payout_targets"));
    assert!(normalized.contains("_seconds_net_payout_rate_map"));
    assert!(normalized.contains("pairs.symbol in ('btc-usdt', 'eth-usdt')"));
    assert!(normalized.contains("having count(*) = 4"));
    assert!(normalized.contains("rates.duration_seconds = cycles.duration_seconds"));
    assert!(normalized.contains("rates.gross_rate = cycles.payout_rate"));
    assert!(normalized.contains("rates.gross_rate = products.payout_rate"));
    for confirmed_gross_rate in ["1.40000000", "1.50000000", "1.60000000", "1.80000000"] {
        assert!(
            normalized.contains(confirmed_gross_rate),
            "missing confirmed gross-rate guard: {confirmed_gross_rate}"
        );
    }
    for (duration, gross, net) in [
        ("60", "1.40000000", "0.40000000"),
        ("120", "1.50000000", "0.50000000"),
        ("180", "1.60000000", "0.60000000"),
        ("300", "1.80000000", "0.80000000"),
    ] {
        assert!(
            normalized.contains(&format!("({duration}, {gross}, {net})")),
            "missing explicit gross-to-net mapping: {duration} / {gross} -> {net}"
        );
    }
    assert!(
        !normalized.contains("update seconds_contract_orders"),
        "historical order payout snapshots must stay immutable"
    );
}

#[tokio::test]
async fn net_payout_migration_maps_only_confirmed_configuration_and_is_idempotent() -> Result<()> {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping net payout migration test: DATABASE_URL is not set");
        return Ok(());
    };

    let mut url = Url::parse(&database_url)?;
    url.set_path("/");
    let server = MySqlPoolOptions::new()
        .max_connections(1)
        .connect(url.as_str())
        .await?;
    let database_name = format!("seconds_net_payout_{}", Uuid::now_v7().simple());
    sqlx::query(&format!(
        "CREATE DATABASE `{database_name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
    ))
    .execute(&server)
    .await?;

    url.set_path(&format!("/{database_name}"));
    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect(url.as_str())
        .await?;
    let exercise_result = exercise_migration(&pool).await;
    pool.close().await;
    let cleanup_result = sqlx::query(&format!("DROP DATABASE `{database_name}`"))
        .execute(&server)
        .await;
    server.close().await;

    exercise_result?;
    cleanup_result?;
    Ok(())
}

async fn exercise_migration(pool: &MySqlPool) -> Result<()> {
    sqlx::raw_sql(
        r#"
        CREATE TABLE trading_pairs (
            id BIGINT UNSIGNED PRIMARY KEY,
            symbol VARCHAR(64) NOT NULL UNIQUE
        );
        CREATE TABLE seconds_contract_products (
            id BIGINT UNSIGNED PRIMARY KEY,
            pair_id BIGINT UNSIGNED NOT NULL,
            duration_seconds INT UNSIGNED NOT NULL,
            payout_rate DECIMAL(18,8) NOT NULL
        );
        CREATE TABLE seconds_contract_product_cycles (
            id BIGINT UNSIGNED PRIMARY KEY,
            product_id BIGINT UNSIGNED NOT NULL,
            duration_seconds INT UNSIGNED NOT NULL,
            payout_rate DECIMAL(18,8) NOT NULL
        );
        CREATE TABLE seconds_contract_orders (
            id BIGINT UNSIGNED PRIMARY KEY,
            payout_rate DECIMAL(18,8) NOT NULL
        );

        INSERT INTO trading_pairs (id, symbol) VALUES
            (1, 'BTC-USDT'), (2, 'ETH-USDT'), (3, 'SOL-USDT');
        INSERT INTO seconds_contract_products (id, pair_id, duration_seconds, payout_rate) VALUES
            (1, 1, 60, 1.40000000),  -- confirmed BTC schedule
            (2, 2, 60, 1.40000000),  -- confirmed ETH schedule
            (3, 3, 60, 1.40000000),  -- same values, unconfirmed pair
            (4, 1, 60, 1.40000000),  -- partial custom BTC schedule
            (5, 1, 60, 1.40000000),  -- custom BTC schedule with one changed rate
            (6, 1, 60, 0.40000000),  -- already-normalized BTC schedule
            (7, 1, 120, 1.40000000), -- wrong legacy default duration
            (8, 2, 120, 1.50000000); -- confirmed schedule with another default cycle
        INSERT INTO seconds_contract_product_cycles (id, product_id, duration_seconds, payout_rate) VALUES
            (11, 1, 60, 1.40000000), (12, 1, 120, 1.50000000),
            (13, 1, 180, 1.60000000), (14, 1, 300, 1.80000000),
            (21, 2, 60, 1.40000000), (22, 2, 120, 1.50000000),
            (23, 2, 180, 1.60000000), (24, 2, 300, 1.80000000),
            (31, 3, 60, 1.40000000), (32, 3, 120, 1.50000000),
            (33, 3, 180, 1.60000000), (34, 3, 300, 1.80000000),
            (41, 4, 60, 1.40000000), (42, 4, 120, 1.50000000),
            (51, 5, 60, 1.40000000), (52, 5, 120, 1.50000000),
            (53, 5, 180, 1.70000000), (54, 5, 300, 1.80000000),
            (61, 6, 60, 0.40000000), (62, 6, 120, 0.50000000),
            (63, 6, 180, 0.60000000), (64, 6, 300, 0.80000000),
            (71, 7, 60, 1.40000000), (72, 7, 120, 1.50000000),
            (73, 7, 180, 1.60000000), (74, 7, 300, 1.80000000),
            (81, 8, 60, 1.40000000), (82, 8, 120, 1.50000000),
            (83, 8, 180, 1.60000000), (84, 8, 300, 1.80000000);
        INSERT INTO seconds_contract_orders (id, payout_rate) VALUES
            (101, 1.40000000), (102, 1.80000000), (103, 0.40000000);
        "#,
    )
    .execute(pool)
    .await?;

    let order_snapshots_before = payout_rates(pool, "seconds_contract_orders").await?;
    sqlx::raw_sql(MIGRATION).execute(pool).await?;

    let expected_product_configuration = vec![
        (1, decimal("0.4")),
        (2, decimal("0.4")),
        (3, decimal("1.4")),
        (4, decimal("1.4")),
        (5, decimal("1.4")),
        (6, decimal("0.4")),
        (7, decimal("1.4")),
        (8, decimal("0.5")),
    ];
    let expected_cycle_configuration = vec![
        (1, 60, decimal("0.4")),
        (1, 120, decimal("0.5")),
        (1, 180, decimal("0.6")),
        (1, 300, decimal("0.8")),
        (2, 60, decimal("0.4")),
        (2, 120, decimal("0.5")),
        (2, 180, decimal("0.6")),
        (2, 300, decimal("0.8")),
        (3, 60, decimal("1.4")),
        (3, 120, decimal("1.5")),
        (3, 180, decimal("1.6")),
        (3, 300, decimal("1.8")),
        (4, 60, decimal("1.4")),
        (4, 120, decimal("1.5")),
        (5, 60, decimal("1.4")),
        (5, 120, decimal("1.5")),
        (5, 180, decimal("1.7")),
        (5, 300, decimal("1.8")),
        (6, 60, decimal("0.4")),
        (6, 120, decimal("0.5")),
        (6, 180, decimal("0.6")),
        (6, 300, decimal("0.8")),
        (7, 60, decimal("1.4")),
        (7, 120, decimal("1.5")),
        (7, 180, decimal("1.6")),
        (7, 300, decimal("1.8")),
        (8, 60, decimal("0.4")),
        (8, 120, decimal("0.5")),
        (8, 180, decimal("0.6")),
        (8, 300, decimal("0.8")),
    ];
    ensure!(
        payout_rates(pool, "seconds_contract_products").await? == expected_product_configuration,
        "product master rates were not normalized exactly"
    );
    ensure!(
        cycle_payout_rates(pool).await? == expected_cycle_configuration,
        "cycle rates were not normalized exactly"
    );
    ensure!(
        payout_rates(pool, "seconds_contract_orders").await? == order_snapshots_before,
        "historical order snapshots changed"
    );

    let products_after_first_run = payout_rates(pool, "seconds_contract_products").await?;
    let cycles_after_first_run = cycle_payout_rates(pool).await?;
    sqlx::raw_sql(MIGRATION).execute(pool).await?;
    ensure!(
        payout_rates(pool, "seconds_contract_products").await? == products_after_first_run,
        "second migration execution changed product rates"
    );
    ensure!(
        cycle_payout_rates(pool).await? == cycles_after_first_run,
        "second migration execution changed cycle rates"
    );
    ensure!(
        payout_rates(pool, "seconds_contract_orders").await? == order_snapshots_before,
        "second migration execution changed order snapshots"
    );
    Ok(())
}

async fn payout_rates(pool: &MySqlPool, table: &str) -> Result<Vec<(u64, BigDecimal)>> {
    let query = format!("SELECT id, payout_rate FROM {table} ORDER BY id");
    Ok(sqlx::query_as(&query).fetch_all(pool).await?)
}

async fn cycle_payout_rates(pool: &MySqlPool) -> Result<Vec<(u64, u32, BigDecimal)>> {
    Ok(sqlx::query_as(
        "SELECT product_id, duration_seconds, payout_rate FROM seconds_contract_product_cycles ORDER BY product_id, duration_seconds",
    )
    .fetch_all(pool)
    .await?)
}
