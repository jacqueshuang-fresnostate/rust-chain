use super::*;

#[tokio::test]
async fn spot_explicit_trigger_migration_preserves_legacy_rows_without_backfill()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let schema = format!("e03_migration_{}", Uuid::now_v7().simple());
    sqlx::query(&format!("CREATE DATABASE `{schema}`"))
        .execute(&pool)
        .await?;
    let options = sqlx::mysql::MySqlConnectOptions::from_str(&std::env::var("DATABASE_URL")?)?
        .database(&schema);
    let fixture_pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;
    let mut connection = fixture_pool.acquire().await?;
    sqlx::raw_sql(
        "CREATE TEMPORARY TABLE spot_orders (
            id BIGINT PRIMARY KEY, order_type VARCHAR(32), side VARCHAR(16),
            trigger_price DECIMAL(38,18), status VARCHAR(32));
         INSERT INTO spot_orders VALUES
            (1, 'stop_limit', 'buy', 10, 'pending'),
            (2, 'stop_limit', 'sell', 20, 'filled'),
            (3, 'limit', 'buy', NULL, 'open');",
    )
    .execute(&mut *connection)
    .await?;
    let before: Vec<(i64, String, String, Option<BigDecimal>, String)> = sqlx::query_as(
        "SELECT id, order_type, side, trigger_price, status FROM spot_orders ORDER BY id",
    )
    .fetch_all(&mut *connection)
    .await?;
    sqlx::raw_sql(include_str!(
        "../../migrations/0132_spot_explicit_trigger_direction.sql"
    ))
    .execute(&mut *connection)
    .await?;
    let after: Vec<(i64, String, String, Option<BigDecimal>, String)> = sqlx::query_as(
        "SELECT id, order_type, side, trigger_price, status FROM spot_orders ORDER BY id",
    )
    .fetch_all(&mut *connection)
    .await?;
    assert_eq!(before, after);
    let metadata: Vec<(Option<String>, Option<chrono::DateTime<Utc>>)> =
        sqlx::query_as("SELECT trigger_direction, triggered_at FROM spot_orders")
            .fetch_all(&mut *connection)
            .await?;
    assert_eq!(metadata, vec![(None, None); 3]);
    assert!(
        sqlx::query("UPDATE spot_orders SET trigger_direction = 'sideways' WHERE id = 1")
            .execute(&mut *connection)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE spot_orders SET triggered_at = NOW(6) WHERE id = 1")
            .execute(&mut *connection)
            .await
            .is_err()
    );
    sqlx::query("DROP TEMPORARY TABLE spot_orders")
        .execute(&mut *connection)
        .await?;
    drop(connection);
    fixture_pool.close().await;
    sqlx::query(&format!("DROP DATABASE `{schema}`"))
        .execute(&pool)
        .await?;
    Ok(())
}

async fn create(app: &axum::Router, token: &str, body: &Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 65_536)
        .await
        .unwrap();
    let payload = if status == StatusCode::UNPROCESSABLE_ENTITY {
        Value::String(String::from_utf8(bytes.to_vec()).unwrap())
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, payload)
}

async fn trigger_state(
    pool: &MySqlPool,
    id: &str,
) -> (Option<String>, Option<chrono::DateTime<Utc>>, String) {
    sqlx::query_as("SELECT trigger_direction, triggered_at, status FROM spot_orders WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn spot_explicit_trigger_persists_across_limit_inventory_failure_and_reconnect()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    for (side, direction, limit, execution) in [
        ("buy", "rising", "8", "8"),
        ("sell", "falling", "12", "12"),
        ("buy", "falling", "12", "11"),
        ("sell", "rising", "8", "9"),
    ] {
        let user = create_user(&pool).await;
        let (base, base_symbol) = create_asset(&pool, "TB").await;
        let (quote, quote_symbol) = create_asset(&pool, "TQ").await;
        let pair = create_pair(&pool, base, quote, &base_symbol, &quote_symbol).await;
        let reserve_asset = if side == "buy" { quote } else { base };
        sqlx::query(
            "INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, 100)",
        )
        .bind(user)
        .bind(reserve_asset)
        .execute(&pool)
        .await?;
        let settings = test_settings();
        let token = issue_token(&settings, format!("user:{user}"), TokenScope::User, 900)?;
        let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
        let body = serde_json::json!({
            "pair_id": pair, "side": side, "order_type": "stop_limit", "price": limit,
            "trigger_price": "10", "trigger_direction": direction, "quantity": "2",
            "idempotency_key": format!("e03-{}", Uuid::now_v7()),
        });
        let (status, first) = create(&app, &token, &body).await;
        assert_eq!(status, StatusCode::OK, "{first}");
        assert_eq!(first["trigger_direction"], direction);
        assert!(first["triggered_at"].is_null());
        let id = first["id"].as_str().unwrap();
        let before_threshold = if direction == "rising" { "9" } else { "11" };
        execute_triggered_spot_limit_orders_with_hub(
            &pool,
            &pair,
            &decimal(before_threshold),
            None,
        )
        .await?;
        assert!(trigger_state(&pool, id).await.1.is_none());

        assert_eq!(
            execute_triggered_spot_limit_orders_with_hub(&pool, &pair, &decimal("10"), None)
                .await?,
            0
        );
        let activated = trigger_state(&pool, id).await;
        assert!(activated.1.is_some());
        assert_eq!(activated.2, "pending");
        assert_eq!(
            execute_triggered_spot_limit_orders_with_hub(&pool, &pair, &decimal(execution), None)
                .await?,
            0
        );
        assert_eq!(trigger_state(&pool, id).await, activated);

        let replay = create(&app, &token, &body).await;
        assert_eq!(replay, (StatusCode::OK, first.clone()));
        let mut changed = body.clone();
        changed["trigger_direction"] = if direction == "rising" {
            "falling"
        } else {
            "rising"
        }
        .into();
        assert_eq!(create(&app, &token, &changed).await.0, StatusCode::CONFLICT);

        // A new pool has no in-memory activation state.
        let reconnected = MySqlPoolOptions::new()
            .max_connections(5)
            .connect(&std::env::var("DATABASE_URL")?)
            .await?;
        fund_system_spot_liquidity(
            &reconnected,
            if side == "buy" { base } else { quote },
            &decimal("100"),
        )
        .await?;
        let mut workers = Vec::new();
        for _ in 0..6 {
            let (pool, pair, price) = (reconnected.clone(), pair.clone(), decimal(execution));
            workers.push(tokio::spawn(async move {
                execute_triggered_spot_limit_orders_with_hub(&pool, &pair, &price, None)
                    .await
                    .unwrap()
            }));
        }
        let mut fills = 0;
        for worker in workers {
            fills += worker.await?;
        }
        assert_eq!(fills, 1);
        let final_state = trigger_state(&pool, id).await;
        assert_eq!(final_state.1, activated.1);
        assert_eq!(final_state.2, "filled");
        let (trades,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM spot_trades WHERE pair_id = ?")
                .bind(pair_id(&pool, &pair).await?)
                .fetch_one(&pool)
                .await?;
        assert_eq!(trades, 1);
        let (journal,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM platform_financial_journal WHERE context = 'spot' AND asset_id IN (?, ?)")
            .bind(base).bind(quote).fetch_one(&pool).await?;
        assert_eq!(journal, 4);
        assert_eq!(
            create(&app, &token, &body).await,
            (StatusCode::OK, first.clone())
        );
        cleanup_fixture(&pool, user, base, quote, &pair, id).await?;
        reconnected.close().await;
    }
    Ok(())
}

#[tokio::test]
async fn spot_explicit_trigger_validation_cached_activation_and_cancel()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let redis = redis_manager()
        .await
        .expect("REDIS_URL is required for the explicit trigger test");
    let user = create_user(&pool).await;
    let (base, base_symbol) = create_asset(&pool, "CB").await;
    let (quote, quote_symbol) = create_asset(&pool, "CQ").await;
    let pair = create_pair(&pool, base, quote, &base_symbol, &quote_symbol).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, 100)")
        .bind(user)
        .bind(quote)
        .execute(&pool)
        .await?;
    cache_market_ticker(&redis, &pair, "10").await?;
    let settings = test_settings();
    let token = issue_token(&settings, format!("user:{user}"), TokenScope::User, 900)?;
    let app = routes().with_state(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_redis(redis.clone()),
    );
    let mut body = serde_json::json!({
        "pair_id": pair, "side": "buy", "order_type": "stop_limit", "price": "8",
        "trigger_price": "10", "quantity": "2", "idempotency_key": format!("e03-{}", Uuid::now_v7())
    });
    assert_eq!(create(&app, &token, &body).await.0, StatusCode::BAD_REQUEST);
    body["trigger_direction"] = "sideways".into();
    assert_eq!(
        create(&app, &token, &body).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    body["trigger_direction"] = "rising".into();
    body["order_type"] = "limit".into();
    assert_eq!(create(&app, &token, &body).await.0, StatusCode::BAD_REQUEST);
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM spot_orders WHERE user_id = ?")
        .bind(user)
        .fetch_one(&pool)
        .await?;
    assert_eq!(count, 0);
    body["order_type"] = "stop_limit".into();
    let (status, first) = create(&app, &token, &body).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert!(first["triggered_at"].as_i64().is_some());
    let id = first["id"].as_str().unwrap();
    let activated = trigger_state(&pool, id).await.1;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/spot/orders/{id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    fund_system_spot_liquidity(&pool, base, &decimal("100")).await?;
    assert_eq!(
        execute_triggered_spot_limit_orders_with_hub(&pool, &pair, &decimal("8"), None).await?,
        0
    );
    assert_eq!(
        trigger_state(&pool, id).await,
        (Some("rising".into()), activated, "cancelled".into())
    );
    let (available, frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user)
    .bind(quote)
    .fetch_one(&pool)
    .await?;
    assert_eq!((available, frozen), (decimal("100"), decimal("0")));
    cleanup_fixture(&pool, user, base, quote, &pair, id).await?;
    let mut connection = redis;
    let _: i64 = connection.del(market_ticker_redis_key(&pair)).await?;
    Ok(())
}

#[tokio::test]
async fn spot_explicit_trigger_immediate_fill_preserves_activation_and_replay()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let Some(redis) = redis_manager().await else {
        return Ok(());
    };
    for (side, direction) in [
        ("buy", "rising"),
        ("buy", "falling"),
        ("sell", "rising"),
        ("sell", "falling"),
    ] {
        let user = create_user(&pool).await;
        let (base, base_symbol) = create_asset(&pool, "IB").await;
        let (quote, quote_symbol) = create_asset(&pool, "IQ").await;
        let pair = create_pair(&pool, base, quote, &base_symbol, &quote_symbol).await;
        sqlx::query(
            "INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, 100)",
        )
        .bind(user)
        .bind(if side == "buy" { quote } else { base })
        .execute(&pool)
        .await?;
        fund_system_spot_liquidity(
            &pool,
            if side == "buy" { base } else { quote },
            &decimal("100"),
        )
        .await?;
        cache_market_ticker(&redis, &pair, "10").await?;
        let settings = test_settings();
        let token = issue_token(&settings, format!("user:{user}"), TokenScope::User, 900)?;
        let app = routes().with_state(
            AppState::new(settings)
                .with_mysql(pool.clone())
                .with_redis(redis.clone()),
        );
        let body = serde_json::json!({
            "pair_id": pair, "side": side, "order_type": "stop_limit",
            "price": "10", "trigger_price": "10", "trigger_direction": direction,
            "quantity": "2", "idempotency_key": format!("e03-immediate-{}", Uuid::now_v7())
        });
        let (status, first) = create(&app, &token, &body).await;
        assert_eq!(status, StatusCode::OK, "{first}");
        assert_eq!(first["status"], "filled");
        assert_eq!(first["trigger_direction"], direction);
        assert!(first["triggered_at"].as_i64().is_some());
        let id = first["id"].as_str().unwrap();
        let activation = trigger_state(&pool, id).await;
        assert!(activation.1.is_some());
        assert_eq!(
            create(&app, &token, &body).await,
            (StatusCode::OK, first.clone())
        );
        assert_eq!(
            execute_triggered_spot_limit_orders_with_hub(&pool, &pair, &decimal("10"), None)
                .await?,
            0
        );
        assert_eq!(trigger_state(&pool, id).await, activation);
        let (trades,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM spot_trades WHERE pair_id = ?")
                .bind(pair_id(&pool, &pair).await?)
                .fetch_one(&pool)
                .await?;
        assert_eq!(trades, 1);
        let (journal,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM platform_financial_journal WHERE context = 'spot' AND asset_id IN (?, ?)",
        )
        .bind(base)
        .bind(quote)
        .fetch_one(&pool)
        .await?;
        assert_eq!(journal, 4);
        cleanup_fixture(&pool, user, base, quote, &pair, id).await?;
        let mut connection = redis.clone();
        let _: i64 = connection.del(market_ticker_redis_key(&pair)).await?;
    }
    Ok(())
}

#[tokio::test]
async fn spot_legacy_stop_limit_keeps_original_comparisons_and_exact_replay()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    for side in ["buy", "sell"] {
        let user = create_user(&pool).await;
        let (base, base_symbol) = create_asset(&pool, "LB").await;
        let (quote, quote_symbol) = create_asset(&pool, "LQ").await;
        let pair = create_pair(&pool, base, quote, &base_symbol, &quote_symbol).await;
        let reserve = if side == "buy" { quote } else { base };
        let amount = if side == "buy" { "20" } else { "2" };
        let id = seed_open_order(&pool, user, &pair, side, "10", "2").await?;
        let key = format!("e03-legacy-{}", Uuid::now_v7());
        let trigger = if side == "buy" { "8" } else { "12" };
        sqlx::query("UPDATE spot_orders SET order_type = 'stop_limit', trigger_price = ?, reserved_asset = ?, reserved_amount = ?, idempotency_key = ? WHERE id = ?")
            .bind(decimal(trigger)).bind(reserve).bind(decimal(amount)).bind(&key).bind(&id).execute(&pool).await?;
        sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, frozen) VALUES (?, ?, ?)")
            .bind(user)
            .bind(reserve)
            .bind(decimal(amount))
            .execute(&pool)
            .await?;
        fund_system_spot_liquidity(
            &pool,
            if side == "buy" { base } else { quote },
            &decimal("100"),
        )
        .await?;
        assert_eq!(
            execute_triggered_spot_limit_orders_with_hub(&pool, &pair, &decimal("10"), None)
                .await?,
            0
        );
        assert_eq!(trigger_state(&pool, &id).await.1, None);
        let settings = test_settings();
        let token = issue_token(&settings, format!("user:{user}"), TokenScope::User, 900)?;
        let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
        let body = serde_json::json!({"pair_id": pair, "side": side, "order_type": "stop_limit",
            "price": "10", "trigger_price": trigger, "quantity": "2", "idempotency_key": key});
        let (status, replay) = create(&app, &token, &body).await;
        assert_eq!(status, StatusCode::OK, "{replay}");
        assert!(replay["trigger_direction"].is_null());
        assert_eq!(
            execute_triggered_spot_limit_orders_with_hub(&pool, &pair, &decimal(trigger), None)
                .await?,
            1
        );
        assert_eq!(
            trigger_state(&pool, &id).await,
            (None, None, "filled".into())
        );
        cleanup_fixture(&pool, user, base, quote, &pair, &id).await?;
    }
    Ok(())
}
