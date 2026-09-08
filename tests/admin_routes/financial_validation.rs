use super::*;

fn financial_request(method: &str, uri: &str, token: &str, payload: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap()
}

async fn recharge_effect_counts(pool: &MySqlPool, user_id: u64) -> (i64, i64, i64, i64) {
    sqlx::query_as(
        r#"SELECT
           (SELECT COUNT(*) FROM wallet_accounts WHERE user_id = ?),
           (SELECT COUNT(*) FROM admin_wallet_recharges WHERE user_id = ?),
           (SELECT COUNT(*) FROM wallet_ledger WHERE user_id = ?),
           (SELECT COUNT(*) FROM admin_audit_logs WHERE target_type = 'wallet_account'
              AND target_id = ? AND action = 'wallet.recharge')"#,
    )
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .bind(user_id.to_string())
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn admin_recharge_rejects_excess_precision_without_any_financial_side_effect()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let user_id = create_user(&pool).await;
    let asset_id = create_asset(&pool, "ARP").await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let uri = format!("/admin/api/v1/users/{user_id}/recharge");

    for (precision, amount) in [(2, "0.001"), (18, "0.0000000000000000001"), (0, "1.1")] {
        sqlx::query("UPDATE assets SET precision_scale = ? WHERE id = ?")
            .bind(precision)
            .bind(asset_id)
            .execute(&pool)
            .await?;
        let response = app
            .clone()
            .oneshot(financial_request(
                "POST",
                &uri,
                &token,
                json!({
                    "asset_id": asset_id, "amount": amount, "reason": "precision rejection",
                    "idempotency_key": format!("precision-{precision}")
                }),
            ))
            .await?;
        let status = response.status();
        let payload = body_json(response).await?;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "amount {amount}: {payload}"
        );
        assert!(payload.to_string().contains("precision"), "{payload}");
        assert_eq!(recharge_effect_counts(&pool, user_id).await, (0, 0, 0, 0));
    }

    for invalid_precision in [-1, 19] {
        sqlx::query("UPDATE assets SET precision_scale = ? WHERE id = ?")
            .bind(invalid_precision)
            .bind(asset_id)
            .execute(&pool)
            .await?;
        let response = app
            .clone()
            .oneshot(financial_request(
                "POST",
                &uri,
                &token,
                json!({
                    "asset_id": asset_id, "amount": "1", "reason": "invalid stored precision",
                    "idempotency_key": format!("invalid-precision-{invalid_precision}")
                }),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(recharge_effect_counts(&pool, user_id).await, (0, 0, 0, 0));
    }

    sqlx::query("UPDATE assets SET precision_scale = 2 WHERE id = ?")
        .bind(asset_id)
        .execute(&pool)
        .await?;
    let payload = json!({ "asset_id": asset_id, "amount": "1.2300000000000000000000",
        "reason": "precision accepted", "idempotency_key": "accepted-precision" });
    let response = app
        .clone()
        .oneshot(financial_request("POST", &uri, &token, payload.clone()))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let first = body_json(response).await?;
    assert_eq!(decimal(first["amount"].as_str().unwrap()), decimal("1.23"));
    assert_eq!(
        decimal(first["available"].as_str().unwrap()),
        decimal("1.23")
    );
    assert_eq!(recharge_effect_counts(&pool, user_id).await, (1, 1, 1, 1));

    // 历史成功收据优先于当前资产有效性；精度收紧及停用都不改写首次响应。
    sqlx::query("UPDATE assets SET precision_scale = 0, status = 'disabled' WHERE id = ?")
        .bind(asset_id)
        .execute(&pool)
        .await?;
    let replay = app
        .clone()
        .oneshot(financial_request("POST", &uri, &token, payload))
        .await?;
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(body_json(replay).await?, first);
    assert_eq!(recharge_effect_counts(&pool, user_id).await, (1, 1, 1, 1));

    sqlx::query("UPDATE assets SET precision_scale = 18, status = 'active' WHERE id = ?")
        .bind(asset_id)
        .execute(&pool)
        .await?;
    let response = app
        .oneshot(financial_request(
            "POST",
            &uri,
            &token,
            json!({
                "asset_id": asset_id, "amount": "0.000000000000000001", "reason": "exact 18 digits",
                "idempotency_key": "accepted-18"
            }),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let exact = body_json(response).await?;
    assert_eq!(exact["amount"], "0.000000000000000001");
    assert_eq!(exact["available"], "1.230000000000000001");
    let amounts: Vec<(BigDecimal, BigDecimal)> = sqlx::query_as(
        "SELECT amount, available_after FROM wallet_ledger WHERE user_id = ? ORDER BY id",
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await?;
    assert_eq!(
        amounts,
        vec![
            (decimal("1.23"), decimal("1.23")),
            (
                decimal("0.000000000000000001"),
                decimal("1.230000000000000001")
            )
        ]
    );

    cleanup_recharge_fixture(&pool, role_id, admin_id, user_id, asset_id).await?;
    Ok(())
}

#[tokio::test]
async fn admin_recharge_reads_precision_after_concurrent_asset_update_commits()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let user_id = create_user(&pool).await;
    let asset_id = create_asset(&pool, "ARL").await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let uri = format!("/admin/api/v1/users/{user_id}/recharge");
    let mut asset_update = pool.begin().await?;
    sqlx::query("UPDATE assets SET precision_scale = 2 WHERE id = ?")
        .bind(asset_id)
        .execute(&mut *asset_update)
        .await?;
    let mut recharge = Box::pin(app.oneshot(financial_request(
        "POST",
        &uri,
        &token,
        json!({
            "asset_id": asset_id, "amount": "0.001", "reason": "concurrent precision change",
            "idempotency_key": "locked-precision"
        }),
    )));
    // 资产配置事务尚未提交时，充值不得以旧精度完成；解锁后必须按最新精度拒绝。
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut recharge)
            .await
            .is_err()
    );
    asset_update.commit().await?;
    let response = tokio::time::timeout(std::time::Duration::from_secs(5), recharge).await??;
    let status = response.status();
    let payload = body_json(response).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{payload}");
    assert!(
        payload.to_string().contains("precision_scale 2"),
        "{payload}"
    );
    assert_eq!(recharge_effect_counts(&pool, user_id).await, (0, 0, 0, 0));
    cleanup_recharge_fixture(&pool, role_id, admin_id, user_id, asset_id).await?;
    Ok(())
}

#[tokio::test]
async fn admin_recharge_decimal_storage_overflow_rolls_back_all_effects()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let user_id = create_user(&pool).await;
    let asset_id = create_asset(&pool, "ARO").await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let uri = format!("/admin/api/v1/users/{user_id}/recharge");
    let response = app
        .clone()
        .oneshot(financial_request(
            "POST",
            &uri,
            &token,
            json!({
                "asset_id": asset_id, "amount": "100000000000000000000",
                "reason": "receipt overflow", "idempotency_key": "overflow-receipt"
            }),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(recharge_effect_counts(&pool, user_id).await, (0, 0, 0, 0));

    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(asset_id)
        .bind(decimal("99999999999999999999.999999999999999999"))
        .execute(&pool)
        .await?;
    let response = app
        .oneshot(financial_request(
            "POST",
            &uri,
            &token,
            json!({
                "asset_id": asset_id, "amount": "0.000000000000000001",
                "reason": "balance overflow", "idempotency_key": "overflow-balance"
            }),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(recharge_effect_counts(&pool, user_id).await, (1, 0, 0, 0));
    let available: BigDecimal =
        sqlx::query_scalar("SELECT available FROM wallet_accounts WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        available,
        decimal("99999999999999999999.999999999999999999")
    );
    cleanup_recharge_fixture(&pool, role_id, admin_id, user_id, asset_id).await?;
    Ok(())
}

async fn cleanup_recharge_fixture(
    pool: &MySqlPool,
    role_id: u64,
    admin_id: u64,
    user_id: u64,
    asset_id: u64,
) -> Result<(), Box<dyn Error>> {
    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(admin_id)
        .execute(pool)
        .await?;
    for table in ["admin_wallet_recharges", "wallet_ledger", "wallet_accounts"] {
        sqlx::query(&format!("DELETE FROM {table} WHERE user_id = ?"))
            .bind(user_id)
            .execute(pool)
            .await?;
    }
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(asset_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(role_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn admin_convert_create_rejects_invalid_financial_config_before_mysql()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let token = issue_token(&settings, "admin:1".to_owned(), TokenScope::Admin, 900)?;
    let app = build_router(AppState::new(settings));
    for invalid in [
        json!({"pricing_mode": "markte"}),
        json!({"spread_rate": "1"}),
        json!({"spread_rate": "1.01"}),
        json!({"spread_rate": "0.999999999"}),
        json!({"fee_rate": "1"}),
        json!({"fee_rate": "0.123456789"}),
        json!({"min_amount": "-1"}),
        json!({"min_amount": "2", "max_amount": "1"}),
        json!({"target_min_amount": "-1"}),
        json!({"target_min_amount": "2", "target_max_amount": "1"}),
    ] {
        let mut payload = json!({"from_asset_id": 1, "to_asset_id": 2, "pricing_mode": "fixed",
            "spread_rate": "0", "fee_rate": "0", "min_amount": "0", "reason": "reject invalid config"});
        payload
            .as_object_mut()
            .unwrap()
            .extend(invalid.as_object().unwrap().clone());
        let response = app
            .clone()
            .oneshot(financial_request(
                "POST",
                "/admin/api/v1/convert/pairs",
                &token,
                payload,
            ))
            .await?;
        let status = response.status();
        let body = body_json(response).await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "invalid {invalid}: {body}");
    }
    Ok(())
}

#[tokio::test]
async fn admin_convert_update_rejects_invalid_config_but_can_disable_legacy_pair()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let from_asset = create_asset(&pool, "AVF").await;
    let to_asset = create_asset(&pool, "AVT").await;
    let pair_id = seed_convert_pair(&pool, from_asset, to_asset, true).await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let uri = format!("/admin/api/v1/convert/pairs/{pair_id}");
    let original = body_json(
        app.clone()
            .oneshot(financial_request("GET", &uri, &token, json!({})))
            .await?,
    )
    .await?;

    for patch in [
        json!({"pricing_mode": "markte"}),
        json!({"spread_rate": "1"}),
        json!({"spread_rate": "0.999999999"}),
        json!({"fee_rate": "1"}),
        json!({"fee_rate": "0.123456789"}),
        json!({"min_amount": "-1"}),
        json!({"max_amount": "0"}),
        json!({"target_min_amount": "-1"}),
        json!({"target_min_amount": "2", "target_max_amount": "1"}),
    ] {
        let mut payload = patch.clone();
        payload["reason"] = json!("invalid patch");
        let response = app
            .clone()
            .oneshot(financial_request("PATCH", &uri, &token, payload))
            .await?;
        let status = response.status();
        let body = body_json(response).await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "patch {patch}: {body}");
        let current = body_json(
            app.clone()
                .oneshot(financial_request("GET", &uri, &token, json!({})))
                .await?,
        )
        .await?;
        assert_eq!(current, original);
    }
    let audit_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM admin_audit_logs WHERE admin_id = ?")
            .bind(admin_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(audit_count, 0);

    // 旧版本可写入未知模式和 spread=1；纯停用允许原样保留它们，但不得顺便写配置。
    sqlx::query(
        "UPDATE convert_pairs SET pricing_mode = ' legacy-mode ', spread_rate = 1 WHERE id = ?",
    )
    .bind(pair_id)
    .execute(&pool)
    .await?;
    let response = app
        .clone()
        .oneshot(financial_request(
            "PATCH",
            &uri,
            &token,
            json!({"enabled": false, "reason": "stop legacy invalid pair"}),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let disabled = body_json(response).await?;
    assert_eq!(disabled["enabled"], false);
    assert_eq!(disabled["pricing_mode"], " legacy-mode ");
    assert_eq!(
        decimal(disabled["spread_rate"].as_str().unwrap()),
        decimal("1")
    );
    for patch in [
        json!({"enabled": true}),
        json!({"enabled": false, "fee_rate": "0"}),
        json!({"enabled": false, "max_amount": null}),
        json!({}),
    ] {
        let mut payload = patch.clone();
        payload["reason"] = json!("strict legacy config");
        let response = app
            .clone()
            .oneshot(financial_request("PATCH", &uri, &token, payload))
            .await?;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "patch: {patch}");
    }
    let audit: (String, Value, Value) = sqlx::query_as(
        "SELECT action, before_json, after_json FROM admin_audit_logs WHERE admin_id = ?",
    )
    .bind(admin_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(audit.0, "convert_pair.update_status");
    assert_eq!(audit.1["enabled"], true);
    assert_eq!(audit.2["enabled"], false);
    assert_eq!(audit.1["pricing_mode"], audit.2["pricing_mode"]);
    let response = app
        .oneshot(financial_request(
            "PATCH",
            &uri,
            &token,
            json!({
                "pricing_mode": " market ", "spread_rate": "0.9999999900", "fee_rate": "0.99999999",
                "enabled": true, "reason": "repair and enable"
            }),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let repaired = body_json(response).await?;
    assert_eq!(repaired["pricing_mode"], "market");
    assert_eq!(repaired["enabled"], true);
    assert_eq!(repaired["spread_rate"], "0.99999999");
    assert_eq!(repaired["fee_rate"], "0.99999999");
    let audit_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM admin_audit_logs WHERE admin_id = ?")
            .bind(admin_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(audit_count, 2);

    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    delete_pair_and_assets(&pool, pair_id, from_asset, to_asset).await?;
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(role_id)
        .execute(&pool)
        .await?;
    Ok(())
}
