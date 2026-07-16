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

static COUNTRY_CONFIG_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static SECURITY_POLICY_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static KYC_CONFIG_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static PLATFORM_BRAND_CONFIG_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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
        let email: Option<String> = sqlx::query_scalar("SELECT email FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .flatten();
        if let Some(email) = email {
            sqlx::query("DELETE FROM user_registration_email_verifications WHERE email = ?")
                .bind(email)
                .execute(pool)
                .await?;
        }
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
        sqlx::query("DELETE FROM user_third_party_bindings WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM user_kyc_submissions WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM user_referrals WHERE user_id = ? OR direct_inviter_id = ?")
            .bind(user_id)
            .bind(user_id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM invite_codes WHERE owner_type = 'user' AND owner_id = ?")
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

async fn upsert_test_country(
    pool: &MySqlPool,
    country_code: &str,
    country_name: &str,
    default_locale: &str,
    supported_locales: Value,
    registration_enabled: bool,
    status: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO country_configs
           (country_code, country_name, default_locale, supported_locales, registration_enabled, status, sort_order)
           VALUES (?, ?, ?, ?, ?, ?, 0)
           ON DUPLICATE KEY UPDATE country_name = VALUES(country_name),
                                   default_locale = VALUES(default_locale),
                                   supported_locales = VALUES(supported_locales),
                                   registration_enabled = VALUES(registration_enabled),
                                   status = VALUES(status)"#,
    )
    .bind(country_code)
    .bind(country_name)
    .bind(default_locale)
    .bind(sqlx::types::Json(supported_locales))
    .bind(registration_enabled)
    .bind(status)
    .execute(pool)
    .await?;
    Ok(())
}

async fn delete_test_country(pool: &MySqlPool, country_code: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE users SET country_code = NULL, preferred_locale = NULL WHERE country_code = ?",
    )
    .bind(country_code)
    .execute(pool)
    .await?;
    sqlx::query("DELETE FROM country_configs WHERE country_code = ?")
        .bind(country_code)
        .execute(pool)
        .await?;
    Ok(())
}

async fn reset_kyc_config(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO kyc_configs
           (name, enabled, target_kyc_level, required_documents_json, allowed_countries_json, country_document_types_json, max_document_size_bytes, updated_by)
           VALUES ('default', TRUE, 1, JSON_ARRAY('identity_front', 'identity_back'), JSON_ARRAY(), JSON_ARRAY(), 5242880, NULL)
           ON DUPLICATE KEY UPDATE enabled = VALUES(enabled),
                                   target_kyc_level = VALUES(target_kyc_level),
                                   required_documents_json = VALUES(required_documents_json),
                                   allowed_countries_json = VALUES(allowed_countries_json),
                                   country_document_types_json = VALUES(country_document_types_json),
                                   max_document_size_bytes = VALUES(max_document_size_bytes),
                                   updated_by = VALUES(updated_by)"#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn reset_platform_brand_config(
    pool: &MySqlPool,
    platform_name: &str,
    logo_url: Option<&str>,
    chart_provider: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO platform_brand_configs
           (name, platform_name, logo_url, chart_provider, updated_by)
           VALUES ('default', ?, ?, ?, NULL)
           ON DUPLICATE KEY UPDATE platform_name = VALUES(platform_name),
                                   logo_url = VALUES(logo_url),
                                   chart_provider = VALUES(chart_provider),
                                   updated_by = VALUES(updated_by)"#,
    )
    .bind(platform_name)
    .bind(logo_url)
    .bind(chart_provider)
    .execute(pool)
    .await?;
    Ok(())
}

async fn upsert_default_smtp_config(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO smtp_configs (name, host, port, security, from_email, enabled)
           VALUES ('default', 'smtp.example.test', 587, 'starttls', 'noreply@example.test', TRUE)
           ON DUPLICATE KEY UPDATE host = VALUES(host),
                                   port = VALUES(port),
                                   security = VALUES(security),
                                   from_email = VALUES(from_email),
                                   enabled = VALUES(enabled)"#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_registration_email_code(
    pool: &MySqlPool,
    email: &str,
    code: &str,
) -> Result<(), Box<dyn Error>> {
    let code_hash = hash_password(code)?;
    sqlx::query(
        "UPDATE user_registration_email_verifications SET status = 'superseded' WHERE email = ? AND purpose = 'register' AND status = 'pending'",
    )
    .bind(email.to_ascii_lowercase())
    .execute(pool)
    .await?;
    sqlx::query(
        r#"INSERT INTO user_registration_email_verifications
           (email, purpose, code_hash, status, expires_at, sent_at)
           VALUES (?, 'register', ?, 'pending', DATE_ADD(UTC_TIMESTAMP(6), INTERVAL 10 MINUTE), UTC_TIMESTAMP(6))"#,
    )
    .bind(email.to_ascii_lowercase())
    .bind(code_hash)
    .execute(pool)
    .await?;
    Ok(())
}

async fn reset_security_policy_invite_required(
    pool: &MySqlPool,
    required: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO security_policy_configs (policy_key, policy_value)
           VALUES ('user_security_policy', ?)
           ON DUPLICATE KEY UPDATE policy_value = VALUES(policy_value)"#,
    )
    .bind(sqlx::types::Json(json!({
        "login_2fa_mode": "user_enabled",
        "registration_invite_required": required,
        "username_login_enabled": false,
        "payment_policies": {
            "withdraw": { "enabled": true, "method": "fund_password" },
            "spot_order": { "enabled": false, "method": "fund_password" },
            "convert": { "enabled": false, "method": "fund_password" },
            "earn_subscribe": { "enabled": false, "method": "fund_password" }
        },
        "third_party_bindings": {
            "coinbase_wallet_enabled": false,
            "telegram_account_enabled": false
        }
    })))
    .execute(pool)
    .await?;
    Ok(())
}

async fn reset_security_policy_third_party_bindings(
    pool: &MySqlPool,
    coinbase_wallet_enabled: bool,
    telegram_account_enabled: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO security_policy_configs (policy_key, policy_value)
           VALUES ('user_security_policy', ?)
           ON DUPLICATE KEY UPDATE policy_value = VALUES(policy_value)"#,
    )
    .bind(sqlx::types::Json(json!({
        "login_2fa_mode": "user_enabled",
        "registration_invite_required": false,
        "username_login_enabled": false,
        "payment_policies": {
            "withdraw": { "enabled": true, "method": "fund_password" },
            "spot_order": { "enabled": false, "method": "fund_password" },
            "convert": { "enabled": false, "method": "fund_password" },
            "earn_subscribe": { "enabled": false, "method": "fund_password" }
        },
        "third_party_bindings": {
            "coinbase_wallet_enabled": coinbase_wallet_enabled,
            "telegram_account_enabled": telegram_account_enabled
        }
    })))
    .execute(pool)
    .await?;
    Ok(())
}

async fn reset_security_policy_username_login(
    pool: &MySqlPool,
    enabled: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO security_policy_configs (policy_key, policy_value)
           VALUES ('user_security_policy', ?)
           ON DUPLICATE KEY UPDATE policy_value = VALUES(policy_value)"#,
    )
    .bind(sqlx::types::Json(json!({
        "login_2fa_mode": "user_enabled",
        "registration_invite_required": false,
        "username_login_enabled": enabled,
        "payment_policies": {
            "withdraw": { "enabled": true, "method": "fund_password" },
            "spot_order": { "enabled": false, "method": "fund_password" },
            "convert": { "enabled": false, "method": "fund_password" },
            "earn_subscribe": { "enabled": false, "method": "fund_password" }
        },
        "third_party_bindings": {
            "coinbase_wallet_enabled": false,
            "telegram_account_enabled": false
        }
    })))
    .execute(pool)
    .await?;
    Ok(())
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
    let agent_id = sqlx::query("INSERT INTO agents (user_id, agent_code, path) VALUES (?, ?, '')")
        .bind(agent_user_id)
        .bind(format!("ref-agent-{}", Uuid::now_v7().simple()))
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
    (agent_user_id, agent_id)
}

async fn create_referral_child_agent(pool: &MySqlPool, parent_agent_id: u64) -> (u64, u64) {
    let (root_agent_id, parent_level, parent_path): (u64, i32, String) =
        sqlx::query_as("SELECT root_agent_id, level, path FROM agents WHERE id = ? LIMIT 1")
            .bind(parent_agent_id)
            .fetch_one(pool)
            .await
            .unwrap();
    let agent_user_id = create_referral_user(pool, "child-agent-owner").await;
    let agent_id = sqlx::query(
        r#"INSERT INTO agents
              (user_id, parent_agent_id, root_agent_id, agent_code, level, path)
           VALUES (?, ?, ?, ?, ?, '')"#,
    )
    .bind(agent_user_id)
    .bind(parent_agent_id)
    .bind(root_agent_id)
    .bind(format!("ref-child-agent-{}", Uuid::now_v7().simple()))
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
async fn public_platform_brand_returns_pc_display_config() -> Result<(), Box<dyn Error>> {
    let _guard = PLATFORM_BRAND_CONFIG_TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    reset_platform_brand_config(
        &pool,
        "Rust Chain",
        Some("https://cdn.example.test/pc-logo.png"),
        "tradingview",
    )
    .await?;
    let settings = test_settings();
    let app = exchange_api::build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/platform/brand")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let payload = body_json(response).await?;
    assert_eq!(status, StatusCode::OK, "payload: {payload}");
    assert_eq!(payload["name"], "default");
    assert_eq!(payload["platform_name"], "Rust Chain");
    assert_eq!(payload["logo_url"], "https://cdn.example.test/pc-logo.png");
    assert_eq!(payload["chart_provider"], "tradingview");

    reset_platform_brand_config(&pool, "Hippo Exchange", None, "klinecharts").await?;
    Ok(())
}

#[tokio::test]
async fn user_registration_requires_active_country_and_persists_locale()
-> Result<(), Box<dyn Error>> {
    let _guard = COUNTRY_CONFIG_TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let app = exchange_api::build_router(AppState::new(settings).with_mysql(pool.clone()));
    let suffix = Uuid::now_v7().simple().to_string();
    let email = format!("country-register-{suffix}@example.test");

    let missing = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/register",
        None,
        json!({ "email": email, "password": "RegisterPassword123!" }),
    )
    .await;
    assert_eq!(missing.status(), StatusCode::BAD_REQUEST);

    let unknown = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/register",
        None,
        json!({ "email": email, "password": "RegisterPassword123!", "country_code": "ZZ" }),
    )
    .await;
    assert_eq!(unknown.status(), StatusCode::BAD_REQUEST);

    upsert_test_country(
        &pool,
        "ZZ",
        "Unavailable Country",
        "en",
        json!(["en"]),
        false,
        "active",
    )
    .await?;
    let disabled_registration = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/register",
        None,
        json!({ "email": email, "password": "RegisterPassword123!", "country_code": "ZZ" }),
    )
    .await;
    assert_eq!(disabled_registration.status(), StatusCode::BAD_REQUEST);
    delete_test_country(&pool, "ZZ").await?;

    upsert_test_country(
        &pool,
        "ZZ",
        "Locale Country",
        "zh",
        json!(["zh", "en"]),
        true,
        "active",
    )
    .await?;
    insert_registration_email_code(&pool, &email, "123456").await?;
    let registered = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/register",
        None,
        json!({ "email": email, "password": "RegisterPassword123!", "country_code": "zz", "code": "123456" }),
    )
    .await;
    let registered_status = registered.status();
    let registered_payload = body_json(registered).await?;
    assert_eq!(
        registered_status,
        StatusCode::OK,
        "payload: {registered_payload}"
    );
    assert_eq!(registered_payload["scope"], "user");

    let stored: (
        u64,
        Option<String>,
        Option<String>,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as(
        "SELECT id, country_code, preferred_locale, email_verified_at FROM users WHERE email = ?",
    )
    .bind(&email)
    .fetch_one(&pool)
    .await?;
    assert_eq!(stored.1.as_deref(), Some("ZZ"));
    assert_eq!(stored.2.as_deref(), Some("zh"));
    assert!(stored.3.is_some());
    let invite_code: String = sqlx::query_scalar(
        "SELECT code FROM invite_codes WHERE owner_type = 'user' AND owner_id = ? LIMIT 1",
    )
    .bind(stored.0)
    .fetch_one(&pool)
    .await?;
    assert_eq!(invite_code.len(), 6);
    assert!(
        invite_code
            .chars()
            .all(|character| character.is_ascii_uppercase() || character.is_ascii_digit())
    );

    cleanup_security_users(&pool, &[stored.0]).await?;
    delete_test_country(&pool, "ZZ").await?;
    Ok(())
}

#[tokio::test]
async fn user_registration_email_code_and_invite_policy_are_enforced() -> Result<(), Box<dyn Error>>
{
    let _country_guard = COUNTRY_CONFIG_TEST_LOCK.lock().await;
    let _policy_guard = SECURITY_POLICY_TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    reset_security_policy_invite_required(&pool, false).await?;
    upsert_test_country(
        &pool,
        "RI",
        "Register Invite Country",
        "en",
        json!(["en"]),
        true,
        "active",
    )
    .await?;
    upsert_default_smtp_config(&pool).await?;

    let settings = test_settings();
    let app = exchange_api::build_router(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_email_sender(std::sync::Arc::new(NoopEmailSender)),
    );

    let initial_config = json_request(
        app.clone(),
        "GET",
        "/api/v1/auth/register/config",
        None,
        json!({}),
    )
    .await;
    let initial_payload = body_json(initial_config).await?;
    assert_eq!(initial_payload["email_code_required"], true);
    assert_eq!(initial_payload["invite_code_required"], false);

    let send_email = format!("register-send-{}@example.test", Uuid::now_v7().simple());
    let sent = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/register/email-code",
        None,
        json!({ "email": send_email }),
    )
    .await;
    let sent_status = sent.status();
    let sent_payload = body_json(sent).await?;
    assert_eq!(sent_status, StatusCode::OK, "payload: {sent_payload}");
    assert_eq!(sent_payload["sent"], true);
    assert!(
        sent_payload["expires_in_seconds"]
            .as_i64()
            .unwrap_or_default()
            > 0
    );
    let code_hash: String = sqlx::query_scalar(
        "SELECT code_hash FROM user_registration_email_verifications WHERE email = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(send_email.to_ascii_lowercase())
    .fetch_one(&pool)
    .await?;
    assert_ne!(code_hash, "123456");
    assert!(code_hash.starts_with("$argon2"));

    reset_security_policy_invite_required(&pool, true).await?;
    let required_config = json_request(
        app.clone(),
        "GET",
        "/api/v1/auth/register/config",
        None,
        json!({}),
    )
    .await;
    let required_payload = body_json(required_config).await?;
    assert_eq!(required_payload["invite_code_required"], true);

    let register_email = format!("register-invite-{}@example.test", Uuid::now_v7().simple());
    insert_registration_email_code(&pool, &register_email, "123456").await?;
    let missing_invite = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/register",
        None,
        json!({ "email": register_email, "password": "RegisterPassword123!", "country_code": "RI", "code": "123456" }),
    )
    .await;
    assert_eq!(missing_invite.status(), StatusCode::BAD_REQUEST);

    let (agent_user_id, agent_id) = create_referral_agent(&pool).await;
    let invite_code = create_agent_invite_code(&pool, agent_id, 5).await;
    let registered = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/register",
        None,
        json!({ "email": register_email, "password": "RegisterPassword123!", "country_code": "RI", "code": "123456", "invite_code": invite_code }),
    )
    .await;
    let registered_status = registered.status();
    let registered_payload = body_json(registered).await?;
    assert_eq!(
        registered_status,
        StatusCode::OK,
        "payload: {registered_payload}"
    );
    let user_id: u64 = sqlx::query_scalar("SELECT id FROM users WHERE email = ?")
        .bind(register_email.to_ascii_lowercase())
        .fetch_one(&pool)
        .await?;
    let referral: (String, u64, Option<u64>, i32) = sqlx::query_as(
        "SELECT direct_inviter_type, direct_inviter_id, root_agent_id, depth FROM user_referrals WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(referral.0, "agent");
    assert_eq!(referral.1, agent_id);
    assert_eq!(referral.2, Some(agent_id));
    assert_eq!(referral.3, 1);
    let user_invite_code: String = sqlx::query_scalar(
        "SELECT code FROM invite_codes WHERE owner_type = 'user' AND owner_id = ? LIMIT 1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;

    let referred_email = format!(
        "register-user-invite-{}@example.test",
        Uuid::now_v7().simple()
    );
    insert_registration_email_code(&pool, &referred_email, "234567").await?;
    let referred = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/register",
        None,
        json!({
            "email": &referred_email,
            "password": "RegisterPassword123!",
            "country_code": "RI",
            "code": "234567",
            "invite_code": &user_invite_code
        }),
    )
    .await;
    let referred_status = referred.status();
    let referred_payload = body_json(referred).await?;
    assert_eq!(
        referred_status,
        StatusCode::OK,
        "payload: {referred_payload}"
    );
    let referred_user_id: u64 = sqlx::query_scalar("SELECT id FROM users WHERE email = ?")
        .bind(referred_email.to_ascii_lowercase())
        .fetch_one(&pool)
        .await?;
    let referred_referral: (String, u64, Option<u64>, i32, String) = sqlx::query_as(
        "SELECT direct_inviter_type, direct_inviter_id, root_agent_id, depth, path FROM user_referrals WHERE user_id = ?",
    )
    .bind(referred_user_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(referred_referral.0, "user");
    assert_eq!(referred_referral.1, user_id);
    assert_eq!(referred_referral.2, Some(agent_id));
    assert_eq!(referred_referral.3, 2);
    assert_eq!(
        referred_referral.4,
        format!("/agent:{agent_id}/user:{user_id}/user:{referred_user_id}")
    );

    sqlx::query("UPDATE agents SET status = 'suspended' WHERE id = ?")
        .bind(agent_id)
        .execute(&pool)
        .await?;
    let blocked_email = format!(
        "register-blocked-user-invite-{}@example.test",
        Uuid::now_v7().simple()
    );
    insert_registration_email_code(&pool, &blocked_email, "345678").await?;
    let blocked = json_request(
        app,
        "POST",
        "/api/v1/auth/register",
        None,
        json!({
            "email": &blocked_email,
            "password": "RegisterPassword123!",
            "country_code": "RI",
            "code": "345678",
            "invite_code": &user_invite_code
        }),
    )
    .await;
    assert_eq!(blocked.status(), StatusCode::BAD_REQUEST);
    let blocked_user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = ?")
        .bind(blocked_email.to_ascii_lowercase())
        .fetch_one(&pool)
        .await?;
    assert_eq!(blocked_user_count, 0);
    sqlx::query("UPDATE agents SET status = 'active' WHERE id = ?")
        .bind(agent_id)
        .execute(&pool)
        .await?;

    let used_count: i32 = sqlx::query_scalar(
        "SELECT used_count FROM invite_codes WHERE owner_type = 'agent' AND owner_id = ?",
    )
    .bind(agent_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(used_count, 1);
    let user_code_used_count: i32 = sqlx::query_scalar(
        "SELECT used_count FROM invite_codes WHERE owner_type = 'user' AND owner_id = ?",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(user_code_used_count, 1);

    cleanup_security_users(&pool, &[referred_user_id, user_id]).await?;
    cleanup_referral_fixture(&pool, &[agent_user_id], &[agent_id]).await?;
    sqlx::query("DELETE FROM user_registration_email_verifications WHERE email = ?")
        .bind(send_email.to_ascii_lowercase())
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM user_registration_email_verifications WHERE email = ?")
        .bind(blocked_email.to_ascii_lowercase())
        .execute(&pool)
        .await?;
    reset_security_policy_invite_required(&pool, false).await?;
    delete_test_country(&pool, "RI").await?;
    Ok(())
}

#[tokio::test]
async fn user_login_by_username_follows_admin_policy() -> Result<(), Box<dyn Error>> {
    let _guard = SECURITY_POLICY_TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    reset_security_policy_username_login(&pool, false).await?;
    let settings = test_settings();
    let user_id = create_security_user(&pool, "username-login", "LoginPassword123!").await;
    sqlx::query("UPDATE users SET username = ? WHERE id = ?")
        .bind("policy_user")
        .bind(user_id)
        .execute(&pool)
        .await?;
    let app = exchange_api::build_router(AppState::new(settings).with_mysql(pool.clone()));

    let disabled_config = json_request(
        app.clone(),
        "GET",
        "/api/v1/auth/login/config",
        None,
        json!({}),
    )
    .await;
    let disabled_config_payload = body_json(disabled_config).await?;
    assert_eq!(disabled_config_payload["username_login_enabled"], false);

    let disabled_login = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/login",
        None,
        json!({ "username": "Policy_User", "password": "LoginPassword123!" }),
    )
    .await;
    assert_eq!(disabled_login.status(), StatusCode::BAD_REQUEST);

    reset_security_policy_username_login(&pool, true).await?;
    let enabled_config = json_request(
        app.clone(),
        "GET",
        "/api/v1/auth/login/config",
        None,
        json!({}),
    )
    .await;
    let enabled_config_payload = body_json(enabled_config).await?;
    assert_eq!(enabled_config_payload["username_login_enabled"], true);

    let enabled_login = json_request(
        app,
        "POST",
        "/api/v1/auth/login",
        None,
        json!({ "username": "Policy_User", "password": "LoginPassword123!" }),
    )
    .await;
    let enabled_login_status = enabled_login.status();
    let enabled_login_payload = body_json(enabled_login).await?;
    assert_eq!(
        enabled_login_status,
        StatusCode::OK,
        "payload: {enabled_login_payload}"
    );
    assert_eq!(enabled_login_payload["scope"], "user");

    cleanup_security_users(&pool, &[user_id]).await?;
    reset_security_policy_username_login(&pool, false).await?;
    Ok(())
}

#[tokio::test]
async fn public_countries_lists_registration_enabled_active_configs() -> Result<(), Box<dyn Error>>
{
    let _guard = COUNTRY_CONFIG_TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let app = exchange_api::build_router(AppState::new(settings).with_mysql(pool.clone()));

    upsert_test_country(
        &pool,
        "QA",
        "Public Country",
        "en",
        json!(["en", "zh"]),
        true,
        "active",
    )
    .await?;
    upsert_test_country(
        &pool,
        "QB",
        "Hidden Country",
        "zh",
        json!(["zh"]),
        false,
        "active",
    )
    .await?;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/countries")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let payload = body_json(response).await?;
    assert_eq!(status, StatusCode::OK, "payload: {payload}");
    let countries = payload["countries"].as_array().unwrap();
    let public_country = countries
        .iter()
        .find(|country| country["country_code"] == "QA")
        .expect("QA country should be public");
    assert_eq!(public_country["country_name"], "Public Country");
    assert_eq!(public_country["default_locale"], "en");
    assert_eq!(public_country["supported_locales"], json!(["en", "zh"]));
    assert!(
        countries
            .iter()
            .all(|country| country["country_code"] != "QB")
    );

    delete_test_country(&pool, "QA").await?;
    delete_test_country(&pool, "QB").await?;
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
    assert_eq!(payload["username"], Value::Null);
    assert_eq!(payload["email_verified_at"], Value::Null);
    assert_eq!(payload["fund_password_set"], false);
    assert!(payload.get("country_code").is_some());
    assert!(payload.get("preferred_locale").is_some());
    assert!(payload.get("default_locale").is_some());
    assert!(payload.get("supported_locales").is_some());

    let username_response = json_request(
        app.clone(),
        "PATCH",
        "/api/v1/user/username",
        Some(&token),
        json!({ "username": "Moon_1024" }),
    )
    .await;
    assert_eq!(username_response.status(), StatusCode::OK);
    let username_payload = body_json(username_response).await?;
    assert_eq!(username_payload["username"], "moon_1024");

    let username_profile = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/user/profile")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let username_profile_payload = body_json(username_profile).await?;
    assert_eq!(username_profile_payload["username"], "moon_1024");

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
    let verified_payload = body_json(verified_response).await?;
    assert!(verified_payload["email_verified_at"].is_number());
    assert_eq!(verified_payload["fund_password_set"], true);

    upsert_test_country(
        &pool,
        "QP",
        "Profile Country",
        "zh",
        json!(["zh", "en"]),
        true,
        "active",
    )
    .await?;
    sqlx::query("UPDATE users SET country_code = 'QP', preferred_locale = 'en' WHERE id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    let locale_response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/user/profile")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let locale_payload = body_json(locale_response).await?;
    assert_eq!(locale_payload["country_code"], "QP");
    assert_eq!(locale_payload["preferred_locale"], "en");
    assert_eq!(locale_payload["default_locale"], "zh");
    assert_eq!(locale_payload["supported_locales"], json!(["zh", "en"]));

    cleanup_profile_user(&pool, user_id).await?;
    delete_test_country(&pool, "QP").await?;
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
async fn user_third_party_bindings_follow_admin_policy() -> Result<(), Box<dyn Error>> {
    let _guard = SECURITY_POLICY_TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    reset_security_policy_third_party_bindings(&pool, false, false).await?;
    let settings = test_settings();
    let user_id = create_security_user(&pool, "third-party", "LoginPassword123!").await;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = exchange_api::build_router(AppState::new(settings).with_mysql(pool.clone()));

    let two_factor = json_request(
        app.clone(),
        "GET",
        "/api/v1/user/2fa",
        Some(&token),
        json!({}),
    )
    .await;
    let two_factor_payload = body_json(two_factor).await?;
    assert_eq!(
        two_factor_payload["third_party_bindings"]["coinbase_wallet_enabled"],
        false
    );
    assert_eq!(
        two_factor_payload["third_party_bindings"]["telegram_account_enabled"],
        false
    );

    let disabled = json_request(
        app.clone(),
        "POST",
        "/api/v1/user/third-party-bindings",
        Some(&token),
        json!({
            "provider": "coinbase_wallet",
            "account_identifier": "0x1234567890abcdef",
            "display_name": "Primary Coinbase"
        }),
    )
    .await;
    let disabled_status = disabled.status();
    let disabled_payload = body_json(disabled).await?;
    assert_eq!(disabled_status, StatusCode::FORBIDDEN);
    assert_eq!(disabled_payload["code"], "third_party_binding_disabled");

    reset_security_policy_third_party_bindings(&pool, true, false).await?;
    let initial = json_request(
        app.clone(),
        "GET",
        "/api/v1/user/third-party-bindings",
        Some(&token),
        json!({}),
    )
    .await;
    let initial_payload = body_json(initial).await?;
    assert_eq!(initial_payload["policy"]["coinbase_wallet_enabled"], true);
    assert_eq!(initial_payload["policy"]["telegram_account_enabled"], false);
    assert_eq!(initial_payload["bindings"], json!([]));

    let bound = json_request(
        app.clone(),
        "POST",
        "/api/v1/user/third-party-bindings",
        Some(&token),
        json!({
            "provider": "coinbase_wallet",
            "account_identifier": "0x1234567890abcdef",
            "display_name": "Primary Coinbase"
        }),
    )
    .await;
    let bound_status = bound.status();
    let bound_payload = body_json(bound).await?;
    assert_eq!(bound_status, StatusCode::OK, "payload: {bound_payload}");
    assert_eq!(bound_payload["bindings"][0]["provider"], "coinbase_wallet");
    assert_eq!(
        bound_payload["bindings"][0]["account_identifier"],
        "0x1234567890abcdef"
    );
    assert_eq!(
        bound_payload["bindings"][0]["display_name"],
        "Primary Coinbase"
    );
    assert_eq!(bound_payload["bindings"][0]["status"], "bound");
    assert!(bound_payload["bindings"][0]["created_at"].is_number());

    let telegram_disabled = json_request(
        app,
        "POST",
        "/api/v1/user/third-party-bindings",
        Some(&token),
        json!({
            "provider": "telegram_account",
            "account_identifier": "@coin_user"
        }),
    )
    .await;
    assert_eq!(telegram_disabled.status(), StatusCode::FORBIDDEN);

    cleanup_security_users(&pool, &[user_id]).await?;
    reset_security_policy_third_party_bindings(&pool, false, false).await?;
    Ok(())
}

#[tokio::test]
async fn user_kyc_status_and_submission_create_pending_review() -> Result<(), Box<dyn Error>> {
    let _guard = KYC_CONFIG_TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    reset_kyc_config(&pool).await?;
    let settings = test_settings();
    let user_id = create_security_user(&pool, "kyc", "LoginPassword123!").await;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = exchange_api::build_router(AppState::new(settings).with_mysql(pool.clone()));

    let status_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/user/kyc")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let status_payload = body_json(status_response).await?;
    assert_eq!(status_payload["config"]["enabled"], true);
    assert_eq!(
        status_payload["config"]["required_documents"],
        json!(["identity_front", "identity_back"])
    );
    assert_eq!(
        status_payload["config"]["country_document_types"],
        json!([])
    );
    assert_eq!(status_payload["latest_submission"], Value::Null);

    sqlx::query(
        r#"UPDATE kyc_configs
           SET country_document_types_json = JSON_ARRAY(JSON_OBJECT('country', 'China', 'document_types', JSON_ARRAY('passport'), 'handheld_document_types', JSON_ARRAY('passport')))
           WHERE name = 'default'"#,
    )
    .execute(&pool)
    .await?;

    let invalid_document_response = json_request(
        app.clone(),
        "POST",
        "/api/v1/user/kyc/submissions",
        Some(&token),
        json!({
            "real_name": "Zhang San",
            "country": "China",
            "id_number": "CN1234567890",
            "document_type": "identity_card",
            "document_front_image": "data:image/png;base64,front",
            "document_back_image": "data:image/png;base64,back"
        }),
    )
    .await;
    assert_eq!(invalid_document_response.status(), StatusCode::BAD_REQUEST);

    let missing_handheld_response = json_request(
        app.clone(),
        "POST",
        "/api/v1/user/kyc/submissions",
        Some(&token),
        json!({
            "real_name": "Zhang San",
            "country": "China",
            "id_number": "CN1234567890",
            "document_type": "passport",
            "document_front_image": "data:image/png;base64,front",
            "document_back_image": "data:image/png;base64,back"
        }),
    )
    .await;
    assert_eq!(missing_handheld_response.status(), StatusCode::BAD_REQUEST);

    let submit_response = json_request(
        app.clone(),
        "POST",
        "/api/v1/user/kyc/submissions",
        Some(&token),
        json!({
            "real_name": "Zhang San",
            "country": "China",
            "id_number": "CN1234567890",
            "document_type": "passport",
            "document_front_image": "data:image/png;base64,front",
            "document_back_image": "data:image/png;base64,back",
            "document_handheld_image": "data:image/png;base64,handheld"
        }),
    )
    .await;
    let submit_status = submit_response.status();
    let submit_payload = body_json(submit_response).await?;
    assert_eq!(submit_status, StatusCode::OK, "payload: {submit_payload}");
    assert_eq!(submit_payload["user_id"], user_id);
    assert_eq!(submit_payload["status"], "pending");
    assert_eq!(submit_payload["target_kyc_level"], 1);
    assert_eq!(submit_payload["real_name"], "Zhang San");
    assert_eq!(submit_payload["document_type"], "passport");
    assert_eq!(
        submit_payload["document_handheld_image"],
        "data:image/png;base64,handheld"
    );

    let latest_response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/user/kyc")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let latest_payload = body_json(latest_response).await?;
    assert_eq!(latest_payload["latest_submission"]["status"], "pending");
    assert_eq!(
        latest_payload["latest_submission"]["id"],
        submit_payload["id"]
    );

    cleanup_security_users(&pool, &[user_id]).await?;
    Ok(())
}

#[tokio::test]
async fn user_kyc_enterprise_submission_requires_enterprise_fields() -> Result<(), Box<dyn Error>> {
    let _guard = KYC_CONFIG_TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    reset_kyc_config(&pool).await?;
    let settings = test_settings();
    let user_id = create_security_user(&pool, "kyc-enterprise", "LoginPassword123!").await;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = exchange_api::build_router(AppState::new(settings).with_mysql(pool.clone()));

    let missing_enterprise_name = json_request(
        app.clone(),
        "POST",
        "/api/v1/user/kyc/submissions",
        Some(&token),
        json!({
            "real_name": "Acme Holdings",
            "submission_type": "enterprise",
            "country": "China",
            "id_number": "CN9999999999",
            "document_type": "identity_card",
            "document_front_image": "data:image/png;base64,front",
            "document_back_image": "data:image/png;base64,back",
            "document_handheld_image": "data:image/png;base64,handheld",
            "business_registration_number": "91310000712345678A"
        }),
    )
    .await;
    assert_eq!(missing_enterprise_name.status(), StatusCode::BAD_REQUEST);

    let missing_business_registration_number = json_request(
        app.clone(),
        "POST",
        "/api/v1/user/kyc/submissions",
        Some(&token),
        json!({
            "real_name": "Acme Holdings",
            "submission_type": "enterprise",
            "country": "China",
            "id_number": "CN9999999999",
            "enterprise_name": "Acme Holdings Ltd",
            "document_type": "identity_card",
            "document_front_image": "data:image/png;base64,front",
            "document_back_image": "data:image/png;base64,back",
            "document_handheld_image": "data:image/png;base64,handheld"
        }),
    )
    .await;
    assert_eq!(
        missing_business_registration_number.status(),
        StatusCode::BAD_REQUEST
    );

    let submit_response = json_request(
        app.clone(),
        "POST",
        "/api/v1/user/kyc/submissions",
        Some(&token),
        json!({
            "real_name": "Acme Holdings",
            "submission_type": "enterprise",
            "enterprise_name": "Acme Holdings Ltd",
            "business_registration_number": "91310000712345678A",
            "country": "China",
            "id_number": "CN9999999999",
            "document_type": "identity_card",
            "document_front_image": "data:image/png;base64,front",
            "document_back_image": "data:image/png;base64,back"
        }),
    )
    .await;
    assert_eq!(submit_response.status(), StatusCode::OK);

    let submit_payload = body_json(submit_response).await?;
    assert_eq!(submit_payload["submission_type"], "enterprise");
    assert_eq!(submit_payload["enterprise_name"], "Acme Holdings Ltd");
    assert_eq!(
        submit_payload["business_registration_number"],
        "91310000712345678A"
    );

    let latest_response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/user/kyc")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let latest_payload = body_json(latest_response).await?;
    assert_eq!(
        latest_payload["latest_submission"]["submission_type"],
        "enterprise"
    );
    assert_eq!(
        latest_payload["latest_submission"]["enterprise_name"],
        "Acme Holdings Ltd"
    );

    cleanup_security_users(&pool, &[user_id]).await?;
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
    let _guard = COUNTRY_CONFIG_TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let email = format!("password-change-{}@example.test", Uuid::now_v7().simple());
    upsert_test_country(
        &pool,
        "PW",
        "Password Country",
        "en",
        json!(["en"]),
        true,
        "active",
    )
    .await?;
    insert_registration_email_code(&pool, &email, "123456").await?;
    let app = exchange_api::build_router(AppState::new(settings).with_mysql(pool.clone()));

    let register = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/register",
        None,
        json!({ "email": email, "password": "OldPassword123!", "country_code": "PW", "code": "123456" }),
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
    delete_test_country(&pool, "PW").await?;
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
async fn user_security_fund_password_reset_uses_email_code() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_security_user(&pool, "fund-reset", "LoginPassword123!").await;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = exchange_api::build_router(AppState::new(settings.clone()).with_mysql(pool.clone()));

    sqlx::query("UPDATE users SET email_verified_at = CURRENT_TIMESTAMP(6) WHERE id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO user_security (user_id, fund_password_hash) VALUES (?, ?)")
        .bind(user_id)
        .bind(hash_password("123456").unwrap())
        .execute(&pool)
        .await?;

    let unconfigured = json_request(
        app.clone(),
        "POST",
        "/api/v1/user/fund-password/reset-code",
        Some(&token),
        json!({}),
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
        app.clone(),
        "POST",
        "/api/v1/user/fund-password/reset-code",
        Some(&token),
        json!({}),
    )
    .await;
    let sent_status = sent.status();
    let sent_payload = body_json(sent).await?;
    assert_eq!(sent_status, StatusCode::OK, "payload: {sent_payload}");
    assert_eq!(sent_payload["sent"], true);
    assert!(sent_payload["expires_at"].is_number());

    let purpose: String = sqlx::query_scalar(
        "SELECT purpose FROM user_email_verifications WHERE user_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(purpose, "fund_password_reset");

    let wrong = json_request(
        app.clone(),
        "POST",
        "/api/v1/user/fund-password/reset",
        Some(&token),
        json!({ "code": "000000", "new_fund_password": "234567" }),
    )
    .await;
    assert_eq!(wrong.status(), StatusCode::BAD_REQUEST);

    let code_hash = hash_password("654321").unwrap();
    sqlx::query(
        r#"INSERT INTO user_email_verifications
           (user_id, email, purpose, code_hash, status, expires_at, sent_at)
           SELECT id, email, 'fund_password_reset', ?, 'pending', DATE_ADD(CURRENT_TIMESTAMP(6), INTERVAL 10 MINUTE), CURRENT_TIMESTAMP(6)
           FROM users WHERE id = ?"#,
    )
    .bind(code_hash)
    .bind(user_id)
    .execute(&pool)
    .await?;

    let reset = json_request(
        app,
        "POST",
        "/api/v1/user/fund-password/reset",
        Some(&token),
        json!({ "code": "654321", "new_fund_password": "234567" }),
    )
    .await;
    let reset_status = reset.status();
    let reset_payload = body_json(reset).await?;
    assert_eq!(reset_status, StatusCode::OK, "payload: {reset_payload}");
    assert_eq!(reset_payload["fund_password_set"], true);

    let changed_hash: String =
        sqlx::query_scalar("SELECT fund_password_hash FROM user_security WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&pool)
            .await?;
    assert!(verify_password(&changed_hash, "234567")?);
    let verification_status: String = sqlx::query_scalar(
        "SELECT status FROM user_email_verifications WHERE user_id = ? AND purpose = 'fund_password_reset' ORDER BY id DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(verification_status, "verified");

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
    let blocked_user_id = create_referral_user(&pool, "blocked-child").await;
    let (agent_user_id, root_agent_id) = create_referral_agent(&pool).await;
    let (child_agent_user_id, agent_id) = create_referral_child_agent(&pool, root_agent_id).await;
    let code = create_agent_invite_code(&pool, agent_id, 3).await;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let child_token = issue_token(
        &settings,
        format!("user:{child_user_id}"),
        TokenScope::User,
        900,
    )
    .unwrap();
    let blocked_token = issue_token(
        &settings,
        format!("user:{blocked_user_id}"),
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
    assert_eq!(user_invite_code.len(), 6);
    assert!(
        user_invite_code
            .chars()
            .all(|char| char.is_ascii_uppercase() || char.is_ascii_digit())
    );
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
        .clone()
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

    sqlx::query("UPDATE agents SET status = 'suspended' WHERE id = ?")
        .bind(root_agent_id)
        .execute(&pool)
        .await?;
    let blocked_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/referral/bind")
                .header("authorization", format!("Bearer {blocked_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(r#"{{"code":"{user_invite_code}"}}"#)))
                .unwrap(),
        )
        .await?;
    assert_eq!(blocked_response.status(), StatusCode::BAD_REQUEST);
    let blocked_referral_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM user_referrals WHERE user_id = ?")
            .bind(blocked_user_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(blocked_referral_count, 0);
    sqlx::query("UPDATE agents SET status = 'active' WHERE id = ?")
        .bind(root_agent_id)
        .execute(&pool)
        .await?;

    cleanup_referral_fixture(
        &pool,
        &[
            blocked_user_id,
            child_user_id,
            user_id,
            child_agent_user_id,
            agent_user_id,
        ],
        &[agent_id, root_agent_id],
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn user_referral_my_code_repairs_legacy_invalid_user_code() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_referral_user(&pool, "legacy-code").await;
    let legacy_code = format!("legacy-{}", Uuid::now_v7().simple());
    let invite_code_id = sqlx::query(
        r#"INSERT INTO invite_codes (owner_type, owner_id, code, status)
           VALUES ('user', ?, ?, 'active')"#,
    )
    .bind(user_id)
    .bind(&legacy_code)
    .execute(&pool)
    .await?
    .last_insert_id();
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = exchange_api::build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/referral/my-code")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let payload = body_json(response).await?;
    assert_eq!(status, StatusCode::OK, "payload: {payload}");
    assert_eq!(payload["id"], invite_code_id);
    let repaired_code = payload["code"].as_str().unwrap();
    assert_ne!(repaired_code, legacy_code);
    assert_eq!(repaired_code.len(), 6);
    assert!(
        repaired_code
            .chars()
            .all(|char| char.is_ascii_uppercase() || char.is_ascii_digit())
    );

    let stored_code: String = sqlx::query_scalar("SELECT code FROM invite_codes WHERE id = ?")
        .bind(invite_code_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(stored_code, repaired_code);

    cleanup_referral_fixture(&pool, &[user_id], &[]).await?;
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
