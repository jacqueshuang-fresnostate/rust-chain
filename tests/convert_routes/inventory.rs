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
    let bytes = axum::body::to_bytes(response.into_body(), 65_536)
        .await
        .unwrap();
    let body =
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!(String::from_utf8_lossy(&bytes)));
    (status, body)
}

#[tokio::test]
async fn configured_inventory_admin_and_authoritative_confirmation_are_atomic() {
    let _guard = TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool().await else {
        return;
    };
    let user = create_user(&pool).await;
    let from = create_asset(&pool, "CF").await;
    let to = create_asset(&pool, "CT").await;
    let pair = seed_convert_pair(&pool, from, to).await;
    let first_quote = seed_convert_quote(&pool, user, pair, from, to).await;
    let role = sqlx::query("INSERT INTO admin_roles(name,permissions) VALUES(?,JSON_ARRAY('*'))")
        .bind(format!("inv-{}", Uuid::now_v7().simple()))
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let admin =
        sqlx::query("INSERT INTO admin_users(username,password_hash,role_id) VALUES(?,'test',?)")
            .bind(format!("inv-{}", Uuid::now_v7().simple()))
            .bind(role)
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_id();
    let settings = test_settings();
    let admin_token =
        issue_token(&settings, format!("admin:{admin}"), TokenScope::Admin, 900).unwrap();
    let token = issue_token(&settings, format!("user:{user}"), TokenScope::User, 900).unwrap();
    let state = AppState::new(settings).with_mysql(pool.clone());
    let admin_app = exchange_api::build_router(state.clone());
    let app = user_routes().with_state(state);
    let path = format!("/admin/api/v1/convert/pairs/{pair}");
    let funding = json!({"inventory":{"revision":0,"enabled":true,"funded_amount":"140","funding_reference":"isolated-test-proof"},"reason":"set explicit funding"});
    let (status, configured) = request(&admin_app, &admin_token, "PATCH", &path, funding).await;
    assert_eq!(status, StatusCode::OK, "{configured}");
    assert_eq!(configured["inventory_revision"], 1);
    assert_eq!(
        decimal(configured["inventory_funded_amount"].as_str().unwrap()),
        decimal("140")
    );
    assert_eq!(
        decimal(configured["inventory_consumed_amount"].as_str().unwrap()),
        decimal("0")
    );
    // Changing pair configuration invalidates earlier quotes. Produce all candidates after configuration.
    let quotes = {
        let mut quotes = Vec::new();
        for _ in 0..24 {
            quotes.push(seed_convert_quote(&pool, user, pair, from, to).await);
        }
        quotes
    };
    let (status, error) = request(
        &app,
        &token,
        "POST",
        "/convert/confirm",
        json!({"quote_id":quotes[0]}),
    )
    .await;
    assert!(!status.is_success(), "{error}");
    let consumed: BigDecimal = sqlx::query_scalar(
        "SELECT consumed_amount FROM convert_inventory_accounts WHERE pair_id=?",
    )
    .bind(pair)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        consumed,
        decimal("0"),
        "wallet failure must roll back inventory"
    );
    for (asset, amount) in [(from, "1000"), (to, "0")] {
        sqlx::query("INSERT INTO wallet_accounts(user_id,asset_id,available) VALUES(?,?,?) ON DUPLICATE KEY UPDATE available=VALUES(available)")
            .bind(user).bind(asset).bind(decimal(amount)).execute(&pool).await.unwrap();
    }
    let mut tasks = tokio::task::JoinSet::new();
    for quote in &quotes {
        let app = app.clone();
        let token = token.clone();
        let quote = quote.clone();
        tasks.spawn(async move {
            let response = request(
                &app,
                &token,
                "POST",
                "/convert/confirm",
                json!({"quote_id":quote}),
            )
            .await;
            (quote, response)
        });
    }
    let mut accepted = Vec::new();
    while let Some(result) = tokio::time::timeout(Duration::from_secs(30), tasks.join_next())
        .await
        .unwrap()
    {
        let (quote, (status, body)) = result.unwrap();
        if status.is_success() {
            accepted.push(quote);
        } else {
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        }
    }
    assert_eq!(accepted.len(), 7);
    let (consumed, count): (BigDecimal, i64) = sqlx::query_as(
        "SELECT consumed_amount,(SELECT COUNT(*) FROM convert_inventory_allocations WHERE pair_id=?) FROM convert_inventory_accounts WHERE pair_id=?",
    ).bind(pair).bind(pair).fetch_one(&pool).await.unwrap();
    assert_eq!(consumed, decimal("140"));
    assert_eq!(count, 7);
    let (status, replay) = request(
        &app,
        &token,
        "POST",
        "/convert/confirm",
        json!({"quote_id":accepted[0]}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{replay}");
    let available: BigDecimal =
        sqlx::query_scalar("SELECT available FROM wallet_accounts WHERE user_id=? AND asset_id=?")
            .bind(user)
            .bind(to)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(available, decimal("140"));
    let (status, reverse_quote) = request(
        &app,
        &token,
        "POST",
        "/convert/quote",
        json!({
            "from_asset_id":to,"to_asset_id":from,"from_amount":"2"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reverse_quote}");
    let (status, refused) = request(
        &app,
        &token,
        "POST",
        "/convert/confirm",
        json!({"quote_id":reverse_quote["quote_id"]}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    assert!(
        refused["message"]
            .as_str()
            .unwrap()
            .contains("reverse confirmation is disabled")
    );
    let (status, error) = request(&admin_app, &admin_token, "PATCH", &path, json!({
        "inventory":{"revision":1,"enabled":true,"funded_amount":"139","funding_reference":"too low"},"reason":"reject decrement"
    })).await;
    assert_eq!(status, StatusCode::CONFLICT, "{error}");
    let (status, disabled) = request(&admin_app, &admin_token, "PATCH", &path, json!({
        "inventory":{"revision":1,"enabled":false,"funded_amount":"140","funding_reference":"disable proof"},"reason":"disable without erasing"
    })).await;
    assert_eq!(status, StatusCode::OK, "{disabled}");
    assert_eq!(disabled["inventory_revision"], 2);
    assert_eq!(
        decimal(disabled["inventory_consumed_amount"].as_str().unwrap()),
        decimal("140")
    );
    let (status, error) = request(&admin_app, &admin_token, "PATCH", &path, json!({
        "inventory":{"revision":1,"enabled":true,"funded_amount":"160","funding_reference":"stale"},"reason":"stale retry"
    })).await;
    assert_eq!(status, StatusCode::CONFLICT, "{error}");
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM convert_inventory_funding_audits WHERE pair_id=?")
            .bind(pair)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 2);
    // The original pre-configuration quote remains unconsumed; no rejected request created a payout.
    let status: String = sqlx::query_scalar("SELECT status FROM convert_quotes WHERE quote_id=?")
        .bind(first_quote)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "quoted");
    pool.close().await;
}
