use super::*;

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
    if status == StatusCode::UNPROCESSABLE_ENTITY {
        let body = axum::body::to_bytes(response.into_body(), 8192)
            .await
            .unwrap();
        return (
            status,
            Value::String(String::from_utf8(body.to_vec()).unwrap()),
        );
    }
    (status, body_json(response).await.unwrap())
}

#[tokio::test]
async fn earn_capacity_concurrent_reservations_replay_tightening_and_release() {
    let Some(pool) = mysql_pool().await else {
        return;
    };
    let mut tx = pool.begin().await.unwrap();
    let user = create_user(&mut tx).await;
    let (asset, _) = create_asset(&mut tx, "EC").await;
    let product = seed_earn_product(&mut tx, asset).await;
    sqlx::query("UPDATE earn_products SET min_subscribe = 1, apr_rate = 0.1, term_days = 365, principal_capacity = 7, liability_capacity = 7.7 WHERE id = ?")
        .bind(product).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO wallet_accounts(user_id,asset_id,available) VALUES(?,?,1000)")
        .bind(user)
        .bind(asset)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let settings = test_settings();
    let token = issue_token(&settings, format!("user:{user}"), TokenScope::User, 900).unwrap();
    let app = user_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let mut tasks = tokio::task::JoinSet::new();
    for i in 0..32 {
        let app = app.clone();
        let token = token.clone();
        tasks.spawn(async move {
            let key = format!("cap-{product}-{i}");
            let result = request(
                &app,
                &token,
                "POST",
                "/earn/subscriptions",
                json!({"product_id": product, "amount": "1", "idempotency_key": key}),
            )
            .await;
            (key, result)
        });
    }
    let mut accepted = Vec::new();
    while let Some(result) = timeout(Duration::from_secs(30), tasks.join_next())
        .await
        .unwrap()
    {
        let (key, (status, body)) = result.unwrap();
        if status.is_success() {
            accepted.push((key, body["subscription"].clone()));
        } else {
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        }
    }
    assert_eq!(accepted.len(), 7);
    let (count, amount): (i64, BigDecimal) =
        sqlx::query_as("SELECT COUNT(*), SUM(amount) FROM earn_subscriptions WHERE product_id = ?")
            .bind(product)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 7);
    assert_eq!(amount, decimal("7"));
    // A tighter cap never blocks replay, but does block fresh admission.
    sqlx::query("UPDATE earn_products SET principal_capacity = 6 WHERE id = ?")
        .bind(product)
        .execute(&pool)
        .await
        .unwrap();
    let (key, first) = &accepted[0];
    let (status, replay) = request(
        &app,
        &token,
        "POST",
        "/earn/subscriptions",
        json!({"product_id":product,"amount":"1","idempotency_key":key}),
    )
    .await;
    assert!(status.is_success(), "{replay}");
    assert_eq!(replay["subscription"]["id"], first["id"]);
    let (status, body) = request(
        &app,
        &token,
        "POST",
        "/earn/subscriptions",
        json!({"product_id":product,"amount":"1","idempotency_key":"tightened"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    for (_, row) in accepted.iter().take(2) {
        let (status, body) = request(
            &app,
            &token,
            "POST",
            &format!("/earn/subscriptions/{}/redeem", row["id"]),
            json!({}),
        )
        .await;
        assert!(status.is_success(), "{body}");
    }
    let (status, body) = request(
        &app,
        &token,
        "POST",
        "/earn/subscriptions",
        json!({"product_id":product,"amount":"1","idempotency_key":"after-release"}),
    )
    .await;
    assert!(status.is_success(), "{body}");
    // Gross liability, not merely principal, is the independently enforceable boundary.
    sqlx::query(
        "UPDATE earn_products SET principal_capacity = NULL, liability_capacity = 6.6 WHERE id = ?",
    )
    .bind(product)
    .execute(&pool)
    .await
    .unwrap();
    let (status, body) = request(
        &app,
        &token,
        "POST",
        "/earn/subscriptions",
        json!({"product_id":product,"amount":"1","idempotency_key":"liability-full"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    sqlx::query("UPDATE earn_products SET liability_capacity = NULL WHERE id = ?")
        .bind(product)
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = request(
        &app,
        &token,
        "POST",
        "/earn/subscriptions",
        json!({"product_id":product,"amount":"1","idempotency_key":"disabled-cap"}),
    )
    .await;
    assert!(status.is_success(), "{body}");
    pool.close().await;
}

#[tokio::test]
async fn earn_capacity_admin_null_zero_precision_and_audit() {
    let Some(pool) = mysql_pool().await else {
        return;
    };
    let admin = create_admin(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let (asset, _) = create_asset(&mut tx, "EA").await;
    sqlx::query("UPDATE assets SET precision_scale = 2 WHERE id = ?")
        .bind(asset)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let settings = test_settings();
    let token = issue_token(&settings, format!("admin:{admin}"), TokenScope::Admin, 900).unwrap();
    let app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let mut body = json!({"asset_id":asset,"name":"capacity","term_days":30,"apr_rate":"0.1",
        "min_subscribe":"1","status":"active","reason":"capacity test",
        "principal_capacity":"0","liability_capacity":"9007199254740993.12"});
    let (status, row) = request(&app, &token, "POST", "/earn/products", body.clone()).await;
    assert!(status.is_success(), "{row}");
    assert_eq!(
        decimal(row["principal_capacity"].as_str().unwrap()),
        decimal("0")
    );
    assert_eq!(
        decimal(row["liability_capacity"].as_str().unwrap()),
        decimal("9007199254740993.12")
    );
    let path = format!("/earn/products/{}", row["id"]);
    for invalid in ["-1", "1.001"] {
        body["principal_capacity"] = json!(invalid);
        let (status, error) = request(&app, &token, "PATCH", &path, body.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    }
    body["principal_capacity"] = json!("100000000000000000000");
    let (status, error) = request(&app, &token, "PATCH", &path, body.clone()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{error}");
    let message = error.as_str().unwrap();
    assert!(message.contains("principal_capacity:"), "{message}");
    assert!(
        message.contains("DECIMAL(38,18) precision or range"),
        "{message}"
    );
    let stored: BigDecimal =
        sqlx::query_scalar("SELECT principal_capacity FROM earn_products WHERE id = ?")
            .bind(row["id"].as_u64().unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored, decimal("0"));
    body["principal_capacity"] = Value::Null;
    body["liability_capacity"] = Value::Null;
    let (status, after) = request(&app, &token, "PATCH", &path, body).await;
    assert!(status.is_success(), "{after}");
    assert!(after["principal_capacity"].is_null() && after["liability_capacity"].is_null());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admin_audit_logs WHERE admin_id = ? AND target_id = ? AND after_json LIKE '%principal_capacity%'")
        .bind(admin).bind(row["id"].as_u64().unwrap()).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 2);
    pool.close().await;
}
