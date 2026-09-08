use super::*;

fn risk_request(method: &str, uri: &str, token: &str, payload: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap()
}

async fn risk_audit_count(pool: &MySqlPool, admin_id: u64) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM admin_audit_logs WHERE admin_id = ? AND target_type = 'risk_rule'",
    )
    .bind(admin_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn clean_risk_admin(pool: &MySqlPool, role_id: u64, admin_id: u64) {
    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(admin_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM risk_rules WHERE created_by = ?")
        .bind(admin_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(role_id)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn risk_config_invalid_create_is_validated_before_mysql() -> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let token = issue_token(&settings, "admin:1", TokenScope::Admin, 900)?;
    let app = build_router(AppState::new(settings));
    for config in [
        json!([]),
        json!({"operations": "spot.order.create"}),
        json!({"max_requests": null}),
    ] {
        let response = app.clone().oneshot(risk_request("POST", "/admin/api/v1/risk/rules", &token,
            json!({"rule_type": "custom", "target_type": "global", "config_json": config, "enabled": false}))).await?;
        let status = response.status();
        let payload = body_json(response).await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{payload}");
        assert_eq!(payload["code"], "VALIDATION_ERROR");
    }
    Ok(())
}

#[tokio::test]
async fn risk_config_invalid_create_has_no_rule_or_audit_side_effects() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    for enabled in [false, true] {
        for config in [
            json!(false),
            json!([]),
            json!({"blocked_operations": [1]}),
            json!({"max_amount": "-1"}),
            json!({"max_price_deviation_bps": 1.5}),
            json!({"max_requests": 4294967296_u64}),
            json!({"window_seconds": 0}),
        ] {
            let response = app.clone().oneshot(risk_request("POST", "/admin/api/v1/risk/rules", &token,
                json!({"rule_type": "custom", "target_type": "global", "config_json": config, "enabled": enabled}))).await?;
            let status = response.status();
            let payload = body_json(response).await?;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{payload}");
            assert_eq!(payload["code"], "VALIDATION_ERROR");
            let count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM risk_rules WHERE created_by = ?")
                    .bind(admin_id)
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(count, 0);
            assert_eq!(risk_audit_count(&pool, admin_id).await, 0);
        }
    }
    let valid = json!({"operations": [], "max_requests": 0, "extension": {"future": null}});
    let response = app.oneshot(risk_request("POST", "/admin/api/v1/risk/rules", &token,
        json!({"rule_type": "custom", "target_type": "global", "config_json": valid, "enabled": false}))).await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_json(response).await?["config_json"], valid);
    assert_eq!(risk_audit_count(&pool, admin_id).await, 1);
    clean_risk_admin(&pool, role_id, admin_id).await;
    Ok(())
}

#[tokio::test]
async fn risk_config_enable_validates_locked_json_but_legacy_disable_is_unconditional()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let mut expected_audits = 0;
    for config in [
        Value::Null,
        json!("legacy"),
        json!([]),
        json!({"operations": false}),
        json!({"max_amount": null}),
        json!({"window_seconds": 0}),
    ] {
        // 旧目标刻意不满足新建格式且不存在；停用/重新启用不应调用创建目标校验。
        let id = sqlx::query("INSERT INTO risk_rules (rule_type, target_type, target_id, config_json, enabled, created_by) VALUES ('legacy', 'pair', ?, ?, TRUE, ?)")
            .bind(format!("REMOVED-{}", Uuid::now_v7().simple())).bind(SqlxJson(config.clone())).bind(admin_id)
            .execute(&pool).await?.last_insert_id();
        let uri = format!("/admin/api/v1/risk/rules/{id}/status");
        for initial_enabled in [true, false] {
            sqlx::query("UPDATE risk_rules SET enabled = ? WHERE id = ?")
                .bind(initial_enabled)
                .bind(id)
                .execute(&pool)
                .await?;
            let response = app
                .clone()
                .oneshot(risk_request(
                    "PATCH",
                    &uri,
                    &token,
                    json!({"enabled": true}),
                ))
                .await?;
            let status = response.status();
            let payload = body_json(response).await?;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{payload}");
            let (enabled, stored): (bool, SqlxJson<Value>) =
                sqlx::query_as("SELECT enabled, config_json FROM risk_rules WHERE id = ?")
                    .bind(id)
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(enabled, initial_enabled);
            assert_eq!(stored.0, config);
            assert_eq!(risk_audit_count(&pool, admin_id).await, expected_audits);
        }
        sqlx::query("UPDATE risk_rules SET enabled = TRUE WHERE id = ?")
            .bind(id)
            .execute(&pool)
            .await?;
        for before_enabled in [true, false] {
            let response = app
                .clone()
                .oneshot(risk_request(
                    "PATCH",
                    &uri,
                    &token,
                    json!({"enabled": false}),
                ))
                .await?;
            assert_eq!(response.status(), StatusCode::OK);
            let payload = body_json(response).await?;
            assert_eq!(payload["enabled"], false);
            assert_eq!(payload["config_json"], config);
            expected_audits += 1;
            assert_eq!(risk_audit_count(&pool, admin_id).await, expected_audits);
            let (before, after, reason): (SqlxJson<Value>, SqlxJson<Value>, Option<String>) = sqlx::query_as("SELECT before_json, after_json, reason FROM admin_audit_logs WHERE admin_id = ? AND target_id = ? ORDER BY id DESC LIMIT 1")
                .bind(admin_id).bind(id.to_string()).fetch_one(&pool).await?;
            assert_eq!(before.0["enabled"], before_enabled);
            assert_eq!(after.0["enabled"], false);
            assert_eq!(before.0["config_json"], config);
            assert_eq!(after.0["config_json"], config);
            assert_eq!(reason, None);
        }
        sqlx::query("UPDATE risk_rules SET config_json = ? WHERE id = ?")
            .bind(SqlxJson(
                json!({"operations": [], "max_requests": 0, "extension": null}),
            ))
            .bind(id)
            .execute(&pool)
            .await?;
        let response = app
            .clone()
            .oneshot(risk_request(
                "PATCH",
                &uri,
                &token,
                json!({"enabled": true}),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(body_json(response).await?["enabled"], true);
        expected_audits += 1;
        assert_eq!(risk_audit_count(&pool, admin_id).await, expected_audits);
    }
    clean_risk_admin(&pool, role_id, admin_id).await;
    Ok(())
}

async fn risk_spot_pair(pool: &MySqlPool, base: u64, quote: u64) -> (u64, String) {
    let symbol = format!("RP{}", &Uuid::now_v7().simple().to_string()[12..]);
    let id = sqlx::query("INSERT INTO trading_pairs (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, status, market_type) VALUES (?, ?, ?, 2, 4, 1, 'active', 'spot')")
        .bind(base).bind(quote).bind(&symbol).execute(pool).await.unwrap().last_insert_id();
    (id, symbol)
}

async fn risk_spot_effects(
    pool: &MySqlPool,
    user_id: u64,
    quote: u64,
) -> (BigDecimal, BigDecimal, i64, i64) {
    sqlx::query_as("SELECT available, frozen, (SELECT COUNT(*) FROM spot_orders WHERE user_id = ?), (SELECT COUNT(*) FROM wallet_ledger WHERE user_id = ?) FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
        .bind(user_id).bind(user_id).bind(user_id).bind(quote).fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn risk_admin_numeric_pair_rule_guards_exact_pair_and_preserves_symbol_rules_and_replay()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let user_id = create_user(&pool).await;
    let base = create_asset(&pool, "RBA").await;
    let other_base = create_asset(&pool, "RBB").await;
    let quote = create_asset(&pool, "RBQ").await;
    let (pair_id, symbol) = risk_spot_pair(&pool, base, quote).await;
    let (other_pair_id, other_symbol) = risk_spot_pair(&pool, other_base, quote).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, 100)")
        .bind(user_id)
        .bind(quote)
        .execute(&pool)
        .await?;
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )?;
    let user_token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900)?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let order = |pair: &str, key: &str| {
        risk_request(
            "POST",
            "/api/v1/spot/orders",
            &user_token,
            json!({"pair_id": pair, "side": "buy", "order_type": "limit", "price": "10", "quantity": "2", "idempotency_key": key}),
        )
    };
    let response = app.clone().oneshot(order(&symbol, "before-risk")).await?;
    let status = response.status();
    let original = body_json(response).await?;
    assert_eq!(status, StatusCode::OK, "{original}");
    let before = risk_spot_effects(&pool, user_id, quote).await;
    let response = app.clone().oneshot(risk_request("POST", "/admin/api/v1/risk/rules", &admin_token,
        json!({"rule_type": "block", "target_type": "pair", "target_id": pair_id.to_string(), "config_json": {"blocked_operations": ["spot.order.create"]}}))).await?;
    let status = response.status();
    let rule = body_json(response).await?;
    assert_eq!(status, StatusCode::OK, "{rule}");
    assert_eq!(rule["target_id"], pair_id.to_string());
    assert_eq!(risk_audit_count(&pool, admin_id).await, 1);
    for pair in [&symbol, &pair_id.to_string()] {
        let response = app.clone().oneshot(order(pair, "new-rejected")).await?;
        let status = response.status();
        let payload = body_json(response).await?;
        assert_eq!(status, StatusCode::FORBIDDEN, "{payload}");
        assert_eq!(payload["code"], "risk_operation_not_allowed");
        assert_eq!(risk_spot_effects(&pool, user_id, quote).await, before);
    }
    let response = app.clone().oneshot(order(&symbol, "before-risk")).await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_json(response).await?, original);
    assert_eq!(risk_spot_effects(&pool, user_id, quote).await, before);
    let response = app
        .clone()
        .oneshot(order(&other_symbol, "other-pair"))
        .await?;
    let status = response.status();
    let payload = body_json(response).await?;
    assert_eq!(status, StatusCode::OK, "{payload}");
    let after_other = risk_spot_effects(&pool, user_id, quote).await;
    assert_eq!(after_other.0, decimal("60"));
    assert_eq!(after_other.1, decimal("40"));
    assert_eq!(after_other.2, 2);
    let numeric_rule = rule["id"].as_u64().unwrap();
    sqlx::query("UPDATE risk_rules SET enabled = FALSE WHERE id = ?")
        .bind(numeric_rule)
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO risk_rules (rule_type, target_type, target_id, config_json, enabled, created_by) VALUES ('legacy', 'pair', ?, ?, TRUE, ?)")
        .bind(&symbol).bind(SqlxJson(json!({"blocked_operations": ["spot.order.create"]}))).bind(admin_id).execute(&pool).await?;
    let response = app
        .clone()
        .oneshot(order(&symbol, "legacy-rejected"))
        .await?;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        body_json(response).await?["code"],
        "risk_operation_not_allowed"
    );
    assert_eq!(risk_spot_effects(&pool, user_id, quote).await, after_other);
    let events: Vec<(String, String, String)> =
        sqlx::query_as("SELECT event_type, decision, reason FROM risk_events WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(&pool)
            .await?;
    assert_eq!(
        events.len(),
        3,
        "replay and unrelated pair must not create risk events"
    );
    for (operation, decision, reason) in events {
        assert_eq!(operation, "spot.order.create");
        assert_eq!(decision, "reject");
        assert_eq!(reason, "该操作已被风控规则限制");
    }
    // 另一交易对的符号与原交易对数字 ID 相撞时，风控必须沿已加载的 canonical symbol 精确取 ID。
    // 不改变旧数字符号规则的命名空间；这里只验证新 Admin 主键目标没有被 OR 查询指向别的行。
    sqlx::query("UPDATE trading_pairs SET symbol = ? WHERE id = ?")
        .bind(pair_id.to_string())
        .bind(other_pair_id)
        .execute(&pool)
        .await?;
    let response = app.clone().oneshot(risk_request("POST", "/admin/api/v1/risk/rules", &admin_token,
        json!({"rule_type": "block", "target_type": "pair", "target_id": other_pair_id.to_string(), "config_json": {"blocked_operations": ["spot.order.create"]}}))).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let response = app
        .oneshot(order(&other_pair_id.to_string(), "numeric-symbol-rejected"))
        .await?;
    let status = response.status();
    let payload = body_json(response).await?;
    assert_eq!(status, StatusCode::FORBIDDEN, "{payload}");
    assert_eq!(payload["code"], "risk_operation_not_allowed");
    assert_eq!(risk_spot_effects(&pool, user_id, quote).await, after_other);
    let event: SqlxJson<Value> = sqlx::query_scalar(
        "SELECT payload_json FROM risk_events WHERE user_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;
    let scopes = event.0["scopes"].as_array().unwrap();
    assert!(scopes.contains(&json!({"dimension": "pair", "value": pair_id.to_string()})));
    assert!(scopes.contains(&json!({"dimension": "pair", "value": other_pair_id.to_string()})));
    sqlx::query("DELETE FROM risk_events WHERE user_id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_ledger WHERE user_id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM spot_orders WHERE user_id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    clean_risk_admin(&pool, role_id, admin_id).await;
    sqlx::query("DELETE FROM trading_pairs WHERE id IN (?, ?)")
        .bind(pair_id)
        .bind(other_pair_id)
        .execute(&pool)
        .await?;
    for asset in [base, other_base, quote] {
        sqlx::query("DELETE FROM assets WHERE id = ?")
            .bind(asset)
            .execute(&pool)
            .await?;
    }
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn risk_config_valid_create_and_enable_roll_back_when_audit_insert_fails()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let config = json!({"operations": [], "max_requests": 0});
    let id = sqlx::query("INSERT INTO risk_rules (rule_type, target_type, config_json, enabled, created_by) VALUES ('custom', 'global', ?, FALSE, ?)")
        .bind(SqlxJson(config.clone())).bind(admin_id).execute(&pool).await?.last_insert_id();
    let trigger = format!("risk_audit_fail_{}", Uuid::now_v7().simple());
    sqlx::raw_sql(&format!("CREATE TRIGGER {trigger} BEFORE INSERT ON admin_audit_logs FOR EACH ROW BEGIN IF NEW.admin_id = {admin_id} AND NEW.target_type = 'risk_rule' THEN SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'risk audit test rejection'; END IF; END"))
        .execute(&pool).await?;
    // 故障只命中本测试管理员；先收集响应并移除触发器，再断言，避免断言失败影响后续切片。
    let created = app
        .clone()
        .oneshot(risk_request(
            "POST",
            "/admin/api/v1/risk/rules",
            &token,
            json!({"rule_type": "custom", "target_type": "global", "config_json": config}),
        ))
        .await;
    let enabled = app
        .oneshot(risk_request(
            "PATCH",
            &format!("/admin/api/v1/risk/rules/{id}/status"),
            &token,
            json!({"enabled": true}),
        ))
        .await;
    sqlx::raw_sql(&format!("DROP TRIGGER {trigger}"))
        .execute(&pool)
        .await?;
    for response in [created?, enabled?] {
        let status = response.status();
        let payload = body_json(response).await?;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{payload}");
    }
    let rows: Vec<(u64, bool, SqlxJson<Value>)> =
        sqlx::query_as("SELECT id, enabled, config_json FROM risk_rules WHERE created_by = ?")
            .bind(admin_id)
            .fetch_all(&pool)
            .await?;
    assert_eq!(rows, vec![(id, false, SqlxJson(config))]);
    assert_eq!(risk_audit_count(&pool, admin_id).await, 0);
    clean_risk_admin(&pool, role_id, admin_id).await;
    Ok(())
}
