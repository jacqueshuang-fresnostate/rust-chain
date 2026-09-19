use super::*;
use exchange_api::build_router;
use serde_json::json;

struct Fixture {
    pool: MySqlPool,
    app: axum::Router,
    admin: u64,
    admin_token: String,
    user_token: String,
    user: u64,
    asset: u64,
    product: u64,
    symbol: String,
    agent: u64,
    hub: EventBroadcastHub,
}

impl Fixture {
    async fn new() -> Option<Self> {
        let url = std::env::var("SECONDS_REFUND_TEST_DATABASE_URL").ok()?;
        assert!(url.starts_with("mysql://root@127.0.0.1:13316/"));
        assert!(url.ends_with("/hardening_commission_test"));
        let pool = MySqlPoolOptions::new()
            .max_connections(12)
            .connect(&url)
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let redis_url =
            std::env::var("SECONDS_REFUND_TEST_REDIS_URL").expect("isolated Redis URL required");
        assert!(redis_url.starts_with("redis://127.0.0.1:"));
        let redis = redis::aio::ConnectionManager::new(redis::Client::open(redis_url).unwrap())
            .await
            .unwrap();
        let settings = test_settings();
        let admin = create_admin(&pool).await;
        let mut tx = pool.begin().await.unwrap();
        let user = create_user(&mut tx).await;
        let (base, base_symbol) = create_asset(&mut tx, "RFB").await;
        let (asset, quote_symbol) = create_asset(&mut tx, "RFQ").await;
        let symbol = format!("{base_symbol}-{quote_symbol}");
        let pair = create_pair(&mut tx, base, asset, &symbol).await;
        let product = seed_seconds_product(&mut tx, pair, asset).await;
        sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available, frozen, locked) VALUES (?, ?, 100000, 7, 9)")
            .bind(user).bind(asset).execute(&mut *tx).await.unwrap();
        tx.commit().await.unwrap();
        let agent = support::seed_direct_agent_commission(&pool, user, "seconds_contract", "0.1")
            .await
            .unwrap()
            .agent_id;
        seed_ticker_at(
            &redis,
            &symbol,
            "100",
            chrono::Utc::now().timestamp_millis(),
        )
        .await;
        let admin_token =
            issue_token(&settings, format!("admin:{admin}"), TokenScope::Admin, 900).unwrap();
        let user_token =
            issue_token(&settings, format!("user:{user}"), TokenScope::User, 900).unwrap();
        let hub = EventBroadcastHub::new(64);
        let app = build_router(
            AppState::new(settings)
                .with_mysql(pool.clone())
                .with_redis(redis)
                .with_event_broadcast_hub(hub.clone()),
        );
        Some(Self {
            pool,
            app,
            admin,
            admin_token,
            user_token,
            user,
            asset,
            product,
            symbol,
            agent,
            hub,
        })
    }

    async fn request(
        &self,
        method: &str,
        path: &str,
        body: Value,
        token: Option<&str>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json");
        if let Some(token) = token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        let response = timeout(
            Duration::from_secs(15),
            self.app
                .clone()
                .oneshot(request.body(Body::from(body.to_string())).unwrap()),
        )
        .await
        .expect("request deadlocked")
        .unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()));
        (status, value)
    }

    async fn policy(&self, version: u64, enabled: bool, wait: Option<u32>) -> (StatusCode, Value) {
        self.request("PATCH", &format!("/admin/api/v1/seconds-contracts/products/{}/refund-policy", self.product),
            json!({"expected_version":version,"enabled":enabled,"wait_seconds":wait,"reason":"explicit test policy"}),
            Some(&self.admin_token)).await
    }

    async fn open(&self) -> (u64, Value) {
        let body = json!({"product_id":self.product,"direction":"up","stake_amount":"10.000000000000000000","idempotency_key":Uuid::now_v7().to_string()});
        let response = self
            .request(
                "POST",
                "/api/v1/seconds-contracts/orders",
                body.clone(),
                Some(&self.user_token),
            )
            .await;
        assert_eq!(response.0, StatusCode::OK, "{response:?}");
        (response.1["order"]["id"].as_u64().unwrap(), body)
    }

    async fn review(&self, id: u64, age: i64) {
        sqlx::query("UPDATE seconds_contract_orders SET status = 'manual_review', expires_at = DATE_SUB(CURRENT_TIMESTAMP(6), INTERVAL ? SECOND), settlement_failure_code = 'missing_settlement_snapshot', settlement_failed_at = DATE_SUB(CURRENT_TIMESTAMP(6), INTERVAL ? SECOND), settlement_window_start = DATE_SUB(CURRENT_TIMESTAMP(6), INTERVAL ? SECOND), settlement_window_end = DATE_ADD(DATE_SUB(CURRENT_TIMESTAMP(6), INTERVAL ? SECOND), INTERVAL 5 SECOND) WHERE id = ?")
            .bind(86400 + id * 10).bind(age).bind(86400 + id * 10).bind(86400 + id * 10).bind(id)
            .execute(&self.pool).await.unwrap();
        sqlx::query("INSERT INTO seconds_contract_settlement_exceptions (order_id, failure_code, detected_at, window_start, window_end) SELECT id, settlement_failure_code, settlement_failed_at, settlement_window_start, settlement_window_end FROM seconds_contract_orders WHERE id = ?")
            .bind(id).execute(&self.pool).await.unwrap();
    }

    async fn refund(&self, id: u64, key: &str, reason: &str) -> (StatusCode, Value) {
        self.request(
            "POST",
            &format!("/admin/api/v1/seconds-contracts/orders/{id}/principal-refund"),
            json!({"idempotency_key":key,"reason":reason}),
            Some(&self.admin_token),
        )
        .await
    }

    async fn balance(&self) -> (BigDecimal, BigDecimal, BigDecimal) {
        sqlx::query_as("SELECT available, frozen, locked FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(self.user).bind(self.asset).fetch_one(&self.pool).await.unwrap()
    }

    async fn commission(&self, id: u64) -> u64 {
        sqlx::query_scalar("SELECT id FROM agent_commission_records WHERE source_type = 'seconds_contract_order' AND source_id = ? AND agent_id = ?")
            .bind(id.to_string()).bind(self.agent).fetch_one(&self.pool).await.unwrap()
    }

    async fn pay(&self, commission: u64) -> (StatusCode, Value) {
        self.request(
            "PATCH",
            &format!("/admin/api/v1/agent-commissions/{commission}/status"),
            json!({"status":"settled","reason":"test source race"}),
            Some(&self.admin_token),
        )
        .await
    }

    async fn assert_untouched(&self, id: u64) {
        let result: (String, i64, i64) = sqlx::query_as("SELECT status, (SELECT COUNT(*) FROM seconds_principal_refunds WHERE order_id = orders.id), (SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'seconds_contract_order' AND ref_id = CAST(orders.id AS CHAR) AND change_type = 'seconds_contract_principal_refund') FROM seconds_contract_orders orders WHERE id = ?")
            .bind(id).fetch_one(&self.pool).await.unwrap();
        assert_eq!(result, ("manual_review".into(), 0, 0));
    }
}

#[tokio::test]
async fn prospective_policy_refund_real_database_contract() {
    let Some(f) = Fixture::new().await else {
        eprintln!("skipped: SECONDS_REFUND_TEST_DATABASE_URL absent");
        return;
    };
    let policy_path = format!(
        "/admin/api/v1/seconds-contracts/products/{}/refund-policy",
        f.product
    );
    let default = f
        .request("GET", &policy_path, Value::Null, Some(&f.admin_token))
        .await;
    assert_eq!(default.0, StatusCode::OK, "{default:?}");
    assert_eq!(
        default.1,
        json!({"product_id":f.product,"version":0,"enabled":false,"wait_seconds":null})
    );
    assert_eq!(f.policy(0, true, None).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(
        f.request(
            "PATCH",
            &policy_path,
            json!({"expected_version":0,"enabled":true,"wait_seconds":0,"reason":" "}),
            Some(&f.admin_token)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (legacy, legacy_body) = f.open().await;
    f.review(legacy, 600).await;
    assert_eq!(f.policy(0, true, Some(3600)).await.0, StatusCode::OK);
    let replay = f
        .request(
            "POST",
            "/api/v1/seconds-contracts/orders",
            legacy_body,
            Some(&f.user_token),
        )
        .await;
    assert_eq!(replay.0, StatusCode::OK);
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM seconds_order_refund_snapshots WHERE order_id = ?",
    )
    .bind(legacy)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(
        count, 0,
        "legacy replay must never acquire a policy snapshot"
    );
    assert_eq!(
        f.refund(legacy, "legacy", "no retroactive rights").await.0,
        StatusCode::CONFLICT
    );
    let (waiting, _) = f.open().await;
    f.review(waiting, 600).await;
    assert_eq!(
        f.refund(waiting, "waiting", "not yet").await.0,
        StatusCode::CONFLICT
    );
    let (elapsed, _) = f.open().await;
    f.review(elapsed, 4000).await;
    assert_eq!(f.policy(1, false, None).await.0, StatusCode::OK);
    assert_eq!(f.policy(1, true, Some(0)).await.0, StatusCode::CONFLICT);
    let (disabled, _) = f.open().await;
    f.review(disabled, 600).await;
    assert_eq!(
        f.refund(disabled, "disabled", "no snapshot").await.0,
        StatusCode::CONFLICT
    );
    let before = f.balance().await;
    let mut refund_events = f.hub.subscribe(&WebSocketChannel::private_user(f.user));
    let mut admin_events = f.hub.subscribe(&WebSocketChannel::private_user(f.admin));
    let (a, b) = tokio::join!(
        f.refund(elapsed, "stable-key", "  no evidence  "),
        f.refund(elapsed, "stable-key", "no evidence")
    );
    assert_eq!(a.0, StatusCode::OK, "{a:?} {b:?}");
    assert_eq!(b.0, StatusCode::OK, "{b:?}");
    assert_eq!(a.1, b.1);
    assert_eq!(a.1["reason"], "no evidence");
    assert_eq!(a.1["policy_version"], 1);
    let event = timeout(Duration::from_millis(100), refund_events.recv())
        .await
        .unwrap()
        .unwrap();
    let event: Value = serde_json::from_str(event.payload()).unwrap();
    assert_eq!(event["type"], "seconds_contract.order.refunded");
    assert_eq!(event["order_id"], elapsed);
    assert_eq!(event["status"], "refunded");
    assert_eq!(
        decimal(event["refund_amount"].as_str().unwrap()),
        decimal("10")
    );
    assert!(event["result"].is_null() && event["settlement_price"].is_null());
    assert!(event.get("payout_amount").is_none());
    assert!(
        timeout(Duration::from_millis(25), refund_events.recv())
            .await
            .is_err()
    );
    if f.admin != f.user {
        assert!(
            timeout(Duration::from_millis(25), admin_events.recv())
                .await
                .is_err()
        );
    }
    let after = f.balance().await;
    assert_eq!(after, (before.0 + decimal("10"), before.1, before.2));
    assert_eq!(
        f.refund(elapsed, "other-key", "no evidence").await.0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        f.refund(elapsed, "stable-key", "changed").await.0,
        StatusCode::CONFLICT
    );
    let other_admin = create_admin(&f.pool).await;
    let other_token = issue_token(
        &test_settings(),
        format!("admin:{other_admin}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    assert_eq!(
        f.request(
            "POST",
            &format!("/admin/api/v1/seconds-contracts/orders/{elapsed}/principal-refund"),
            json!({"idempotency_key":"stable-key","reason":"no evidence"}),
            Some(&other_token)
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let restart_pool = MySqlPoolOptions::new()
        .max_connections(2)
        .connect(&std::env::var("SECONDS_REFUND_TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    let restarted = build_router(AppState::new(test_settings()).with_mysql(restart_pool.clone()));
    let replay = restarted
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/admin/api/v1/seconds-contracts/orders/{elapsed}/principal-refund"
                ))
                .header("authorization", format!("Bearer {}", f.admin_token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"idempotency_key":"stable-key","reason":"no evidence"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(body_json(replay).await.unwrap(), a.1);
    restart_pool.close().await;
    let commission = f.commission(elapsed).await;
    let status: String =
        sqlx::query_scalar("SELECT status FROM agent_commission_records WHERE id = ?")
            .bind(commission)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(status, "rejected");
    assert_eq!(f.pay(commission).await.0, StatusCode::CONFLICT);
    let settle = f
        .request(
            "POST",
            &format!("/admin/api/v1/seconds-contracts/orders/{elapsed}/settle"),
            json!({"result":"auto","reason":"cannot settle refunded"}),
            Some(&f.admin_token),
        )
        .await;
    assert_eq!(settle.0, StatusCode::CONFLICT);
    let (sum, legs): (BigDecimal, i64) = sqlx::query_as("SELECT SUM(amount), COUNT(*) FROM platform_financial_journal WHERE transaction_key = ? AND asset_id = ?")
        .bind(format!("seconds_contract:{elapsed}:refund")).bind(f.asset).fetch_one(&f.pool).await.unwrap();
    assert_eq!(sum, decimal("0"));
    assert_eq!(legs, 2);
    let audit: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admin_audit_logs WHERE action = 'seconds_contract_order.principal_refund' AND target_id = ?")
        .bind(elapsed.to_string()).fetch_one(&f.pool).await.unwrap();
    assert_eq!(audit, 1);
    let frozen: (Option<String>, Option<BigDecimal>) =
        sqlx::query_as("SELECT result, settlement_price FROM seconds_contract_orders WHERE id = ?")
            .bind(elapsed)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(frozen, (None, None));
    assert_eq!(f.policy(2, true, Some(0)).await.0, StatusCode::OK);
    let (collision, _) = f.open().await;
    f.review(collision, 600).await;
    let before = f.balance().await;
    assert_eq!(
        f.refund(collision, "stable-key", "no evidence").await.0,
        StatusCode::CONFLICT
    );
    f.assert_untouched(collision).await;
    assert_eq!(f.balance().await, before);
    let (race, _) = f.open().await;
    f.review(race, 600).await;
    let race_commission = f.commission(race).await;
    let (refund, pay) = tokio::join!(
        f.refund(race, "race", "no evidence"),
        f.pay(race_commission)
    );
    assert_eq!(refund.0, StatusCode::OK, "{refund:?}");
    assert_eq!(pay.0, StatusCode::CONFLICT, "{pay:?}");
    let (evidence, _) = f.open().await;
    f.review(evidence, 600).await;
    seed_order_event_price(&f.pool, evidence, "101").await;
    let settle_path = format!("/admin/api/v1/seconds-contracts/orders/{evidence}/settle");
    let (refund, settle) = tokio::join!(
        f.refund(evidence, "evidence", "must not refund"),
        f.request(
            "POST",
            &settle_path,
            json!({"result":"auto","reason":"evidence recovery"}),
            Some(&f.admin_token)
        )
    );
    assert_eq!(refund.0, StatusCode::CONFLICT, "{refund:?}");
    assert_eq!(settle.0, StatusCode::OK, "{settle:?}");
    // Hold the wallet so refund holds the empty history range while an archive insert arrives.
    let (late, _) = f.open().await;
    f.review(late, 600).await;
    let mut wallet_lock = f.pool.begin().await.unwrap();
    sqlx::query(
        "SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ? FOR UPDATE",
    )
    .bind(f.user)
    .bind(f.asset)
    .fetch_one(&mut *wallet_lock)
    .await
    .unwrap();
    let refund_future = f.refund(late, "late-archive", "no evidence at linearization");
    tokio::pin!(refund_future);
    assert!(
        timeout(Duration::from_millis(150), &mut refund_future)
            .await
            .is_err()
    );
    let archive = seed_order_event_price(&f.pool, late, "101");
    tokio::pin!(archive);
    assert!(
        timeout(Duration::from_millis(150), &mut archive)
            .await
            .is_err(),
        "empty-window lock must block late archive"
    );
    wallet_lock.commit().await.unwrap();
    let (late_refund, ()) = tokio::join!(&mut refund_future, &mut archive);
    assert_eq!(late_refund.0, StatusCode::OK, "{late_refund:?}");
    assert_eq!(
        f.request(
            "POST",
            &format!("/admin/api/v1/seconds-contracts/orders/{late}/settle"),
            json!({"result":"auto","reason":"late archive cannot resettle"}),
            Some(&f.admin_token)
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    for kind in [
        "paid",
        "payout",
        "missing_debit",
        "precision",
        "journal",
        "audit",
        "corrupt",
    ] {
        let (id, _) = f.open().await;
        f.review(id, 600).await;
        let commission = f.commission(id).await;
        match kind {
            "paid" => {
                sqlx::query("UPDATE agent_commission_records SET status = 'settled' WHERE id = ?")
                    .bind(commission)
                    .execute(&f.pool)
                    .await
                    .unwrap();
            }
            "payout" => {
                sqlx::query("INSERT INTO wallet_ledger (user_id, asset_id, amount, balance_type, balance_after, available_after, frozen_after, locked_after, change_type, ref_type, ref_id) VALUES (?, ?, 1, 'available', 1, 1, 0, 0, 'agent_commission_payout', 'agent_commission', ?)").bind(f.user).bind(f.asset).bind(commission.to_string()).execute(&f.pool).await.unwrap();
            }
            "missing_debit" => {
                sqlx::query("UPDATE wallet_ledger SET change_type = 'fixture_missing' WHERE ref_type = 'seconds_contract_order' AND ref_id = ?").bind(id.to_string()).execute(&f.pool).await.unwrap();
            }
            "precision" => {
                sqlx::query("UPDATE seconds_contract_orders SET stake_amount = 10.1 WHERE id = ?")
                    .bind(id)
                    .execute(&f.pool)
                    .await
                    .unwrap();
                sqlx::query("UPDATE wallet_ledger SET amount = -10.1 WHERE ref_type = 'seconds_contract_order' AND ref_id = ?").bind(id.to_string()).execute(&f.pool).await.unwrap();
                sqlx::query("UPDATE assets SET precision_scale = 0 WHERE id = ?")
                    .bind(f.asset)
                    .execute(&f.pool)
                    .await
                    .unwrap();
            }
            "journal" => {
                sqlx::query("INSERT INTO platform_financial_journal (context, transaction_key, asset_id, account_code, amount, ref_type, ref_id) VALUES ('seconds_contract', ?, ?, 'platform_seconds_pending_liability', 10, 'seconds_contract_order', ?)")
                    .bind(format!("seconds_contract:{id}:refund")).bind(f.asset).bind(id.to_string()).execute(&f.pool).await.unwrap();
            }
            "audit" => {
                sqlx::raw_sql(&format!("CREATE TRIGGER e08_refund_audit_failure BEFORE INSERT ON admin_audit_logs FOR EACH ROW BEGIN IF NEW.action = 'seconds_contract_order.principal_refund' AND NEW.target_id = '{id}' THEN SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'isolated refund audit fault'; END IF; END")).execute(&f.pool).await.unwrap();
            }
            "corrupt" => {
                seed_order_event_price(&f.pool, id, "101").await;
                sqlx::query("UPDATE market_price_ticks SET source = 'BITGET' WHERE symbol = REPLACE(UPPER(?), '-', '') AND observed_at = (SELECT expires_at FROM seconds_contract_orders WHERE id = ?)")
                    .bind(&f.symbol).bind(id).execute(&f.pool).await.unwrap();
            }
            _ => unreachable!(),
        }
        let before = f.balance().await;
        let mut failed_events = f.hub.subscribe(&WebSocketChannel::private_user(f.user));
        let result = f
            .refund(id, &format!("reject-{kind}"), "invalid evidence")
            .await;
        if kind == "audit" {
            sqlx::raw_sql("DROP TRIGGER e08_refund_audit_failure")
                .execute(&f.pool)
                .await
                .unwrap();
        }
        if kind == "precision" {
            sqlx::query("UPDATE assets SET precision_scale = 18 WHERE id = ?")
                .bind(f.asset)
                .execute(&f.pool)
                .await
                .unwrap();
        }
        assert_ne!(result.0, StatusCode::OK, "{kind}: {result:?}");
        if kind == "corrupt" {
            assert_eq!(result.0, StatusCode::BAD_REQUEST, "{result:?}");
        }
        f.assert_untouched(id).await;
        assert_eq!(f.balance().await, before, "{kind}");
        assert!(
            timeout(Duration::from_millis(25), failed_events.recv())
                .await
                .is_err(),
            "{kind} emitted failed refund"
        );
    }
    let path = format!("/admin/api/v1/seconds-contracts/orders/{collision}/principal-refund");
    assert_eq!(
        f.request(
            "POST",
            &path,
            json!({"reason":"x","idempotency_key":"x"}),
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.request(
            "POST",
            &path,
            json!({"reason":"x","idempotency_key":"x"}),
            Some(&f.user_token)
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        f.request(
            "POST",
            &path,
            json!({"reason":"x","idempotency_key":"x","amount":"99"}),
            Some(&f.admin_token)
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let role: u64 = sqlx::query_scalar("SELECT role_id FROM admin_users WHERE id = ?")
        .bind(f.admin)
        .fetch_one(&f.pool)
        .await
        .unwrap();
    for permissions in [
        json!(["seconds.orders.read"]),
        json!(["seconds.orders.write"]),
    ] {
        sqlx::query("UPDATE admin_roles SET permissions = ? WHERE id = ?")
            .bind(sqlx::types::Json(permissions))
            .bind(role)
            .execute(&f.pool)
            .await
            .unwrap();
        assert_eq!(
            f.request(
                "POST",
                &path,
                json!({"reason":"x","idempotency_key":"x"}),
                Some(&f.admin_token)
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
    sqlx::query(
        "UPDATE admin_roles SET permissions = JSON_ARRAY('seconds.orders.settle') WHERE id = ?",
    )
    .bind(role)
    .execute(&f.pool)
    .await
    .unwrap();
    let authorized = f
        .refund(collision, "settle-only", "authorized principal refund")
        .await;
    assert_eq!(authorized.0, StatusCode::OK, "{authorized:?}");
    sqlx::query("UPDATE admin_roles SET permissions = JSON_ARRAY('*') WHERE id = ?")
        .bind(role)
        .execute(&f.pool)
        .await
        .unwrap();
    let order_json = f
        .request(
            "GET",
            "/api/v1/seconds-contracts/orders?limit=100",
            Value::Null,
            Some(&f.user_token),
        )
        .await;
    assert!(
        order_json.1["orders"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["id"] == elapsed && o["status"] == "refunded" && o["result"].is_null())
    );
    for alias in ["/openapi.json", "/api/openapi.json"] {
        let docs = f.request("GET", alias, Value::Null, None).await;
        assert_eq!(docs.0, StatusCode::OK);
        assert!(
            docs.1["paths"]["/admin/api/v1/seconds-contracts/orders/{id}/principal-refund"]["post"]
                .is_object()
        );
        assert!(
            docs.1["paths"]["/admin/api/v1/seconds-contracts/products/{id}/refund-policy"]["patch"]
                .is_object()
        );
    }
}
