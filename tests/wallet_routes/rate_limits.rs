use super::*;
use exchange_api::modules::risk::{
    RiskScope, StoredRiskRule, infrastructure::user_request_count_key, resolve_risk_policy,
};
use redis::AsyncCommands;

async fn submit(app: &Router, token: &str, payload: &Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallet/withdrawals")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    (response.status(), body_json(response).await.unwrap())
}

#[tokio::test]
async fn withdrawal_rate_limit_blocks_without_counter_and_preserves_exact_replay()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let Ok(redis_url) = std::env::var("REDIS_URL") else {
        eprintln!("skipping withdrawal rate-limit integration test without REDIS_URL");
        return Ok(());
    };
    let redis = redis::Client::open(redis_url)?
        .get_connection_manager()
        .await?;
    let mut connection = redis.clone();
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (asset_id, _) = create_asset(&pool).await;
    let asset_symbol: String = sqlx::query_scalar("SELECT symbol FROM assets WHERE id = ?")
        .bind(asset_id)
        .fetch_one(&pool)
        .await?;
    allow_withdrawal_network(&pool, "tron", &asset_symbol).await?;
    seed_wallet(&pool, user_id, asset_id, &Uuid::now_v7().to_string()).await;
    seed_fund_password(&pool, user_id, "123456").await;
    let config = json!({
        "operations": ["wallet.withdrawal.create"],
        "max_requests": 1,
        "window_seconds": 3600
    });
    let rule_id = sqlx::query(
        "INSERT INTO risk_rules (rule_type, target_type, target_id, config_json, enabled) VALUES ('rate_limit', 'user', ?, ?, TRUE)",
    )
    .bind(user_id.to_string())
    .bind(sqlx::types::Json(config.clone()))
    .execute(&pool)
    .await?
    .last_insert_id();
    let policy = resolve_risk_policy(
        &[StoredRiskRule {
            target_type: "user".into(),
            target_id: Some(user_id.to_string()),
            config,
        }],
        "wallet.withdrawal.create",
        &[RiskScope::new("user", user_id.to_string())],
    );
    let counter_key = user_request_count_key(
        "wallet.withdrawal.create",
        &policy.rate_limit_scope,
        user_id,
    );
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900)?;
    let without_counter =
        routes().with_state(AppState::new(settings.clone()).with_mysql(pool.clone()));
    let quote =
        create_withdrawal_quote(&without_counter, &token, &asset_symbol, "trc20", "1").await?;
    let mut payload = json!({
        "quote_id": quote["quote_id"],
        "asset_symbol": asset_symbol,
        "network": "trc20",
        "address": "TRateLimitFixture",
        "amount": "1",
        "fee": quote["fee"],
        "idempotency_key": Uuid::now_v7().to_string(),
        "fund_password": "123456"
    });
    let (status, rejected) = submit(&without_counter, &token, &payload).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{rejected}");
    assert_eq!(rejected["code"], "risk_rate_limit_unavailable");
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM wallet_withdrawal_requests WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(count, 0);
    let before: (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(before, (decimal("12.5"), decimal("1.5")));

    let app = routes().with_state(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_redis(redis),
    );
    let (status, first) = submit(&app, &token, &payload).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let (status, replay) = submit(&app, &token, &payload).await;
    assert_eq!(status, StatusCode::OK, "{replay}");
    assert_eq!(first, replay);
    let counter: u32 = connection.get(&counter_key).await?;
    assert_eq!(counter, 1, "exact replay must not consume another attempt");
    let after_first: (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    let second_quote = create_withdrawal_quote(&app, &token, &asset_symbol, "trc20", "1").await?;
    payload["quote_id"] = second_quote["quote_id"].clone();
    payload["fee"] = second_quote["fee"].clone();
    payload["idempotency_key"] = json!(Uuid::now_v7().to_string());
    let (status, limited) = submit(&app, &token, &payload).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{limited}");
    assert_eq!(limited["code"], "risk_rate_limit");

    // 有 Redis 句柄但计数命令失败时也不能把未知计数当零。
    let _: () = connection.set(&counter_key, "invalid-counter").await?;
    let (status, unavailable) = submit(&app, &token, &payload).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{unavailable}");
    assert_eq!(unavailable["code"], "risk_rate_limit_unavailable");
    let final_balances: (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(after_first, final_balances);
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM wallet_withdrawal_requests WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(count, 1);

    let _: u64 = connection.del(counter_key).await?;
    sqlx::query("DELETE FROM risk_events WHERE user_id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM risk_rules WHERE id = ?")
        .bind(rule_id)
        .execute(&pool)
        .await?;
    cleanup_wallet_route_fixture(&pool, user_id, asset_id).await?;
    Ok(())
}
