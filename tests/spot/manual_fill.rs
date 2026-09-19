use super::*;

async fn post_fill(
    app: &axum::Router,
    token: Option<&str>,
    body: &Value,
) -> axum::response::Response {
    let mut request = Request::builder()
        .method("POST")
        .uri("/spot/fills")
        .header("content-type", "application/json");
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    app.clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}

async fn snapshot(pool: &MySqlPool, buyer: u64, seller: u64) -> Value {
    let wallets: Vec<(u64, u64, BigDecimal, BigDecimal)> = sqlx::query_as(
        "SELECT user_id, asset_id, available, frozen FROM wallet_accounts WHERE user_id IN (?, ?) ORDER BY user_id, asset_id",
    ).bind(buyer).bind(seller).fetch_all(pool).await.unwrap();
    let orders: Vec<(u64, String, BigDecimal)> = sqlx::query_as(
        "SELECT id, status, filled_quantity FROM spot_orders WHERE user_id IN (?, ?) ORDER BY id",
    )
    .bind(buyer)
    .bind(seller)
    .fetch_all(pool)
    .await
    .unwrap();
    let ledger: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM wallet_ledger WHERE user_id IN (?, ?)")
            .bind(buyer)
            .bind(seller)
            .fetch_one(pool)
            .await
            .unwrap();
    let trades: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM spot_trades t JOIN spot_orders o ON o.id = t.buy_order_id WHERE o.user_id = ?",
    ).bind(buyer).fetch_one(pool).await.unwrap();
    let commissions: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM agent_commission_records WHERE user_id IN (?, ?)")
            .bind(buyer)
            .bind(seller)
            .fetch_one(pool)
            .await
            .unwrap();
    let journal: Vec<(u64, String, BigDecimal)> = sqlx::query_as(
        "SELECT asset_id, account_code, amount FROM platform_financial_journal WHERE context = 'spot' AND asset_id IN (SELECT asset_id FROM wallet_accounts WHERE user_id IN (?, ?)) ORDER BY asset_id, account_code, id",
    ).bind(buyer).bind(seller).fetch_all(pool).await.unwrap();
    serde_json::json!({ "wallets": wallets, "orders": orders, "ledger": ledger.0, "trades": trades.0, "commissions": commissions.0, "journal": journal })
}

#[tokio::test]
async fn manual_fill_actor_reason_replay_and_audit_failure_are_atomic() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let (other_role, other_admin) = create_admin_user(&pool).await;
    let buyer = create_user(&pool).await;
    let seller = create_user(&pool).await;
    let (base, base_symbol) = create_asset(&pool, "AB").await;
    let (quote, quote_symbol) = create_asset(&pool, "AQ").await;
    let pair = create_pair(&pool, base, quote, &base_symbol, &quote_symbol).await;
    let buy = seed_open_order(&pool, buyer, &pair, "buy", "10", "2").await?;
    let sell = seed_open_order(&pool, seller, &pair, "sell", "10", "2").await?;
    for (user, asset, frozen) in [
        (buyer, base, "0"),
        (buyer, quote, "20"),
        (seller, base, "2"),
        (seller, quote, "0"),
    ] {
        sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, 0, ?)")
            .bind(user).bind(asset).bind(decimal(frozen)).execute(&pool).await?;
    }
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )?;
    let other_token = issue_token(
        &settings,
        format!("admin:{other_admin}"),
        TokenScope::Admin,
        900,
    )?;
    let user_token = issue_token(&settings, format!("user:{buyer}"), TokenScope::User, 900)?;
    let app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let body = serde_json::json!({
        "buy_order_id": buy, "sell_order_id": sell, "price": "10", "quantity": "1",
        "reason": "matched signed orders", "idempotency_key": format!("e07-{}", Uuid::now_v7())
    });
    let before = snapshot(&pool, buyer, seller).await;
    assert_eq!(
        post_fill(&app, None, &body).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post_fill(&app, Some(&user_token), &body).await.status(),
        StatusCode::FORBIDDEN
    );
    let mut missing = body.clone();
    missing.as_object_mut().unwrap().remove("reason");
    assert_eq!(
        post_fill(&app, Some(&token), &missing).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    for reason in [" ", &"r".repeat(513)] {
        let mut invalid = body.clone();
        invalid["reason"] = reason.into();
        assert_eq!(
            post_fill(&app, Some(&token), &invalid).await.status(),
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(snapshot(&pool, buyer, seller).await, before);

    // A narrowly scoped trigger proves that failure of the final audit INSERT rolls back every financial write.
    let trigger = format!("e07_audit_failure_{admin_id}");
    sqlx::raw_sql(&format!(
        "CREATE TRIGGER {trigger} BEFORE INSERT ON admin_audit_logs FOR EACH ROW BEGIN \
         IF NEW.admin_id = {admin_id} AND NEW.action = 'spot.fill' THEN \
         SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'e07 audit failure injection'; END IF; END"
    ))
    .execute(&pool)
    .await?;
    let failed = post_fill(&app, Some(&token), &body).await;
    sqlx::raw_sql(&format!("DROP TRIGGER {trigger}"))
        .execute(&pool)
        .await?;
    assert_eq!(failed.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(snapshot(&pool, buyer, seller).await, before);
    let (audit_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM admin_audit_logs WHERE admin_id = ?")
            .bind(admin_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(audit_count, 0);

    let mut calls = Vec::new();
    for _ in 0..10 {
        let (app, token, body) = (app.clone(), token.clone(), body.clone());
        calls.push(tokio::spawn(async move {
            post_fill(&app, Some(&token), &body).await
        }));
    }
    let mut trade_id = None;
    for call in calls {
        let response = call.await?;
        let status = response.status();
        let payload = body_json(response).await?;
        assert_eq!(status, StatusCode::OK, "{payload}");
        if let Some(id) = &trade_id {
            assert_eq!(&payload["trade"]["id"], id);
        }
        trade_id = Some(payload["trade"]["id"].clone());
    }
    let after = snapshot(&pool, buyer, seller).await;
    assert_eq!(after["trades"], 1);
    assert_eq!(after["ledger"], 4);
    assert_eq!(after["journal"].as_array().unwrap().len(), 4);
    for (asset, amount) in [(base, decimal("1")), (quote, decimal("10"))] {
        let legs: Vec<(String, BigDecimal)> = sqlx::query_as(
            "SELECT account_code, amount FROM platform_financial_journal WHERE transaction_key = ? AND asset_id = ? ORDER BY account_code",
        ).bind(format!("spot:{}:fill", trade_id.as_ref().unwrap().as_str().unwrap()))
            .bind(asset).fetch_all(&pool).await?;
        assert_eq!(
            legs,
            vec![
                ("user_spot_source_liability".into(), amount.clone()),
                ("user_spot_target_liability".into(), -amount)
            ]
        );
    }
    assert_eq!(
        after["orders"][0][2].as_str().map(decimal),
        Some(decimal("1"))
    );
    let audit: Vec<(u64, String, Value, Value)> = sqlx::query_as(
        "SELECT admin_id, reason, before_json, after_json FROM admin_audit_logs WHERE action = 'spot.fill' AND admin_id = ?",
    ).bind(admin_id).fetch_all(&pool).await?;
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].0, admin_id);
    assert_eq!(audit[0].1, body["reason"]);
    assert_eq!(
        audit[0].2["buy_order"]["filled_quantity"]
            .as_str()
            .map(decimal),
        Some(decimal("0"))
    );
    assert_eq!(audit[0].3["trade"]["id"], trade_id.unwrap());
    assert_eq!(audit[0].3["idempotency_key"], body["idempotency_key"]);

    let mut different_reason = body.clone();
    different_reason["reason"] = "different reason".into();
    assert_eq!(
        post_fill(&app, Some(&token), &different_reason)
            .await
            .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        post_fill(&app, Some(&other_token), &body).await.status(),
        StatusCode::CONFLICT
    );
    let mut different_price = body.clone();
    different_price["price"] = "9".into();
    assert_eq!(
        post_fill(&app, Some(&token), &different_price)
            .await
            .status(),
        StatusCode::CONFLICT
    );
    let mut different_quantity = body.clone();
    different_quantity["quantity"] = "2".into();
    assert_eq!(
        post_fill(&app, Some(&token), &different_quantity)
            .await
            .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(snapshot(&pool, buyer, seller).await, after);

    // An old/automatic trade with no manual audit cannot be adopted by a manual replay.
    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    assert_eq!(
        post_fill(&app, Some(&token), &body).await.status(),
        StatusCode::CONFLICT
    );
    assert_eq!(snapshot(&pool, buyer, seller).await, after);
    let (audit_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM admin_audit_logs WHERE admin_id = ?")
            .bind(admin_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(audit_count, 0);

    cleanup_fill_fixture(&pool, buyer, seller, base, quote, &pair, &buy, &sell).await?;
    cleanup_manual_fill_admin(&pool, role_id, admin_id).await?;
    cleanup_manual_fill_admin(&pool, other_role, other_admin).await?;
    Ok(())
}
