use super::*;

fn request(method: &str, uri: &str, token: &str, payload: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap()
}

#[tokio::test]
async fn numeric_safety_admin_pair_uses_locked_asset_precision_before_writes()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let (base, base_symbol) = create_asset_with_symbol(&pool, "NPB").await;
    let (quote, quote_symbol) = create_asset_with_symbol(&pool, "NPQ").await;
    sqlx::query("UPDATE assets SET precision_scale=2 WHERE id IN (?, ?)")
        .bind(base)
        .bind(quote)
        .execute(&pool)
        .await?;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let payload = json!({
        "base_asset_id":base, "quote_asset_id":quote, "symbol":format!("{base_symbol}-{quote_symbol}"),
        "price_precision":8, "qty_precision":2, "min_order_value":"1.23",
        "status":"active", "market_type":"external", "reason":"numeric safety"
    });
    for (field, value) in [
        ("qty_precision", json!(3)),
        ("min_order_value", json!("1.001")),
    ] {
        let mut invalid = payload.clone();
        invalid[field] = value;
        let response = app
            .clone()
            .oneshot(request(
                "POST",
                "/admin/api/v1/market-pairs",
                &token,
                invalid,
            ))
            .await?;
        let status = response.status();
        let body = body_json(response).await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM trading_pairs WHERE base_asset=?")
        .bind(base)
        .fetch_one(&pool)
        .await?;
    assert_eq!(count, 0);
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/admin/api/v1/market-pairs",
            &token,
            payload.clone(),
        ))
        .await?;
    let status = response.status();
    let body = body_json(response).await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pair_id = body["id"].as_u64().unwrap();
    let mut update = payload;
    for field in ["base_asset_id", "quote_asset_id", "symbol"] {
        update.as_object_mut().unwrap().remove(field);
    }
    let mut invalid = update.clone();
    invalid["qty_precision"] = json!(3);
    let response = app
        .clone()
        .oneshot(request(
            "PATCH",
            &format!("/admin/api/v1/market-pairs/{pair_id}"),
            &token,
            invalid,
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    sqlx::query("UPDATE assets SET precision_scale=19 WHERE id=?")
        .bind(quote)
        .execute(&pool)
        .await?;
    let response = app
        .oneshot(request(
            "PATCH",
            &format!("/admin/api/v1/market-pairs/{pair_id}"),
            &token,
            update,
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let precision: i32 = sqlx::query_scalar("SELECT qty_precision FROM trading_pairs WHERE id=?")
        .bind(pair_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(precision, 2);
    let audits: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admin_audit_logs WHERE admin_id=?")
        .bind(admin_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(audits, 1);
    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id=?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM trading_pairs WHERE id=?")
        .bind(pair_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id IN (?,?)")
        .bind(base)
        .bind(quote)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_users WHERE id=?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id=?")
        .bind(role_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn numeric_safety_admin_recharge_overflow_rolls_back_receipt_balance_and_audit()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let user_id = create_user(&pool).await;
    let asset_id = create_asset(&pool, "NRA").await;
    let maximum = decimal("99999999999999999999.999999999999999999");
    sqlx::query("INSERT INTO wallet_accounts (user_id,asset_id,available) VALUES (?,?,?)")
        .bind(user_id)
        .bind(asset_id)
        .bind(&maximum)
        .execute(&pool)
        .await?;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let response = app.oneshot(request("POST", &format!("/admin/api/v1/users/{user_id}/recharge"), &token, json!({
        "asset_id":asset_id,"amount":"0.000000000000000001",
        "reason":"numeric overflow regression","idempotency_key":format!("numeric-{}",Uuid::now_v7())
    }))).await?;
    let status = response.status();
    let body = body_json(response).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let available: BigDecimal =
        sqlx::query_scalar("SELECT available FROM wallet_accounts WHERE user_id=? AND asset_id=?")
            .bind(user_id)
            .bind(asset_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(available, maximum);
    for table in ["admin_wallet_recharges", "wallet_ledger"] {
        let count: i64 =
            sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table} WHERE user_id=?"))
                .bind(user_id)
                .fetch_one(&pool)
                .await?;
        assert_eq!(count, 0, "{table}");
    }
    let audits: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admin_audit_logs WHERE admin_id=?")
        .bind(admin_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(audits, 0);
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id=?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id=?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id=?")
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_users WHERE id=?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id=?")
        .bind(role_id)
        .execute(&pool)
        .await?;
    Ok(())
}
