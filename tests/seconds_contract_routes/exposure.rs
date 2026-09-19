use super::*;
use chrono::Utc;
use serde_json::json;

async fn stage_review_fixture(pool: &MySqlPool, order_id: u64) {
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("UPDATE seconds_contract_orders SET expires_at=DATE_SUB(CURRENT_TIMESTAMP(6), INTERVAL 1 HOUR) WHERE id=?")
        .bind(order_id).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE seconds_contract_orders SET status='manual_review', settlement_failure_code='missing_settlement_snapshot', settlement_failed_at=CURRENT_TIMESTAMP(6), settlement_window_start=expires_at, settlement_window_end=DATE_ADD(expires_at,INTERVAL 5 SECOND), next_settlement_attempt_at=NULL WHERE id=?")
        .bind(order_id).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO seconds_contract_settlement_exceptions(order_id,failure_code,detected_at,window_start,window_end) SELECT id,settlement_failure_code,settlement_failed_at,settlement_window_start,settlement_window_end FROM seconds_contract_orders WHERE id=?")
        .bind(order_id).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
}

async fn request(
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
    let body =
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!(String::from_utf8_lossy(&bytes)));
    (status, body)
}

#[tokio::test]
async fn seconds_payout_capacity_serializes_opening_and_preserves_pending_liability() {
    let Some(pool) = mysql_pool().await else {
        return;
    };
    let Some(redis) = redis_manager().await else {
        return;
    };
    let admin = create_admin(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let user = create_user(&mut tx).await;
    let (base, base_symbol) = create_asset(&mut tx, "SB").await;
    let (asset, asset_symbol) = create_asset(&mut tx, "SQ").await;
    let symbol = format!("{base_symbol}-{asset_symbol}");
    let pair = create_pair(&mut tx, base, asset, &symbol).await;
    let product = seed_seconds_product(&mut tx, pair, asset).await;
    sqlx::query("UPDATE seconds_contract_products SET open_payout_capacity=63 WHERE id=?")
        .bind(product)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO wallet_accounts(user_id,asset_id,available) VALUES(?,?,1000)")
        .bind(user)
        .bind(asset)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    seed_ticker_at(&redis, &symbol, "100", Utc::now().timestamp_millis()).await;
    let settings = test_settings();
    let token = issue_token(&settings, format!("user:{user}"), TokenScope::User, 900).unwrap();
    let admin_token =
        issue_token(&settings, format!("admin:{admin}"), TokenScope::Admin, 900).unwrap();
    let state = state_with_mysql_and_redis(settings, pool.clone(), Some(redis));
    let app = user_routes().with_state(state.clone());
    let admin_app = admin_routes().with_state(state);
    let mut tasks = tokio::task::JoinSet::new();
    for i in 0..32 {
        let app = app.clone();
        let token = token.clone();
        tasks.spawn(async move {
            let key = format!("cap-{product}-{i}");
            let result = request(&app, &token, "POST", "/seconds-contracts/orders",
                json!({"product_id":product,"direction":"up","stake_amount":"5","idempotency_key":key})).await;
            (key, result)
        });
    }
    let mut accepted = Vec::new();
    while let Some(result) = tokio::time::timeout(Duration::from_secs(30), tasks.join_next())
        .await
        .unwrap()
    {
        let (key, (status, body)) = result.unwrap();
        if status.is_success() {
            accepted.push((key, body["order"].clone()));
        } else {
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        }
    }
    assert_eq!(accepted.len(), 7);
    let snapshots: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM seconds_order_refund_snapshots WHERE product_id=?",
    )
    .bind(product)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        snapshots, 0,
        "missing policy never grants legacy refund eligibility"
    );
    let path = format!("/seconds-contracts/products/{product}");
    let mut config = json!({"pair_id":pair,"stake_asset":asset,"duration_seconds":60,"payout_rate":"0.1",
        "min_stake":"5","max_stake":"100","status":"active","open_payout_capacity":"54","reason":"tighten budget"});
    let (status, row) = request(&admin_app, &admin_token, "PATCH", &path, config.clone()).await;
    assert_eq!(status, StatusCode::OK, "{row}");
    assert_eq!(
        decimal(row["open_payout_capacity"].as_str().unwrap()),
        decimal("54")
    );
    let (key, first) = &accepted[0];
    stage_review_fixture(&pool, first["id"].as_u64().unwrap()).await;
    let body =
        json!({"product_id":product,"direction":"up","stake_amount":"5","idempotency_key":key});
    let (status, replay) = request(&app, &token, "POST", "/seconds-contracts/orders", body).await;
    assert_eq!(status, StatusCode::OK, "{replay}");
    assert_eq!(replay["order"]["id"], first["id"]);
    let open = |key: &str| json!({"product_id":product,"direction":"up","stake_amount":"5","idempotency_key":key});
    let (status, denied) = request(
        &app,
        &token,
        "POST",
        "/seconds-contracts/orders",
        open("tightened"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{denied}");
    // Real existing settlement releases only final obligations; no settlement code is changed.
    for (_, row) in accepted.iter().skip(1).take(2) {
        let id = row["id"].as_u64().unwrap();
        make_order_due_with_event_price(&pool, id, "101").await;
        let (status, settled) = request(
            &admin_app,
            &admin_token,
            "POST",
            &format!("/seconds-contracts/orders/{id}/settle"),
            json!({"result":"auto","reason":"release settled liability"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{settled}");
    }
    let (status, opened) = request(
        &app,
        &token,
        "POST",
        "/seconds-contracts/orders",
        open("after-release"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{opened}");
    assert_eq!(
        decimal(opened["order"]["payout_rate"].as_str().unwrap()),
        decimal("0.1")
    );
    // Five old snapshots reserve 45; the new snapshot reserves 5.5. Another 5.5 exceeds 54.
    let (status, denied) = request(
        &app,
        &token,
        "POST",
        "/seconds-contracts/orders",
        open("snapshot-rates"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{denied}");
    config["stake_asset"] = json!(base);
    let (status, denied) = request(&admin_app, &admin_token, "PATCH", &path, config.clone()).await;
    assert_eq!(status, StatusCode::CONFLICT, "{denied}");
    config["stake_asset"] = json!(asset);
    config["open_payout_capacity"] = json!("56");
    let (status, row) = request(&admin_app, &admin_token, "PATCH", &path, config.clone()).await;
    assert_eq!(status, StatusCode::OK, "{row}");
    let policy_path = format!("{path}/refund-policy");
    let (status, row) = request(&admin_app, &admin_token, "PATCH", &policy_path,
        json!({"expected_version":0,"enabled":true,"wait_seconds":0,"reason":"prospective test policy"})).await;
    assert_eq!(status, StatusCode::OK, "{row}");
    let mut retries = tokio::task::JoinSet::new();
    for _ in 0..16 {
        let app = app.clone();
        let token = token.clone();
        let body = open("same-key-limited");
        retries.spawn(async move {
            request(&app, &token, "POST", "/seconds-contracts/orders", body).await
        });
    }
    let mut replayed_id = None;
    while let Some(result) = retries.join_next().await {
        let (status, row) = result.unwrap();
        assert_eq!(status, StatusCode::OK, "{row}");
        let id = row["order"]["id"].as_u64().unwrap();
        assert_eq!(*replayed_id.get_or_insert(id), id);
    }
    let snapshots: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM seconds_order_refund_snapshots WHERE product_id=?",
    )
    .bind(product)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        snapshots, 1,
        "only the newly inserted order receives one policy snapshot"
    );
    let old_snapshot: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM seconds_order_refund_snapshots WHERE order_id=?")
            .bind(first["id"].as_u64().unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        old_snapshot, 0,
        "enabling a policy cannot retrofit legacy orders"
    );
    let (status, row) = request(&admin_app, &admin_token, "PATCH", &policy_path,
        json!({"expected_version":1,"enabled":false,"wait_seconds":null,"reason":"close prospective policy"})).await;
    assert_eq!(status, StatusCode::OK, "{row}");
    let refund_order = replayed_id.unwrap();
    stage_review_fixture(&pool, refund_order).await;
    let (status, denied) = request(
        &app,
        &token,
        "POST",
        "/seconds-contracts/orders",
        open("before-refund"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{denied}");
    let refund_path = format!("/seconds-contracts/orders/{refund_order}/principal-refund");
    let refund_body = json!({"idempotency_key":format!("capacity-refund-{refund_order}"),"reason":"verified missing event evidence"});
    let (status, receipt) = request(
        &admin_app,
        &admin_token,
        "POST",
        &refund_path,
        refund_body.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(decimal(receipt["amount"].as_str().unwrap()), decimal("5"));
    let (status, replay) =
        request(&admin_app, &admin_token, "POST", &refund_path, refund_body).await;
    assert_eq!(status, StatusCode::OK, "{replay}");
    let (status, opened) = request(
        &app,
        &token,
        "POST",
        "/seconds-contracts/orders",
        open("after-refund"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{opened}");
    let status: String =
        sqlx::query_scalar("SELECT status FROM seconds_contract_orders WHERE id=?")
            .bind(refund_order)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "refunded");
    for value in [json!("0"), Value::Null] {
        config["open_payout_capacity"] = value.clone();
        let (status, row) = request(&admin_app, &admin_token, "PATCH", &path, config.clone()).await;
        assert_eq!(status, StatusCode::OK, "{row}");
        let (status, row) = request(
            &app,
            &token,
            "POST",
            "/seconds-contracts/orders",
            open(if value.is_null() { "inactive" } else { "zero" }),
        )
        .await;
        assert_eq!(
            status,
            if value.is_null() {
                StatusCode::OK
            } else {
                StatusCode::BAD_REQUEST
            },
            "{row}"
        );
    }
    for (value, expected_status) in [
        ("-1", StatusCode::BAD_REQUEST),
        ("0.0000000000000000001", StatusCode::UNPROCESSABLE_ENTITY),
        ("100000000000000000000", StatusCode::UNPROCESSABLE_ENTITY),
    ] {
        config["open_payout_capacity"] = json!(value);
        let (status, row) = request(&admin_app, &admin_token, "PATCH", &path, config.clone()).await;
        assert_eq!(status, expected_status, "{row}");
        if status == StatusCode::UNPROCESSABLE_ENTITY {
            let message = row.as_str().unwrap();
            assert!(message.contains("open_payout_capacity:"), "{message}");
            assert!(
                message.contains("DECIMAL(38,18) precision or range"),
                "{message}"
            );
        }
    }
    let stored: Option<BigDecimal> = sqlx::query_scalar(
        "SELECT open_payout_capacity FROM seconds_contract_products WHERE id = ?",
    )
    .bind(product)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(stored.is_none());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admin_audit_logs WHERE admin_id=? AND target_type='seconds_contract_product' AND target_id=? AND after_json LIKE '%open_payout_capacity%'")
        .bind(admin).bind(product).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 4);
    pool.close().await;
}
