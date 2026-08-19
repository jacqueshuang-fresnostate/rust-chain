use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode},
};
use exchange_api::{
    build_router,
    config::Settings,
    modules::auth::{TokenScope, issue_token},
    state::AppState,
};
use secrecy::SecretString;
use serde_json::{Value, json};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::error::Error;
use tower::ServiceExt;
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
struct AgentFixture {
    agent_user_id: u64,
    agent_id: u64,
    agent_admin_id: u64,
}

#[derive(Debug, Clone, Copy)]
struct AdminFixture {
    role_id: u64,
    admin_id: u64,
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
            eprintln!("skipping support route test because DATABASE_URL is not set");
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

fn client_message_id(prefix: &str) -> String {
    format!("{prefix}-{}", &Uuid::now_v7().simple().to_string()[..16])
}

async fn create_user(pool: &MySqlPool, label: &str) -> u64 {
    sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(format!(
            "support-{label}-{}@example.test",
            Uuid::now_v7().simple()
        ))
        .bind("not-a-real-hash")
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id()
}

async fn create_root_agent(pool: &MySqlPool, label: &str) -> AgentFixture {
    let agent_user_id = create_user(pool, &format!("agent-owner-{label}")).await;
    let agent_id = sqlx::query("INSERT INTO agents (user_id, agent_code, path) VALUES (?, ?, '')")
        .bind(agent_user_id)
        .bind(format!("support-{label}-{}", Uuid::now_v7().simple()))
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
    let agent_admin_id = sqlx::query(
        r#"INSERT INTO agent_admin_users (agent_id, username, password_hash)
           VALUES (?, ?, ?)"#,
    )
    .bind(agent_id)
    .bind(format!("support-{label}-{}", Uuid::now_v7().simple()))
    .bind("not-a-real-hash")
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id();
    AgentFixture {
        agent_user_id,
        agent_id,
        agent_admin_id,
    }
}

async fn create_child_agent(pool: &MySqlPool, parent: AgentFixture, label: &str) -> AgentFixture {
    let (root_agent_id, level, path): (u64, i32, String) =
        sqlx::query_as("SELECT root_agent_id, level, path FROM agents WHERE id = ? LIMIT 1")
            .bind(parent.agent_id)
            .fetch_one(pool)
            .await
            .unwrap();
    let agent_user_id = create_user(pool, &format!("agent-owner-{label}")).await;
    let agent_id = sqlx::query(
        r#"INSERT INTO agents
              (user_id, parent_agent_id, root_agent_id, agent_code, level, path)
           VALUES (?, ?, ?, ?, ?, '')"#,
    )
    .bind(agent_user_id)
    .bind(parent.agent_id)
    .bind(root_agent_id)
    .bind(format!("support-{label}-{}", Uuid::now_v7().simple()))
    .bind(level + 1)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id();
    sqlx::query("UPDATE agents SET path = ? WHERE id = ?")
        .bind(format!("{path}/agent:{agent_id}"))
        .bind(agent_id)
        .execute(pool)
        .await
        .unwrap();
    let agent_admin_id = sqlx::query(
        r#"INSERT INTO agent_admin_users (agent_id, username, password_hash)
           VALUES (?, ?, ?)"#,
    )
    .bind(agent_id)
    .bind(format!("support-{label}-{}", Uuid::now_v7().simple()))
    .bind("not-a-real-hash")
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id();
    AgentFixture {
        agent_user_id,
        agent_id,
        agent_admin_id,
    }
}

async fn create_admin(pool: &MySqlPool, label: &str) -> AdminFixture {
    let role_id =
        sqlx::query("INSERT INTO admin_roles (name, permissions) VALUES (?, JSON_ARRAY('*'))")
            .bind(format!("support-{label}-{}", Uuid::now_v7().simple()))
            .execute(pool)
            .await
            .unwrap()
            .last_insert_id();
    let admin_id =
        sqlx::query("INSERT INTO admin_users (username, password_hash, role_id) VALUES (?, ?, ?)")
            .bind(format!("support-{label}-{}", Uuid::now_v7().simple()))
            .bind("not-a-real-hash")
            .bind(role_id)
            .execute(pool)
            .await
            .unwrap()
            .last_insert_id();
    AdminFixture { role_id, admin_id }
}

async fn refer_user_to_agent(pool: &MySqlPool, user_id: u64, agent_id: u64) {
    sqlx::query(
        r#"INSERT INTO user_referrals
              (user_id, direct_inviter_id, direct_inviter_type, root_agent_id, depth, path)
           VALUES (?, ?, 'agent', ?, 1, ?)"#,
    )
    .bind(user_id)
    .bind(agent_id)
    .bind(agent_id)
    .bind(format!("/{agent_id}/{agent_id}/{user_id}"))
    .execute(pool)
    .await
    .unwrap();
}

async fn refer_user_to_user(
    pool: &MySqlPool,
    user_id: u64,
    inviter_user_id: u64,
    root_agent_id: u64,
    depth: i32,
    path: String,
) {
    sqlx::query(
        r#"INSERT INTO user_referrals
              (user_id, direct_inviter_id, direct_inviter_type, root_agent_id, depth, path)
           VALUES (?, ?, 'user', ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(inviter_user_id)
    .bind(root_agent_id)
    .bind(depth)
    .bind(path)
    .execute(pool)
    .await
    .unwrap();
}

async fn request_json(
    app: &Router,
    method: Method,
    uri: impl AsRef<str>,
    token: &str,
    body: Option<Value>,
) -> Result<(StatusCode, Value), Box<dyn Error>> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri.as_ref())
        .header("authorization", format!("Bearer {token}"));
    let body = if let Some(body) = body {
        builder = builder.header("content-type", "application/json");
        Body::from(body.to_string())
    } else {
        Body::empty()
    };
    let response = app.clone().oneshot(builder.body(body)?).await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 128 * 1024).await?;
    let payload = serde_json::from_slice(&bytes)?;
    Ok((status, payload))
}

async fn cleanup_fixtures(
    pool: &MySqlPool,
    support_user_ids: &[u64],
    agents_child_first: &[AgentFixture],
    admin: Option<AdminFixture>,
) -> Result<(), sqlx::Error> {
    for user_id in support_user_ids {
        sqlx::query("DELETE FROM support_conversations WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM user_referrals WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;
    }
    if let Some(admin) = admin {
        sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
            .bind(admin.admin_id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM admin_users WHERE id = ?")
            .bind(admin.admin_id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM admin_roles WHERE id = ?")
            .bind(admin.role_id)
            .execute(pool)
            .await?;
    }
    for agent in agents_child_first {
        sqlx::query("DELETE FROM agent_admin_users WHERE id = ?")
            .bind(agent.agent_admin_id)
            .execute(pool)
            .await?;
    }
    for agent in agents_child_first {
        sqlx::query("DELETE FROM agents WHERE id = ?")
            .bind(agent.agent_id)
            .execute(pool)
            .await?;
    }
    for user_id in support_user_ids {
        sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;
    }
    for agent in agents_child_first {
        sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(agent.agent_user_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

#[tokio::test]
async fn support_routes_enforce_exact_child_owner_not_parent_subtree() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let parent = create_root_agent(&pool, "exact-parent").await;
    let child = create_child_agent(&pool, parent, "exact-child").await;
    let sibling = create_child_agent(&pool, parent, "exact-sibling").await;
    let unrelated = create_root_agent(&pool, "exact-unrelated").await;
    let user_id = create_user(&pool, "exact-user").await;
    refer_user_to_agent(&pool, user_id, child.agent_id).await;

    let user_token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900)?;
    let child_token = issue_token(
        &settings,
        format!("agent:{}", child.agent_admin_id),
        TokenScope::Agent,
        900,
    )?;
    let parent_token = issue_token(
        &settings,
        format!("agent:{}", parent.agent_admin_id),
        TokenScope::Agent,
        900,
    )?;
    let unrelated_token = issue_token(
        &settings,
        format!("agent:{}", unrelated.agent_admin_id),
        TokenScope::Agent,
        900,
    )?;
    let sibling_token = issue_token(
        &settings,
        format!("agent:{}", sibling.agent_admin_id),
        TokenScope::Agent,
        900,
    )?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let (status, sent) = request_json(
        &app,
        Method::POST,
        "/api/v1/support/conversation/messages",
        &user_token,
        Some(json!({
            "body": "我的订单需要直属代理协助",
            "client_message_id": client_message_id("exact-owner")
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {sent}");
    assert_eq!(
        sent["conversation"]["assigned_agent_id"].as_u64(),
        Some(child.agent_id)
    );
    assert_eq!(sent["conversation"]["staff_unread_count"], 1);
    let conversation_id = sent["conversation"]["id"].as_u64().unwrap();

    let (status, detail) = request_json(
        &app,
        Method::GET,
        format!("/agent/api/v1/support/conversations/{conversation_id}"),
        &child_token,
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {detail}");
    assert_eq!(detail["assigned_agent_id"], child.agent_id);

    let (status, child_queue) = request_json(
        &app,
        Method::GET,
        "/agent/api/v1/support/conversations?unread_only=true",
        &child_token,
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {child_queue}");
    assert_eq!(child_queue["total"], 1);
    assert_eq!(child_queue["conversations"][0]["id"], conversation_id);

    for token in [&parent_token, &sibling_token, &unrelated_token] {
        let (status, payload) = request_json(
            &app,
            Method::GET,
            format!("/agent/api/v1/support/conversations/{conversation_id}"),
            token,
            None,
        )
        .await?;
        assert_eq!(status, StatusCode::NOT_FOUND, "payload: {payload}");

        for (method, uri, body) in [
            (
                Method::GET,
                format!("/agent/api/v1/support/conversations/{conversation_id}/messages"),
                None,
            ),
            (
                Method::POST,
                format!("/agent/api/v1/support/conversations/{conversation_id}/messages"),
                Some(json!({
                    "body": "越权回复不应写入",
                    "client_message_id": client_message_id("denied-reply")
                })),
            ),
            (
                Method::POST,
                format!("/agent/api/v1/support/conversations/{conversation_id}/read"),
                Some(json!({ "message_id": sent["message"]["id"] })),
            ),
            (
                Method::PATCH,
                format!("/agent/api/v1/support/conversations/{conversation_id}/status"),
                Some(json!({ "status": "closed" })),
            ),
        ] {
            let (status, payload) = request_json(&app, method, uri, token, body).await?;
            assert_eq!(status, StatusCode::NOT_FOUND, "payload: {payload}");
        }

        let (status, queue) = request_json(
            &app,
            Method::GET,
            "/agent/api/v1/support/conversations",
            token,
            None,
        )
        .await?;
        assert_eq!(status, StatusCode::OK, "payload: {queue}");
        assert_eq!(queue["total"], 0);
    }

    cleanup_fixtures(
        &pool,
        &[user_id],
        &[child, sibling, parent, unrelated],
        None,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn unassigned_support_remains_admin_usable_and_preserves_message_semantics()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool, "unassigned-user").await;
    let admin = create_admin(&pool, "unassigned-admin").await;
    let user_token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900)?;
    let admin_token = issue_token(
        &settings,
        format!("admin:{}", admin.admin_id),
        TokenScope::Admin,
        900,
    )?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let (status, empty) = request_json(
        &app,
        Method::GET,
        "/api/v1/support/conversation",
        &user_token,
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {empty}");
    assert!(empty["conversation"].is_null());

    let duplicate_id = client_message_id("duplicate");
    let send_body = json!({
        "body": "无人分配时请平台管理员处理",
        "client_message_id": duplicate_id
    });
    let (status, first_send) = request_json(
        &app,
        Method::POST,
        "/api/v1/support/conversation/messages",
        &user_token,
        Some(send_body.clone()),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {first_send}");
    assert!(first_send["conversation"]["assigned_agent_id"].is_null());
    assert_eq!(first_send["conversation"]["staff_unread_count"], 1);
    assert_eq!(first_send["replayed"], false);
    let conversation_id = first_send["conversation"]["id"].as_u64().unwrap();
    let user_message_id = first_send["message"]["id"].as_u64().unwrap();

    let (status, replay) = request_json(
        &app,
        Method::POST,
        "/api/v1/support/conversation/messages",
        &user_token,
        Some(send_body),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {replay}");
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["message"]["id"], user_message_id);

    let (status, conflict) = request_json(
        &app,
        Method::POST,
        "/api/v1/support/conversation/messages",
        &user_token,
        Some(json!({
            "body": "同一客户端键不可改写正文",
            "client_message_id": duplicate_id
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::CONFLICT, "payload: {conflict}");
    let message_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM support_messages WHERE conversation_id = ?")
            .bind(conversation_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(message_count, 1);

    let (status, queue) = request_json(
        &app,
        Method::GET,
        "/admin/api/v1/support/conversations?unassigned=true&unread_only=true",
        &admin_token,
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {queue}");
    assert_eq!(queue["total"], 1);
    assert_eq!(queue["conversations"][0]["id"], conversation_id);

    let (status, staff_read) = request_json(
        &app,
        Method::POST,
        format!("/admin/api/v1/support/conversations/{conversation_id}/read"),
        &admin_token,
        Some(json!({ "message_id": user_message_id })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {staff_read}");
    assert_eq!(staff_read["staff_read_message_id"], user_message_id);
    assert_eq!(staff_read["staff_unread_count"], 0);

    let (status, admin_send) = request_json(
        &app,
        Method::POST,
        format!("/admin/api/v1/support/conversations/{conversation_id}/messages"),
        &admin_token,
        Some(json!({
            "body": "平台管理员已接入",
            "client_message_id": client_message_id("admin-reply")
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {admin_send}");
    assert_eq!(admin_send["message"]["sender_type"], "admin");
    assert_eq!(admin_send["conversation"]["user_unread_count"], 1);
    let admin_message_id = admin_send["message"]["id"].as_u64().unwrap();

    let (status, user_read) = request_json(
        &app,
        Method::POST,
        "/api/v1/support/conversation/read",
        &user_token,
        Some(json!({ "message_id": admin_message_id })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {user_read}");
    assert_eq!(user_read["user_read_message_id"], admin_message_id);
    assert_eq!(user_read["user_unread_count"], 0);

    let (status, delayed_read) = request_json(
        &app,
        Method::POST,
        "/api/v1/support/conversation/read",
        &user_token,
        Some(json!({ "message_id": user_message_id })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {delayed_read}");
    assert_eq!(delayed_read["user_read_message_id"], admin_message_id);

    let (status, newest_page) = request_json(
        &app,
        Method::GET,
        "/api/v1/support/conversation/messages?limit=1",
        &user_token,
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {newest_page}");
    assert_eq!(newest_page["messages"][0]["id"], admin_message_id);
    assert_eq!(newest_page["messages"][0]["read_by_recipient"], true);
    assert_eq!(newest_page["has_more"], true);
    assert_eq!(newest_page["next_before_id"], admin_message_id);

    let (status, older_page) = request_json(
        &app,
        Method::GET,
        format!("/api/v1/support/conversation/messages?limit=1&before_id={admin_message_id}"),
        &user_token,
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {older_page}");
    assert_eq!(older_page["messages"][0]["id"], user_message_id);
    assert_eq!(older_page["messages"][0]["read_by_recipient"], true);
    assert_eq!(older_page["has_more"], false);

    let (status, closed) = request_json(
        &app,
        Method::PATCH,
        "/api/v1/support/conversation/status",
        &user_token,
        Some(json!({ "status": "closed" })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {closed}");
    assert_eq!(closed["status"], "closed");
    assert!(!closed["closed_at"].is_null());

    let (status, reopened) = request_json(
        &app,
        Method::PATCH,
        format!("/admin/api/v1/support/conversations/{conversation_id}/status"),
        &admin_token,
        Some(json!({ "status": "open" })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {reopened}");
    assert_eq!(reopened["status"], "open");
    assert!(reopened["closed_at"].is_null());

    request_json(
        &app,
        Method::PATCH,
        "/api/v1/support/conversation/status",
        &user_token,
        Some(json!({ "status": "closed" })),
    )
    .await?;
    let (status, auto_reopened) = request_json(
        &app,
        Method::POST,
        "/api/v1/support/conversation/messages",
        &user_token,
        Some(json!({
            "body": "新消息应自动重开会话",
            "client_message_id": client_message_id("auto-reopen")
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {auto_reopened}");
    assert_eq!(auto_reopened["conversation"]["status"], "open");
    assert!(auto_reopened["conversation"]["closed_at"].is_null());
    let message_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM support_messages WHERE conversation_id = ?")
            .bind(conversation_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(message_count, 3);

    cleanup_fixtures(&pool, &[user_id], &[], Some(admin)).await?;
    Ok(())
}

#[tokio::test]
async fn admin_reassignment_moves_support_subtree_and_preserves_unrelated_conversations()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let old_agent = create_root_agent(&pool, "reassign-old").await;
    let new_agent = create_root_agent(&pool, "reassign-new").await;
    let user_id = create_user(&pool, "reassign-user").await;
    let descendant_user_id = create_user(&pool, "reassign-descendant").await;
    let unrelated_user_id = create_user(&pool, "reassign-unrelated").await;
    let admin = create_admin(&pool, "reassign-admin").await;
    refer_user_to_agent(&pool, user_id, old_agent.agent_id).await;
    let old_root_path = format!("/{}/{}/{user_id}", old_agent.agent_id, old_agent.agent_id);
    refer_user_to_user(
        &pool,
        descendant_user_id,
        user_id,
        old_agent.agent_id,
        2,
        format!("{old_root_path}/{descendant_user_id}"),
    )
    .await;
    refer_user_to_agent(&pool, unrelated_user_id, old_agent.agent_id).await;

    let user_token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900)?;
    let descendant_user_token = issue_token(
        &settings,
        format!("user:{descendant_user_id}"),
        TokenScope::User,
        900,
    )?;
    let unrelated_user_token = issue_token(
        &settings,
        format!("user:{unrelated_user_id}"),
        TokenScope::User,
        900,
    )?;
    let old_agent_token = issue_token(
        &settings,
        format!("agent:{}", old_agent.agent_admin_id),
        TokenScope::Agent,
        900,
    )?;
    let new_agent_token = issue_token(
        &settings,
        format!("agent:{}", new_agent.agent_admin_id),
        TokenScope::Agent,
        900,
    )?;
    let admin_token = issue_token(
        &settings,
        format!("admin:{}", admin.admin_id),
        TokenScope::Admin,
        900,
    )?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let (status, sent) = request_json(
        &app,
        Method::POST,
        "/api/v1/support/conversation/messages",
        &user_token,
        Some(json!({
            "body": "改派前消息",
            "client_message_id": client_message_id("reassign-before")
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {sent}");
    assert_eq!(
        sent["conversation"]["assigned_agent_id"],
        old_agent.agent_id
    );
    let conversation_id = sent["conversation"]["id"].as_u64().unwrap();
    let message_id = sent["message"]["id"].as_u64().unwrap();

    let (status, descendant_sent) = request_json(
        &app,
        Method::POST,
        "/api/v1/support/conversation/messages",
        &descendant_user_token,
        Some(json!({
            "body": "后代用户改派前消息",
            "client_message_id": client_message_id("reassign-descendant")
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {descendant_sent}");
    let descendant_conversation_id = descendant_sent["conversation"]["id"].as_u64().unwrap();
    let descendant_message_id = descendant_sent["message"]["id"].as_u64().unwrap();

    let (status, unrelated_sent) = request_json(
        &app,
        Method::POST,
        "/api/v1/support/conversation/messages",
        &unrelated_user_token,
        Some(json!({
            "body": "同代理但不在改派子树的消息",
            "client_message_id": client_message_id("reassign-unrelated")
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {unrelated_sent}");
    let unrelated_conversation_id = unrelated_sent["conversation"]["id"].as_u64().unwrap();
    let unrelated_message_id = unrelated_sent["message"]["id"].as_u64().unwrap();

    for (target_conversation_id, target_message_id) in [
        (conversation_id, message_id),
        (descendant_conversation_id, descendant_message_id),
        (unrelated_conversation_id, unrelated_message_id),
    ] {
        let (status, read) = request_json(
            &app,
            Method::POST,
            format!("/agent/api/v1/support/conversations/{target_conversation_id}/read"),
            &old_agent_token,
            Some(json!({ "message_id": target_message_id })),
        )
        .await?;
        assert_eq!(status, StatusCode::OK, "payload: {read}");
        assert_eq!(read["staff_read_message_id"], target_message_id);
    }

    let (status, assignment) = request_json(
        &app,
        Method::PATCH,
        format!("/admin/api/v1/users/{user_id}/agent"),
        &admin_token,
        Some(json!({
            "agent_id": new_agent.agent_id,
            "reason": "客服直属所有者改派测试"
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {assignment}");
    assert_eq!(assignment["root_agent_id"], new_agent.agent_id);

    let (status, old_detail) = request_json(
        &app,
        Method::GET,
        format!("/agent/api/v1/support/conversations/{conversation_id}"),
        &old_agent_token,
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "payload: {old_detail}");

    let (status, new_detail) = request_json(
        &app,
        Method::GET,
        format!("/agent/api/v1/support/conversations/{conversation_id}"),
        &new_agent_token,
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {new_detail}");
    assert_eq!(new_detail["assigned_agent_id"], new_agent.agent_id);
    assert!(new_detail["staff_read_message_id"].is_null());
    assert_eq!(new_detail["staff_unread_count"], 1);

    let (status, descendant_old_detail) = request_json(
        &app,
        Method::GET,
        format!("/agent/api/v1/support/conversations/{descendant_conversation_id}"),
        &old_agent_token,
        None,
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "payload: {descendant_old_detail}"
    );
    let (status, descendant_new_detail) = request_json(
        &app,
        Method::GET,
        format!("/agent/api/v1/support/conversations/{descendant_conversation_id}"),
        &new_agent_token,
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "payload: {descendant_new_detail}");
    assert_eq!(
        descendant_new_detail["assigned_agent_id"],
        new_agent.agent_id
    );
    assert!(descendant_new_detail["staff_read_message_id"].is_null());

    let descendant_referral: (Option<u64>, String) =
        sqlx::query_as("SELECT root_agent_id, path FROM user_referrals WHERE user_id = ? LIMIT 1")
            .bind(descendant_user_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(descendant_referral.0, Some(new_agent.agent_id));
    assert_eq!(
        descendant_referral.1,
        format!(
            "/{}/{}/{user_id}/{descendant_user_id}",
            new_agent.agent_id, new_agent.agent_id
        )
    );

    let (assigned_agent_id, staff_cursor): (Option<u64>, Option<u64>) = sqlx::query_as(
        "SELECT assigned_agent_id, staff_read_message_id FROM support_conversations WHERE id = ?",
    )
    .bind(conversation_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(assigned_agent_id, Some(new_agent.agent_id));
    assert_eq!(staff_cursor, None);

    let (descendant_agent_id, descendant_staff_cursor): (Option<u64>, Option<u64>) =
        sqlx::query_as(
            "SELECT assigned_agent_id, staff_read_message_id FROM support_conversations WHERE id = ?",
        )
        .bind(descendant_conversation_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(descendant_agent_id, Some(new_agent.agent_id));
    assert_eq!(descendant_staff_cursor, None);

    let (unrelated_agent_id, unrelated_staff_cursor): (Option<u64>, Option<u64>) = sqlx::query_as(
        "SELECT assigned_agent_id, staff_read_message_id FROM support_conversations WHERE id = ?",
    )
    .bind(unrelated_conversation_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(unrelated_agent_id, Some(old_agent.agent_id));
    assert_eq!(unrelated_staff_cursor, Some(unrelated_message_id));

    cleanup_fixtures(
        &pool,
        &[user_id, descendant_user_id, unrelated_user_id],
        &[old_agent, new_agent],
        Some(admin),
    )
    .await?;
    Ok(())
}
