use super::*;

async fn call(
    app: &axum::Router,
    token: &str,
    method: &str,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
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
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, body)
}

async fn wallet(pool: &MySqlPool, user: u64, asset: u64) -> (BigDecimal, BigDecimal) {
    sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user)
    .bind(asset)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn counts(pool: &MySqlPool, base: u64, quote: u64, pair: &str) -> (i64, i64, i64, i64) {
    sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM spot_orders WHERE pair_id = p.id), \
         (SELECT COUNT(*) FROM spot_trades WHERE pair_id = p.id), \
         (SELECT COUNT(*) FROM wallet_ledger WHERE asset_id IN (?, ?)), \
         (SELECT COUNT(*) FROM platform_financial_journal WHERE asset_id IN (?, ?)) \
         FROM trading_pairs p WHERE p.symbol = ?",
    )
    .bind(base)
    .bind(quote)
    .bind(base)
    .bind(quote)
    .bind(pair)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn spot_numeric_partial_full_cancel_and_concurrent_replay_conserve_real_balances()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    for complete in [false, true] {
        let settings = test_settings();
        let buyer = create_user(&pool).await;
        let seller = create_user(&pool).await;
        let (base, bs) = create_asset(&pool, "NB").await;
        let (quote, qs) = create_asset(&pool, "NQ").await;
        let pair = create_pair(&pool, base, quote, &bs, &qs).await;
        sqlx::query("UPDATE assets SET precision_scale = 8 WHERE id IN (?, ?)")
            .bind(base)
            .bind(quote)
            .execute(&pool)
            .await?;
        sqlx::query("UPDATE trading_pairs SET price_precision = 8, qty_precision = 8, min_order_value = 0.00000001 WHERE symbol = ?")
            .bind(&pair).execute(&pool).await?;
        for (user, asset, available) in [
            (buyer, quote, "10"),
            (buyer, base, "0"),
            (seller, base, "1"),
            (seller, quote, "0"),
        ] {
            sqlx::query(
                "INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)",
            )
            .bind(user)
            .bind(asset)
            .bind(decimal(available))
            .execute(&pool)
            .await?;
        }
        let buyer_token = issue_token(&settings, format!("user:{buyer}"), TokenScope::User, 900)?;
        let seller_token = issue_token(&settings, format!("user:{seller}"), TokenScope::User, 900)?;
        let (role, admin) = create_admin_user(&pool).await;
        let admin_token = issue_token(&settings, format!("admin:{admin}"), TokenScope::Admin, 900)?;
        let app = routes().with_state(AppState::new(settings.clone()).with_mysql(pool.clone()));
        let admin_app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
        let buy_body = serde_json::json!({
            "pair_id": pair, "side": "buy", "order_type": "limit", "price": "1.00000001",
            "quantity": "1.0000000000", "idempotency_key": format!("n04-buy-{}", Uuid::now_v7())
        });
        let (status, buy) =
            call(&app, &buyer_token, "POST", "/spot/orders", buy_body.clone()).await;
        assert_eq!(status, StatusCode::OK, "{buy}");
        let buy_id = buy["id"].as_str().unwrap().to_owned();
        let (status, sell) = call(
            &app,
            &seller_token,
            "POST",
            "/spot/orders",
            serde_json::json!({
                "pair_id": pair, "side": "sell", "order_type": "limit", "price": "1.00000001",
                "quantity": "1", "idempotency_key": format!("n04-sell-{}", Uuid::now_v7())
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{sell}");
        let sell_id = sell["id"].as_str().unwrap().to_owned();
        assert_eq!(
            wallet(&pool, buyer, quote).await,
            (decimal("8.99999999"), decimal("1.00000001"))
        );

        let initial = counts(&pool, base, quote, &pair).await;
        for (price, quantity) in [
            ("1.00000001", "0.000000001"),
            ("1.000000001", "0.1"),
            ("1.00000001", "0.0000000000000000001"),
            ("100000000000000000000", "1"),
            ("1e4294967296", "1"),
        ] {
            let mut invalid = buy_body.clone();
            invalid["price"] = price.into();
            invalid["quantity"] = quantity.into();
            invalid["idempotency_key"] = Uuid::now_v7().to_string().into();
            let (status, _) = call(&app, &buyer_token, "POST", "/spot/orders", invalid).await;
            assert!(status.is_client_error());
            assert_eq!(counts(&pool, base, quote, &pair).await, initial);
        }

        let fill = serde_json::json!({
            "buy_order_id": buy_id, "sell_order_id": sell_id, "price": "1.00000001",
            "quantity": "0.33333333", "reason": "numeric regression",
            "idempotency_key": format!("n04-fill-{}", Uuid::now_v7())
        });
        let mut calls = Vec::new();
        for _ in 0..8 {
            let (app, token, body) = (admin_app.clone(), admin_token.clone(), fill.clone());
            calls.push(tokio::spawn(async move {
                call(&app, &token, "POST", "/spot/fills", body).await
            }));
        }
        let mut trade_id = None;
        for task in calls {
            let (status, response) = task.await?;
            assert_eq!(status, StatusCode::OK, "{response}");
            if let Some(id) = &trade_id {
                assert_eq!(&response["trade"]["id"], id);
            }
            trade_id = Some(response["trade"]["id"].clone());
        }
        assert_eq!(
            wallet(&pool, buyer, quote).await,
            (decimal("8.99999999"), decimal("0.66666668"))
        );
        assert_eq!(
            wallet(&pool, seller, quote).await,
            (decimal("0.33333333"), decimal("0"))
        );
        assert_eq!(
            wallet(&pool, buyer, base).await,
            (decimal("0.33333333"), decimal("0"))
        );
        let after_first = counts(&pool, base, quote, &pair).await;
        assert_eq!(after_first, (2, 1, 8, 4));
        let (status, replay) = call(&app, &buyer_token, "POST", "/spot/orders", buy_body).await;
        assert_eq!(status, StatusCode::OK, "{replay}");
        assert_eq!(replay, buy);
        assert_eq!(counts(&pool, base, quote, &pair).await, after_first);
        let mut invalid_fill = fill.clone();
        invalid_fill["quantity"] = "0.000000001".into();
        invalid_fill["idempotency_key"] = Uuid::now_v7().to_string().into();
        let (status, _) = call(
            &admin_app,
            &admin_token,
            "POST",
            "/spot/fills",
            invalid_fill,
        )
        .await;
        assert!(status.is_client_error());
        assert_eq!(counts(&pool, base, quote, &pair).await, after_first);

        // Failure on the last credit must undo already-executed debit/base legs and the trade.
        sqlx::query("UPDATE wallet_accounts SET available = 99999999999999999999.99999999 WHERE user_id = ? AND asset_id = ?")
            .bind(seller).bind(quote).execute(&pool).await?;
        let mut overflow_fill = fill.clone();
        overflow_fill["idempotency_key"] = Uuid::now_v7().to_string().into();
        let (status, _) = call(
            &admin_app,
            &admin_token,
            "POST",
            "/spot/fills",
            overflow_fill,
        )
        .await;
        assert!(status.is_client_error());
        assert_eq!(counts(&pool, base, quote, &pair).await, after_first);
        assert_eq!(
            wallet(&pool, buyer, quote).await,
            (decimal("8.99999999"), decimal("0.66666668"))
        );
        assert_eq!(
            wallet(&pool, seller, base).await,
            (decimal("0"), decimal("0.66666667"))
        );
        sqlx::query(
            "UPDATE wallet_accounts SET available = 0.33333333 WHERE user_id = ? AND asset_id = ?",
        )
        .bind(seller)
        .bind(quote)
        .execute(&pool)
        .await?;

        if complete {
            for quantity in ["0.33333333", "0.33333334"] {
                let mut next = fill.clone();
                next["quantity"] = quantity.into();
                next["idempotency_key"] = Uuid::now_v7().to_string().into();
                let (status, response) =
                    call(&admin_app, &admin_token, "POST", "/spot/fills", next).await;
                assert_eq!(status, StatusCode::OK, "{response}");
            }
            assert_eq!(
                wallet(&pool, buyer, quote).await,
                (decimal("9"), decimal("0"))
            );
            assert_eq!(
                wallet(&pool, seller, quote).await,
                (decimal("1"), decimal("0"))
            );
            assert_eq!(
                wallet(&pool, buyer, base).await,
                (decimal("1"), decimal("0"))
            );
        } else {
            // Historical NULL and zero snapshots must still use actual freeze/debit evidence.
            sqlx::query(
                "UPDATE spot_orders SET reserved_asset = NULL, reserved_amount = NULL WHERE id = ?",
            )
            .bind(&buy_id)
            .execute(&pool)
            .await?;
            sqlx::query("UPDATE spot_orders SET reserved_amount = 0 WHERE id = ?")
                .bind(&sell_id)
                .execute(&pool)
                .await?;
            for (id, token) in [(&buy_id, &buyer_token), (&sell_id, &seller_token)] {
                let path = format!("/spot/orders/{id}");
                let mut cancellations = Vec::new();
                for _ in 0..8 {
                    let (app, token, path) = (app.clone(), token.clone(), path.clone());
                    cancellations.push(tokio::spawn(async move {
                        call(&app, &token, "DELETE", &path, Value::Null).await
                    }));
                }
                let mut changed = 0;
                for cancel in cancellations {
                    let (status, response) = cancel.await?;
                    assert_eq!(status, StatusCode::OK, "{response}");
                    changed += usize::from(response["cancelled"] == true);
                }
                assert_eq!(changed, 1);
                let count = counts(&pool, base, quote, &pair).await;
                let (status, _) = call(&app, token, "DELETE", &path, Value::Null).await;
                assert_eq!(status, StatusCode::OK);
                assert_eq!(counts(&pool, base, quote, &pair).await, count);
            }
            assert_eq!(
                wallet(&pool, buyer, quote).await,
                (decimal("9.66666667"), decimal("0"))
            );
            assert_eq!(
                wallet(&pool, seller, base).await,
                (decimal("0.66666667"), decimal("0"))
            );
        }
        let sums: Vec<(u64, BigDecimal)> = sqlx::query_as(
            "SELECT asset_id, SUM(amount) FROM platform_financial_journal WHERE asset_id IN (?, ?) GROUP BY asset_id",
        ).bind(base).bind(quote).fetch_all(&pool).await?;
        assert_eq!(sums.len(), 2);
        assert!(sums.iter().all(|(_, sum)| sum == &decimal("0")));
        let (quote_debits, quote_credits): (BigDecimal, BigDecimal) = sqlx::query_as(
            "SELECT -SUM(CASE WHEN balance_type = 'frozen' THEN amount ELSE 0 END), \
             SUM(CASE WHEN balance_type = 'available' THEN amount ELSE 0 END) FROM wallet_ledger \
             WHERE asset_id = ? AND change_type = 'spot_trade_settlement'",
        )
        .bind(quote)
        .fetch_one(&pool)
        .await?;
        assert_eq!(quote_debits, quote_credits);
        cleanup_fill_fixture(&pool, buyer, seller, base, quote, &pair, &buy_id, &sell_id).await?;
        cleanup_manual_fill_admin(&pool, role, admin).await?;
    }
    Ok(())
}

#[tokio::test]
async fn spot_numeric_auto_both_sides_20dp_product_and_zero_quote_rejection()
-> Result<(), Box<dyn Error>> {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .try_init();
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    for (side, price, quantity, precision, expected_quote) in [
        ("buy", "1.0000000001", "1.0000000001", 8, "1"),
        ("sell", "1.0000000001", "1.0000000001", 8, "1"),
        (
            "buy",
            "0.000000001",
            "0.000000001",
            18,
            "0.000000000000000001",
        ),
        (
            "sell",
            "0.000000001",
            "0.000000001",
            18,
            "0.000000000000000001",
        ),
    ] {
        let settings = test_settings();
        let user = create_user(&pool).await;
        let (base, bs) = create_asset(&pool, "XB").await;
        let (quote, qs) = create_asset(&pool, "XQ").await;
        let pair = create_pair(&pool, base, quote, &bs, &qs).await;
        sqlx::query("UPDATE assets SET precision_scale = 10 WHERE id = ?")
            .bind(base)
            .execute(&pool)
            .await?;
        sqlx::query("UPDATE assets SET precision_scale = ? WHERE id = ?")
            .bind(precision)
            .bind(quote)
            .execute(&pool)
            .await?;
        sqlx::query("UPDATE trading_pairs SET price_precision = 10, qty_precision = 10, min_order_value = ? WHERE symbol = ?")
            .bind(decimal(if precision == 8 { "0.00000001" } else { "0.000000000000000001" }))
            .bind(&pair).execute(&pool).await?;
        for asset in [base, quote] {
            sqlx::query(
                "INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, 10)",
            )
            .bind(user)
            .bind(asset)
            .execute(&pool)
            .await?;
            fund_system_spot_liquidity(&pool, asset, &decimal("100")).await?;
        }
        let token = issue_token(&settings, format!("user:{user}"), TokenScope::User, 900)?;
        let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
        let intent = serde_json::json!({
            "pair_id": pair, "side": side, "order_type": "limit", "price": price,
            "quantity": quantity, "idempotency_key": Uuid::now_v7().to_string()
        });
        let (status, order) = call(&app, &token, "POST", "/spot/orders", intent.clone()).await;
        assert_eq!(status, StatusCode::OK, "{order}");
        let id = order["id"].as_str().unwrap();
        let (reserve,): (BigDecimal,) =
            sqlx::query_as("SELECT reserved_amount FROM spot_orders WHERE id = ?")
                .bind(id)
                .fetch_one(&pool)
                .await?;
        assert_eq!(
            reserve,
            decimal(if side == "buy" {
                expected_quote
            } else {
                quantity
            })
        );
        assert_eq!(
            execute_triggered_spot_limit_orders_with_hub(&pool, &pair, &decimal(price), None)
                .await?,
            1
        );
        assert_eq!(
            wallet(&pool, user, quote).await,
            (
                if side == "buy" {
                    decimal("10") - decimal(expected_quote)
                } else {
                    decimal("10") + decimal(expected_quote)
                },
                decimal("0")
            )
        );
        assert_eq!(
            wallet(&pool, user, base).await,
            (
                if side == "buy" {
                    decimal("10") + decimal(quantity)
                } else {
                    decimal("10") - decimal(quantity)
                },
                decimal("0")
            )
        );
        let before = counts(&pool, base, quote, &pair).await;
        assert_eq!(
            execute_triggered_spot_limit_orders_with_hub(&pool, &pair, &decimal(price), None)
                .await?,
            0
        );
        assert_eq!(counts(&pool, base, quote, &pair).await, before);
        sqlx::query("UPDATE spot_orders SET request_fingerprint = NULL, idempotency_response_json = NULL WHERE id = ?")
            .bind(id).execute(&pool).await?;
        let (status, replay) = call(&app, &token, "POST", "/spot/orders", intent).await;
        assert_eq!(status, StatusCode::OK, "{replay}");
        assert_eq!(replay["id"], order["id"]);
        assert_eq!(counts(&pool, base, quote, &pair).await, before);
        // Even with a legacy tiny minimum, a generated zero quote cannot create/freezes an order.
        sqlx::query(
            "UPDATE trading_pairs SET min_order_value = 0.000000000000000001 WHERE symbol = ?",
        )
        .bind(&pair)
        .execute(&pool)
        .await?;
        let (status, _) = call(
            &app,
            &token,
            "POST",
            "/spot/orders",
            serde_json::json!({
                "pair_id": pair, "side": side, "order_type": "limit", "price": "0.000000001",
                "quantity": if precision == 8 { "0.000000001" } else { "0.0000000001" },
                "idempotency_key": Uuid::now_v7().to_string()
            }),
        )
        .await;
        assert!(status.is_client_error());
        assert_eq!(counts(&pool, base, quote, &pair).await, before);
        cleanup_fixture(&pool, user, base, quote, &pair, id).await?;
    }
    Ok(())
}

#[tokio::test]
async fn spot_numeric_historical_refund_preserves_stored_dust_and_rejects_missing_evidence()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user = create_user(&pool).await;
    let (base, bs) = create_asset(&pool, "LB").await;
    let (quote, qs) = create_asset(&pool, "LQ").await;
    let pair = create_pair(&pool, base, quote, &bs, &qs).await;
    let order = seed_open_order(&pool, user, &pair, "buy", "1", "1").await?;
    sqlx::query("UPDATE assets SET precision_scale = 8 WHERE id = ?")
        .bind(quote)
        .execute(&pool)
        .await?;
    sqlx::query(
        "UPDATE spot_orders SET reserved_amount = NULL, reserved_asset = NULL WHERE id = ?",
    )
    .bind(&order)
    .execute(&pool)
    .await?;
    // Another order's 5 units are not this order's evidence.
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, 0, 6.000000000000000001)")
        .bind(user).bind(quote).execute(&pool).await?;
    let token = issue_token(&settings, format!("user:{user}"), TokenScope::User, 900)?;
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let path = format!("/spot/orders/{order}");
    let before = counts(&pool, base, quote, &pair).await;
    let (status, _) = call(&app, &token, "DELETE", &path, Value::Null).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(counts(&pool, base, quote, &pair).await, before);
    sqlx::query(
        "INSERT INTO wallet_ledger (user_id, asset_id, amount, change_type, balance_type, balance_after, \
         available_after, frozen_after, locked_after, ref_type, ref_id) \
         VALUES (?, ?, 1.000000000000000001, 'spot_freeze', 'frozen', 6.000000000000000001, \
         0, 6.000000000000000001, 0, 'spot_order', ?)",
    ).bind(user).bind(quote).bind(&order).execute(&pool).await?;
    // A partial quantity without any debit evidence must not release the original total.
    sqlx::query("UPDATE spot_orders SET filled_quantity = 0.25 WHERE id = ?")
        .bind(&order)
        .execute(&pool)
        .await?;
    let (status, _) = call(&app, &token, "DELETE", &path, Value::Null).await;
    assert_eq!(status, StatusCode::CONFLICT);
    sqlx::query("UPDATE spot_orders SET filled_quantity = 0 WHERE id = ?")
        .bind(&order)
        .execute(&pool)
        .await?;
    let (status, response) = call(&app, &token, "DELETE", &path, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(
        wallet(&pool, user, quote).await,
        (decimal("1.000000000000000001"), decimal("5"))
    );
    let before = counts(&pool, base, quote, &pair).await;
    let (status, _) = call(&app, &token, "DELETE", &path, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(counts(&pool, base, quote, &pair).await, before);
    cleanup_fixture(&pool, user, base, quote, &pair, &order).await?;
    Ok(())
}
