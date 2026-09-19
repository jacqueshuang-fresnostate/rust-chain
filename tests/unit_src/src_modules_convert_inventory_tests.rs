use super::*;

fn local_test_options(url: &str) -> Result<sqlx::mysql::MySqlConnectOptions, &'static str> {
    let options: sqlx::mysql::MySqlConnectOptions = url
        .parse()
        .map_err(|_| "invalid inventory test database URL")?;
    if !matches!(options.get_host(), "127.0.0.1" | "localhost" | "::1")
        || options.get_socket().is_some()
        || !options
            .get_database()
            .is_some_and(|name| name.ends_with("_test"))
    {
        return Err("inventory tests require a loopback TCP host and a dedicated _test database");
    }
    Ok(options)
}

#[test]
fn inventory_test_database_is_explicit_and_local() {
    assert!(local_test_options("mysql://root@127.0.0.1:13316/e06_pool_inventory_test").is_ok());
    for url in [
        "mysql://root@db.example/e06_pool_inventory_test",
        "mysql://root@localhost/production",
        "mysql://root@localhost",
        "mysql://root@localhost/e06_test?socket=/tmp/mysql.sock",
    ] {
        assert!(local_test_options(url).is_err(), "unsafe test URL: {url}");
    }
}

#[tokio::test]
async fn explicit_inventory_concurrency_rollback_replay_and_revision() {
    let Ok(url) = std::env::var("CONVERT_INVENTORY_DATABASE_URL") else {
        eprintln!("skipping inventory database test: CONVERT_INVENTORY_DATABASE_URL is unset");
        return;
    };
    let options = local_test_options(&url).expect("isolated inventory test database");
    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(12)
        .connect_with(options)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let suffix = uuid::Uuid::now_v7().simple().to_string();
    let mut assets = Vec::new();
    for prefix in ["IF", "IT"] {
        let id = sqlx::query("INSERT INTO assets(symbol,name,precision_scale,asset_type,status) VALUES(?,?,2,'coin','active')")
            .bind(format!("{prefix}{}", &suffix[16..])).bind("inventory test")
            .execute(&pool).await.unwrap().last_insert_id();
        assets.push(id);
    }
    let pair = sqlx::query("INSERT INTO convert_pairs(from_asset,to_asset,pricing_mode,spread_rate,min_amount,enabled) VALUES(?,?,'fixed',0,1,TRUE)")
        .bind(assets[0]).bind(assets[1]).execute(&pool).await.unwrap().last_insert_id();
    let asset = assets[1];
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM convert_pairs WHERE id = ? FOR UPDATE")
        .bind(pair)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    consume_output_in_tx(
        &mut tx,
        pair,
        &uuid::Uuid::now_v7().to_string(),
        asset,
        &BigDecimal::from(100),
    )
    .await
    .unwrap();
    configure_inventory_in_tx(
        &mut tx,
        pair,
        asset,
        1,
        0,
        true,
        &BigDecimal::from(7),
        "test deposit",
        "test",
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..32 {
        let pool = pool.clone();
        tasks.spawn(async move {
            let mut tx = pool.begin().await.unwrap();
            sqlx::query("SELECT id FROM convert_pairs WHERE id = ? FOR UPDATE")
                .bind(pair)
                .fetch_one(&mut *tx)
                .await
                .unwrap();
            let quote = uuid::Uuid::now_v7().to_string();
            let result =
                consume_output_in_tx(&mut tx, pair, &quote, asset, &BigDecimal::from(1)).await;
            if result.is_ok() {
                consume_output_in_tx(&mut tx, pair, &quote, asset, &BigDecimal::from(1))
                    .await
                    .unwrap();
                tx.commit().await.unwrap();
                true
            } else {
                assert!(matches!(result, Err(AppError::Validation(_))));
                tx.rollback().await.unwrap();
                false
            }
        });
    }
    let mut accepted = 0;
    while let Some(result) =
        tokio::time::timeout(std::time::Duration::from_secs(30), tasks.join_next())
            .await
            .unwrap()
    {
        accepted += u32::from(result.unwrap());
    }
    assert_eq!(accepted, 7);
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM convert_pairs WHERE id = ? FOR UPDATE")
        .bind(pair)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert!(
        configure_inventory_in_tx(
            &mut tx,
            pair,
            asset,
            1,
            0,
            true,
            &BigDecimal::from(8),
            "stale",
            "test"
        )
        .await
        .is_err()
    );
    assert!(
        configure_inventory_in_tx(
            &mut tx,
            pair,
            asset,
            1,
            1,
            true,
            &BigDecimal::from(6),
            "too low",
            "test"
        )
        .await
        .is_err()
    );
    assert!(
        configure_inventory_in_tx(
            &mut tx,
            pair,
            asset,
            1,
            1,
            true,
            &"8.001".parse().unwrap(),
            "precision",
            "test"
        )
        .await
        .is_err()
    );
    configure_inventory_in_tx(
        &mut tx,
        pair,
        asset,
        1,
        1,
        true,
        &BigDecimal::from(8),
        "top up",
        "test",
    )
    .await
    .unwrap();
    consume_output_in_tx(
        &mut tx,
        pair,
        &uuid::Uuid::now_v7().to_string(),
        asset,
        &BigDecimal::from(1),
    )
    .await
    .unwrap();
    tx.rollback().await.unwrap();
    let row: (BigDecimal, BigDecimal, u64) = sqlx::query_as("SELECT funded_amount,consumed_amount,revision FROM convert_inventory_accounts WHERE pair_id=?")
        .bind(pair).fetch_one(&pool).await.unwrap();
    assert_eq!(row, (BigDecimal::from(7), BigDecimal::from(7), 1));
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM convert_inventory_allocations WHERE pair_id=?")
            .bind(pair)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 7);
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM convert_pairs WHERE id = ? FOR UPDATE")
        .bind(pair)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    configure_inventory_in_tx(
        &mut tx,
        pair,
        asset,
        1,
        1,
        false,
        &BigDecimal::from(7),
        "disable",
        "test",
    )
    .await
    .unwrap();
    consume_output_in_tx(
        &mut tx,
        pair,
        &uuid::Uuid::now_v7().to_string(),
        asset,
        &BigDecimal::from(99),
    )
    .await
    .unwrap();
    assert!(
        ensure_inventory_asset_in_tx(&mut tx, pair, assets[0])
            .await
            .is_err()
    );
    tx.commit().await.unwrap();
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM convert_inventory_funding_audits WHERE pair_id=?")
            .bind(pair)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 2);
    pool.close().await;
}
