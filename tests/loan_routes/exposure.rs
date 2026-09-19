use super::*;
use utoipa::OpenApi;

#[test]
fn exposure_openapi_child_has_nullable_decimal_policy_and_inventory_fields() {
    let document = serde_json::to_value(super::loan_openapi::LoanApiDoc::openapi()).unwrap();
    let schemas = &document["components"]["schemas"];
    for field in ["user_principal_limit", "product_principal_capacity"] {
        assert_eq!(
            schemas["LoanPrincipalExposurePolicy"]["properties"][field]["type"],
            json!(["string", "null"])
        );
    }
    assert_eq!(
        schemas["LoanPrincipalExposurePolicy"]["properties"]["deny_borrowing_while_overdue"]["default"],
        false
    );
    let response = schemas["LoanProductResponse"].to_string();
    assert!(response.contains("reserved_principal"));
    assert!(response.contains("outstanding_principal"));
    assert!(document["paths"]["/admin/api/v1/loan/products"]["post"].is_object());
    assert!(document["paths"]["/admin/api/v1/loan/products/{id}"]["patch"].is_object());
}

struct ExposureHarness {
    pool: MySqlPool,
    app: axum::Router,
    admin: String,
    users: Vec<String>,
    user_ids: Vec<u64>,
    asset_id: u64,
}

impl ExposureHarness {
    async fn new() -> Option<Self> {
        let pool = mysql_pool().await?;
        let settings = test_settings();
        let governance = seed_loan_product_governance_fixture(&pool).await.unwrap();
        let admin = issue_token(
            &settings,
            format!("admin:{}", governance.admin_id),
            TokenScope::Admin,
            900,
        )
        .unwrap();
        let mut users = Vec::new();
        let mut user_ids = Vec::new();
        for _ in 0..4 {
            let user_id =
                sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, 'test')")
                    .bind(format!("loan-exposure-{}@example.test", Uuid::now_v7()))
                    .execute(&pool)
                    .await
                    .unwrap()
                    .last_insert_id();
            users.push(
                issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap(),
            );
            user_ids.push(user_id);
        }
        let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
        Some(Self {
            pool,
            app,
            admin,
            users,
            user_ids,
            asset_id: governance.asset_id,
        })
    }

    async fn product(&self, asset_id: u64, user_limit: Value, capacity: Value) -> Value {
        let mut body = loan_product_write_body(asset_id, "Exposure product", "Exposure test");
        body["min_amount"] = json!("0.000000000000000001");
        body["interest_rate"] = json!("0");
        body["max_amount"] = Value::Null;
        body["user_principal_limit"] = user_limit;
        body["product_principal_capacity"] = capacity;
        let (status, payload) = call(
            self.app.clone(),
            &self.admin,
            "POST",
            "/admin/api/v1/loan/products",
            body,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{payload}");
        payload
    }

    async fn update(&self, product: &Value, patch: Value) -> (StatusCode, Value) {
        let id = product["id"].as_u64().unwrap();
        let (_, mut body) = call(
            self.app.clone(),
            &self.admin,
            "GET",
            &format!("/admin/api/v1/loan/products/{id}"),
            Value::Null,
        )
        .await;
        body["reason"] = json!("Exposure policy update");
        for (key, value) in patch.as_object().unwrap() {
            body[key] = value.clone();
        }
        call(
            self.app.clone(),
            &self.admin,
            "PATCH",
            &format!("/admin/api/v1/loan/products/{id}"),
            body,
        )
        .await
    }

    async fn borrow(
        &self,
        user: usize,
        product: &Value,
        amount: &str,
        key: &str,
    ) -> (StatusCode, Value) {
        call(
            self.app.clone(),
            &self.users[user],
            "POST",
            "/api/v1/loan/orders",
            json!({"product_id": product["id"], "amount": amount, "idempotency_key": key}),
        )
        .await
    }

    async fn action(
        &self,
        order: &Value,
        action: &str,
        user: Option<usize>,
    ) -> (StatusCode, Value) {
        let id = order["order"]["id"].as_u64().unwrap();
        let prefix = if user.is_some() {
            "/api/v1"
        } else {
            "/admin/api/v1"
        };
        let token = user
            .map(|index| self.users[index].as_str())
            .unwrap_or(&self.admin);
        call(
            self.app.clone(),
            token,
            "POST",
            &format!("{prefix}/loan/orders/{id}/{action}"),
            json!({"reason":"test"}),
        )
        .await
    }
}

async fn call(
    app: axum::Router,
    token: &str,
    method: &str,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(if body.is_null() {
                    Body::empty()
                } else {
                    Body::from(body.to_string())
                })
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    (status, body_json(response).await.unwrap())
}

#[tokio::test]
async fn exposure_configuration_roundtrips_validates_and_audits() {
    let Some(h) = ExposureHarness::new().await else {
        return;
    };
    let product = h.product(h.asset_id, Value::Null, Value::Null).await;
    assert!(product["user_principal_limit"].is_null());
    assert!(product["product_principal_capacity"].is_null());
    assert_eq!(product["deny_borrowing_while_overdue"], false);
    assert_eq!(
        decimal(product["reserved_principal"].as_str().unwrap()),
        decimal("0")
    );
    assert_eq!(
        decimal(product["outstanding_principal"].as_str().unwrap()),
        decimal("0")
    );
    for field in ["user_principal_limit", "product_principal_capacity"] {
        for invalid in ["-1", "0.0000000000000000001", "100000000000000000000"] {
            let (status, payload) = h.update(&product, json!({field: invalid})).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{field}: {payload}");
        }
        let (status, updated) = h
            .update(&product, json!({field:"0.0000000000000000010"}))
            .await;
        assert_eq!(status, StatusCode::OK, "{updated}");
        assert_eq!(
            decimal(updated[field].as_str().unwrap()),
            decimal("0.000000000000000001")
        );
    }
    let (status, updated) = h
        .update(
            &product,
            json!({
                "user_principal_limit":"0", "product_principal_capacity":"10.50",
                "deny_borrowing_while_overdue":true
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    let audit: SqlxJson<Value> = sqlx::query_scalar(
        "SELECT after_json FROM admin_audit_logs WHERE target_type = 'loan_product' AND target_id = ? ORDER BY id DESC LIMIT 1"
    ).bind(product["id"].as_u64().unwrap().to_string()).fetch_one(&h.pool).await.unwrap();
    assert_eq!(audit["deny_borrowing_while_overdue"], true);
    assert_eq!(
        decimal(audit["user_principal_limit"].as_str().unwrap()),
        decimal("0")
    );
    assert_eq!(
        h.borrow(0, &product, "1", "zero-limit").await.0,
        StatusCode::BAD_REQUEST
    );
    let (status, cleared) = h
        .update(
            &product,
            json!({
                "user_principal_limit":null, "product_principal_capacity":null,
                "deny_borrowing_while_overdue":false
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{cleared}");
    assert!(cleared["user_principal_limit"].is_null());
    assert!(cleared["product_principal_capacity"].is_null());
    assert_eq!(
        h.borrow(0, &product, "100000", "disabled-limits").await.0,
        StatusCode::OK
    );
    assert!(
        sqlx::query("UPDATE loan_products SET user_principal_limit = -1 WHERE id = ?")
            .bind(product["id"].as_u64().unwrap())
            .execute(&h.pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE loan_products SET product_principal_capacity = -1 WHERE id = ?")
            .bind(product["id"].as_u64().unwrap())
            .execute(&h.pool)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn exposure_boundaries_cross_product_asset_isolation_and_terminal_release() {
    let Some(h) = ExposureHarness::new().await else {
        return;
    };
    let a = h.product(h.asset_id, json!("10"), json!("20")).await;
    let b = h.product(h.asset_id, json!("10"), json!("4")).await;
    let other_asset = create_asset(&h.pool, "EXO").await.unwrap();
    let c = h.product(other_asset, json!("100"), json!("100")).await;
    let (status, foreign) = h.borrow(0, &c, "100", "foreign").await;
    assert_eq!(status, StatusCode::OK, "{foreign}");
    let (status, first) = h.borrow(0, &a, "6", "six").await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let (status, second) = h.borrow(0, &b, "4", "four").await;
    assert_eq!(status, StatusCode::OK, "{second}");
    let (status, failed) = h
        .borrow(0, &a, "0.000000000000000001", "above-boundary")
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{failed}");
    assert_eq!(
        h.borrow(1, &b, "0.000000000000000001", "product-full")
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let (status, replay) = h.borrow(0, &a, "6.000", "six").await;
    assert_eq!(status, StatusCode::OK, "{replay}");
    assert_eq!(replay["changed"], false);
    assert_eq!(h.borrow(0, &a, "5", "six").await.0, StatusCode::CONFLICT);
    assert_eq!(
        h.update(&b, json!({"user_principal_limit":"9"})).await.0,
        StatusCode::OK
    );
    let (status, denied) = h.action(&second, "approve", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{denied}");
    assert!(
        order_ledger(&h.pool, second["order"]["id"].as_u64().unwrap())
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        h.update(&b, json!({"user_principal_limit":"10"})).await.0,
        StatusCode::OK
    );
    let (status, approved) = h.action(&second, "approve", None).await;
    assert_eq!(status, StatusCode::OK, "{approved}");
    assert_eq!(approved["order"]["status"], "disbursed");
    assert_eq!(
        h.borrow(0, &a, "1", "disbursed-counts").await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(h.action(&first, "cancel", Some(0)).await.0, StatusCode::OK);
    let (status, replacement) = h.borrow(0, &a, "6", "replacement").await;
    assert_eq!(status, StatusCode::OK, "{replacement}");
    assert_eq!(
        h.action(&replacement, "reject", None).await.0,
        StatusCode::OK
    );
    assert_eq!(h.action(&second, "repay", Some(0)).await.0, StatusCode::OK);
    let (status, full) = h.borrow(0, &a, "10", "after-repaid").await;
    assert_eq!(status, StatusCode::OK, "{full}");
    // Terminal-state accounting fixture; liquidation financial behavior is covered separately.
    sqlx::query("UPDATE loan_orders SET status = 'liquidated' WHERE id = ?")
        .bind(full["order"]["id"].as_u64().unwrap())
        .execute(&h.pool)
        .await
        .unwrap();
    assert_eq!(
        h.borrow(0, &a, "10", "after-liquidated").await.0,
        StatusCode::OK
    );
    assert_eq!(
        h.update(&a, json!({"asset_id":other_asset})).await.0,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn exposure_concurrent_many_small_applications_and_approval_recheck() {
    let Some(h) = ExposureHarness::new().await else {
        return;
    };
    let a = h.product(h.asset_id, json!("0.05"), json!("0.07")).await;
    let b = h.product(h.asset_id, json!("0.05"), Value::Null).await;
    let mut tasks = tokio::task::JoinSet::new();
    for index in 0..30 {
        let app = h.app.clone();
        let token = h.users[0].clone();
        let id = if index % 2 == 0 {
            a["id"].clone()
        } else {
            b["id"].clone()
        };
        tasks.spawn(async move {
            call(app, &token, "POST", "/api/v1/loan/orders",
                json!({"product_id":id, "amount":"0.01", "idempotency_key":format!("small-{index}")})).await
        });
    }
    let mut accepted = Vec::new();
    while let Some(result) = tasks.join_next().await {
        let (status, payload) = result.unwrap();
        assert!(
            matches!(status, StatusCode::OK | StatusCode::BAD_REQUEST),
            "{status}: {payload}"
        );
        if status == StatusCode::OK {
            accepted.push(payload);
        }
    }
    assert_eq!(accepted.len(), 5);
    let total: BigDecimal =
        sqlx::query_scalar("SELECT SUM(amount) FROM loan_orders WHERE user_id = ?")
            .bind(h.user_ids[0])
            .fetch_one(&h.pool)
            .await
            .unwrap();
    assert_eq!(total, decimal("0.05"));
    // A separate asset/product makes the multi-user product-cap test independent.
    let asset = create_asset(&h.pool, "EXC").await.unwrap();
    let capacity = h.product(asset, Value::Null, json!("0.07")).await;
    for index in 0..32 {
        let app = h.app.clone();
        let token = h.users[index % h.users.len()].clone();
        let id = capacity["id"].clone();
        tasks.spawn(async move {
            call(app, &token, "POST", "/api/v1/loan/orders",
                json!({"product_id":id,"amount":"0.01","idempotency_key":format!("capacity-{index}")})).await
        });
    }
    let mut reservations = Vec::new();
    while let Some(result) = tasks.join_next().await {
        let (status, payload) = result.unwrap();
        assert!(
            matches!(status, StatusCode::OK | StatusCode::BAD_REQUEST),
            "{status}: {payload}"
        );
        if status == StatusCode::OK {
            reservations.push(payload);
        }
    }
    assert_eq!(reservations.len(), 7);
    assert_eq!(
        h.update(&capacity, json!({"product_principal_capacity":"0.06"}))
            .await
            .0,
        StatusCode::OK
    );
    for order in &reservations {
        assert_eq!(
            h.action(order, "approve", None).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        h.action(&reservations[0], "reject", None).await.0,
        StatusCode::OK
    );
    for order in &reservations[1..] {
        let app = h.app.clone();
        let token = h.admin.clone();
        let id = order["order"]["id"].as_u64().unwrap();
        tasks.spawn(async move {
            call(
                app,
                &token,
                "POST",
                &format!("/admin/api/v1/loan/orders/{id}/approve"),
                json!({}),
            )
            .await
        });
    }
    while let Some(result) = tasks.join_next().await {
        let (status, payload) = result.unwrap();
        assert_eq!(status, StatusCode::OK, "{payload}");
        assert_eq!(payload["order"]["status"], "disbursed");
    }
    let (_, current) = call(
        h.app.clone(),
        &h.admin,
        "GET",
        &format!("/admin/api/v1/loan/products/{}", capacity["id"]),
        Value::Null,
    )
    .await;
    assert_eq!(
        decimal(current["reserved_principal"].as_str().unwrap()),
        decimal("0")
    );
    assert_eq!(
        decimal(current["outstanding_principal"].as_str().unwrap()),
        decimal("0.06")
    );
    let credits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wallet_ledger WHERE asset_id = ? AND change_type = 'loan_disbursement'"
    ).bind(asset).fetch_one(&h.pool).await.unwrap();
    assert_eq!(credits, 6);
}

#[tokio::test]
async fn exposure_overdue_policy_defaults_and_rechecks_newly_due_debt() {
    let Some(h) = ExposureHarness::new().await else {
        return;
    };
    let product = h.product(h.asset_id, Value::Null, Value::Null).await;
    let (_, debt) = h.borrow(0, &product, "1", "debt").await;
    assert_eq!(h.action(&debt, "approve", None).await.0, StatusCode::OK);
    sqlx::query("UPDATE loan_orders SET due_at = DATE_SUB(CURRENT_TIMESTAMP(6), INTERVAL 1 SECOND) WHERE id = ?")
        .bind(debt["order"]["id"].as_u64().unwrap()).execute(&h.pool).await.unwrap();
    let (status, pending) = h.borrow(0, &product, "1", "default-allows").await;
    assert_eq!(status, StatusCode::OK, "{pending}");
    assert_eq!(
        h.update(&product, json!({"deny_borrowing_while_overdue":true}))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        h.borrow(0, &product, "1", "overdue-denied").await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        h.action(&pending, "approve", None).await.0,
        StatusCode::BAD_REQUEST
    );
    let (status, replay) = h.borrow(0, &product, "1", "default-allows").await;
    assert_eq!(status, StatusCode::OK, "{replay}");
    assert_eq!(replay["changed"], false);
    sqlx::query("UPDATE loan_orders SET status = 'overdue' WHERE id = ?")
        .bind(debt["order"]["id"].as_u64().unwrap())
        .execute(&h.pool)
        .await
        .unwrap();
    assert_eq!(
        h.borrow(0, &product, "1", "marked-overdue").await.0,
        StatusCode::BAD_REQUEST
    );
    let (repaid, approved) = tokio::join!(
        h.action(&debt, "repay", Some(0)),
        h.action(&pending, "approve", None)
    );
    assert_eq!(repaid.0, StatusCode::OK, "{}", repaid.1);
    assert!(
        matches!(approved.0, StatusCode::OK | StatusCode::BAD_REQUEST),
        "{}",
        approved.1
    );
    assert_eq!(h.action(&pending, "approve", None).await.0, StatusCode::OK);
    assert_eq!(
        h.borrow(0, &product, "1", "after-cure").await.0,
        StatusCode::OK
    );
}
