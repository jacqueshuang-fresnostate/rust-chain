use axum::{
    Router, async_trait,
    body::Body,
    http::{Request, StatusCode},
};
use exchange_api::{
    config::Settings,
    error::AppResult,
    infra::email::{EmailMessage, EmailSender, SmtpEmailConfig},
    modules::auth::{TokenScope, hash_password, issue_token, verify_password},
    state::AppState,
};
use secrecy::SecretString;
use serde_json::{Value, json};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::error::Error;
use tower::ServiceExt;
use uuid::Uuid;

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
    }
}

async fn mysql_pool() -> Option<MySqlPool> {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!("skipping MySQL route integration test because DATABASE_URL is not set");
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

async fn create_profile_user(pool: &MySqlPool) -> u64 {
    let suffix = Uuid::now_v7().simple().to_string();
    sqlx::query(
        r#"INSERT INTO users (email, phone, password_hash, status, kyc_level, created_at)
           VALUES (?, ?, ?, 'active', 2, ?)"#,
    )
    .bind(format!("profile-{suffix}@example.test"))
    .bind(format!("188{}", &suffix[16..27]))
    .bind("not-a-real-hash")
    .bind("2026-05-30 03:16:05.123000")
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id()
}

async fn create_security_user(pool: &MySqlPool, label: &str, password: &str) -> u64 {
    let suffix = Uuid::now_v7().simple().to_string();
    sqlx::query("INSERT INTO users (email, phone, password_hash) VALUES (?, ?, ?)")
        .bind(format!("security-{label}-{suffix}@example.test"))
        .bind(format!("177{}", &suffix[16..27]))
        .bind(hash_password(password).unwrap())
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id()
}

async fn cleanup_security_users(pool: &MySqlPool, user_ids: &[u64]) -> Result<(), sqlx::Error> {
    for user_id in user_ids {
        sqlx::query("DELETE FROM audit_events WHERE actor_type = 'user' AND actor_id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM user_email_verifications WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM refresh_tokens WHERE actor_type = 'user' AND actor_id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM user_security WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM smtp_configs WHERE name = 'default'")
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

async fn cleanup_profile_user(pool: &MySqlPool, user_id: u64) -> Result<(), sqlx::Error> {
    cleanup_security_users(pool, &[user_id]).await
}

async fn body_json(response: axum::response::Response) -> Result<Value, Box<dyn Error>> {
    let body = axum::body::to_bytes(response.into_body(), 65_536).await?;
    Ok(serde_json::from_slice(&body)?)
}

#[derive(Debug)]
struct NoopEmailSender;

#[async_trait]
impl EmailSender for NoopEmailSender {
    async fn send(&self, _config: SmtpEmailConfig, _message: EmailMessage) -> AppResult<()> {
        Ok(())
    }
}

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

async fn json_request(
    app: Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Value,
) -> axum::response::Response {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("authorization", bearer(token));
    }

    app.oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}

async fn create_referral_user(pool: &MySqlPool, label: &str) -> u64 {
    let suffix = Uuid::now_v7().simple();
    sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(format!("referral-{label}-{suffix}@example.test"))
        .bind("not-a-real-hash")
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id()
}

async fn create_referral_agent(pool: &MySqlPool) -> (u64, u64) {
    let agent_user_id = create_referral_user(pool, "agent-owner").await;
    let agent_id = sqlx::query("INSERT INTO agents (user_id, agent_code) VALUES (?, ?)")
        .bind(agent_user_id)
        .bind(format!("ref-agent-{}", Uuid::now_v7().simple()))
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id();
    (agent_user_id, agent_id)
}

async fn create_agent_invite_code(pool: &MySqlPool, agent_id: u64, usage_limit: i32) -> String {
    let code = format!("agent-bind-{}", Uuid::now_v7().simple());
    sqlx::query(
        r#"INSERT INTO invite_codes (owner_type, owner_id, code, usage_limit, status)
           VALUES ('agent', ?, ?, ?, 'active')"#,
    )
    .bind(agent_id)
    .bind(&code)
    .bind(usage_limit)
    .execute(pool)
    .await
    .unwrap();
    code
}

async fn cleanup_referral_fixture(
    pool: &MySqlPool,
    user_ids: &[u64],
    agent_ids: &[u64],
) -> Result<(), sqlx::Error> {
    for user_id in user_ids {
        sqlx::query("DELETE FROM user_referrals WHERE user_id = ? OR direct_inviter_id = ?")
            .bind(user_id)
            .bind(user_id)
            .execute(pool)
            .await?;
    }
    for user_id in user_ids {
        sqlx::query("DELETE FROM invite_codes WHERE owner_type = 'user' AND owner_id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;
    }
    for agent_id in agent_ids {
        sqlx::query("DELETE FROM invite_codes WHERE owner_type = 'agent' AND owner_id = ?")
            .bind(agent_id)
            .execute(pool)
            .await?;
    }
    for agent_id in agent_ids {
        sqlx::query("DELETE FROM agents WHERE id = ?")
            .bind(agent_id)
            .execute(pool)
            .await?;
    }
    for user_id in user_ids {
        sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

#[tokio::test]
async fn user_security_profile_route_returns_authenticated_user_with_timestamp()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_profile_user(&pool).await;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = exchange_api::build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/user/profile")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 8192).await?;

    assert_eq!(
        status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );
    let payload: Value = serde_json::from_slice(&body)?;
    assert_eq!(payload["id"], user_id);
    assert!(payload["email"].as_str().unwrap().starts_with("profile-"));
    assert!(payload["phone"].as_str().unwrap().starts_with("188"));
    assert_eq!(payload["status"], "active");
    assert_eq!(payload["kyc_level"], 2);
    assert_eq!(payload["created_at"], 1_780_110_965_123_i64);
    assert!(payload["created_at"].is_number());
    assert_eq!(payload["email_verified_at"], Value::Null);
    assert_eq!(payload["fund_password_set"], false);

    sqlx::query("UPDATE users SET email_verified_at = ? WHERE id = ?")
        .bind("2026-05-30 04:00:00.000000")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO user_security (user_id, fund_password_hash) VALUES (?, ?)")
        .bind(user_id)
        .bind(hash_password("123456").unwrap())
        .execute(&pool)
        .await?;

    let verified_response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/user/profile")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let verified_payload = body_json(verified_response).await?;
    assert!(verified_payload["email_verified_at"].is_number());
    assert_eq!(verified_payload["fund_password_set"], true);

    cleanup_profile_user(&pool, user_id).await?;
    Ok(())
}

#[tokio::test]
async fn user_profile_route_rejects_non_user_tokens() -> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = exchange_api::build_router(AppState::new(settings));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/user/profile")
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    Ok(())
}

#[tokio::test]
async fn user_security_email_bind_code_requires_user_scope_and_records_hashed_code()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_security_user(&pool, "bind-code", "OldPassword123!").await;
    let user_token =
        issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = exchange_api::build_router(AppState::new(settings.clone()).with_mysql(pool.clone()));

    let body = json!({ "email": format!("bind-{}@example.test", Uuid::now_v7().simple()) });
    let missing = json_request(
        app.clone(),
        "POST",
        "/api/v1/user/email/bind-code",
        None,
        body.clone(),
    )
    .await;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let admin = json_request(
        app.clone(),
        "POST",
        "/api/v1/user/email/bind-code",
        Some(&admin_token),
        body.clone(),
    )
    .await;
    assert_eq!(admin.status(), StatusCode::FORBIDDEN);

    let unconfigured = json_request(
        app.clone(),
        "POST",
        "/api/v1/user/email/bind-code",
        Some(&user_token),
        body.clone(),
    )
    .await;
    assert_eq!(unconfigured.status(), StatusCode::INTERNAL_SERVER_ERROR);

    sqlx::query(
        r#"INSERT INTO smtp_configs (name, host, port, security, from_email, enabled)
           VALUES ('default', 'smtp.example.test', 587, 'starttls', 'noreply@example.test', TRUE)
           ON DUPLICATE KEY UPDATE host = VALUES(host), port = VALUES(port), security = VALUES(security), from_email = VALUES(from_email), enabled = VALUES(enabled)"#,
    )
    .execute(&pool)
    .await?;
    let app = exchange_api::build_router(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_email_sender(std::sync::Arc::new(NoopEmailSender)),
    );

    let sent = json_request(
        app,
        "POST",
        "/api/v1/user/email/bind-code",
        Some(&user_token),
        body,
    )
    .await;
    let sent_status = sent.status();
    let sent_payload = body_json(sent).await?;
    assert_eq!(sent_status, StatusCode::OK, "payload: {sent_payload}");
    assert_eq!(sent_payload["sent"], true);
    assert!(sent_payload["expires_at"].is_number());
    assert!(sent_payload.get("code").is_none());

    let code_hash: String = sqlx::query_scalar(
        "SELECT code_hash FROM user_email_verifications WHERE user_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;
    assert_ne!(code_hash, "123456");
    assert!(code_hash.starts_with("$argon2"));

    cleanup_security_users(&pool, &[user_id]).await?;
    Ok(())
}

#[tokio::test]
async fn user_security_email_bind_verifies_code_and_marks_email_verified()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_security_user(&pool, "bind", "OldPassword123!").await;
    let other_user_id = create_security_user(&pool, "bind-other", "OldPassword123!").await;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = exchange_api::build_router(AppState::new(settings).with_mysql(pool.clone()));
    let email = format!("verified-{}@example.test", Uuid::now_v7().simple());
    let duplicate_email = format!("duplicate-{}@example.test", Uuid::now_v7().simple());
    sqlx::query("UPDATE users SET email = ? WHERE id = ?")
        .bind(&duplicate_email)
        .bind(other_user_id)
        .execute(&pool)
        .await?;
    sqlx::query(
        r#"INSERT INTO user_email_verifications
           (user_id, email, purpose, code_hash, status, expires_at, sent_at)
           VALUES (?, ?, 'bind', ?, 'pending', DATE_ADD(CURRENT_TIMESTAMP(6), INTERVAL 10 MINUTE), CURRENT_TIMESTAMP(6))"#,
    )
    .bind(user_id)
    .bind(&email)
    .bind(hash_password("654321").unwrap())
    .execute(&pool)
    .await?;

    let wrong = json_request(
        app.clone(),
        "POST",
        "/api/v1/user/email/bind",
        Some(&token),
        json!({ "email": email, "code": "000000" }),
    )
    .await;
    assert_eq!(wrong.status(), StatusCode::BAD_REQUEST);
    let attempts_after_wrong: i32 = sqlx::query_scalar(
        "SELECT attempt_count FROM user_email_verifications WHERE user_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(attempts_after_wrong, 1);

    sqlx::query("UPDATE users SET status = 'disabled' WHERE id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    let disabled = json_request(
        app.clone(),
        "POST",
        "/api/v1/user/email/bind",
        Some(&token),
        json!({ "email": email, "code": "654321" }),
    )
    .await;
    assert_eq!(disabled.status(), StatusCode::UNAUTHORIZED);
    sqlx::query("UPDATE users SET status = 'active' WHERE id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;

    let duplicate = json_request(
        app.clone(),
        "POST",
        "/api/v1/user/email/bind",
        Some(&token),
        json!({ "email": duplicate_email, "code": "654321" }),
    )
    .await;
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);

    let bound = json_request(
        app,
        "POST",
        "/api/v1/user/email/bind",
        Some(&token),
        json!({ "email": email, "code": "654321" }),
    )
    .await;
    let bound_status = bound.status();
    let bound_payload = body_json(bound).await?;
    assert_eq!(bound_status, StatusCode::OK, "payload: {bound_payload}");
    assert_eq!(bound_payload["email"], email);
    assert!(bound_payload["email_verified_at"].is_number());

    let stored: (String, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as("SELECT email, email_verified_at FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(stored.0, email);
    assert!(stored.1.is_some());
    let verification_status: String = sqlx::query_scalar(
        "SELECT status FROM user_email_verifications WHERE user_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(verification_status, "verified");

    cleanup_security_users(&pool, &[user_id, other_user_id]).await?;
    Ok(())
}

#[tokio::test]
async fn user_security_password_change_requires_old_password_and_revokes_refresh_tokens()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let email = format!("password-change-{}@example.test", Uuid::now_v7().simple());
    let app = exchange_api::build_router(AppState::new(settings).with_mysql(pool.clone()));

    let register = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/register",
        None,
        json!({ "email": email, "password": "OldPassword123!" }),
    )
    .await;
    let register_payload = body_json(register).await?;
    let token = register_payload["access_token"]
        .as_str()
        .unwrap()
        .to_owned();
    let old_refresh = register_payload["refresh_token"]
        .as_str()
        .unwrap()
        .to_owned();
    let user_id: u64 = sqlx::query_scalar("SELECT id FROM users WHERE email = ?")
        .bind(&email)
        .fetch_one(&pool)
        .await?;

    let wrong = json_request(
        app.clone(),
        "PATCH",
        "/api/v1/user/password",
        Some(&token),
        json!({ "old_password": "WrongPassword123!", "new_password": "NewPassword123!" }),
    )
    .await;
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);

    let changed = json_request(
        app.clone(),
        "PATCH",
        "/api/v1/user/password",
        Some(&token),
        json!({ "old_password": "OldPassword123!", "new_password": "NewPassword123!" }),
    )
    .await;
    let changed_status = changed.status();
    let changed_payload = body_json(changed).await?;
    assert_eq!(changed_status, StatusCode::OK, "payload: {changed_payload}");
    assert_eq!(changed_payload["scope"], "user");
    assert!(changed_payload["access_token"].is_string());
    assert!(changed_payload["refresh_token"].is_string());

    let old_login = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/login",
        None,
        json!({ "email": email, "password": "OldPassword123!" }),
    )
    .await;
    assert_eq!(old_login.status(), StatusCode::UNAUTHORIZED);

    let new_login = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/login",
        None,
        json!({ "email": email, "password": "NewPassword123!" }),
    )
    .await;
    assert_eq!(new_login.status(), StatusCode::OK);

    let old_refresh_response = json_request(
        app,
        "POST",
        "/api/v1/auth/refresh",
        None,
        json!({ "refresh_token": old_refresh }),
    )
    .await;
    assert_eq!(old_refresh_response.status(), StatusCode::UNAUTHORIZED);

    cleanup_security_users(&pool, &[user_id]).await?;
    Ok(())
}

#[tokio::test]
async fn user_security_fund_password_create_and_change_enforce_password_rules()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_security_user(&pool, "fund", "LoginPassword123!").await;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = exchange_api::build_router(AppState::new(settings).with_mysql(pool.clone()));

    let invalid = json_request(
        app.clone(),
        "POST",
        "/api/v1/user/fund-password",
        Some(&token),
        json!({ "login_password": "LoginPassword123!", "fund_password": "abcdef" }),
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);

    let wrong_login = json_request(
        app.clone(),
        "POST",
        "/api/v1/user/fund-password",
        Some(&token),
        json!({ "login_password": "WrongPassword123!", "fund_password": "123456" }),
    )
    .await;
    assert_eq!(wrong_login.status(), StatusCode::UNAUTHORIZED);

    let created = json_request(
        app.clone(),
        "POST",
        "/api/v1/user/fund-password",
        Some(&token),
        json!({ "login_password": "LoginPassword123!", "fund_password": "123456" }),
    )
    .await;
    let created_status = created.status();
    let created_payload = body_json(created).await?;
    assert_eq!(created_status, StatusCode::OK, "payload: {created_payload}");
    assert_eq!(created_payload["fund_password_set"], true);

    let stored_hash: String =
        sqlx::query_scalar("SELECT fund_password_hash FROM user_security WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&pool)
            .await?;
    assert_ne!(stored_hash, "123456");
    assert!(verify_password(&stored_hash, "123456")?);

    let duplicate = json_request(
        app.clone(),
        "POST",
        "/api/v1/user/fund-password",
        Some(&token),
        json!({ "login_password": "LoginPassword123!", "fund_password": "234567" }),
    )
    .await;
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);

    let wrong_old = json_request(
        app.clone(),
        "PATCH",
        "/api/v1/user/fund-password",
        Some(&token),
        json!({ "old_fund_password": "000000", "new_fund_password": "234567" }),
    )
    .await;
    assert_eq!(wrong_old.status(), StatusCode::UNAUTHORIZED);

    let changed = json_request(
        app,
        "PATCH",
        "/api/v1/user/fund-password",
        Some(&token),
        json!({ "old_fund_password": "123456", "new_fund_password": "234567" }),
    )
    .await;
    let changed_status = changed.status();
    let changed_payload = body_json(changed).await?;
    assert_eq!(changed_status, StatusCode::OK, "payload: {changed_payload}");
    assert_eq!(changed_payload["fund_password_set"], true);

    let changed_hash: String =
        sqlx::query_scalar("SELECT fund_password_hash FROM user_security WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&pool)
            .await?;
    assert!(verify_password(&changed_hash, "234567")?);

    cleanup_security_users(&pool, &[user_id]).await?;
    Ok(())
}

#[tokio::test]
async fn user_referral_routes_bind_agent_code_and_return_invites() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_referral_user(&pool, "invitee").await;
    let child_user_id = create_referral_user(&pool, "child").await;
    let (agent_user_id, agent_id) = create_referral_agent(&pool).await;
    let code = create_agent_invite_code(&pool, agent_id, 3).await;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let child_token = issue_token(
        &settings,
        format!("user:{child_user_id}"),
        TokenScope::User,
        900,
    )
    .unwrap();
    let app = exchange_api::build_router(AppState::new(settings).with_mysql(pool.clone()));

    let bind_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/referral/bind")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(r#"{{"code":"{code}"}}"#)))
                .unwrap(),
        )
        .await
        .unwrap();
    let bind_status = bind_response.status();
    let bind_body = axum::body::to_bytes(bind_response.into_body(), 8192).await?;
    assert_eq!(
        bind_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&bind_body)
    );
    let bound: Value = serde_json::from_slice(&bind_body)?;
    assert_eq!(bound["bound"], true);
    assert_eq!(bound["direct_inviter_type"], "agent");
    assert_eq!(bound["direct_inviter_id"], agent_id);
    assert_eq!(bound["root_agent_id"], agent_id);
    assert_eq!(bound["depth"], 1);
    assert!(bound["created_at"].is_number());

    let my_code_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/referral/my-code")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(my_code_response.status(), StatusCode::OK);
    let my_code_body = axum::body::to_bytes(my_code_response.into_body(), 8192).await?;
    let my_code: Value = serde_json::from_slice(&my_code_body)?;
    let user_invite_code = my_code["code"].as_str().unwrap().to_owned();
    assert_eq!(my_code["owner_type"], "user");
    assert_eq!(my_code["owner_id"], user_id);
    assert_eq!(my_code["root_agent_id"], agent_id);
    assert!(my_code["created_at"].is_number());

    let child_bind_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/referral/bind")
                .header("authorization", format!("Bearer {child_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(r#"{{"code":"{user_invite_code}"}}"#)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(child_bind_response.status(), StatusCode::OK);

    let invites_response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/referral/my-invites")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invites_response.status(), StatusCode::OK);
    let invites_body = axum::body::to_bytes(invites_response.into_body(), 8192).await?;
    let invites: Value = serde_json::from_slice(&invites_body)?;
    let users = invites["users"].as_array().unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0]["user_id"], child_user_id);
    assert_eq!(users[0]["direct_inviter_type"], "user");
    assert_eq!(users[0]["direct_inviter_id"], user_id);
    assert_eq!(users[0]["root_agent_id"], agent_id);
    assert_eq!(users[0]["depth"], 2);
    assert!(users[0]["created_at"].is_number());

    cleanup_referral_fixture(&pool, &[child_user_id, user_id, agent_user_id], &[agent_id]).await?;
    Ok(())
}

#[tokio::test]
async fn user_referral_bind_rejects_inactive_and_exhausted_codes() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_referral_user(&pool, "blocked").await;
    let (agent_user_id, agent_id) = create_referral_agent(&pool).await;
    let disabled_code = format!("disabled-{}", Uuid::now_v7().simple());
    let exhausted_code = format!("exhausted-{}", Uuid::now_v7().simple());
    sqlx::query(
        r#"INSERT INTO invite_codes (owner_type, owner_id, code, usage_limit, used_count, status)
           VALUES ('agent', ?, ?, 10, 0, 'disabled'), ('agent', ?, ?, 1, 1, 'active')"#,
    )
    .bind(agent_id)
    .bind(&disabled_code)
    .bind(agent_id)
    .bind(&exhausted_code)
    .execute(&pool)
    .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = exchange_api::build_router(AppState::new(settings).with_mysql(pool.clone()));

    for code in [disabled_code, exhausted_code] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/referral/bind")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(format!(r#"{{"code":"{code}"}}"#)))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    cleanup_referral_fixture(&pool, &[user_id, agent_user_id], &[agent_id]).await?;
    Ok(())
}

#[tokio::test]
async fn user_referral_bind_rejects_disabled_agent_codes() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_referral_user(&pool, "disabled-agent").await;
    let (agent_user_id, agent_id) = create_referral_agent(&pool).await;
    let code = create_agent_invite_code(&pool, agent_id, 3).await;
    sqlx::query("UPDATE agents SET status = 'disabled' WHERE id = ?")
        .bind(agent_id)
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = exchange_api::build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/referral/bind")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(r#"{{"code":"{code}"}}"#)))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    cleanup_referral_fixture(&pool, &[user_id, agent_user_id], &[agent_id]).await?;
    Ok(())
}
