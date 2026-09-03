use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use bigdecimal::BigDecimal;
use exchange_api::{
    build_router,
    config::Settings,
    modules::auth::{TokenScope, hash_password, issue_token},
    state::AppState,
};
use secrecy::SecretString;
use serde_json::{Value, json};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::{error::Error, str::FromStr};
use tower::ServiceExt;
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
struct AgentFixture {
    agent_user_id: u64,
    agent_id: u64,
    admin_user_id: u64,
}

fn test_settings() -> Settings {
    Settings {
        app_env: "test".to_owned(),
        app_host: "127.0.0.1".parse().unwrap(),
        app_port: 0,
        database_url: SecretString::new("mysql://test:test@localhost/test".to_owned()),
        mongodb_uri: SecretString::new("mongodb://localhost:27017".to_owned()),
        mongodb_database: "exchange_test".to_owned(),
        redis_url: SecretString::new("redis://localhost:6379".to_owned()),
        rabbitmq_url: SecretString::new("amqp://guest:guest@localhost:5672/%2f".to_owned()),
        jwt_secret: SecretString::new("test-secret".to_owned()),
        credential_encryption_key: Some(SecretString::new(
            "0123456789abcdef0123456789abcdef".to_owned(),
        )),
        jwt_access_ttl_seconds: 900,
        jwt_refresh_ttl_seconds: 2_592_000,
        bitget_rest_base_url: "https://bitget.test".to_owned(),
        bitget_ws_url: "wss://bitget.test/ws".to_owned(),
        htx_rest_base_url: "https://htx.test".to_owned(),
        htx_ws_url: "wss://htx.test/ws".to_owned(),
        coinbase_rest_base_url: "https://coinbase.test".to_owned(),
        coinbase_ws_url: "wss://coinbase.test/ws".to_owned(),
        market_feed_symbols: Vec::new(),
        market_feed_intervals: Vec::new(),
        market_feed_providers: Vec::new(),
        market_feed_reconnect_seconds: 5,
        market_feed_rest_fallback_timeout_seconds: 3,
        event_inbox_retry_scan_seconds: 10,
        event_outbox_publisher_enabled: true,
        event_outbox_publisher_interval_seconds: 5,
        unlock_scanner_enabled: true,
        unlock_scanner_interval_seconds: 10,
        unlock_scanner_batch_limit: 100,
        kline_recovery_enabled: true,
        kline_recovery_interval_seconds: 30,
        kline_recovery_batch_limit: 100,
        seconds_contract_settlement_enabled: true,
        seconds_contract_settlement_interval_seconds: 5,
        seconds_contract_settlement_batch_limit: 100,
        earn_auto_redemption_enabled: true,
        earn_auto_redemption_interval_seconds: 60,
        earn_auto_redemption_batch_limit: 100,
        margin_liquidation_enabled: true,
        margin_liquidation_interval_seconds: 5,
        margin_liquidation_batch_limit: 100,
        margin_interest_enabled: true,
        margin_interest_interval_seconds: 60,
        margin_interest_batch_limit: 100,
        agent_commission_auto_settle_enabled: false,
        agent_commission_auto_settle_interval_seconds: 60,
        agent_commission_auto_settle_min_age_seconds: 3600,
        agent_commission_auto_settle_batch_limit: 100,
    }
}

async fn mysql_pool() -> Option<MySqlPool> {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!("skipping MySQL agent route test because DATABASE_URL is not set");
            return None;
        }
    };

    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    Some(pool)
}

async fn create_user(pool: &MySqlPool, label: &str) -> u64 {
    let email = format!(
        "agent-route-{label}-{}@example.test",
        Uuid::now_v7().simple()
    );
    sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(email)
        .bind("not-a-real-hash")
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id()
}

async fn create_user_with_password(pool: &MySqlPool, label: &str, password: &str) -> (u64, String) {
    let email = format!(
        "agent-route-{label}-{}@example.test",
        Uuid::now_v7().simple()
    );
    let user_id = sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(&email)
        .bind(hash_password(password).unwrap())
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id();

    (user_id, email)
}

async fn create_admin_with_password(
    pool: &MySqlPool,
    label: &str,
    password: &str,
) -> (u64, u64, String) {
    let role_name = format!("agent-route-role-{label}-{}", Uuid::now_v7().simple());
    let role_id =
        sqlx::query("INSERT INTO admin_roles (name, permissions) VALUES (?, JSON_ARRAY('*'))")
            .bind(role_name)
            .execute(pool)
            .await
            .unwrap()
            .last_insert_id();
    let username = format!("agent-route-admin-{label}-{}", Uuid::now_v7().simple());
    let admin_id =
        sqlx::query("INSERT INTO admin_users (username, password_hash, role_id) VALUES (?, ?, ?)")
            .bind(&username)
            .bind(hash_password(password).unwrap())
            .bind(role_id)
            .execute(pool)
            .await
            .unwrap()
            .last_insert_id();

    (role_id, admin_id, username)
}

async fn create_agent(pool: &MySqlPool, label: &str) -> AgentFixture {
    create_agent_with_password(pool, label, "not-a-real-password").await
}

async fn create_agent_with_password(pool: &MySqlPool, label: &str, password: &str) -> AgentFixture {
    let agent_user_id = create_user(pool, &format!("agent-owner-{label}")).await;
    let agent_code = format!("agent-{}-{}", label, Uuid::now_v7().simple());
    let agent_id = sqlx::query("INSERT INTO agents (user_id, agent_code, path) VALUES (?, ?, '')")
        .bind(agent_user_id)
        .bind(agent_code)
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id();
    sqlx::query("UPDATE agents SET root_agent_id = ?, path = ? WHERE id = ?")
        .bind(agent_id)
        .bind(format!("/agent:{agent_id}"))
        .bind(agent_id)
        .execute(pool)
        .await
        .unwrap();
    let username = format!("agent-admin-{}-{}", label, Uuid::now_v7().simple());
    let admin_user_id = sqlx::query(
        "INSERT INTO agent_admin_users (agent_id, username, password_hash) VALUES (?, ?, ?)",
    )
    .bind(agent_id)
    .bind(username)
    .bind(hash_password(password).unwrap())
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id();

    AgentFixture {
        agent_user_id,
        agent_id,
        admin_user_id,
    }
}

async fn create_child_agent(pool: &MySqlPool, parent: AgentFixture, label: &str) -> AgentFixture {
    let (root_agent_id, parent_level, parent_path): (u64, i32, String) =
        sqlx::query_as("SELECT root_agent_id, level, path FROM agents WHERE id = ? LIMIT 1")
            .bind(parent.agent_id)
            .fetch_one(pool)
            .await
            .unwrap();
    let agent_user_id = create_user(pool, &format!("agent-owner-{label}")).await;
    let agent_code = format!("agent-{}-{}", label, Uuid::now_v7().simple());
    let agent_id = sqlx::query(
        r#"INSERT INTO agents
              (user_id, parent_agent_id, root_agent_id, agent_code, level, path)
           VALUES (?, ?, ?, ?, ?, '')"#,
    )
    .bind(agent_user_id)
    .bind(parent.agent_id)
    .bind(root_agent_id)
    .bind(agent_code)
    .bind(parent_level + 1)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id();
    sqlx::query("UPDATE agents SET path = ? WHERE id = ?")
        .bind(format!("{parent_path}/agent:{agent_id}"))
        .bind(agent_id)
        .execute(pool)
        .await
        .unwrap();
    let admin_user_id = sqlx::query(
        "INSERT INTO agent_admin_users (agent_id, username, password_hash) VALUES (?, ?, ?)",
    )
    .bind(agent_id)
    .bind(format!("agent-admin-{}-{}", label, Uuid::now_v7().simple()))
    .bind(hash_password("not-a-real-password").unwrap())
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id();

    AgentFixture {
        agent_user_id,
        agent_id,
        admin_user_id,
    }
}

async fn refer_user_to_agent(pool: &MySqlPool, user_id: u64, agent_id: u64, depth: u32) {
    refer_user_with_inviter(pool, user_id, agent_id, agent_id, "agent", depth).await;
}

async fn refer_user_with_inviter(
    pool: &MySqlPool,
    user_id: u64,
    root_agent_id: u64,
    inviter_id: u64,
    inviter_type: &str,
    depth: u32,
) {
    sqlx::query(
        r#"INSERT INTO user_referrals
           (user_id, direct_inviter_id, direct_inviter_type, root_agent_id, depth, path)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(inviter_id)
    .bind(inviter_type)
    .bind(root_agent_id)
    .bind(depth)
    .bind(format!("/{root_agent_id}/{inviter_id}/{user_id}"))
    .execute(pool)
    .await
    .unwrap();
}

async fn create_unassigned_referral(pool: &MySqlPool, user_id: u64) {
    sqlx::query(
        r#"INSERT INTO user_referrals
           (user_id, direct_inviter_id, direct_inviter_type, root_agent_id, depth, path)
           VALUES (?, NULL, NULL, NULL, 0, ?)"#,
    )
    .bind(user_id)
    .bind(format!("/{user_id}"))
    .execute(pool)
    .await
    .unwrap();
}

async fn response_json(response: axum::response::Response) -> Result<Value, Box<dyn Error>> {
    let body = axum::body::to_bytes(response.into_body(), 8192).await?;
    Ok(serde_json::from_slice(&body)?)
}

async fn agent_get_json(
    app: axum::Router,
    token: &str,
    uri: impl AsRef<str>,
) -> Result<(StatusCode, Value), Box<dyn Error>> {
    let response = app
        .oneshot(
            Request::builder()
                .uri(uri.as_ref())
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    let status = response.status();
    let payload = response_json(response).await?;
    Ok((status, payload))
}

async fn cleanup_agent_admin_username(pool: &MySqlPool, username: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM agent_admin_users WHERE username = ?")
        .bind(username)
        .execute(pool)
        .await?;
    Ok(())
}

async fn cleanup_agent_fixture(
    pool: &MySqlPool,
    agents: &[AgentFixture],
    team_user_ids: &[u64],
) -> Result<(), sqlx::Error> {
    for agent in agents {
        sqlx::query("DELETE FROM invite_codes WHERE owner_type = 'agent' AND owner_id = ?")
            .bind(agent.agent_id)
            .execute(pool)
            .await?;
    }
    for user_id in team_user_ids {
        sqlx::query("DELETE FROM user_referrals WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;
    }
    for agent in agents {
        sqlx::query("DELETE FROM agent_admin_users WHERE id = ?")
            .bind(agent.admin_user_id)
            .execute(pool)
            .await?;
    }
    for agent in agents {
        sqlx::query("DELETE FROM agents WHERE id = ?")
            .bind(agent.agent_id)
            .execute(pool)
            .await?;
    }
    for user_id in team_user_ids {
        sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;
    }
    for agent in agents {
        sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(agent.agent_user_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

async fn create_invite_code(pool: &MySqlPool, agent_id: u64, status: &str) -> u64 {
    let code = format!("invite-{}", Uuid::now_v7().simple());
    sqlx::query(
        r#"INSERT INTO invite_codes (owner_type, owner_id, code, usage_limit, status)
           VALUES ('agent', ?, ?, 10, ?)"#,
    )
    .bind(agent_id)
    .bind(code)
    .bind(status)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id()
}

struct CommissionSeed<'a> {
    agent_id: u64,
    user_id: u64,
    source_type: &'a str,
    source_id: &'a str,
    source_amount: &'a str,
    commission_amount: &'a str,
    status: &'a str,
}

async fn create_commission_record(
    pool: &MySqlPool,
    agent_id: u64,
    user_id: u64,
    source_type: &str,
    source_amount: &str,
    commission_amount: &str,
    status: &str,
) -> u64 {
    let source_id = format!("agent-seeded-{}", Uuid::now_v7());
    create_commission_record_with_source_id(
        pool,
        CommissionSeed {
            agent_id,
            user_id,
            source_type,
            source_id: &source_id,
            source_amount,
            commission_amount,
            status,
        },
    )
    .await
}

async fn create_commission_record_with_source_id(
    pool: &MySqlPool,
    seed: CommissionSeed<'_>,
) -> u64 {
    sqlx::query(
        r#"INSERT INTO agent_commission_records
           (agent_id, user_id, source_type, source_id, source_amount, commission_amount, status)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(seed.agent_id)
    .bind(seed.user_id)
    .bind(seed.source_type)
    .bind(seed.source_id)
    .bind(BigDecimal::from_str(seed.source_amount).unwrap())
    .bind(BigDecimal::from_str(seed.commission_amount).unwrap())
    .bind(seed.status)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id()
}

async fn create_commission_payout_ledger(
    pool: &MySqlPool,
    user_id: u64,
    asset_id: u64,
    commission_id: u64,
    amount: &str,
    balance_after: &str,
) -> u64 {
    let amount = BigDecimal::from_str(amount).unwrap();
    let balance_after = BigDecimal::from_str(balance_after).unwrap();
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, 'agent_commission_payout', ?, 'available', ?, ?, 0, 0,
                   'agent_commission', ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(amount)
    .bind(balance_after.clone())
    .bind(balance_after)
    .bind(commission_id.to_string())
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id()
}

async fn create_asset(pool: &MySqlPool, label: &str) -> u64 {
    let symbol =
        format!("{}{}", label, &Uuid::now_v7().simple().to_string()[..8]).to_ascii_uppercase();
    sqlx::query("INSERT INTO assets (symbol, name, precision_scale) VALUES (?, ?, 18)")
        .bind(&symbol)
        .bind(&symbol)
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id()
}

async fn create_convert_pair(pool: &MySqlPool, from_asset: u64, to_asset: u64) -> u64 {
    sqlx::query(
        r#"INSERT INTO convert_pairs
           (from_asset, to_asset, pricing_mode, spread_rate, min_amount, enabled)
           VALUES (?, ?, 'fixed', 0, 1, TRUE)"#,
    )
    .bind(from_asset)
    .bind(to_asset)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id()
}

async fn create_convert_order(
    pool: &MySqlPool,
    pair: (u64, u64, u64),
    user_id: u64,
    amounts: (&str, &str),
    status: &str,
) -> String {
    let quote_id = Uuid::now_v7().to_string();
    sqlx::query(
        r#"INSERT INTO convert_orders
           (quote_id, convert_pair_id, user_id, from_asset, to_asset, from_amount, to_amount, rate, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, 2, ?)"#,
    )
    .bind(&quote_id)
    .bind(pair.0)
    .bind(user_id)
    .bind(pair.1)
    .bind(pair.2)
    .bind(BigDecimal::from_str(amounts.0).unwrap())
    .bind(BigDecimal::from_str(amounts.1).unwrap())
    .bind(status)
    .execute(pool)
    .await
    .unwrap();
    quote_id
}

#[tokio::test]
async fn agent_register_route_rejects_public_self_service_accounts() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let agent = create_agent(&pool, "register-disabled").await;
    let username = format!("agent-self-register-{}", Uuid::now_v7().simple());
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/agent/api/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "username": username,
                        "password": "agent-password-1",
                        "agent_id": agent.agent_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let payload = response_json(response).await?;
    assert_eq!(status, StatusCode::FORBIDDEN, "payload: {payload}");
    assert_eq!(payload["code"], "FORBIDDEN");
    let created_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM agent_admin_users WHERE username = ?")
            .bind(&username)
            .fetch_one(&pool)
            .await?;
    assert_eq!(created_count, 0);

    cleanup_agent_admin_username(&pool, &username).await?;
    cleanup_agent_fixture(&pool, &[agent], &[]).await?;
    Ok(())
}

#[tokio::test]
async fn agent_login_route_rejects_inactive_parent_agent() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let password = "agent-login-password-1";
    let agent = create_agent_with_password(&pool, "inactive-login", password).await;
    sqlx::query("UPDATE agents SET status = 'suspended' WHERE id = ?")
        .bind(agent.agent_id)
        .execute(&pool)
        .await?;
    let username: String =
        sqlx::query_scalar("SELECT username FROM agent_admin_users WHERE id = ? LIMIT 1")
            .bind(agent.admin_user_id)
            .fetch_one(&pool)
            .await?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/agent/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "username": username, "password": password }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    cleanup_agent_fixture(&pool, &[agent], &[]).await?;
    Ok(())
}

#[tokio::test]
async fn agent_login_route_issues_agent_tokens_and_records_last_login() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let password = "agent-login-password-2";
    let agent = create_agent_with_password(&pool, "active-login", password).await;
    let username: String =
        sqlx::query_scalar("SELECT username FROM agent_admin_users WHERE id = ? LIMIT 1")
            .bind(agent.admin_user_id)
            .fetch_one(&pool)
            .await?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/agent/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "username": username, "password": password }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let payload = response_json(response).await?;
    assert_eq!(status, StatusCode::OK, "payload: {payload}");
    assert_eq!(payload["scope"], "agent");
    assert_eq!(payload["token_type"], "Bearer");
    assert!(
        payload["access_token"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert!(
        payload["refresh_token"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );

    let last_login_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT last_login_at FROM agent_admin_users WHERE id = ? LIMIT 1")
            .bind(agent.admin_user_id)
            .fetch_one(&pool)
            .await?;
    assert!(last_login_at.is_some());

    sqlx::query("DELETE FROM refresh_tokens WHERE actor_type = 'agent' AND actor_id = ?")
        .bind(agent.admin_user_id)
        .execute(&pool)
        .await?;
    cleanup_agent_fixture(&pool, &[agent], &[]).await?;
    Ok(())
}

#[tokio::test]
async fn agent_refresh_routes_enforce_scope_and_active_status() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let agent_password = "agent-refresh-password-1";
    let admin_password = "admin-refresh-password-1";
    let user_password = "user-refresh-password-1";
    let agent = create_agent_with_password(&pool, "refresh", agent_password).await;
    let username: String =
        sqlx::query_scalar("SELECT username FROM agent_admin_users WHERE id = ? LIMIT 1")
            .bind(agent.admin_user_id)
            .fetch_one(&pool)
            .await?;
    let (role_id, admin_id, admin_username) =
        create_admin_with_password(&pool, "refresh", admin_password).await;
    let (user_id, user_email) = create_user_with_password(&pool, "refresh", user_password).await;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let agent_login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/agent/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "username": username, "password": agent_password }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let agent_tokens = response_json(agent_login).await?;
    let agent_refresh_token = agent_tokens["refresh_token"].as_str().unwrap().to_owned();

    let admin_login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "username": admin_username, "password": admin_password }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let admin_tokens = response_json(admin_login).await?;
    let admin_refresh_token = admin_tokens["refresh_token"].as_str().unwrap().to_owned();

    let user_login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "email": user_email, "password": user_password }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let user_tokens = response_json(user_login).await?;
    let user_refresh_token = user_tokens["refresh_token"].as_str().unwrap().to_owned();

    for refresh_token in [&admin_refresh_token, &user_refresh_token] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/agent/api/v1/auth/refresh")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "refresh_token": refresh_token }).to_string(),
                    ))
                    .unwrap(),
            )
            .await?;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/auth/refresh")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "refresh_token": agent_refresh_token }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let agent_login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/agent/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "username": username, "password": agent_password }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let active_agent_tokens = response_json(agent_login).await?;
    let active_agent_refresh_token = active_agent_tokens["refresh_token"].as_str().unwrap();
    sqlx::query("UPDATE agent_admin_users SET status = 'disabled' WHERE id = ?")
        .bind(agent.admin_user_id)
        .execute(&pool)
        .await?;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/agent/api/v1/auth/refresh")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "refresh_token": active_agent_refresh_token }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    sqlx::query("UPDATE agent_admin_users SET status = 'active' WHERE id = ?")
        .bind(agent.admin_user_id)
        .execute(&pool)
        .await?;
    let agent_login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/agent/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "username": username, "password": agent_password }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let active_agent_tokens = response_json(agent_login).await?;
    let active_agent_refresh_token = active_agent_tokens["refresh_token"].as_str().unwrap();
    sqlx::query("UPDATE agents SET status = 'suspended' WHERE id = ?")
        .bind(agent.agent_id)
        .execute(&pool)
        .await?;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/agent/api/v1/auth/refresh")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "refresh_token": active_agent_refresh_token }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    sqlx::query("DELETE FROM refresh_tokens WHERE actor_type = 'agent' AND actor_id = ?")
        .bind(agent.admin_user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM refresh_tokens WHERE actor_type = 'admin' AND actor_id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM refresh_tokens WHERE actor_type = 'user' AND actor_id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(role_id)
        .execute(&pool)
        .await?;
    cleanup_agent_fixture(&pool, &[agent], &[user_id]).await?;
    Ok(())
}

#[tokio::test]
async fn agent_me_route_requires_agent_scope() -> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    for token in [user_token, admin_token] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/agent/api/v1/me")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await?;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    Ok(())
}

#[tokio::test]
async fn agent_me_route_returns_current_agent_identity_without_sensitive_fields()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let agent = create_agent_with_password(&pool, "me-active", "agent-me-password-1").await;
    sqlx::query("UPDATE agent_admin_users SET last_login_at = CURRENT_TIMESTAMP(6) WHERE id = ?")
        .bind(agent.admin_user_id)
        .execute(&pool)
        .await?;
    let (username, agent_code, level): (String, String, i32) = sqlx::query_as(
        r#"SELECT agent_admin_users.username, agents.agent_code, agents.level
           FROM agent_admin_users
           INNER JOIN agents ON agents.id = agent_admin_users.agent_id
           WHERE agent_admin_users.id = ?
           LIMIT 1"#,
    )
    .bind(agent.admin_user_id)
    .fetch_one(&pool)
    .await?;
    let token = issue_token(
        &settings,
        format!("agent:{}", agent.admin_user_id),
        TokenScope::Agent,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/me")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let payload = response_json(response).await?;
    assert_eq!(status, StatusCode::OK, "payload: {payload}");
    assert_eq!(payload["agent_admin_id"], agent.admin_user_id);
    assert_eq!(payload["agent_id"], agent.agent_id);
    assert_eq!(payload["username"], username);
    assert_eq!(payload["agent_code"], agent_code);
    assert_eq!(payload["level"], level);
    assert_eq!(payload["agent_status"], "active");
    assert_eq!(payload["admin_status"], "active");
    assert!(
        payload["last_login_at"]
            .as_i64()
            .is_some_and(|value| value > 0)
    );
    assert!(payload.get("password_hash").is_none());
    assert!(payload.get("access_token").is_none());
    assert!(payload.get("refresh_token").is_none());

    cleanup_agent_fixture(&pool, &[agent], &[]).await?;
    Ok(())
}

#[tokio::test]
async fn agent_me_route_rejects_disabled_agent_context() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let agent = create_agent_with_password(&pool, "me-disabled", "agent-me-password-2").await;
    let token = issue_token(
        &settings,
        format!("agent:{}", agent.admin_user_id),
        TokenScope::Agent,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    sqlx::query("UPDATE agent_admin_users SET status = 'disabled' WHERE id = ?")
        .bind(agent.admin_user_id)
        .execute(&pool)
        .await?;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/me")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    sqlx::query("UPDATE agent_admin_users SET status = 'active' WHERE id = ?")
        .bind(agent.admin_user_id)
        .execute(&pool)
        .await?;
    sqlx::query("UPDATE agents SET status = 'suspended' WHERE id = ?")
        .bind(agent.agent_id)
        .execute(&pool)
        .await?;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/me")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    cleanup_agent_fixture(&pool, &[agent], &[]).await?;
    Ok(())
}

#[tokio::test]
async fn agent_users_route_rejects_non_agent_scopes() -> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));

    for path in [
        "/agent/api/v1/dashboard",
        "/agent/api/v1/users",
        "/agent/api/v1/users/1/assets",
        "/agent/api/v1/users/1/margin-positions",
        "/agent/api/v1/users/1/seconds-contract-orders",
        "/agent/api/v1/invite-codes",
        "/agent/api/v1/team-tree",
        "/agent/api/v1/commissions",
        "/agent/api/v1/convert/stats",
    ] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await?;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("authorization", format!("Bearer {user_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await?;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("authorization", format!("Bearer {admin_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await?;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    Ok(())
}

#[tokio::test]
async fn agent_convert_stats_only_summarize_authenticated_agent_team() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let agent_a = create_agent(&pool, "convert-a").await;
    let agent_b = create_agent(&pool, "convert-b").await;
    let direct_user = create_user(&pool, "convert-direct").await;
    let nested_user = create_user(&pool, "convert-nested").await;
    let other_agent_user = create_user(&pool, "convert-other").await;
    let unassigned_user = create_user(&pool, "convert-unassigned").await;
    refer_user_to_agent(&pool, direct_user, agent_a.agent_id, 1).await;
    refer_user_with_inviter(&pool, nested_user, agent_a.agent_id, direct_user, "user", 2).await;
    refer_user_to_agent(&pool, other_agent_user, agent_b.agent_id, 1).await;
    create_unassigned_referral(&pool, unassigned_user).await;
    let from_asset = create_asset(&pool, "acfsfrom").await;
    let to_asset = create_asset(&pool, "acfsoto").await;
    let pair_id = create_convert_pair(&pool, from_asset, to_asset).await;
    let direct_order = create_convert_order(
        &pool,
        (pair_id, from_asset, to_asset),
        direct_user,
        ("10.000000000000000000", "20.000000000000000000"),
        "pending",
    )
    .await;
    let nested_order = create_convert_order(
        &pool,
        (pair_id, from_asset, to_asset),
        nested_user,
        ("30.000000000000000000", "60.000000000000000000"),
        "completed",
    )
    .await;
    let other_order = create_convert_order(
        &pool,
        (pair_id, from_asset, to_asset),
        other_agent_user,
        ("50.000000000000000000", "100.000000000000000000"),
        "completed",
    )
    .await;
    let unassigned_order = create_convert_order(
        &pool,
        (pair_id, from_asset, to_asset),
        unassigned_user,
        ("70.000000000000000000", "140.000000000000000000"),
        "pending",
    )
    .await;

    let token = issue_token(
        &settings,
        format!("agent:{}", agent_a.admin_user_id),
        TokenScope::Agent,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/convert/stats")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 8192).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );
    let stats: Value = serde_json::from_slice(&body)?;
    assert_eq!(stats["agent_id"], agent_a.agent_id);
    assert_eq!(stats["total_orders"], 2);
    assert_eq!(stats["pending_orders"], 1);
    assert_eq!(stats["completed_orders"], 1);
    assert_eq!(stats["total_from_amount"], "40.000000000000000000");
    assert_eq!(stats["total_to_amount"], "80.000000000000000000");

    for quote_id in [
        &direct_order,
        &nested_order,
        &other_order,
        &unassigned_order,
    ] {
        sqlx::query("DELETE FROM convert_orders WHERE quote_id = ?")
            .bind(quote_id)
            .execute(&pool)
            .await?;
    }
    sqlx::query("DELETE FROM convert_pairs WHERE id = ?")
        .bind(pair_id)
        .execute(&pool)
        .await?;
    for asset_id in [from_asset, to_asset] {
        sqlx::query("DELETE FROM assets WHERE id = ?")
            .bind(asset_id)
            .execute(&pool)
            .await?;
    }
    cleanup_agent_fixture(
        &pool,
        &[agent_a, agent_b],
        &[direct_user, nested_user, other_agent_user, unassigned_user],
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn agent_dashboard_only_summarizes_authenticated_agent_team() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let agent_a = create_agent(&pool, "dashboard-a").await;
    let agent_b = create_agent(&pool, "dashboard-b").await;
    let direct_user = create_user(&pool, "dashboard-direct").await;
    let nested_user = create_user(&pool, "dashboard-nested").await;
    let other_agent_user = create_user(&pool, "dashboard-other").await;
    let unassigned_user = create_user(&pool, "dashboard-unassigned").await;
    refer_user_to_agent(&pool, direct_user, agent_a.agent_id, 1).await;
    refer_user_with_inviter(&pool, nested_user, agent_a.agent_id, direct_user, "user", 2).await;
    refer_user_to_agent(&pool, other_agent_user, agent_b.agent_id, 1).await;
    create_unassigned_referral(&pool, unassigned_user).await;
    let active_invite_code = create_invite_code(&pool, agent_a.agent_id, "active").await;
    let disabled_invite_code = create_invite_code(&pool, agent_a.agent_id, "disabled").await;
    let other_invite_code = create_invite_code(&pool, agent_b.agent_id, "active").await;
    let direct_commission = create_commission_record(
        &pool,
        agent_a.agent_id,
        direct_user,
        "spot_trade",
        "100.000000000000000000",
        "5.000000000000000000",
        "pending",
    )
    .await;
    let nested_commission = create_commission_record(
        &pool,
        agent_a.agent_id,
        nested_user,
        "convert_order",
        "200.000000000000000000",
        "8.000000000000000000",
        "settled",
    )
    .await;
    let other_commission = create_commission_record(
        &pool,
        agent_b.agent_id,
        other_agent_user,
        "spot_trade",
        "300.000000000000000000",
        "15.000000000000000000",
        "pending",
    )
    .await;
    let unassigned_commission = create_commission_record(
        &pool,
        agent_a.agent_id,
        unassigned_user,
        "spot_trade",
        "400.000000000000000000",
        "20.000000000000000000",
        "pending",
    )
    .await;

    let token = issue_token(
        &settings,
        format!("agent:{}", agent_a.admin_user_id),
        TokenScope::Agent,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/dashboard")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 8192).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );
    let dashboard: Value = serde_json::from_slice(&body)?;
    assert_eq!(dashboard["agent_id"], agent_a.agent_id);
    assert_eq!(dashboard["team_user_count"], 2);
    assert_eq!(dashboard["active_invite_code_count"], 1);
    assert_eq!(dashboard["commission_record_count"], 2);
    assert_eq!(
        dashboard["pending_commission_amount"],
        "5.000000000000000000"
    );
    assert_eq!(
        dashboard["settled_commission_amount"],
        "8.000000000000000000"
    );
    assert_eq!(
        dashboard["total_commission_amount"],
        "13.000000000000000000"
    );
    let commission_assets = dashboard["commission_assets"].as_array().unwrap();
    assert_eq!(commission_assets.len(), 1);
    assert_eq!(commission_assets[0]["payout_asset_id"], Value::Null);
    assert_eq!(commission_assets[0]["commission_record_count"], 2);
    assert_eq!(
        commission_assets[0]["pending_commission_amount"],
        "5.000000000000000000"
    );
    assert_eq!(
        commission_assets[0]["settled_commission_amount"],
        "8.000000000000000000"
    );
    assert_eq!(
        commission_assets[0]["total_commission_amount"],
        "13.000000000000000000"
    );

    for record_id in [
        direct_commission,
        nested_commission,
        other_commission,
        unassigned_commission,
    ] {
        sqlx::query("DELETE FROM agent_commission_records WHERE id = ?")
            .bind(record_id)
            .execute(&pool)
            .await?;
    }
    for code_id in [active_invite_code, disabled_invite_code, other_invite_code] {
        sqlx::query("DELETE FROM invite_codes WHERE id = ?")
            .bind(code_id)
            .execute(&pool)
            .await?;
    }
    cleanup_agent_fixture(
        &pool,
        &[agent_a, agent_b],
        &[direct_user, nested_user, other_agent_user, unassigned_user],
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn agent_commissions_only_return_authenticated_agent_team_records()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let agent_a = create_agent(&pool, "commission-a").await;
    let agent_b = create_agent(&pool, "commission-b").await;
    let direct_user = create_user(&pool, "commission-direct").await;
    let nested_user = create_user(&pool, "commission-nested").await;
    let other_agent_user = create_user(&pool, "commission-other").await;
    let unassigned_user = create_user(&pool, "commission-unassigned").await;
    refer_user_to_agent(&pool, direct_user, agent_a.agent_id, 1).await;
    refer_user_with_inviter(&pool, nested_user, agent_a.agent_id, direct_user, "user", 2).await;
    refer_user_to_agent(&pool, other_agent_user, agent_b.agent_id, 1).await;
    create_unassigned_referral(&pool, unassigned_user).await;

    let direct_source_id = format!("direct-{}", Uuid::now_v7());
    let nested_source_id = format!("nested-{}", Uuid::now_v7());
    let direct_commission = create_commission_record_with_source_id(
        &pool,
        CommissionSeed {
            agent_id: agent_a.agent_id,
            user_id: direct_user,
            source_type: "spot_trade",
            source_id: &direct_source_id,
            source_amount: "100.500000000000000000",
            commission_amount: "5.025000000000000000",
            status: "pending",
        },
    )
    .await;
    let nested_commission = create_commission_record_with_source_id(
        &pool,
        CommissionSeed {
            agent_id: agent_a.agent_id,
            user_id: nested_user,
            source_type: "convert_order",
            source_id: &nested_source_id,
            source_amount: "200.000000000000000000",
            commission_amount: "8.000000000000000000",
            status: "settled",
        },
    )
    .await;
    let payout_asset = create_asset(&pool, "acpayout").await;
    let payout_ledger = create_commission_payout_ledger(
        &pool,
        agent_a.agent_user_id,
        payout_asset,
        nested_commission,
        "8.000000000000000000",
        "18.000000000000000000",
    )
    .await;
    let other_agent_commission = create_commission_record(
        &pool,
        agent_b.agent_id,
        other_agent_user,
        "spot_trade",
        "300.000000000000000000",
        "15.000000000000000000",
        "pending",
    )
    .await;
    let unassigned_commission = create_commission_record(
        &pool,
        agent_a.agent_id,
        unassigned_user,
        "spot_trade",
        "400.000000000000000000",
        "20.000000000000000000",
        "pending",
    )
    .await;

    let token = issue_token(
        &settings,
        format!("agent:{}", agent_a.admin_user_id),
        TokenScope::Agent,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/commissions")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 8192).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );
    let commissions: Value = serde_json::from_slice(&body)?;
    assert_eq!(commissions["agent_id"], agent_a.agent_id);
    assert_eq!(commissions["total_records"], 2);
    assert_eq!(
        commissions["total_commission_amount"],
        "13.025000000000000000"
    );
    let records = commissions["commissions"].as_array().unwrap();
    let listed_ids = records
        .iter()
        .map(|record| record["id"].as_u64().unwrap())
        .collect::<Vec<_>>();
    // 列表按最新记录优先返回，避免超过分页上限后新佣金永远不可见。
    assert_eq!(listed_ids, vec![nested_commission, direct_commission]);
    assert_eq!(records[1]["user_id"], direct_user);
    assert_eq!(records[1]["source_type"], "spot_trade");
    assert_eq!(records[1]["source_id"], direct_source_id);
    assert_eq!(records[1]["source_amount"], "100.500000000000000000");
    assert_eq!(records[1]["commission_amount"], "5.025000000000000000");
    assert_eq!(records[1]["status"], "pending");
    assert_eq!(records[1]["depth"], 1);
    assert_eq!(records[1]["payout_ledger_id"], Value::Null);
    assert_eq!(records[1]["payout_asset_id"], Value::Null);
    assert_eq!(records[1]["payout_amount"], Value::Null);
    assert_eq!(records[1]["payout_balance_after"], Value::Null);
    assert_eq!(records[1]["payout_created_at"], Value::Null);
    assert_eq!(records[0]["user_id"], nested_user);
    assert_eq!(records[0]["source_id"], nested_source_id);
    assert_eq!(records[0]["status"], "settled");
    assert_eq!(records[0]["depth"], 2);
    assert_eq!(records[0]["payout_ledger_id"], payout_ledger);
    assert_eq!(records[0]["payout_asset_id"], payout_asset);
    assert_eq!(records[0]["payout_amount"], "8.000000000000000000");
    assert_eq!(records[0]["payout_balance_after"], "18.000000000000000000");
    assert!(records[0]["payout_created_at"].as_i64().unwrap() > 0);
    assert!(!listed_ids.contains(&other_agent_commission));
    assert!(!listed_ids.contains(&unassigned_commission));

    sqlx::query("DELETE FROM wallet_ledger WHERE id = ?")
        .bind(payout_ledger)
        .execute(&pool)
        .await?;
    for record_id in [
        direct_commission,
        nested_commission,
        other_agent_commission,
        unassigned_commission,
    ] {
        sqlx::query("DELETE FROM agent_commission_records WHERE id = ?")
            .bind(record_id)
            .execute(&pool)
            .await?;
    }
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(payout_asset)
        .execute(&pool)
        .await?;
    cleanup_agent_fixture(
        &pool,
        &[agent_a, agent_b],
        &[direct_user, nested_user, other_agent_user, unassigned_user],
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn agent_invite_codes_are_scoped_to_authenticated_agent() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let agent_a = create_agent(&pool, "invite-a").await;
    let agent_b = create_agent(&pool, "invite-b").await;
    let owned_code = create_invite_code(&pool, agent_a.agent_id, "active").await;
    let other_code = create_invite_code(&pool, agent_b.agent_id, "active").await;
    let token = issue_token(
        &settings,
        format!("agent:{}", agent_a.admin_user_id),
        TokenScope::Agent,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/agent/api/v1/invite-codes")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "usage_limit": 25 }).to_string()))
                .unwrap(),
        )
        .await?;
    let create_status = create_response.status();
    let create_body = axum::body::to_bytes(create_response.into_body(), 8192).await?;
    assert_eq!(
        create_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&create_body)
    );
    let created: Value = serde_json::from_slice(&create_body)?;
    assert_eq!(created["owner_id"], agent_a.agent_id);
    assert_eq!(created["usage_limit"], 25);
    assert_eq!(created["status"], "active");
    let created_code = created["code"].as_str().unwrap();
    assert_eq!(created_code.len(), 6);
    assert!(
        created_code
            .chars()
            .all(|character| character.is_ascii_uppercase() || character.is_ascii_digit())
    );
    let created_code_id = created["id"].as_u64().unwrap();

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/invite-codes")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let list_status = list_response.status();
    let list_body = axum::body::to_bytes(list_response.into_body(), 8192).await?;
    assert_eq!(
        list_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&list_body)
    );
    let invite_codes: Value = serde_json::from_slice(&list_body)?;
    assert_eq!(
        invite_codes["invite_codes"][0]["id"].as_u64(),
        Some(created_code_id),
        "newest active invite code must be listed first"
    );
    let listed_ids = invite_codes["invite_codes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|code| code["id"].as_u64().unwrap())
        .collect::<Vec<_>>();
    assert!(listed_ids.contains(&owned_code));
    assert!(listed_ids.contains(&created_code_id));
    assert!(!listed_ids.contains(&other_code));

    let deactivate_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/agent/api/v1/invite-codes/{owned_code}/status"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "status": "disabled" }).to_string()))
                .unwrap(),
        )
        .await?;
    assert_eq!(deactivate_response.status(), StatusCode::OK);
    let (owned_status,): (String,) = sqlx::query_as("SELECT status FROM invite_codes WHERE id = ?")
        .bind(owned_code)
        .fetch_one(&pool)
        .await?;
    assert_eq!(owned_status, "disabled");

    let other_response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/agent/api/v1/invite-codes/{other_code}/status"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "status": "disabled" }).to_string()))
                .unwrap(),
        )
        .await?;
    assert_eq!(other_response.status(), StatusCode::NOT_FOUND);
    let (other_status,): (String,) = sqlx::query_as("SELECT status FROM invite_codes WHERE id = ?")
        .bind(other_code)
        .fetch_one(&pool)
        .await?;
    assert_eq!(other_status, "active");

    cleanup_agent_fixture(&pool, &[agent_a, agent_b], &[]).await?;
    Ok(())
}

#[tokio::test]
async fn agent_team_tree_only_returns_authenticated_agent_referrals() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let agent_a = create_agent(&pool, "tree-a").await;
    let agent_b = create_agent(&pool, "tree-b").await;
    let direct_user = create_user(&pool, "tree-direct").await;
    let nested_user = create_user(&pool, "tree-nested").await;
    let other_agent_user = create_user(&pool, "tree-other").await;
    let unassigned_user = create_user(&pool, "tree-unassigned").await;
    refer_user_to_agent(&pool, direct_user, agent_a.agent_id, 1).await;
    refer_user_with_inviter(&pool, nested_user, agent_a.agent_id, direct_user, "user", 2).await;
    refer_user_to_agent(&pool, other_agent_user, agent_b.agent_id, 1).await;
    create_unassigned_referral(&pool, unassigned_user).await;

    let token = issue_token(
        &settings,
        format!("agent:{}", agent_a.admin_user_id),
        TokenScope::Agent,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/team-tree")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 8192).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );
    let tree: Value = serde_json::from_slice(&body)?;
    assert_eq!(tree["root_agent_id"], agent_a.agent_id);
    let nodes = tree["nodes"].as_array().unwrap();
    let listed_ids = nodes
        .iter()
        .map(|node| node["user_id"].as_u64().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(listed_ids, vec![direct_user, nested_user]);
    assert_eq!(nodes[0]["direct_inviter_type"], "agent");
    assert_eq!(nodes[0]["direct_inviter_id"], agent_a.agent_id);
    assert_eq!(nodes[0]["depth"], 1);
    assert_eq!(nodes[1]["direct_inviter_type"], "user");
    assert_eq!(nodes[1]["direct_inviter_id"], direct_user);
    assert_eq!(nodes[1]["depth"], 2);
    assert!(!listed_ids.contains(&other_agent_user));
    assert!(!listed_ids.contains(&unassigned_user));

    cleanup_agent_fixture(
        &pool,
        &[agent_a, agent_b],
        &[direct_user, nested_user, other_agent_user, unassigned_user],
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn agent_users_route_rejects_suspended_agent_root() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let agent = create_agent(&pool, "suspended").await;
    let team_user = create_user(&pool, "suspended-team").await;
    refer_user_to_agent(&pool, team_user, agent.agent_id, 1).await;
    sqlx::query("UPDATE agents SET status = 'suspended' WHERE id = ?")
        .bind(agent.agent_id)
        .execute(&pool)
        .await?;

    let token = issue_token(
        &settings,
        format!("agent:{}", agent.admin_user_id),
        TokenScope::Agent,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/users")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    cleanup_agent_fixture(&pool, &[agent], &[team_user]).await?;
    Ok(())
}

#[tokio::test]
async fn agent_users_route_only_returns_authenticated_agent_team() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let agent_a = create_agent(&pool, "a").await;
    let agent_b = create_agent(&pool, "b").await;
    let direct_team_user = create_user(&pool, "direct").await;
    let nested_team_user = create_user(&pool, "nested").await;
    let other_agent_user = create_user(&pool, "other-agent").await;
    let unassigned_user = create_user(&pool, "unassigned").await;
    refer_user_to_agent(&pool, direct_team_user, agent_a.agent_id, 1).await;
    refer_user_with_inviter(
        &pool,
        nested_team_user,
        agent_a.agent_id,
        direct_team_user,
        "user",
        2,
    )
    .await;
    refer_user_to_agent(&pool, other_agent_user, agent_b.agent_id, 1).await;
    create_unassigned_referral(&pool, unassigned_user).await;

    let token = issue_token(
        &settings,
        format!("agent:{}", agent_a.admin_user_id),
        TokenScope::Agent,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/users")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 8192).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );
    let users: Value = serde_json::from_slice(&body)?;
    let listed_ids = users["users"]
        .as_array()
        .unwrap()
        .iter()
        .map(|user| user["user_id"].as_u64().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(listed_ids, vec![direct_team_user, nested_team_user]);
    assert!(!listed_ids.contains(&other_agent_user));
    assert!(!listed_ids.contains(&unassigned_user));
    assert_eq!(users["users"][0]["owner_agent_id"], agent_a.agent_id);
    assert_eq!(users["users"][0]["root_agent_id"], agent_a.agent_id);
    assert_eq!(users["users"][0]["direct_inviter_type"], "agent");
    assert_eq!(users["users"][0]["direct_inviter_id"], agent_a.agent_id);
    assert_eq!(users["users"][1]["owner_agent_id"], agent_a.agent_id);
    assert_eq!(users["users"][1]["root_agent_id"], agent_a.agent_id);
    assert_eq!(users["users"][1]["direct_inviter_type"], "user");
    assert_eq!(users["users"][1]["direct_inviter_id"], direct_team_user);

    cleanup_agent_fixture(
        &pool,
        &[agent_a, agent_b],
        &[
            direct_team_user,
            nested_team_user,
            other_agent_user,
            unassigned_user,
        ],
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn agent_hierarchy_scopes_users_and_blocks_children_when_parent_is_suspended()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let root = create_agent(&pool, "hier-root").await;
    let child = create_child_agent(&pool, root, "hier-child").await;
    let grandchild = create_child_agent(&pool, child, "hier-grand").await;
    let sibling = create_child_agent(&pool, root, "hier-sibling").await;
    let root_user = create_user(&pool, "hierarchy-root-user").await;
    let child_user = create_user(&pool, "hierarchy-child-user").await;
    let grandchild_user = create_user(&pool, "hierarchy-grandchild-user").await;
    let sibling_user = create_user(&pool, "hierarchy-sibling-user").await;
    let grandchild_referred_user = create_user(&pool, "hierarchy-user-invite").await;
    refer_user_to_agent(&pool, root_user, root.agent_id, 1).await;
    refer_user_to_agent(&pool, child_user, child.agent_id, 1).await;
    refer_user_to_agent(&pool, grandchild_user, grandchild.agent_id, 1).await;
    refer_user_to_agent(&pool, sibling_user, sibling.agent_id, 1).await;
    refer_user_with_inviter(
        &pool,
        grandchild_referred_user,
        grandchild.agent_id,
        grandchild_user,
        "user",
        2,
    )
    .await;

    let root_token = issue_token(
        &settings,
        format!("agent:{}", root.admin_user_id),
        TokenScope::Agent,
        900,
    )?;
    let child_token = issue_token(
        &settings,
        format!("agent:{}", child.admin_user_id),
        TokenScope::Agent,
        900,
    )?;
    let grandchild_token = issue_token(
        &settings,
        format!("agent:{}", grandchild.admin_user_id),
        TokenScope::Agent,
        900,
    )?;
    let sibling_token = issue_token(
        &settings,
        format!("agent:{}", sibling.admin_user_id),
        TokenScope::Agent,
        900,
    )?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let root_users = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/users")
                .header("authorization", format!("Bearer {root_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(root_users.status(), StatusCode::OK);
    let root_payload = response_json(root_users).await?;
    let root_user_ids = root_payload["users"]
        .as_array()
        .unwrap()
        .iter()
        .map(|user| user["user_id"].as_u64().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        root_user_ids,
        vec![
            root_user,
            child_user,
            sibling_user,
            grandchild_user,
            grandchild_referred_user,
        ]
    );
    let referred_user = root_payload["users"]
        .as_array()
        .unwrap()
        .iter()
        .find(|user| user["user_id"] == grandchild_referred_user)
        .unwrap();
    assert_eq!(referred_user["owner_agent_id"], grandchild.agent_id);
    assert_eq!(referred_user["root_agent_id"], grandchild.agent_id);
    assert_eq!(referred_user["direct_inviter_type"], "user");
    assert_eq!(referred_user["direct_inviter_id"], grandchild_user);

    let child_users = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/users")
                .header("authorization", format!("Bearer {child_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(child_users.status(), StatusCode::OK);
    let child_payload = response_json(child_users).await?;
    let child_user_ids = child_payload["users"]
        .as_array()
        .unwrap()
        .iter()
        .map(|user| user["user_id"].as_u64().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        child_user_ids,
        vec![child_user, grandchild_user, grandchild_referred_user]
    );

    let grandchild_users = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/users")
                .header("authorization", format!("Bearer {grandchild_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(grandchild_users.status(), StatusCode::OK);
    let grandchild_payload = response_json(grandchild_users).await?;
    let grandchild_user_ids = grandchild_payload["users"]
        .as_array()
        .unwrap()
        .iter()
        .map(|user| user["user_id"].as_u64().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        grandchild_user_ids,
        vec![grandchild_user, grandchild_referred_user]
    );

    let sibling_users = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/users")
                .header("authorization", format!("Bearer {sibling_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(sibling_users.status(), StatusCode::OK);
    let sibling_payload = response_json(sibling_users).await?;
    let sibling_user_ids = sibling_payload["users"]
        .as_array()
        .unwrap()
        .iter()
        .map(|user| user["user_id"].as_u64().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(sibling_user_ids, vec![sibling_user]);

    let sub_agents = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/sub-agents")
                .header("authorization", format!("Bearer {root_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(sub_agents.status(), StatusCode::OK);
    let sub_agent_payload = response_json(sub_agents).await?;
    let sub_agent_ids = sub_agent_payload["agents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|agent| agent["id"].as_u64().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        sub_agent_ids,
        vec![child.agent_id, sibling.agent_id, grandchild.agent_id]
    );

    sqlx::query("UPDATE agents SET status = 'suspended' WHERE id = ?")
        .bind(child.agent_id)
        .execute(&pool)
        .await?;
    let blocked_grandchild = app
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/users")
                .header("authorization", format!("Bearer {grandchild_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(blocked_grandchild.status(), StatusCode::UNAUTHORIZED);

    cleanup_agent_fixture(
        &pool,
        &[grandchild, child, sibling, root],
        &[
            root_user,
            child_user,
            grandchild_user,
            sibling_user,
            grandchild_referred_user,
        ],
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn agent_dashboard_breaks_down_commissions_per_payout_asset() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let agent = create_agent(&pool, "dashboard-assets").await;
    let team_user = create_user(&pool, "dashboard-assets-user").await;
    refer_user_to_agent(&pool, team_user, agent.agent_id, 1).await;
    let asset_a = create_asset(&pool, "adba").await;
    let asset_b = create_asset(&pool, "adbb").await;
    let pending_commission = create_commission_record(
        &pool,
        agent.agent_id,
        team_user,
        "convert_order",
        "100.000000000000000000",
        "5.000000000000000000",
        "pending",
    )
    .await;
    let settled_commission = create_commission_record(
        &pool,
        agent.agent_id,
        team_user,
        "spot_trade",
        "200.000000000000000000",
        "8.000000000000000000",
        "settled",
    )
    .await;
    for (commission_id, asset_id) in [(pending_commission, asset_a), (settled_commission, asset_b)]
    {
        sqlx::query("UPDATE agent_commission_records SET payout_asset_id = ? WHERE id = ?")
            .bind(asset_id)
            .bind(commission_id)
            .execute(&pool)
            .await?;
    }
    let token = issue_token(
        &settings,
        format!("agent:{}", agent.admin_user_id),
        TokenScope::Agent,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/dashboard")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let dashboard = response_json(response).await?;
    assert_eq!(status, StatusCode::OK, "payload: {dashboard}");
    assert_eq!(dashboard["commission_record_count"], 2);
    // 多资产佣金不做跨币种求和，顶层金额归零，以 commission_assets 明细为准。
    assert_eq!(dashboard["pending_commission_amount"], "0");
    assert_eq!(dashboard["settled_commission_amount"], "0");
    assert_eq!(dashboard["total_commission_amount"], "0");
    let commission_assets = dashboard["commission_assets"].as_array().unwrap();
    assert_eq!(commission_assets.len(), 2);
    let (first, second) = if asset_a < asset_b { (0, 1) } else { (1, 0) };
    assert_eq!(commission_assets[first]["payout_asset_id"], asset_a);
    assert_eq!(commission_assets[first]["commission_record_count"], 1);
    assert_eq!(
        commission_assets[first]["pending_commission_amount"],
        "5.000000000000000000"
    );
    assert_eq!(
        commission_assets[first]["total_commission_amount"],
        "5.000000000000000000"
    );
    assert_eq!(commission_assets[second]["payout_asset_id"], asset_b);
    assert_eq!(commission_assets[second]["commission_record_count"], 1);
    assert_eq!(
        commission_assets[second]["settled_commission_amount"],
        "8.000000000000000000"
    );
    assert_eq!(
        commission_assets[second]["total_commission_amount"],
        "8.000000000000000000"
    );

    for record_id in [pending_commission, settled_commission] {
        sqlx::query("DELETE FROM agent_commission_records WHERE id = ?")
            .bind(record_id)
            .execute(&pool)
            .await?;
    }
    for asset_id in [asset_a, asset_b] {
        sqlx::query("DELETE FROM assets WHERE id = ?")
            .bind(asset_id)
            .execute(&pool)
            .await?;
    }
    cleanup_agent_fixture(&pool, &[agent], &[team_user]).await?;
    Ok(())
}

#[tokio::test]
async fn agent_lists_support_newest_first_commission_pagination() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let agent = create_agent(&pool, "commission-page").await;
    let team_user = create_user(&pool, "commission-page-user").await;
    refer_user_to_agent(&pool, team_user, agent.agent_id, 1).await;
    let mut commission_ids = Vec::new();
    for index in 0..3 {
        commission_ids.push(
            create_commission_record(
                &pool,
                agent.agent_id,
                team_user,
                "spot_trade",
                "100.000000000000000000",
                &format!("{}.000000000000000000", index + 1),
                "pending",
            )
            .await,
        );
    }
    let invite_code_a = create_invite_code(&pool, agent.agent_id, "active").await;
    let invite_code_b = create_invite_code(&pool, agent.agent_id, "active").await;
    let token = issue_token(
        &settings,
        format!("agent:{}", agent.admin_user_id),
        TokenScope::Agent,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let get_json = |uri: String| {
        let app = app.clone();
        let token = token.clone();
        async move {
            let response = app
                .oneshot(
                    Request::builder()
                        .uri(uri)
                        .header("authorization", format!("Bearer {token}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await?;
            assert_eq!(response.status(), StatusCode::OK);
            response_json(response).await
        }
    };

    let first_page = get_json("/agent/api/v1/commissions?limit=2".to_owned()).await?;
    let first_page_ids = first_page["commissions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| record["id"].as_u64().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(first_page_ids, vec![commission_ids[2], commission_ids[1]]);
    assert_eq!(first_page["total_records"], 2);

    let second_page = get_json("/agent/api/v1/commissions?limit=2&offset=2".to_owned()).await?;
    let second_page_ids = second_page["commissions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| record["id"].as_u64().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(second_page_ids, vec![commission_ids[0]]);

    let invite_page = get_json("/agent/api/v1/invite-codes?limit=1&offset=1".to_owned()).await?;
    let invite_page_ids = invite_page["invite_codes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|code| code["id"].as_u64().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(invite_page_ids, vec![invite_code_a]);

    let users_page = get_json("/agent/api/v1/users?limit=1".to_owned()).await?;
    assert_eq!(users_page["users"].as_array().unwrap().len(), 1);

    for record_id in &commission_ids {
        sqlx::query("DELETE FROM agent_commission_records WHERE id = ?")
            .bind(record_id)
            .execute(&pool)
            .await?;
    }
    let _ = (invite_code_a, invite_code_b);
    cleanup_agent_fixture(&pool, &[agent], &[team_user]).await?;
    Ok(())
}

#[tokio::test]
async fn agent_password_change_requires_current_password_and_rotates_login()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let current_password = "agent-change-old-1";
    let new_password = "agent-change-new-1";
    let agent = create_agent_with_password(&pool, "password-change", current_password).await;
    let username: String =
        sqlx::query_scalar("SELECT username FROM agent_admin_users WHERE id = ? LIMIT 1")
            .bind(agent.admin_user_id)
            .fetch_one(&pool)
            .await?;
    let token = issue_token(
        &settings,
        format!("agent:{}", agent.admin_user_id),
        TokenScope::Agent,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let change_request = |body: Value| {
        Request::builder()
            .method("POST")
            .uri("/agent/api/v1/password/change")
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    };
    let login_request = |password: &str| {
        Request::builder()
            .method("POST")
            .uri("/agent/api/v1/auth/login")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({ "username": username, "password": password }).to_string(),
            ))
            .unwrap()
    };

    let seed_login = app.clone().oneshot(login_request(current_password)).await?;
    assert_eq!(seed_login.status(), StatusCode::OK);

    let wrong_current = app
        .clone()
        .oneshot(change_request(json!({
            "current_password": "agent-change-wrong",
            "new_password": new_password
        })))
        .await?;
    assert_eq!(wrong_current.status(), StatusCode::BAD_REQUEST);

    let weak_new = app
        .clone()
        .oneshot(change_request(json!({
            "current_password": current_password,
            "new_password": "123"
        })))
        .await?;
    assert_eq!(weak_new.status(), StatusCode::BAD_REQUEST);

    let unchanged = app
        .clone()
        .oneshot(change_request(json!({
            "current_password": current_password,
            "new_password": current_password
        })))
        .await?;
    assert_eq!(unchanged.status(), StatusCode::BAD_REQUEST);

    let still_old_password = app.clone().oneshot(login_request(current_password)).await?;
    assert_eq!(still_old_password.status(), StatusCode::OK);

    let changed = app
        .clone()
        .oneshot(change_request(json!({
            "current_password": current_password,
            "new_password": new_password
        })))
        .await?;
    let changed_status = changed.status();
    let changed_payload = response_json(changed).await?;
    assert_eq!(changed_status, StatusCode::OK, "payload: {changed_payload}");
    assert_eq!(changed_payload["changed"], true);
    assert_eq!(changed_payload["requires_relogin"], true);

    let stale_login = app.clone().oneshot(login_request(current_password)).await?;
    assert_eq!(stale_login.status(), StatusCode::UNAUTHORIZED);

    let rotated_login = app.clone().oneshot(login_request(new_password)).await?;
    assert_eq!(rotated_login.status(), StatusCode::OK);

    let revoked_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM refresh_tokens
           WHERE actor_type = 'agent' AND actor_id = ? AND revoked_at IS NOT NULL"#,
    )
    .bind(agent.admin_user_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(revoked_count, 2);

    sqlx::query("DELETE FROM refresh_tokens WHERE actor_type = 'agent' AND actor_id = ?")
        .bind(agent.admin_user_id)
        .execute(&pool)
        .await?;
    cleanup_agent_fixture(&pool, &[agent], &[]).await?;
    Ok(())
}

#[tokio::test]
async fn linked_agent_user_my_code_is_concurrent_safe_and_tracks_latest_portal_code()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let agent = create_agent(&pool, "linked-code").await;
    let portal_invitee = create_user(&pool, "linked-code-portal-invitee").await;
    let legacy_invitee = create_user(&pool, "linked-code-legacy-invitee").await;
    let user_token = issue_token(
        &settings,
        format!("user:{}", agent.agent_user_id),
        TokenScope::User,
        900,
    )?;
    let invitee_token = issue_token(
        &settings,
        format!("user:{legacy_invitee}"),
        TokenScope::User,
        900,
    )?;
    let portal_invitee_token = issue_token(
        &settings,
        format!("user:{portal_invitee}"),
        TokenScope::User,
        900,
    )?;
    let agent_token = issue_token(
        &settings,
        format!("agent:{}", agent.admin_user_id),
        TokenScope::Agent,
        900,
    )?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let first = agent_get_json(app.clone(), &user_token, "/api/v1/referral/my-code");
    let second = agent_get_json(app.clone(), &user_token, "/api/v1/referral/my-code");
    let (first, second) = tokio::join!(first, second);
    let (first_status, first_payload) = first?;
    let (second_status, second_payload) = second?;
    assert_eq!(first_status, StatusCode::OK, "payload: {first_payload}");
    assert_eq!(second_status, StatusCode::OK, "payload: {second_payload}");
    assert_eq!(first_payload["owner_type"], "agent");
    assert_eq!(first_payload["owner_id"], agent.agent_id);
    assert_eq!(first_payload["code"], second_payload["code"]);
    let generated = first_payload["code"].as_str().unwrap();
    assert_eq!(generated.len(), 6);
    assert!(
        generated
            .chars()
            .all(|character| character.is_ascii_uppercase() || character.is_ascii_digit())
    );
    let initial_active_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM invite_codes WHERE owner_type = 'agent' AND owner_id = ? AND status = 'active'",
    )
    .bind(agent.agent_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(initial_active_count, 1);

    let legacy_code = format!("legacy-agent-code-{}", Uuid::now_v7().simple());
    sqlx::query(
        "INSERT INTO invite_codes (owner_type, owner_id, code, status) VALUES ('agent', ?, ?, 'active')",
    )
    .bind(agent.agent_id)
    .bind(&legacy_code)
    .execute(&pool)
    .await?;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/agent/api/v1/invite-codes")
                .header("authorization", format!("Bearer {agent_token}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"usage_limit":12}"#))?,
        )
        .await?;
    let create_status = create_response.status();
    let portal_code = response_json(create_response).await?;
    assert_eq!(create_status, StatusCode::OK, "payload: {portal_code}");
    let portal_code_text = portal_code["code"].as_str().unwrap();
    assert_eq!(portal_code_text.len(), 6);
    assert!(
        portal_code_text
            .chars()
            .all(|character| character.is_ascii_uppercase() || character.is_ascii_digit())
    );

    let (latest_status, latest) =
        agent_get_json(app.clone(), &user_token, "/api/v1/referral/my-code").await?;
    assert_eq!(latest_status, StatusCode::OK, "payload: {latest}");
    assert_eq!(latest["code"], portal_code["code"]);
    assert_eq!(latest["owner_type"], "agent");

    // 状态切换与关联用户读取共用代理协调锁；停用最新码后必须立即回退到仍启用的历史码。
    // 重复停用是幂等成功，而不是依赖 MySQL changed rows 把既有记录误报为不存在。
    for _ in 0..2 {
        let disabled_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!(
                        "/agent/api/v1/invite-codes/{}/status",
                        portal_code["id"].as_u64().unwrap()
                    ))
                    .header("authorization", format!("Bearer {agent_token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"status":"disabled"}"#))?,
            )
            .await?;
        let disabled_status = disabled_response.status();
        let disabled = response_json(disabled_response).await?;
        assert_eq!(disabled_status, StatusCode::OK, "payload: {disabled}");
        assert_eq!(disabled["status"], "disabled");
    }
    let (fallback_status, fallback) =
        agent_get_json(app.clone(), &user_token, "/api/v1/referral/my-code").await?;
    assert_eq!(fallback_status, StatusCode::OK, "payload: {fallback}");
    assert_eq!(fallback["code"], legacy_code);

    let reenabled_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/agent/api/v1/invite-codes/{}/status",
                    portal_code["id"].as_u64().unwrap()
                ))
                .header("authorization", format!("Bearer {agent_token}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"status":"active"}"#))?,
        )
        .await?;
    let reenabled_status = reenabled_response.status();
    let reenabled = response_json(reenabled_response).await?;
    assert_eq!(reenabled_status, StatusCode::OK, "payload: {reenabled}");
    assert_eq!(reenabled["status"], "active");
    let (_, latest_again) =
        agent_get_json(app.clone(), &user_token, "/api/v1/referral/my-code").await?;
    assert_eq!(latest_again["code"], portal_code["code"]);

    let portal_bind = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/referral/bind")
                .header("authorization", format!("Bearer {portal_invitee_token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "code": portal_code_text }).to_string()))?,
        )
        .await?;
    let portal_bind_status = portal_bind.status();
    let portal_bind_payload = response_json(portal_bind).await?;
    assert_eq!(
        portal_bind_status,
        StatusCode::OK,
        "payload: {portal_bind_payload}"
    );
    assert_eq!(portal_bind_payload["root_agent_id"], agent.agent_id);

    let legacy_bind = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/referral/bind")
                .header("authorization", format!("Bearer {invitee_token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "code": legacy_code }).to_string()))?,
        )
        .await?;
    let legacy_bind_status = legacy_bind.status();
    let legacy_bind_payload = response_json(legacy_bind).await?;
    assert_eq!(
        legacy_bind_status,
        StatusCode::OK,
        "payload: {legacy_bind_payload}"
    );
    assert_eq!(legacy_bind_payload["root_agent_id"], agent.agent_id);

    cleanup_agent_fixture(&pool, &[agent], &[portal_invitee, legacy_invitee]).await?;
    Ok(())
}

#[tokio::test]
async fn agent_user_financial_views_enforce_subtree_filters_totals_and_read_only_snapshots()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let root = create_agent(&pool, "pf-root").await;
    let child = create_child_agent(&pool, root, "pf-child").await;
    let grandchild = create_child_agent(&pool, child, "pf-grandchild").await;
    let sibling = create_child_agent(&pool, root, "pf-sibling").await;
    let other_root = create_agent(&pool, "pf-other-root").await;
    let root_user = create_user(&pool, "portfolio-root-user").await;
    let child_user = create_user(&pool, "portfolio-child-user").await;
    let sibling_user = create_user(&pool, "portfolio-sibling-user").await;
    let other_user = create_user(&pool, "portfolio-other-user").await;
    let unassigned_user = create_user(&pool, "portfolio-unassigned-user").await;
    refer_user_to_agent(&pool, root_user, root.agent_id, 1).await;
    refer_user_to_agent(&pool, child_user, grandchild.agent_id, 1).await;
    refer_user_to_agent(&pool, sibling_user, sibling.agent_id, 1).await;
    refer_user_to_agent(&pool, other_user, other_root.agent_id, 1).await;
    create_unassigned_referral(&pool, unassigned_user).await;

    let base_asset = create_asset(&pool, "pfbase").await;
    let quote_asset = create_asset(&pool, "pfquote").await;
    let (base_symbol, quote_symbol): (String, String) = sqlx::query_as(
        "SELECT (SELECT symbol FROM assets WHERE id = ?), (SELECT symbol FROM assets WHERE id = ?)",
    )
    .bind(base_asset)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    let pair_symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, 18, 18, 1, 'active', 'external')"#,
    )
    .bind(base_asset)
    .bind(quote_asset)
    .bind(&pair_symbol)
    .execute(&pool)
    .await?
    .last_insert_id();
    let margin_product_id = sqlx::query(
        r#"INSERT INTO margin_products
           (pair_id, margin_asset, margin_mode, margin_modes, leverage_levels,
            max_leverage, min_margin, max_margin, maintenance_margin_rate, status)
           VALUES (?, ?, 'isolated', JSON_ARRAY('isolated'), JSON_ARRAY('2'),
                   5, 1, 1000, 0.05, 'active')"#,
    )
    .bind(pair_id)
    .bind(quote_asset)
    .execute(&pool)
    .await?
    .last_insert_id();
    let seconds_product_id = sqlx::query(
        r#"INSERT INTO seconds_contract_products
           (pair_id, stake_asset, duration_seconds, payout_rate, min_stake, max_stake, status)
           VALUES (?, ?, 60, 0.8, 1, 1000, 'active')"#,
    )
    .bind(pair_id)
    .bind(quote_asset)
    .execute(&pool)
    .await?
    .last_insert_id();

    sqlx::query(
        r#"INSERT INTO wallet_accounts (user_id, asset_id, available, frozen, locked)
           VALUES (?, ?, 123.456789012345678901, 2, 3), (?, ?, 999, 0, 0)"#,
    )
    .bind(child_user)
    .bind(quote_asset)
    .bind(other_user)
    .bind(quote_asset)
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"INSERT INTO margin_wallet_accounts (user_id, asset_id, available, frozen, locked)
           VALUES (?, ?, 45.5, 4, 5), (?, ?, 888, 0, 0)"#,
    )
    .bind(child_user)
    .bind(quote_asset)
    .bind(other_user)
    .bind(quote_asset)
    .execute(&pool)
    .await?;

    let insert_margin = |user_id: u64, status: &'static str, suffix: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query(
                r#"INSERT INTO margin_positions
                   (user_id, product_id, pair_id, margin_asset, wallet_scope, margin_mode,
                    direction, order_type, margin_amount, leverage, notional_amount,
                    borrowed_amount, interest_amount, entry_price, exit_price, realized_pnl,
                    status, idempotency_key, closed_at)
                   VALUES (?, ?, ?, ?, 'spot', 'isolated', 'long', 'market', 10, 2, 20,
                           10, 0.5, 100, IF(? = 'closed', 110, NULL),
                           IF(? = 'closed', 5, NULL), ?, ?,
                           IF(? = 'closed', CURRENT_TIMESTAMP(6), NULL))"#,
            )
            .bind(user_id)
            .bind(margin_product_id)
            .bind(pair_id)
            .bind(quote_asset)
            .bind(status)
            .bind(status)
            .bind(status)
            .bind(format!(
                "agent-portfolio-margin-{suffix}-{}",
                Uuid::now_v7().simple()
            ))
            .bind(status)
            .execute(&pool)
            .await
            .map(|result| result.last_insert_id())
        }
    };
    let opened_margin = insert_margin(child_user, "opened", "opened").await?;
    let closed_margin = insert_margin(child_user, "closed", "closed").await?;
    let other_margin = insert_margin(other_user, "opened", "other").await?;

    let insert_seconds = |user_id: u64, status: &'static str, suffix: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query(
                r#"INSERT INTO seconds_contract_orders
                   (user_id, product_id, pair_id, stake_asset, direction, stake_amount,
                    duration_seconds, payout_rate, entry_price, settlement_price, status,
                    result, idempotency_key, expires_at, settled_at,
                    settlement_failure_code, settlement_failed_at,
                    settlement_window_start, settlement_window_end)
                   VALUES (?, ?, ?, ?, 'up', 7.25, 60, 0.8, 100,
                           IF(? = 'settled', 110, NULL), ?,
                           IF(? = 'settled', 'win', NULL), ?,
                           DATE_ADD(CURRENT_TIMESTAMP(6), INTERVAL 60 SECOND),
                           IF(? = 'settled', CURRENT_TIMESTAMP(6), NULL),
                           IF(? = 'manual_review', 'test_price_unavailable', NULL),
                           IF(? = 'manual_review', CURRENT_TIMESTAMP(6), NULL),
                           IF(? = 'manual_review', DATE_SUB(CURRENT_TIMESTAMP(6), INTERVAL 1 SECOND), NULL),
                           IF(? = 'manual_review', CURRENT_TIMESTAMP(6), NULL))"#,
            )
            .bind(user_id)
            .bind(seconds_product_id)
            .bind(pair_id)
            .bind(quote_asset)
            .bind(status)
            .bind(status)
            .bind(status)
            .bind(format!(
                "agent-portfolio-seconds-{suffix}-{}",
                Uuid::now_v7().simple()
            ))
            .bind(status)
            .bind(status)
            .bind(status)
            .bind(status)
            .bind(status)
            .execute(&pool)
            .await
            .map(|result| result.last_insert_id())
        }
    };
    let opened_seconds = insert_seconds(child_user, "opened", "opened").await?;
    let settled_seconds = insert_seconds(child_user, "settled", "settled").await?;
    let review_seconds = insert_seconds(child_user, "manual_review", "review").await?;
    let other_seconds = insert_seconds(other_user, "opened", "other").await?;

    let wallet_before: (BigDecimal, BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen, locked FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(child_user)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    let margin_wallet_before: (BigDecimal, BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen, locked FROM margin_wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(child_user)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    let margin_statuses_before: Vec<(u64, String)> =
        sqlx::query_as("SELECT id, status FROM margin_positions WHERE user_id = ? ORDER BY id")
            .bind(child_user)
            .fetch_all(&pool)
            .await?;
    let seconds_statuses_before: Vec<(u64, String)> = sqlx::query_as(
        "SELECT id, status FROM seconds_contract_orders WHERE user_id = ? ORDER BY id",
    )
    .bind(child_user)
    .fetch_all(&pool)
    .await?;

    let root_token = issue_token(
        &settings,
        format!("agent:{}", root.admin_user_id),
        TokenScope::Agent,
        900,
    )?;
    let child_token = issue_token(
        &settings,
        format!("agent:{}", child.admin_user_id),
        TokenScope::Agent,
        900,
    )?;
    let grandchild_token = issue_token(
        &settings,
        format!("agent:{}", grandchild.admin_user_id),
        TokenScope::Agent,
        900,
    )?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let (status, assets) = agent_get_json(
        app.clone(),
        &root_token,
        format!("/agent/api/v1/users/{child_user}/assets?limit=1"),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {assets}");
    assert_eq!(assets["total"], 2);
    assert_eq!(assets["assets"].as_array().unwrap().len(), 1);
    assert_eq!(assets["assets"][0]["account_type"], "spot");
    assert!(assets["assets"][0]["available"].is_string());
    assert!(
        assets["assets"][0]
            .as_object()
            .unwrap()
            .contains_key("logo_url")
    );
    assert_eq!(assets["assets"][0]["precision_scale"], 18);
    assert!(assets["assets"][0]["updated_at"].is_number());

    let (status, positions) = agent_get_json(
        app.clone(),
        &root_token,
        format!("/agent/api/v1/users/{child_user}/margin-positions"),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {positions}");
    assert_eq!(positions["total"], 2);
    let returned_margin_ids = positions["positions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_u64().unwrap())
        .collect::<Vec<_>>();
    assert!(returned_margin_ids.contains(&opened_margin));
    assert!(returned_margin_ids.contains(&closed_margin));
    assert!(!returned_margin_ids.contains(&other_margin));
    for position in positions["positions"].as_array().unwrap() {
        for field in [
            "margin_amount",
            "leverage",
            "notional_amount",
            "borrowed_amount",
            "interest_amount",
        ] {
            assert!(position[field].is_string(), "{field} must be Decimal text");
        }
        assert!(position["opened_at"].is_number());
        assert!(position["created_at"].is_number());
        assert!(position["closed_at"].is_null() || position["closed_at"].is_number());
    }

    let (status, opened_positions) = agent_get_json(
        app.clone(),
        &root_token,
        format!("/agent/api/v1/users/{child_user}/margin-positions?status=opened&limit=1"),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {opened_positions}");
    assert_eq!(opened_positions["total"], 1);
    assert_eq!(opened_positions["positions"][0]["id"], opened_margin);

    let (status, orders) = agent_get_json(
        app.clone(),
        &root_token,
        format!("/agent/api/v1/users/{child_user}/seconds-contract-orders?limit=2"),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {orders}");
    assert_eq!(orders["total"], 3);
    assert_eq!(orders["orders"].as_array().unwrap().len(), 2);
    for order in orders["orders"].as_array().unwrap() {
        assert!(order["stake_amount"].is_string());
        assert!(order["payout_rate"].is_string());
        assert!(order["entry_price"].is_null() || order["entry_price"].is_string());
        assert!(order["settlement_price"].is_null() || order["settlement_price"].is_string());
        assert!(order["expires_at"].is_number());
        assert!(order["created_at"].is_number());
        assert!(order["settled_at"].is_null() || order["settled_at"].is_number());
    }
    let all_statuses = agent_get_json(
        app.clone(),
        &root_token,
        format!("/agent/api/v1/users/{child_user}/seconds-contract-orders?limit=100"),
    )
    .await?
    .1["orders"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["status"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert!(all_statuses.contains(&"opened".to_owned()));
    assert!(all_statuses.contains(&"settled".to_owned()));
    assert!(all_statuses.contains(&"manual_review".to_owned()));

    let (status, review_orders) = agent_get_json(
        app.clone(),
        &root_token,
        format!(
            "/agent/api/v1/users/{child_user}/seconds-contract-orders?status=manual_review&limit=1"
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {review_orders}");
    assert_eq!(review_orders["total"], 1);
    assert_eq!(review_orders["orders"][0]["id"], review_seconds);

    let (child_own_status, _) = agent_get_json(
        app.clone(),
        &child_token,
        format!("/agent/api/v1/users/{child_user}/assets"),
    )
    .await?;
    assert_eq!(child_own_status, StatusCode::OK);
    let (grandchild_own_status, _) = agent_get_json(
        app.clone(),
        &grandchild_token,
        format!("/agent/api/v1/users/{child_user}/assets"),
    )
    .await?;
    assert_eq!(grandchild_own_status, StatusCode::OK);
    for target_user in [
        root_user,
        sibling_user,
        other_user,
        unassigned_user,
        u64::MAX,
    ] {
        for suffix in ["assets", "margin-positions", "seconds-contract-orders"] {
            let (denied, denied_payload) = agent_get_json(
                app.clone(),
                &child_token,
                format!(
                    "/agent/api/v1/users/{target_user}/{suffix}?agent_id={}",
                    root.agent_id
                ),
            )
            .await?;
            assert_eq!(denied, StatusCode::NOT_FOUND, "payload: {denied_payload}");
        }
    }

    let wallet_after: (BigDecimal, BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen, locked FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(child_user)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    let margin_wallet_after: (BigDecimal, BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen, locked FROM margin_wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(child_user)
    .bind(quote_asset)
    .fetch_one(&pool)
    .await?;
    let margin_statuses_after: Vec<(u64, String)> =
        sqlx::query_as("SELECT id, status FROM margin_positions WHERE user_id = ? ORDER BY id")
            .bind(child_user)
            .fetch_all(&pool)
            .await?;
    let seconds_statuses_after: Vec<(u64, String)> = sqlx::query_as(
        "SELECT id, status FROM seconds_contract_orders WHERE user_id = ? ORDER BY id",
    )
    .bind(child_user)
    .fetch_all(&pool)
    .await?;
    assert_eq!(wallet_after, wallet_before);
    assert_eq!(margin_wallet_after, margin_wallet_before);
    assert_eq!(margin_statuses_after, margin_statuses_before);
    assert_eq!(seconds_statuses_after, seconds_statuses_before);

    // 资产精度来自可变的数据库配置，接口必须拒绝超出 Decimal(38,18) 边界的脏值。
    sqlx::query("UPDATE assets SET precision_scale = 19 WHERE id = ?")
        .bind(quote_asset)
        .execute(&pool)
        .await?;
    let invalid_precision = agent_get_json(
        app.clone(),
        &root_token,
        format!("/agent/api/v1/users/{child_user}/assets"),
    )
    .await?;
    sqlx::query("UPDATE assets SET precision_scale = 18 WHERE id = ?")
        .bind(quote_asset)
        .execute(&pool)
        .await?;
    assert_eq!(invalid_precision.0, StatusCode::INTERNAL_SERVER_ERROR);

    for order_id in [
        opened_seconds,
        settled_seconds,
        review_seconds,
        other_seconds,
    ] {
        sqlx::query("DELETE FROM seconds_contract_orders WHERE id = ?")
            .bind(order_id)
            .execute(&pool)
            .await?;
    }
    for position_id in [opened_margin, closed_margin, other_margin] {
        sqlx::query("DELETE FROM margin_positions WHERE id = ?")
            .bind(position_id)
            .execute(&pool)
            .await?;
    }
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id IN (?, ?)")
        .bind(child_user)
        .bind(other_user)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM margin_wallet_accounts WHERE user_id IN (?, ?)")
        .bind(child_user)
        .bind(other_user)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM seconds_contract_product_cycles WHERE product_id = ?")
        .bind(seconds_product_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM seconds_contract_products WHERE id = ?")
        .bind(seconds_product_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM margin_products WHERE id = ?")
        .bind(margin_product_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM trading_pairs WHERE id = ?")
        .bind(pair_id)
        .execute(&pool)
        .await?;
    for asset_id in [base_asset, quote_asset] {
        sqlx::query("DELETE FROM assets WHERE id = ?")
            .bind(asset_id)
            .execute(&pool)
            .await?;
    }
    cleanup_agent_fixture(
        &pool,
        &[grandchild, child, sibling, root, other_root],
        &[
            root_user,
            child_user,
            sibling_user,
            other_user,
            unassigned_user,
        ],
    )
    .await?;
    Ok(())
}
