use super::*;
use exchange_api::modules::wallet::repository::{
    WalletChainBroadcastCommand, WalletChainBroadcastResult, WalletChainGateway,
    WalletChainGatewayError, WalletChainPollPage, WalletChainWithdrawalQueryResult,
};

struct MustNotBroadcast;

#[axum::async_trait]
impl WalletChainGateway for MustNotBroadcast {
    async fn broadcast_withdrawal(
        &self,
        _: &str,
        _: Option<&str>,
        _: &WalletChainBroadcastCommand,
    ) -> Result<WalletChainBroadcastResult, WalletChainGatewayError> {
        panic!("cooling must reject before gateway invocation");
    }
    async fn query_withdrawal(
        &self,
        _: &str,
        _: Option<&str>,
        _: &str,
    ) -> Result<WalletChainWithdrawalQueryResult, WalletChainGatewayError> {
        panic!("an unbroadcast cooled request has nothing to reconcile");
    }
    async fn poll_chain_events(
        &self,
        _: &str,
        _: Option<&str>,
        _: Option<&str>,
        _: u32,
    ) -> exchange_api::error::AppResult<WalletChainPollPage> {
        Ok(WalletChainPollPage {
            next_cursor: None,
            deposits: vec![],
            withdrawals: vec![],
        })
    }
}

async fn request(
    app: &Router,
    token: &str,
    method: &str,
    path: &str,
    payload: Value,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    (response.status(), body_json(response).await.unwrap())
}

async fn payload(app: &Router, token: &str, symbol: &str) -> Value {
    let quote = create_withdrawal_quote(app, token, symbol, "tron", "1")
        .await
        .unwrap();
    json!({
        "quote_id": quote["quote_id"], "asset_symbol": symbol, "network": "tron",
        "address": "TPolicyFixture", "amount": "1", "fee": quote["fee"],
        "idempotency_key": Uuid::now_v7().to_string(), "fund_password": "123456"
    })
}

#[tokio::test]
async fn withdrawal_policy_concurrency_review_replay_release_and_audit()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let user_id = create_user(&pool).await;
    let (asset_id, _) = create_asset(&pool).await;
    let symbol: String = sqlx::query_scalar("SELECT symbol FROM assets WHERE id = ?")
        .bind(asset_id)
        .fetch_one(&pool)
        .await?;
    allow_withdrawal_network(&pool, "tron", &symbol).await?;
    seed_wallet(&pool, user_id, asset_id, &Uuid::now_v7().to_string()).await;
    seed_fund_password(&pool, user_id, "123456").await;
    let (role1, admin1) = create_admin(&pool).await;
    let (role2, admin2) = create_admin(&pool).await;
    let settings = test_settings();
    let user_token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900)?;
    let admin_token = issue_token(&settings, format!("admin:{admin1}"), TokenScope::Admin, 900)?;
    let second_admin = issue_token(&settings, format!("admin:{admin2}"), TokenScope::Admin, 900)?;
    let app = routes().with_state(AppState::new(settings.clone()).with_mysql(pool.clone()));
    let admin = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let path = format!("/wallet/withdrawal-policies/{asset_id}");
    let (status, default) = request(&admin, &admin_token, "GET", &path, json!(null)).await;
    assert_eq!(status, StatusCode::OK, "{default}");
    assert_eq!(default["policy"]["enabled"], false);
    assert_eq!(default["revision"], 0);
    let mut policy = default["policy"].clone();
    policy["enabled"] = json!(true);
    policy["allowances"] = json!([{
        "user_id": null, "kyc_level": null, "window": "rolling", "window_seconds": 3600,
        "pending_mode": "all_outstanding", "max_amount": "1"
    }]);
    policy["review_tiers"] = json!([{"min_amount":"0","required_approvals":2}]);
    let trigger = format!("withdrawal_policy_audit_{}", Uuid::now_v7().simple());
    sqlx::raw_sql(&format!(
        "CREATE TRIGGER {trigger} BEFORE INSERT ON admin_audit_logs FOR EACH ROW BEGIN \
         IF NEW.reason = 'force-failure-fixture' THEN SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'fixture audit failure'; END IF; END"
    )).execute(&pool).await?;
    let failed = json!({"expected_revision":0,"policy":policy,"reason":"force-failure-fixture"});
    assert_eq!(
        request(&admin, &admin_token, "PATCH", &path, failed)
            .await
            .0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        request(&admin, &admin_token, "GET", &path, json!(null))
            .await
            .1["revision"],
        0
    );
    sqlx::query(
        "UPDATE admin_roles SET permissions = JSON_ARRAY('system.security.read') WHERE id = ?",
    )
    .bind(role2)
    .execute(&pool)
    .await?;
    assert_eq!(
        request(
            &admin,
            &second_admin,
            "PATCH",
            &path,
            json!({"expected_revision":0,"policy":policy,"reason":"unauthorized"})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    sqlx::query("UPDATE admin_roles SET permissions = JSON_ARRAY('*') WHERE id = ?")
        .bind(role2)
        .execute(&pool)
        .await?;
    let save = json!({"expected_revision":0,"policy":policy,"reason":"fixture policy approval"});
    assert_eq!(
        request(&admin, &admin_token, "PATCH", &path, save.clone())
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        request(&admin, &admin_token, "PATCH", &path, save).await.0,
        StatusCode::CONFLICT
    );
    // Concurrent split requests cannot both pass an allowance of one.
    let a = payload(&app, &user_token, &symbol).await;
    let b = payload(&app, &user_token, &symbol).await;
    let (first, second) = tokio::join!(
        request(&app, &user_token, "POST", "/wallet/withdrawals", a.clone()),
        request(&app, &user_token, "POST", "/wallet/withdrawals", b.clone()),
    );
    let (accepted, accepted_payload, blocked, retry_payload) = if first.0 == StatusCode::OK {
        (first, a, second, b)
    } else {
        (second, b, first, a)
    };
    assert_eq!(accepted.0, StatusCode::OK, "{accepted:?}");
    assert_eq!(blocked.0, StatusCode::FORBIDDEN, "{blocked:?}");
    assert_eq!(blocked.1["code"], "withdrawal_allowance_exceeded");
    let id = accepted.1["id"].as_u64().unwrap();
    let before: (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        request(
            &app,
            &user_token,
            "POST",
            "/wallet/withdrawals",
            accepted_payload
        )
        .await
        .1,
        accepted.1
    );
    let approve = format!("/wallet/withdrawals/{id}/approve");
    assert_eq!(
        request(
            &admin,
            &admin_token,
            "POST",
            &approve,
            json!({"reason":"force-failure-fixture"})
        )
        .await
        .0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    let votes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wallet_withdrawal_reviews WHERE withdrawal_id = ?",
    )
    .bind(id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(votes, 0, "audit failure rolls back the review vote");
    sqlx::raw_sql(&format!("DROP TRIGGER {trigger}"))
        .execute(&pool)
        .await?;
    assert_eq!(
        request(&admin, &admin_token, "POST", &approve, json!({}))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    for _ in 0..2 {
        let (status, review) = request(
            &admin,
            &admin_token,
            "POST",
            &approve,
            json!({"reason":"first review"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{review}");
        assert_eq!(review["status"], "pending_review");
        assert_eq!(review["approval_count"], 1);
        assert_eq!(review["required_approvals"], 2);
    }
    // Subsequent disable does not reduce an existing request's review snapshot.
    policy["enabled"] = json!(false);
    assert_eq!(
        request(
            &admin,
            &admin_token,
            "PATCH",
            &path,
            json!({"expected_revision":1,"policy":policy,"reason":"disable new policy"})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        request(
            &admin,
            &admin_token,
            "POST",
            &approve,
            json!({"reason":"same reviewer"})
        )
        .await
        .1["status"],
        "pending_review"
    );
    let (status, approved) = request(
        &admin,
        &second_admin,
        "POST",
        &approve,
        json!({"reason":"independent review"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{approved}");
    assert_eq!(approved["status"], "approved");
    assert_eq!(approved["approval_count"], 2);
    let after: (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(before, after);
    assert_eq!(
        request(
            &admin,
            &admin_token,
            "POST",
            &format!("/wallet/withdrawals/{id}/reject"),
            json!({"reason":"release fixture"})
        )
        .await
        .0,
        StatusCode::OK
    );
    policy["enabled"] = json!(true);
    assert_eq!(
        request(
            &admin,
            &admin_token,
            "PATCH",
            &path,
            json!({"expected_revision":2,"policy":policy,"reason":"reenable fixture"})
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, released) = request(
        &app,
        &user_token,
        "POST",
        "/wallet/withdrawals",
        retry_payload,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{released}");
    let reviews: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admin_audit_logs WHERE admin_id IN (?, ?) AND action = 'wallet.withdrawal.review'")
        .bind(admin1).bind(admin2).fetch_one(&pool).await?;
    assert_eq!(reviews, 2);
    cleanup_wallet_route_fixture(&pool, user_id, asset_id).await?;
    for (role, admin) in [(role1, admin1), (role2, admin2)] {
        sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
            .bind(admin)
            .execute(&pool)
            .await?;
        sqlx::query("DELETE FROM admin_users WHERE id = ?")
            .bind(admin)
            .execute(&pool)
            .await?;
        sqlx::query("DELETE FROM admin_roles WHERE id = ?")
            .bind(role)
            .execute(&pool)
            .await?;
    }
    Ok(())
}

#[tokio::test]
async fn withdrawal_policy_address_and_security_cooling_preserve_no_funds_on_rejection()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let user_id = create_user(&pool).await;
    let (asset_id, _) = create_asset(&pool).await;
    let symbol: String = sqlx::query_scalar("SELECT symbol FROM assets WHERE id = ?")
        .bind(asset_id)
        .fetch_one(&pool)
        .await?;
    allow_withdrawal_network(&pool, "tron", &symbol).await?;
    seed_wallet(&pool, user_id, asset_id, &Uuid::now_v7().to_string()).await;
    seed_fund_password(&pool, user_id, "123456").await;
    let (role, admin_id) = create_admin(&pool).await;
    let settings = test_settings();
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900)?;
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )?;
    let app = routes().with_state(AppState::new(settings.clone()).with_mysql(pool.clone()));
    let admin = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let path = format!("/wallet/withdrawal-policies/{asset_id}");
    let (_, initial) = request(&admin, &admin_token, "GET", &path, json!(null)).await;
    let mut policy = initial["policy"].clone();
    policy["enabled"] = json!(true);
    policy["address_cooling_seconds"] = json!(60);
    assert_eq!(
        request(
            &admin,
            &admin_token,
            "PATCH",
            &path,
            json!({"expected_revision":0,"policy":policy,"reason":"cooling fixture"})
        )
        .await
        .0,
        StatusCode::OK
    );
    let input = payload(&app, &token, &symbol).await;
    assert_eq!(
        request(&app, &token, "POST", "/wallet/withdrawals", input.clone())
            .await
            .1["code"],
        "withdrawal_address_cooling"
    );
    let address = json!({"network":"trc20","address":"TPolicyFixture","fund_password":"123456"});
    let registered = request(
        &app,
        &token,
        "POST",
        "/wallet/withdrawal-addresses",
        address.clone(),
    )
    .await;
    assert_eq!(registered.0, StatusCode::OK, "{registered:?}");
    assert_eq!(
        request(
            &app,
            &token,
            "POST",
            "/wallet/withdrawal-addresses",
            address
        )
        .await
        .1,
        registered.1
    );
    assert_eq!(
        request(&app, &token, "POST", "/wallet/withdrawals", input.clone())
            .await
            .1["code"],
        "withdrawal_address_cooling"
    );
    sqlx::query("UPDATE wallet_withdrawal_addresses SET created_at = DATE_SUB(CURRENT_TIMESTAMP(6), INTERVAL 61 SECOND) WHERE user_id = ?").bind(user_id).execute(&pool).await?;
    policy["security_cooling_seconds"] = json!(60);
    assert_eq!(
        request(
            &admin,
            &admin_token,
            "PATCH",
            &path,
            json!({"expected_revision":1,"policy":policy,"reason":"security fixture"})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        request(&app, &token, "POST", "/wallet/withdrawals", input.clone())
            .await
            .1["code"],
        "withdrawal_security_cooling"
    );
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM wallet_withdrawal_requests WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(count, 0);
    let balance: (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(balance, (decimal("12.5"), decimal("1.5")));
    // Backdate fixture creation clocks without bypassing credential-change triggers.
    sqlx::query("UPDATE users SET created_at = DATE_SUB(CURRENT_TIMESTAMP(6), INTERVAL 61 SECOND) WHERE id = ?").bind(user_id).execute(&pool).await?;
    sqlx::query("UPDATE user_security SET created_at = DATE_SUB(CURRENT_TIMESTAMP(6), INTERVAL 61 SECOND) WHERE user_id = ?").bind(user_id).execute(&pool).await?;
    let accepted = request(&app, &token, "POST", "/wallet/withdrawals", input.clone()).await;
    assert_eq!(accepted.0, StatusCode::OK, "{accepted:?}");
    let second_input = payload(&app, &token, &symbol).await;
    let (status, second_request) =
        request(&app, &token, "POST", "/wallet/withdrawals", second_input).await;
    assert_eq!(status, StatusCode::OK, "{second_request}");
    let approved_id = second_request["id"].as_u64().unwrap();
    assert_eq!(
        request(
            &admin,
            &admin_token,
            "POST",
            &format!("/wallet/withdrawals/{approved_id}/approve"),
            json!({"reason":"pre-change approval"})
        )
        .await
        .1["status"],
        "approved"
    );
    sqlx::query("UPDATE users SET password_hash = 'changed-fixture-password' WHERE id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    let changed: Option<chrono::DateTime<Utc>> =
        sqlx::query_scalar("SELECT withdrawal_security_changed_at FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_one(&pool)
            .await?;
    assert!(changed.is_some());
    let gateway_id = sqlx::query(
        "INSERT INTO wallet_chain_gateways (network, broadcast_url, status) VALUES ('tron', 'http://fixture.invalid/no-broadcast', 'active')",
    ).execute(&pool).await?.last_insert_id();
    let summary = exchange_api::workers::wallet_chain::run_once_with_gateway(
        &pool,
        None,
        &MustNotBroadcast,
        exchange_api::workers::wallet_chain::WalletChainWorkerConfig {
            enabled: true,
            interval_seconds: 1,
            batch_limit: 100,
            max_attempts: 3,
        },
    )
    .await?;
    assert_eq!(summary.withdrawal_broadcasted, 0);
    let state: (String, u32, Option<chrono::DateTime<Utc>>) = sqlx::query_as(
        "SELECT status, retry_count, released_at FROM wallet_withdrawal_requests WHERE id = ?",
    )
    .bind(approved_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(state, ("approved".into(), 0, None));
    sqlx::query("DELETE FROM wallet_chain_gateways WHERE id = ?")
        .bind(gateway_id)
        .execute(&pool)
        .await?;
    assert_eq!(
        request(&app, &token, "POST", "/wallet/withdrawals", input)
            .await
            .0,
        StatusCode::OK,
        "exact replay ignores subsequent cooling"
    );
    let id = accepted.1["id"].as_u64().unwrap();
    assert_eq!(
        request(
            &admin,
            &admin_token,
            "POST",
            &format!("/wallet/withdrawals/{id}/approve"),
            json!({"reason":"review"})
        )
        .await
        .1["code"],
        "withdrawal_security_cooling"
    );
    sqlx::query("INSERT INTO user_two_factor_settings (user_id, totp_secret_encrypted, totp_enabled) VALUES (?, 'fixture-encrypted', TRUE)").bind(user_id).execute(&pool).await?;
    sqlx::query("UPDATE user_two_factor_settings SET totp_secret_encrypted = NULL, totp_enabled = FALSE WHERE user_id = ?").bind(user_id).execute(&pool).await?;
    let totp_before: chrono::DateTime<Utc> = sqlx::query_scalar(
        "SELECT withdrawal_security_changed_at FROM user_two_factor_settings WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;
    sqlx::query("UPDATE user_two_factor_settings SET last_verified_at = CURRENT_TIMESTAMP WHERE user_id = ?").bind(user_id).execute(&pool).await?;
    let totp_after: chrono::DateTime<Utc> = sqlx::query_scalar(
        "SELECT withdrawal_security_changed_at FROM user_two_factor_settings WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        totp_before, totp_after,
        "ordinary verification must not extend cooling"
    );
    sqlx::query("DELETE FROM user_two_factor_settings WHERE user_id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    cleanup_wallet_route_fixture(&pool, user_id, asset_id).await?;
    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(role)
        .execute(&pool)
        .await?;
    Ok(())
}

/// Opt-in local browser fixture: no background workers or non-test database.
#[tokio::test]
#[ignore = "manual local browser fixture, serves for four minutes"]
async fn withdrawal_policy_browser_fixture() -> Result<(), Box<dyn Error>> {
    let url = std::env::var("DATABASE_URL")?;
    assert!(
        url.ends_with("/hardening_withdrawal_test"),
        "only the isolated database is permitted"
    );
    let pool = mysql_pool().await.unwrap();
    let (role, admin_id) = create_admin(&pool).await;
    let (asset_id, _) = create_asset(&pool).await;
    let settings = test_settings();
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        600,
    )?;
    println!(
        "WITHDRAWAL_PREVIEW {}",
        json!({"asset_id": asset_id, "admin_id": admin_id, "token": token})
    );
    let app = exchange_api::build_router(AppState::new(settings).with_mysql(pool.clone()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:18134").await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::time::sleep(std::time::Duration::from_secs(240)).await
        })
        .await?;
    sqlx::query("DELETE FROM wallet_withdrawal_policies WHERE asset_id = ?")
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(role)
        .execute(&pool)
        .await?;
    Ok(())
}
