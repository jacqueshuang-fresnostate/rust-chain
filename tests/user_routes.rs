use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use exchange_api::{
    config::Settings,
    modules::auth::{TokenScope, issue_token},
    state::AppState,
};
use secrecy::SecretString;
use serde_json::Value;
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
    let suffix = Uuid::now_v7().simple();
    sqlx::query(
        r#"INSERT INTO users (email, phone, password_hash, status, kyc_level, created_at)
           VALUES (?, ?, ?, 'active', 2, ?)"#,
    )
    .bind(format!("profile-{suffix}@example.test"))
    .bind(format!("188{}", &suffix.to_string()[..8]))
    .bind("not-a-real-hash")
    .bind("2026-05-30 03:16:05.123000")
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id()
}

async fn cleanup_profile_user(pool: &MySqlPool, user_id: u64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
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
async fn user_profile_route_returns_authenticated_user_with_timestamp() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_profile_user(&pool).await;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = exchange_api::build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
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
