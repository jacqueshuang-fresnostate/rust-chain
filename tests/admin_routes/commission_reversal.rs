use super::*;

struct Fixture {
    pool: MySqlPool,
    app: axum::Router,
    token: String,
    role: u64,
    admin: u64,
    owner: u64,
    user: u64,
    agent: u64,
    asset: u64,
    other_asset: u64,
    pair: u64,
}

impl Fixture {
    async fn new(pool: MySqlPool) -> Result<Self, Box<dyn Error>> {
        let settings = test_settings();
        let (role, admin) = create_admin_user(&pool).await;
        let owner = create_user(&pool).await;
        let user = create_user(&pool).await;
        let agent = sqlx::query("INSERT INTO agents (user_id, agent_code, path) VALUES (?, ?, '')")
            .bind(owner)
            .bind(format!("E08{}", Uuid::now_v7().simple()))
            .execute(&pool)
            .await?
            .last_insert_id();
        let asset = create_asset(&pool, &format!("E8{agent}A")).await;
        let other_asset = create_asset(&pool, &format!("E8{agent}B")).await;
        let pair = seed_convert_pair(&pool, asset, other_asset, true).await;
        let token = issue_token(&settings, format!("admin:{admin}"), TokenScope::Admin, 900)?;
        let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
        Ok(Self {
            pool,
            app,
            token,
            role,
            admin,
            owner,
            user,
            agent,
            asset,
            other_asset,
            pair,
        })
    }

    async fn commission(&self) -> Result<u64, Box<dyn Error>> {
        let source = seed_convert_order(
            &self.pool,
            self.user,
            self.pair,
            self.asset,
            self.other_asset,
            "completed",
        )
        .await;
        let id = seed_agent_commission_with_source_id(
            &self.pool,
            AgentCommissionSeed {
                agent_id: self.agent,
                user_id: self.user,
                source_type: "convert_order",
                source_id: &source,
                source_amount: "100",
                commission_amount: "5",
                status: "pending",
            },
        )
        .await;
        sqlx::query("UPDATE agent_commission_records SET payout_asset_id = ?, commission_rate = 0.05 WHERE id = ?")
            .bind(self.asset).bind(id).execute(&self.pool).await?;
        Ok(id)
    }

    async fn command(&self, id: u64, action: &str, body: Value) -> (StatusCode, Value) {
        let response = self
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method(if action == "status" { "PATCH" } else { "POST" })
                    .uri(format!("/admin/api/v1/agent-commissions/{id}/{action}"))
                    .header(AUTHORIZATION, format!("Bearer {}", self.token))
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload = serde_json::from_slice(&body)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&body).into_owned()));
        (status, payload)
    }

    async fn pay(&self, id: u64) {
        let result = self
            .command(
                id,
                "status",
                json!({"status":"settled","reason":"original payout"}),
            )
            .await;
        assert_eq!(result.0, StatusCode::OK, "{result:?}");
    }

    async fn reverse(&self, id: u64, key: &str, reason: &str) -> (StatusCode, Value) {
        self.command(
            id,
            "reversal",
            json!({"idempotency_key":key,"reason":reason}),
        )
        .await
    }

    async fn balance(&self) -> (BigDecimal, BigDecimal, BigDecimal) {
        sqlx::query_as("SELECT available, frozen, locked FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(self.owner).bind(self.asset).fetch_one(&self.pool).await.unwrap()
    }

    async fn assert_not_reversed(&self, id: u64) {
        let status: String =
            sqlx::query_scalar("SELECT status FROM agent_commission_records WHERE id = ?")
                .bind(id)
                .fetch_one(&self.pool)
                .await
                .unwrap();
        assert_eq!(status, "settled");
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM agent_commission_reversals WHERE commission_id = ?",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .unwrap();
        assert_eq!(count, 0);
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'agent_commission' AND ref_id = ? AND change_type = 'agent_commission_reversal'")
            .bind(id.to_string()).fetch_one(&self.pool).await.unwrap();
        assert_eq!(count, 0);
    }
}

#[tokio::test]
async fn commission_reversal_real_db_atomic_replay_concurrency_and_insufficient_balance()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let f = Fixture::new(pool).await?;
    let id = f.commission().await?;
    assert_eq!(
        f.reverse(id, "pending", "not paid").await.0,
        StatusCode::CONFLICT
    );
    let (first, second) = tokio::join!(
        f.command(id, "status", json!({"status":"settled"})),
        f.command(id, "status", json!({"status":"settled"}))
    );
    assert!(matches!(
        (first.0, second.0),
        (StatusCode::OK, StatusCode::CONFLICT) | (StatusCode::CONFLICT, StatusCode::OK)
    ));
    assert_eq!(f.balance().await.0, decimal("5"));
    let original: (String, String, BigDecimal, BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT source_type, source_id, source_amount, commission_rate, commission_amount FROM agent_commission_records WHERE id = ?"
    ).bind(id).fetch_one(&f.pool).await?;
    let legs: Vec<(String, BigDecimal)> = sqlx::query_as(
        "SELECT account_code, amount FROM platform_financial_journal WHERE transaction_key = ? ORDER BY account_code"
    ).bind(format!("agent_commission:{id}:payout")).fetch_all(&f.pool).await?;
    assert_eq!(
        legs,
        vec![
            ("platform_commission_expense".into(), decimal("5")),
            ("user_commission_wallet_liability".into(), decimal("-5"))
        ]
    );
    let (first, second) = tokio::join!(
        f.reverse(id, "reverse-1", "invalid source"),
        f.reverse(id, "reverse-1", "invalid source")
    );
    assert_eq!(first.0, StatusCode::OK, "{first:?}");
    assert_eq!(second, first);
    assert_eq!(first.1["admin_id"], f.admin);
    assert_eq!(first.1["original_commission"]["source_id"], original.1);
    assert_eq!(first.1["amount"], "5.000000000000000000");
    assert_eq!(f.balance().await.0, decimal("0"));
    // A new pool/router simulates process restart; replay is durable, not an in-memory cache.
    let restarted_pool = MySqlPoolOptions::new()
        .max_connections(2)
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;
    let restarted = build_router(AppState::new(test_settings()).with_mysql(restarted_pool.clone()));
    let response = restarted
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/api/v1/agent-commissions/{id}/reversal"))
                .header(AUTHORIZATION, format!("Bearer {}", f.token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"idempotency_key":"reverse-1","reason":" invalid source "}).to_string(),
                ))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_json(response).await?, first.1);
    restarted_pool.close().await;
    let (_, another_admin) = create_admin_user(&f.pool).await;
    let other_actor = issue_token(
        &test_settings(),
        format!("admin:{another_admin}"),
        TokenScope::Admin,
        900,
    )?;
    let response = f
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/api/v1/agent-commissions/{id}/reversal"))
                .header(AUTHORIZATION, format!("Bearer {other_actor}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"idempotency_key":"reverse-1","reason":"invalid source"}).to_string(),
                ))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        f.reverse(id, "reverse-1", "changed reason").await.0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        f.reverse(id, "new-key", "invalid source").await.0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        f.command(id, "status", json!({"status":"settled"})).await.0,
        StatusCode::CONFLICT
    );
    let after: (String, String, BigDecimal, BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT source_type, source_id, source_amount, commission_rate, commission_amount FROM agent_commission_records WHERE id = ?"
    ).bind(id).fetch_one(&f.pool).await?;
    assert_eq!(original, after);
    let audit: (i64, String) = sqlx::query_as("SELECT COUNT(*), MAX(reason) FROM admin_audit_logs WHERE target_id = ? AND action = 'agent_commission.reverse'")
        .bind(id.to_string()).fetch_one(&f.pool).await?;
    assert_eq!(audit, (1, "invalid source".into()));
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type='agent_commission' AND ref_id=?",
    )
    .bind(id.to_string())
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(count, 2);
    let totals: Vec<(String, BigDecimal)> = sqlx::query_as(
        "SELECT account_code, SUM(amount) FROM platform_financial_journal WHERE ref_type = 'agent_commission' AND ref_id = ? GROUP BY account_code"
    ).bind(id.to_string()).fetch_all(&f.pool).await?;
    assert_eq!(totals.len(), 2);
    assert!(totals.iter().all(|(_, amount)| amount == &decimal("0")));
    let unbalanced: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM (SELECT transaction_key, asset_id FROM platform_financial_journal WHERE context='agent_commission' GROUP BY transaction_key, asset_id HAVING SUM(amount) <> 0) groups_with_error"
    ).fetch_one(&f.pool).await?;
    assert_eq!(unbalanced, 0);

    // Same actor/key cannot reverse a different commission, even with sufficient funds.
    let other = f.commission().await?;
    f.pay(other).await;
    assert_eq!(
        f.reverse(other, "reverse-1", "invalid source").await.0,
        StatusCode::CONFLICT
    );
    f.assert_not_reversed(other).await;
    sqlx::query("UPDATE wallet_accounts SET available=4, frozen=12, locked=13 WHERE user_id=? AND asset_id=?")
        .bind(f.owner).bind(f.asset).execute(&f.pool).await?;
    assert_eq!(
        f.reverse(other, "insufficient", "refund source").await.0,
        StatusCode::CONFLICT
    );
    f.assert_not_reversed(other).await;
    assert_eq!(
        f.balance().await,
        (decimal("4"), decimal("12"), decimal("13"))
    );
    sqlx::query("UPDATE wallet_accounts SET available=5 WHERE user_id=? AND asset_id=?")
        .bind(f.owner)
        .bind(f.asset)
        .execute(&f.pool)
        .await?;
    assert_eq!(
        f.reverse(other, "insufficient", "refund source").await.0,
        StatusCode::OK
    );
    assert_eq!(
        f.balance().await,
        (decimal("0"), decimal("12"), decimal("13"))
    );

    // Different commissions compete for the same available funds; at most one debit commits.
    let a = f.commission().await?;
    let b = f.commission().await?;
    f.pay(a).await;
    f.pay(b).await;
    sqlx::query("UPDATE wallet_accounts SET available=5 WHERE user_id=? AND asset_id=?")
        .bind(f.owner)
        .bind(f.asset)
        .execute(&f.pool)
        .await?;
    let (ra, rb) = tokio::join!(
        f.reverse(a, "concurrent-a", "source revoked"),
        f.reverse(b, "concurrent-b", "source revoked")
    );
    assert!(
        matches!(
            (ra.0, rb.0),
            (StatusCode::OK, StatusCode::CONFLICT) | (StatusCode::CONFLICT, StatusCode::OK)
        ),
        "{ra:?} {rb:?}"
    );
    assert_eq!(f.balance().await.0, decimal("0"));

    // Authorization, mandatory reason and strict payload prevent client-selected debit terms.
    let no_auth = f
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/api/v1/agent-commissions/{id}/reversal"))
                .header("content-type", "application/json")
                .body(Body::from("{}"))?,
        )
        .await?;
    assert_eq!(no_auth.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        f.reverse(id, "valid", "   ").await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        f.reverse(id, "bad key", "reason").await.0,
        StatusCode::BAD_REQUEST
    );
    let forged = f
        .command(
            id,
            "reversal",
            json!({"idempotency_key":"valid","reason":"reason","amount":"1","admin_id":77}),
        )
        .await;
    assert!(forged.0.is_client_error());
    sqlx::query(
        "UPDATE admin_roles SET permissions=JSON_ARRAY('agents.commissions.read') WHERE id=?",
    )
    .bind(f.role)
    .execute(&f.pool)
    .await?;
    assert_eq!(
        f.reverse(id, "reverse-1", "invalid source").await.0,
        StatusCode::FORBIDDEN
    );
    sqlx::query(
        "UPDATE admin_roles SET permissions=JSON_ARRAY('agents.commissions.settle') WHERE id=?",
    )
    .bind(f.role)
    .execute(&f.pool)
    .await?;
    assert_eq!(
        f.reverse(id, "reverse-1", "invalid source").await.0,
        StatusCode::OK
    );
    sqlx::query("UPDATE admin_roles SET permissions=JSON_ARRAY('*') WHERE id=?")
        .bind(f.role)
        .execute(&f.pool)
        .await?;

    // Duplicate journal legs are failures, never silently accepted after debiting a wallet.
    let duplicate = f.commission().await?;
    f.pay(duplicate).await;
    sqlx::query("INSERT INTO platform_financial_journal (transaction_key, context, account_code, asset_id, amount, ref_type, ref_id) VALUES (?, 'agent_commission', 'platform_commission_expense', ?, -5, 'agent_commission', ?)")
        .bind(format!("agent_commission:{duplicate}:reverse")).bind(f.asset).bind(duplicate.to_string()).execute(&f.pool).await?;
    let balance = f.balance().await;
    assert_eq!(
        f.reverse(duplicate, "duplicate-journal", "reverse").await.0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    f.assert_not_reversed(duplicate).await;
    assert_eq!(f.balance().await, balance);
    sqlx::query("DELETE FROM platform_financial_journal WHERE transaction_key=?")
        .bind(format!("agent_commission:{duplicate}:reverse"))
        .execute(&f.pool)
        .await?;

    // Audit insertion failure rolls back all four financial writes.
    sqlx::raw_sql("CREATE TRIGGER e08_fail_audit BEFORE INSERT ON admin_audit_logs FOR EACH ROW BEGIN IF NEW.action = 'agent_commission.reverse' THEN SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='e08 audit failure'; END IF; END")
        .execute(&f.pool).await?;
    let failed = f.reverse(duplicate, "audit-failure", "reverse").await;
    sqlx::raw_sql("DROP TRIGGER e08_fail_audit")
        .execute(&f.pool)
        .await?;
    assert_eq!(failed.0, StatusCode::INTERNAL_SERVER_ERROR, "{failed:?}");
    f.assert_not_reversed(duplicate).await;
    assert_eq!(f.balance().await, balance);
    let legs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM platform_financial_journal WHERE transaction_key=?",
    )
    .bind(format!("agent_commission:{duplicate}:reverse"))
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(legs, 0);
    assert_eq!(
        f.reverse(duplicate, "audit-failure", "reverse").await.0,
        StatusCode::OK
    );
    // Case-distinct raw keys remain distinct despite a normal text collation.
    let lower = f.commission().await?;
    let upper = f.commission().await?;
    f.pay(lower).await;
    f.pay(upper).await;
    assert_eq!(
        f.reverse(lower, "case-key", "reverse").await.0,
        StatusCode::OK
    );
    assert_eq!(
        f.reverse(upper, "CASE-KEY", "reverse").await.0,
        StatusCode::OK
    );
    assert_eq!(
        f.reverse(lower, "CASE-KEY", "reverse").await.0,
        StatusCode::CONFLICT
    );
    let metadata: (String, String) = sqlx::query_as(
        "SELECT CHARACTER_SET_NAME, COLLATION_NAME FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='agent_commission_reversals' AND COLUMN_NAME='idempotency_key'"
    ).fetch_one(&f.pool).await?;
    assert_eq!(metadata, ("utf8mb4".into(), "utf8mb4_unicode_ci".into()));

    // Legacy settled flags without a unique payment fact must never invent a debit.
    let missing = f.commission().await?;
    sqlx::query("UPDATE agent_commission_records SET status='settled' WHERE id=?")
        .bind(missing)
        .execute(&f.pool)
        .await?;
    assert_eq!(
        f.reverse(missing, "missing-ledger", "legacy").await.0,
        StatusCode::CONFLICT
    );
    f.assert_not_reversed(missing).await;

    // A changed current agent binding cannot redirect the debit away from the original payee.
    let rebound = f.commission().await?;
    f.pay(rebound).await;
    let new_owner = create_user(&f.pool).await;
    sqlx::query("UPDATE agents SET user_id=? WHERE id=?")
        .bind(new_owner)
        .bind(f.agent)
        .execute(&f.pool)
        .await?;
    let response = f.reverse(rebound, "rebound", "original payee").await;
    assert_eq!(response.0, StatusCode::OK);
    assert_eq!(response.1["agent_user_id"], f.owner);
    assert_eq!(f.balance().await.0, decimal("0"));
    sqlx::query("UPDATE agents SET user_id=? WHERE id=?")
        .bind(f.owner)
        .bind(f.agent)
        .execute(&f.pool)
        .await?;

    // Payout-side journal failure also rolls back the credit and retains pending status.
    let failed_payout = f.commission().await?;
    sqlx::query("INSERT INTO platform_financial_journal (transaction_key, context, account_code, asset_id, amount, ref_type, ref_id) VALUES (?, 'agent_commission', 'platform_commission_expense', ?, 5, 'agent_commission', ?)")
        .bind(format!("agent_commission:{failed_payout}:payout")).bind(f.asset).bind(failed_payout.to_string()).execute(&f.pool).await?;
    assert_eq!(
        f.command(failed_payout, "status", json!({"status":"settled"}))
            .await
            .0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    let pending: String =
        sqlx::query_scalar("SELECT status FROM agent_commission_records WHERE id=?")
            .bind(failed_payout)
            .fetch_one(&f.pool)
            .await?;
    assert_eq!(pending, "pending");
    assert_eq!(f.balance().await.0, decimal("0"));
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type='agent_commission' AND ref_id=?",
    )
    .bind(failed_payout.to_string())
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(count, 0);
    sqlx::query("DELETE FROM platform_financial_journal WHERE transaction_key=?")
        .bind(format!("agent_commission:{failed_payout}:payout"))
        .execute(&f.pool)
        .await?;
    Ok(())
}
