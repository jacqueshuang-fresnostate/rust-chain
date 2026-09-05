use axum::{
    async_trait,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header::AUTHORIZATION},
};
use bigdecimal::BigDecimal;
use chrono::TimeZone;
use exchange_api::{
    build_router,
    config::Settings,
    error::AppResult,
    infra::{
        email::{EmailMessage, EmailSender, SmtpEmailConfig},
        secrets::encrypt_secret,
    },
    modules::{
        auth::{TokenScope, hash_password, issue_token, verify_password},
        quick_recharge::gmpay_signature,
        security::{TOTP_DIGITS, TOTP_STEP_SECONDS, base32_decode_no_padding, totp_code_for_time},
    },
    state::AppState,
    workers::market_feed::MarketFeedSupervisorHandle,
};
use secrecy::SecretString;
use serde_json::{Value, json};
use sqlx::{MySqlPool, migrate::MigrateError, mysql::MySqlPoolOptions, types::Json as SqlxJson};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    str::FromStr,
    sync::Arc,
};
use tower::ServiceExt;
use uuid::Uuid;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_string_contains, header, method, path},
};

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
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

static SMTP_CONFIG_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static MARKET_FEED_CONFIG_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static UPLOAD_CONFIG_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static COUNTRY_CONFIG_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static SECURITY_POLICY_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static KYC_CONFIG_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static PLATFORM_BRAND_CONFIG_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static QUICK_RECHARGE_CONFIG_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Debug)]
struct RecordingEmailSender {
    pool: MySqlPool,
    admin_id: u64,
}

#[async_trait]
impl EmailSender for RecordingEmailSender {
    async fn send(&self, _config: SmtpEmailConfig, _message: EmailMessage) -> AppResult<()> {
        let audits = sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*) FROM admin_audit_logs
               WHERE admin_id = ? AND action = 'smtp_config.test'"#,
        )
        .bind(self.admin_id)
        .fetch_one(&self.pool)
        .await?;
        assert!(audits > 0);
        Ok(())
    }
}

async fn mysql_pool() -> Option<MySqlPool> {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!("skipping MySQL admin route test because DATABASE_URL is not set");
            return None;
        }
    };

    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .unwrap();
    // 集成测试可复用已按当前 0102 SQL 完成迁移的本地容器。
    // SQLx 会在迁移文件后续完善过期间报校验和不一致；表结构由独立 0102 真实迁移测试校验。
    match sqlx::migrate!("./migrations").run(&pool).await {
        Ok(()) | Err(MigrateError::VersionMismatch(102)) => {}
        Err(error) => panic!("admin route test migrations failed: {error}"),
    }
    Some(pool)
}

async fn create_user(pool: &MySqlPool) -> u64 {
    let email = format!("admin-route-user-{}@example.test", Uuid::now_v7().simple());
    create_user_with_email(pool, email).await
}

async fn create_user_with_email(pool: &MySqlPool, email: String) -> u64 {
    sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(email)
        .bind("not-a-real-hash")
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id()
}

async fn create_user_without_email(pool: &MySqlPool) -> u64 {
    sqlx::query("INSERT INTO users (password_hash) VALUES (?)")
        .bind("not-a-real-hash")
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id()
}

async fn create_user_with_login_password(pool: &MySqlPool, password: &str) -> (u64, String) {
    let email = format!("admin-route-login-{}@example.test", Uuid::now_v7().simple());
    let user_id = sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(&email)
        .bind(hash_password(password).unwrap())
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id();

    (user_id, email)
}

async fn create_asset(pool: &MySqlPool, prefix: &str) -> u64 {
    create_asset_with_symbol(pool, prefix).await.0
}

async fn create_asset_with_symbol(pool: &MySqlPool, prefix: &str) -> (u64, String) {
    let suffix = Uuid::now_v7().simple().to_string();
    let symbol = format!("{}{}", prefix, &suffix[..10]).to_ascii_uppercase();
    let asset_id = sqlx::query(
        "INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, 18, 'coin', 'active')",
    )
    .bind(&symbol)
    .bind(&symbol)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id();
    (asset_id, symbol)
}

async fn upsert_deposit_network_config(pool: &MySqlPool, network: &str, group_code: &str) {
    sqlx::query(
        r#"INSERT INTO deposit_network_configs
           (network, display_name, address_group_code, address_group_name, asset_symbols_json, status, sort_order)
           VALUES (?, ?, ?, ?, NULL, 'active', 0)
           ON DUPLICATE KEY UPDATE
             display_name = VALUES(display_name),
             address_group_code = VALUES(address_group_code),
             address_group_name = VALUES(address_group_name),
             asset_symbols_json = NULL,
             status = 'active'"#,
    )
    .bind(network)
    .bind(network)
    .bind(group_code)
    .bind(group_code)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_convert_pair(pool: &MySqlPool, from_asset: u64, to_asset: u64, enabled: bool) -> u64 {
    sqlx::query(
        r#"INSERT INTO convert_pairs
           (from_asset, to_asset, pricing_mode, spread_rate, min_amount, max_amount, enabled)
           VALUES (?, ?, 'fixed', 0, 1, NULL, ?)"#,
    )
    .bind(from_asset)
    .bind(to_asset)
    .bind(enabled)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id()
}

async fn seed_convert_order(
    pool: &MySqlPool,
    user_id: u64,
    pair_id: u64,
    from_asset: u64,
    to_asset: u64,
    status: &str,
) -> String {
    let quote_id = Uuid::now_v7().to_string();
    sqlx::query(
        r#"INSERT INTO convert_orders
           (quote_id, convert_pair_id, user_id, from_asset, to_asset, from_amount, to_amount, rate, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&quote_id)
    .bind(pair_id)
    .bind(user_id)
    .bind(from_asset)
    .bind(to_asset)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("20.000000000000000000"))
    .bind(decimal("2.000000000000000000"))
    .bind(status)
    .execute(pool)
    .await
    .unwrap();
    quote_id
}

#[derive(Debug)]
struct AdminMarginLiquidationFixture {
    record_id: u64,
    base_asset: u64,
    margin_asset: u64,
    pair_id: u64,
    symbol: String,
    product_id: u64,
    position_id: u64,
}

async fn seed_margin_liquidation_record(
    pool: &MySqlPool,
    user_id: u64,
    prefix: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> AdminMarginLiquidationFixture {
    let (base_asset, base_symbol) = create_asset_with_symbol(pool, &format!("{prefix}B")).await;
    let (margin_asset, quote_symbol) = create_asset_with_symbol(pool, &format!("{prefix}Q")).await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, 18, 18, ?, 'active', 'external')"#,
    )
    .bind(base_asset)
    .bind(margin_asset)
    .bind(&symbol)
    .bind(decimal("1.000000000000000000"))
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id();
    let product_id = sqlx::query(
        r#"INSERT INTO margin_products
           (pair_id, margin_asset, margin_mode, margin_modes, leverage_levels, max_leverage, min_margin, max_margin, maintenance_margin_rate, status)
           VALUES (?, ?, 'isolated', JSON_ARRAY('isolated'), ?, ?, ?, ?, ?, 'active')"#,
    )
    .bind(pair_id)
    .bind(margin_asset)
    .bind(SqlxJson(vec!["2".to_owned(), "5".to_owned()]))
    .bind(decimal("5.00000000"))
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("1000.000000000000000000"))
    .bind(decimal("0.05000000"))
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id();
    let position_id = sqlx::query(
        r#"INSERT INTO margin_positions
           (user_id, product_id, pair_id, margin_asset, margin_mode, direction, margin_amount,
            leverage, notional_amount, entry_price, status, idempotency_key)
           VALUES (?, ?, ?, ?, 'isolated', 'long', ?, ?, ?, ?, 'liquidated', ?)"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(margin_asset)
    .bind(decimal("20.000000000000000000"))
    .bind(decimal("5.00000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(format!(
        "admin-margin-liquidation-{}",
        Uuid::now_v7().simple()
    ))
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id();
    let record_id = sqlx::query(
        r#"INSERT INTO margin_liquidation_records
           (position_id, user_id, product_id, pair_id, margin_asset, direction, margin_amount,
            notional_amount, interest_amount, entry_price, mark_price, maintenance_margin_rate, equity,
            maintenance_margin, realized_pnl, payout_amount, reason, liquidated_at)
           VALUES (?, ?, ?, ?, ?, 'long', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'maintenance_margin', ?)"#,
    )
    .bind(position_id)
    .bind(user_id)
    .bind(product_id)
    .bind(pair_id)
    .bind(margin_asset)
    .bind(decimal("20.000000000000000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(decimal("1.250000000000000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(decimal("84.000000000000000000"))
    .bind(decimal("0.05000000"))
    .bind(decimal("2.750000000000000000"))
    .bind(decimal("5.000000000000000000"))
    .bind(decimal("-16.000000000000000000"))
    .bind(decimal("2.750000000000000000"))
    .bind(now.naive_utc())
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id();

    AdminMarginLiquidationFixture {
        record_id,
        base_asset,
        margin_asset,
        pair_id,
        symbol,
        product_id,
        position_id,
    }
}

async fn create_admin_user(pool: &MySqlPool) -> (u64, u64) {
    let suffix = Uuid::now_v7().simple().to_string();
    let role_id =
        sqlx::query("INSERT INTO admin_roles (name, permissions) VALUES (?, JSON_ARRAY('*'))")
            .bind(format!("admin-route-role-{suffix}"))
            .execute(pool)
            .await
            .unwrap()
            .last_insert_id();
    let admin_id =
        sqlx::query("INSERT INTO admin_users (username, password_hash, role_id) VALUES (?, ?, ?)")
            .bind(format!("admin-route-user-{suffix}"))
            .bind("not-a-real-hash")
            .bind(role_id)
            .execute(pool)
            .await
            .unwrap()
            .last_insert_id();

    (role_id, admin_id)
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

async fn reset_platform_brand_config(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO platform_brand_configs
           (name, platform_name, logo_url, chart_provider, updated_by)
           VALUES ('default', 'Hippo Exchange', NULL, 'klinecharts', NULL)
           ON DUPLICATE KEY UPDATE platform_name = VALUES(platform_name),
                                   logo_url = VALUES(logo_url),
                                   chart_provider = VALUES(chart_provider),
                                   updated_by = VALUES(updated_by)"#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn delete_pair_fixture(
    pool: &MySqlPool,
    pair_id: u64,
    from_asset: u64,
    to_asset: u64,
    admin_id: u64,
    role_id: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(admin_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM convert_pairs WHERE id = ?")
        .bind(pair_id)
        .execute(pool)
        .await?;
    for asset_id in [from_asset, to_asset] {
        sqlx::query("DELETE FROM assets WHERE id = ?")
            .bind(asset_id)
            .execute(pool)
            .await?;
    }
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(role_id)
        .execute(pool)
        .await?;
    Ok(())
}

async fn delete_pair_and_assets(
    pool: &MySqlPool,
    pair_id: u64,
    from_asset: u64,
    to_asset: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM convert_pairs WHERE id = ?")
        .bind(pair_id)
        .execute(pool)
        .await?;
    for asset_id in [from_asset, to_asset] {
        sqlx::query("DELETE FROM assets WHERE id = ?")
            .bind(asset_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

#[tokio::test]
async fn bootstrap_admin_is_gated_until_password_rotation_and_old_session_is_revoked()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let (role_id, admin_id, username) = create_login_admin(&pool).await;
    sqlx::query("UPDATE admin_users SET must_change_password = TRUE WHERE id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;

    let settings = test_settings();
    let auth_manager = exchange_api::infra::auth::memory_manager(&settings);
    let app = build_router(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_auth_manager(auth_manager),
    );
    let login = app
        .clone()
        .oneshot(admin_login_request(&username, LOGIN_LOCKOUT_TEST_PASSWORD))
        .await?;
    assert_eq!(login.status(), StatusCode::OK);
    let login_payload = body_json(login).await?;
    let old_token = login_payload["access_token"].as_str().unwrap().to_owned();
    let old_refresh_token = login_payload["refresh_token"].as_str().unwrap().to_owned();

    let me = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/access/me")
                .header(AUTHORIZATION, format!("Bearer {old_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(me.status(), StatusCode::OK);
    assert_eq!(body_json(me).await?["must_change_password"], true);

    let blocked = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/access/permissions")
                .header(AUTHORIZATION, format!("Bearer {old_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(blocked.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        body_json(blocked).await?["code"],
        "ADMIN_PASSWORD_CHANGE_REQUIRED"
    );

    let blocked_two_factor = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/auth/2fa")
                .header(AUTHORIZATION, format!("Bearer {old_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(blocked_two_factor.status(), StatusCode::FORBIDDEN);

    // 聚合路由层也必须拦截未显式挂 AdminAuth 提取器的历史管理读路由。
    // 这条回归用例防止新增/存量 handler 遗漏提取器时绕过首次改密闸门。
    let blocked_route_without_extractor = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/loan/products?limit=1")
                .header(AUTHORIZATION, format!("Bearer {old_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(
        blocked_route_without_extractor.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        body_json(blocked_route_without_extractor).await?["code"],
        "ADMIN_PASSWORD_CHANGE_REQUIRED"
    );

    // 刷新端点不经 AdminAuth 提取器，因此必须在主体回表处独立拦住强制改密账号。
    let blocked_refresh = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/auth/refresh")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "refresh_token": old_refresh_token.clone() }).to_string(),
                ))?,
        )
        .await?;
    assert_eq!(blocked_refresh.status(), StatusCode::UNAUTHORIZED);

    let new_password = "RotatedPass456!";
    let changed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/auth/password")
                .header(AUTHORIZATION, format!("Bearer {old_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "current_password": LOGIN_LOCKOUT_TEST_PASSWORD,
                        "new_password": new_password,
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(changed.status(), StatusCode::OK);
    assert_eq!(body_json(changed).await?["requires_relogin"], true);

    let old_session = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/access/me")
                .header(AUTHORIZATION, format!("Bearer {old_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(old_session.status(), StatusCode::UNAUTHORIZED);
    let revoked_refresh = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/auth/refresh")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "refresh_token": old_refresh_token }).to_string(),
                ))?,
        )
        .await?;
    assert_eq!(revoked_refresh.status(), StatusCode::UNAUTHORIZED);

    let (must_change, password_hash): (bool, String) =
        sqlx::query_as("SELECT must_change_password, password_hash FROM admin_users WHERE id = ?")
            .bind(admin_id)
            .fetch_one(&pool)
            .await?;
    assert!(!must_change);
    assert!(verify_password(&password_hash, new_password)?);

    let old_login = app
        .clone()
        .oneshot(admin_login_request(&username, LOGIN_LOCKOUT_TEST_PASSWORD))
        .await?;
    assert_eq!(old_login.status(), StatusCode::UNAUTHORIZED);
    let new_login = app
        .clone()
        .oneshot(admin_login_request(&username, new_password))
        .await?;
    assert_eq!(new_login.status(), StatusCode::OK);
    let new_token = body_json(new_login).await?["access_token"]
        .as_str()
        .unwrap()
        .to_owned();
    let allowed = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/access/permissions")
                .header(AUTHORIZATION, format!("Bearer {new_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(allowed.status(), StatusCode::OK);

    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM admin_audit_logs WHERE admin_id = ? AND action = 'admin.password.change'",
    )
    .bind(admin_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(audit_count, 1);

    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    delete_login_admin(&pool, role_id, admin_id, &username).await?;
    Ok(())
}

async fn delete_rule_fixture(
    pool: &MySqlPool,
    pair_id: u64,
    from_asset: u64,
    to_asset: u64,
    admin_id: u64,
    role_id: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM admin_audit_logs WHERE admin_id = ? AND target_type = 'new_coin_convert_rule'",
    )
    .bind(admin_id)
    .execute(pool)
    .await?;
    sqlx::query("DELETE FROM new_coin_convert_rules WHERE convert_pair_id = ?")
        .bind(pair_id)
        .execute(pool)
        .await?;
    delete_pair_fixture(pool, pair_id, from_asset, to_asset, admin_id, role_id).await
}

async fn delete_new_coin_project_fixture(
    pool: &MySqlPool,
    project_id: u64,
    asset_id: u64,
    admin_id: u64,
    role_id: u64,
) -> Result<(), sqlx::Error> {
    delete_new_coin_project_fixture_with_pairs(pool, project_id, asset_id, &[], admin_id, role_id)
        .await
}

async fn delete_new_coin_project_fixture_with_pairs(
    pool: &MySqlPool,
    project_id: u64,
    asset_id: u64,
    pair_ids: &[u64],
    admin_id: u64,
    role_id: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM admin_audit_logs WHERE admin_id = ? AND target_type = 'new_coin_project'",
    )
    .bind(admin_id)
    .execute(pool)
    .await?;
    sqlx::query("DELETE FROM new_coin_lifecycle_events WHERE project_id = ?")
        .bind(project_id)
        .execute(pool)
        .await?;
    sqlx::query("UPDATE new_coin_projects SET post_listing_pair_id = NULL WHERE id = ?")
        .bind(project_id)
        .execute(pool)
        .await?;
    for pair_id in pair_ids {
        sqlx::query("DELETE FROM trading_pairs WHERE id = ?")
            .bind(pair_id)
            .execute(pool)
            .await?;
    }
    sqlx::query("DELETE FROM new_coin_projects WHERE id = ?")
        .bind(project_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(asset_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(role_id)
        .execute(pool)
        .await?;
    Ok(())
}

async fn delete_new_coin_distribution_fixture(
    pool: &MySqlPool,
    project_id: u64,
    asset_id: u64,
    user_id: u64,
    admin_id: u64,
    role_id: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM wallet_ledger WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM new_coin_distributions WHERE project_id = ?")
        .bind(project_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM asset_unlock_records WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE sources FROM asset_lock_position_sources sources INNER JOIN asset_lock_positions positions ON positions.id = sources.lock_position_id WHERE positions.user_id = ? AND positions.asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM asset_lock_positions WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM new_coin_lifecycle_events WHERE project_id = ?")
        .bind(project_id)
        .execute(pool)
        .await?;
    sqlx::query(
        "DELETE FROM admin_audit_logs WHERE admin_id = ? AND target_type = 'new_coin_distribution'",
    )
    .bind(admin_id)
    .execute(pool)
    .await?;
    sqlx::query("DELETE FROM new_coin_projects WHERE id = ?")
        .bind(project_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(asset_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(role_id)
        .execute(pool)
        .await?;
    Ok(())
}

async fn delete_order_fixture(
    pool: &MySqlPool,
    pair_id: u64,
    from_asset: u64,
    to_asset: u64,
    user_ids: &[u64],
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM convert_orders WHERE convert_pair_id = ?")
        .bind(pair_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM convert_pairs WHERE id = ?")
        .bind(pair_id)
        .execute(pool)
        .await?;
    for asset_id in [from_asset, to_asset] {
        sqlx::query("DELETE FROM assets WHERE id = ?")
            .bind(asset_id)
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

async fn delete_margin_liquidation_fixture(
    pool: &MySqlPool,
    fixtures: &[AdminMarginLiquidationFixture],
    user_ids: &[u64],
) -> Result<(), sqlx::Error> {
    for fixture in fixtures {
        sqlx::query("DELETE FROM margin_liquidation_records WHERE id = ?")
            .bind(fixture.record_id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM margin_positions WHERE id = ?")
            .bind(fixture.position_id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM margin_products WHERE id = ?")
            .bind(fixture.product_id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM trading_pairs WHERE id = ?")
            .bind(fixture.pair_id)
            .execute(pool)
            .await?;
        for asset_id in [fixture.base_asset, fixture.margin_asset] {
            sqlx::query("DELETE FROM assets WHERE id = ?")
                .bind(asset_id)
                .execute(pool)
                .await?;
        }
    }
    for user_id in user_ids {
        sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

#[derive(sqlx::FromRow, Debug)]
struct AdminAuditRow {
    action: String,
    target_type: String,
    target_id: String,
    before_json: Option<Value>,
    after_json: Option<Value>,
    reason: Option<String>,
}

struct AgentCommissionSeed<'a> {
    agent_id: u64,
    user_id: u64,
    source_type: &'a str,
    source_id: &'a str,
    source_amount: &'a str,
    commission_amount: &'a str,
    status: &'a str,
}

async fn seed_agent_commission(
    pool: &MySqlPool,
    agent_id: u64,
    user_id: u64,
    source_type: &str,
    source_amount: &str,
    commission_amount: &str,
    status: &str,
) -> u64 {
    let source_id = format!("admin-seeded-{}", Uuid::now_v7());
    seed_agent_commission_with_source_id(
        pool,
        AgentCommissionSeed {
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

async fn seed_agent_commission_with_source_id(
    pool: &MySqlPool,
    seed: AgentCommissionSeed<'_>,
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
    .bind(decimal(seed.source_amount))
    .bind(decimal(seed.commission_amount))
    .bind(seed.status)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id()
}

async fn delete_admin_agent_management_fixture(
    pool: &MySqlPool,
    admin_id: u64,
    role_id: u64,
    agent_ids: &[u64],
    user_ids: &[u64],
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(admin_id)
        .execute(pool)
        .await?;
    for agent_id in agent_ids {
        sqlx::query("DELETE FROM agent_commission_records WHERE agent_id = ?")
            .bind(agent_id)
            .execute(pool)
            .await?;
    }
    for user_id in user_ids {
        sqlx::query("DELETE FROM user_referrals WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;
    }
    for agent_id in agent_ids {
        sqlx::query("DELETE FROM agent_admin_users WHERE agent_id = ?")
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
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(role_id)
        .execute(pool)
        .await?;
    Ok(())
}

async fn body_json(response: axum::response::Response) -> Result<Value, Box<dyn Error>> {
    let body = to_bytes(response.into_body(), 65_536).await?;
    Ok(serde_json::from_slice(&body)?)
}

fn form_body_value(body: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    body.split('&')
        .find_map(|part| part.strip_prefix(&prefix))
        .map(|value| value.replace('+', " "))
}

#[tokio::test]
async fn admin_security_policy_routes_are_registered_after_auth() {
    let settings = test_settings();
    let token = issue_token(&settings, "admin:42", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));

    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/security-policy")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let patch_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/security-policy")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "login_2fa_mode": "mandatory",
                        "registration_invite_required": true,
                        "payment_policies": {
                            "withdraw": {"enabled": true, "method": "fund_password_and_two_factor"},
                            "spot_order": {"enabled": true, "method": "two_factor"},
                            "convert": {"enabled": false, "method": "fund_password"},
                            "earn_subscribe": {"enabled": false, "method": "fund_password"}
                        },
                        "reason": "enable stronger security policy"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch_response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let reset_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/users/42/2fa/reset")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"reason": "user lost authenticator"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reset_response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn admin_security_policy_crud_and_reset_two_factor_audit() -> Result<(), Box<dyn Error>> {
    let _guard = SECURITY_POLICY_TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let key = settings.exposed_credential_encryption_key().unwrap();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let user_id = create_user(&pool).await;
    let encrypted_secret = encrypt_secret("JBSWY3DPEHPK3PXP", key)?;
    sqlx::query(
        r#"INSERT INTO user_two_factor_settings
           (user_id, totp_secret_encrypted, totp_enabled, login_2fa_enabled, confirmed_at, last_verified_at)
           VALUES (?, ?, TRUE, TRUE, CURRENT_TIMESTAMP(6), CURRENT_TIMESTAMP(6))
           ON DUPLICATE KEY UPDATE
              totp_secret_encrypted = VALUES(totp_secret_encrypted),
              totp_enabled = TRUE,
              login_2fa_enabled = TRUE,
              confirmed_at = CURRENT_TIMESTAMP(6),
              last_verified_at = CURRENT_TIMESTAMP(6)"#,
    )
    .bind(user_id)
    .bind(encrypted_secret)
    .execute(&pool)
    .await?;

    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    sqlx::query(
        r#"INSERT INTO security_policy_configs (policy_key, policy_value, updated_by)
           VALUES ('user_security_policy', JSON_OBJECT(
               'login_2fa_mode', 'user_enabled',
               'registration_invite_required', FALSE,
               'username_login_enabled', FALSE,
               'payment_policies', JSON_OBJECT(
                   'withdraw', JSON_OBJECT('enabled', TRUE, 'method', 'fund_password'),
                   'spot_order', JSON_OBJECT('enabled', FALSE, 'method', 'fund_password'),
                   'convert', JSON_OBJECT('enabled', FALSE, 'method', 'fund_password'),
                   'earn_subscribe', JSON_OBJECT('enabled', FALSE, 'method', 'fund_password')
               ),
               'third_party_bindings', JSON_OBJECT(
                   'coinbase_wallet_enabled', FALSE,
                   'telegram_account_enabled', FALSE
               )
           ), ?)
           ON DUPLICATE KEY UPDATE
               policy_value = VALUES(policy_value),
               updated_by = VALUES(updated_by)"#,
    )
    .bind(admin_id)
    .execute(&pool)
    .await?;

    let current = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/security-policy")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(current.status(), StatusCode::OK);
    let current_payload = body_json(current).await?;
    assert_eq!(current_payload["login_2fa_mode"], "user_enabled");
    assert_eq!(current_payload["registration_invite_required"], false);
    assert_eq!(current_payload["username_login_enabled"], false);
    assert_eq!(
        current_payload["payment_policies"]["withdraw"]["method"],
        "fund_password"
    );
    assert_eq!(
        current_payload["third_party_bindings"]["coinbase_wallet_enabled"],
        false
    );
    assert_eq!(
        current_payload["third_party_bindings"]["telegram_account_enabled"],
        false
    );

    let missing_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/security-policy")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "login_2fa_mode": "mandatory",
                        "registration_invite_required": true,
                        "username_login_enabled": true,
                        "payment_policies": {
                            "withdraw": {"enabled": true, "method": "fund_password_and_two_factor"},
                            "spot_order": {"enabled": true, "method": "two_factor"},
                            "convert": {"enabled": false, "method": "fund_password"},
                            "earn_subscribe": {"enabled": false, "method": "fund_password"}
                        },
                        "third_party_bindings": {
                            "coinbase_wallet_enabled": true,
                            "telegram_account_enabled": false
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_reason.status(), StatusCode::BAD_REQUEST);
    let missing_reason_payload = body_json(missing_reason).await?;
    assert_eq!(
        missing_reason_payload["message"],
        "validation error: reason is required"
    );

    let updated = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/security-policy")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "login_2fa_mode": "mandatory",
                        "registration_invite_required": true,
                        "username_login_enabled": true,
                        "payment_policies": {
                            "withdraw": {"enabled": true, "method": "fund_password_and_two_factor"},
                            "spot_order": {"enabled": true, "method": "two_factor"},
                            "convert": {"enabled": false, "method": "fund_password"},
                            "earn_subscribe": {"enabled": false, "method": "fund_password"}
                        },
                        "third_party_bindings": {
                            "coinbase_wallet_enabled": true,
                            "telegram_account_enabled": true
                        },
                        "reason": "enable stronger security policy"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(updated.status(), StatusCode::OK);
    let updated_payload = body_json(updated).await?;
    assert_eq!(updated_payload["login_2fa_mode"], "mandatory");
    assert_eq!(updated_payload["registration_invite_required"], true);
    assert_eq!(updated_payload["username_login_enabled"], true);
    assert_eq!(
        updated_payload["payment_policies"]["withdraw"]["method"],
        "fund_password_and_two_factor"
    );
    assert_eq!(
        updated_payload["payment_policies"]["spot_order"]["method"],
        "two_factor"
    );
    assert_eq!(
        updated_payload["third_party_bindings"]["coinbase_wallet_enabled"],
        true
    );
    assert_eq!(
        updated_payload["third_party_bindings"]["telegram_account_enabled"],
        true
    );

    let reset = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/api/v1/users/{user_id}/2fa/reset"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"reason": "user lost authenticator"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reset.status(), StatusCode::OK);
    let reset_payload = body_json(reset).await?;
    assert_eq!(reset_payload["user_id"], user_id);
    assert_eq!(reset_payload["totp_enabled"], false);
    assert_eq!(reset_payload["login_2fa_enabled"], false);

    let stored: (Option<String>, bool, bool) = sqlx::query_as(
        r#"SELECT totp_secret_encrypted, totp_enabled, login_2fa_enabled
           FROM user_two_factor_settings
           WHERE user_id = ?"#,
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;
    assert!(stored.0.is_none());
    assert!(!stored.1);
    assert!(!stored.2);

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type IN ('security_policy', 'user_two_factor')
           ORDER BY id ASC"#,
    )
    .bind(admin_id)
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 2);
    assert_eq!(audits[0].action, "security_policy.update");
    assert_eq!(audits[0].target_type, "security_policy");
    assert_eq!(audits[0].target_id, "0");
    assert_eq!(
        audits[0].reason.as_deref(),
        Some("enable stronger security policy")
    );
    assert_eq!(
        audits[0].before_json.as_ref().unwrap()["login_2fa_mode"],
        "user_enabled"
    );
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["login_2fa_mode"],
        "mandatory"
    );
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["registration_invite_required"],
        true
    );
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["username_login_enabled"],
        true
    );
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["third_party_bindings"]["coinbase_wallet_enabled"],
        true
    );
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["third_party_bindings"]["telegram_account_enabled"],
        true
    );
    assert_eq!(audits[1].action, "user_2fa.reset");
    assert_eq!(audits[1].target_type, "user_two_factor");
    assert_eq!(audits[1].target_id, user_id.to_string());
    assert_eq!(audits[1].reason.as_deref(), Some("user lost authenticator"));
    assert_eq!(
        audits[1].before_json.as_ref().unwrap()["totp_enabled"],
        true
    );
    assert_eq!(
        audits[1].after_json.as_ref().unwrap()["totp_enabled"],
        false
    );

    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM user_two_factor_settings WHERE user_id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM security_policy_configs WHERE policy_key = 'user_security_policy'")
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
    Ok(())
}

#[tokio::test]
async fn admin_kyc_config_list_detail_and_manual_review() -> Result<(), Box<dyn Error>> {
    let _guard = KYC_CONFIG_TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    reset_kyc_config(&pool).await?;
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let user_id = create_user(&pool).await;
    let submission_id = sqlx::query(
        r#"INSERT INTO user_kyc_submissions
           (user_id, real_name, country, id_number, document_type, document_front_image, document_back_image, document_handheld_image, status, target_kyc_level)
           VALUES (?, 'Zhang San', 'China', 'CN1234567890', 'identity_card', 'data:image/png;base64,front', 'data:image/png;base64,back', 'data:image/png;base64,handheld', 'pending', 1)"#,
    )
    .bind(user_id)
    .execute(&pool)
    .await?
    .last_insert_id();
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let config_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/kyc/config")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let config_payload = body_json(config_response).await?;
    assert_eq!(config_payload["enabled"], true);
    assert_eq!(config_payload["target_kyc_level"], 1);

    let updated_config = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/kyc/config")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "enabled": true,
                        "target_kyc_level": 2,
                        "required_documents": ["identity_front", "identity_back"],
                        "allowed_countries": ["China"],
                        "country_document_types": [
                            { "country": "China", "document_types": ["identity_card", "passport"], "handheld_document_types": ["passport"] }
                        ],
                        "max_document_size_bytes": 6291456,
                        "reason": "tighten kyc config"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let updated_config_status = updated_config.status();
    let updated_config_payload = body_json(updated_config).await?;
    assert_eq!(
        updated_config_status,
        StatusCode::OK,
        "payload: {updated_config_payload}"
    );
    assert_eq!(updated_config_payload["target_kyc_level"], 2);
    assert_eq!(
        updated_config_payload["allowed_countries"],
        json!(["China"])
    );
    assert_eq!(
        updated_config_payload["country_document_types"],
        json!([{ "country": "China", "document_types": ["identity_card", "passport"], "handheld_document_types": ["passport"] }])
    );

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/kyc/submissions?status=pending&limit=10")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let list_payload = body_json(list_response).await?;
    let submissions = list_payload["submissions"].as_array().unwrap();
    assert!(
        submissions
            .iter()
            .any(|submission| submission["id"] == submission_id)
    );

    let detail_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/api/v1/kyc/submissions/{submission_id}"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let detail_payload = body_json(detail_response).await?;
    assert_eq!(
        detail_payload["document_front_image"],
        "data:image/png;base64,front"
    );
    assert_eq!(
        detail_payload["document_handheld_image"],
        "data:image/png;base64,handheld"
    );
    assert_eq!(detail_payload["status"], "pending");

    let review_response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/admin/api/v1/kyc/submissions/{submission_id}/review"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "status": "approved",
                        "kyc_level": 2,
                        "reason": "identity checked"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let review_status = review_response.status();
    let review_payload = body_json(review_response).await?;
    assert_eq!(review_status, StatusCode::OK, "payload: {review_payload}");
    assert_eq!(review_payload["status"], "approved");
    assert_eq!(review_payload["reviewed_by"], admin_id);

    let kyc_level: i32 = sqlx::query_scalar("SELECT kyc_level FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(kyc_level, 2);
    let audit_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM admin_audit_logs
           WHERE admin_id = ? AND action IN ('kyc.config.update', 'kyc.submission.approve')"#,
    )
    .bind(admin_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(audit_count, 2);

    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM user_kyc_submissions WHERE user_id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    reset_kyc_config(&pool).await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
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
    Ok(())
}

#[tokio::test]
async fn admin_kyc_list_and_detail_includes_enterprise_fields() -> Result<(), Box<dyn Error>> {
    let _guard = KYC_CONFIG_TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    reset_kyc_config(&pool).await?;
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let user_id = create_user(&pool).await;
    let submission_id = sqlx::query(
        r#"INSERT INTO user_kyc_submissions
           (user_id, real_name, submission_type, enterprise_name, business_registration_number, country, id_number, document_type, document_front_image, document_back_image, document_handheld_image, status, target_kyc_level)
           VALUES (?, 'Acme Holdings', 'enterprise', 'Acme Holdings Ltd', '91310000712345678A', 'China', 'CN9999999999', 'identity_card', 'data:image/png;base64,front', 'data:image/png;base64,back', 'data:image/png;base64,handheld', 'pending', 1)"#,
    )
    .bind(user_id)
    .execute(&pool)
    .await?
    .last_insert_id();
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/kyc/submissions?status=pending&limit=10")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let list_payload = body_json(list_response).await?;
    let submissions = list_payload["submissions"].as_array().unwrap();
    let submission = submissions
        .iter()
        .find(|item| item["id"].as_u64() == Some(submission_id))
        .expect("submitted enterprise kyc should appear");
    assert_eq!(submission["submission_type"], "enterprise");
    assert_eq!(submission["enterprise_name"], "Acme Holdings Ltd");
    assert_eq!(
        submission["business_registration_number"],
        "91310000712345678A"
    );

    let detail_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/api/v1/kyc/submissions/{submission_id}"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let detail_payload = body_json(detail_response).await?;
    assert_eq!(detail_payload["submission_type"], "enterprise");
    assert_eq!(detail_payload["enterprise_name"], "Acme Holdings Ltd");
    assert_eq!(
        detail_payload["business_registration_number"],
        "91310000712345678A"
    );

    sqlx::query("DELETE FROM user_kyc_submissions WHERE user_id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
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
    Ok(())
}

#[tokio::test]
async fn admin_country_config_crud_filters_and_audit() -> Result<(), Box<dyn Error>> {
    let _guard = COUNTRY_CONFIG_TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let invalid_default = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/countries")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "country_code": "qa",
                        "country_name": "Invalid Country",
                        "remark": "无效国家",
                        "default_locale": "zh",
                        "supported_locales": ["en"],
                        "registration_enabled": true,
                        "status": "active",
                        "sort_order": 1,
                        "reason": "invalid default locale"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid_default.status(), StatusCode::BAD_REQUEST);

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/countries")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "country_code": " qa ",
                        "country_name": "Admin Country",
                        "remark": "后台国家",
                        "default_locale": "zh",
                        "supported_locales": ["zh", "en"],
                        "registration_enabled": true,
                        "status": "active",
                        "sort_order": 7,
                        "reason": "create country"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let create_status = create.status();
    let created = body_json(create).await?;
    assert_eq!(create_status, StatusCode::OK, "payload: {created}");
    let country_id = created["id"].as_u64().unwrap();
    assert_eq!(created["country_code"], "QA");
    assert_eq!(created["country_name"], "Admin Country");
    assert_eq!(created["remark"], "后台国家");
    assert_eq!(created["default_locale"], "zh");
    assert_eq!(created["supported_locales"], json!(["zh", "en"]));
    assert_eq!(created["registration_enabled"], true);
    assert_eq!(created["status"], "active");
    assert_eq!(created["sort_order"], 7);
    assert!(created["created_at"].is_number());
    assert!(created["updated_at"].is_number());

    let listed = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/countries?country_code=QA&status=active&registration_enabled=true&limit=10")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let listed_status = listed.status();
    let listed_payload = body_json(listed).await?;
    assert_eq!(listed_status, StatusCode::OK, "payload: {listed_payload}");
    let countries = listed_payload["countries"].as_array().unwrap();
    assert!(countries.iter().any(|country| country["id"] == country_id));

    let update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/countries/{country_id}"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "country_name": "Updated Country",
                        "remark": "更新国家",
                        "default_locale": "en",
                        "supported_locales": ["en"],
                        "registration_enabled": false,
                        "sort_order": 9,
                        "reason": "update country"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let update_status = update.status();
    let updated = body_json(update).await?;
    assert_eq!(update_status, StatusCode::OK, "payload: {updated}");
    assert_eq!(updated["country_name"], "Updated Country");
    assert_eq!(updated["remark"], "更新国家");
    assert_eq!(updated["default_locale"], "en");
    assert_eq!(updated["supported_locales"], json!(["en"]));
    assert_eq!(updated["registration_enabled"], false);
    assert_eq!(updated["status"], "active");

    let status_update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/countries/{country_id}/status"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "status": "disabled", "reason": "disable country" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let status_update_status = status_update.status();
    let disabled = body_json(status_update).await?;
    assert_eq!(status_update_status, StatusCode::OK, "payload: {disabled}");
    assert_eq!(disabled["status"], "disabled");

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type = 'country_config' AND target_id = ?
           ORDER BY id"#,
    )
    .bind(admin_id)
    .bind(country_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 3, "audits: {audits:?}");
    assert_eq!(audits[0].action, "country_config.create");
    assert!(audits[0].before_json.is_none());
    assert_eq!(audits[0].after_json.as_ref().unwrap()["country_code"], "QA");
    assert_eq!(audits[0].reason.as_deref(), Some("create country"));
    assert_eq!(audits[1].action, "country_config.update");
    assert_eq!(
        audits[1].before_json.as_ref().unwrap()["default_locale"],
        "zh"
    );
    assert_eq!(
        audits[1].after_json.as_ref().unwrap()["default_locale"],
        "en"
    );
    assert_eq!(audits[1].reason.as_deref(), Some("update country"));
    assert_eq!(audits[2].action, "country_config.status.update");
    assert_eq!(audits[2].before_json.as_ref().unwrap()["status"], "active");
    assert_eq!(audits[2].after_json.as_ref().unwrap()["status"], "disabled");
    assert_eq!(audits[2].reason.as_deref(), Some("disable country"));

    sqlx::query(
        "DELETE FROM admin_audit_logs WHERE admin_id = ? AND target_type = 'country_config'",
    )
    .bind(admin_id)
    .execute(&pool)
    .await?;
    sqlx::query("DELETE FROM country_configs WHERE id = ?")
        .bind(country_id)
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
    Ok(())
}

fn admin_news_content(default_locale: &str, items: Vec<Value>) -> Value {
    json!({
        "version": 1,
        "default_locale": default_locale,
        "items": items
    })
}

fn admin_news_content_item(
    locale: &str,
    country_code: &str,
    title: &str,
    summary: &str,
    text: &str,
) -> Value {
    json!({
        "locale": locale,
        "country_code": country_code,
        "title": title,
        "summary": summary,
        "content": [
            { "type": "p", "children": [{ "text": text }] }
        ]
    })
}

#[tokio::test]
async fn admin_news_routes_require_admin_scope_mysql_and_validation() -> Result<(), Box<dyn Error>>
{
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));
    let content = admin_news_content(
        "zh-CN",
        vec![admin_news_content_item(
            "zh-CN",
            "CN",
            "新闻标题",
            "新闻摘要",
            "新闻内容",
        )],
    );
    let body = json!({
        "title": "News Center Auth Test",
        "category": "general",
        "status": "draft",
        "country_code": "CN",
        "default_locale": "zh-CN",
        "content_json": content,
        "reason": "create news auth test"
    })
    .to_string();

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/news")
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let user = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/news")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(user.status(), StatusCode::FORBIDDEN);

    let invalid_category = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/news")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "News Center Auth Test",
                        "category": "unknown",
                        "status": "draft",
                        "country_code": "CN",
                        "default_locale": "zh-CN",
                        "content_json": content,
                        "reason": "create news auth test"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid_category.status(), StatusCode::BAD_REQUEST);
    let invalid_category_payload = body_json(invalid_category).await?;
    assert_eq!(invalid_category_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        invalid_category_payload["message"],
        "validation error: unsupported news category"
    );

    let invalid_content = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/news")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "News Center Auth Test",
                        "category": "general",
                        "status": "draft",
                        "country_code": "CN",
                        "default_locale": "zh-CN",
                        "content_json": admin_news_content("zh-CN", vec![]),
                        "reason": "create news auth test"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid_content.status(), StatusCode::BAD_REQUEST);
    let invalid_content_payload = body_json(invalid_content).await?;
    assert_eq!(invalid_content_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        invalid_content_payload["message"],
        "validation error: news content items are required"
    );

    let extra_content_field = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/news")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "News Center Auth Test",
                        "category": "general",
                        "status": "draft",
                        "country_code": "CN",
                        "default_locale": "zh-CN",
                        "content_json": {
                            "version": 1,
                            "default_locale": "zh-CN",
                            "seo": { "title": "out of scope" },
                            "items": [admin_news_content_item("zh-CN", "CN", "新闻标题", "新闻摘要", "新闻内容")]
                        },
                        "reason": "create news auth test"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(extra_content_field.status(), StatusCode::BAD_REQUEST);
    let extra_content_payload = body_json(extra_content_field).await?;
    assert_eq!(extra_content_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        extra_content_payload["message"],
        "validation error: news content field is unsupported"
    );

    let extra_item_field = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/news")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "News Center Auth Test",
                        "category": "general",
                        "status": "draft",
                        "country_code": "CN",
                        "default_locale": "zh-CN",
                        "content_json": {
                            "version": 1,
                            "default_locale": "zh-CN",
                            "items": [{
                                "locale": "zh-CN",
                                "country_code": "CN",
                                "title": "新闻标题",
                                "summary": "新闻摘要",
                                "cover_url": "https://example.test/cover.png",
                                "content": [{ "type": "p", "children": [{ "text": "新闻内容" }] }]
                            }]
                        },
                        "reason": "create news auth test"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(extra_item_field.status(), StatusCode::BAD_REQUEST);
    let extra_item_payload = body_json(extra_item_field).await?;
    assert_eq!(extra_item_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        extra_item_payload["message"],
        "validation error: news content item field is unsupported"
    );

    let empty_rich_text = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/news")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "News Center Auth Test",
                        "category": "general",
                        "status": "draft",
                        "country_code": "CN",
                        "default_locale": "zh-CN",
                        "content_json": admin_news_content("zh-CN", vec![json!({
                            "locale": "zh-CN",
                            "country_code": "CN",
                            "title": "新闻标题",
                            "summary": [{ "type": "p", "children": [{ "text": "新闻摘要", "bold": true }] }],
                            "content": [{ "type": "p", "children": [{ "text": "   " }] }]
                        })]),
                        "reason": "create news auth test"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(empty_rich_text.status(), StatusCode::BAD_REQUEST);
    let empty_rich_text_payload = body_json(empty_rich_text).await?;
    assert_eq!(empty_rich_text_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        empty_rich_text_payload["message"],
        "validation error: news content body is required"
    );

    let image_rich_text = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/news")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "News Center Image Test",
                        "category": "general",
                        "status": "draft",
                        "country_code": "CN",
                        "default_locale": "zh-CN",
                        "content_json": admin_news_content("zh-CN", vec![json!({
                            "locale": "zh-CN",
                            "country_code": "CN",
                            "title": "新闻标题",
                            "summary": "新闻摘要",
                            "content": [{ "type": "image", "url": "https://cdn.example.test/news/body.png", "alt": "新闻配图" }]
                        })]),
                        "reason": "create news image test"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(image_rich_text.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let image_rich_text_payload = body_json(image_rich_text).await?;
    assert_eq!(image_rich_text_payload["code"], "INTERNAL_ERROR");
    assert!(
        image_rich_text_payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for admin convert routes")
    );

    let admin = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/news")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await?;
    assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let payload = body_json(admin).await?;
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for admin convert routes")
    );

    Ok(())
}

#[tokio::test]
async fn admin_news_crud_filters_status_and_audit() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let unique = Uuid::now_v7().simple().to_string();
    let title = format!("新闻中心测试 {unique}");
    let content = admin_news_content(
        "zh-CN",
        vec![
            admin_news_content_item("zh-CN", "CN", &title, "中文摘要", "中文内容"),
            admin_news_content_item(
                "en-US",
                "US",
                "News Center Test",
                "English summary",
                "English body",
            ),
        ],
    );
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/news")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": title,
                        "category": "market",
                        "status": "draft",
                        "country_code": "CN",
                        "default_locale": "zh-CN",
                        "content_json": content,
                        "reason": "create multilingual news"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let create_status = create.status();
    let created = body_json(create).await?;
    assert_eq!(create_status, StatusCode::OK, "payload: {created}");
    let news_id = created["id"].as_u64().unwrap();
    assert_eq!(created["title"], title);
    assert_eq!(created["category"], "market");
    assert_eq!(created["status"], "draft");
    assert_eq!(created["country_code"], "CN");
    assert_eq!(created["default_locale"], "zh-CN");
    assert!(created["published_at"].is_null());
    assert_eq!(
        created["content_json"]["items"].as_array().unwrap().len(),
        2
    );

    let listed = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/news?status=draft&category=market&country_code=CN&locale=zh-CN&q={unique}&limit=10&offset=0"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let listed_status = listed.status();
    let listed_payload = body_json(listed).await?;
    assert_eq!(listed_status, StatusCode::OK, "payload: {listed_payload}");
    let news = listed_payload["news"].as_array().unwrap();
    assert_eq!(news.len(), 1);
    assert_eq!(news[0]["id"], news_id);

    let detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/api/v1/news/{news_id}"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let detail_status = detail.status();
    let detail_payload = body_json(detail).await?;
    assert_eq!(detail_status, StatusCode::OK, "payload: {detail_payload}");
    assert_eq!(detail_payload["id"], news_id);
    assert_eq!(detail_payload["content_json"]["default_locale"], "zh-CN");
    assert!(!detail_payload.to_string().contains("password"));
    assert!(!detail_payload.to_string().contains("token"));

    let updated_title = format!("新闻中心更新 {unique}");
    let updated_content = admin_news_content(
        "en-US",
        vec![
            admin_news_content_item(
                "en-US",
                "GLOBAL",
                &updated_title,
                "Updated summary",
                "Updated body",
            ),
            admin_news_content_item("zh-CN", "CN", "新闻中心更新", "更新摘要", "更新内容"),
        ],
    );
    let update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/news/{news_id}"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": updated_title,
                        "category": "system",
                        "country_code": "GLOBAL",
                        "default_locale": "en-US",
                        "content_json": updated_content,
                        "reason": "update multilingual news"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let update_status = update.status();
    let updated = body_json(update).await?;
    assert_eq!(update_status, StatusCode::OK, "payload: {updated}");
    assert_eq!(updated["title"], updated_title);
    assert_eq!(updated["category"], "system");
    assert_eq!(updated["country_code"], "GLOBAL");
    assert_eq!(updated["default_locale"], "en-US");

    let publish = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/news/{news_id}/status"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "status": "published", "reason": "publish news" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let publish_status = publish.status();
    let published = body_json(publish).await?;
    assert_eq!(publish_status, StatusCode::OK, "payload: {published}");
    assert_eq!(published["status"], "published");
    assert!(published["published_at"].is_number());

    let stored = sqlx::query_as::<_, (String, String, String, Option<String>, String, SqlxJson<Value>)>(
        "SELECT title, category, status, country_code, default_locale, content_json FROM admin_news_items WHERE id = ?",
    )
    .bind(news_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(stored.0, updated_title);
    assert_eq!(stored.1, "system");
    assert_eq!(stored.2, "published");
    assert_eq!(stored.3.as_deref(), Some("GLOBAL"));
    assert_eq!(stored.4, "en-US");
    assert_eq!(stored.5.0["default_locale"], "en-US");

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type = 'admin_news_item' AND target_id = ?
           ORDER BY id"#,
    )
    .bind(admin_id)
    .bind(news_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 3, "audits: {audits:?}");
    assert_eq!(audits[0].action, "admin_news_item.create");
    assert!(audits[0].before_json.is_none());
    assert_eq!(audits[0].after_json.as_ref().unwrap()["status"], "draft");
    assert_eq!(
        audits[0].reason.as_deref(),
        Some("create multilingual news")
    );
    assert_eq!(audits[1].action, "admin_news_item.update");
    assert_eq!(
        audits[1].before_json.as_ref().unwrap()["category"],
        "market"
    );
    assert_eq!(audits[1].after_json.as_ref().unwrap()["category"], "system");
    assert_eq!(
        audits[1].reason.as_deref(),
        Some("update multilingual news")
    );
    assert_eq!(audits[2].action, "admin_news_item.status.update");
    assert_eq!(audits[2].before_json.as_ref().unwrap()["status"], "draft");
    assert_eq!(
        audits[2].after_json.as_ref().unwrap()["status"],
        "published"
    );
    assert_eq!(audits[2].reason.as_deref(), Some("publish news"));

    sqlx::query(
        "DELETE FROM admin_audit_logs WHERE admin_id = ? AND target_type = 'admin_news_item'",
    )
    .bind(admin_id)
    .execute(&pool)
    .await?;
    sqlx::query("DELETE FROM admin_news_items WHERE id = ?")
        .bind(news_id)
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
    Ok(())
}

#[tokio::test]
async fn admin_dashboard_requires_admin_scope_and_mysql() -> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/dashboard")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let user = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/dashboard")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(user.status(), StatusCode::FORBIDDEN);

    let admin = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/dashboard")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let payload = body_json(admin).await?;
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for admin convert routes")
    );

    Ok(())
}

#[tokio::test]
async fn admin_dashboard_returns_operational_summary_shape() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let mut settings = test_settings();
    settings.app_env = "staging".to_owned();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let action = format!(
        "dashboard.summary.{}",
        &Uuid::now_v7().simple().to_string()[..12]
    );
    let audit_id = sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, after_json, reason)
           VALUES (?, ?, 'dashboard_summary', 'summary',
                   JSON_OBJECT('api_key', 'DASHBOARD_SHOULD_NOT_LEAK'),
                   'dashboard private detail')"#,
    )
    .bind(admin_id)
    .bind(&action)
    .execute(&pool)
    .await?
    .last_insert_id();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/dashboard")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let payload = body_json(response).await?;
    assert_eq!(status, StatusCode::OK, "payload: {payload}");
    assert!(payload["generated_at"].is_number());
    assert_eq!(payload["environment"], "staging");
    assert!(payload["users"]["total"].is_number());
    assert!(payload["wallet"]["active_assets"].is_number());
    let active_gateways: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM wallet_chain_gateways WHERE status = 'active'")
            .fetch_one(&pool)
            .await?;
    let expected_custody = if active_gateways > 0 {
        "active"
    } else {
        "not_configured"
    };
    assert_eq!(payload["wallet"]["custody_status"], expected_custody);
    assert!(payload["market"]["active_pairs"].is_number());
    assert_eq!(payload["market"]["feed_runtime_status"], "not_started");
    assert!(payload["market"]["feed_symbols"].as_array().is_some());
    assert!(payload["trading"]["spot_open_orders"].is_number());
    assert!(payload["products"]["margin_open_positions"].is_number());
    assert!(payload["risk"]["pending_outbox_events"].is_number());
    assert!(payload["audit"]["admin_actions_24h"].as_i64().unwrap() >= 1);
    let latest_action = payload["audit"]["latest_actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["action"] == action)
        .expect("dashboard audit action should be included");
    for private_field in ["before_json", "after_json", "reason", "ip", "request_id"] {
        assert!(latest_action.get(private_field).is_none());
    }
    let serialized_payload = payload.to_string();
    assert!(!serialized_payload.contains("DASHBOARD_SHOULD_NOT_LEAK"));
    assert!(!serialized_payload.contains("0123456789abcdef0123456789abcdef"));

    sqlx::query("DELETE FROM admin_audit_logs WHERE id = ?")
        .bind(audit_id)
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
    Ok(())
}

#[tokio::test]
async fn admin_dashboard_wallet_summary_counts_live_pipeline_and_custody()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let suffix = Uuid::now_v7().simple().to_string();
    let user_id = sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(format!("dashboard-wallet-{suffix}@example.test"))
        .bind("not-a-real-hash")
        .execute(&pool)
        .await?
        .last_insert_id();
    let symbol = format!("DW{}", &suffix[..8]).to_ascii_uppercase();
    let asset_id =
        sqlx::query("INSERT INTO assets (symbol, name, precision_scale) VALUES (?, ?, 18)")
            .bind(&symbol)
            .bind(&symbol)
            .execute(&pool)
            .await?
            .last_insert_id();
    let baseline_deposits: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM wallet_deposit_events WHERE status = 'observed'")
            .fetch_one(&pool)
            .await?;
    let baseline_withdrawals: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM wallet_withdrawal_requests
           WHERE status IN ('pending_review', 'approved', 'broadcasting',
                            'broadcasted', 'manual_review')"#,
    )
    .fetch_one(&pool)
    .await?;

    let deposit_id = sqlx::query(
        r#"INSERT INTO wallet_deposit_events
           (user_id, asset_id, asset_symbol, network, address, tx_hash, amount,
            required_confirmations, status)
           VALUES (?, ?, ?, ?, ?, ?, 1, 12, 'observed')"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(&symbol)
    .bind(format!("net-{suffix}"))
    .bind(format!("addr-{suffix}"))
    .bind(format!("tx-{suffix}"))
    .execute(&pool)
    .await?
    .last_insert_id();
    let withdrawal_id = sqlx::query(
        r#"INSERT INTO wallet_withdrawal_requests
           (user_id, asset_id, asset_symbol, network, address, amount, fee, total_reserved,
            status, security_method, idempotency_key, gateway_request_id)
           VALUES (?, ?, ?, ?, ?, 1, 0, 1, 'pending_review', 'none', ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(&symbol)
    .bind(format!("net-{suffix}"))
    .bind(format!("addr-{suffix}"))
    .bind(format!("idem-{suffix}"))
    .bind(Uuid::now_v7().to_string())
    .execute(&pool)
    .await?
    .last_insert_id();
    let gateway_id = sqlx::query(
        r#"INSERT INTO wallet_chain_gateways (network, broadcast_url, status)
           VALUES (?, 'https://gateway.test/broadcast', 'active')"#,
    )
    .bind(format!("net-{suffix}"))
    .execute(&pool)
    .await?
    .last_insert_id();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/dashboard")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let payload = body_json(response).await?;
    assert_eq!(status, StatusCode::OK, "payload: {payload}");
    assert_eq!(payload["wallet"]["pending_deposits"], baseline_deposits + 1);
    assert_eq!(
        payload["wallet"]["pending_withdrawals"],
        baseline_withdrawals + 1
    );
    assert_eq!(payload["wallet"]["custody_status"], "active");

    sqlx::query("DELETE FROM wallet_chain_gateways WHERE id = ?")
        .bind(gateway_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_withdrawal_requests WHERE id = ?")
        .bind(withdrawal_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_deposit_events WHERE id = ?")
        .bind(deposit_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
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
    Ok(())
}

#[tokio::test]
async fn admin_register_route_requires_existing_active_admin_once_bootstrapped()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let user_token =
        issue_token(&settings, format!("user:{admin_id}"), TokenScope::User, 900).unwrap();
    let (ghost_role_id, ghost_admin_id) = create_admin_user(&pool).await;
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(ghost_admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(ghost_role_id)
        .execute(&pool)
        .await?;
    let ghost_token = issue_token(
        &settings,
        format!("admin:{ghost_admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let username = format!("admin-register-{}", Uuid::now_v7().simple());
    let body = json!({
        "username": username,
        "password": "admin-register-password-1",
        "role_id": role_id
    })
    .to_string();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let request = |token: Option<&str>| {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/admin/api/v1/auth/register")
            .header("content-type", "application/json");
        if let Some(token) = token {
            builder = builder.header(AUTHORIZATION, format!("Bearer {token}"));
        }
        builder.body(Body::from(body.clone())).unwrap()
    };

    let missing = app.clone().oneshot(request(None)).await?;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let wrong_scope = app.clone().oneshot(request(Some(&user_token))).await?;
    assert_eq!(wrong_scope.status(), StatusCode::FORBIDDEN);

    let ghost = app.clone().oneshot(request(Some(&ghost_token))).await?;
    assert_eq!(ghost.status(), StatusCode::FORBIDDEN);

    let blocked_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM admin_users WHERE username = ?")
            .bind(&username)
            .fetch_one(&pool)
            .await?;
    assert_eq!(blocked_count, 0);

    let created = app.oneshot(request(Some(&admin_token))).await?;
    let status = created.status();
    let payload = body_json(created).await?;
    assert_eq!(status, StatusCode::OK, "payload: {payload}");
    assert_eq!(payload["scope"], "admin");
    assert!(
        payload["access_token"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );

    let new_admin_id: u64 = sqlx::query_scalar("SELECT id FROM admin_users WHERE username = ?")
        .bind(&username)
        .fetch_one(&pool)
        .await?;
    sqlx::query("DELETE FROM refresh_tokens WHERE actor_type = 'admin' AND actor_id = ?")
        .bind(new_admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(new_admin_id)
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
    Ok(())
}

#[tokio::test]
async fn admin_core_resource_routes_require_admin_scope_and_mysql() -> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));
    let risk_rule_body = json!({
        "rule_type": "withdraw_limit",
        "target_type": "user",
        "target_id": "1",
        "config_json": { "daily_limit": "1000" },
        "enabled": true
    })
    .to_string();
    let risk_status_body = json!({ "enabled": false, "reason": "pause rule" }).to_string();
    let market_feed_config_body = json!({
        "symbols": ["BTC-USDT"],
        "intervals": ["1m"],
        "providers": ["bitget"],
        "enabled": true,
        "reason": "update feed config"
    })
    .to_string();
    let market_feed_credential_body = json!({
        "auth_type": "none",
        "enabled": true,
        "reason": "disable credential"
    })
    .to_string();
    let market_feed_reload_body = json!({ "reason": "reload feed config" }).to_string();
    let smtp_config_body = json!({
        "name": "Scope SMTP",
        "host": "smtp.example.test",
        "port": 587,
        "security": "starttls",
        "username": "scope-smtp-user",
        "password": "scope-smtp-password",
        "from_email": "noreply@example.test",
        "from_name": "Exchange Test",
        "enabled": true,
        "priority": 100,
        "reason": "update smtp config"
    })
    .to_string();
    let smtp_delivery_settings_body = json!({
        "strategy": "round_robin",
        "reason": "switch smtp strategy"
    })
    .to_string();
    let smtp_test_body = json!({
        "recipient": "ops@example.test",
        "reason": "test smtp config"
    })
    .to_string();
    let platform_brand_body = json!({
        "platform_name": "Scope Exchange",
        "logo_url": "https://cdn.example.test/scope-logo.png",
        "reason": "update pc brand"
    })
    .to_string();
    let user_create_body = json!({
        "email": "scope-admin-create-user@example.test",
        "password": "Password123!",
        "status": "active",
        "kyc_level": 0,
        "reason": "scope create user"
    })
    .to_string();
    let user_recharge_body = json!({
        "asset_id": 1,
        "amount": "1.000000000000000000",
        "reason": "scope recharge user",
        "idempotency_key": "scope-recharge-user"
    })
    .to_string();
    let cases = [
        ("GET", "/admin/api/v1/users?limit=1", None),
        ("GET", "/admin/api/v1/users/1", None),
        (
            "POST",
            "/admin/api/v1/users",
            Some(user_create_body.as_str()),
        ),
        (
            "POST",
            "/admin/api/v1/users/1/recharge",
            Some(user_recharge_body.as_str()),
        ),
        ("GET", "/admin/api/v1/wallet/accounts?limit=1", None),
        ("GET", "/admin/api/v1/wallet/ledger?limit=1", None),
        ("GET", "/admin/api/v1/risk/rules?limit=1", None),
        (
            "POST",
            "/admin/api/v1/risk/rules",
            Some(risk_rule_body.as_str()),
        ),
        (
            "PATCH",
            "/admin/api/v1/risk/rules/1/status",
            Some(risk_status_body.as_str()),
        ),
        ("GET", "/admin/api/v1/risk/events?limit=1", None),
        ("GET", "/admin/api/v1/market-feed/config", None),
        (
            "PATCH",
            "/admin/api/v1/market-feed/config",
            Some(market_feed_config_body.as_str()),
        ),
        ("GET", "/admin/api/v1/market-feed/status", None),
        ("GET", "/admin/api/v1/market-feed/credentials", None),
        (
            "PATCH",
            "/admin/api/v1/market-feed/credentials/bitget",
            Some(market_feed_credential_body.as_str()),
        ),
        (
            "POST",
            "/admin/api/v1/market-feed/reload",
            Some(market_feed_reload_body.as_str()),
        ),
        ("GET", "/admin/api/v1/smtp/config", None),
        (
            "PATCH",
            "/admin/api/v1/smtp/config",
            Some(smtp_config_body.as_str()),
        ),
        ("GET", "/admin/api/v1/smtp/configs", None),
        (
            "POST",
            "/admin/api/v1/smtp/configs",
            Some(smtp_config_body.as_str()),
        ),
        (
            "PATCH",
            "/admin/api/v1/smtp/configs/1",
            Some(smtp_config_body.as_str()),
        ),
        (
            "PATCH",
            "/admin/api/v1/smtp/delivery-settings",
            Some(smtp_delivery_settings_body.as_str()),
        ),
        (
            "POST",
            "/admin/api/v1/smtp/test",
            Some(smtp_test_body.as_str()),
        ),
        ("GET", "/admin/api/v1/platform/brand", None),
        (
            "PATCH",
            "/admin/api/v1/platform/brand",
            Some(platform_brand_body.as_str()),
        ),
        ("GET", "/admin/api/v1/new-coins/subscriptions?limit=1", None),
        ("GET", "/admin/api/v1/new-coins/distributions?limit=1", None),
    ];

    for (method, path, body) in cases {
        let mut missing_builder = Request::builder().method(method).uri(path);
        if body.is_some() {
            missing_builder = missing_builder.header("content-type", "application/json");
        }
        let missing = app
            .clone()
            .oneshot(
                missing_builder
                    .body(body.map_or_else(Body::empty, |value| Body::from(value.to_owned())))
                    .unwrap(),
            )
            .await?;
        assert_eq!(missing.status(), StatusCode::UNAUTHORIZED, "path: {path}");

        let mut user_builder = Request::builder()
            .method(method)
            .uri(path)
            .header(AUTHORIZATION, format!("Bearer {user_token}"));
        if body.is_some() {
            user_builder = user_builder.header("content-type", "application/json");
        }
        let user = app
            .clone()
            .oneshot(
                user_builder
                    .body(body.map_or_else(Body::empty, |value| Body::from(value.to_owned())))
                    .unwrap(),
            )
            .await?;
        assert_eq!(user.status(), StatusCode::FORBIDDEN, "path: {path}");

        let mut admin_builder = Request::builder()
            .method(method)
            .uri(path)
            .header(AUTHORIZATION, format!("Bearer {admin_token}"));
        if body.is_some() {
            admin_builder = admin_builder.header("content-type", "application/json");
        }
        let admin = app
            .clone()
            .oneshot(
                admin_builder
                    .body(body.map_or_else(Body::empty, |value| Body::from(value.to_owned())))
                    .unwrap(),
            )
            .await?;
        assert_eq!(
            admin.status(),
            StatusCode::INTERNAL_SERVER_ERROR,
            "path: {path}"
        );
        let payload = body_json(admin).await?;
        assert_eq!(payload["code"], "INTERNAL_ERROR");
        assert!(
            payload["message"]
                .as_str()
                .unwrap()
                .contains("mysql pool is not configured for admin convert routes"),
            "path: {path}, payload: {payload}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn admin_platform_brand_config_save_and_audit() -> Result<(), Box<dyn Error>> {
    let _guard = PLATFORM_BRAND_CONFIG_TEST_LOCK.lock().await;
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    reset_platform_brand_config(&pool).await?;
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let current = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/platform/brand")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let current_status = current.status();
    let current_payload = body_json(current).await?;
    assert_eq!(current_status, StatusCode::OK, "payload: {current_payload}");
    assert_eq!(current_payload["name"], "default");
    assert_eq!(current_payload["platform_name"], "Hippo Exchange");
    assert_eq!(current_payload["logo_url"], Value::Null);
    assert_eq!(current_payload["chart_provider"], "klinecharts");

    let missing_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/platform/brand")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "platform_name": "Rust Chain",
                        "logo_url": "https://cdn.example.test/logo.png",
                        "reason": " "
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing_reason.status(), StatusCode::BAD_REQUEST);

    let invalid_logo = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/platform/brand")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "platform_name": "Rust Chain",
                        "logo_url": "javascript:alert(1)",
                        "reason": "reject unsafe logo"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid_logo.status(), StatusCode::BAD_REQUEST);

    let invalid_chart_provider = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/platform/brand")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "platform_name": "Rust Chain",
                        "logo_url": "https://cdn.example.test/logo.png",
                        "chart_provider": "unknown",
                        "reason": "reject unsupported chart provider"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid_chart_provider.status(), StatusCode::BAD_REQUEST);

    let saved = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/platform/brand")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "platform_name": "Rust Chain",
                        "logo_url": "https://cdn.example.test/logo.png",
                        "chart_provider": "tradingview",
                        "reason": "update pc brand"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let saved_status = saved.status();
    let saved_payload = body_json(saved).await?;
    assert_eq!(saved_status, StatusCode::OK, "payload: {saved_payload}");
    assert_eq!(saved_payload["platform_name"], "Rust Chain");
    assert_eq!(
        saved_payload["logo_url"],
        "https://cdn.example.test/logo.png"
    );
    assert_eq!(saved_payload["chart_provider"], "tradingview");
    assert_eq!(saved_payload["updated_by"], admin_id);

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type = 'platform_brand_config'
           ORDER BY id ASC"#,
    )
    .bind(admin_id)
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].action, "platform_brand.update");
    assert_eq!(
        audits[0].target_id,
        saved_payload["id"].as_u64().unwrap().to_string()
    );
    assert_eq!(
        audits[0].before_json.as_ref().unwrap()["platform_name"],
        "Hippo Exchange"
    );
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["platform_name"],
        "Rust Chain"
    );
    assert_eq!(
        audits[0].before_json.as_ref().unwrap()["chart_provider"],
        "klinecharts"
    );
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["chart_provider"],
        "tradingview"
    );
    assert_eq!(audits[0].reason.as_deref(), Some("update pc brand"));

    sqlx::query(
        "DELETE FROM admin_audit_logs WHERE admin_id = ? AND target_type = 'platform_brand_config'",
    )
    .bind(admin_id)
    .execute(&pool)
    .await?;
    reset_platform_brand_config(&pool).await?;
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(role_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn admin_smtp_routes_require_admin_scope_and_mysql() -> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/smtp/config")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let user = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/smtp/config")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(user.status(), StatusCode::FORBIDDEN);

    let admin = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/smtp/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let payload = body_json(admin).await?;
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for admin convert routes")
    );

    Ok(())
}

#[tokio::test]
async fn admin_smtp_config_save_masks_secrets_and_requires_reason() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let _smtp_lock = SMTP_CONFIG_TEST_LOCK.lock().await;
    let settings = test_settings();
    sqlx::query("DELETE FROM smtp_configs WHERE name = 'default'")
        .execute(&pool)
        .await?;
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let missing_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/smtp/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "host": "smtp.example.test",
                        "port": 587,
                        "security": "starttls",
                        "username": "smtp-user@example.test",
                        "password": "smtp-secret-value",
                        "from_email": "noreply@example.test",
                        "from_name": "Exchange Test",
                        "enabled": true,
                        "reason": " "
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing_reason.status(), StatusCode::BAD_REQUEST);

    let incomplete_credentials = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/smtp/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "host": "smtp.example.test",
                        "port": 587,
                        "security": "starttls",
                        "username": "smtp-user@example.test",
                        "from_email": "noreply@example.test",
                        "from_name": "Exchange Test",
                        "enabled": true,
                        "reason": "configure smtp"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(incomplete_credentials.status(), StatusCode::BAD_REQUEST);

    let saved = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/smtp/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "host": "smtp.example.test",
                        "port": 587,
                        "security": "starttls",
                        "username": "smtp-user@example.test",
                        "password": "smtp-secret-value",
                        "from_email": "noreply@example.test",
                        "from_name": "Exchange Test",
                        "verification_code_template_html": "<p>{{subject}}：<strong>{{code}}</strong></p>",
                        "verification_code_templates": [
                            {
                                "key": "default",
                                "name": "通用验证码",
                                "purpose": null,
                                "html": "<p>{{subject}}：<strong>{{code}}</strong></p>",
                                "enabled": true
                            },
                            {
                                "key": "fund_password_reset",
                                "name": "资金密码验证码",
                                "purpose": "fund_password_reset",
                                "html": "<p>资金验证码：<strong>{{code}}</strong></p>",
                                "enabled": true
                            }
                        ],
                        "enabled": true,
                        "reason": "configure smtp"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let saved_status = saved.status();
    let saved_payload = body_json(saved).await?;
    assert_eq!(saved_status, StatusCode::OK, "payload: {saved_payload}");
    assert_eq!(saved_payload["host"], "smtp.example.test");
    assert_eq!(saved_payload["port"], 587);
    assert_eq!(saved_payload["security"], "starttls");
    assert_eq!(saved_payload["username_mask"], "smtp****test");
    assert_eq!(saved_payload["password_set"], true);
    assert_eq!(
        saved_payload["verification_code_template_html"],
        "<p>{{subject}}：<strong>{{code}}</strong></p>"
    );
    assert_eq!(
        saved_payload["verification_code_templates"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        saved_payload["verification_code_templates"][1]["purpose"],
        "fund_password_reset"
    );
    assert_eq!(saved_payload["username"], Value::Null);
    assert_eq!(saved_payload["password"], Value::Null);
    assert!(!saved_payload.to_string().contains("smtp-secret-value"));

    let stored: (String, String, Option<String>, Option<SqlxJson<Vec<Value>>>) = sqlx::query_as(
        "SELECT username_ciphertext, password_ciphertext, verification_code_template_html, verification_code_templates_json FROM smtp_configs WHERE name = 'default'",
    )
    .fetch_one(&pool)
    .await?;
    assert!(!stored.0.contains("smtp-user@example.test"));
    assert!(!stored.1.contains("smtp-secret-value"));
    assert_eq!(
        stored.2.as_deref(),
        Some("<p>{{subject}}：<strong>{{code}}</strong></p>")
    );
    assert_eq!(stored.3.as_ref().unwrap().0.len(), 2);
    assert_eq!(
        stored.3.as_ref().unwrap().0[1]["key"],
        "fund_password_reset"
    );

    let current = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/smtp/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let current_payload = body_json(current).await?;
    assert_eq!(current_payload["password_set"], true);
    assert!(!current_payload.to_string().contains("smtp-secret-value"));

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type = 'smtp_config'
           ORDER BY id ASC"#,
    )
    .bind(admin_id)
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].action, "smtp_config.save");
    assert_eq!(audits[0].reason.as_deref(), Some("configure smtp"));
    let audit_text = audits[0].after_json.as_ref().unwrap().to_string();
    assert!(audit_text.contains("password_set"));
    assert!(!audit_text.contains("smtp-secret-value"));
    assert!(!audit_text.contains(&stored.1));

    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM smtp_configs WHERE name = 'default'")
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
    Ok(())
}

#[tokio::test]
async fn admin_smtp_test_uses_configured_sender_and_audits_without_secrets()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let _smtp_lock = SMTP_CONFIG_TEST_LOCK.lock().await;
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    sqlx::query("DELETE FROM smtp_configs WHERE name = 'default'")
        .execute(&pool)
        .await?;
    let key = settings.exposed_credential_encryption_key().unwrap();
    let username_ciphertext = encrypt_secret("smtp-user@example.test", key)?;
    let password_ciphertext = encrypt_secret("smtp-secret-value", key)?;
    sqlx::query(
        r#"INSERT INTO smtp_configs
           (name, host, port, security, username_ciphertext, password_ciphertext, username_mask, from_email, from_name, enabled, updated_by)
           VALUES ('default', 'smtp.example.test', 587, 'starttls', ?, ?, 'smtp****test', 'noreply@example.test', 'Exchange Test', TRUE, ?)"#,
    )
    .bind(&username_ciphertext)
    .bind(&password_ciphertext)
    .bind(admin_id)
    .execute(&pool)
    .await?;
    let app = build_router(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_email_sender(Arc::new(RecordingEmailSender {
                pool: pool.clone(),
                admin_id,
            })),
    );

    let sent = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/smtp/test")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "recipient": "operator@example.test",
                        "reason": "verify smtp delivery"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let sent_status = sent.status();
    let sent_payload = body_json(sent).await?;
    assert_eq!(sent_status, StatusCode::OK, "payload: {sent_payload}");
    assert_eq!(sent_payload["sent"], true);
    assert_eq!(sent_payload["recipient"], "operator@example.test");

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type = 'smtp_config'
           ORDER BY id ASC"#,
    )
    .bind(admin_id)
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].action, "smtp_config.test");
    assert_eq!(audits[0].reason.as_deref(), Some("verify smtp delivery"));
    let audit_text = audits[0].after_json.as_ref().unwrap().to_string();
    assert!(audit_text.contains("operator@example.test"));
    assert!(!audit_text.contains("smtp-secret-value"));

    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM smtp_configs WHERE name = 'default'")
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
    Ok(())
}

#[tokio::test]
async fn admin_quick_recharge_test_route_requires_admin_scope_and_mysql()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));
    let body = json!({
        "amount": "12.50",
        "reason": "test quick recharge config"
    })
    .to_string();

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/quick-recharge/config/test")
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let user = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/quick-recharge/config/test")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(user.status(), StatusCode::FORBIDDEN);

    let admin = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/quick-recharge/config/test")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await?;
    assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let payload = body_json(admin).await?;
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for quick recharge routes")
    );

    Ok(())
}

#[tokio::test]
async fn admin_quick_recharge_test_creates_provider_order_without_wallet_order()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let _quick_recharge_lock = QUICK_RECHARGE_CONFIG_TEST_LOCK.lock().await;
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let key = settings.exposed_credential_encryption_key().unwrap();
    let secret_ciphertext = encrypt_secret("epusdt-secret", key)?;
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/payments/gmpay/v1/order/create-transaction"))
        .and(body_string_contains("pid=1000"))
        .and(body_string_contains("amount=12.5"))
        .and(body_string_contains("name=Admin+Quick+Recharge+Test"))
        .respond_with(|request: &wiremock::Request| {
            let body = String::from_utf8(request.body.clone()).unwrap();
            let order_id = form_body_value(&body, "order_id").unwrap();
            let amount = form_body_value(&body, "amount").unwrap();
            ResponseTemplate::new(200).set_body_json(json!({
                "status_code": 200,
                "message": "ok",
                "data": {
                    "trade_id": "GMTEST202606130001",
                    "order_id": order_id,
                    "amount": amount,
                    "currency": "cny",
                    "actual_amount": "3.210000000000000000",
                    "receive_address": "TQuickRechargeTestAddress",
                    "token": "usdt",
                    "expiration_time": 1_775_100_000,
                    "payment_url": "https://cashier.example/GMTEST202606130001"
                }
            }))
        })
        .mount(&server)
        .await;

    sqlx::query(
        r#"INSERT INTO quick_recharge_configs
           (name, provider, enabled, api_base_url, merchant_pid, merchant_secret_ciphertext,
            merchant_secret_mask, currency, token, network, notify_url, redirect_url,
            min_amount, max_amount, updated_by)
           VALUES ('default', 'gmpay', FALSE, ?, '1000', ?, 'epu****cret',
                   'cny', 'usdt', 'tron', 'https://merchant.example/notify',
                   'https://merchant.example/return', 10, 1000, ?)
           ON DUPLICATE KEY UPDATE
               enabled = VALUES(enabled),
               api_base_url = VALUES(api_base_url),
               merchant_pid = VALUES(merchant_pid),
               merchant_secret_ciphertext = VALUES(merchant_secret_ciphertext),
               merchant_secret_mask = VALUES(merchant_secret_mask),
               currency = VALUES(currency),
               token = VALUES(token),
               network = VALUES(network),
               notify_url = VALUES(notify_url),
               redirect_url = VALUES(redirect_url),
               min_amount = VALUES(min_amount),
               max_amount = VALUES(max_amount),
               updated_by = VALUES(updated_by)"#,
    )
    .bind(server.uri())
    .bind(&secret_ciphertext)
    .bind(admin_id)
    .execute(&pool)
    .await?;

    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let tested = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/quick-recharge/config/test")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "amount": "12.50",
                        "reason": "verify quick recharge provider"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let tested_status = tested.status();
    let tested_payload = body_json(tested).await?;
    assert_eq!(tested_status, StatusCode::OK, "payload: {tested_payload}");
    assert_eq!(tested_payload["provider_trade_id"], "GMTEST202606130001");
    assert_eq!(tested_payload["fiat_amount"], "12.500000000000000000");
    assert_eq!(tested_payload["actual_amount"], "3.210000000000000000");
    assert_eq!(
        tested_payload["payment_url"],
        "https://cashier.example/GMTEST202606130001"
    );

    let order_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM quick_recharge_orders WHERE order_id = ?")
            .bind(tested_payload["order_id"].as_str().unwrap())
            .fetch_one(&pool)
            .await?;
    assert_eq!(order_count, 0);

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type = 'quick_recharge_config'
           ORDER BY id ASC"#,
    )
    .bind(admin_id)
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].action, "quick_recharge_config.test");
    assert_eq!(
        audits[0].reason.as_deref(),
        Some("verify quick recharge provider")
    );
    let audit_text = audits[0].after_json.as_ref().unwrap().to_string();
    assert!(audit_text.contains("GMTEST202606130001"));
    assert!(!audit_text.contains("epusdt-secret"));
    assert!(!audit_text.contains(&secret_ciphertext));

    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query(
        r#"UPDATE quick_recharge_configs
           SET enabled = FALSE,
               api_base_url = NULL,
               merchant_pid = NULL,
               merchant_secret_ciphertext = NULL,
               merchant_secret_mask = NULL,
               notify_url = NULL,
               redirect_url = NULL,
               pc_app_redirect_url = NULL,
               mac_app_redirect_url = NULL,
               ios_app_redirect_url = NULL,
               android_app_redirect_url = NULL,
               mobile_web_redirect_url = NULL,
               desktop_web_redirect_url = NULL,
               currency = 'cny',
               token = 'usdt',
               network = 'tron',
               min_amount = 0.010000000000000000,
               max_amount = NULL,
               updated_by = NULL
           WHERE name = 'default'"#,
    )
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

    Ok(())
}

#[tokio::test]
async fn admin_quick_recharge_order_delete_removes_unpaid_orders_only() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let user_id = create_user(&pool).await;
    let user_email: String = sqlx::query_scalar("SELECT email FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&pool)
        .await?;
    let (asset_id, symbol) = create_asset_with_symbol(&pool, "QDR").await;
    let pending_order_id = format!("QRDEL{}", &Uuid::now_v7().simple().to_string()[..18]);
    let paid_order_id = format!("QRPAID{}", &Uuid::now_v7().simple().to_string()[..18]);
    let pending_row_id = sqlx::query(
        r#"INSERT INTO quick_recharge_orders
           (order_id, user_id, user_email, asset_id, asset_symbol, currency, token, network,
            fiat_amount, actual_amount, provider_trade_id, status)
           VALUES (?, ?, ?, ?, ?, 'cny', ?, 'tron', ?, ?, ?, 'pending')"#,
    )
    .bind(&pending_order_id)
    .bind(user_id)
    .bind(&user_email)
    .bind(asset_id)
    .bind(&symbol)
    .bind(&symbol)
    .bind(decimal("66.000000000000000000"))
    .bind(decimal("9.000000000000000000"))
    .bind(format!("GM{pending_order_id}"))
    .execute(&pool)
    .await?
    .last_insert_id();
    sqlx::query(
        r#"INSERT INTO quick_recharge_orders
           (order_id, user_id, user_email, asset_id, asset_symbol, currency, token, network,
            fiat_amount, actual_amount, provider_trade_id, status, paid_at)
           VALUES (?, ?, ?, ?, ?, 'cny', ?, 'tron', ?, ?, ?, 'paid', CURRENT_TIMESTAMP(6))"#,
    )
    .bind(&paid_order_id)
    .bind(user_id)
    .bind(&user_email)
    .bind(asset_id)
    .bind(&symbol)
    .bind(&symbol)
    .bind(decimal("88.000000000000000000"))
    .bind(decimal("12.000000000000000000"))
    .bind(format!("GM{paid_order_id}"))
    .execute(&pool)
    .await?;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let deleted = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/admin/api/v1/quick-recharge/orders/{pending_order_id}"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "reason": "delete unpaid quick recharge order" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
    let pending_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM quick_recharge_orders WHERE order_id = ?")
            .bind(&pending_order_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(pending_count, 0);
    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type = 'quick_recharge_order'
           ORDER BY id DESC"#,
    )
    .bind(admin_id)
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].action, "quick_recharge_order.delete");
    assert_eq!(audits[0].target_id, pending_row_id.to_string());
    assert_eq!(
        audits[0].reason.as_deref(),
        Some("delete unpaid quick recharge order")
    );
    assert_eq!(
        audits[0].before_json.as_ref().unwrap()["order_id"],
        pending_order_id
    );
    assert!(audits[0].after_json.is_none());

    let paid_delete = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/admin/api/v1/quick-recharge/orders/{paid_order_id}"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "reason": "delete paid quick recharge order" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let paid_delete_status = paid_delete.status();
    let paid_delete_payload = body_json(paid_delete).await?;
    assert_eq!(paid_delete_status, StatusCode::CONFLICT);
    assert_eq!(
        paid_delete_payload["message"],
        "conflict: paid quick recharge order cannot be deleted"
    );
    let paid_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM quick_recharge_orders WHERE order_id = ?")
            .bind(&paid_order_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(paid_count, 1);

    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM quick_recharge_orders WHERE order_id = ?")
        .bind(&paid_order_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
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

    Ok(())
}

#[tokio::test]
async fn gmpay_quick_recharge_notify_marks_order_paid_and_is_idempotent()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let _quick_recharge_lock = QUICK_RECHARGE_CONFIG_TEST_LOCK.lock().await;
    let settings = test_settings();
    let key = settings.exposed_credential_encryption_key().unwrap();
    let secret_ciphertext = encrypt_secret("epusdt-secret", key)?;
    let user_id = create_user(&pool).await;
    let user_email: String = sqlx::query_scalar("SELECT email FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&pool)
        .await?;
    let (asset_id, symbol) = create_asset_with_symbol(&pool, "GMP").await;
    let order_id = Uuid::now_v7().simple().to_string();
    let trade_id = format!("GMNOTIFY{}", &Uuid::now_v7().simple().to_string()[..16]);
    let notify_amount = "88.000000000000000000";
    let actual_amount = "12.345000000000000000";
    let receive_address = format!("TGmpayNotify{}", &Uuid::now_v7().simple().to_string()[..18]);
    let block_transaction_id = format!("0x{}", Uuid::now_v7().simple());

    sqlx::query(
        r#"INSERT INTO quick_recharge_configs
           (name, provider, enabled, api_base_url, merchant_pid, merchant_secret_ciphertext,
            merchant_secret_mask, currency, token, network, notify_url, redirect_url,
            min_amount, max_amount, updated_by)
           VALUES ('default', 'gmpay', TRUE, 'https://pay.example.test', '1000', ?, 'epu****cret',
                   'cny', ?, 'tron', 'https://api.example.test/api/v1/payments/gmpay/notify',
                   'https://merchant.example/return', 1, NULL, NULL)
           ON DUPLICATE KEY UPDATE
               enabled = VALUES(enabled),
               api_base_url = VALUES(api_base_url),
               merchant_pid = VALUES(merchant_pid),
               merchant_secret_ciphertext = VALUES(merchant_secret_ciphertext),
               merchant_secret_mask = VALUES(merchant_secret_mask),
               currency = VALUES(currency),
               token = VALUES(token),
               network = VALUES(network),
               notify_url = VALUES(notify_url),
               redirect_url = VALUES(redirect_url),
               min_amount = VALUES(min_amount),
               max_amount = VALUES(max_amount),
               updated_by = VALUES(updated_by)"#,
    )
    .bind(&secret_ciphertext)
    .bind(&symbol)
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"INSERT INTO quick_recharge_orders
           (order_id, user_id, user_email, asset_id, asset_symbol, currency, token, network,
            fiat_amount, provider_trade_id, actual_amount, receive_address, payment_url, status)
           VALUES (?, ?, ?, ?, ?, 'cny', ?, 'tron', ?, ?, ?, ?, 'https://cashier.example.test/pay', 'pending')"#,
    )
    .bind(&order_id)
    .bind(user_id)
    .bind(&user_email)
    .bind(asset_id)
    .bind(&symbol)
    .bind(&symbol)
    .bind(decimal(notify_amount))
    .bind(&trade_id)
    .bind(decimal(actual_amount))
    .bind(&receive_address)
    .execute(&pool)
    .await?;

    let mut signed_params = BTreeMap::new();
    signed_params.insert("pid".to_owned(), "1000".to_owned());
    signed_params.insert("trade_id".to_owned(), trade_id.clone());
    signed_params.insert("order_id".to_owned(), order_id.clone());
    signed_params.insert("amount".to_owned(), notify_amount.to_owned());
    signed_params.insert("actual_amount".to_owned(), actual_amount.to_owned());
    signed_params.insert("token".to_owned(), symbol.clone());
    signed_params.insert("status".to_owned(), "2".to_owned());
    signed_params.insert("receive_address".to_owned(), receive_address.clone());
    signed_params.insert(
        "block_transaction_id".to_owned(),
        block_transaction_id.clone(),
    );
    let signature = gmpay_signature(&signed_params, "epusdt-secret");
    let payload = json!({
        "pid": "1000",
        "trade_id": trade_id,
        "order_id": order_id,
        "amount": notify_amount,
        "actual_amount": actual_amount,
        "token": symbol,
        "status": "2",
        "receive_address": receive_address,
        "block_transaction_id": block_transaction_id,
        "signature": signature
    });

    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    for attempt in 0..2 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/payments/gmpay/notify")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await?;
        let status = response.status();
        let body = to_bytes(response.into_body(), 8192).await?;
        assert_eq!(
            status,
            StatusCode::OK,
            "attempt {attempt} payload: {}",
            String::from_utf8_lossy(&body)
        );
        assert_eq!(String::from_utf8_lossy(&body), "ok");
    }

    let order = sqlx::query_as::<_, (String, BigDecimal, String, Option<SqlxJson<Value>>)>(
        r#"SELECT status, actual_amount, block_transaction_id, callback_payload_json
           FROM quick_recharge_orders
           WHERE order_id = ?"#,
    )
    .bind(payload["order_id"].as_str().unwrap())
    .fetch_one(&pool)
    .await?;
    assert_eq!(order.0, "paid");
    assert_eq!(order.1, decimal(actual_amount));
    assert_eq!(order.2, payload["block_transaction_id"].as_str().unwrap());
    assert_eq!(
        order.3.as_ref().unwrap().0["signature"],
        payload["signature"]
    );

    let available: BigDecimal = sqlx::query_scalar(
        "SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(available, decimal(actual_amount));
    let ledger_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM wallet_ledger
           WHERE user_id = ? AND asset_id = ? AND change_type = 'quick_recharge'
             AND ref_type = 'quick_recharge' AND ref_id = ?"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(payload["order_id"].as_str().unwrap())
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 1);

    sqlx::query("DELETE FROM wallet_ledger WHERE ref_type = 'quick_recharge' AND ref_id = ?")
        .bind(payload["order_id"].as_str().unwrap())
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM quick_recharge_orders WHERE order_id = ?")
        .bind(payload["order_id"].as_str().unwrap())
        .execute(&pool)
        .await?;
    sqlx::query(
        r#"UPDATE quick_recharge_configs
           SET enabled = FALSE,
               api_base_url = NULL,
               merchant_pid = NULL,
               merchant_secret_ciphertext = NULL,
               merchant_secret_mask = NULL,
               notify_url = NULL,
               redirect_url = NULL,
               pc_app_redirect_url = NULL,
               mac_app_redirect_url = NULL,
               ios_app_redirect_url = NULL,
               android_app_redirect_url = NULL,
               mobile_web_redirect_url = NULL,
               desktop_web_redirect_url = NULL,
               currency = 'cny',
               token = 'usdt',
               network = 'tron',
               min_amount = 0.010000000000000000,
               max_amount = NULL,
               updated_by = NULL
           WHERE name = 'default'"#,
    )
    .execute(&pool)
    .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;

    Ok(())
}

#[tokio::test]
async fn admin_upload_config_save_masks_secrets_and_requires_reason() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let _upload_lock = UPLOAD_CONFIG_TEST_LOCK.lock().await;
    let settings = test_settings();
    sqlx::query("DELETE FROM upload_storage_configs WHERE name = 'default'")
        .execute(&pool)
        .await?;
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let empty = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/upload/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let empty_status = empty.status();
    let empty_payload = body_json(empty).await?;
    assert_eq!(empty_status, StatusCode::OK, "payload: {empty_payload}");
    assert_eq!(empty_payload, Value::Null);

    let missing_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/upload/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "image_bed",
                        "endpoint": "https://oss.example.test/api/v1/upload",
                        "bearer_token": "image-bed-token-value",
                        "enabled": true,
                        "reason": " "
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing_reason.status(), StatusCode::BAD_REQUEST);

    let rejected_url_with_userinfo = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/upload/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "image_bed",
                        "endpoint": "https://user:secret@oss.example.test/api/v1/upload",
                        "bearer_token": "image-bed-token-value",
                        "enabled": true,
                        "reason": "reject secret in upload endpoint"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(rejected_url_with_userinfo.status(), StatusCode::BAD_REQUEST);

    let rejected_url_with_query = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/upload/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "local",
                        "local_root": std::env::temp_dir().to_string_lossy(),
                        "public_base_url": "https://cdn.example.test/uploads?token=secret",
                        "enabled": true,
                        "reason": "reject secret in public url"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(rejected_url_with_query.status(), StatusCode::BAD_REQUEST);

    let rejected_url_with_fragment = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/upload/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "local",
                        "local_root": std::env::temp_dir().to_string_lossy(),
                        "public_base_url": "https://cdn.example.test/uploads#secret",
                        "enabled": true,
                        "reason": "reject fragment in public url"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(rejected_url_with_fragment.status(), StatusCode::BAD_REQUEST);

    let rejected_http_image_bed_endpoint = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/upload/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "image_bed",
                        "endpoint": "http://oss.example.test/api/v1/upload",
                        "bearer_token": "image-bed-token-value",
                        "enabled": true,
                        "reason": "reject insecure image bed endpoint"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(
        rejected_http_image_bed_endpoint.status(),
        StatusCode::BAD_REQUEST
    );

    let rejected_http_oss_endpoint = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/upload/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "oss",
                        "endpoint": "http://oss.example.test",
                        "bucket": "images",
                        "access_key": "access-key-value",
                        "secret_key": "secret-key-value",
                        "enabled": true,
                        "reason": "reject insecure oss endpoint"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(rejected_http_oss_endpoint.status(), StatusCode::BAD_REQUEST);

    let rejected_unsupported_mime = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/upload/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "image_bed",
                        "endpoint": "https://oss.example.test/api/v1/upload",
                        "bearer_token": "image-bed-token-value",
                        "allowed_mime_types": ["image/svg+xml"],
                        "enabled": true,
                        "reason": "reject unsupported image mime"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(rejected_unsupported_mime.status(), StatusCode::BAD_REQUEST);

    let rejected_long_file_field = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/upload/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "image_bed",
                        "endpoint": "https://oss.example.test/api/v1/upload",
                        "file_field": "f".repeat(65),
                        "bearer_token": "image-bed-token-value",
                        "enabled": true,
                        "reason": "reject long upload file field"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(rejected_long_file_field.status(), StatusCode::BAD_REQUEST);

    let rejected_long_local_root = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/upload/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "local",
                        "local_root": format!("/tmp/{}", "r".repeat(512)),
                        "public_base_url": "https://cdn.example.test/uploads",
                        "enabled": true,
                        "reason": "reject long local root"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(rejected_long_local_root.status(), StatusCode::BAD_REQUEST);

    let rejected_long_key_prefix = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/upload/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "local",
                        "local_root": std::env::temp_dir().to_string_lossy(),
                        "public_base_url": "https://cdn.example.test/uploads",
                        "key_prefix": "p".repeat(129),
                        "enabled": true,
                        "reason": "reject long key prefix"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(rejected_long_key_prefix.status(), StatusCode::BAD_REQUEST);

    let rejected_s3_bucket_with_query = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/upload/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "s3",
                        "bucket": "images?token=secret",
                        "region": "us-east-1",
                        "access_key": "access-key-value",
                        "secret_key": "secret-key-value",
                        "enabled": true,
                        "reason": "reject unsafe s3 bucket"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(
        rejected_s3_bucket_with_query.status(),
        StatusCode::BAD_REQUEST
    );

    let rejected_s3_bucket_with_path = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/upload/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "s3",
                        "bucket": "../images",
                        "region": "us-east-1",
                        "access_key": "access-key-value",
                        "secret_key": "secret-key-value",
                        "enabled": true,
                        "reason": "reject path-like s3 bucket"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(
        rejected_s3_bucket_with_path.status(),
        StatusCode::BAD_REQUEST
    );

    let rejected_s3_region_with_query = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/upload/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "s3",
                        "bucket": "images",
                        "region": "us-east-1?token=secret",
                        "access_key": "access-key-value",
                        "secret_key": "secret-key-value",
                        "enabled": true,
                        "reason": "reject unsafe s3 region"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(
        rejected_s3_region_with_query.status(),
        StatusCode::BAD_REQUEST
    );

    let rejected_oss_bucket_with_fragment = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/upload/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "oss",
                        "endpoint": "https://oss.example.test",
                        "bucket": "images#secret",
                        "access_key": "access-key-value",
                        "secret_key": "secret-key-value",
                        "enabled": true,
                        "reason": "reject unsafe oss bucket"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(
        rejected_oss_bucket_with_fragment.status(),
        StatusCode::BAD_REQUEST
    );

    let saved = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/upload/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "image_bed",
                        "endpoint": "https://oss.example.test/api/v1/upload",
                        "file_field": "file",
                        "bearer_token": "abcd1234",
                        "max_file_size_bytes": 1_048_576,
                        "allowed_mime_types": ["image/png"],
                        "enabled": true,
                        "reason": "configure upload storage"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let saved_status = saved.status();
    let saved_payload = body_json(saved).await?;
    assert_eq!(saved_status, StatusCode::OK, "payload: {saved_payload}");
    assert_eq!(saved_payload["provider"], "image_bed");
    assert_eq!(saved_payload["file_field"], "file");
    assert_eq!(saved_payload["bearer_token_mask"], "********");
    assert_eq!(saved_payload["bearer_token_set"], true);
    assert_eq!(saved_payload["bearer_token"], Value::Null);
    assert!(!saved_payload.to_string().contains("abcd1234"));

    let first_ciphertext: String = sqlx::query_scalar(
        "SELECT bearer_token_ciphertext FROM upload_storage_configs WHERE name = 'default'",
    )
    .fetch_one(&pool)
    .await?;
    assert!(!first_ciphertext.contains("abcd1234"));

    let retained = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/upload/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "image_bed",
                        "endpoint": "https://oss.example.test/api/v1/upload",
                        "file_field": "upload",
                        "bearer_token": " ",
                        "max_file_size_bytes": 2_097_152,
                        "allowed_mime_types": ["image/png", "image/jpeg"],
                        "enabled": true,
                        "reason": "keep existing upload token"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let retained_status = retained.status();
    let retained_payload = body_json(retained).await?;
    assert_eq!(
        retained_status,
        StatusCode::OK,
        "payload: {retained_payload}"
    );
    assert_eq!(retained_payload["file_field"], "upload");
    assert_eq!(retained_payload["bearer_token_mask"], "********");
    let retained_ciphertext: String = sqlx::query_scalar(
        "SELECT bearer_token_ciphertext FROM upload_storage_configs WHERE name = 'default'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(retained_ciphertext, first_ciphertext);

    let rejected_endpoint_change = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/upload/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "image_bed",
                        "endpoint": "https://other.example.test/api/v1/upload",
                        "file_field": "upload",
                        "bearer_token": " ",
                        "max_file_size_bytes": 2_097_152,
                        "allowed_mime_types": ["image/png", "image/jpeg"],
                        "enabled": true,
                        "reason": "reject upload endpoint change without token"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(rejected_endpoint_change.status(), StatusCode::BAD_REQUEST);
    let endpoint_after_reject: String =
        sqlx::query_scalar("SELECT endpoint FROM upload_storage_configs WHERE name = 'default'")
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        endpoint_after_reject,
        "https://oss.example.test/api/v1/upload"
    );

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type = 'upload_storage_config'
           ORDER BY id ASC"#,
    )
    .bind(admin_id)
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 2);
    assert_eq!(audits[0].action, "upload_storage_config.save");
    assert_eq!(
        audits[0].reason.as_deref(),
        Some("configure upload storage")
    );
    let audit_text = audits[0].after_json.as_ref().unwrap().to_string();
    assert!(audit_text.contains("bearer_token_set"));
    assert!(!audit_text.contains("image-bed-token-value"));
    assert!(!audit_text.contains(&retained_ciphertext));

    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM upload_storage_configs WHERE name = 'default'")
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
    Ok(())
}

#[tokio::test]
async fn admin_uploads_images_with_image_bed_config_and_records_object()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let _upload_lock = UPLOAD_CONFIG_TEST_LOCK.lock().await;
    let settings = test_settings();
    sqlx::query("DELETE FROM upload_storage_configs WHERE name = 'default'")
        .execute(&pool)
        .await?;
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/upload"))
        .and(header("authorization", "Bearer image-bed-token-value"))
        .and(wiremock::matchers::body_string_contains("name=\"file\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "file": {
                "id": "remote-file-id",
                "name": "2.png",
                "size": 8,
                "type": "image/gif"
            },
            "links": {
                "download": "https://oss.example.test/file/remote-file-id.png",
                "share": "https://oss.example.test/s/remote-file-id",
                "delete": "https://oss.example.test/api/v1/file/remote-file-id"
            }
        })))
        .mount(&server)
        .await;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let saved = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/upload/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "image_bed",
                        "endpoint": format!("{}/api/v1/upload", server.uri()),
                        "file_field": "file",
                        "bearer_token": "image-bed-token-value",
                        "max_file_size_bytes": 1024,
                        "allowed_mime_types": ["image/gif"],
                        "enabled": true,
                        "reason": "configure image bed upload"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(saved.status(), StatusCode::OK);

    let boundary = "upload-boundary";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"2.gif\"\r\nContent-Type: image/gif\r\n\r\nGIF89aDATA!\r\n--{boundary}--\r\n"
    );
    let uploaded = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/uploads/images")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await?;
    let upload_status = uploaded.status();
    let upload_payload = body_json(uploaded).await?;
    assert_eq!(upload_status, StatusCode::OK, "payload: {upload_payload}");
    assert_eq!(upload_payload["provider"], "image_bed");
    assert_eq!(
        upload_payload["download_url"],
        "https://oss.example.test/file/remote-file-id.png"
    );
    assert_eq!(
        upload_payload["share_url"],
        "https://oss.example.test/s/remote-file-id"
    );
    assert_eq!(upload_payload["size_bytes"], 8);
    assert_eq!(upload_payload["mime_type"], "image/gif");
    assert!(!upload_payload.to_string().contains("image-bed-token-value"));

    let stored: (String, String, String, Option<String>, u64) = sqlx::query_as(
        r#"SELECT provider, object_key, public_url, original_filename, uploaded_by
           FROM upload_objects
           WHERE uploaded_by = ?
           ORDER BY id DESC
           LIMIT 1"#,
    )
    .bind(admin_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(stored.0, "image_bed");
    assert_eq!(stored.1, upload_payload["object_key"].as_str().unwrap());
    assert_eq!(stored.2, "https://oss.example.test/file/remote-file-id.png");
    assert_eq!(stored.3.as_deref(), Some("2.gif"));
    assert_eq!(stored.4, admin_id);

    sqlx::query("DELETE FROM upload_storage_configs WHERE name = 'default'")
        .execute(&pool)
        .await?;
    let long_remote_id = "r".repeat(600);
    let long_remote_mime = format!("image/{}", "x".repeat(200));
    let long_download_url = format!("https://oss.example.test/file/{}", "d".repeat(1100));
    let long_share_url = format!("https://oss.example.test/s/{}", "s".repeat(1100));
    let long_delete_url = format!("https://oss.example.test/api/v1/file/{}", "x".repeat(1100));
    Mock::given(method("POST"))
        .and(path("/api/v1/long-response-upload"))
        .and(header("authorization", "Bearer image-bed-token-value"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "file": {
                "id": long_remote_id,
                "name": "remote.gif",
                "size": 8,
                "type": long_remote_mime
            },
            "links": {
                "download": long_download_url,
                "share": long_share_url,
                "delete": long_delete_url
            }
        })))
        .mount(&server)
        .await;
    let saved = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/upload/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "image_bed",
                        "endpoint": format!("{}/api/v1/long-response-upload", server.uri()),
                        "file_field": "file",
                        "bearer_token": "image-bed-token-value",
                        "max_file_size_bytes": 1024,
                        "allowed_mime_types": ["image/gif"],
                        "enabled": true,
                        "reason": "configure image bed upload with long response"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(saved.status(), StatusCode::OK);

    let boundary = "long-response-boundary";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"long.gif\"\r\nContent-Type: image/gif\r\n\r\nGIF89aDATA!\r\n--{boundary}--\r\n"
    );
    let uploaded = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/uploads/images")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await?;
    let upload_status = uploaded.status();
    let upload_payload = body_json(uploaded).await?;
    assert_eq!(upload_status, StatusCode::OK, "payload: {upload_payload}");
    assert_ne!(upload_payload["object_key"], Value::String("r".repeat(600)));
    assert_eq!(upload_payload["mime_type"], "image/gif");
    let stored: (String, String, Option<String>, Option<String>, String) = sqlx::query_as(
        r#"SELECT object_key, public_url, share_url, delete_url, mime_type
           FROM upload_objects
           WHERE uploaded_by = ?
           ORDER BY id DESC
           LIMIT 1"#,
    )
    .bind(admin_id)
    .fetch_one(&pool)
    .await?;
    assert!(stored.0.len() <= 512);
    assert_eq!(stored.1, upload_payload["download_url"].as_str().unwrap());
    assert_eq!(stored.2.as_deref(), upload_payload["share_url"].as_str());
    assert_eq!(stored.3.as_deref(), upload_payload["delete_url"].as_str());
    assert_eq!(stored.4, "image/gif");

    Mock::given(method("POST"))
        .and(path("/api/v1/unsafe-response-upload"))
        .and(header("authorization", "Bearer image-bed-token-value"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "file": {
                "id": "unsafe-response",
                "name": "unsafe.gif",
                "size": 8,
                "type": "image/gif"
            },
            "links": {
                "download": "javascript:alert(1)",
                "share": "https://user:secret@oss.example.test/s/unsafe-response",
                "delete": "https://oss.example.test/api/v1/file/unsafe-response?token=secret"
            }
        })))
        .mount(&server)
        .await;
    let saved = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/upload/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "image_bed",
                        "endpoint": format!("{}/api/v1/unsafe-response-upload", server.uri()),
                        "file_field": "file",
                        "bearer_token": "image-bed-token-value",
                        "max_file_size_bytes": 1024,
                        "allowed_mime_types": ["image/gif"],
                        "enabled": true,
                        "reason": "configure image bed upload with unsafe response"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(saved.status(), StatusCode::OK);

    let boundary = "unsafe-response-boundary";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"unsafe.gif\"\r\nContent-Type: image/gif\r\n\r\nGIF89aDATA!\r\n--{boundary}--\r\n"
    );
    let rejected_unsafe_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/uploads/images")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await?;
    let rejected_status = rejected_unsafe_response.status();
    let rejected_payload = body_json(rejected_unsafe_response).await?;
    assert_eq!(
        rejected_status,
        StatusCode::BAD_REQUEST,
        "payload: {rejected_payload}"
    );

    sqlx::query("DELETE FROM upload_objects WHERE uploaded_by = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM upload_storage_configs WHERE name = 'default'")
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
    Ok(())
}

#[tokio::test]
async fn admin_uploads_images_accepts_configured_size_above_axum_default_limit()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let _upload_lock = UPLOAD_CONFIG_TEST_LOCK.lock().await;
    let settings = test_settings();
    sqlx::query("DELETE FROM upload_storage_configs WHERE name = 'default'")
        .execute(&pool)
        .await?;
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let local_root = std::env::temp_dir().join(format!(
        "exchange-large-upload-test-{}",
        Uuid::now_v7().simple()
    ));
    fs::create_dir_all(&local_root)?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let saved = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/upload/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "local",
                        "local_root": local_root.to_string_lossy(),
                        "public_base_url": "https://cdn.example.test/uploads",
                        "key_prefix": "large-images",
                        "max_file_size_bytes": 4_000_000,
                        "allowed_mime_types": ["image/gif"],
                        "enabled": true,
                        "reason": "configure large local upload"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(saved.status(), StatusCode::OK);

    let boundary = "large-upload-boundary";
    let large_bytes = "A".repeat(3_000_000);
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"large.gif\"\r\nContent-Type: image/gif\r\n\r\nGIF89a{large_bytes}\r\n--{boundary}--\r\n"
    );
    let uploaded = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/uploads/images")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await?;
    let upload_status = uploaded.status();
    let upload_body = to_bytes(uploaded.into_body(), usize::MAX).await?;
    assert_eq!(
        upload_status,
        StatusCode::OK,
        "body: {}",
        String::from_utf8_lossy(&upload_body)
    );
    let upload_payload: Value = serde_json::from_slice(&upload_body)?;
    let object_key = upload_payload["object_key"].as_str().unwrap();
    assert!(object_key.starts_with("large-images/"));
    assert!(local_root.join(object_key).is_file());

    sqlx::query("DELETE FROM upload_objects WHERE uploaded_by = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM upload_storage_configs WHERE name = 'default'")
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
    fs::remove_dir_all(local_root)?;
    Ok(())
}

#[tokio::test]
async fn admin_uploads_images_to_local_storage_with_safe_object_key() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let _upload_lock = UPLOAD_CONFIG_TEST_LOCK.lock().await;
    let settings = test_settings();
    sqlx::query("DELETE FROM upload_storage_configs WHERE name = 'default'")
        .execute(&pool)
        .await?;
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let local_root =
        std::env::temp_dir().join(format!("exchange-upload-test-{}", Uuid::now_v7().simple()));
    fs::create_dir_all(&local_root)?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let rejected_prefix = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/upload/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "local",
                        "local_root": local_root.to_string_lossy(),
                        "public_base_url": "https://cdn.example.test/uploads",
                        "key_prefix": "../escape",
                        "max_file_size_bytes": 1024,
                        "allowed_mime_types": ["image/gif"],
                        "enabled": true,
                        "reason": "reject unsafe local prefix"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(rejected_prefix.status(), StatusCode::BAD_REQUEST);

    let saved = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/upload/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "local",
                        "local_root": local_root.to_string_lossy(),
                        "public_base_url": "https://cdn.example.test/uploads",
                        "key_prefix": "images",
                        "max_file_size_bytes": 1024,
                        "allowed_mime_types": ["image/gif"],
                        "enabled": true,
                        "reason": "configure local upload"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(saved.status(), StatusCode::OK);

    let boundary = "local-upload-boundary";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"../../evil.gif\"\r\nContent-Type: image/gif\r\n\r\nGIF89aDATA!\r\n--{boundary}--\r\n"
    );
    let uploaded = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/uploads/images")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await?;
    let upload_status = uploaded.status();
    let upload_payload = body_json(uploaded).await?;
    assert_eq!(upload_status, StatusCode::OK, "payload: {upload_payload}");
    let object_key = upload_payload["object_key"].as_str().unwrap();
    assert!(object_key.starts_with("images/"));
    assert!(!object_key.contains(".."));
    assert!(!object_key.contains("evil"));
    assert!(local_root.join(object_key).is_file());
    assert_eq!(
        upload_payload["download_url"],
        format!("https://cdn.example.test/uploads/{object_key}")
    );

    let boundary = "long-name-upload-boundary";
    let long_filename = format!("{}{}.gif", "a".repeat(320), "-avatar");
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{long_filename}\"\r\nContent-Type: image/gif\r\n\r\nGIF89aDATA!\r\n--{boundary}--\r\n"
    );
    let long_name_upload = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/uploads/images")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await?;
    let long_name_status = long_name_upload.status();
    let long_name_payload = body_json(long_name_upload).await?;
    assert_eq!(
        long_name_status,
        StatusCode::OK,
        "payload: {long_name_payload}"
    );
    let stored_filename: String = sqlx::query_scalar(
        r#"SELECT original_filename
           FROM upload_objects
           WHERE uploaded_by = ?
           ORDER BY id DESC
           LIMIT 1"#,
    )
    .bind(admin_id)
    .fetch_one(&pool)
    .await?;
    assert!(stored_filename.len() <= 255);
    assert!(stored_filename.ends_with(".gif"));
    assert!(!stored_filename.contains('/'));
    assert!(!stored_filename.contains('\\'));

    let boundary = "oversized-upload-boundary";
    let oversized_bytes = "A".repeat(2_000_000);
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"huge.gif\"\r\nContent-Type: image/gif\r\n\r\nGIF89a{oversized_bytes}\r\n--{boundary}--\r\n"
    );
    let oversized = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/uploads/images")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await?;
    assert_eq!(oversized.status(), StatusCode::BAD_REQUEST);

    let boundary = "bad-upload-boundary";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"bad.txt\"\r\nContent-Type: text/plain\r\n\r\nTEXT\r\n--{boundary}--\r\n"
    );
    let rejected = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/uploads/images")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await?;
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);

    sqlx::query("DELETE FROM upload_objects WHERE uploaded_by = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM upload_storage_configs WHERE name = 'default'")
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
    fs::remove_dir_all(local_root)?;
    Ok(())
}

#[tokio::test]
async fn admin_market_feed_routes_require_admin_scope_mysql_and_supervisor()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/market-feed/config")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let user = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/market-feed/config")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(user.status(), StatusCode::FORBIDDEN);

    let admin = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/market-feed/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let payload = body_json(admin).await?;
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for admin convert routes")
    );

    Ok(())
}

#[tokio::test]
async fn admin_market_feed_rejects_invalid_interval() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let invalid = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/market-feed/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "symbols": ["BTC-USDT"],
                        "intervals": ["2m"],
                        "providers": ["bitget"],
                        "enabled": true,
                        "reason": "invalid interval"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let invalid_payload = body_json(invalid).await?;
    assert_eq!(invalid_payload["code"], "VALIDATION_ERROR");

    let multi_provider = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/market-feed/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "symbols": ["BTC-USDT"],
                        "intervals": ["1m"],
                        "providers": ["bitget", "htx"],
                        "enabled": true,
                        "reason": "multiple providers"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(multi_provider.status(), StatusCode::BAD_REQUEST);
    let multi_provider_payload = body_json(multi_provider).await?;
    assert_eq!(multi_provider_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        multi_provider_payload["message"],
        "validation error: market feed only supports one enabled provider"
    );

    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(role_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn admin_market_feed_config_credentials_reload_and_status() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let _market_feed_lock = MARKET_FEED_CONFIG_TEST_LOCK.lock().await;
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let supervisor = MarketFeedSupervisorHandle::new_for_tests();
    let app = build_router(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_market_feed_supervisor(supervisor.clone()),
    );

    let saved = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/market-feed/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "symbols": ["BTC-USDT", "ETH_USDT"],
                        "intervals": ["1m", "5m", "1h"],
                        "providers": ["bitget"],
                        "enabled": true,
                        "reason": "enable external market feed"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let saved_status = saved.status();
    let saved_payload = body_json(saved).await?;
    assert_eq!(saved_status, StatusCode::OK, "payload: {saved_payload}");
    let config_id = saved_payload["id"].as_u64().unwrap();
    let saved_version = saved_payload["version"].as_u64().unwrap();
    assert_eq!(saved_payload["symbols"], json!(["BTCUSDT", "ETHUSDT"]));
    assert_eq!(saved_payload["providers"], json!(["bitget"]));
    assert_eq!(saved_payload["needs_reload"], true);

    let credential = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/market-feed/credentials/bitget")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "auth_type": "api_key",
                        "api_key": "abcd1234wxyz",
                        "api_secret": "secret-value",
                        "passphrase": "pass-value",
                        "enabled": true,
                        "reason": "store bitget credential"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let credential_status = credential.status();
    let credential_payload = body_json(credential).await?;
    assert_eq!(
        credential_status,
        StatusCode::OK,
        "payload: {credential_payload}"
    );
    assert_eq!(credential_payload["provider"], "bitget");
    assert_eq!(credential_payload["api_key_mask"], "abcd****wxyz");
    assert_eq!(credential_payload["api_key"], Value::Null);
    assert_eq!(credential_payload["api_secret"], Value::Null);
    assert_eq!(credential_payload["passphrase"], Value::Null);

    let ciphertext: String = sqlx::query_scalar(
        "SELECT api_key_ciphertext FROM market_source_credentials WHERE provider = 'bitget'",
    )
    .fetch_one(&pool)
    .await?;
    assert_ne!(ciphertext, "abcd1234wxyz");
    assert!(!ciphertext.contains("abcd1234wxyz"));

    let credentials = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/market-feed/credentials")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let credentials_payload = body_json(credentials).await?;
    assert_eq!(
        credentials_payload["credentials"][0]["api_key_mask"],
        "abcd****wxyz"
    );
    assert!(!credentials_payload.to_string().contains("secret-value"));

    let reload = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/market-feed/reload")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "reason": "apply market feed config" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let reload_status = reload.status();
    let reload_payload = body_json(reload).await?;
    assert_eq!(reload_status, StatusCode::OK, "payload: {reload_payload}");
    assert_eq!(reload_payload["config"]["needs_reload"], false);
    assert_eq!(reload_payload["config"]["last_reload_status"], "success");
    assert_eq!(reload_payload["runtime"]["last_reload_status"], "success");
    assert_eq!(
        reload_payload["runtime"]["symbols"],
        json!(["BTCUSDT", "ETHUSDT"])
    );
    assert_eq!(reload_payload["runtime"]["providers"], json!(["bitget"]));
    supervisor.stop().await;

    let status = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/market-feed/status")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let status_payload = body_json(status).await?;
    assert_eq!(status_payload["saved_config"]["id"], config_id);
    assert_eq!(status_payload["runtime"]["applied_version"], saved_version);

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type IN ('market_feed_config', 'market_source_credential')
           ORDER BY id ASC"#,
    )
    .bind(admin_id)
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 3);
    assert_eq!(audits[0].action, "market_feed_config.save");
    assert_eq!(audits[1].action, "market_source_credential.upsert");
    assert_eq!(
        audits[1].after_json.as_ref().unwrap()["api_key_mask"],
        "abcd****wxyz"
    );
    assert!(
        !audits[1]
            .after_json
            .as_ref()
            .unwrap()
            .to_string()
            .contains("secret-value")
    );
    assert_eq!(audits[2].action, "market_feed_config.reload");
    assert_eq!(
        audits[2].reason.as_deref(),
        Some("apply market feed config")
    );

    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM market_source_credentials WHERE provider IN ('bitget', 'htx')")
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM market_feed_configs WHERE id = ?")
        .bind(config_id)
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
    Ok(())
}

#[tokio::test]
async fn admin_market_feed_reload_skips_disabled_config() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let _market_feed_lock = MARKET_FEED_CONFIG_TEST_LOCK.lock().await;
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_market_feed_supervisor(MarketFeedSupervisorHandle::new_for_tests()),
    );

    let saved = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/market-feed/config")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "symbols": [],
                        "intervals": ["1m"],
                        "providers": ["htx"],
                        "enabled": false,
                        "reason": "disable market feed"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let saved_payload = body_json(saved).await?;
    let config_id = saved_payload["id"].as_u64().unwrap();

    let reload = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/market-feed/reload")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "reason": "stop feed subscriptions" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let reload_status = reload.status();
    let reload_payload = body_json(reload).await?;
    assert_eq!(reload_status, StatusCode::OK, "payload: {reload_payload}");
    assert_eq!(reload_payload["config"]["last_reload_status"], "skipped");
    assert_eq!(reload_payload["runtime"]["last_reload_status"], "skipped");

    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM market_feed_configs WHERE id = ?")
        .bind(config_id)
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
    Ok(())
}

#[tokio::test]
async fn admin_lists_users_and_reads_user_detail() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (_role_id, admin_id) = create_admin_user(&pool).await;
    let email = format!("admin-list-user-{}@example.test", Uuid::now_v7().simple());
    let user_id = create_user_with_email(&pool, email.clone()).await;
    let other_user_id = create_user(&pool).await;
    let invite_code = Uuid::now_v7().simple().to_string()[..6].to_ascii_uppercase();
    let phone_suffix = Uuid::now_v7().simple().to_string();
    sqlx::query("UPDATE users SET phone = ?, kyc_level = 2 WHERE id = ?")
        .bind(format!("+8613{}", &phone_suffix[16..25]))
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO invite_codes (owner_type, owner_id, code, status) VALUES ('user', ?, ?, 'active')")
        .bind(user_id)
        .bind(&invite_code)
        .execute(&pool)
        .await?;
    sqlx::query("UPDATE users SET status = 'suspended' WHERE id = ?")
        .bind(other_user_id)
        .execute(&pool)
        .await?;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/users?email={email}&status=active&limit=10"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let list_status = list.status();
    let list_payload = body_json(list).await?;
    assert_eq!(list_status, StatusCode::OK, "payload: {list_payload}");
    let users = list_payload["users"].as_array().unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0]["id"], user_id);
    assert_eq!(users[0]["email"], email);
    assert_eq!(users[0]["invite_code"], invite_code);
    assert_eq!(users[0]["status"], "active");
    assert_eq!(users[0]["kyc_level"], 2);
    assert!(users[0]["created_at"].is_number());
    assert!(users[0]["updated_at"].is_number());

    let email_list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/api/v1/users?email={email}&limit=10"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let email_list_status = email_list.status();
    let email_list_payload = body_json(email_list).await?;
    assert_eq!(
        email_list_status,
        StatusCode::OK,
        "payload: {email_list_payload}"
    );
    let email_users = email_list_payload["users"].as_array().unwrap();
    assert_eq!(email_users.len(), 1);
    assert_eq!(email_users[0]["id"], user_id);
    assert_eq!(email_users[0]["email"], email);
    assert_eq!(email_users[0]["invite_code"], invite_code);

    let detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/api/v1/users/{user_id}"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let detail_status = detail.status();
    let detail_payload = body_json(detail).await?;
    assert_eq!(detail_status, StatusCode::OK, "payload: {detail_payload}");
    assert_eq!(detail_payload["id"], user_id);
    assert_eq!(detail_payload["email"], email);
    assert_eq!(detail_payload["invite_code"], invite_code);

    sqlx::query("DELETE FROM invite_codes WHERE owner_type = 'user' AND owner_id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id IN (?, ?)")
        .bind(user_id)
        .bind(other_user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn admin_create_user_creates_hashed_user_and_audit_log() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (_role_id, admin_id) = create_admin_user(&pool).await;
    let email = format!("admin-create-user-{}@example.test", Uuid::now_v7().simple());
    let phone_suffix = Uuid::now_v7().simple().to_string();
    let phone = format!("+8613{}", &phone_suffix[16..25]);
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/users")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": email,
                        "phone": phone,
                        "password": "Password123!",
                        "status": "active",
                        "kyc_level": 2,
                        "reason": "create support user"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let create_status = create.status();
    let create_payload = body_json(create).await?;
    assert_eq!(create_status, StatusCode::OK, "payload: {create_payload}");
    let user_id = create_payload["id"].as_u64().unwrap();
    assert_eq!(create_payload["email"], email);
    assert_eq!(create_payload["phone"], phone);
    assert_eq!(create_payload["status"], "active");
    assert_eq!(create_payload["kyc_level"], 2);
    let invite_code = create_payload["invite_code"].as_str().unwrap();
    assert_eq!(invite_code.len(), 6);
    assert!(
        invite_code
            .chars()
            .all(|char| char.is_ascii_uppercase() || char.is_ascii_digit())
    );

    let stored = sqlx::query_as::<_, (String,)>("SELECT password_hash FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&pool)
        .await?;
    assert_ne!(stored.0, "Password123!");
    assert!(stored.0.starts_with("$argon2"));
    let stored_invite_code: String = sqlx::query_scalar(
        "SELECT code FROM invite_codes WHERE owner_type = 'user' AND owner_id = ? LIMIT 1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(stored_invite_code, invite_code);

    let audit = sqlx::query_as::<_, (String, String, String)>(
        r#"SELECT action, target_type, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_id = ?
           ORDER BY id DESC LIMIT 1"#,
    )
    .bind(admin_id)
    .bind(user_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(audit.0, "user.create");
    assert_eq!(audit.1, "user");
    assert_eq!(audit.2, "create support user");

    let missing_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/users")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": format!("missing-reason-{}@example.test", Uuid::now_v7().simple()),
                        "password": "Password123!"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing_reason.status(), StatusCode::BAD_REQUEST);

    let duplicate = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/users")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": create_payload["email"].as_str().unwrap(),
                        "password": "Password123!",
                        "reason": "duplicate user"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);

    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ? AND target_type = 'user'")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM invite_codes WHERE owner_type = 'user' AND owner_id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn admin_recharges_user_wallet_with_ledger_and_audit() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let (other_role_id, other_admin_id) = create_admin_user(&pool).await;
    let user_id = create_user(&pool).await;
    let (asset_id, symbol) = create_asset_with_symbol(&pool, "ARU").await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let other_token = issue_token(
        &settings,
        format!("admin:{other_admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let idempotency_key = format!("admin-recharge-{}", Uuid::now_v7().simple());

    let missing_key = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/api/v1/users/{user_id}/recharge"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "asset_id": asset_id,
                        "amount": "25.500000000000000000",
                        "reason": "manual support recharge"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing_key.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let request_body = json!({
        "asset_id": asset_id,
        "amount": "25.500000000000000000",
        "reason": "manual support recharge",
        "idempotency_key": idempotency_key
    })
    .to_string();
    let mut concurrent = tokio::task::JoinSet::new();
    for _ in 0..20 {
        let request_app = app.clone();
        let request_token = token.clone();
        let request_body = request_body.clone();
        concurrent.spawn(async move {
            request_app
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(format!("/admin/api/v1/users/{user_id}/recharge"))
                        .header(AUTHORIZATION, format!("Bearer {request_token}"))
                        .header("content-type", "application/json")
                        .body(Body::from(request_body))
                        .unwrap(),
                )
                .await
                .unwrap()
        });
    }
    let mut concurrent_payloads = Vec::new();
    while let Some(response) = concurrent.join_next().await {
        let response = response?;
        let status = response.status();
        let payload = body_json(response).await?;
        assert_eq!(status, StatusCode::OK, "payload: {payload}");
        concurrent_payloads.push(payload);
    }
    assert_eq!(concurrent_payloads.len(), 20);
    let created_payload = concurrent_payloads[0].clone();
    assert!(
        concurrent_payloads
            .iter()
            .all(|payload| payload == &created_payload)
    );
    assert_eq!(created_payload["user_id"], user_id);
    assert_eq!(created_payload["asset_id"], asset_id);
    assert_eq!(created_payload["asset_symbol"], symbol);
    assert_eq!(created_payload["amount"], "25.500000000000000000");
    assert_eq!(created_payload["available"], "25.500000000000000000");
    assert_eq!(created_payload["frozen"], "0.000000000000000000");
    assert_eq!(created_payload["locked"], "0.000000000000000000");
    let recharge_id = created_payload["recharge_id"].as_str().unwrap().to_owned();

    let conflict = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/api/v1/users/{user_id}/recharge"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "asset_id": asset_id,
                        "amount": "25.600000000000000000",
                        "reason": "manual support recharge",
                        "idempotency_key": idempotency_key
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(conflict.status(), StatusCode::CONFLICT);

    let available: BigDecimal = sqlx::query_scalar(
        "SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(available, decimal("25.500000000000000000"));

    let ledger = sqlx::query_as::<_, (String, BigDecimal, String, BigDecimal, String, String)>(
        r#"SELECT change_type, amount, balance_type, available_after, ref_type, ref_id
           FROM wallet_ledger
           WHERE user_id = ? AND asset_id = ?
           ORDER BY id DESC LIMIT 1"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger.0, "admin_recharge");
    assert_eq!(ledger.1, decimal("25.500000000000000000"));
    assert_eq!(ledger.2, "available");
    assert_eq!(ledger.3, decimal("25.500000000000000000"));
    assert_eq!(ledger.4, "admin_recharge");
    assert_eq!(ledger.5, recharge_id);

    let audit = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type = 'wallet_account'
           ORDER BY id DESC LIMIT 1"#,
    )
    .bind(admin_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(audit.action, "wallet.recharge");
    assert_eq!(audit.target_id, user_id.to_string());
    assert_eq!(audit.reason.as_deref(), Some("manual support recharge"));
    let after_json = audit.after_json.as_ref().unwrap();
    assert_eq!(after_json["user_id"], user_id);
    assert_eq!(after_json["asset_id"], asset_id);
    assert_eq!(after_json["asset_symbol"], symbol);
    assert_eq!(after_json["amount"], "25.500000000000000000");
    assert_eq!(after_json["available"], "25.500000000000000000");

    let second = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/api/v1/users/{user_id}/recharge"))
                .header(AUTHORIZATION, format!("Bearer {other_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "asset_id": asset_id,
                        "amount": "4.500000000000000000",
                        "reason": "second support recharge",
                        "idempotency_key": idempotency_key
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let second_status = second.status();
    let second_payload = body_json(second).await?;
    assert_eq!(second_status, StatusCode::OK, "payload: {second_payload}");
    assert_eq!(second_payload["available"], "30.000000000000000000");

    let replay = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/api/v1/users/{user_id}/recharge"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "asset_id": asset_id,
                        "amount": "25.5000",
                        "reason": "  manual support recharge  ",
                        "idempotency_key": format!("  {idempotency_key}  ")
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let replay_status = replay.status();
    let replay_payload = body_json(replay).await?;
    assert_eq!(replay_status, StatusCode::OK, "payload: {replay_payload}");
    assert_eq!(replay_payload, created_payload);

    let missing_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/api/v1/users/{user_id}/recharge"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "asset_id": asset_id,
                        "amount": "1.000000000000000000",
                        "idempotency_key": "admin-recharge-missing-reason"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing_reason.status(), StatusCode::BAD_REQUEST);

    let invalid_amount = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/api/v1/users/{user_id}/recharge"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "asset_id": asset_id,
                        "amount": "0.000000000000000000",
                        "reason": "zero recharge",
                        "idempotency_key": "admin-recharge-zero"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid_amount.status(), StatusCode::BAD_REQUEST);

    let invalid_user = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/admin/api/v1/users/{}/recharge",
                    user_id + 999_999
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "asset_id": asset_id,
                        "amount": "1.000000000000000000",
                        "reason": "missing user recharge",
                        "idempotency_key": "admin-recharge-missing-user"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid_user.status(), StatusCode::NOT_FOUND);

    let (receipt_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM admin_wallet_recharges WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE user_id = ? AND asset_id = ? AND ref_type = 'admin_recharge'",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    let (audit_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM admin_audit_logs WHERE target_type = 'wallet_account' AND target_id = ? AND action = 'wallet.recharge'",
    )
    .bind(user_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(receipt_count, 2);
    assert_eq!(ledger_count, 2);
    assert_eq!(audit_count, 2);

    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id IN (?, ?)")
        .bind(admin_id)
        .bind(other_admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_wallet_recharges WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_ledger WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_users WHERE id IN (?, ?)")
        .bind(admin_id)
        .bind(other_admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id IN (?, ?)")
        .bind(role_id)
        .bind(other_role_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn admin_deposit_address_pool_create_list_update_reclaim_and_audit()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let user_email = format!(
        "admin-deposit-address-{}@example.test",
        Uuid::now_v7().simple()
    );
    let user_id = create_user_with_email(&pool, user_email.clone()).await;
    let (asset_id, symbol) = create_asset_with_symbol(&pool, "ADA").await;
    upsert_deposit_network_config(&pool, "base", "A").await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let address = format!("0x{}", Uuid::now_v7().simple());

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/deposit-address-pool")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "network": "Base",
                        "address": address,
                        "asset_symbol": symbol.to_ascii_lowercase(),
                        "status": "available",
                        "memo": "memo-1",
                        "remark": "initial pool address",
                        "reason": "create deposit address"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let create_status = create.status();
    let created = body_json(create).await?;
    assert_eq!(create_status, StatusCode::OK, "payload: {created}");
    let address_id = created["id"].as_u64().unwrap();
    assert_eq!(created["network"], "base");
    assert_eq!(created["address_group_code"], "A");
    assert_eq!(created["asset_symbol"], symbol);
    assert_eq!(created["asset_symbols"], json!([symbol.clone()]));
    assert_eq!(created["status"], "available");

    let single_asset_symbols = json!([symbol.clone()]);
    let listed = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/deposit-address-pool?network=base&status=available&asset_symbol={symbol}&address={address}&limit=10"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let listed_status = listed.status();
    let listed_payload = body_json(listed).await?;
    assert_eq!(listed_status, StatusCode::OK, "payload: {listed_payload}");
    assert!(
        listed_payload["addresses"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| {
                item["id"] == address_id
                    && item["network"] == "base"
                    && item["address_group_code"] == "A"
                    && item["asset_symbol"] == symbol
                    && item["asset_symbols"] == single_asset_symbols
                    && item["status"] == "available"
            })
    );

    let update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/deposit-address-pool/{address_id}"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "network": "base",
                        "address": address,
                        "asset_symbol": symbol,
                        "status": "disabled",
                        "memo": "memo-2",
                        "remark": "paused pool address",
                        "reason": "pause deposit address"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let update_status = update.status();
    let updated = body_json(update).await?;
    assert_eq!(update_status, StatusCode::OK, "payload: {updated}");
    assert_eq!(updated["status"], "disabled");
    assert_eq!(updated["memo"], "memo-2");
    assert_eq!(updated["asset_symbols"], single_asset_symbols);

    sqlx::query(
        r#"UPDATE deposit_address_pool
           SET status = 'assigned',
               assigned_user_id = ?,
               assigned_user_email = ?,
               assigned_asset_symbol = ?,
               assigned_at = CURRENT_TIMESTAMP(6)
           WHERE id = ?"#,
    )
    .bind(user_id)
    .bind(&user_email)
    .bind(&symbol)
    .bind(address_id)
    .execute(&pool)
    .await?;

    let assigned_list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/deposit-address-pool?email={user_email}&status=assigned&limit=10"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let assigned_payload = body_json(assigned_list).await?;
    let assigned_rows = assigned_payload["addresses"].as_array().unwrap();
    assert!(assigned_rows.iter().any(|item| {
        item["id"] == address_id
            && item["assigned_user_email"] == user_email
            && item["assigned_asset_symbol"] == symbol
    }));

    let reclaim = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/admin/api/v1/deposit-address-pool/{address_id}/reclaim"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "reason": "reclaim address" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let reclaim_status = reclaim.status();
    let reclaimed = body_json(reclaim).await?;
    assert_eq!(reclaim_status, StatusCode::OK, "payload: {reclaimed}");
    assert_eq!(reclaimed["status"], "available");
    assert!(reclaimed["assigned_user_id"].is_null());
    assert!(reclaimed["assigned_asset_symbol"].is_null());

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type = 'deposit_address_pool' AND target_id = ?
           ORDER BY id"#,
    )
    .bind(admin_id)
    .bind(address_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 3, "audits: {audits:?}");
    assert_eq!(audits[0].action, "deposit_address_pool.create");
    assert!(audits[0].before_json.is_none());
    assert_eq!(audits[0].after_json.as_ref().unwrap()["network"], "base");
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["asset_symbols"],
        single_asset_symbols
    );
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["address_group_code"],
        "A"
    );
    assert_eq!(audits[0].reason.as_deref(), Some("create deposit address"));
    assert_eq!(audits[1].action, "deposit_address_pool.update");
    assert_eq!(
        audits[1].before_json.as_ref().unwrap()["status"],
        "available"
    );
    assert_eq!(audits[1].after_json.as_ref().unwrap()["status"], "disabled");
    assert_eq!(audits[1].reason.as_deref(), Some("pause deposit address"));
    assert_eq!(audits[2].action, "deposit_address_pool.reclaim");
    assert_eq!(
        audits[2].before_json.as_ref().unwrap()["status"],
        "assigned"
    );
    assert_eq!(
        audits[2].after_json.as_ref().unwrap()["status"],
        "available"
    );
    assert_eq!(audits[2].reason.as_deref(), Some("reclaim address"));

    sqlx::query(
        "DELETE FROM admin_audit_logs WHERE admin_id = ? AND target_type = 'deposit_address_pool'",
    )
    .bind(admin_id)
    .execute(&pool)
    .await?;
    sqlx::query("DELETE FROM deposit_address_pool WHERE id = ?")
        .bind(address_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
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
    Ok(())
}

#[tokio::test]
async fn admin_deposit_address_pool_batch_create_supports_multiple_assets()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let (asset_id, symbol) = create_asset_with_symbol(&pool, "DAB").await;
    let (asset_id_two, symbol_two) = create_asset_with_symbol(&pool, "DAC").await;
    upsert_deposit_network_config(&pool, "tron", "C").await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let address_one = format!("TBatch{}", &Uuid::now_v7().simple().to_string()[..24]);
    let address_two = format!("TBatch{}", &Uuid::now_v7().simple().to_string()[..24]);

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/deposit-address-pool/batch")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "network": "tron",
                        "asset_symbols": [symbol.to_ascii_lowercase(), symbol_two.clone()],
                        "status": "available",
                        "entries": [
                            { "address": address_one, "memo": "memo-a", "remark": "primary tron address" },
                            { "address": address_two, "memo": "memo-b", "remark": "backup tron address" }
                        ],
                        "reason": "batch create deposit addresses"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let create_status = create.status();
    let created = body_json(create).await?;
    assert_eq!(create_status, StatusCode::OK, "payload: {created}");
    let rows = created["addresses"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "payload: {created}");
    let asset_symbols = json!([symbol.clone(), symbol_two.clone()]);
    let created_ids: Vec<u64> = rows
        .iter()
        .map(|row| {
            assert_eq!(row["network"], "tron");
            assert_eq!(row["address_group_code"], "C");
            assert!(row["asset_symbol"].is_null());
            assert_eq!(row["asset_symbols"], asset_symbols);
            row["id"].as_u64().unwrap()
        })
        .collect();

    let listed = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/deposit-address-pool?network=tron&asset_symbol={symbol_two}&limit=10"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let listed_payload = body_json(listed).await?;
    let listed_rows = listed_payload["addresses"].as_array().unwrap();
    assert!(
        created_ids
            .iter()
            .all(|id| listed_rows.iter().any(|row| row["id"] == *id))
    );

    let audit_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM admin_audit_logs
           WHERE admin_id = ?
             AND target_type = 'deposit_address_pool'
             AND action = 'deposit_address_pool.create'
             AND reason = 'batch create deposit addresses'"#,
    )
    .bind(admin_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(audit_count, 2);

    sqlx::query(
        "DELETE FROM admin_audit_logs WHERE admin_id = ? AND target_type = 'deposit_address_pool'",
    )
    .bind(admin_id)
    .execute(&pool)
    .await?;
    sqlx::query("DELETE FROM deposit_address_pool WHERE id IN (?, ?)")
        .bind(created_ids[0])
        .bind(created_ids[1])
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id IN (?, ?)")
        .bind(asset_id)
        .bind(asset_id_two)
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
    Ok(())
}

#[tokio::test]
async fn admin_lists_wallet_accounts_and_ledger() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (_role_id, admin_id) = create_admin_user(&pool).await;
    let user_email = format!(
        "admin-wallet-filter-{}@example.test",
        Uuid::now_v7().simple()
    );
    let user_id = create_user_with_email(&pool, user_email.clone()).await;
    let other_user_id = create_user(&pool).await;
    let internal_user_email = format!(
        "admin-wallet-robot-{}@internal.local",
        Uuid::now_v7().simple()
    );
    let internal_user_id = create_user_with_email(&pool, internal_user_email.clone()).await;
    let (asset_id, symbol) = create_asset_with_symbol(&pool, "AWL").await;
    let (empty_asset_id, empty_symbol) = create_asset_with_symbol(&pool, "AWE").await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let account_id = sqlx::query(
        r#"INSERT INTO wallet_accounts (user_id, asset_id, available, frozen, locked)
           VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(decimal("100.000000000000000000"))
    .bind(decimal("5.000000000000000000"))
    .bind(decimal("10.000000000000000000"))
    .execute(&pool)
    .await?
    .last_insert_id();
    let other_account_id = sqlx::query(
        r#"INSERT INTO wallet_accounts (user_id, asset_id, available, frozen, locked)
           VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(other_user_id)
    .bind(asset_id)
    .bind(decimal("200.000000000000000000"))
    .bind(decimal("0.000000000000000000"))
    .bind(decimal("0.000000000000000000"))
    .execute(&pool)
    .await?
    .last_insert_id();
    let internal_account_id = sqlx::query(
        r#"INSERT INTO wallet_accounts (user_id, asset_id, available, frozen, locked)
           VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(internal_user_id)
    .bind(asset_id)
    .bind(decimal("300.000000000000000000"))
    .bind(decimal("0.000000000000000000"))
    .bind(decimal("0.000000000000000000"))
    .execute(&pool)
    .await?
    .last_insert_id();
    let ledger_id = sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, 'deposit', ?, 'available', ?, ?, ?, ?, 'manual', ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(decimal("100.000000000000000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(decimal("100.000000000000000000"))
    .bind(decimal("5.000000000000000000"))
    .bind(decimal("10.000000000000000000"))
    .bind(format!("admin-wallet-ledger-{user_id}-{asset_id}"))
    .execute(&pool)
    .await?
    .last_insert_id();
    let internal_ledger_id = sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, 'deposit', ?, 'available', ?, ?, ?, ?, 'manual', ?)"#,
    )
    .bind(internal_user_id)
    .bind(asset_id)
    .bind(decimal("300.000000000000000000"))
    .bind(decimal("300.000000000000000000"))
    .bind(decimal("300.000000000000000000"))
    .bind(decimal("0.000000000000000000"))
    .bind(decimal("0.000000000000000000"))
    .bind(format!("admin-wallet-ledger-{internal_user_id}-{asset_id}"))
    .execute(&pool)
    .await?
    .last_insert_id();
    let other_ledger_id = sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, 'deposit', ?, 'available', ?, ?, ?, ?, 'manual', ?)"#,
    )
    .bind(other_user_id)
    .bind(asset_id)
    .bind(decimal("200.000000000000000000"))
    .bind(decimal("200.000000000000000000"))
    .bind(decimal("200.000000000000000000"))
    .bind(decimal("0.000000000000000000"))
    .bind(decimal("0.000000000000000000"))
    .bind(format!("admin-wallet-ledger-{other_user_id}-{asset_id}"))
    .execute(&pool)
    .await?
    .last_insert_id();

    let accounts = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/wallet/accounts?email={user_email}&asset_id={asset_id}&limit=10"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let accounts_status = accounts.status();
    let accounts_payload = body_json(accounts).await?;
    assert_eq!(
        accounts_status,
        StatusCode::OK,
        "payload: {accounts_payload}"
    );
    let accounts = accounts_payload["accounts"].as_array().unwrap();
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0]["id"], account_id);
    assert_eq!(accounts[0]["user_email"], user_email);
    assert_eq!(accounts[0]["asset_symbol"], symbol);
    assert_eq!(accounts[0]["available"], "100.000000000000000000");
    assert_eq!(accounts[0]["account_exists"], true);

    let default_accounts = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/wallet/accounts?asset_id={asset_id}&limit=20"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let default_accounts_status = default_accounts.status();
    let default_accounts_payload = body_json(default_accounts).await?;
    assert_eq!(
        default_accounts_status,
        StatusCode::OK,
        "payload: {default_accounts_payload}"
    );
    let default_accounts = default_accounts_payload["accounts"].as_array().unwrap();
    assert!(
        default_accounts
            .iter()
            .all(|account| account["user_email"] != internal_user_email)
    );

    let internal_accounts = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/wallet/accounts?asset_id={asset_id}&include_internal=true&limit=20"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let internal_accounts_status = internal_accounts.status();
    let internal_accounts_payload = body_json(internal_accounts).await?;
    assert_eq!(
        internal_accounts_status,
        StatusCode::OK,
        "payload: {internal_accounts_payload}"
    );
    let internal_accounts = internal_accounts_payload["accounts"].as_array().unwrap();
    assert!(internal_accounts.iter().any(|account| {
        account["id"] == internal_account_id
            && account["user_email"] == internal_user_email
            && account["asset_symbol"] == symbol
    }));

    let default_internal_users = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/users?email={internal_user_email}&limit=10"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let default_internal_users_status = default_internal_users.status();
    let default_internal_users_payload = body_json(default_internal_users).await?;
    assert_eq!(
        default_internal_users_status,
        StatusCode::OK,
        "payload: {default_internal_users_payload}"
    );
    assert_eq!(
        default_internal_users_payload["users"]
            .as_array()
            .unwrap()
            .len(),
        0
    );

    let included_internal_users = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/users?email={internal_user_email}&include_internal=true&limit=10"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let included_internal_users_status = included_internal_users.status();
    let included_internal_users_payload = body_json(included_internal_users).await?;
    assert_eq!(
        included_internal_users_status,
        StatusCode::OK,
        "payload: {included_internal_users_payload}"
    );
    let included_internal_users = included_internal_users_payload["users"].as_array().unwrap();
    assert_eq!(included_internal_users.len(), 1);
    assert_eq!(included_internal_users[0]["id"], internal_user_id);
    assert_eq!(included_internal_users[0]["email"], internal_user_email);

    let include_empty_accounts = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/wallet/accounts?email={user_email}&asset_id={empty_asset_id}&include_empty=true&limit=20"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let include_empty_status = include_empty_accounts.status();
    let include_empty_payload = body_json(include_empty_accounts).await?;
    assert_eq!(
        include_empty_status,
        StatusCode::OK,
        "payload: {include_empty_payload}"
    );
    let include_empty_accounts = include_empty_payload["accounts"].as_array().unwrap();
    let empty_account = include_empty_accounts
        .iter()
        .find(|account| account["asset_id"] == empty_asset_id)
        .unwrap();
    assert_eq!(empty_account["id"], Value::Null);
    assert_eq!(empty_account["user_id"], user_id);
    assert_eq!(empty_account["user_email"], user_email);
    assert_eq!(empty_account["asset_symbol"], empty_symbol);
    assert_eq!(empty_account["available"], "0.000000000000000000");
    assert_eq!(empty_account["frozen"], "0.000000000000000000");
    assert_eq!(empty_account["locked"], "0.000000000000000000");
    assert_eq!(empty_account["account_exists"], false);

    let mismatched_include_empty_accounts = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/wallet/accounts?user_id={user_id}&email=missing-{user_email}&asset_id={empty_asset_id}&include_empty=true&limit=20"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let mismatched_include_empty_status = mismatched_include_empty_accounts.status();
    let mismatched_include_empty_payload = body_json(mismatched_include_empty_accounts).await?;
    assert_eq!(
        mismatched_include_empty_status,
        StatusCode::OK,
        "payload: {mismatched_include_empty_payload}"
    );
    assert_eq!(
        mismatched_include_empty_payload["accounts"]
            .as_array()
            .unwrap()
            .len(),
        0
    );

    let hidden_internal_empty_accounts = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/wallet/accounts?user_id={internal_user_id}&asset_id={empty_asset_id}&include_empty=true&limit=20"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let hidden_internal_empty_status = hidden_internal_empty_accounts.status();
    let hidden_internal_empty_payload = body_json(hidden_internal_empty_accounts).await?;
    assert_eq!(
        hidden_internal_empty_status,
        StatusCode::OK,
        "payload: {hidden_internal_empty_payload}"
    );
    assert_eq!(
        hidden_internal_empty_payload["accounts"]
            .as_array()
            .unwrap()
            .len(),
        0
    );

    let included_internal_empty_accounts = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/wallet/accounts?user_id={internal_user_id}&asset_id={empty_asset_id}&include_empty=true&include_internal=true&limit=20"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let included_internal_empty_status = included_internal_empty_accounts.status();
    let included_internal_empty_payload = body_json(included_internal_empty_accounts).await?;
    assert_eq!(
        included_internal_empty_status,
        StatusCode::OK,
        "payload: {included_internal_empty_payload}"
    );
    let included_internal_empty_accounts = included_internal_empty_payload["accounts"]
        .as_array()
        .unwrap();
    let internal_empty_account = included_internal_empty_accounts
        .iter()
        .find(|account| account["asset_id"] == empty_asset_id)
        .unwrap();
    assert_eq!(internal_empty_account["id"], Value::Null);
    assert_eq!(internal_empty_account["user_id"], internal_user_id);
    assert_eq!(internal_empty_account["user_email"], internal_user_email);

    let account_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(empty_asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(account_count, 0);

    let ledger = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/wallet/ledger?email={user_email}&asset_id={asset_id}&change_type=deposit&ref_type=manual&limit=10"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let ledger_status = ledger.status();
    let ledger_payload = body_json(ledger).await?;
    assert_eq!(ledger_status, StatusCode::OK, "payload: {ledger_payload}");
    let ledger = ledger_payload["ledger"].as_array().unwrap();
    assert_eq!(ledger.len(), 1);
    assert_eq!(ledger[0]["id"], ledger_id);
    assert_eq!(ledger[0]["user_email"], user_email);
    assert_eq!(ledger[0]["asset_symbol"], symbol);
    assert_eq!(ledger[0]["balance_type"], "available");
    assert!(ledger[0]["created_at"].is_number());

    let default_ledger = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/wallet/ledger?asset_id={asset_id}&limit=20"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let default_ledger_status = default_ledger.status();
    let default_ledger_payload = body_json(default_ledger).await?;
    assert_eq!(
        default_ledger_status,
        StatusCode::OK,
        "payload: {default_ledger_payload}"
    );
    let default_ledger = default_ledger_payload["ledger"].as_array().unwrap();
    assert!(
        default_ledger
            .iter()
            .all(|entry| entry["user_email"] != internal_user_email)
    );

    let internal_ledger = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/wallet/ledger?asset_id={asset_id}&include_internal=true&limit=20"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let internal_ledger_status = internal_ledger.status();
    let internal_ledger_payload = body_json(internal_ledger).await?;
    assert_eq!(
        internal_ledger_status,
        StatusCode::OK,
        "payload: {internal_ledger_payload}"
    );
    let internal_ledger = internal_ledger_payload["ledger"].as_array().unwrap();
    assert!(internal_ledger.iter().any(|entry| {
        entry["id"] == internal_ledger_id
            && entry["user_email"] == internal_user_email
            && entry["asset_symbol"] == symbol
    }));

    sqlx::query("DELETE FROM wallet_ledger WHERE id IN (?, ?, ?)")
        .bind(ledger_id)
        .bind(other_ledger_id)
        .bind(internal_ledger_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE id IN (?, ?, ?)")
        .bind(account_id)
        .bind(other_account_id)
        .bind(internal_account_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id IN (?, ?)")
        .bind(asset_id)
        .bind(empty_asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id IN (?, ?, ?)")
        .bind(user_id)
        .bind(other_user_id)
        .bind(internal_user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn admin_manages_risk_rules_and_lists_events() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (_role_id, admin_id) = create_admin_user(&pool).await;
    let user_email = format!("admin-risk-filter-{}@example.test", Uuid::now_v7().simple());
    let user_id = create_user_with_email(&pool, user_email.clone()).await;
    let other_user_id = create_user(&pool).await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let created = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/risk/rules")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "rule_type": "withdraw_limit",
                        "target_type": "user",
                        "target_id": user_id.to_string(),
                        "config_json": { "daily_limit": "1000.000000000000000000" },
                        "enabled": true,
                        "reason": "create risk rule"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let created_status = created.status();
    let created_payload = body_json(created).await?;
    assert_eq!(created_status, StatusCode::OK, "payload: {created_payload}");
    let rule_id = created_payload["id"].as_u64().unwrap();
    assert_eq!(created_payload["rule_type"], "withdraw_limit");
    assert_eq!(created_payload["target_type"], "user");
    assert_eq!(created_payload["target_id"], user_id.to_string());
    assert_eq!(created_payload["enabled"], true);
    assert!(created_payload["created_at"].is_number());

    let listed = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/risk/rules?rule_type=withdraw_limit&target_type=user&enabled=true&limit=10")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let listed_status = listed.status();
    let listed_payload = body_json(listed).await?;
    assert_eq!(listed_status, StatusCode::OK, "payload: {listed_payload}");
    let rules = listed_payload["rules"].as_array().unwrap();
    assert!(rules.iter().any(|rule| rule["id"] == rule_id));

    let updated = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/risk/rules/{rule_id}/status"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "enabled": false, "reason": "pause risk rule" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let updated_status = updated.status();
    let updated_payload = body_json(updated).await?;
    assert_eq!(updated_status, StatusCode::OK, "payload: {updated_payload}");
    assert_eq!(updated_payload["id"], rule_id);
    assert_eq!(updated_payload["enabled"], false);

    let event_id = sqlx::query(
        r#"INSERT INTO risk_events
           (user_id, actor_type, actor_id, event_type, risk_level, decision, reason, payload_json)
           VALUES (?, 'user', ?, 'withdraw', 'high', 'review', 'manual review', ?)"#,
    )
    .bind(user_id)
    .bind(user_id)
    .bind(sqlx::types::Json(json!({ "rule_id": rule_id })))
    .execute(&pool)
    .await?
    .last_insert_id();
    let other_event_id = sqlx::query(
        r#"INSERT INTO risk_events
           (user_id, actor_type, actor_id, event_type, risk_level, decision, reason, payload_json)
           VALUES (?, 'user', ?, 'withdraw', 'high', 'review', 'manual review', ?)"#,
    )
    .bind(other_user_id)
    .bind(other_user_id)
    .bind(sqlx::types::Json(json!({ "rule_id": rule_id })))
    .execute(&pool)
    .await?
    .last_insert_id();

    let events = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/risk/events?email={user_email}&decision=review&risk_level=high&limit=10"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let events_status = events.status();
    let events_payload = body_json(events).await?;
    assert_eq!(events_status, StatusCode::OK, "payload: {events_payload}");
    let events = events_payload["events"].as_array().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["id"], event_id);
    assert_eq!(events[0]["risk_level"], "high");
    assert_eq!(events[0]["decision"], "review");
    assert!(events[0]["created_at"].is_number());
    assert!(!events.iter().any(|event| event["id"] == other_event_id));

    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM admin_audit_logs WHERE admin_id = ? AND target_type = 'risk_rule' AND target_id = ?",
    )
    .bind(admin_id)
    .bind(rule_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(audit_count, 2);

    sqlx::query("DELETE FROM risk_events WHERE id IN (?, ?)")
        .bind(event_id)
        .bind(other_event_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ? AND target_type = 'risk_rule'")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM risk_rules WHERE id = ?")
        .bind(rule_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id IN (?, ?)")
        .bind(user_id)
        .bind(other_user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn admin_asset_routes_require_admin_scope_mysql_and_validation() -> Result<(), Box<dyn Error>>
{
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));
    let body = json!({
        "symbol": "btc",
        "name": "Bitcoin",
        "precision_scale": 8,
        "asset_type": "coin",
        "status": "active"
    })
    .to_string();

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/assets")
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let user = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/assets")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(user.status(), StatusCode::FORBIDDEN);

    let invalid = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/assets")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "symbol": "BTC",
                        "name": "Bitcoin",
                        "precision_scale": -1,
                        "asset_type": "coin",
                        "status": "active"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let invalid_payload = body_json(invalid).await?;
    assert_eq!(invalid_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        invalid_payload["message"],
        "validation error: asset precision_scale must be between 0 and 18"
    );

    let admin = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/assets")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await?;
    assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let payload = body_json(admin).await?;
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for admin convert routes")
    );

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/assets?limit=1")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(list.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let detail_missing = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/assets/1")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(detail_missing.status(), StatusCode::UNAUTHORIZED);

    let detail_user = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/assets/1")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(detail_user.status(), StatusCode::FORBIDDEN);

    let detail_admin = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/assets/1")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(detail_admin.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let update_body = json!({
        "name": "Bitcoin Updated",
        "precision_scale": 6,
        "asset_type": "stablecoin",
        "status": "disabled",
        "reason": "update asset"
    })
    .to_string();

    let update_missing = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/assets/1")
                .header("content-type", "application/json")
                .body(Body::from(update_body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(update_missing.status(), StatusCode::UNAUTHORIZED);

    let update_user = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/assets/1")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(update_body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(update_user.status(), StatusCode::FORBIDDEN);

    let invalid_update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/assets/1")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Bitcoin Updated",
                        "precision_scale": -1,
                        "asset_type": "coin",
                        "status": "active",
                        "reason": "invalid asset"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid_update.status(), StatusCode::BAD_REQUEST);

    let blank_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/assets/1")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Bitcoin Updated",
                        "precision_scale": 6,
                        "asset_type": "coin",
                        "status": "active",
                        "reason": " "
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(blank_reason.status(), StatusCode::BAD_REQUEST);

    let unknown_field = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/assets/1")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "symbol": "ETH",
                        "name": "Bitcoin Updated",
                        "precision_scale": 6,
                        "asset_type": "coin",
                        "status": "active",
                        "reason": "unknown field"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(unknown_field.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let update_admin = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/assets/1")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(update_body))
                .unwrap(),
        )
        .await?;
    assert_eq!(update_admin.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let delete_missing = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/admin/api/v1/assets/1")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(delete_missing.status(), StatusCode::UNAUTHORIZED);

    let delete_user = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/admin/api/v1/assets/1")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(delete_user.status(), StatusCode::FORBIDDEN);

    let blank_delete_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/admin/api/v1/assets/1")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "reason": "   " }).to_string()))
                .unwrap(),
        )
        .await?;
    assert_eq!(blank_delete_reason.status(), StatusCode::BAD_REQUEST);
    let blank_delete_payload = body_json(blank_delete_reason).await?;
    assert_eq!(
        blank_delete_payload["message"],
        "validation error: reason is required"
    );

    let delete_admin = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/admin/api/v1/assets/1")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "reason": "delete asset" }).to_string()))
                .unwrap(),
        )
        .await?;
    assert_eq!(delete_admin.status(), StatusCode::INTERNAL_SERVER_ERROR);

    Ok(())
}

#[tokio::test]
async fn admin_asset_create_list_and_audit() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let user_email = format!(
        "admin-asset-wallet-{}@example.test",
        Uuid::now_v7().simple()
    );
    let user_id = create_user_with_email(&pool, user_email).await;
    let other_user_id = create_user(&pool).await;
    let user_token =
        issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let symbol = format!("AST{}", &Uuid::now_v7().simple().to_string()[..10]).to_ascii_uppercase();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/assets")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "symbol": symbol.to_ascii_lowercase(),
                        "name": "Asset Test Coin",
                        "precision_scale": 8,
                        "asset_type": "coin",
                        "status": "active",
                        "deposit_enabled": false,
                        "withdraw_enabled": false,
                        "margin_transfer_enabled": true,
                        "min_deposit_amount": "1.500000000000000000",
                        "deposit_fee": "0.010000000000000000",
                        "withdraw_fee": "0.250000000000000000",
                        "withdraw_fee_tiers": [
                            { "min_amount": "1", "max_amount": "100", "fee_rate_percent": "1" }
                        ],
                        "reason": "create asset"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let create_status = create.status();
    let created = body_json(create).await?;
    assert_eq!(create_status, StatusCode::OK, "payload: {created}");
    let asset_id = created["id"].as_u64().unwrap();
    assert_eq!(created["symbol"], symbol);
    assert_eq!(created["name"], "Asset Test Coin");
    assert_eq!(created["precision_scale"], 8);
    assert_eq!(created["asset_type"], "coin");
    assert_eq!(created["status"], "active");
    assert_eq!(created["deposit_enabled"], false);
    assert_eq!(created["withdraw_enabled"], false);
    assert_eq!(created["margin_transfer_enabled"], true);
    assert_eq!(created["min_deposit_amount"], "1.500000000000000000");
    assert_eq!(created["deposit_fee"], "0.010000000000000000");
    assert_eq!(created["withdraw_fee"], "0.250000000000000000");
    assert_eq!(created["withdraw_fee_tiers"][0]["min_amount"], "1");
    assert_eq!(created["withdraw_fee_tiers"][0]["max_amount"], "100");
    assert_eq!(created["withdraw_fee_tiers"][0]["fee_rate_percent"], "1");
    assert!(created["created_at"].is_number());

    let wallet_accounts = sqlx::query_as::<_, (u64, u64, BigDecimal, BigDecimal, BigDecimal)>(
        r#"SELECT user_id, asset_id, available, frozen, locked
           FROM wallet_accounts
           WHERE asset_id = ? AND user_id IN (?, ?)
           ORDER BY user_id"#,
    )
    .bind(asset_id)
    .bind(user_id)
    .bind(other_user_id)
    .fetch_all(&pool)
    .await?;
    assert_eq!(wallet_accounts.len(), 2);
    for account in wallet_accounts {
        assert_eq!(account.1, asset_id);
        assert_eq!(account.2, decimal("0.000000000000000000"));
        assert_eq!(account.3, decimal("0.000000000000000000"));
        assert_eq!(account.4, decimal("0.000000000000000000"));
    }

    let user_accounts = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/wallet/accounts")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let user_accounts_status = user_accounts.status();
    let user_accounts_payload = body_json(user_accounts).await?;
    assert_eq!(
        user_accounts_status,
        StatusCode::OK,
        "payload: {user_accounts_payload}"
    );
    let user_created_account = user_accounts_payload["accounts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|account| account["asset_id"].as_u64() == Some(asset_id))
        .expect("created asset account should be visible to the user");
    assert_eq!(user_created_account["symbol"], created["symbol"]);
    assert_eq!(
        decimal(user_created_account["available"].as_str().unwrap()),
        decimal("0.000000000000000000")
    );

    let active_delete = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/admin/api/v1/assets/{asset_id}"))
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "reason": "delete active asset" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let active_delete_status = active_delete.status();
    let active_delete_payload = body_json(active_delete).await?;
    assert_eq!(active_delete_status, StatusCode::BAD_REQUEST);
    assert_eq!(
        active_delete_payload["message"],
        "validation error: asset must be disabled before deletion"
    );

    let duplicate = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/assets")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "symbol": symbol,
                        "name": "Duplicate Asset",
                        "precision_scale": 8,
                        "asset_type": "coin",
                        "status": "active"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);

    let listed = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/assets?symbol={}&status=active&asset_type=coin&limit=10",
                    created["symbol"].as_str().unwrap()
                ))
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let listed_status = listed.status();
    let listed_payload = body_json(listed).await?;
    assert_eq!(listed_status, StatusCode::OK, "payload: {listed_payload}");
    let assets = listed_payload["assets"].as_array().unwrap();
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0]["id"], asset_id);
    assert_eq!(assets[0]["deposit_enabled"], false);
    assert_eq!(assets[0]["withdraw_enabled"], false);
    assert_eq!(assets[0]["margin_transfer_enabled"], true);
    assert_eq!(assets[0]["min_deposit_amount"], "1.500000000000000000");
    assert_eq!(assets[0]["deposit_fee"], "0.010000000000000000");
    assert_eq!(assets[0]["withdraw_fee"], "0.250000000000000000");
    assert_eq!(assets[0]["withdraw_fee_tiers"][0]["fee_rate_percent"], "1");

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type = 'asset' AND target_id = ?
           ORDER BY id"#,
    )
    .bind(admin_id)
    .bind(asset_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].action, "asset.create");
    assert!(audits[0].before_json.is_none());
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["symbol"],
        created["symbol"]
    );
    assert_eq!(audits[0].reason.as_deref(), Some("create asset"));

    let detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/api/v1/assets/{asset_id}"))
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let detail_status = detail.status();
    let detail_payload = body_json(detail).await?;
    assert_eq!(detail_status, StatusCode::OK, "payload: {detail_payload}");
    assert_eq!(detail_payload["id"], asset_id);
    assert_eq!(detail_payload["symbol"], created["symbol"]);
    assert_eq!(detail_payload["min_deposit_amount"], "1.500000000000000000");
    assert_eq!(detail_payload["withdraw_fee_tiers"][0]["max_amount"], "100");

    let update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/assets/{asset_id}"))
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Asset Test Coin Updated",
                        "precision_scale": 6,
                        "asset_type": "stablecoin",
                        "status": "disabled",
                        "deposit_enabled": true,
                        "withdraw_enabled": true,
                        "margin_transfer_enabled": false,
                        "min_deposit_amount": "2.000000000000000000",
                        "deposit_fee": "0.020000000000000000",
                        "withdraw_fee": "0.300000000000000000",
                        "withdraw_fee_tiers": [
                            { "min_amount": "2", "max_amount": null, "fee_rate_percent": "2" }
                        ],
                        "reason": "update asset config"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let update_status = update.status();
    let updated = body_json(update).await?;
    assert_eq!(update_status, StatusCode::OK, "payload: {updated}");
    assert_eq!(updated["id"], asset_id);
    assert_eq!(updated["symbol"], created["symbol"]);
    assert_eq!(updated["name"], "Asset Test Coin Updated");
    assert_eq!(updated["precision_scale"], 6);
    assert_eq!(updated["asset_type"], "stablecoin");
    assert_eq!(updated["status"], "disabled");
    assert_eq!(updated["deposit_enabled"], true);
    assert_eq!(updated["withdraw_enabled"], true);
    assert_eq!(updated["margin_transfer_enabled"], false);
    assert_eq!(updated["min_deposit_amount"], "2.000000000000000000");
    assert_eq!(updated["deposit_fee"], "0.020000000000000000");
    assert_eq!(updated["withdraw_fee"], "0.300000000000000000");
    assert_eq!(updated["withdraw_fee_tiers"][0]["min_amount"], "2");
    assert!(updated["withdraw_fee_tiers"][0]["max_amount"].is_null());
    assert_eq!(updated["withdraw_fee_tiers"][0]["fee_rate_percent"], "2");

    let persisted = sqlx::query_as::<
        _,
        (
            String,
            String,
            i32,
            String,
            String,
            bool,
            bool,
            bool,
            BigDecimal,
            BigDecimal,
            BigDecimal,
        ),
    >(
        r#"SELECT symbol,
                      name,
                      precision_scale,
                      asset_type,
                      status,
                      deposit_enabled,
                      withdraw_enabled,
                      margin_transfer_enabled,
                      min_deposit_amount,
                      deposit_fee,
                      withdraw_fee
               FROM assets
               WHERE id = ?"#,
    )
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(persisted.0, created["symbol"].as_str().unwrap());
    assert_eq!(persisted.1, "Asset Test Coin Updated");
    assert_eq!(persisted.2, 6);
    assert_eq!(persisted.3, "stablecoin");
    assert_eq!(persisted.4, "disabled");
    assert!(persisted.5);
    assert!(persisted.6);
    assert!(!persisted.7);
    assert_eq!(persisted.8, decimal("2.000000000000000000"));
    assert_eq!(persisted.9, decimal("0.020000000000000000"));
    assert_eq!(persisted.10, decimal("0.300000000000000000"));

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type = 'asset' AND target_id = ?
           ORDER BY id"#,
    )
    .bind(admin_id)
    .bind(asset_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 2);
    assert_eq!(audits[1].action, "asset.config.update");
    assert_eq!(
        audits[1].before_json.as_ref().unwrap()["name"],
        "Asset Test Coin"
    );
    assert_eq!(
        audits[1].after_json.as_ref().unwrap()["name"],
        "Asset Test Coin Updated"
    );
    assert_eq!(
        audits[1].after_json.as_ref().unwrap()["asset_type"],
        "stablecoin"
    );
    assert_eq!(audits[1].after_json.as_ref().unwrap()["status"], "disabled");
    assert_eq!(
        audits[1].before_json.as_ref().unwrap()["deposit_enabled"],
        false
    );
    assert_eq!(
        audits[1].after_json.as_ref().unwrap()["deposit_enabled"],
        true
    );
    assert_eq!(
        audits[1].before_json.as_ref().unwrap()["withdraw_enabled"],
        false
    );
    assert_eq!(
        audits[1].after_json.as_ref().unwrap()["withdraw_enabled"],
        true
    );
    assert_eq!(
        audits[1].before_json.as_ref().unwrap()["margin_transfer_enabled"],
        true
    );
    assert_eq!(
        audits[1].after_json.as_ref().unwrap()["margin_transfer_enabled"],
        false
    );
    assert_eq!(
        audits[1].before_json.as_ref().unwrap()["min_deposit_amount"],
        "1.500000000000000000"
    );
    assert_eq!(
        audits[1].after_json.as_ref().unwrap()["min_deposit_amount"],
        "2.000000000000000000"
    );
    assert_eq!(
        audits[1].after_json.as_ref().unwrap()["deposit_fee"],
        "0.020000000000000000"
    );
    assert_eq!(
        audits[1].after_json.as_ref().unwrap()["withdraw_fee"],
        "0.300000000000000000"
    );
    assert_eq!(
        audits[1].after_json.as_ref().unwrap()["withdraw_fee_tiers"][0]["fee_rate_percent"],
        "2"
    );
    assert_eq!(audits[1].reason.as_deref(), Some("update asset config"));

    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(decimal("1.000000000000000000"))
        .bind(user_id)
        .bind(asset_id)
        .execute(&pool)
        .await?;
    let referenced_delete = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/admin/api/v1/assets/{asset_id}"))
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "reason": "delete referenced asset" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let referenced_delete_status = referenced_delete.status();
    let referenced_delete_payload = body_json(referenced_delete).await?;
    assert_eq!(referenced_delete_status, StatusCode::BAD_REQUEST);
    assert_eq!(
        referenced_delete_payload["message"],
        "validation error: asset with related records cannot be deleted"
    );
    sqlx::query("UPDATE wallet_accounts SET available = 0 WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(&pool)
        .await?;

    let delete = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/admin/api/v1/assets/{asset_id}"))
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "reason": "delete disabled asset" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(delete.status(), StatusCode::NO_CONTENT);

    let (asset_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM assets WHERE id = ?")
        .bind(asset_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(asset_count, 0);
    let (wallet_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM wallet_accounts WHERE asset_id = ?")
            .bind(asset_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(wallet_count, 0);

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type = 'asset' AND target_id = ?
           ORDER BY id"#,
    )
    .bind(admin_id)
    .bind(asset_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 3);
    assert_eq!(audits[2].action, "asset.delete");
    assert_eq!(audits[2].target_type, "asset");
    assert_eq!(audits[2].target_id, asset_id.to_string());
    assert_eq!(
        audits[2].before_json.as_ref().unwrap()["status"],
        "disabled"
    );
    assert!(audits[2].after_json.is_none());
    assert_eq!(audits[2].reason.as_deref(), Some("delete disabled asset"));

    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ? AND target_type = 'asset'")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id IN (?, ?)")
        .bind(user_id)
        .bind(other_user_id)
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
    Ok(())
}

#[tokio::test]
async fn admin_trading_pair_routes_require_admin_scope_mysql_and_validation()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));
    let body = json!({
        "base_asset_id": 1,
        "quote_asset_id": 2,
        "symbol": "btc-usdt",
        "price_precision": 8,
        "qty_precision": 6,
        "min_order_value": "10.000000000000000000",
        "status": "active",
        "market_type": "external"
    })
    .to_string();

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/market-pairs")
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let user = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/market-pairs")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(user.status(), StatusCode::FORBIDDEN);

    let invalid = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/market-pairs")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "base_asset_id": 1,
                        "quote_asset_id": 1,
                        "symbol": "BTC-USDT",
                        "price_precision": 8,
                        "qty_precision": 6,
                        "min_order_value": "10.000000000000000000",
                        "status": "active",
                        "market_type": "external"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let invalid_payload = body_json(invalid).await?;
    assert_eq!(invalid_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        invalid_payload["message"],
        "validation error: trading pair assets must be different"
    );

    let admin = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/market-pairs")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await?;
    assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let payload = body_json(admin).await?;
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for admin convert routes")
    );

    let list = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/market-pairs?limit=1")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(list.status(), StatusCode::INTERNAL_SERVER_ERROR);

    Ok(())
}

#[tokio::test]
async fn admin_trading_pair_detail_and_status_routes_require_admin_scope_mysql()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));
    let status_body = json!({
        "status": "active",
        "reason": "enable pair"
    })
    .to_string();

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/market-pairs/1")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let user = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/market-pairs/1")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(user.status(), StatusCode::FORBIDDEN);

    let admin = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/market-pairs/1")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let payload = body_json(admin).await?;
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for admin convert routes")
    );

    let patch_missing = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/market-pairs/1/status")
                .header("content-type", "application/json")
                .body(Body::from(status_body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(patch_missing.status(), StatusCode::UNAUTHORIZED);

    let patch_user = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/market-pairs/1/status")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(status_body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(patch_user.status(), StatusCode::FORBIDDEN);

    let invalid = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/market-pairs/1/status")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "status": "archived",
                        "reason": "invalid status"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);

    let missing_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/market-pairs/1/status")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "status": "active", "reason": " " }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing_reason.status(), StatusCode::BAD_REQUEST);

    let patch_admin = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/market-pairs/1/status")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(status_body))
                .unwrap(),
        )
        .await?;
    assert_eq!(patch_admin.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let update_body = json!({
        "price_precision": 8,
        "qty_precision": 6,
        "min_order_value": "10.000000000000000000",
        "status": "active",
        "market_type": "external",
        "reason": "update pair config"
    })
    .to_string();

    let update_missing = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/market-pairs/1")
                .header("content-type", "application/json")
                .body(Body::from(update_body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(update_missing.status(), StatusCode::UNAUTHORIZED);

    let update_user = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/market-pairs/1")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(update_body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(update_user.status(), StatusCode::FORBIDDEN);

    let invalid_market_type = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/market-pairs/1")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "price_precision": 8,
                        "qty_precision": 6,
                        "min_order_value": "10.000000000000000000",
                        "status": "active",
                        "market_type": "archive",
                        "reason": "invalid market type"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid_market_type.status(), StatusCode::BAD_REQUEST);

    let blank_update_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/market-pairs/1")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "price_precision": 8,
                        "qty_precision": 6,
                        "min_order_value": "10.000000000000000000",
                        "status": "active",
                        "market_type": "external",
                        "reason": " "
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(blank_update_reason.status(), StatusCode::BAD_REQUEST);

    let unknown_update_field = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/market-pairs/1")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "price_precision": 8,
                        "qty_precision": 6,
                        "min_order_value": "10.000000000000000000",
                        "market_type": "external",
                        "status": "active",
                        "base_asset_id": 999,
                        "reason": "unknown field"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(
        unknown_update_field.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let config_update_admin = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/market-pairs/1")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(update_body))
                .unwrap(),
        )
        .await?;
    assert_eq!(
        config_update_admin.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );

    Ok(())
}

#[tokio::test]
async fn admin_trading_pair_create_detail_status_update_and_audit() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let (base_asset, base_symbol) = create_asset_with_symbol(&pool, "TPDB").await;
    let (quote_asset, quote_symbol) = create_asset_with_symbol(&pool, "TPDQ").await;
    let symbol = format!("{base_symbol}-{quote_symbol}");
    let pair_id = sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, 8, 6, '10.000000000000000000', 'disabled', 'external')"#,
    )
    .bind(base_asset)
    .bind(quote_asset)
    .bind(&symbol)
    .execute(&pool)
    .await?
    .last_insert_id();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/api/v1/market-pairs/{pair_id}"))
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let detail_status = detail.status();
    let detail_payload = body_json(detail).await?;
    assert_eq!(detail_status, StatusCode::OK, "payload: {detail_payload}");
    assert_eq!(detail_payload["id"], pair_id);
    assert_eq!(detail_payload["status"], "disabled");
    assert_eq!(detail_payload["symbol"], symbol);

    let update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/market-pairs/{pair_id}/status"))
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "status": "active",
                        "reason": "enable listed pair"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let update_status = update.status();
    let updated = body_json(update).await?;
    assert_eq!(update_status, StatusCode::OK, "payload: {updated}");
    assert_eq!(updated["id"], pair_id);
    assert_eq!(updated["status"], "active");

    let stored_status: String = sqlx::query_scalar("SELECT status FROM trading_pairs WHERE id = ?")
        .bind(pair_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(stored_status, "active");

    let config_update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/market-pairs/{pair_id}"))
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "price_precision": 10,
                        "qty_precision": 4,
                        "min_order_value": "25.000000000000000000",
                        "status": "disabled",
                        "market_type": "strategy",
                        "reason": "adjust pair config"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let config_update_status = config_update.status();
    let config_updated = body_json(config_update).await?;
    assert_eq!(
        config_update_status,
        StatusCode::OK,
        "payload: {config_updated}"
    );
    assert_eq!(config_updated["id"], pair_id);
    assert_eq!(config_updated["symbol"], symbol);
    assert_eq!(config_updated["base_asset_id"], base_asset);
    assert_eq!(config_updated["quote_asset_id"], quote_asset);
    assert_eq!(config_updated["status"], "disabled");
    assert_eq!(config_updated["price_precision"], 10);
    assert_eq!(config_updated["qty_precision"], 4);
    assert_eq!(config_updated["min_order_value"], "25.000000000000000000");
    assert_eq!(config_updated["market_type"], "strategy");

    let stored_config: (i32, i32, BigDecimal, String, String, u64, u64) = sqlx::query_as(
        r#"SELECT price_precision, qty_precision, min_order_value, market_type, status, base_asset, quote_asset
           FROM trading_pairs WHERE id = ?"#,
    )
    .bind(pair_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(stored_config.0, 10);
    assert_eq!(stored_config.1, 4);
    assert_eq!(stored_config.2, decimal("25.000000000000000000"));
    assert_eq!(stored_config.3, "strategy");
    assert_eq!(stored_config.4, "disabled");
    assert_eq!(stored_config.5, base_asset);
    assert_eq!(stored_config.6, quote_asset);

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type = 'trading_pair' AND target_id = ?
           ORDER BY id"#,
    )
    .bind(admin_id)
    .bind(pair_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 2);
    assert_eq!(audits[0].action, "trading_pair.status.update");
    assert_eq!(
        audits[0].before_json.as_ref().unwrap()["status"],
        "disabled"
    );
    assert_eq!(audits[0].after_json.as_ref().unwrap()["status"], "active");
    assert_eq!(audits[0].reason.as_deref(), Some("enable listed pair"));
    assert_eq!(audits[1].action, "trading_pair.config.update");
    assert_eq!(
        audits[1].before_json.as_ref().unwrap()["price_precision"],
        8
    );
    assert_eq!(
        audits[1].after_json.as_ref().unwrap()["price_precision"],
        10
    );
    assert_eq!(
        audits[1].before_json.as_ref().unwrap()["market_type"],
        "external"
    );
    assert_eq!(
        audits[1].after_json.as_ref().unwrap()["market_type"],
        "strategy"
    );
    assert_eq!(audits[1].after_json.as_ref().unwrap()["status"], "disabled");
    assert_eq!(audits[1].reason.as_deref(), Some("adjust pair config"));

    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ? AND target_type = 'trading_pair'")
        .bind(admin_id)
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
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(role_id)
        .execute(&pool)
        .await?;

    Ok(())
}

#[tokio::test]
async fn admin_trading_pair_create_list_and_audit() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let (base_asset, base_symbol) = create_asset_with_symbol(&pool, "TPB").await;
    let (quote_asset, quote_symbol) = create_asset_with_symbol(&pool, "TPQ").await;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let requested_symbol = format!("{base_symbol}-{quote_symbol}").to_ascii_lowercase();

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/market-pairs")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "base_asset_id": base_asset,
                        "quote_asset_id": quote_asset,
                        "symbol": requested_symbol,
                        "price_precision": 8,
                        "qty_precision": 6,
                        "min_order_value": "10.000000000000000000",
                        "status": "active",
                        "market_type": "external",
                        "reason": "create spot pair"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let create_status = create.status();
    let created = body_json(create).await?;
    assert_eq!(create_status, StatusCode::OK, "payload: {created}");
    let pair_id = created["id"].as_u64().unwrap();
    assert_eq!(created["base_asset_id"], base_asset);
    assert_eq!(created["quote_asset_id"], quote_asset);
    assert_eq!(created["symbol"], format!("{base_symbol}-{quote_symbol}"));
    assert_eq!(created["base_asset"], base_symbol);
    assert_eq!(created["quote_asset"], quote_symbol);
    assert_eq!(created["price_precision"], 8);
    assert_eq!(created["qty_precision"], 6);
    assert_eq!(created["min_order_value"], "10.000000000000000000");
    assert_eq!(created["status"], "active");
    assert_eq!(created["market_type"], "external");
    assert!(created["created_at"].is_number());

    let duplicate = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/market-pairs")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "base_asset_id": base_asset,
                        "quote_asset_id": quote_asset,
                        "symbol": format!("{base_symbol}-{quote_symbol}"),
                        "price_precision": 8,
                        "qty_precision": 6,
                        "min_order_value": "10.000000000000000000",
                        "status": "active",
                        "market_type": "external"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);

    let listed = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/market-pairs?symbol={}&status=active&market_type=external&limit=10",
                    created["symbol"].as_str().unwrap()
                ))
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let listed_status = listed.status();
    let listed_payload = body_json(listed).await?;
    assert_eq!(listed_status, StatusCode::OK, "payload: {listed_payload}");
    let pairs = listed_payload["pairs"].as_array().unwrap();
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0]["id"], pair_id);

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type = 'trading_pair' AND target_id = ?
           ORDER BY id"#,
    )
    .bind(admin_id)
    .bind(pair_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].action, "trading_pair.create");
    assert!(audits[0].before_json.is_none());
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["symbol"],
        created["symbol"]
    );
    assert_eq!(audits[0].reason.as_deref(), Some("create spot pair"));

    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ? AND target_type = 'trading_pair'")
        .bind(admin_id)
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
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(role_id)
        .execute(&pool)
        .await?;

    Ok(())
}

#[tokio::test]
async fn admin_market_strategy_routes_require_admin_scope_mysql_and_validation()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));
    let body = json!({
        "pair_id": 1,
        "strategy_type": "price_path",
        "start_price": "1.000000000000000000",
        "target_price": "2.000000000000000000",
        "start_time": 1770000000000_i64,
        "end_time": 1770003600000_i64,
        "volatility": "0.01000000",
        "volume_min": "10.000000000000000000",
        "volume_max": "20.000000000000000000",
        "status": "active"
    })
    .to_string();

    let presets = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/market-strategies/presets")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let presets_status = presets.status();
    let presets_payload = body_json(presets).await?;
    assert_eq!(presets_status, StatusCode::OK, "payload: {presets_payload}");
    assert_eq!(presets_payload["presets"].as_array().unwrap().len(), 7);
    assert_eq!(presets_payload["presets"][0]["code"], "custom_path");
    assert_eq!(presets_payload["presets"][5]["code"], "crash_recovery");

    let invalid_generator_preview = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/market-strategies/preview")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "pair_id": 1,
                        "strategy_type": "price_path",
                        "start_price": "1",
                        "target_price": "2",
                        "start_time": 1770000000000_i64,
                        "end_time": 1770003600000_i64,
                        "volatility": "0.01",
                        "volume_min": "10",
                        "volume_max": "20",
                        "generator": {
                            "scenario": "trend_up",
                            "seed_mode": "fixed",
                            "seed": "fixed-preview",
                            "mean_reversion_strength": "0.55",
                            "noise_scale": "5.1",
                            "wick_scale": "0.75",
                            "volume_shape": "trend"
                        },
                        "sample_count": 24
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let invalid_generator_status = invalid_generator_preview.status();
    let invalid_generator_payload = body_json(invalid_generator_preview).await?;
    assert_eq!(invalid_generator_status, StatusCode::BAD_REQUEST);
    assert_eq!(
        invalid_generator_payload["message"],
        "validation error: 噪声强度必须在 0～5 之间"
    );

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/market-strategies")
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let user = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/market-strategies")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(user.status(), StatusCode::FORBIDDEN);

    let invalid = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/market-strategies")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "pair_id": 1,
                        "strategy_type": "price_path",
                        "start_price": "1.000000000000000000",
                        "target_price": "2.000000000000000000",
                        "start_time": 1770000000000_i64,
                        "end_time": 1770003600000_i64,
                        "volatility": "0.01000000",
                        "volume_min": "20.000000000000000000",
                        "volume_max": "10.000000000000000000"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let invalid_payload = body_json(invalid).await?;
    assert_eq!(invalid_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        invalid_payload["message"],
        "validation error: volume_max must be greater than or equal to volume_min"
    );

    let admin = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/market-strategies")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await?;
    assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let payload = body_json(admin).await?;
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for admin convert routes")
    );

    let status = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/market-strategies/1/status")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "status": "paused" }).to_string()))
                .unwrap(),
        )
        .await?;
    assert_eq!(status.status(), StatusCode::INTERNAL_SERVER_ERROR);

    Ok(())
}

#[tokio::test]
async fn admin_market_strategy_create_list_update_and_audit() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let base_asset = create_asset(&pool, "MSB").await;
    let quote_asset = create_asset(&pool, "MSQ").await;
    let external_base_asset = create_asset(&pool, "MEB").await;
    let external_quote_asset = create_asset(&pool, "MEQ").await;
    let strategy_symbol = format!("MS{}", Uuid::now_v7().simple()).to_ascii_uppercase();
    let external_symbol = format!("ME{}", Uuid::now_v7().simple()).to_ascii_uppercase();
    let strategy_pair_id = sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, 8, 8, ?, 'active', 'strategy')"#,
    )
    .bind(base_asset)
    .bind(quote_asset)
    .bind(&strategy_symbol)
    .bind(decimal("1.000000000000000000"))
    .execute(&pool)
    .await?
    .last_insert_id();
    let external_pair_id = sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, 8, 8, ?, 'active', 'external')"#,
    )
    .bind(external_base_asset)
    .bind(external_quote_asset)
    .bind(&external_symbol)
    .bind(decimal("1.000000000000000000"))
    .execute(&pool)
    .await?
    .last_insert_id();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let start_time = chrono::Utc.with_ymd_and_hms(2026, 2, 1, 8, 0, 0).unwrap();
    let end_time = chrono::Utc.with_ymd_and_hms(2026, 2, 1, 9, 0, 0).unwrap();

    let user = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/admin/api/v1/market-strategies")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(user.status(), StatusCode::FORBIDDEN);

    let external = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/market-strategies")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "pair_id": external_pair_id,
                        "strategy_type": "price_path",
                        "start_price": "1.000000000000000000",
                        "target_price": "2.000000000000000000",
                        "start_time": start_time.timestamp_millis(),
                        "end_time": end_time.timestamp_millis(),
                        "volatility": "0.01000000",
                        "volume_min": "10.000000000000000000",
                        "volume_max": "20.000000000000000000"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(external.status(), StatusCode::BAD_REQUEST);
    let external_payload = body_json(external).await?;
    assert_eq!(
        external_payload["message"],
        "validation error: market strategy can only be bound to internal or strategy pairs"
    );

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/market-strategies")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "pair_id": strategy_pair_id,
                        "strategy_type": "price_path",
                        "start_price": "1.000000000000000000",
                        "target_price": "2.000000000000000000",
                        "start_time": start_time.timestamp_millis(),
                        "end_time": end_time.timestamp_millis(),
                        "volatility": "0.01000000",
                        "volume_min": "10.000000000000000000",
                        "volume_max": "20.000000000000000000",
                        "status": "active",
                        "reason": "create strategy"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let create_status = create.status();
    let created = body_json(create).await?;
    assert_eq!(create_status, StatusCode::OK, "payload: {created}");
    let strategy_id = created["id"].as_u64().unwrap();
    assert_eq!(created["pair_id"], strategy_pair_id);
    assert_eq!(created["symbol"], strategy_symbol);
    assert_eq!(created["market_type"], "strategy");
    assert_eq!(created["strategy_type"], "price_path");
    assert_eq!(created["status"], "active");
    assert_eq!(created["run_status"], "running");
    assert!(created["start_time"].is_number());
    assert_eq!(created["start_time"], start_time.timestamp_millis());
    assert!(created["end_time"].is_number());
    assert_eq!(created["end_time"], end_time.timestamp_millis());
    assert!(created["created_at"].is_number());
    assert_eq!(created["generator"]["scenario"], "custom_path");
    assert_eq!(created["generator"]["seed_mode"], "auto");
    assert!(
        created["generator"]["seed"]
            .as_str()
            .is_some_and(|seed| !seed.is_empty())
    );

    let listed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/admin/api/v1/market-strategies?pair_id={strategy_pair_id}&status=active&limit=10"
                ))
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let listed_status = listed.status();
    let listed_payload = body_json(listed).await?;
    assert_eq!(listed_status, StatusCode::OK, "payload: {listed_payload}");
    let strategies = listed_payload["strategies"].as_array().unwrap();
    assert_eq!(strategies.len(), 1);
    assert_eq!(strategies[0]["id"], strategy_id);
    assert_eq!(strategies[0]["run_status"], "running");
    assert!(strategies[0]["last_kline_open_time"].is_number());

    let (version, version_admin, seed, config_json): (i32, Option<u64>, String, Value) =
        sqlx::query_as(
            r#"SELECT version, created_by, seed, config_json
               FROM strategy_versions
               WHERE strategy_id = ?"#,
        )
        .bind(strategy_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(version, 1);
    assert_eq!(version_admin, Some(admin_id));
    assert!(!seed.is_empty());
    assert_eq!(config_json["strategy_type"], "price_path");
    assert_eq!(config_json["start_time"], start_time.timestamp_millis());
    assert_eq!(config_json["generator"]["scenario"], "custom_path");
    assert_eq!(config_json["generator"]["mean_reversion_strength"], "0.55");

    let (run_status, current_price, last_kline_open_time): (
        String,
        BigDecimal,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as(
        "SELECT run_status, current_price, last_kline_open_time FROM strategy_runs WHERE strategy_id = ?",
    )
    .bind(strategy_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(run_status, "running");
    assert_eq!(current_price, decimal("1.000000000000000000"));
    assert_eq!(last_kline_open_time, Some(start_time));

    let update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/admin/api/v1/market-strategies/{strategy_id}/status"
                ))
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "status": "paused", "reason": "pause strategy" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let update_status = update.status();
    let updated = body_json(update).await?;
    assert_eq!(update_status, StatusCode::OK, "payload: {updated}");
    assert_eq!(updated["id"], strategy_id);
    assert_eq!(updated["status"], "paused");
    assert_eq!(updated["run_status"], "paused");

    let (stored_status, stored_run_status): (String, String) = sqlx::query_as(
        r#"SELECT strategies.status, runs.run_status
           FROM market_strategies strategies
           INNER JOIN strategy_runs runs ON runs.strategy_id = strategies.id
           WHERE strategies.id = ?"#,
    )
    .bind(strategy_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(stored_status, "paused");
    assert_eq!(stored_run_status, "paused");

    let events: Vec<(String, Value)> = sqlx::query_as(
        r#"SELECT event_type, payload_json
           FROM strategy_events
           WHERE strategy_id = ?
           ORDER BY id"#,
    )
    .bind(strategy_id)
    .fetch_all(&pool)
    .await?;
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].0, "market_strategy.create");
    assert_eq!(events[0].1["after"]["status"], "active");
    assert_eq!(events[1].0, "market_strategy.status.update");
    assert_eq!(events[1].1["before"]["status"], "active");
    assert_eq!(events[1].1["after"]["status"], "paused");

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type = 'market_strategy' AND target_id = ?
           ORDER BY id"#,
    )
    .bind(admin_id)
    .bind(strategy_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 2);
    assert_eq!(audits[0].action, "market_strategy.create");
    assert!(audits[0].before_json.is_none());
    assert_eq!(audits[0].after_json.as_ref().unwrap()["status"], "active");
    assert_eq!(audits[0].reason.as_deref(), Some("create strategy"));
    assert_eq!(audits[1].action, "market_strategy.status.update");
    assert_eq!(audits[1].before_json.as_ref().unwrap()["status"], "active");
    assert_eq!(audits[1].after_json.as_ref().unwrap()["status"], "paused");
    assert_eq!(audits[1].reason.as_deref(), Some("pause strategy"));

    sqlx::query(
        "DELETE FROM admin_audit_logs WHERE admin_id = ? AND target_type = 'market_strategy'",
    )
    .bind(admin_id)
    .execute(&pool)
    .await?;
    sqlx::query("DELETE FROM strategy_events WHERE strategy_id = ?")
        .bind(strategy_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM strategy_runs WHERE strategy_id = ?")
        .bind(strategy_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM strategy_versions WHERE strategy_id = ?")
        .bind(strategy_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM market_strategies WHERE id = ?")
        .bind(strategy_id)
        .execute(&pool)
        .await?;
    for pair_id in [strategy_pair_id, external_pair_id] {
        sqlx::query("DELETE FROM trading_pairs WHERE id = ?")
            .bind(pair_id)
            .execute(&pool)
            .await?;
    }
    for asset_id in [
        base_asset,
        quote_asset,
        external_base_asset,
        external_quote_asset,
    ] {
        sqlx::query("DELETE FROM assets WHERE id = ?")
            .bind(asset_id)
            .execute(&pool)
            .await?;
    }
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(role_id)
        .execute(&pool)
        .await?;

    Ok(())
}

#[tokio::test]
async fn admin_market_strategy_update_config_versions_and_audit() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let base_asset = create_asset(&pool, "MUB").await;
    let quote_asset = create_asset(&pool, "MUQ").await;
    let symbol = format!("MU{}", Uuid::now_v7().simple()).to_ascii_uppercase();
    let pair_id = sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, 8, 8, ?, 'active', 'strategy')"#,
    )
    .bind(base_asset)
    .bind(quote_asset)
    .bind(&symbol)
    .bind(decimal("1.000000000000000000"))
    .execute(&pool)
    .await?
    .last_insert_id();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let start_time = chrono::Utc.with_ymd_and_hms(2026, 4, 1, 8, 0, 0).unwrap();
    let end_time = chrono::Utc.with_ymd_and_hms(2026, 4, 1, 9, 0, 0).unwrap();
    let update_start = chrono::Utc.with_ymd_and_hms(2026, 4, 1, 10, 0, 0).unwrap();
    let update_end = chrono::Utc.with_ymd_and_hms(2026, 4, 1, 11, 0, 0).unwrap();

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/market-strategies")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "pair_id": pair_id,
                        "strategy_type": "price_path",
                        "start_price": "1.000000000000000000",
                        "target_price": "2.000000000000000000",
                        "start_time": start_time.timestamp_millis(),
                        "end_time": end_time.timestamp_millis(),
                        "volatility": "0.01000000",
                        "volume_min": "10.000000000000000000",
                        "volume_max": "20.000000000000000000",
                        "generator": {
                            "scenario": "range",
                            "seed_mode": "fixed",
                            "seed": "route-fixed-seed-v1",
                            "mean_reversion_strength": "0.90",
                            "noise_scale": "1.15",
                            "wick_scale": "0.95",
                            "volume_shape": "uniform"
                        },
                        "status": "active",
                        "reason": "create before update"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let create_status = create.status();
    let created = body_json(create).await?;
    assert_eq!(create_status, StatusCode::OK, "payload: {created}");
    let strategy_id = created["id"].as_u64().unwrap();
    assert_eq!(created["generator"]["scenario"], "range");
    assert_eq!(created["generator"]["seed_mode"], "fixed");
    assert_eq!(created["generator"]["seed"], "route-fixed-seed-v1");

    let preview_body = json!({
        "pair_id": pair_id,
        "strategy_type": "price_path",
        "start_price": "1.000000000000000000",
        "target_price": "2.000000000000000000",
        "start_time": start_time.timestamp_millis(),
        "end_time": end_time.timestamp_millis(),
        "volatility": "0.01000000",
        "volume_min": "10.000000000000000000",
        "volume_max": "20.000000000000000000",
        "generator": {
            "scenario": "range",
            "seed_mode": "fixed",
            "seed": "route-fixed-seed-v1",
            "mean_reversion_strength": "0.90",
            "noise_scale": "1.15",
            "wick_scale": "0.95",
            "volume_shape": "uniform"
        },
        "sample_count": 6
    });
    let mut replayed_samples = None;
    for _ in 0..2 {
        let preview = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/v1/market-strategies/preview")
                    .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(preview_body.to_string()))
                    .unwrap(),
            )
            .await?;
        let preview_status = preview.status();
        let preview_payload = body_json(preview).await?;
        assert_eq!(preview_status, StatusCode::OK, "payload: {preview_payload}");
        assert_eq!(preview_payload["preview_seed"], "route-fixed-seed-v1");
        assert_eq!(preview_payload["preview_version"], 1);
        assert_eq!(preview_payload["one_minute_count"], 60);
        assert_eq!(preview_payload["sample_count"], 6);
        if let Some(previous) = replayed_samples.as_ref() {
            assert_eq!(&preview_payload["samples"], previous);
        } else {
            replayed_samples = Some(preview_payload["samples"].clone());
        }
    }
    let version_count_after_preview: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM strategy_versions WHERE strategy_id = ?")
            .bind(strategy_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(version_count_after_preview, 1);

    let active_update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/market-strategies/{strategy_id}"))
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "strategy_type": "price_path",
                        "start_price": "1.100000000000000000",
                        "target_price": "2.200000000000000000",
                        "start_time": update_start.timestamp_millis(),
                        "end_time": update_end.timestamp_millis(),
                        "volatility": "0.02000000",
                        "volume_min": "12.000000000000000000",
                        "volume_max": "24.000000000000000000",
                        "reason": "try update active"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let active_update_status = active_update.status();
    assert_eq!(active_update_status, StatusCode::CONFLICT);
    let active_update_payload = body_json(active_update).await?;
    assert_eq!(active_update_payload["code"], "CONFLICT");
    assert_eq!(
        active_update_payload["message"],
        "conflict: active market strategy must be paused or disabled before update"
    );

    let pause = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/admin/api/v1/market-strategies/{strategy_id}/status"
                ))
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "status": "paused", "reason": "pause before config update" })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(pause.status(), StatusCode::OK);

    let edit_preview = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/market-strategies/preview")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "strategy_id": strategy_id,
                        "pair_id": pair_id,
                        "strategy_type": "price_path_v2",
                        "start_price": "1.100000000000000000",
                        "target_price": "2.200000000000000000",
                        "start_time": update_start.timestamp_millis(),
                        "end_time": update_end.timestamp_millis(),
                        "volatility": "0.02000000",
                        "volume_min": "12.000000000000000000",
                        "volume_max": "24.000000000000000000",
                        "generator": {
                            "scenario": "trend_up",
                            "seed_mode": "auto",
                            "seed": null,
                            "regenerate_seed": false,
                            "mean_reversion_strength": "0.45",
                            "noise_scale": "0.80",
                            "wick_scale": "0.60",
                            "volume_shape": "trend"
                        },
                        "sample_count": 4
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let edit_preview_status = edit_preview.status();
    let edit_preview_payload = body_json(edit_preview).await?;
    assert_eq!(
        edit_preview_status,
        StatusCode::OK,
        "payload: {edit_preview_payload}"
    );
    assert_eq!(edit_preview_payload["preview_version"], 2);
    assert_eq!(edit_preview_payload["preview_seed"], "route-fixed-seed-v1");
    assert_eq!(edit_preview_payload["sample_count"], 4);
    let version_count_after_edit_preview: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM strategy_versions WHERE strategy_id = ?")
            .bind(strategy_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(version_count_after_edit_preview, 1);

    let missing_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/market-strategies/{strategy_id}"))
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "strategy_type": "price_path",
                        "start_price": "1.100000000000000000",
                        "target_price": "2.200000000000000000",
                        "start_time": update_start.timestamp_millis(),
                        "end_time": update_end.timestamp_millis(),
                        "volatility": "0.02000000",
                        "volume_min": "12.000000000000000000",
                        "volume_max": "24.000000000000000000"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let missing_reason_status = missing_reason.status();
    let missing_reason_payload = body_json(missing_reason).await?;
    assert_eq!(
        missing_reason_status,
        StatusCode::BAD_REQUEST,
        "payload: {missing_reason_payload}"
    );
    assert_eq!(missing_reason_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        missing_reason_payload["message"],
        "validation error: reason is required"
    );

    let update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/market-strategies/{strategy_id}"))
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "strategy_type": "price_path_v2",
                        "start_price": "1.100000000000000000",
                        "target_price": "2.200000000000000000",
                        "start_time": update_start.timestamp_millis(),
                        "end_time": update_end.timestamp_millis(),
                        "volatility": "0.02000000",
                        "volume_min": "12.000000000000000000",
                        "volume_max": "24.000000000000000000",
                        "generator": {
                            "scenario": "trend_up",
                            "seed_mode": "auto",
                            "seed": null,
                            "regenerate_seed": false,
                            "mean_reversion_strength": "0.45",
                            "noise_scale": "0.80",
                            "wick_scale": "0.60",
                            "volume_shape": "trend"
                        },
                        "reason": "update config"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let update_status = update.status();
    let updated = body_json(update).await?;
    assert_eq!(update_status, StatusCode::OK, "payload: {updated}");
    assert_eq!(updated["id"], strategy_id);
    assert_eq!(updated["pair_id"], pair_id);
    assert_eq!(updated["status"], "paused");
    assert_eq!(updated["run_status"], "paused");
    assert_eq!(updated["strategy_type"], "price_path_v2");
    assert_eq!(updated["start_price"], "1.100000000000000000");
    assert_eq!(updated["target_price"], "2.200000000000000000");
    assert_eq!(updated["start_time"], update_start.timestamp_millis());
    assert_eq!(updated["end_time"], update_end.timestamp_millis());
    assert_eq!(updated["generator"]["scenario"], "trend_up");
    assert_eq!(updated["generator"]["seed_mode"], "auto");
    assert_eq!(updated["generator"]["seed"], "route-fixed-seed-v1");

    let (stored_type, stored_start, stored_target, stored_status): (
        String,
        BigDecimal,
        BigDecimal,
        String,
    ) = sqlx::query_as(
        r#"SELECT strategy_type, start_price, target_price, status
           FROM market_strategies
           WHERE id = ?"#,
    )
    .bind(strategy_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(stored_type, "price_path_v2");
    assert_eq!(stored_start, decimal("1.100000000000000000"));
    assert_eq!(stored_target, decimal("2.200000000000000000"));
    assert_eq!(stored_status, "paused");

    let (run_status, current_price, last_kline_open_time, recovery_status): (
        String,
        BigDecimal,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<String>,
    ) = sqlx::query_as(
        r#"SELECT run_status, current_price, last_kline_open_time, recovery_status
           FROM strategy_runs
           WHERE strategy_id = ?"#,
    )
    .bind(strategy_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(run_status, "paused");
    assert_eq!(current_price, decimal("1.100000000000000000"));
    assert_eq!(last_kline_open_time, Some(update_start));
    assert_eq!(recovery_status.as_deref(), Some("idle"));

    let versions: Vec<(i32, Option<u64>, String, Value)> = sqlx::query_as(
        r#"SELECT version, created_by, seed, config_json
           FROM strategy_versions
           WHERE strategy_id = ?
           ORDER BY version"#,
    )
    .bind(strategy_id)
    .fetch_all(&pool)
    .await?;
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0].0, 1);
    assert_eq!(versions[1].0, 2);
    assert_eq!(versions[1].1, Some(admin_id));
    assert_eq!(versions[0].2, "route-fixed-seed-v1");
    assert_eq!(versions[1].2, "route-fixed-seed-v1");
    assert_eq!(versions[1].3["strategy_type"], "price_path_v2");
    assert_eq!(versions[1].3["start_time"], update_start.timestamp_millis());
    assert_eq!(versions[1].3["status"], "paused");
    assert_eq!(versions[1].3["generator"]["scenario"], "trend_up");

    let events: Vec<(String, Value)> = sqlx::query_as(
        r#"SELECT event_type, payload_json
           FROM strategy_events
           WHERE strategy_id = ?
           ORDER BY id"#,
    )
    .bind(strategy_id)
    .fetch_all(&pool)
    .await?;
    assert_eq!(events.len(), 4);
    assert_eq!(events[2].0, "market_strategy.update");
    assert_eq!(events[2].1["before"]["strategy_type"], "price_path");
    assert_eq!(events[2].1["after"]["strategy_type"], "price_path_v2");
    assert_eq!(events[3].0, "market_strategy.nodes.snapshot");

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type = 'market_strategy' AND target_id = ?
           ORDER BY id"#,
    )
    .bind(admin_id)
    .bind(strategy_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 3);
    assert_eq!(audits[2].action, "market_strategy.update");
    assert_eq!(
        audits[2].before_json.as_ref().unwrap()["strategy_type"],
        "price_path"
    );
    assert_eq!(
        audits[2].after_json.as_ref().unwrap()["strategy_type"],
        "price_path_v2"
    );
    assert_eq!(audits[2].reason.as_deref(), Some("update config"));

    let version_history = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/market-strategies/{strategy_id}/versions?limit=20&offset=0"
                ))
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let version_history_status = version_history.status();
    let version_history_payload = body_json(version_history).await?;
    assert_eq!(
        version_history_status,
        StatusCode::OK,
        "payload: {version_history_payload}"
    );
    assert_eq!(version_history_payload["total"], 2);
    assert_eq!(version_history_payload["versions"][0]["version"], 2);
    assert_eq!(version_history_payload["versions"][0]["active"], true);
    assert_eq!(
        version_history_payload["versions"][0]["generator"]["scenario"],
        "trend_up"
    );
    assert_eq!(version_history_payload["versions"][1]["version"], 1);
    assert_eq!(version_history_payload["versions"][1]["active"], false);

    let restore = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/admin/api/v1/market-strategies/{strategy_id}/versions/1/restore"
                ))
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "reason": "restore original deterministic path" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let restore_status = restore.status();
    let restored = body_json(restore).await?;
    assert_eq!(restore_status, StatusCode::OK, "payload: {restored}");
    assert_eq!(restored["strategy_type"], "price_path");
    assert_eq!(restored["start_price"], "1.000000000000000000");
    assert_eq!(restored["generator"]["scenario"], "range");
    assert_eq!(restored["generator"]["seed"], "route-fixed-seed-v1");

    let (active_version, restored_seed): (i32, String) = sqlx::query_as(
        r#"SELECT runs.active_version, versions.seed
           FROM strategy_runs runs
           INNER JOIN strategy_versions versions
             ON versions.strategy_id = runs.strategy_id
            AND versions.version = runs.active_version
           WHERE runs.strategy_id = ?"#,
    )
    .bind(strategy_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(active_version, 3);
    assert_eq!(restored_seed, "route-fixed-seed-v1");

    let version_count_after_restore: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM strategy_versions WHERE strategy_id = ?")
            .bind(strategy_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(version_count_after_restore, 3);
    let restore_event_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM strategy_events
           WHERE strategy_id = ? AND event_type = 'market_strategy.version.restored'"#,
    )
    .bind(strategy_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(restore_event_count, 1);
    let restore_audit_reason: String = sqlx::query_scalar(
        r#"SELECT reason FROM admin_audit_logs
           WHERE admin_id = ? AND target_type = 'market_strategy' AND target_id = ?
             AND action = 'market_strategy.version.restore'
           ORDER BY id DESC LIMIT 1"#,
    )
    .bind(admin_id)
    .bind(strategy_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(restore_audit_reason, "restore original deterministic path");

    sqlx::query(
        "DELETE FROM admin_audit_logs WHERE admin_id = ? AND target_type = 'market_strategy'",
    )
    .bind(admin_id)
    .execute(&pool)
    .await?;
    sqlx::query("DELETE FROM strategy_events WHERE strategy_id = ?")
        .bind(strategy_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM strategy_runs WHERE strategy_id = ?")
        .bind(strategy_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM strategy_versions WHERE strategy_id = ?")
        .bind(strategy_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM market_strategies WHERE id = ?")
        .bind(strategy_id)
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
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(role_id)
        .execute(&pool)
        .await?;

    Ok(())
}

#[tokio::test]
async fn admin_market_strategy_status_update_rolls_back_when_run_checkpoint_missing()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let base_asset = create_asset(&pool, "MRB").await;
    let quote_asset = create_asset(&pool, "MRQ").await;
    let symbol = format!("MR{}", Uuid::now_v7().simple()).to_ascii_uppercase();
    let pair_id = sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, 8, 8, ?, 'active', 'strategy')"#,
    )
    .bind(base_asset)
    .bind(quote_asset)
    .bind(&symbol)
    .bind(decimal("1.000000000000000000"))
    .execute(&pool)
    .await?
    .last_insert_id();
    let start_time = chrono::Utc.with_ymd_and_hms(2026, 3, 1, 8, 0, 0).unwrap();
    let end_time = chrono::Utc.with_ymd_and_hms(2026, 3, 1, 9, 0, 0).unwrap();
    let strategy_id = sqlx::query(
        r#"INSERT INTO market_strategies
           (pair_id, strategy_type, start_price, target_price, start_time, end_time,
            volatility, volume_min, volume_max, status)
           VALUES (?, 'price_path', ?, ?, ?, ?, ?, ?, ?, 'active')"#,
    )
    .bind(pair_id)
    .bind(decimal("1.000000000000000000"))
    .bind(decimal("2.000000000000000000"))
    .bind(start_time)
    .bind(end_time)
    .bind(decimal("0.01000000"))
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("20.000000000000000000"))
    .execute(&pool)
    .await?
    .last_insert_id();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let update = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/admin/api/v1/market-strategies/{strategy_id}/status"
                ))
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "status": "paused", "reason": "missing checkpoint" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let update_status = update.status();
    let payload = body_json(update).await?;
    assert_eq!(update_status, StatusCode::CONFLICT, "payload: {payload}");
    assert_eq!(payload["code"], "CONFLICT");
    assert_eq!(
        payload["message"],
        "conflict: market strategy run checkpoint is missing"
    );

    let (stored_status, run_count): (String, i64) = sqlx::query_as(
        r#"SELECT strategies.status, COUNT(runs.strategy_id) AS run_count
           FROM market_strategies strategies
           LEFT JOIN strategy_runs runs ON runs.strategy_id = strategies.id
           WHERE strategies.id = ?
           GROUP BY strategies.id, strategies.status"#,
    )
    .bind(strategy_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(stored_status, "active");
    assert_eq!(run_count, 0);

    let event_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM strategy_events WHERE strategy_id = ?")
            .bind(strategy_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(event_count, 0);
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM admin_audit_logs WHERE admin_id = ? AND target_type = 'market_strategy' AND target_id = ?",
    )
    .bind(admin_id)
    .bind(strategy_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(audit_count, 0);

    sqlx::query("DELETE FROM market_strategies WHERE id = ?")
        .bind(strategy_id)
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
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(role_id)
        .execute(&pool)
        .await?;

    Ok(())
}

#[tokio::test]
async fn admin_agent_management_routes_require_admin_scope_mysql_and_validation()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));
    let body = json!({
        "user_id": 1,
        "agent_code": "agent-code-1",
        "admin_username": "agent-admin-1",
        "admin_password_hash": "hash",
        "level": 1
    })
    .to_string();

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/agents")
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let user = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/agents")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(user.status(), StatusCode::FORBIDDEN);

    let invalid = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/agents")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "user_id": 1,
                        "agent_code": "   ",
                        "admin_username": "agent-admin-1",
                        "admin_password_hash": "hash"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let invalid_payload = body_json(invalid).await?;
    assert_eq!(invalid_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        invalid_payload["message"],
        "validation error: agent_code is required"
    );

    let invalid_user = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/agents")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "user_id": 0,
                        "agent_code": "agent-code-1",
                        "admin_username": "agent-admin-1",
                        "admin_password_hash": "hash"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid_user.status(), StatusCode::BAD_REQUEST);
    let invalid_user_payload = body_json(invalid_user).await?;
    assert_eq!(invalid_user_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        invalid_user_payload["message"],
        "validation error: user_id is required"
    );

    let admin = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/agents")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await?;
    assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let payload = body_json(admin).await?;
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for admin convert routes")
    );

    let status = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/agents/1/status")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "status": "active" }).to_string()))
                .unwrap(),
        )
        .await?;
    assert_eq!(status.status(), StatusCode::INTERNAL_SERVER_ERROR);

    Ok(())
}

#[tokio::test]
async fn admin_agent_commission_status_route_requires_admin_scope_mysql_and_validation()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));
    let body = json!({ "status": "settled", "reason": "settle payout" }).to_string();

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/agent-commissions/1/status")
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let user = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/agent-commissions/1/status")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(user.status(), StatusCode::FORBIDDEN);

    let invalid = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/agent-commissions/1/status")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "status": "paid" }).to_string()))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let invalid_payload = body_json(invalid).await?;
    assert_eq!(invalid_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        invalid_payload["message"],
        "validation error: unsupported agent commission status"
    );

    let admin = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/agent-commissions/1/status")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await?;
    assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let payload = body_json(admin).await?;
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for admin convert routes")
    );

    Ok(())
}

#[tokio::test]
async fn admin_agent_commission_rule_routes_require_admin_scope_mysql_and_validation()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/agent-commission-rules")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let user = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/agent-commission-rules")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(user.status(), StatusCode::FORBIDDEN);

    let missing_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/agent-commission-rules")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "agent_id": 1,
                        "product_type": "convert",
                        "commission_rate": "0.05000000"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing_reason.status(), StatusCode::BAD_REQUEST);
    let missing_reason_payload = body_json(missing_reason).await?;
    assert_eq!(missing_reason_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        missing_reason_payload["message"],
        "validation error: reason is required"
    );

    let unsupported_product = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/agent-commission-rules")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "agent_id": 1,
                        "product_type": "earn",
                        "commission_rate": "0.05000000",
                        "reason": "create spot rule"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(unsupported_product.status(), StatusCode::BAD_REQUEST);
    let unsupported_product_payload = body_json(unsupported_product).await?;
    assert_eq!(
        unsupported_product_payload["message"],
        "validation error: unsupported agent commission product type"
    );

    let invalid_rate = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/agent-commission-rules")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "agent_id": 1,
                        "product_type": "convert",
                        "commission_rate": "1.10000000",
                        "reason": "create invalid rule"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid_rate.status(), StatusCode::BAD_REQUEST);
    let invalid_rate_payload = body_json(invalid_rate).await?;
    assert_eq!(
        invalid_rate_payload["message"],
        "validation error: commission_rate must be between 0 and 1"
    );

    let missing_update_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/agent-commission-rules/1")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "status": "active" }).to_string()))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing_update_reason.status(), StatusCode::BAD_REQUEST);

    let invalid_update_status = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/agent-commission-rules/1")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "status": "archived", "reason": "archive rule" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid_update_status.status(), StatusCode::BAD_REQUEST);
    let invalid_update_status_payload = body_json(invalid_update_status).await?;
    assert_eq!(
        invalid_update_status_payload["message"],
        "validation error: unsupported agent commission rule status"
    );

    let admin = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/agent-commission-rules")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);

    Ok(())
}

#[tokio::test]
async fn admin_agent_commission_rules_crud_filters_and_audits() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let agent_owner_id = create_user(&pool).await;
    let commission_user_id = create_user(&pool).await;
    let agent_code = format!("R{}", Uuid::now_v7().simple()).to_ascii_uppercase();
    let agent_id = sqlx::query(
        r#"INSERT INTO agents (user_id, agent_code, level, path, status)
           VALUES (?, ?, 1, '', 'active')"#,
    )
    .bind(agent_owner_id)
    .bind(agent_code)
    .execute(&pool)
    .await?
    .last_insert_id();
    sqlx::query("UPDATE agents SET root_agent_id = ?, path = ? WHERE id = ?")
        .bind(agent_id)
        .bind(format!("/agent:{agent_id}"))
        .bind(agent_id)
        .execute(&pool)
        .await?;
    let existing_record_id = seed_agent_commission(
        &pool,
        agent_id,
        commission_user_id,
        "convert_order",
        "100.000000000000000000",
        "5.000000000000000000",
        "pending",
    )
    .await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/agent-commission-rules")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "agent_id": agent_id,
                        "product_type": "convert",
                        "commission_rate": "0.08000000",
                        "reason": "create convert commission rule"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let create_status = create.status();
    let created = body_json(create).await?;
    assert_eq!(create_status, StatusCode::OK, "payload: {created}");
    let rule_id = created["id"].as_u64().unwrap();
    assert_eq!(created["agent_id"], agent_id);
    assert_eq!(created["product_type"], "convert");
    assert_eq!(created["commission_rate"], "0.08000000");
    assert_eq!(created["status"], "active");
    assert!(created["created_at"].is_number());
    assert!(created["updated_at"].is_number());

    let filtered = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/agent-commission-rules?agent_id={agent_id}&product_type=convert&status=active&limit=10&offset=0"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let filtered_status = filtered.status();
    let filtered_payload = body_json(filtered).await?;
    assert_eq!(
        filtered_status,
        StatusCode::OK,
        "payload: {filtered_payload}"
    );
    let rules = filtered_payload["rules"].as_array().unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0]["id"], rule_id);

    let offset_filtered = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/agent-commission-rules?agent_id={agent_id}&limit=1&offset=1"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let offset_filtered_status = offset_filtered.status();
    let offset_filtered_payload = body_json(offset_filtered).await?;
    assert_eq!(
        offset_filtered_status,
        StatusCode::OK,
        "payload: {offset_filtered_payload}"
    );
    assert_eq!(
        offset_filtered_payload["rules"].as_array().unwrap().len(),
        0
    );

    let update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/agent-commission-rules/{rule_id}"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "commission_rate": "0.12000000",
                        "status": "disabled",
                        "reason": "disable convert commission rule"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let update_status = update.status();
    let updated = body_json(update).await?;
    assert_eq!(update_status, StatusCode::OK, "payload: {updated}");
    assert_eq!(updated["id"], rule_id);
    assert_eq!(updated["commission_rate"], "0.12000000");
    assert_eq!(updated["status"], "disabled");

    let (record_commission_amount, record_status): (BigDecimal, String) = sqlx::query_as(
        "SELECT commission_amount, status FROM agent_commission_records WHERE id = ?",
    )
    .bind(existing_record_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(record_commission_amount, decimal("5.000000000000000000"));
    assert_eq!(record_status, "pending");

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type = 'agent_commission_rule' AND target_id = ?
           ORDER BY id"#,
    )
    .bind(admin_id)
    .bind(rule_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 2, "audits: {audits:?}");
    assert_eq!(audits[0].action, "agent_commission_rule.create");
    assert!(audits[0].before_json.is_none());
    assert_eq!(audits[0].after_json.as_ref().unwrap()["status"], "active");
    assert_eq!(
        audits[0].reason.as_deref(),
        Some("create convert commission rule")
    );
    assert_eq!(audits[1].action, "agent_commission_rule.update");
    assert_eq!(audits[1].before_json.as_ref().unwrap()["status"], "active");
    assert_eq!(audits[1].after_json.as_ref().unwrap()["status"], "disabled");
    assert_eq!(
        audits[1].reason.as_deref(),
        Some("disable convert commission rule")
    );

    sqlx::query("DELETE FROM agent_commission_rules WHERE agent_id = ?")
        .bind(agent_id)
        .execute(&pool)
        .await?;
    delete_admin_agent_management_fixture(
        &pool,
        admin_id,
        role_id,
        &[agent_id],
        &[agent_owner_id, commission_user_id],
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn admin_agent_commission_status_updates_pending_records_and_audits()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let agent_owner_id = create_user(&pool).await;
    let commission_user_id = create_user(&pool).await;
    let agent_code = format!("C{}", Uuid::now_v7().simple()).to_ascii_uppercase();
    let agent_id = sqlx::query(
        r#"INSERT INTO agents (user_id, agent_code, level, path, status)
           VALUES (?, ?, 1, '', 'active')"#,
    )
    .bind(agent_owner_id)
    .bind(agent_code)
    .execute(&pool)
    .await?
    .last_insert_id();
    sqlx::query("UPDATE agents SET root_agent_id = ?, path = ? WHERE id = ?")
        .bind(agent_id)
        .bind(format!("/agent:{agent_id}"))
        .bind(agent_id)
        .execute(&pool)
        .await?;
    let from_asset = create_asset(&pool, "APF").await;
    let to_asset = create_asset(&pool, "APT").await;
    let pair_id = seed_convert_pair(&pool, from_asset, to_asset, true).await;
    let quote_id = seed_convert_order(
        &pool,
        commission_user_id,
        pair_id,
        from_asset,
        to_asset,
        "completed",
    )
    .await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(agent_owner_id)
        .bind(from_asset)
        .bind(decimal("1.000000000000000000"))
        .execute(&pool)
        .await?;
    let pending_settle_id = seed_agent_commission_with_source_id(
        &pool,
        AgentCommissionSeed {
            agent_id,
            user_id: commission_user_id,
            source_type: "convert_order",
            source_id: &quote_id,
            source_amount: "100.000000000000000000",
            commission_amount: "5.000000000000000000",
            status: "pending",
        },
    )
    .await;
    sqlx::query(
        "UPDATE agent_commission_records SET payout_asset_id = ?, commission_rate = ? WHERE id = ?",
    )
    .bind(from_asset)
    .bind(decimal("0.05000000"))
    .bind(pending_settle_id)
    .execute(&pool)
    .await?;
    let pending_reject_id = seed_agent_commission(
        &pool,
        agent_id,
        commission_user_id,
        "spot_trade",
        "200.000000000000000000",
        "10.000000000000000000",
        "pending",
    )
    .await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let settle = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/admin/api/v1/agent-commissions/{pending_settle_id}/status"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "status": "settled", "reason": "settle payout" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let settle_status = settle.status();
    let settled = body_json(settle).await?;
    assert_eq!(settle_status, StatusCode::OK, "payload: {settled}");
    assert_eq!(settled["id"], pending_settle_id);
    assert_eq!(settled["agent_id"], agent_id);
    assert_eq!(settled["user_id"], commission_user_id);
    assert_eq!(settled["source_type"], "convert_order");
    assert_eq!(settled["commission_amount"], "5.000000000000000000");
    assert_eq!(settled["status"], "settled");

    let (stored_status,): (String,) =
        sqlx::query_as("SELECT status FROM agent_commission_records WHERE id = ?")
            .bind(pending_settle_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(stored_status, "settled");
    let (agent_available,): (BigDecimal,) = sqlx::query_as(
        "SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ? LIMIT 1",
    )
    .bind(agent_owner_id)
    .bind(from_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(agent_available, decimal("6.000000000000000000"));
    let (ledger_count, ledger_amount, ledger_balance_after): (i64, BigDecimal, BigDecimal) =
        sqlx::query_as(
            r#"SELECT COUNT(*), COALESCE(MAX(amount), 0), COALESCE(MAX(balance_after), 0)
               FROM wallet_ledger
               WHERE user_id = ? AND asset_id = ? AND change_type = 'agent_commission_payout'
                 AND balance_type = 'available' AND ref_type = 'agent_commission' AND ref_id = ?"#,
        )
        .bind(agent_owner_id)
        .bind(from_asset)
        .bind(pending_settle_id.to_string())
        .fetch_one(&pool)
        .await?;
    assert_eq!(ledger_count, 1);
    assert_eq!(ledger_amount, decimal("5.000000000000000000"));
    assert_eq!(ledger_balance_after, decimal("6.000000000000000000"));

    let unsupported_settle = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/admin/api/v1/agent-commissions/{pending_reject_id}/status"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "status": "settled", "reason": "settle unsupported source" })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let unsupported_settle_status = unsupported_settle.status();
    let unsupported_settle_payload = body_json(unsupported_settle).await?;
    assert_eq!(
        unsupported_settle_status,
        StatusCode::CONFLICT,
        "payload: {unsupported_settle_payload}"
    );
    assert_eq!(unsupported_settle_payload["code"], "CONFLICT");
    assert_eq!(
        unsupported_settle_payload["message"],
        "conflict: agent commission source cannot be settled without payout support"
    );
    let (unsupported_settle_stored_status,): (String,) =
        sqlx::query_as("SELECT status FROM agent_commission_records WHERE id = ?")
            .bind(pending_reject_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(unsupported_settle_stored_status, "pending");

    let reject = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/admin/api/v1/agent-commissions/{pending_reject_id}/status"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "status": "rejected", "reason": "invalid referral" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let reject_status = reject.status();
    let rejected = body_json(reject).await?;
    assert_eq!(reject_status, StatusCode::OK, "payload: {rejected}");
    assert_eq!(rejected["id"], pending_reject_id);
    assert_eq!(rejected["status"], "rejected");

    let repeat = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/admin/api/v1/agent-commissions/{pending_settle_id}/status"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "status": "rejected" }).to_string()))
                .unwrap(),
        )
        .await?;
    assert_eq!(repeat.status(), StatusCode::CONFLICT);
    let repeat_payload = body_json(repeat).await?;
    assert_eq!(repeat_payload["code"], "CONFLICT");
    assert_eq!(
        repeat_payload["message"],
        "conflict: agent commission status can only be updated from pending"
    );
    let (ledger_count_after_repeat,): (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM wallet_ledger
           WHERE user_id = ? AND asset_id = ? AND change_type = 'agent_commission_payout'
             AND ref_type = 'agent_commission' AND ref_id = ?"#,
    )
    .bind(agent_owner_id)
    .bind(from_asset)
    .bind(pending_settle_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count_after_repeat, 1);
    let (rejected_ledger_count,): (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM wallet_ledger
           WHERE change_type = 'agent_commission_payout' AND ref_type = 'agent_commission' AND ref_id = ?"#,
    )
    .bind(pending_reject_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(rejected_ledger_count, 0);

    sqlx::query(
        "DELETE FROM wallet_ledger WHERE ref_type = 'agent_commission' AND ref_id IN (?, ?)",
    )
    .bind(pending_settle_id.to_string())
    .bind(pending_reject_id.to_string())
    .execute(&pool)
    .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
        .bind(agent_owner_id)
        .bind(from_asset)
        .execute(&pool)
        .await?;

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type = 'agent_commission'
             AND target_id IN (?, ?)
           ORDER BY id"#,
    )
    .bind(admin_id)
    .bind(pending_settle_id.to_string())
    .bind(pending_reject_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 2);
    assert!(audits.iter().all(|audit| {
        audit.action == "agent_commission.status.update"
            && audit.target_type == "agent_commission"
            && audit.before_json.as_ref().unwrap()["status"] == "pending"
    }));
    assert_eq!(audits[0].target_id, pending_settle_id.to_string());
    assert_eq!(audits[0].after_json.as_ref().unwrap()["status"], "settled");
    assert_eq!(audits[0].reason.as_deref(), Some("settle payout"));
    assert_eq!(audits[1].target_id, pending_reject_id.to_string());
    assert_eq!(audits[1].after_json.as_ref().unwrap()["status"], "rejected");
    assert_eq!(audits[1].reason.as_deref(), Some("invalid referral"));

    delete_admin_agent_management_fixture(&pool, admin_id, role_id, &[agent_id], &[agent_owner_id])
        .await?;
    delete_order_fixture(&pool, pair_id, from_asset, to_asset, &[commission_user_id]).await?;
    Ok(())
}

#[tokio::test]
async fn admin_agent_commission_batch_status_route_requires_admin_scope_mysql_and_validation()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));
    let body = json!({ "ids": [1, 2], "status": "settled", "reason": "settle payout" }).to_string();
    let batch_request = |token: Option<&str>, body: String| {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/admin/api/v1/agent-commissions/batch-status")
            .header("content-type", "application/json");
        if let Some(token) = token {
            builder = builder.header(AUTHORIZATION, format!("Bearer {token}"));
        }
        builder.body(Body::from(body)).unwrap()
    };

    let missing = app
        .clone()
        .oneshot(batch_request(None, body.clone()))
        .await?;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let user = app
        .clone()
        .oneshot(batch_request(Some(&user_token), body.clone()))
        .await?;
    assert_eq!(user.status(), StatusCode::FORBIDDEN);

    let invalid_status = app
        .clone()
        .oneshot(batch_request(
            Some(&admin_token),
            json!({ "ids": [1], "status": "paid" }).to_string(),
        ))
        .await?;
    assert_eq!(invalid_status.status(), StatusCode::BAD_REQUEST);
    let invalid_status_payload = body_json(invalid_status).await?;
    assert_eq!(invalid_status_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        invalid_status_payload["message"],
        "validation error: unsupported agent commission status"
    );

    let empty_ids = app
        .clone()
        .oneshot(batch_request(
            Some(&admin_token),
            json!({ "ids": [], "status": "settled" }).to_string(),
        ))
        .await?;
    assert_eq!(empty_ids.status(), StatusCode::BAD_REQUEST);
    let empty_ids_payload = body_json(empty_ids).await?;
    assert_eq!(
        empty_ids_payload["message"],
        "validation error: at least one agent commission id is required"
    );

    let oversized_ids = (1..=201).collect::<Vec<u64>>();
    let oversized = app
        .clone()
        .oneshot(batch_request(
            Some(&admin_token),
            json!({ "ids": oversized_ids, "status": "settled" }).to_string(),
        ))
        .await?;
    assert_eq!(oversized.status(), StatusCode::BAD_REQUEST);
    let oversized_payload = body_json(oversized).await?;
    assert_eq!(
        oversized_payload["message"],
        "validation error: a single batch cannot contain more than 200 agent commissions"
    );

    let duplicated = app
        .clone()
        .oneshot(batch_request(
            Some(&admin_token),
            json!({ "ids": [1, 1], "status": "settled" }).to_string(),
        ))
        .await?;
    assert_eq!(duplicated.status(), StatusCode::BAD_REQUEST);
    let duplicated_payload = body_json(duplicated).await?;
    assert_eq!(
        duplicated_payload["message"],
        "validation error: duplicate agent commission id in batch"
    );

    let admin = app.oneshot(batch_request(Some(&admin_token), body)).await?;
    assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let payload = body_json(admin).await?;
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for admin convert routes")
    );

    Ok(())
}

#[tokio::test]
async fn admin_agent_commission_batch_status_settles_per_record_and_reports_outcomes()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let agent_owner_id = create_user(&pool).await;
    let commission_user_id = create_user(&pool).await;
    let agent_code = format!("B{}", Uuid::now_v7().simple()).to_ascii_uppercase();
    let agent_id = sqlx::query(
        r#"INSERT INTO agents (user_id, agent_code, level, path, status)
           VALUES (?, ?, 1, '', 'active')"#,
    )
    .bind(agent_owner_id)
    .bind(agent_code)
    .execute(&pool)
    .await?
    .last_insert_id();
    sqlx::query("UPDATE agents SET root_agent_id = ?, path = ? WHERE id = ?")
        .bind(agent_id)
        .bind(format!("/agent:{agent_id}"))
        .bind(agent_id)
        .execute(&pool)
        .await?;
    let from_asset = create_asset(&pool, "ABF").await;
    let to_asset = create_asset(&pool, "ABT").await;
    let pair_id = seed_convert_pair(&pool, from_asset, to_asset, true).await;
    let quote_id = seed_convert_order(
        &pool,
        commission_user_id,
        pair_id,
        from_asset,
        to_asset,
        "completed",
    )
    .await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(agent_owner_id)
        .bind(from_asset)
        .bind(decimal("1.000000000000000000"))
        .execute(&pool)
        .await?;
    let pending_settle_id = seed_agent_commission_with_source_id(
        &pool,
        AgentCommissionSeed {
            agent_id,
            user_id: commission_user_id,
            source_type: "convert_order",
            source_id: &quote_id,
            source_amount: "100.000000000000000000",
            commission_amount: "5.000000000000000000",
            status: "pending",
        },
    )
    .await;
    sqlx::query(
        "UPDATE agent_commission_records SET payout_asset_id = ?, commission_rate = ? WHERE id = ?",
    )
    .bind(from_asset)
    .bind(decimal("0.05000000"))
    .bind(pending_settle_id)
    .execute(&pool)
    .await?;
    let unsupported_id = seed_agent_commission(
        &pool,
        agent_id,
        commission_user_id,
        "spot_trade",
        "200.000000000000000000",
        "10.000000000000000000",
        "pending",
    )
    .await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let settle = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/agent-commissions/batch-status")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "ids": [pending_settle_id, unsupported_id, u64::MAX],
                        "status": "settled",
                        "reason": "batch settle payout"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let settle_status = settle.status();
    let settled = body_json(settle).await?;
    assert_eq!(settle_status, StatusCode::OK, "payload: {settled}");
    let results = settled["results"].as_array().unwrap();
    assert_eq!(results.len(), 3);
    assert_eq!(results[0]["id"], pending_settle_id);
    assert_eq!(results[0]["status"], "ok");
    assert!(results[0]["error"].is_null());
    assert_eq!(results[1]["id"], unsupported_id);
    assert_eq!(results[1]["status"], "failed");
    assert_eq!(
        results[1]["error"],
        "conflict: agent commission source cannot be settled without payout support"
    );
    assert_eq!(results[2]["id"], u64::MAX);
    assert_eq!(results[2]["status"], "failed");
    assert_eq!(results[2]["error"], "not found");

    let (settle_stored_status,): (String,) =
        sqlx::query_as("SELECT status FROM agent_commission_records WHERE id = ?")
            .bind(pending_settle_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(settle_stored_status, "settled");
    let (unsupported_stored_status,): (String,) =
        sqlx::query_as("SELECT status FROM agent_commission_records WHERE id = ?")
            .bind(unsupported_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(unsupported_stored_status, "pending");
    let (agent_available,): (BigDecimal,) = sqlx::query_as(
        "SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ? LIMIT 1",
    )
    .bind(agent_owner_id)
    .bind(from_asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(agent_available, decimal("6.000000000000000000"));
    let (ledger_count,): (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM wallet_ledger
           WHERE user_id = ? AND asset_id = ? AND change_type = 'agent_commission_payout'
             AND ref_type = 'agent_commission' AND ref_id = ?"#,
    )
    .bind(agent_owner_id)
    .bind(from_asset)
    .bind(pending_settle_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 1);

    let repeat = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/agent-commissions/batch-status")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "ids": [pending_settle_id], "status": "settled" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let repeat_payload = body_json(repeat).await?;
    assert_eq!(repeat_payload["results"][0]["status"], "failed");
    assert_eq!(
        repeat_payload["results"][0]["error"],
        "conflict: agent commission status can only be updated from pending"
    );
    let (ledger_count_after_repeat,): (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM wallet_ledger
           WHERE user_id = ? AND asset_id = ? AND change_type = 'agent_commission_payout'
             AND ref_type = 'agent_commission' AND ref_id = ?"#,
    )
    .bind(agent_owner_id)
    .bind(from_asset)
    .bind(pending_settle_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count_after_repeat, 1);

    let reject = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/agent-commissions/batch-status")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "ids": [unsupported_id],
                        "status": "rejected",
                        "reason": "batch reject"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let rejected = body_json(reject).await?;
    assert_eq!(rejected["results"][0]["status"], "ok");
    let (rejected_stored_status,): (String,) =
        sqlx::query_as("SELECT status FROM agent_commission_records WHERE id = ?")
            .bind(unsupported_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(rejected_stored_status, "rejected");

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type = 'agent_commission'
             AND target_id IN (?, ?)
           ORDER BY id"#,
    )
    .bind(admin_id)
    .bind(pending_settle_id.to_string())
    .bind(unsupported_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 2, "audits: {audits:?}");
    assert!(
        audits
            .iter()
            .all(|audit| audit.action == "agent_commission.status.update")
    );
    assert_eq!(audits[0].target_id, pending_settle_id.to_string());
    assert_eq!(audits[0].after_json.as_ref().unwrap()["status"], "settled");
    assert_eq!(audits[0].reason.as_deref(), Some("batch settle payout"));
    assert_eq!(audits[1].target_id, unsupported_id.to_string());
    assert_eq!(audits[1].after_json.as_ref().unwrap()["status"], "rejected");
    assert_eq!(audits[1].reason.as_deref(), Some("batch reject"));

    sqlx::query(
        "DELETE FROM wallet_ledger WHERE ref_type = 'agent_commission' AND ref_id IN (?, ?)",
    )
    .bind(pending_settle_id.to_string())
    .bind(unsupported_id.to_string())
    .execute(&pool)
    .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
        .bind(agent_owner_id)
        .bind(from_asset)
        .execute(&pool)
        .await?;
    delete_admin_agent_management_fixture(&pool, admin_id, role_id, &[agent_id], &[agent_owner_id])
        .await?;
    delete_order_fixture(&pool, pair_id, from_asset, to_asset, &[commission_user_id]).await?;
    Ok(())
}

#[tokio::test]
async fn admin_agents_list_detail_filters_and_password_hashing() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let agent_owner_email = format!("admin-agent-owner-{}@example.test", Uuid::now_v7().simple());
    let agent_owner_id = create_user_with_email(&pool, agent_owner_email.clone()).await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let agent_code = format!("L{}", Uuid::now_v7().simple()).to_ascii_uppercase();
    let admin_username = format!("agent-admin-list-{}", Uuid::now_v7().simple());
    let initial_password = "initial-agent-password-1";

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/agents")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "user_id": agent_owner_id,
                        "agent_code": agent_code,
                        "admin_username": admin_username,
                        "admin_password": initial_password,
                        "level": 1,
                        "reason": "create agent with initial password"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let create_status = create.status();
    let created = body_json(create).await?;
    assert_eq!(create_status, StatusCode::OK, "payload: {created}");
    let agent_id = created["id"].as_u64().unwrap();
    let agent_admin_user_id = created["admin_user_id"].as_u64().unwrap();
    assert_eq!(created["user_id"], agent_owner_id);
    assert_eq!(created["email"], agent_owner_email);
    assert_eq!(created["agent_code"], agent_code);
    assert_eq!(created["level"], 1);
    assert_eq!(created["parent_agent_id"], Value::Null);
    assert_eq!(created["root_agent_id"], agent_id);
    assert_eq!(created["path"], format!("/agent:{agent_id}"));
    assert_eq!(created["status"], "active");
    assert_eq!(created["admin_username"], admin_username);
    assert_eq!(created["admin_status"], "active");
    let created_text = created.to_string();
    assert!(!created_text.contains(initial_password));
    assert!(!created_text.contains("password_hash"));
    assert!(!created_text.contains("admin_password"));

    let stored_hash: String =
        sqlx::query_scalar("SELECT password_hash FROM agent_admin_users WHERE id = ?")
            .bind(agent_admin_user_id)
            .fetch_one(&pool)
            .await?;
    assert_ne!(stored_hash, initial_password);
    assert!(stored_hash.starts_with("$argon2"));
    assert!(verify_password(&stored_hash, initial_password)?);

    let second_agent_admin_user_id = sqlx::query(
        r#"INSERT INTO agent_admin_users (agent_id, username, password_hash, status)
           VALUES (?, ?, ?, 'suspended')"#,
    )
    .bind(agent_id)
    .bind(format!("{admin_username}-duplicate"))
    .bind(&stored_hash)
    .execute(&pool)
    .await?
    .last_insert_id();
    assert_ne!(second_agent_admin_user_id, agent_admin_user_id);

    let detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/api/v1/agents/{agent_id}"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let detail_status = detail.status();
    let detail_payload = body_json(detail).await?;
    assert_eq!(detail_status, StatusCode::OK, "payload: {detail_payload}");
    assert_eq!(detail_payload["id"], agent_id);
    assert_eq!(detail_payload["email"], agent_owner_email);
    assert!(!detail_payload.to_string().contains("password_hash"));

    let filtered = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/agents?agent_id={agent_id}&user_id={agent_owner_id}&agent_code={agent_code}&email={agent_owner_email}&status=active&limit=10&offset=0"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let filtered_status = filtered.status();
    let filtered_payload = body_json(filtered).await?;
    assert_eq!(
        filtered_status,
        StatusCode::OK,
        "payload: {filtered_payload}"
    );
    let agents = filtered_payload["agents"].as_array().unwrap();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0]["id"], agent_id);
    assert_eq!(agents[0]["email"], agent_owner_email);
    assert!(!filtered_payload.to_string().contains("password_hash"));

    let offset_filtered = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/agents?agent_id={agent_id}&limit=1&offset=1"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let offset_filtered_status = offset_filtered.status();
    let offset_filtered_payload = body_json(offset_filtered).await?;
    assert_eq!(
        offset_filtered_status,
        StatusCode::OK,
        "payload: {offset_filtered_payload}"
    );
    assert_eq!(
        offset_filtered_payload["agents"].as_array().unwrap().len(),
        0
    );

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type = 'agent' AND target_id = ?
           ORDER BY id"#,
    )
    .bind(admin_id)
    .bind(agent_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 1, "audits: {audits:?}");
    assert_eq!(audits[0].action, "agent.create");
    assert!(audits[0].before_json.is_none());
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["agent_code"],
        agent_code
    );
    let audit_text = audits[0].after_json.as_ref().unwrap().to_string();
    assert!(!audit_text.contains(initial_password));
    assert!(!audit_text.contains(&stored_hash));
    assert!(!audit_text.contains("password_hash"));
    assert_eq!(
        audits[0].reason.as_deref(),
        Some("create agent with initial password")
    );

    delete_admin_agent_management_fixture(&pool, admin_id, role_id, &[agent_id], &[agent_owner_id])
        .await?;
    Ok(())
}

#[tokio::test]
async fn admin_agents_create_three_level_hierarchy_and_reject_a_fourth_level()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let owner_ids = [
        create_user(&pool).await,
        create_user(&pool).await,
        create_user(&pool).await,
        create_user(&pool).await,
    ];
    let team_user_ids = [
        create_user(&pool).await,
        create_user(&pool).await,
        create_user(&pool).await,
    ];
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let root_code = format!("R{}", Uuid::now_v7().simple()).to_ascii_uppercase();
    let root_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/agents")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "user_id": owner_ids[0],
                        "agent_code": root_code,
                        "admin_username": format!("root-{}", Uuid::now_v7().simple()),
                        "admin_password_hash": "not-a-real-hash",
                        "level": 1
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(root_response.status(), StatusCode::OK);
    let root = body_json(root_response).await?;
    let root_id = root["id"].as_u64().unwrap();
    assert_eq!(root["level"], 1);
    assert_eq!(root["root_agent_id"], root_id);
    assert_eq!(root["path"], format!("/agent:{root_id}"));

    let child_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/agents")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "user_id": owner_ids[1],
                        "parent_agent_id": root_id,
                        "agent_code": format!("C{}", Uuid::now_v7().simple()).to_ascii_uppercase(),
                        "admin_username": format!("child-{}", Uuid::now_v7().simple()),
                        "admin_password_hash": "not-a-real-hash",
                        "level": 2
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(child_response.status(), StatusCode::OK);
    let child = body_json(child_response).await?;
    let child_id = child["id"].as_u64().unwrap();
    assert_eq!(child["parent_agent_id"], root_id);
    assert_eq!(child["root_agent_id"], root_id);
    assert_eq!(child["level"], 2);
    assert_eq!(child["path"], format!("/agent:{root_id}/agent:{child_id}"));

    let grandchild_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/agents")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "user_id": owner_ids[2],
                        "parent_agent_id": child_id,
                        "agent_code": format!("G{}", Uuid::now_v7().simple()).to_ascii_uppercase(),
                        "admin_username": format!("grandchild-{}", Uuid::now_v7().simple()),
                        "admin_password_hash": "not-a-real-hash"
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(grandchild_response.status(), StatusCode::OK);
    let grandchild = body_json(grandchild_response).await?;
    let grandchild_id = grandchild["id"].as_u64().unwrap();
    assert_eq!(grandchild["parent_agent_id"], child_id);
    assert_eq!(grandchild["root_agent_id"], root_id);
    assert_eq!(grandchild["level"], 3);

    let fourth_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/agents")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "user_id": owner_ids[3],
                        "parent_agent_id": grandchild_id,
                        "agent_code": format!("X{}", Uuid::now_v7().simple()).to_ascii_uppercase(),
                        "admin_username": format!("fourth-{}", Uuid::now_v7().simple()),
                        "admin_password_hash": "not-a-real-hash"
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(fourth_response.status(), StatusCode::BAD_REQUEST);

    let filtered = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/agents?parent_agent_id={root_id}&root_agent_id={root_id}&level=2"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(filtered.status(), StatusCode::OK);
    let filtered = body_json(filtered).await?;
    assert_eq!(filtered["agents"].as_array().unwrap().len(), 1);
    assert_eq!(filtered["agents"][0]["id"], child_id);

    for (user_id, agent_id) in [
        (team_user_ids[0], root_id),
        (team_user_ids[1], child_id),
        (team_user_ids[2], grandchild_id),
    ] {
        sqlx::query(
            r#"INSERT INTO user_referrals
               (user_id, direct_inviter_id, direct_inviter_type, root_agent_id, depth, path)
               VALUES (?, ?, 'agent', ?, 1, ?)"#,
        )
        .bind(user_id)
        .bind(agent_id)
        .bind(agent_id)
        .bind(format!("/agent:{agent_id}/user:{user_id}"))
        .execute(&pool)
        .await?;
    }

    for (agent_id, expected_user_ids) in [
        (root_id, team_user_ids.to_vec()),
        (child_id, team_user_ids[1..].to_vec()),
        (grandchild_id, vec![team_user_ids[2]]),
    ] {
        let users_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/admin/api/v1/agents/{agent_id}/users?limit=10"))
                    .header(AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(users_response.status(), StatusCode::OK);
        let users_payload = body_json(users_response).await?;
        let actual_user_ids = users_payload["users"]
            .as_array()
            .unwrap()
            .iter()
            .map(|user| user["user_id"].as_u64().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(actual_user_ids, expected_user_ids);
    }

    delete_admin_agent_management_fixture(
        &pool,
        admin_id,
        role_id,
        &[grandchild_id, child_id, root_id],
        &[
            owner_ids[0],
            owner_ids[1],
            owner_ids[2],
            owner_ids[3],
            team_user_ids[0],
            team_user_ids[1],
            team_user_ids[2],
        ],
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn admin_agent_management_create_update_assign_list_and_audit() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let agent_owner_id = create_user(&pool).await;
    let team_user_email = format!("admin-agent-team-{}@example.test", Uuid::now_v7().simple());
    let team_user_id = create_user_with_email(&pool, team_user_email.clone()).await;
    let other_user_id = create_user(&pool).await;
    let child_user_id = create_user(&pool).await;
    let unrelated_team_user_id = create_user(&pool).await;
    let reserved_collision_user_id = create_user(&pool).await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let agent_code = format!("A{}", Uuid::now_v7().simple()).to_ascii_uppercase();
    let admin_username = format!("agent-admin-{}", Uuid::now_v7().simple());

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/agents")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "user_id": agent_owner_id,
                        "agent_code": agent_code,
                        "admin_username": admin_username,
                        "admin_password_hash": "not-a-real-hash",
                        "level": 1,
                        "reason": "create managed agent"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let create_status = create.status();
    let created = body_json(create).await?;
    assert_eq!(create_status, StatusCode::OK, "payload: {created}");
    let agent_id = created["id"].as_u64().unwrap();
    let agent_admin_user_id = created["admin_user_id"].as_u64().unwrap();
    assert_eq!(created["user_id"], agent_owner_id);
    assert_eq!(created["agent_code"], agent_code);
    assert_eq!(created["level"], 1);
    assert_eq!(created["parent_agent_id"], Value::Null);
    assert_eq!(created["root_agent_id"], agent_id);
    assert_eq!(created["status"], "active");
    assert_eq!(created["admin_username"], admin_username);
    assert_eq!(created["admin_status"], "active");

    let duplicate = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/agents")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "user_id": agent_owner_id,
                        "agent_code": agent_code,
                        "admin_username": format!("agent-admin-{}", Uuid::now_v7().simple()),
                        "admin_password_hash": "not-a-real-hash"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let duplicate_status = duplicate.status();
    let duplicate_payload = body_json(duplicate).await?;
    assert_eq!(
        duplicate_status,
        StatusCode::CONFLICT,
        "payload: {duplicate_payload}"
    );

    let missing_user = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/agents")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "user_id": u64::MAX,
                        "agent_code": format!("M{}", Uuid::now_v7().simple()).to_ascii_uppercase(),
                        "admin_username": format!("agent-admin-{}", Uuid::now_v7().simple()),
                        "admin_password_hash": "not-a-real-hash"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let missing_user_status = missing_user.status();
    let missing_user_payload = body_json(missing_user).await?;
    assert_eq!(
        missing_user_status,
        StatusCode::NOT_FOUND,
        "payload: {missing_user_payload}"
    );

    let status_update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/agents/{agent_id}/status"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "status": "suspended", "reason": "risk control" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let status_update_status = status_update.status();
    let updated = body_json(status_update).await?;
    assert_eq!(status_update_status, StatusCode::OK, "payload: {updated}");
    assert_eq!(updated["id"], agent_id);
    assert_eq!(updated["status"], "suspended");

    let assign_to_suspended = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/users/{team_user_id}/agent"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "agent_id": agent_id, "reason": "manual assignment" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let assign_to_suspended_status = assign_to_suspended.status();
    let assign_to_suspended_payload = body_json(assign_to_suspended).await?;
    assert_eq!(
        assign_to_suspended_status,
        StatusCode::CONFLICT,
        "payload: {assign_to_suspended_payload}"
    );
    assert_eq!(assign_to_suspended_payload["code"], "CONFLICT");
    assert!(
        !assign_to_suspended_payload
            .to_string()
            .contains("not-a-real-hash")
    );

    let reactivate = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/agents/{agent_id}/status"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "status": "active", "reason": "enable assignment" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let reactivate_status = reactivate.status();
    let reactivate_payload = body_json(reactivate).await?;
    assert_eq!(
        reactivate_status,
        StatusCode::OK,
        "payload: {reactivate_payload}"
    );
    assert_eq!(reactivate_payload["status"], "active");

    sqlx::query(
        r#"INSERT INTO user_referrals
           (user_id, direct_inviter_id, direct_inviter_type, root_agent_id, depth, path)
           VALUES (?, NULL, NULL, NULL, 0, ?)"#,
    )
    .bind(team_user_id)
    .bind(format!("/{team_user_id}"))
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"INSERT INTO user_referrals
           (user_id, direct_inviter_id, direct_inviter_type, root_agent_id, depth, path)
           VALUES (?, ?, 'user', NULL, 1, ?)"#,
    )
    .bind(child_user_id)
    .bind(team_user_id)
    .bind(format!("/{team_user_id}/{child_user_id}"))
    .execute(&pool)
    .await?;

    let unrelated_agent_code = format!("B{}", Uuid::now_v7().simple()).to_ascii_uppercase();
    let unrelated_agent_id = sqlx::query(
        r#"INSERT INTO agents (user_id, agent_code, level, path, status)
           VALUES (?, ?, 1, '', 'active')"#,
    )
    .bind(reserved_collision_user_id)
    .bind(&unrelated_agent_code)
    .execute(&pool)
    .await?
    .last_insert_id();
    sqlx::query("UPDATE agents SET root_agent_id = ?, path = ? WHERE id = ?")
        .bind(unrelated_agent_id)
        .bind(format!("/agent:{unrelated_agent_id}"))
        .bind(unrelated_agent_id)
        .execute(&pool)
        .await?;
    sqlx::query(
        r#"INSERT INTO user_referrals
           (user_id, direct_inviter_id, direct_inviter_type, root_agent_id, depth, path)
           VALUES (?, ?, 'user', ?, 1, ?)"#,
    )
    .bind(unrelated_team_user_id)
    .bind(team_user_id)
    .bind(unrelated_agent_id)
    .bind(format!("/{team_user_id}/{unrelated_team_user_id}"))
    .execute(&pool)
    .await?;

    let assign = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/users/{team_user_id}/agent"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "agent_id": agent_id, "reason": "manual assignment" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let assign_status = assign.status();
    let assigned = body_json(assign).await?;
    assert_eq!(assign_status, StatusCode::OK, "payload: {assigned}");
    assert_eq!(assigned["user_id"], team_user_id);
    assert_eq!(assigned["root_agent_id"], agent_id);
    assert_eq!(assigned["direct_inviter_type"], "agent");
    assert_eq!(assigned["depth"], 1);

    let users = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/api/v1/agents/{agent_id}/users?limit=10"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let users_status = users.status();
    let users_payload = body_json(users).await?;
    assert_eq!(users_status, StatusCode::OK, "payload: {users_payload}");
    let users = users_payload["users"].as_array().unwrap();
    assert_eq!(users.len(), 2);
    assert!(users.iter().any(|user| user["user_id"] == team_user_id
        && user["owner_agent_id"] == agent_id
        && user["root_agent_id"] == agent_id
        && user["direct_inviter_type"] == "agent"
        && user["depth"] == 1));
    assert!(users.iter().any(|user| user["user_id"] == child_user_id
        && user["owner_agent_id"] == agent_id
        && user["root_agent_id"] == agent_id
        && user["direct_inviter_type"] == "user"
        && user["direct_inviter_id"] == team_user_id
        && user["depth"] == 2));
    let unrelated_referral: (Option<u64>, String) =
        sqlx::query_as("SELECT root_agent_id, path FROM user_referrals WHERE user_id = ? LIMIT 1")
            .bind(unrelated_team_user_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        unrelated_referral.0,
        Some(unrelated_agent_id),
        "different old root_agent_id should block subtree migration"
    );
    assert_eq!(
        unrelated_referral.1,
        format!("/{team_user_id}/{unrelated_team_user_id}"),
        "path should not be rewritten for a row sharing old path but belonging to another root"
    );

    seed_agent_commission(
        &pool,
        agent_id,
        team_user_id,
        "spot_trade",
        "100.000000000000000000",
        "5.000000000000000000",
        "pending",
    )
    .await;
    seed_agent_commission(
        &pool,
        agent_id,
        other_user_id,
        "spot_trade",
        "200.000000000000000000",
        "10.000000000000000000",
        "pending",
    )
    .await;

    let commissions = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/agent-commissions?agent_id={agent_id}&email={team_user_email}&status=pending&limit=10"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let commissions_status = commissions.status();
    let commissions_payload = body_json(commissions).await?;
    assert_eq!(
        commissions_status,
        StatusCode::OK,
        "payload: {commissions_payload}"
    );
    let commissions = commissions_payload["commissions"].as_array().unwrap();
    assert_eq!(commissions.len(), 1);
    assert_eq!(commissions[0]["agent_id"], agent_id);
    assert_eq!(commissions[0]["user_id"], team_user_id);
    assert_eq!(commissions[0]["commission_amount"], "5.000000000000000000");
    assert_eq!(commissions[0]["status"], "pending");

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type IN ('agent', 'user_referral')
           ORDER BY id"#,
    )
    .bind(admin_id)
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 4);
    assert_eq!(audits[0].action, "agent.create");
    assert_eq!(audits[0].target_type, "agent");
    assert_eq!(audits[0].target_id, agent_id.to_string());
    assert!(audits[0].before_json.is_none());
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["agent_code"],
        agent_code
    );
    assert_eq!(audits[0].reason.as_deref(), Some("create managed agent"));
    assert_eq!(audits[1].action, "agent.status.update");
    assert_eq!(audits[1].before_json.as_ref().unwrap()["status"], "active");
    assert_eq!(
        audits[1].after_json.as_ref().unwrap()["status"],
        "suspended"
    );
    assert_eq!(audits[1].reason.as_deref(), Some("risk control"));
    assert_eq!(audits[2].action, "agent.status.update");
    assert_eq!(
        audits[2].before_json.as_ref().unwrap()["status"],
        "suspended"
    );
    assert_eq!(audits[2].after_json.as_ref().unwrap()["status"], "active");
    assert_eq!(audits[2].reason.as_deref(), Some("enable assignment"));
    assert_eq!(audits[3].action, "user_referral.assign_agent");
    assert_eq!(audits[3].target_type, "user_referral");
    assert_eq!(audits[3].target_id, team_user_id.to_string());
    assert_eq!(
        audits[3].after_json.as_ref().unwrap()["root_agent_id"],
        agent_id
    );
    assert_eq!(audits[3].reason.as_deref(), Some("manual assignment"));

    delete_admin_agent_management_fixture(
        &pool,
        admin_id,
        role_id,
        &[agent_id, unrelated_agent_id],
        &[
            agent_owner_id,
            team_user_id,
            child_user_id,
            other_user_id,
            reserved_collision_user_id,
            unrelated_team_user_id,
        ],
    )
    .await?;
    let _ = agent_admin_user_id;
    Ok(())
}

#[tokio::test]
async fn admin_new_coin_post_listing_purchase_routes_require_admin_scope_and_validation()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));
    let body = json!({
        "enabled": true,
        "pair_id": 1,
        "reason": "open listed purchase"
    })
    .to_string();

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/new-coins/1/post-listing-purchase")
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let user = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/new-coins/1/post-listing-purchase")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(user.status(), StatusCode::FORBIDDEN);

    let invalid = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/new-coins/1/post-listing-purchase")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "enabled": true }).to_string()))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let invalid_payload = body_json(invalid).await?;
    assert_eq!(invalid_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        invalid_payload["message"],
        "validation error: pair_id is required when post-listing purchase is enabled"
    );

    let admin = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/new-coins/1/post-listing-purchase")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await?;
    assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let payload = body_json(admin).await?;
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for admin convert routes")
    );

    Ok(())
}

#[tokio::test]
async fn admin_new_coin_post_listing_purchase_updates_project_pair_and_audit()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let (asset_id, base_symbol) = create_asset_with_symbol(&pool, "AP").await;
    let quote_asset = create_asset(&pool, "AQ").await;
    let project_id = sqlx::query(
        r#"INSERT INTO new_coin_projects
           (asset_id, symbol, lifecycle_status, total_supply, issue_price, listed_at,
            quote_asset_id, unlock_type, fixed_unlock_at, status, remaining_supply)
           VALUES (?, ?, 'listed', ?, ?, CURRENT_TIMESTAMP(6), ?, 'fixed_time',
                   DATE_ADD(CURRENT_TIMESTAMP(6), INTERVAL 7 DAY), 'active', 1000000)"#,
    )
    .bind(asset_id)
    .bind(&base_symbol)
    .bind(decimal("1000000.000000000000000000"))
    .bind(decimal("1.000000000000000000"))
    .bind(quote_asset)
    .execute(&pool)
    .await?
    .last_insert_id();
    let pair_id = sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision,
            min_order_value, status, market_type)
           VALUES (?, ?, ?, 2, 4, ?, 'disabled', 'spot')"#,
    )
    .bind(asset_id)
    .bind(quote_asset)
    .bind(format!("{base_symbol}-USDT"))
    .bind(decimal("1.000000000000000000"))
    .execute(&pool)
    .await?
    .last_insert_id();
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let enable = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/admin/api/v1/new-coins/{project_id}/post-listing-purchase"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "enabled": true,
                        "pair_id": pair_id,
                        "reason": "open listed purchase"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let enable_status = enable.status();
    let enabled = body_json(enable).await?;
    assert_eq!(enable_status, StatusCode::OK, "payload: {enabled}");
    assert_eq!(enabled["id"], project_id);
    assert_eq!(enabled["post_listing_purchase_enabled"], true);
    assert_eq!(enabled["post_listing_pair_id"], pair_id);
    assert_eq!(enabled["post_listing_pair_status"], "active");

    let (project_enabled, project_pair_id): (bool, Option<u64>) = sqlx::query_as(
        "SELECT post_listing_purchase_enabled, post_listing_pair_id FROM new_coin_projects WHERE id = ?",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await?;
    assert!(project_enabled);
    assert_eq!(project_pair_id, Some(pair_id));
    let (pair_status,): (String,) = sqlx::query_as("SELECT status FROM trading_pairs WHERE id = ?")
        .bind(pair_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(pair_status, "active");

    let invalid_pair = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/admin/api/v1/new-coins/{project_id}/post-listing-purchase"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "enabled": true,
                        "pair_id": u64::MAX,
                        "reason": "wrong pair"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid_pair.status(), StatusCode::NOT_FOUND);

    let disable = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/admin/api/v1/new-coins/{project_id}/post-listing-purchase"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "enabled": false,
                        "reason": "close listed purchase"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let disable_status = disable.status();
    let disabled = body_json(disable).await?;
    assert_eq!(disable_status, StatusCode::OK, "payload: {disabled}");
    assert_eq!(disabled["post_listing_purchase_enabled"], false);
    assert_eq!(disabled["post_listing_pair_id"], Value::Null);
    assert_eq!(disabled["post_listing_pair_status"], Value::Null);

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type = 'new_coin_project' AND target_id = ?
           ORDER BY id"#,
    )
    .bind(admin_id)
    .bind(project_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 2);
    assert!(audits.iter().all(|audit| {
        audit.action == "new_coin_project.post_listing_purchase.update"
            && audit.target_id == project_id.to_string()
    }));
    assert_eq!(
        audits[0].before_json.as_ref().unwrap()["post_listing_purchase_enabled"],
        false
    );
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["post_listing_purchase_enabled"],
        true
    );
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["post_listing_pair_id"],
        pair_id
    );
    assert_eq!(audits[0].reason.as_deref(), Some("open listed purchase"));
    assert_eq!(
        audits[1].before_json.as_ref().unwrap()["post_listing_purchase_enabled"],
        true
    );
    assert_eq!(
        audits[1].after_json.as_ref().unwrap()["post_listing_purchase_enabled"],
        false
    );
    assert_eq!(audits[1].reason.as_deref(), Some("close listed purchase"));

    delete_new_coin_project_fixture_with_pairs(
        &pool,
        project_id,
        asset_id,
        &[pair_id],
        admin_id,
        role_id,
    )
    .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(quote_asset)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn admin_new_coin_project_routes_require_admin_scope_and_mysql() -> Result<(), Box<dyn Error>>
{
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));
    let body = json!({
        "asset_id": 1,
        "symbol": "NEWTEST",
        "lifecycle_status": "preheat",
        "total_supply": "1000000.000000000000000000",
        "issue_price": "1.000000000000000000",
        "quote_asset_id": 2,
        "unlock_type": "fixed_time",
        "fixed_unlock_at": 1794309753000_i64
    })
    .to_string();

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/new-coins")
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let user = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/new-coins")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(user.status(), StatusCode::FORBIDDEN);

    let invalid = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/new-coins")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "asset_id": 1,
                        "symbol": "NEWTEST",
                        "lifecycle_status": "preheat",
                        "total_supply": "1000000.000000000000000000",
                        "issue_price": "1.000000000000000000",
                        "quote_asset_id": 2,
                        "unlock_type": "fixed_time"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let invalid_payload = body_json(invalid).await?;
    assert_eq!(invalid_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        invalid_payload["message"],
        "validation error: fixed_unlock_at is required for fixed_time unlock"
    );

    let immediate = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/new-coins")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "asset_id": 1,
                        "symbol": "NEWTEST",
                        "lifecycle_status": "preheat",
                        "total_supply": "1000000.000000000000000000",
                        "issue_price": "1.000000000000000000",
                        "quote_asset_id": 2,
                        "unlock_type": "immediate_on_listing"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(immediate.status(), StatusCode::BAD_REQUEST);
    let immediate_payload = body_json(immediate).await?;
    assert_eq!(
        immediate_payload["message"],
        "validation error: listed_at is required for immediate_on_listing unlock"
    );

    let admin = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/new-coins")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await?;
    assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let payload = body_json(admin).await?;
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for admin convert routes")
    );

    Ok(())
}

#[tokio::test]
async fn admin_new_coin_lifecycle_routes_require_admin_scope_and_mysql()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));
    let body = json!({ "lifecycle_status": "subscription" }).to_string();

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/new-coins/1/lifecycle")
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let user = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/new-coins/1/lifecycle")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(user.status(), StatusCode::FORBIDDEN);

    let invalid = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/new-coins/1/lifecycle")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "lifecycle_status": "archived" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let invalid_payload = body_json(invalid).await?;
    assert_eq!(invalid_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        invalid_payload["message"],
        "validation error: unsupported new coin lifecycle_status"
    );

    let admin = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/new-coins/1/lifecycle")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await?;
    assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let payload = body_json(admin).await?;
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for admin convert routes")
    );

    Ok(())
}

#[tokio::test]
async fn admin_new_coin_unlock_rule_routes_require_admin_scope_and_mysql()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));
    let body = json!({
        "unlock_type": "fixed_time",
        "fixed_unlock_at": 1794309753000_i64
    })
    .to_string();

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/new-coins/1/unlock-rule")
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let user = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/new-coins/1/unlock-rule")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(user.status(), StatusCode::FORBIDDEN);

    let invalid = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/new-coins/1/unlock-rule")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "unlock_type": "fixed_time" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let invalid_payload = body_json(invalid).await?;
    assert_eq!(invalid_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        invalid_payload["message"],
        "validation error: fixed_unlock_at is required for fixed_time unlock"
    );

    let admin = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/new-coins/1/unlock-rule")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await?;
    assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let payload = body_json(admin).await?;
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for admin convert routes")
    );

    Ok(())
}

#[tokio::test]
async fn admin_new_coin_unlock_fee_rule_routes_require_admin_scope_and_mysql()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));
    let body = json!({
        "unlock_fee_enabled": true,
        "unlock_fee_rate": "0.04000000",
        "unlock_fee_basis": "market_value",
        "unlock_fee_asset": 1
    })
    .to_string();

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/new-coins/1/unlock-fee-rule")
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let user = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/new-coins/1/unlock-fee-rule")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(user.status(), StatusCode::FORBIDDEN);

    let invalid = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/new-coins/1/unlock-fee-rule")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "unlock_fee_enabled": true,
                        "unlock_fee_rate": "0.00000000",
                        "unlock_fee_basis": "market_value",
                        "unlock_fee_asset": 1
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let invalid_payload = body_json(invalid).await?;
    assert_eq!(invalid_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        invalid_payload["message"],
        "validation error: unlock_fee_rate must be positive when unlock fee is enabled"
    );

    let admin = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/new-coins/1/unlock-fee-rule")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await?;
    assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let payload = body_json(admin).await?;
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for admin convert routes")
    );

    Ok(())
}

#[tokio::test]
async fn admin_new_coin_rule_updates_modify_project_events_and_audits() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let asset_id = create_asset(&pool, "ANU").await;
    let quote_asset_id = create_asset(&pool, "ANV").await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let symbol = format!("ANU{}", Uuid::now_v7().simple()).to_ascii_uppercase();
    let fixed_unlock_at = chrono::Utc
        .with_ymd_and_hms(2026, 11, 10, 11, 22, 33)
        .unwrap();

    let listed_at = chrono::Utc.with_ymd_and_hms(2026, 10, 1, 8, 0, 0).unwrap();
    let project_id = sqlx::query(
        r#"INSERT INTO new_coin_projects
           (asset_id, symbol, lifecycle_status, total_supply, issue_price, quote_asset_id,
            listed_at, unlock_type, fixed_unlock_at, status, remaining_supply)
           VALUES (?, ?, 'listed', ?, ?, ?, ?, 'fixed_time', ?, 'active', 1000000)"#,
    )
    .bind(asset_id)
    .bind(&symbol)
    .bind(decimal("1000000.000000000000000000"))
    .bind(decimal("1.000000000000000000"))
    .bind(quote_asset_id)
    .bind(listed_at)
    .bind(fixed_unlock_at)
    .execute(&pool)
    .await?
    .last_insert_id();

    let invalid_unlock = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/new-coins/{project_id}/unlock-rule"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "unlock_type": "relative_period" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid_unlock.status(), StatusCode::BAD_REQUEST);
    let (unchanged_unlock_type,): (String,) =
        sqlx::query_as("SELECT unlock_type FROM new_coin_projects WHERE id = ?")
            .bind(project_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(unchanged_unlock_type, "fixed_time");

    let unlock_update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/new-coins/{project_id}/unlock-rule"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "unlock_type": "relative_period",
                        "relative_unlock_seconds": 259200,
                        "reason": "switch to per-order unlock"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let unlock_status = unlock_update.status();
    let unlock_payload = body_json(unlock_update).await?;
    assert_eq!(unlock_status, StatusCode::OK, "payload: {unlock_payload}");
    assert_eq!(unlock_payload["id"], project_id);
    assert_eq!(unlock_payload["unlock_type"], "relative_period");
    assert_eq!(unlock_payload["listed_at"], 1_790_841_600_000_i64);
    assert!(unlock_payload["fixed_unlock_at"].is_null());
    assert_eq!(unlock_payload["relative_unlock_seconds"], 259200);

    let (listed_at_after_unlock,): (chrono::DateTime<chrono::Utc>,) =
        sqlx::query_as("SELECT listed_at FROM new_coin_projects WHERE id = ?")
            .bind(project_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(listed_at_after_unlock, listed_at);

    let fee_update = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/admin/api/v1/new-coins/{project_id}/unlock-fee-rule"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "unlock_fee_enabled": true,
                        "unlock_fee_rate": "0.04000000",
                        "unlock_fee_basis": "profit",
                        "unlock_fee_asset": quote_asset_id,
                        "reason": "charge miner fee on profit"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let fee_status = fee_update.status();
    let fee_payload = body_json(fee_update).await?;
    assert_eq!(fee_status, StatusCode::OK, "payload: {fee_payload}");
    assert_eq!(fee_payload["unlock_fee_enabled"], true);
    assert_eq!(fee_payload["unlock_fee_rate"], "0.04000000");
    assert_eq!(fee_payload["unlock_fee_basis"], "profit");
    assert_eq!(fee_payload["unlock_fee_asset"], quote_asset_id);

    let events = sqlx::query_as::<_, (String, Option<u64>, Value)>(
        r#"SELECT event_type, created_by, payload_json
           FROM new_coin_lifecycle_events
           WHERE project_id = ?
           ORDER BY id"#,
    )
    .bind(project_id)
    .fetch_all(&pool)
    .await?;
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].0, "new_coin_project.unlock_rule.update");
    assert_eq!(events[0].1, Some(admin_id));
    assert_eq!(events[0].2["before"]["unlock_type"], "fixed_time");
    assert_eq!(events[0].2["after"]["unlock_type"], "relative_period");
    assert_eq!(events[1].0, "new_coin_project.unlock_fee_rule.update");
    assert_eq!(events[1].1, Some(admin_id));
    assert_eq!(events[1].2["before"]["unlock_fee_enabled"], false);
    assert_eq!(events[1].2["after"]["unlock_fee_basis"], "profit");

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
               FROM admin_audit_logs
               WHERE admin_id = ? AND target_type = 'new_coin_project' AND target_id = ?
               ORDER BY id"#,
    )
    .bind(admin_id)
    .bind(project_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 2);
    assert_eq!(audits[0].action, "new_coin_project.unlock_rule.update");
    assert_eq!(
        audits[0].before_json.as_ref().unwrap()["unlock_type"],
        "fixed_time"
    );
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["unlock_type"],
        "relative_period"
    );
    assert_eq!(
        audits[0].reason.as_deref(),
        Some("switch to per-order unlock")
    );
    assert_eq!(audits[1].action, "new_coin_project.unlock_fee_rule.update");
    assert_eq!(
        audits[1].after_json.as_ref().unwrap()["unlock_fee_basis"],
        "profit"
    );
    assert_eq!(
        audits[1].reason.as_deref(),
        Some("charge miner fee on profit")
    );

    delete_new_coin_project_fixture(&pool, project_id, asset_id, admin_id, role_id).await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(quote_asset_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn admin_new_coin_project_create_lists_events_and_audits() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let asset_id = create_asset(&pool, "ANP").await;
    let quote_asset_id = create_asset(&pool, "ANQ").await;
    sqlx::query(
        "UPDATE assets SET deposit_enabled = FALSE, withdraw_enabled = FALSE WHERE id IN (?, ?)",
    )
    .bind(asset_id)
    .bind(quote_asset_id)
    .execute(&pool)
    .await?;
    let asset_switches: Vec<(u64, String, bool, bool)> = sqlx::query_as(
        "SELECT id, status, deposit_enabled, withdraw_enabled FROM assets WHERE id IN (?, ?) ORDER BY id",
    )
    .bind(asset_id)
    .bind(quote_asset_id)
    .fetch_all(&pool)
    .await?;
    assert_eq!(asset_switches.len(), 2);
    assert!(
        asset_switches
            .iter()
            .all(|(_, status, deposit_enabled, withdraw_enabled)| {
                status == "active" && !deposit_enabled && !withdraw_enabled
            })
    );
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let symbol = format!("ANP{}", Uuid::now_v7().simple()).to_ascii_uppercase();
    let create_payload = json!({
        "asset_id": asset_id,
        "symbol": symbol,
        "lifecycle_status": "preheat",
        "total_supply": "1000000.000000000000000000",
        "issue_price": "1.000000000000000000",
        "quote_asset_id": quote_asset_id,
        "unlock_type": "fixed_time",
        "fixed_unlock_at": 1794309753000_i64,
        "unlock_fee_enabled": true,
        "unlock_fee_rate": "0.04000000",
        "unlock_fee_basis": "market_value",
        "unlock_fee_asset": quote_asset_id,
        "reason": "create new coin project"
    })
    .to_string();

    sqlx::query("UPDATE assets SET status = 'disabled' WHERE id = ?")
        .bind(quote_asset_id)
        .execute(&pool)
        .await?;
    let disabled_quote_asset = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/new-coins")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(create_payload.clone()))
                .unwrap(),
        )
        .await?;
    let disabled_status = disabled_quote_asset.status();
    let disabled_payload = body_json(disabled_quote_asset).await?;
    sqlx::query("UPDATE assets SET status = 'active' WHERE id = ?")
        .bind(quote_asset_id)
        .execute(&pool)
        .await?;
    assert_eq!(
        disabled_status,
        StatusCode::BAD_REQUEST,
        "payload: {disabled_payload}"
    );
    assert_eq!(disabled_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        disabled_payload["message"],
        "validation error: new coin asset must be active"
    );

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/new-coins")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(create_payload))
                .unwrap(),
        )
        .await?;
    let create_status = create.status();
    let created = body_json(create).await?;
    assert_eq!(create_status, StatusCode::OK, "payload: {created}");
    let project_id = created["id"].as_u64().unwrap();
    assert_eq!(created["asset_id"], asset_id);
    assert_eq!(created["symbol"], symbol);
    assert_eq!(created["lifecycle_status"], "preheat");
    assert_eq!(created["total_supply"], "1000000.000000000000000000");
    assert_eq!(created["issue_price"], "1.000000000000000000");
    assert_eq!(created["quote_asset_id"], quote_asset_id);
    assert_eq!(created["reserved_supply"], "0");
    assert_eq!(created["allocated_supply"], "0");
    assert_eq!(created["remaining_supply"], "1000000.000000000000000000");
    assert_eq!(created["unlock_type"], "fixed_time");
    assert_eq!(created["unlock_fee_enabled"], true);
    assert_eq!(created["unlock_fee_rate"], "0.04000000");
    assert_eq!(created["unlock_fee_basis"], "market_value");
    assert_eq!(created["unlock_fee_asset"], quote_asset_id);

    let listed = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/new-coins?limit=20")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let list_status = listed.status();
    let list_payload = body_json(listed).await?;
    assert_eq!(list_status, StatusCode::OK, "payload: {list_payload}");
    assert!(
        list_payload["projects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|project| project["id"] == project_id && project["symbol"] == symbol),
        "payload: {list_payload}"
    );

    let (event_type, event_admin_id): (String, Option<u64>) = sqlx::query_as(
        "SELECT event_type, created_by FROM new_coin_lifecycle_events WHERE project_id = ? LIMIT 1",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(event_type, "new_coin_project.create");
    assert_eq!(event_admin_id, Some(admin_id));

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
               FROM admin_audit_logs
               WHERE admin_id = ? AND target_type = 'new_coin_project' AND target_id = ?
               ORDER BY id"#,
    )
    .bind(admin_id)
    .bind(project_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].action, "new_coin_project.create");
    assert!(audits[0].before_json.is_none());
    assert_eq!(audits[0].after_json.as_ref().unwrap()["symbol"], symbol);
    assert_eq!(audits[0].reason.as_deref(), Some("create new coin project"));

    delete_new_coin_project_fixture(&pool, project_id, asset_id, admin_id, role_id).await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(quote_asset_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn admin_new_coin_distribution_routes_require_admin_scope_and_mysql()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));
    let body = json!({
        "user_id": 1,
        "quantity": "10.000000000000000000",
        "idempotency_key": "admin-dist-key-1", "reason": "manual distribution"
    })
    .to_string();

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/new-coins/1/distribute")
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let user = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/new-coins/1/distribute")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(user.status(), StatusCode::FORBIDDEN);

    let invalid = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/new-coins/1/distribute")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "user_id": 1,
                        "quantity": "0.000000000000000000",
                        "idempotency_key": "admin-dist-key-1", "reason": "manual distribution"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let invalid_payload = body_json(invalid).await?;
    assert_eq!(invalid_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        invalid_payload["message"],
        "validation error: quantity must be positive"
    );

    let admin = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/new-coins/1/distribute")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await?;
    assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let payload = body_json(admin).await?;
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for admin convert routes")
    );

    Ok(())
}

#[tokio::test]
async fn admin_new_coin_distribution_creates_wallet_lock_event_and_audit()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let user_id = create_user(&pool).await;
    let asset_id = create_asset(&pool, "AND").await;
    let quote_asset_id = create_asset(&pool, "ANE").await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let symbol = format!("AND{}", Uuid::now_v7().simple()).to_ascii_uppercase();
    let unlock_at = chrono::Utc
        .with_ymd_and_hms(2026, 11, 10, 11, 22, 33)
        .unwrap();

    let project_id = sqlx::query(
        r#"INSERT INTO new_coin_projects
           (asset_id, symbol, lifecycle_status, total_supply, issue_price, quote_asset_id,
            remaining_supply, unlock_type, fixed_unlock_at, unlock_fee_enabled, unlock_fee_rate,
            unlock_fee_basis, unlock_fee_asset, status)
           VALUES (?, ?, 'preheat', ?, ?, ?, ?, 'fixed_time', ?, TRUE, 0.1,
                   'market_value', ?, 'active')"#,
    )
    .bind(asset_id)
    .bind(&symbol)
    .bind(decimal("1000000.000000000000000000"))
    .bind(decimal("1.000000000000000000"))
    .bind(quote_asset_id)
    .bind(decimal("1000000.000000000000000000"))
    .bind(unlock_at)
    .bind(quote_asset_id)
    .execute(&pool)
    .await?
    .last_insert_id();

    let invalid_lifecycle = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/api/v1/new-coins/{project_id}/distribute"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "user_id": user_id,
                        "quantity": "10.000000000000000000",
                        "idempotency_key": "admin-dist-invalid-lifecycle"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid_lifecycle.status(), StatusCode::BAD_REQUEST);

    let (distribution_count_before,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM new_coin_distributions WHERE project_id = ?")
            .bind(project_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(distribution_count_before, 0);

    sqlx::query("UPDATE new_coin_projects SET lifecycle_status = 'distribution' WHERE id = ?")
        .bind(project_id)
        .execute(&pool)
        .await?;

    let idempotency_key = format!("admin-dist-{}", Uuid::now_v7());
    let distribute = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/api/v1/new-coins/{project_id}/distribute"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "user_id": user_id,
                        "quantity": "10.000000000000000000",
                        "idempotency_key": idempotency_key,
                        "reason": "manual distribution"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let distribute_status = distribute.status();
    let distributed = body_json(distribute).await?;
    assert_eq!(distribute_status, StatusCode::OK, "payload: {distributed}");
    let distribution_id = distributed["id"].as_u64().unwrap();
    let lock_position_id = distributed["lock_position_id"].as_u64().unwrap();
    assert_eq!(distributed["project_id"], project_id);
    assert_eq!(distributed["user_id"], user_id);
    assert_eq!(distributed["asset_id"], asset_id);
    assert_eq!(distributed["quantity"], "10.000000000000000000");
    assert_eq!(distributed["status"], "locked");
    assert_eq!(distributed["idempotency_key"], idempotency_key);

    let (available, locked): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, locked FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(available, decimal("0.000000000000000000"));
    assert_eq!(locked, decimal("10.000000000000000000"));

    let (position_id, locked_amount, remaining_amount, merge_key): (
        u64,
        BigDecimal,
        BigDecimal,
        String,
    ) = sqlx::query_as(
        r#"SELECT id, locked_amount, remaining_amount, merge_key
           FROM asset_lock_positions
           WHERE user_id = ? AND asset_id = ?"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(position_id, lock_position_id);
    assert_eq!(locked_amount, decimal("10.000000000000000000"));
    assert_eq!(remaining_amount, decimal("10.000000000000000000"));
    assert!(merge_key.contains("fixed_time"));

    let (source_type, source_id, source_amount): (String, String, BigDecimal) = sqlx::query_as(
        r#"SELECT source_type, source_id, source_amount
           FROM asset_lock_position_sources
           WHERE lock_position_id = ?"#,
    )
    .bind(lock_position_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(source_type, "new_coin_distribution");
    assert_eq!(source_id, idempotency_key);
    assert_eq!(source_amount, decimal("10.000000000000000000"));

    sqlx::query("UPDATE new_coin_projects SET unlock_fee_rate = 0.9 WHERE id = ?")
        .bind(project_id)
        .execute(&pool)
        .await?;
    let (
        unlock_quantity,
        unlock_price,
        fee_enabled,
        fee_asset,
        fee_amount,
        fee_status,
        unlock_status,
    ): (
        BigDecimal,
        BigDecimal,
        bool,
        Option<u64>,
        Option<BigDecimal>,
        String,
        String,
    ) = sqlx::query_as(
        r#"SELECT unlock_quantity, unlock_price, unlock_fee_enabled, unlock_fee_asset,
                  unlock_fee_amount, fee_paid_status, status
           FROM asset_unlock_records
           WHERE idempotency_key = ? AND lock_position_id = ?"#,
    )
    .bind(format!("new_coin_distribution:{idempotency_key}"))
    .bind(lock_position_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(unlock_quantity, decimal("10.000000000000000000"));
    assert_eq!(unlock_price, decimal("1.000000000000000000"));
    assert!(fee_enabled);
    assert_eq!(fee_asset, Some(quote_asset_id));
    assert_eq!(fee_amount, Some(decimal("1.000000000000000000")));
    assert_eq!(fee_status, "pending");
    assert_eq!(unlock_status, "pending");

    let ledger = sqlx::query_as::<_, (String, String, String, BigDecimal, String)>(
        r#"SELECT change_type, balance_type, ref_id, amount, ref_type
           FROM wallet_ledger
           WHERE user_id = ? AND asset_id = ?"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger.0, "new_coin_distribution_lock");
    assert_eq!(ledger.1, "locked");
    assert_eq!(ledger.2, idempotency_key);
    assert_eq!(ledger.3, decimal("10.000000000000000000"));
    assert_eq!(ledger.4, "new_coin_distribution");

    let (reserved_supply, allocated_supply, remaining_supply): (
        BigDecimal,
        BigDecimal,
        BigDecimal,
    ) = sqlx::query_as(
        "SELECT reserved_supply, allocated_supply, remaining_supply FROM new_coin_projects WHERE id = ?",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(reserved_supply, decimal("0.000000000000000000"));
    assert_eq!(allocated_supply, decimal("10.000000000000000000"));
    assert_eq!(remaining_supply, decimal("999990.000000000000000000"));

    let (event_type, event_admin_id, event_payload): (String, Option<u64>, Value) = sqlx::query_as(
        r#"SELECT event_type, created_by, payload_json
               FROM new_coin_lifecycle_events
               WHERE project_id = ?
               ORDER BY id DESC
               LIMIT 1"#,
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(event_type, "new_coin_distribution.create");
    assert_eq!(event_admin_id, Some(admin_id));
    assert_eq!(event_payload["distribution"]["id"], distribution_id);
    assert_eq!(
        event_payload["distribution"]["lock_position_id"],
        lock_position_id
    );

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
               FROM admin_audit_logs
               WHERE admin_id = ? AND target_type = 'new_coin_distribution' AND target_id = ?
               ORDER BY id"#,
    )
    .bind(admin_id)
    .bind(distribution_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].action, "new_coin_distribution.create");
    assert!(audits[0].before_json.is_none());
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["id"],
        distribution_id
    );
    assert_eq!(audits[0].reason.as_deref(), Some("manual distribution"));

    let duplicate = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/api/v1/new-coins/{project_id}/distribute"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "user_id": user_id,
                        "quantity": "10.000000000000000000",
                        "idempotency_key": idempotency_key, "reason": "manual distribution"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(duplicate.status(), StatusCode::OK);
    assert_eq!(body_json(duplicate).await?, distributed);

    let duplicate_with_spaces = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/api/v1/new-coins/{project_id}/distribute"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "user_id": user_id,
                        "quantity": "10.000000000000000000",
                        "idempotency_key": format!("  {idempotency_key}  "), "reason": "manual distribution"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(duplicate_with_spaces.status(), StatusCode::OK);
    assert_eq!(body_json(duplicate_with_spaces).await?, distributed);

    delete_new_coin_distribution_fixture(&pool, project_id, asset_id, user_id, admin_id, role_id)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(quote_asset_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn admin_new_coin_lifecycle_transition_updates_project_events_and_audits()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let asset_id = create_asset(&pool, "ALP").await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let symbol = format!("ALP{}", Uuid::now_v7().simple()).to_ascii_uppercase();

    let project_id = sqlx::query(
        r#"INSERT INTO new_coin_projects
           (asset_id, symbol, lifecycle_status, total_supply, issue_price, unlock_type, status, remaining_supply)
           VALUES (?, ?, 'preheat', ?, ?, 'immediate_on_listing', 'active', 1000000)"#,
    )
    .bind(asset_id)
    .bind(&symbol)
    .bind(decimal("1000000.000000000000000000"))
    .bind(decimal("1.000000000000000000"))
    .execute(&pool)
    .await?
    .last_insert_id();

    let invalid = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/new-coins/{project_id}/lifecycle"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "lifecycle_status": "listed", "reason": "skip ahead" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let invalid_payload = body_json(invalid).await?;
    assert_eq!(invalid_payload["code"], "VALIDATION_ERROR");

    let (unchanged,): (String,) =
        sqlx::query_as("SELECT lifecycle_status FROM new_coin_projects WHERE id = ?")
            .bind(project_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(unchanged, "preheat");

    let subscription = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/new-coins/{project_id}/lifecycle"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "lifecycle_status": "subscription",
                        "reason": "open subscription"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let subscription_status = subscription.status();
    let subscription_payload = body_json(subscription).await?;
    assert_eq!(
        subscription_status,
        StatusCode::OK,
        "payload: {subscription_payload}"
    );
    assert_eq!(subscription_payload["id"], project_id);
    assert_eq!(subscription_payload["lifecycle_status"], "subscription");
    assert!(subscription_payload["listed_at"].is_null());

    let distribution = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/new-coins/{project_id}/lifecycle"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "lifecycle_status": "distribution" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(distribution.status(), StatusCode::OK);

    let listing_started_at = chrono::Utc::now().timestamp_millis();
    let listed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/new-coins/{project_id}/lifecycle"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "lifecycle_status": "listed",
                        "reason": "list project"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let listed_status = listed.status();
    let listed_payload = body_json(listed).await?;
    assert_eq!(listed_status, StatusCode::OK, "payload: {listed_payload}");
    assert_eq!(listed_payload["lifecycle_status"], "listed");
    assert!(listed_payload["listed_at"].is_null());
    assert!(listed_payload["actual_listed_at"].as_i64().unwrap() >= listing_started_at);

    let backward = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/new-coins/{project_id}/lifecycle"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "lifecycle_status": "distribution" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(backward.status(), StatusCode::BAD_REQUEST);

    let (current_status,): (String,) =
        sqlx::query_as("SELECT lifecycle_status FROM new_coin_projects WHERE id = ?")
            .bind(project_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(current_status, "listed");

    let events = sqlx::query_as::<_, (String, Option<u64>, Value)>(
        r#"SELECT event_type, created_by, payload_json
           FROM new_coin_lifecycle_events
           WHERE project_id = ?
           ORDER BY id"#,
    )
    .bind(project_id)
    .fetch_all(&pool)
    .await?;
    assert_eq!(events.len(), 3);
    assert!(
        events
            .iter()
            .all(|event| event.0 == "new_coin_project.lifecycle.update")
    );
    assert!(events.iter().all(|event| event.1 == Some(admin_id)));
    assert_eq!(events[0].2["before"]["lifecycle_status"], "preheat");
    assert_eq!(events[0].2["after"]["lifecycle_status"], "subscription");
    assert_eq!(events[2].2["before"]["lifecycle_status"], "distribution");
    assert_eq!(events[2].2["after"]["lifecycle_status"], "listed");

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
               FROM admin_audit_logs
               WHERE admin_id = ? AND target_type = 'new_coin_project' AND target_id = ?
               ORDER BY id"#,
    )
    .bind(admin_id)
    .bind(project_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 3);
    assert!(
        audits
            .iter()
            .all(|audit| audit.action == "new_coin_project.lifecycle.update")
    );
    assert_eq!(
        audits[0].before_json.as_ref().unwrap()["lifecycle_status"],
        "preheat"
    );
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["lifecycle_status"],
        "subscription"
    );
    assert_eq!(audits[0].reason.as_deref(), Some("open subscription"));
    assert_eq!(
        audits[2].before_json.as_ref().unwrap()["lifecycle_status"],
        "distribution"
    );
    assert_eq!(
        audits[2].after_json.as_ref().unwrap()["lifecycle_status"],
        "listed"
    );
    assert_eq!(audits[2].reason.as_deref(), Some("list project"));

    delete_new_coin_project_fixture(&pool, project_id, asset_id, admin_id, role_id).await?;
    Ok(())
}

#[tokio::test]
async fn admin_margin_liquidation_routes_require_admin_scope_and_mysql()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));

    for uri in [
        "/admin/api/v1/margin/liquidations",
        "/admin/api/v1/margin/liquidations/1",
    ] {
        let missing = app
            .clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await?;
        assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

        let user = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .header(AUTHORIZATION, format!("Bearer {user_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await?;
        assert_eq!(user.status(), StatusCode::FORBIDDEN);

        let admin = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await?;
        assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let payload = body_json(admin).await?;
        assert_eq!(payload["code"], "INTERNAL_ERROR");
        assert!(
            payload["message"]
                .as_str()
                .unwrap()
                .contains("mysql pool is not configured for admin convert routes")
        );
    }

    let admin = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/margin/liquidations")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let payload = body_json(admin).await?;
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for admin convert routes")
    );

    Ok(())
}

#[tokio::test]
async fn admin_margin_liquidations_list_filters_seeded_records() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_email = format!(
        "admin-margin-liquidation-{}@example.test",
        Uuid::now_v7().simple()
    );
    let user_id = create_user_with_email(&pool, user_email.clone()).await;
    let other_user_id = create_user_without_email(&pool).await;
    let now = chrono::Utc
        .with_ymd_and_hms(2026, 5, 29, 16, 30, 45)
        .unwrap();
    let target = seed_margin_liquidation_record(&pool, user_id, "AML", now).await;
    let other = seed_margin_liquidation_record(
        &pool,
        other_user_id,
        "AMO",
        now + chrono::TimeDelta::seconds(1),
    )
    .await;
    let token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let filtered = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/margin/liquidations?email={user_email}&limit=10"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let filtered_status = filtered.status();
    let filtered_payload = body_json(filtered).await?;
    assert_eq!(
        filtered_status,
        StatusCode::OK,
        "payload: {filtered_payload}"
    );
    assert_eq!(filtered_payload["total"], 1);
    let liquidations = filtered_payload["liquidations"].as_array().unwrap();
    assert_eq!(liquidations.len(), 1);
    assert_eq!(liquidations[0]["id"], target.record_id);
    assert_eq!(liquidations[0]["position_id"], target.position_id);
    assert_eq!(liquidations[0]["user_id"], user_id);
    assert_eq!(liquidations[0]["email"], user_email);
    assert_eq!(liquidations[0]["product_id"], target.product_id);
    assert_eq!(liquidations[0]["pair_id"], target.pair_id);
    assert_eq!(liquidations[0]["symbol"], target.symbol);
    assert_eq!(liquidations[0]["margin_asset"], target.margin_asset);
    assert_eq!(liquidations[0]["direction"], "long");
    assert_eq!(liquidations[0]["margin_amount"], "20.000000000000000000");
    assert_eq!(liquidations[0]["notional_amount"], "100.000000000000000000");
    assert_eq!(liquidations[0]["interest_amount"], "1.250000000000000000");
    assert_eq!(liquidations[0]["entry_price"], "100.000000000000000000");
    assert_eq!(liquidations[0]["mark_price"], "84.000000000000000000");
    assert_eq!(liquidations[0]["maintenance_margin_rate"], "0.05000000");
    assert_eq!(liquidations[0]["equity"], "2.750000000000000000");
    assert_eq!(
        liquidations[0]["maintenance_margin"],
        "5.000000000000000000"
    );
    assert_eq!(liquidations[0]["realized_pnl"], "-16.000000000000000000");
    assert_eq!(liquidations[0]["payout_amount"], "2.750000000000000000");
    assert_eq!(liquidations[0]["reason"], "maintenance_margin");
    assert_eq!(liquidations[0]["liquidated_at"], now.timestamp_millis());
    assert!(liquidations[0]["created_at"].as_i64().is_some());

    let detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/margin/liquidations/{}",
                    target.record_id
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let detail_status = detail.status();
    let detail_payload = body_json(detail).await?;
    assert_eq!(detail_status, StatusCode::OK, "payload: {detail_payload}");
    assert_eq!(detail_payload["id"], target.record_id);
    assert_eq!(detail_payload["position_id"], target.position_id);
    assert_eq!(detail_payload["user_id"], user_id);
    assert_eq!(detail_payload["email"], user_email);
    assert_eq!(detail_payload["product_id"], target.product_id);
    assert_eq!(detail_payload["pair_id"], target.pair_id);
    assert_eq!(detail_payload["symbol"], target.symbol);
    assert_eq!(detail_payload["margin_asset"], target.margin_asset);
    assert_eq!(detail_payload["direction"], "long");
    assert_eq!(detail_payload["margin_amount"], "20.000000000000000000");
    assert_eq!(detail_payload["notional_amount"], "100.000000000000000000");
    assert_eq!(detail_payload["interest_amount"], "1.250000000000000000");
    assert_eq!(detail_payload["entry_price"], "100.000000000000000000");
    assert_eq!(detail_payload["mark_price"], "84.000000000000000000");
    assert_eq!(detail_payload["maintenance_margin_rate"], "0.05000000");
    assert_eq!(detail_payload["equity"], "2.750000000000000000");
    assert_eq!(detail_payload["maintenance_margin"], "5.000000000000000000");
    assert_eq!(detail_payload["realized_pnl"], "-16.000000000000000000");
    assert_eq!(detail_payload["payout_amount"], "2.750000000000000000");
    assert_eq!(detail_payload["reason"], "maintenance_margin");
    assert_eq!(detail_payload["liquidated_at"], now.timestamp_millis());
    assert!(detail_payload["created_at"].as_i64().is_some());
    assert_eq!(detail_payload, liquidations[0]);

    for filter in [
        format!("user_id={user_id}"),
        format!("email={user_email}"),
        format!("pair_id={}", target.pair_id),
        format!("position_id={}", target.position_id),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/admin/api/v1/margin/liquidations?{filter}&limit=10"
                    ))
                    .header(AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await?;
        let status = response.status();
        let payload = body_json(response).await?;
        assert_eq!(status, StatusCode::OK, "filter {filter}: {payload}");
        assert_eq!(payload["total"], 1, "filter {filter}: {payload}");
        let rows = payload["liquidations"].as_array().unwrap();
        assert_eq!(rows.len(), 1, "filter {filter}: {payload}");
        assert_eq!(
            rows[0]["id"], target.record_id,
            "filter {filter}: {payload}"
        );
    }

    let unknown_detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/margin/liquidations/999999999999")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(unknown_detail.status(), StatusCode::NOT_FOUND);

    assert!(other.record_id > target.record_id);
    let first_page = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/margin/liquidations?limit=1&offset=0")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let first_page_status = first_page.status();
    let first_page_payload = body_json(first_page).await?;
    assert_eq!(
        first_page_status,
        StatusCode::OK,
        "payload: {first_page_payload}"
    );
    assert_eq!(first_page_payload["total"], 2);
    let first_page_liquidations = first_page_payload["liquidations"].as_array().unwrap();
    assert_eq!(first_page_liquidations.len(), 1);
    let other_liquidation = &first_page_liquidations[0];
    assert_eq!(other_liquidation["id"], other.record_id);
    assert!(other_liquidation["email"].is_null());
    assert_eq!(other_liquidation["symbol"], other.symbol);

    let second_page = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/margin/liquidations?limit=1&offset=1")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let second_page_status = second_page.status();
    let second_page_payload = body_json(second_page).await?;
    assert_eq!(
        second_page_status,
        StatusCode::OK,
        "payload: {second_page_payload}"
    );
    assert_eq!(second_page_payload["total"], 2);
    let second_page_liquidations = second_page_payload["liquidations"].as_array().unwrap();
    assert_eq!(second_page_liquidations.len(), 1);
    assert_eq!(second_page_liquidations[0]["id"], target.record_id);
    assert_eq!(second_page_liquidations[0]["email"], user_email);
    assert_eq!(second_page_liquidations[0]["symbol"], target.symbol);

    let other_detail = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/margin/liquidations/{}",
                    other.record_id
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let other_detail_status = other_detail.status();
    let other_detail_payload = body_json(other_detail).await?;
    assert_eq!(
        other_detail_status,
        StatusCode::OK,
        "payload: {other_detail_payload}"
    );
    assert_eq!(other_detail_payload, *other_liquidation);

    delete_margin_liquidation_fixture(&pool, &[target, other], &[user_id, other_user_id]).await?;
    Ok(())
}

#[tokio::test]
async fn admin_convert_pair_routes_require_admin_scope_and_mysql() -> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/convert/pairs")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let user = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/convert/pairs")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(user.status(), StatusCode::FORBIDDEN);

    let admin = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/convert/pairs")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let payload = body_json(admin).await?;
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for admin convert routes")
    );

    Ok(())
}

#[tokio::test]
async fn admin_convert_detail_routes_require_admin_scope_mysql_and_reason()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));

    let pair_missing = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/convert/pairs/1")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(pair_missing.status(), StatusCode::UNAUTHORIZED);

    let pair_user = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/convert/pairs/1")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(pair_user.status(), StatusCode::FORBIDDEN);

    let pair_admin = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/convert/pairs/1")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(pair_admin.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let delete_pair_missing = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/admin/api/v1/convert/pairs/1")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(delete_pair_missing.status(), StatusCode::UNAUTHORIZED);

    let delete_pair_user = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/admin/api/v1/convert/pairs/1")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(delete_pair_user.status(), StatusCode::FORBIDDEN);

    let delete_pair_admin = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/admin/api/v1/convert/pairs/1")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "reason": "delete pair" }).to_string()))
                .unwrap(),
        )
        .await?;
    assert_eq!(
        delete_pair_admin.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );

    let order_admin = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/convert/orders/1")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(order_admin.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let blank_create_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/convert/pairs")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "from_asset_id": 1,
                        "to_asset_id": 2,
                        "pricing_mode": "fixed",
                        "spread_rate": "0.01000000",
                        "min_amount": "1.000000000000000000",
                        "enabled": true,
                        "reason": "   "
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(blank_create_reason.status(), StatusCode::BAD_REQUEST);
    let blank_create_payload = body_json(blank_create_reason).await?;
    assert_eq!(
        blank_create_payload["message"],
        "validation error: reason is required"
    );

    let blank_update_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/convert/pairs/1")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "enabled": false, "reason": "   " }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(blank_update_reason.status(), StatusCode::BAD_REQUEST);
    let blank_update_payload = body_json(blank_update_reason).await?;
    assert_eq!(
        blank_update_payload["message"],
        "validation error: reason is required"
    );

    let blank_delete_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/admin/api/v1/convert/pairs/1")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "reason": "   " }).to_string()))
                .unwrap(),
        )
        .await?;
    assert_eq!(blank_delete_reason.status(), StatusCode::BAD_REQUEST);
    let blank_delete_payload = body_json(blank_delete_reason).await?;
    assert_eq!(
        blank_delete_payload["message"],
        "validation error: reason is required"
    );

    let long_reason = "R".repeat(513);
    let long_create_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/convert/pairs")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "from_asset_id": 1,
                        "to_asset_id": 2,
                        "pricing_mode": "fixed",
                        "spread_rate": "0.01000000",
                        "min_amount": "1.000000000000000000",
                        "enabled": true,
                        "reason": long_reason
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(long_create_reason.status(), StatusCode::BAD_REQUEST);
    let long_create_payload = body_json(long_create_reason).await?;
    assert_eq!(
        long_create_payload["message"],
        "validation error: reason is too long"
    );

    let long_update_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/admin/api/v1/convert/pairs/1")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "enabled": false, "reason": "R".repeat(513) }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(long_update_reason.status(), StatusCode::BAD_REQUEST);
    let long_update_payload = body_json(long_update_reason).await?;
    assert_eq!(
        long_update_payload["message"],
        "validation error: reason is too long"
    );

    let long_delete_reason = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/admin/api/v1/convert/pairs/1")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "reason": "R".repeat(513) }).to_string()))
                .unwrap(),
        )
        .await?;
    assert_eq!(long_delete_reason.status(), StatusCode::BAD_REQUEST);
    let long_delete_payload = body_json(long_delete_reason).await?;
    assert_eq!(
        long_delete_payload["message"],
        "validation error: reason is too long"
    );

    Ok(())
}

#[tokio::test]
async fn admin_convert_new_coin_rule_routes_require_admin_scope_and_mysql()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));

    let body = json!({
        "convert_pair_id": 1,
        "rate_source": "fixed",
        "fixed_rate": "2.000000000000000000"
    })
    .to_string();

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/convert/new-coin-rules")
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let user = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/convert/new-coin-rules")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await?;
    assert_eq!(user.status(), StatusCode::FORBIDDEN);

    let invalid = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/convert/new-coin-rules")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "convert_pair_id": 1,
                        "rate_source": "floating",
                        "fixed_rate": "2.000000000000000000"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let invalid_payload = body_json(invalid).await?;
    assert_eq!(invalid_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        invalid_payload["message"],
        "validation error: only fixed rate_source is supported for new coin convert rules"
    );

    let admin = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/convert/new-coin-rules")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await?;
    assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let payload = body_json(admin).await?;
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for admin convert routes")
    );

    Ok(())
}

#[tokio::test]
async fn admin_convert_new_coin_rule_create_updates_existing_and_audits()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let from_asset = create_asset(&pool, "ANF").await;
    let to_asset = create_asset(&pool, "ANT").await;
    let pair_id = seed_convert_pair(&pool, from_asset, to_asset, true).await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/convert/new-coin-rules")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "convert_pair_id": pair_id,
                        "rate_source": "fixed",
                        "fixed_rate": "2.000000000000000000",
                        "status": "active",
                        "reason": "create rule"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let create_status = create.status();
    let created = body_json(create).await?;
    assert_eq!(create_status, StatusCode::OK, "payload: {created}");
    let rule_id = created["id"].as_u64().unwrap();
    assert_eq!(created["convert_pair_id"], pair_id);
    assert_eq!(created["rate_source"], "fixed");
    assert_eq!(created["fixed_rate"], "2.000000000000000000");
    assert_eq!(created["status"], "active");
    assert_eq!(created["created_by"], admin_id);

    let update = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/convert/new-coin-rules")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "convert_pair_id": pair_id,
                        "rate_source": "fixed",
                        "fixed_rate": "3.000000000000000000",
                        "status": "paused",
                        "reason": "pause rule"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let update_status = update.status();
    let updated = body_json(update).await?;
    assert_eq!(update_status, StatusCode::OK, "payload: {updated}");
    assert_eq!(updated["id"], rule_id);
    assert_eq!(updated["fixed_rate"], "3.000000000000000000");
    assert_eq!(updated["status"], "paused");

    let (rule_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM new_coin_convert_rules WHERE convert_pair_id = ?")
            .bind(pair_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(rule_count, 1);

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
               FROM admin_audit_logs
               WHERE admin_id = ? AND target_type = 'new_coin_convert_rule' AND target_id = ?
               ORDER BY id"#,
    )
    .bind(admin_id)
    .bind(rule_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 2);
    assert_eq!(audits[0].action, "new_coin_convert_rule.create");
    assert!(audits[0].before_json.is_none());
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["fixed_rate"],
        "2.000000000000000000"
    );
    assert_eq!(audits[0].reason.as_deref(), Some("create rule"));
    assert_eq!(audits[1].action, "new_coin_convert_rule.update");
    assert_eq!(
        audits[1].before_json.as_ref().unwrap()["fixed_rate"],
        "2.000000000000000000"
    );
    assert_eq!(
        audits[1].after_json.as_ref().unwrap()["fixed_rate"],
        "3.000000000000000000"
    );
    assert_eq!(audits[1].reason.as_deref(), Some("pause rule"));

    delete_rule_fixture(&pool, pair_id, from_asset, to_asset, admin_id, role_id).await?;
    Ok(())
}

#[tokio::test]
async fn admin_audit_log_routes_require_admin_scope_and_mysql() -> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/audit-logs")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let user = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/audit-logs")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(user.status(), StatusCode::FORBIDDEN);

    let admin = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/audit-logs")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let payload = body_json(admin).await?;
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for admin convert routes")
    );

    Ok(())
}

#[tokio::test]
async fn admin_audit_logs_list_filters_and_timestamps() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let (other_role_id, other_admin_id) = create_admin_user(&pool).await;
    let suffix = Uuid::now_v7().simple().to_string();
    let target_id = format!("audit-list-{suffix}");
    let other_target_id = format!("audit-list-other-{suffix}");
    let first_action = format!("audit.list.first.{}", &suffix[..12]);
    let second_action = format!("audit.list.second.{}", &suffix[..12]);
    let other_action = format!("audit.list.other.{}", &suffix[..12]);
    let first_created_at = chrono::Utc.with_ymd_and_hms(2026, 5, 30, 10, 0, 0).unwrap();
    // 数据库存到微秒，但审计 API 的筛选/响应合同为毫秒；上界应覆盖该毫秒内的全部微秒。
    let second_created_at = chrono::Utc.with_ymd_and_hms(2026, 5, 30, 10, 5, 0).unwrap()
        + chrono::Duration::microseconds(500);
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let first_audit_id = sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, before_json, after_json, reason, ip, created_at)
           VALUES (?, ?, 'audit_list_target', ?, JSON_OBJECT('enabled', false), JSON_OBJECT('enabled', true), 'first reason', '127.0.0.1', ?)"#,
    )
    .bind(admin_id)
    .bind(&first_action)
    .bind(&target_id)
    .bind(first_created_at.naive_utc())
    .execute(&pool)
    .await?
    .last_insert_id();
    let second_audit_id = sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, before_json, after_json, reason, ip, created_at)
           VALUES (?, ?, 'audit_list_target', ?, JSON_OBJECT('enabled', true), JSON_OBJECT('enabled', false), 'second reason', '127.0.0.2', ?)"#,
    )
    .bind(admin_id)
    .bind(&second_action)
    .bind(&target_id)
    .bind(second_created_at.naive_utc())
    .execute(&pool)
    .await?
    .last_insert_id();
    let other_audit_id = sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, after_json, reason)
           VALUES (?, ?, 'audit_list_target', ?, JSON_OBJECT('ignored', true), 'other reason')"#,
    )
    .bind(other_admin_id)
    .bind(&other_action)
    .bind(&other_target_id)
    .execute(&pool)
    .await?
    .last_insert_id();

    let filtered = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/audit-logs?admin_id={admin_id}&target_type=audit_list_target&target_id={target_id}&limit=10"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let filtered_status = filtered.status();
    let filtered_payload = body_json(filtered).await?;
    assert_eq!(
        filtered_status,
        StatusCode::OK,
        "payload: {filtered_payload}"
    );
    let logs = filtered_payload["logs"].as_array().unwrap();
    assert_eq!(logs.len(), 2);
    assert_eq!(logs[0]["id"], second_audit_id);
    assert_eq!(logs[0]["admin_id"], admin_id);
    assert_eq!(logs[0]["action"], second_action);
    assert_eq!(logs[0]["target_type"], "audit_list_target");
    assert_eq!(logs[0]["target_id"], target_id);
    assert_eq!(logs[0]["before_json"]["enabled"], true);
    assert_eq!(logs[0]["after_json"]["enabled"], false);
    assert_eq!(logs[0]["reason"], "second reason");
    assert_eq!(logs[0]["ip"], "127.0.0.2");
    assert_eq!(logs[0]["created_at"], second_created_at.timestamp_millis());
    assert_eq!(logs[1]["id"], first_audit_id);
    assert_eq!(logs[1]["created_at"], first_created_at.timestamp_millis());

    let by_time_range = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/audit-logs?target_type=audit_list_target&target_id={target_id}&created_from={}&created_to={}&limit=10",
                    second_created_at.timestamp_millis(),
                    second_created_at.timestamp_millis()
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let by_time_range_status = by_time_range.status();
    let by_time_range_payload = body_json(by_time_range).await?;
    assert_eq!(
        by_time_range_status,
        StatusCode::OK,
        "payload: {by_time_range_payload}"
    );
    let time_range_logs = by_time_range_payload["logs"].as_array().unwrap();
    assert_eq!(time_range_logs.len(), 1);
    assert_eq!(time_range_logs[0]["id"], second_audit_id);
    assert_eq!(by_time_range_payload["total"], 1);

    let invalid_time_range = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/audit-logs?created_from={}&created_to={}",
                    second_created_at.timestamp_millis(),
                    first_created_at.timestamp_millis()
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let invalid_time_range_status = invalid_time_range.status();
    let invalid_time_range_payload = body_json(invalid_time_range).await?;
    assert_eq!(invalid_time_range_status, StatusCode::BAD_REQUEST);
    assert_eq!(invalid_time_range_payload["code"], "VALIDATION_ERROR");

    let by_action = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/audit-logs?action={other_action}&limit=10"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let by_action_status = by_action.status();
    let by_action_payload = body_json(by_action).await?;
    assert_eq!(
        by_action_status,
        StatusCode::OK,
        "payload: {by_action_payload}"
    );
    let action_logs = by_action_payload["logs"].as_array().unwrap();
    assert_eq!(action_logs.len(), 1);
    assert_eq!(action_logs[0]["id"], other_audit_id);
    assert_eq!(action_logs[0]["admin_id"], other_admin_id);

    sqlx::query("DELETE FROM admin_audit_logs WHERE id IN (?, ?, ?)")
        .bind(first_audit_id)
        .bind(second_audit_id)
        .bind(other_audit_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_users WHERE id IN (?, ?)")
        .bind(admin_id)
        .bind(other_admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id IN (?, ?)")
        .bind(role_id)
        .bind(other_role_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn admin_convert_order_routes_require_admin_scope_and_mysql() -> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));

    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/convert/orders")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let user = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/convert/orders")
                .header(AUTHORIZATION, format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(user.status(), StatusCode::FORBIDDEN);

    let admin = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/convert/orders")
                .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(admin.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let payload = body_json(admin).await?;
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for admin convert routes")
    );

    Ok(())
}

#[tokio::test]
async fn admin_convert_orders_list_filters_by_user_and_status() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_email = format!(
        "admin-convert-filter-{}@example.test",
        Uuid::now_v7().simple()
    );
    let other_user_email = format!(
        "admin-convert-other-{}@example.test",
        Uuid::now_v7().simple()
    );
    let user_id = create_user_with_email(&pool, user_email.clone()).await;
    let other_user_id = create_user_with_email(&pool, other_user_email.clone()).await;
    let (from_asset, from_symbol) = create_asset_with_symbol(&pool, "AOF").await;
    let (to_asset, to_symbol) = create_asset_with_symbol(&pool, "AOT").await;
    let pair_id = seed_convert_pair(&pool, from_asset, to_asset, true).await;
    let pending_quote =
        seed_convert_order(&pool, user_id, pair_id, from_asset, to_asset, "pending").await;
    let _completed_quote =
        seed_convert_order(&pool, user_id, pair_id, from_asset, to_asset, "completed").await;
    let _other_quote = seed_convert_order(
        &pool,
        other_user_id,
        pair_id,
        from_asset,
        to_asset,
        "pending",
    )
    .await;
    let (pending_order_id,): (u64,) =
        sqlx::query_as("SELECT id FROM convert_orders WHERE quote_id = ?")
            .bind(&pending_quote)
            .fetch_one(&pool)
            .await?;
    let token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let filtered = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/convert/orders?email={user_email}&status=pending&limit=10"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let filtered_status = filtered.status();
    let filtered_payload = body_json(filtered).await?;
    assert_eq!(
        filtered_status,
        StatusCode::OK,
        "payload: {filtered_payload}"
    );
    let orders = filtered_payload["orders"].as_array().unwrap();
    assert_eq!(orders.len(), 1);
    assert!(orders[0].get("quote_id").is_none());
    assert!(orders[0].get("user_id").is_none());
    assert!(orders[0].get("convert_pair_id").is_none());
    assert!(orders[0].get("from_asset_id").is_none());
    assert!(orders[0].get("to_asset_id").is_none());
    assert_eq!(orders[0]["id"], pending_order_id);
    assert_eq!(orders[0]["user_email"], user_email);
    assert_eq!(orders[0]["from_asset_symbol"], from_symbol);
    assert_eq!(orders[0]["to_asset_symbol"], to_symbol);
    assert_eq!(orders[0]["from_amount"], "10.000000000000000000");
    assert_eq!(orders[0]["to_amount"], "20.000000000000000000");
    assert_eq!(orders[0]["rate"], "2.000000000000000000");
    assert_eq!(orders[0]["status"], "pending");

    let detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/api/v1/convert/orders/{pending_order_id}"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let detail_status = detail.status();
    let detail_payload = body_json(detail).await?;
    assert_eq!(detail_status, StatusCode::OK, "payload: {detail_payload}");
    assert!(detail_payload.get("quote_id").is_none());
    assert!(detail_payload.get("user_id").is_none());
    assert!(detail_payload.get("convert_pair_id").is_none());
    assert_eq!(detail_payload["user_email"], user_email);
    assert_eq!(detail_payload["from_asset_symbol"], from_symbol);
    assert_eq!(detail_payload["to_asset_symbol"], to_symbol);

    let all = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/convert/orders?limit=2")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let all_status = all.status();
    let all_payload = body_json(all).await?;
    assert_eq!(all_status, StatusCode::OK, "payload: {all_payload}");
    assert_eq!(all_payload["orders"].as_array().unwrap().len(), 2);
    assert!(
        all_payload["orders"]
            .as_array()
            .unwrap()
            .iter()
            .any(|order| order["user_email"] == other_user_email)
    );

    delete_order_fixture(
        &pool,
        pair_id,
        from_asset,
        to_asset,
        &[user_id, other_user_id],
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn admin_convert_pair_create_rolls_back_when_audit_cannot_be_written()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let missing_admin_id = 9_999_999_999_u64;
    let from_asset = create_asset(&pool, "ARF").await;
    let to_asset = create_asset(&pool, "ART").await;
    let token = issue_token(
        &settings,
        format!("admin:{missing_admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/convert/pairs")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "from_asset_id": from_asset,
                        "to_asset_id": to_asset,
                        "pricing_mode": "fixed",
                        "spread_rate": "0.01000000",
                        "fee_rate": "0.00100000",
                        "min_amount": "1.000000000000000000",
                        "enabled": true,
                        "reason": "create convert pair rollback"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let (pair_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM convert_pairs WHERE from_asset = ? AND to_asset = ?")
            .bind(from_asset)
            .bind(to_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(pair_count, 0);

    for asset_id in [from_asset, to_asset] {
        sqlx::query("DELETE FROM assets WHERE id = ?")
            .bind(asset_id)
            .execute(&pool)
            .await?;
    }
    Ok(())
}

#[tokio::test]
async fn admin_convert_pair_update_rolls_back_when_audit_cannot_be_written()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let missing_admin_id = 9_999_999_998_u64;
    let from_asset = create_asset(&pool, "AUF").await;
    let to_asset = create_asset(&pool, "AUT").await;
    let pair_id = seed_convert_pair(&pool, from_asset, to_asset, true).await;
    let token = issue_token(
        &settings,
        format!("admin:{missing_admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/convert/pairs/{pair_id}"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "enabled": false, "reason": "update convert pair rollback" })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let (enabled,): (bool,) = sqlx::query_as("SELECT enabled FROM convert_pairs WHERE id = ?")
        .bind(pair_id)
        .fetch_one(&pool)
        .await?;
    assert!(enabled);

    delete_pair_and_assets(&pool, pair_id, from_asset, to_asset).await?;
    Ok(())
}

#[tokio::test]
async fn admin_convert_pair_routes_create_list_update_and_audit() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let (from_asset, from_symbol) = create_asset_with_symbol(&pool, "ACF").await;
    let (to_asset, to_symbol) = create_asset_with_symbol(&pool, "ACT").await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/convert/pairs")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "from_asset_id": from_asset,
                        "to_asset_id": to_asset,
                        "pricing_mode": "fixed",
                        "spread_rate": "0.01000000",
                        "fee_rate": "0.00100000",
                        "min_amount": "1.000000000000000000",
                        "max_amount": "100.000000000000000000",
                        "target_min_amount": "10.000000000000000000",
                        "target_max_amount": "1000.000000000000000000",
                        "enabled": true,
                        "reason": "initial convert pair"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let create_status = create.status();
    let created = body_json(create).await?;
    assert_eq!(create_status, StatusCode::OK, "payload: {created}");
    let pair_id = created["id"].as_u64().unwrap();
    assert_eq!(created["from_asset_id"], from_asset);
    assert_eq!(created["from_asset_symbol"], from_symbol);
    assert_eq!(created["to_asset_id"], to_asset);
    assert_eq!(created["to_asset_symbol"], to_symbol);
    assert_eq!(created["pricing_mode"], "fixed");
    assert_eq!(created["spread_rate"], "0.01000000");
    assert_eq!(created["fee_rate"], "0.00100000");
    assert_eq!(created["min_amount"], "1.000000000000000000");
    assert_eq!(created["max_amount"], "100.000000000000000000");
    assert_eq!(created["target_min_amount"], "10.000000000000000000");
    assert_eq!(created["target_max_amount"], "1000.000000000000000000");
    assert_eq!(created["enabled"], true);

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/convert/pairs")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let list_status = list.status();
    let listed = body_json(list).await?;
    assert_eq!(list_status, StatusCode::OK, "payload: {listed}");
    assert!(listed["pairs"].as_array().unwrap().iter().any(|pair| {
        pair["id"] == pair_id
            && pair["from_asset_symbol"] == from_symbol
            && pair["to_asset_symbol"] == to_symbol
            && pair["fee_rate"] == "0.00100000"
            && pair["target_min_amount"] == "10.000000000000000000"
            && pair["target_max_amount"] == "1000.000000000000000000"
            && pair["enabled"] == true
    }));

    let edit = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/convert/pairs/{pair_id}"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "from_asset_id": from_asset,
                        "to_asset_id": to_asset,
                        "pricing_mode": "market",
                        "spread_rate": "0.02000000",
                        "fee_rate": "0.00300000",
                        "min_amount": "2.000000000000000000",
                        "max_amount": null,
                        "target_min_amount": "20.000000000000000000",
                        "target_max_amount": null,
                        "enabled": true,
                        "reason": "edit convert pair"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let edit_status = edit.status();
    let edited = body_json(edit).await?;
    assert_eq!(edit_status, StatusCode::OK, "payload: {edited}");
    assert_eq!(edited["id"], pair_id);
    assert_eq!(edited["pricing_mode"], "market");
    assert_eq!(edited["spread_rate"], "0.02000000");
    assert_eq!(edited["fee_rate"], "0.00300000");
    assert_eq!(edited["min_amount"], "2.000000000000000000");
    assert!(edited["max_amount"].is_null());
    assert_eq!(edited["target_min_amount"], "20.000000000000000000");
    assert!(edited["target_max_amount"].is_null());
    assert_eq!(edited["enabled"], true);

    let update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/convert/pairs/{pair_id}"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "enabled": false, "reason": "pause pair" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let update_status = update.status();
    let updated = body_json(update).await?;
    assert_eq!(update_status, StatusCode::OK, "payload: {updated}");
    assert_eq!(updated["id"], pair_id);
    assert_eq!(updated["from_asset_symbol"], from_symbol);
    assert_eq!(updated["to_asset_symbol"], to_symbol);
    assert_eq!(updated["enabled"], false);

    let (enabled,): (bool,) = sqlx::query_as("SELECT enabled FROM convert_pairs WHERE id = ?")
        .bind(pair_id)
        .fetch_one(&pool)
        .await?;
    assert!(!enabled);

    let delete = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/admin/api/v1/convert/pairs/{pair_id}"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "reason": "remove disabled pair" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(delete.status(), StatusCode::NO_CONTENT);

    let (pair_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM convert_pairs WHERE id = ?")
        .bind(pair_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(pair_count, 0);

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
               FROM admin_audit_logs
               WHERE admin_id = ? AND target_type = 'convert_pair' AND target_id = ?
               ORDER BY id"#,
    )
    .bind(admin_id)
    .bind(pair_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 4);
    assert_eq!(audits[0].action, "convert_pair.create");
    assert_eq!(audits[0].target_type, "convert_pair");
    assert_eq!(audits[0].target_id, pair_id.to_string());
    assert!(audits[0].before_json.is_none());
    assert_eq!(audits[0].after_json.as_ref().unwrap()["enabled"], true);
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["from_asset_symbol"],
        from_symbol
    );
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["to_asset_symbol"],
        to_symbol
    );
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["target_min_amount"],
        "10.000000000000000000"
    );
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["target_max_amount"],
        "1000.000000000000000000"
    );
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["fee_rate"],
        "0.00100000"
    );
    assert_eq!(audits[0].reason.as_deref(), Some("initial convert pair"));
    assert_eq!(audits[1].action, "convert_pair.update");
    assert_eq!(
        audits[1].before_json.as_ref().unwrap()["pricing_mode"],
        "fixed"
    );
    assert_eq!(
        audits[1].after_json.as_ref().unwrap()["pricing_mode"],
        "market"
    );
    assert_eq!(
        audits[1].after_json.as_ref().unwrap()["min_amount"],
        "2.000000000000000000"
    );
    assert_eq!(
        audits[1].after_json.as_ref().unwrap()["fee_rate"],
        "0.00300000"
    );
    assert!(audits[1].after_json.as_ref().unwrap()["max_amount"].is_null());
    assert_eq!(
        audits[1].after_json.as_ref().unwrap()["target_min_amount"],
        "20.000000000000000000"
    );
    assert!(audits[1].after_json.as_ref().unwrap()["target_max_amount"].is_null());
    assert_eq!(audits[1].reason.as_deref(), Some("edit convert pair"));
    assert_eq!(audits[2].action, "convert_pair.update_status");
    assert_eq!(audits[2].before_json.as_ref().unwrap()["enabled"], true);
    assert_eq!(audits[2].after_json.as_ref().unwrap()["enabled"], false);
    assert_eq!(audits[2].reason.as_deref(), Some("pause pair"));
    assert_eq!(audits[3].action, "convert_pair.delete");
    assert_eq!(audits[3].before_json.as_ref().unwrap()["enabled"], false);
    assert!(audits[3].after_json.is_none());
    assert_eq!(audits[3].reason.as_deref(), Some("remove disabled pair"));

    delete_pair_fixture(&pool, pair_id, from_asset, to_asset, admin_id, role_id).await?;
    Ok(())
}

#[tokio::test]
async fn admin_convert_pair_delete_requires_disabled_and_no_references()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let user_id = create_user(&pool).await;
    let from_asset = create_asset(&pool, "CDF").await;
    let to_asset = create_asset(&pool, "CDT").await;
    let pair_id = seed_convert_pair(&pool, from_asset, to_asset, true).await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let active_delete = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/admin/api/v1/convert/pairs/{pair_id}"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "reason": "delete active convert pair" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let active_delete_status = active_delete.status();
    let active_delete_payload = body_json(active_delete).await?;
    assert_eq!(active_delete_status, StatusCode::BAD_REQUEST);
    assert_eq!(
        active_delete_payload["message"],
        "validation error: convert pair must be disabled before deletion"
    );

    sqlx::query("UPDATE convert_pairs SET enabled = false WHERE id = ?")
        .bind(pair_id)
        .execute(&pool)
        .await?;
    seed_convert_order(&pool, user_id, pair_id, from_asset, to_asset, "completed").await;

    let referenced_delete = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/admin/api/v1/convert/pairs/{pair_id}"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "reason": "delete referenced convert pair" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let referenced_delete_status = referenced_delete.status();
    let referenced_delete_payload = body_json(referenced_delete).await?;
    assert_eq!(referenced_delete_status, StatusCode::BAD_REQUEST);
    assert_eq!(
        referenced_delete_payload["message"],
        "validation error: convert pair with related records cannot be deleted"
    );

    let (pair_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM convert_pairs WHERE id = ?")
        .bind(pair_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(pair_count, 1);

    delete_order_fixture(&pool, pair_id, from_asset, to_asset, &[user_id]).await?;
    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(admin_id)
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
    Ok(())
}

#[tokio::test]
async fn admin_new_coin_listing_routes_require_admin_scope_and_mysql() -> Result<(), Box<dyn Error>>
{
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));
    let paths = [
        "/admin/api/v1/new-coins/1/subscriptions",
        "/admin/api/v1/new-coins/1/distributions",
        "/admin/api/v1/new-coins/subscriptions",
        "/admin/api/v1/new-coins/distributions",
        "/admin/api/v1/new-coins/purchases",
        "/admin/api/v1/new-coins/lock-positions",
        "/admin/api/v1/new-coins/unlocks",
    ];

    for path in paths {
        let missing = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await?;
        assert_eq!(missing.status(), StatusCode::UNAUTHORIZED, "path: {path}");

        let user = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header(AUTHORIZATION, format!("Bearer {user_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await?;
        assert_eq!(user.status(), StatusCode::FORBIDDEN, "path: {path}");

        let admin = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header(AUTHORIZATION, format!("Bearer {admin_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await?;
        assert_eq!(
            admin.status(),
            StatusCode::INTERNAL_SERVER_ERROR,
            "path: {path}"
        );
        let payload = body_json(admin).await?;
        assert_eq!(payload["code"], "INTERNAL_ERROR");
        assert!(
            payload["message"]
                .as_str()
                .unwrap()
                .contains("mysql pool is not configured for admin convert routes"),
            "path: {path}, payload: {payload}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn admin_new_coin_listing_routes_filter_seeded_records() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let user_email = format!(
        "admin-new-coin-filter-{}@example.test",
        Uuid::now_v7().simple()
    );
    let user_id = create_user_with_email(&pool, user_email.clone()).await;
    let other_user_id = create_user(&pool).await;
    let asset_id = create_asset(&pool, "ANL").await;
    let quote_asset = create_asset(&pool, "AQL").await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let symbol = format!("ANL{}", Uuid::now_v7().simple()).to_ascii_uppercase();
    let pair_symbol = format!("{symbol}USDT");
    let unlock_at = chrono::Utc
        .with_ymd_and_hms(2026, 11, 10, 11, 22, 33)
        .unwrap();

    let project_id = sqlx::query(
        r#"INSERT INTO new_coin_projects
           (asset_id, symbol, lifecycle_status, total_supply, issue_price, listed_at,
            unlock_type, fixed_unlock_at, status, remaining_supply)
           VALUES (?, ?, 'listed', ?, ?, ?, 'fixed_time', ?, 'active', 1000000)"#,
    )
    .bind(asset_id)
    .bind(&symbol)
    .bind(decimal("1000000.000000000000000000"))
    .bind(decimal("1.000000000000000000"))
    .bind(chrono::Utc.with_ymd_and_hms(2026, 10, 1, 8, 0, 0).unwrap())
    .bind(unlock_at)
    .execute(&pool)
    .await?
    .last_insert_id();
    let pair_id = sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, 8, 8, ?, 'active', 'spot')"#,
    )
    .bind(asset_id)
    .bind(quote_asset)
    .bind(&pair_symbol)
    .bind(decimal("1.000000000000000000"))
    .execute(&pool)
    .await?
    .last_insert_id();
    let lock_position_id = sqlx::query(
        r#"INSERT INTO asset_lock_positions
           (user_id, asset_id, unlock_type, unlock_at, locked_amount, released_amount,
            remaining_amount, merge_key, status)
           VALUES (?, ?, 'fixed_time', ?, ?, 0, ?, ?, 'active')"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(unlock_at)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("10.000000000000000000"))
    .bind(format!("admin-list-lock-{project_id}-{user_id}"))
    .execute(&pool)
    .await?
    .last_insert_id();
    let subscription_id = sqlx::query(
        r#"INSERT INTO new_coin_subscriptions
           (project_id, user_id, quote_asset, quote_amount, requested_quantity,
            allocated_quantity, status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, 'allocated', ?)"#,
    )
    .bind(project_id)
    .bind(user_id)
    .bind(quote_asset)
    .bind(decimal("20.000000000000000000"))
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("10.000000000000000000"))
    .bind(format!("admin-list-sub-{project_id}"))
    .execute(&pool)
    .await?
    .last_insert_id();
    let other_lock_position_id = sqlx::query(
        r#"INSERT INTO asset_lock_positions
           (user_id, asset_id, unlock_type, unlock_at, locked_amount, released_amount,
            remaining_amount, merge_key, status)
           VALUES (?, ?, 'fixed_time', ?, ?, 0, ?, ?, 'active')"#,
    )
    .bind(other_user_id)
    .bind(asset_id)
    .bind(unlock_at)
    .bind(decimal("8.000000000000000000"))
    .bind(decimal("8.000000000000000000"))
    .bind(format!(
        "admin-list-lock-other-{project_id}-{other_user_id}"
    ))
    .execute(&pool)
    .await?
    .last_insert_id();
    let other_subscription_id = sqlx::query(
        r#"INSERT INTO new_coin_subscriptions
           (project_id, user_id, quote_asset, quote_amount, requested_quantity,
            allocated_quantity, status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, 'allocated', ?)"#,
    )
    .bind(project_id)
    .bind(other_user_id)
    .bind(quote_asset)
    .bind(decimal("8.000000000000000000"))
    .bind(decimal("4.000000000000000000"))
    .bind(decimal("4.000000000000000000"))
    .bind(format!("admin-list-sub-other-{project_id}"))
    .execute(&pool)
    .await?
    .last_insert_id();
    let distribution_id = sqlx::query(
        r#"INSERT INTO new_coin_distributions
           (project_id, user_id, subscription_id, asset_id, quantity, lock_position_id,
            status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, 'locked', ?)"#,
    )
    .bind(project_id)
    .bind(user_id)
    .bind(subscription_id)
    .bind(asset_id)
    .bind(decimal("10.000000000000000000"))
    .bind(lock_position_id)
    .bind(format!("admin-list-dist-{project_id}"))
    .execute(&pool)
    .await?
    .last_insert_id();
    sqlx::query(
        r#"INSERT INTO new_coin_distributions
           (project_id, user_id, subscription_id, asset_id, quantity, lock_position_id,
            status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, 'locked', ?)"#,
    )
    .bind(project_id)
    .bind(other_user_id)
    .bind(other_subscription_id)
    .bind(asset_id)
    .bind(decimal("4.000000000000000000"))
    .bind(other_lock_position_id)
    .bind(format!("admin-list-dist-other-{project_id}"))
    .execute(&pool)
    .await?;
    let purchase_id = sqlx::query(
        r#"INSERT INTO new_coin_purchase_orders
           (project_id, user_id, pair_id, base_asset, quote_asset, price, quantity,
            quote_amount, lock_position_id, status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'locked', ?)"#,
    )
    .bind(project_id)
    .bind(user_id)
    .bind(pair_id)
    .bind(asset_id)
    .bind(quote_asset)
    .bind(decimal("2.000000000000000000"))
    .bind(decimal("5.000000000000000000"))
    .bind(decimal("10.000000000000000000"))
    .bind(lock_position_id)
    .bind(format!("admin-list-purchase-{project_id}"))
    .execute(&pool)
    .await?
    .last_insert_id();
    sqlx::query(
        r#"INSERT INTO new_coin_purchase_orders
           (project_id, user_id, pair_id, base_asset, quote_asset, price, quantity,
            quote_amount, lock_position_id, status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'locked', ?)"#,
    )
    .bind(project_id)
    .bind(other_user_id)
    .bind(pair_id)
    .bind(asset_id)
    .bind(quote_asset)
    .bind(decimal("2.000000000000000000"))
    .bind(decimal("4.000000000000000000"))
    .bind(decimal("8.000000000000000000"))
    .bind(other_lock_position_id)
    .bind(format!("admin-list-purchase-other-{project_id}"))
    .execute(&pool)
    .await?;
    let unlock_id = sqlx::query(
        r#"INSERT INTO asset_unlock_records
           (user_id, asset_id, lock_position_id, unlock_quantity, unlock_price,
            unlock_fee_enabled, unlock_fee_rate, unlock_fee_basis, unlock_fee_asset,
            unlock_fee_amount, fee_paid_status, status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, TRUE, ?, 'profit', ?, ?, 'pending', 'pending', ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(lock_position_id)
    .bind(decimal("5.000000000000000000"))
    .bind(decimal("2.000000000000000000"))
    .bind(decimal("0.04000000"))
    .bind(quote_asset)
    .bind(decimal("0.400000000000000000"))
    .bind(format!("admin-list-unlock-{project_id}"))
    .execute(&pool)
    .await?
    .last_insert_id();
    sqlx::query(
        r#"INSERT INTO asset_unlock_records
           (user_id, asset_id, lock_position_id, unlock_quantity, unlock_price,
            unlock_fee_enabled, unlock_fee_rate, unlock_fee_basis, unlock_fee_asset,
            unlock_fee_amount, fee_paid_status, status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, TRUE, ?, 'profit', ?, ?, 'pending', 'pending', ?)"#,
    )
    .bind(other_user_id)
    .bind(asset_id)
    .bind(other_lock_position_id)
    .bind(decimal("4.000000000000000000"))
    .bind(decimal("2.000000000000000000"))
    .bind(decimal("0.04000000"))
    .bind(quote_asset)
    .bind(decimal("0.320000000000000000"))
    .bind(format!("admin-list-unlock-other-{project_id}"))
    .execute(&pool)
    .await?;

    for subscriptions_path in [
        format!(
            "/admin/api/v1/new-coins/{project_id}/subscriptions?email={user_email}&status=allocated&limit=10"
        ),
        format!(
            "/admin/api/v1/new-coins/subscriptions?project_id={project_id}&email={user_email}&status=allocated&limit=10"
        ),
    ] {
        let subscriptions = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(subscriptions_path)
                    .header(AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await?;
        let subscriptions_status = subscriptions.status();
        let subscriptions_payload = body_json(subscriptions).await?;
        assert_eq!(
            subscriptions_status,
            StatusCode::OK,
            "payload: {subscriptions_payload}"
        );
        let subscriptions = subscriptions_payload["subscriptions"].as_array().unwrap();
        assert_eq!(subscriptions.len(), 1);
        assert_eq!(subscriptions[0]["id"], subscription_id);
        assert_eq!(subscriptions[0]["user_id"], user_id);
        assert_eq!(subscriptions[0]["quote_asset"], quote_asset);
        assert_eq!(subscriptions[0]["status"], "allocated");
    }

    for distributions_path in [
        format!(
            "/admin/api/v1/new-coins/{project_id}/distributions?email={user_email}&status=locked&limit=10"
        ),
        format!(
            "/admin/api/v1/new-coins/distributions?project_id={project_id}&email={user_email}&status=locked&limit=10"
        ),
    ] {
        let distributions = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(distributions_path)
                    .header(AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await?;
        let distributions_status = distributions.status();
        let distributions_payload = body_json(distributions).await?;
        assert_eq!(
            distributions_status,
            StatusCode::OK,
            "payload: {distributions_payload}"
        );
        let distributions = distributions_payload["distributions"].as_array().unwrap();
        assert_eq!(distributions.len(), 1);
        assert_eq!(distributions[0]["id"], distribution_id);
        assert_eq!(distributions[0]["subscription_id"], subscription_id);
        assert_eq!(distributions[0]["lock_position_id"], lock_position_id);
        assert_eq!(distributions[0]["status"], "locked");
    }

    let purchases = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/new-coins/purchases?project_id={project_id}&email={user_email}&status=locked&limit=10"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let purchases_status = purchases.status();
    let purchases_payload = body_json(purchases).await?;
    assert_eq!(
        purchases_status,
        StatusCode::OK,
        "payload: {purchases_payload}"
    );
    let purchases = purchases_payload["purchases"].as_array().unwrap();
    assert_eq!(purchases.len(), 1);
    assert_eq!(purchases[0]["id"], purchase_id);
    assert_eq!(purchases[0]["pair_id"], pair_id);
    assert_eq!(purchases[0]["quote_amount"], "10.000000000000000000");
    assert_eq!(purchases[0]["lock_position_id"], lock_position_id);

    let lock_positions = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/new-coins/lock-positions?email={user_email}&asset_id={asset_id}&status=active&limit=10"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let lock_positions_status = lock_positions.status();
    let lock_positions_payload = body_json(lock_positions).await?;
    assert_eq!(
        lock_positions_status,
        StatusCode::OK,
        "payload: {lock_positions_payload}"
    );
    let lock_positions = lock_positions_payload["lock_positions"].as_array().unwrap();
    assert_eq!(lock_positions.len(), 1);
    assert_eq!(lock_positions[0]["id"], lock_position_id);
    assert_eq!(lock_positions[0]["unlock_type"], "fixed_time");
    assert_eq!(
        lock_positions[0]["remaining_amount"],
        "10.000000000000000000"
    );

    let unlocks = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/new-coins/unlocks?email={user_email}&asset_id={asset_id}&status=pending&fee_paid_status=pending&limit=10"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let unlocks_status = unlocks.status();
    let unlocks_payload = body_json(unlocks).await?;
    assert_eq!(unlocks_status, StatusCode::OK, "payload: {unlocks_payload}");
    let unlocks = unlocks_payload["unlocks"].as_array().unwrap();
    assert_eq!(unlocks.len(), 1);
    assert_eq!(unlocks[0]["id"], unlock_id);
    assert_eq!(unlocks[0]["lock_position_id"], lock_position_id);
    assert_eq!(unlocks[0]["unlock_fee_basis"], "profit");
    assert_eq!(unlocks[0]["fee_paid_status"], "pending");

    sqlx::query("DELETE FROM asset_unlock_records WHERE idempotency_key IN (?, ?)")
        .bind(format!("admin-list-unlock-{project_id}"))
        .bind(format!("admin-list-unlock-other-{project_id}"))
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM new_coin_purchase_orders WHERE project_id = ?")
        .bind(project_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM new_coin_distributions WHERE project_id = ?")
        .bind(project_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM new_coin_subscriptions WHERE project_id = ?")
        .bind(project_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM asset_lock_positions WHERE id IN (?, ?)")
        .bind(lock_position_id)
        .bind(other_lock_position_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM trading_pairs WHERE id = ?")
        .bind(pair_id)
        .execute(&pool)
        .await?;
    delete_new_coin_project_fixture(&pool, project_id, asset_id, admin_id, role_id).await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(quote_asset)
        .execute(&pool)
        .await?;
    for id in [user_id, other_user_id] {
        sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(id)
            .execute(&pool)
            .await?;
    }
    Ok(())
}

#[tokio::test]
async fn admin_wallet_ledger_offset_paging_returns_disjoint_pages_and_filtered_total()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (_role_id, admin_id) = create_admin_user(&pool).await;
    let user_email = format!("admin-ledger-page-{}@example.test", Uuid::now_v7().simple());
    let user_id = create_user_with_email(&pool, user_email.clone()).await;
    let other_user_id = create_user(&pool).await;
    let (asset_id, _symbol) = create_asset_with_symbol(&pool, "APG").await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let mut ledger_ids = Vec::new();
    for (owner_id, index) in [(user_id, 0), (user_id, 1), (user_id, 2), (other_user_id, 3)] {
        let id = sqlx::query(
            r#"INSERT INTO wallet_ledger
               (user_id, asset_id, change_type, amount, balance_type, balance_after,
                available_after, frozen_after, locked_after, ref_type, ref_id)
               VALUES (?, ?, 'deposit', ?, 'available', ?, ?, ?, ?, 'manual', ?)"#,
        )
        .bind(owner_id)
        .bind(asset_id)
        .bind(decimal("1.000000000000000000"))
        .bind(decimal("1.000000000000000000"))
        .bind(decimal("1.000000000000000000"))
        .bind(decimal("0.000000000000000000"))
        .bind(decimal("0.000000000000000000"))
        .bind(format!("admin-ledger-page-{user_id}-{index}"))
        .execute(&pool)
        .await?
        .last_insert_id();
        ledger_ids.push(id);
    }

    let mut page_ids = Vec::new();
    for offset in [0, 2] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/admin/api/v1/wallet/ledger?user_id={user_id}&limit=2&offset={offset}"
                    ))
                    .header(AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await?;
        let status = response.status();
        let payload = body_json(response).await?;
        assert_eq!(status, StatusCode::OK, "payload: {payload}");
        // 总数必须反映筛选条件本身，而不是当前页行数。
        assert_eq!(payload["total"], 3, "payload: {payload}");
        page_ids.extend(
            payload["ledger"]
                .as_array()
                .unwrap()
                .iter()
                .map(|entry| entry["id"].as_u64().unwrap()),
        );
    }
    assert_eq!(page_ids.len(), 3);
    let unique_ids = page_ids.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(unique_ids.len(), 3, "pages must not overlap: {page_ids:?}");
    assert!(unique_ids.iter().all(|id| ledger_ids[..3].contains(id)));

    let unfiltered = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/wallet/ledger?limit=1")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let unfiltered_status = unfiltered.status();
    let unfiltered_payload = body_json(unfiltered).await?;
    assert_eq!(
        unfiltered_status,
        StatusCode::OK,
        "payload: {unfiltered_payload}"
    );
    assert_eq!(unfiltered_payload["ledger"].as_array().unwrap().len(), 1);
    assert!(unfiltered_payload["total"].as_i64().unwrap() >= 4);

    sqlx::query("DELETE FROM wallet_ledger WHERE user_id IN (?, ?)")
        .bind(user_id)
        .bind(other_user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id IN (?, ?)")
        .bind(user_id)
        .bind(other_user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn admin_agent_commissions_offset_paging_totals_respect_status_filter()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let agent_owner_id = create_user(&pool).await;
    let commission_user_id = create_user(&pool).await;
    let agent_code = format!("P{}", Uuid::now_v7().simple()).to_ascii_uppercase();
    let agent_id = sqlx::query(
        r#"INSERT INTO agents (user_id, agent_code, level, path, status)
           VALUES (?, ?, 1, '', 'active')"#,
    )
    .bind(agent_owner_id)
    .bind(agent_code)
    .execute(&pool)
    .await?
    .last_insert_id();
    sqlx::query("UPDATE agents SET root_agent_id = ?, path = ? WHERE id = ?")
        .bind(agent_id)
        .bind(format!("/agent:{agent_id}"))
        .bind(agent_id)
        .execute(&pool)
        .await?;
    for status in ["pending", "pending", "pending", "settled"] {
        seed_agent_commission(
            &pool,
            agent_id,
            commission_user_id,
            "convert_order",
            "100.000000000000000000",
            "5.000000000000000000",
            status,
        )
        .await;
    }
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let mut page_ids = Vec::new();
    for offset in [0, 2] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/admin/api/v1/agent-commissions?agent_id={agent_id}&status=pending&limit=2&offset={offset}"
                    ))
                    .header(AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await?;
        let status_code = response.status();
        let payload = body_json(response).await?;
        assert_eq!(status_code, StatusCode::OK, "payload: {payload}");
        assert_eq!(payload["total"], 3, "payload: {payload}");
        page_ids.extend(
            payload["commissions"]
                .as_array()
                .unwrap()
                .iter()
                .map(|entry| entry["id"].as_u64().unwrap()),
        );
    }
    assert_eq!(page_ids.len(), 3);
    assert_eq!(
        page_ids.iter().copied().collect::<BTreeSet<_>>().len(),
        3,
        "pages must not overlap: {page_ids:?}"
    );

    let all_statuses = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/admin/api/v1/agent-commissions?agent_id={agent_id}&limit=1"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let all_statuses_code = all_statuses.status();
    let all_statuses_payload = body_json(all_statuses).await?;
    assert_eq!(
        all_statuses_code,
        StatusCode::OK,
        "payload: {all_statuses_payload}"
    );
    assert_eq!(
        all_statuses_payload["commissions"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(all_statuses_payload["total"], 4);

    delete_admin_agent_management_fixture(
        &pool,
        admin_id,
        role_id,
        &[agent_id],
        &[agent_owner_id, commission_user_id],
    )
    .await?;
    Ok(())
}

const LOGIN_LOCKOUT_TEST_PASSWORD: &str = "CorrectPassword123!";
const ADMIN_TOTP_TEST_SECRET: &str = "JBSWY3DPEHPK3PXP";

async fn create_login_admin(pool: &MySqlPool) -> (u64, u64, String) {
    let suffix = Uuid::now_v7().simple().to_string();
    let role_id =
        sqlx::query("INSERT INTO admin_roles (name, permissions) VALUES (?, JSON_ARRAY('*'))")
            .bind(format!("admin-login-role-{suffix}"))
            .execute(pool)
            .await
            .unwrap()
            .last_insert_id();
    let username = format!("admin-login-{suffix}");
    let admin_id =
        sqlx::query("INSERT INTO admin_users (username, password_hash, role_id) VALUES (?, ?, ?)")
            .bind(&username)
            .bind(hash_password(LOGIN_LOCKOUT_TEST_PASSWORD).unwrap())
            .bind(role_id)
            .execute(pool)
            .await
            .unwrap()
            .last_insert_id();

    (role_id, admin_id, username)
}

async fn delete_login_admin(
    pool: &MySqlPool,
    role_id: u64,
    admin_id: u64,
    username: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM admin_login_two_factor_challenges WHERE admin_id = ?")
        .bind(admin_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM admin_two_factor_settings WHERE admin_id = ?")
        .bind(admin_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM refresh_tokens WHERE actor_type = 'admin' AND actor_id = ?")
        .bind(admin_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM login_failure_counters WHERE actor_type = 'admin' AND identifier = ?")
        .bind(username)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(role_id)
        .execute(pool)
        .await?;
    Ok(())
}

fn admin_login_request(username: &str, password: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/admin/api/v1/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "username": username, "password": password }).to_string(),
        ))
        .unwrap()
}

fn totp_code_at(secret: &str, offset_seconds: i64) -> String {
    let bytes = base32_decode_no_padding(secret).unwrap();
    let timestamp = (chrono::Utc::now().timestamp() + offset_seconds).max(0) as u64;
    totp_code_for_time(&bytes, timestamp, TOTP_STEP_SECONDS, TOTP_DIGITS)
}

fn invalid_totp_code(secret: &str) -> String {
    let accepted = [
        totp_code_at(secret, -(TOTP_STEP_SECONDS as i64)),
        totp_code_at(secret, 0),
        totp_code_at(secret, TOTP_STEP_SECONDS as i64),
    ];

    (0..10)
        .map(|value| format!("{value:06}"))
        .find(|candidate| !accepted.contains(candidate))
        .unwrap()
}

async fn login_failure_count(pool: &MySqlPool, username: &str) -> Option<u32> {
    sqlx::query_scalar::<_, u32>(
        "SELECT failure_count FROM login_failure_counters WHERE actor_type = 'admin' AND identifier = ?",
    )
    .bind(username)
    .fetch_optional(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn admin_login_locks_out_after_repeated_password_failures() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let (role_id, admin_id, username) = create_login_admin(&pool).await;
    let app = build_router(AppState::new(test_settings()).with_mysql(pool.clone()));

    for attempt in 1..=4 {
        let response = app
            .clone()
            .oneshot(admin_login_request(&username, "WrongPassword!"))
            .await?;
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "attempt {attempt} must stay unauthorized"
        );
    }

    let locking = app
        .clone()
        .oneshot(admin_login_request(&username, "WrongPassword!"))
        .await?;
    assert_eq!(locking.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        body_json(locking).await?["code"],
        "login_temporarily_locked"
    );
    assert_eq!(login_failure_count(&pool, &username).await, Some(5));

    // 锁定期内即使密码正确也必须拒绝。
    let locked = app
        .clone()
        .oneshot(admin_login_request(&username, LOGIN_LOCKOUT_TEST_PASSWORD))
        .await?;
    assert_eq!(locked.status(), StatusCode::BAD_REQUEST);
    assert_eq!(body_json(locked).await?["code"], "login_temporarily_locked");

    // 锁定到期后自愈，无需人工干预即可重新登录。
    sqlx::query(
        r#"UPDATE login_failure_counters
           SET locked_until = UTC_TIMESTAMP(6) - INTERVAL 1 SECOND,
               window_expires_at = UTC_TIMESTAMP(6) - INTERVAL 1 SECOND
           WHERE actor_type = 'admin' AND identifier = ?"#,
    )
    .bind(&username)
    .execute(&pool)
    .await?;

    let healed = app
        .clone()
        .oneshot(admin_login_request(&username, LOGIN_LOCKOUT_TEST_PASSWORD))
        .await?;
    assert_eq!(healed.status(), StatusCode::OK);
    assert!(body_json(healed).await?["access_token"].is_string());
    assert_eq!(login_failure_count(&pool, &username).await, None);

    delete_login_admin(&pool, role_id, admin_id, &username).await?;
    Ok(())
}

#[tokio::test]
async fn successful_admin_login_clears_login_failure_counter() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let (role_id, admin_id, username) = create_login_admin(&pool).await;
    let app = build_router(AppState::new(test_settings()).with_mysql(pool.clone()));

    for _ in 0..2 {
        let response = app
            .clone()
            .oneshot(admin_login_request(&username, "WrongPassword!"))
            .await?;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    assert_eq!(login_failure_count(&pool, &username).await, Some(2));

    let success = app
        .clone()
        .oneshot(admin_login_request(&username, LOGIN_LOCKOUT_TEST_PASSWORD))
        .await?;
    assert_eq!(success.status(), StatusCode::OK);
    assert_eq!(login_failure_count(&pool, &username).await, None);

    // 计数已归零，后续 4 次失败不应再触发锁定。
    for _ in 0..4 {
        let response = app
            .clone()
            .oneshot(admin_login_request(&username, "WrongPassword!"))
            .await?;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    delete_login_admin(&pool, role_id, admin_id, &username).await?;
    Ok(())
}

#[tokio::test]
async fn admin_login_requires_totp_when_two_factor_enabled() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let key = settings
        .exposed_credential_encryption_key()
        .unwrap()
        .to_owned();
    let (role_id, admin_id, username) = create_login_admin(&pool).await;
    sqlx::query(
        r#"INSERT INTO admin_two_factor_settings
           (admin_id, totp_secret_encrypted, totp_enabled, confirmed_at, last_verified_at)
           VALUES (?, ?, TRUE, CURRENT_TIMESTAMP(6), CURRENT_TIMESTAMP(6))"#,
    )
    .bind(admin_id)
    .bind(encrypt_secret(ADMIN_TOTP_TEST_SECRET, &key)?)
    .execute(&pool)
    .await?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let challenge_response = app
        .clone()
        .oneshot(admin_login_request(&username, LOGIN_LOCKOUT_TEST_PASSWORD))
        .await?;
    assert_eq!(challenge_response.status(), StatusCode::OK);
    let challenge_payload = body_json(challenge_response).await?;
    assert_eq!(challenge_payload["requires_2fa"], true);
    assert!(challenge_payload["access_token"].is_null());
    let challenge_id = challenge_payload["challenge_id"]
        .as_str()
        .unwrap()
        .to_owned();

    let two_factor_request = |challenge_id: &str, code: &str| {
        Request::builder()
            .method("POST")
            .uri("/admin/api/v1/auth/login/2fa")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({ "challenge_id": challenge_id, "totp_code": code }).to_string(),
            ))
            .unwrap()
    };

    let wrong_code = app
        .clone()
        .oneshot(two_factor_request(
            &challenge_id,
            &invalid_totp_code(ADMIN_TOTP_TEST_SECRET),
        ))
        .await?;
    assert_eq!(wrong_code.status(), StatusCode::BAD_REQUEST);
    assert_eq!(body_json(wrong_code).await?["code"], "invalid_2fa_code");

    let verified = app
        .clone()
        .oneshot(two_factor_request(
            &challenge_id,
            &totp_code_at(ADMIN_TOTP_TEST_SECRET, 0),
        ))
        .await?;
    assert_eq!(verified.status(), StatusCode::OK);
    let verified_payload = body_json(verified).await?;
    assert!(verified_payload["access_token"].is_string());
    assert_eq!(verified_payload["scope"], "admin");

    // 挑战一次性消费，重放同一个 challenge 必须失败。
    let replayed = app
        .clone()
        .oneshot(two_factor_request(
            &challenge_id,
            &totp_code_at(ADMIN_TOTP_TEST_SECRET, 0),
        ))
        .await?;
    assert_eq!(replayed.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        body_json(replayed).await?["code"],
        "login_2fa_challenge_expired"
    );

    delete_login_admin(&pool, role_id, admin_id, &username).await?;
    Ok(())
}

#[tokio::test]
async fn admin_without_two_factor_logs_in_and_can_enroll() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let (role_id, admin_id, username) = create_login_admin(&pool).await;
    let app = build_router(AppState::new(test_settings()).with_mysql(pool.clone()));

    let login = app
        .clone()
        .oneshot(admin_login_request(&username, LOGIN_LOCKOUT_TEST_PASSWORD))
        .await?;
    assert_eq!(login.status(), StatusCode::OK);
    let login_payload = body_json(login).await?;
    assert!(login_payload["requires_2fa"].is_null());
    let token = login_payload["access_token"].as_str().unwrap().to_owned();

    let status = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/auth/2fa")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(status.status(), StatusCode::OK);
    assert_eq!(body_json(status).await?["totp_enabled"], false);

    let setup = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/auth/2fa/setup")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(setup.status(), StatusCode::OK);
    let setup_payload = body_json(setup).await?;
    let secret = setup_payload["secret"].as_str().unwrap().to_owned();
    assert!(
        setup_payload["otpauth_uri"]
            .as_str()
            .unwrap()
            .contains(&secret)
    );

    let code_request = |uri: &str, code: &str| {
        Request::builder()
            .method("POST")
            .uri(uri.to_owned())
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(json!({ "totp_code": code }).to_string()))
            .unwrap()
    };

    let confirmed = app
        .clone()
        .oneshot(code_request(
            "/admin/api/v1/auth/2fa/confirm",
            &totp_code_at(&secret, 0),
        ))
        .await?;
    assert_eq!(confirmed.status(), StatusCode::OK);
    assert_eq!(body_json(confirmed).await?["totp_enabled"], true);

    let challenged = app
        .clone()
        .oneshot(admin_login_request(&username, LOGIN_LOCKOUT_TEST_PASSWORD))
        .await?;
    assert_eq!(challenged.status(), StatusCode::OK);
    assert_eq!(body_json(challenged).await?["requires_2fa"], true);

    let disabled = app
        .clone()
        .oneshot(code_request(
            "/admin/api/v1/auth/2fa/disable",
            &totp_code_at(&secret, 0),
        ))
        .await?;
    assert_eq!(disabled.status(), StatusCode::OK);
    assert_eq!(body_json(disabled).await?["totp_enabled"], false);

    let plain_login = app
        .clone()
        .oneshot(admin_login_request(&username, LOGIN_LOCKOUT_TEST_PASSWORD))
        .await?;
    assert_eq!(plain_login.status(), StatusCode::OK);
    assert!(body_json(plain_login).await?["access_token"].is_string());

    delete_login_admin(&pool, role_id, admin_id, &username).await?;
    Ok(())
}

#[tokio::test]
async fn admin_agent_password_reset_rotates_login_credentials_and_audits()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let agent_owner_id = create_user(&pool).await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let agent_code = format!("P{}", Uuid::now_v7().simple()).to_ascii_uppercase();
    let agent_username = format!("agent-pwd-{}", Uuid::now_v7().simple());
    let initial_password = "agent-initial-1";
    let rotated_password = "agent-rotated-1";

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/agents")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "user_id": agent_owner_id,
                        "agent_code": agent_code,
                        "admin_username": agent_username,
                        "admin_password": initial_password,
                        "reason": "create agent for password rotation"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let create_status = create.status();
    let created = body_json(create).await?;
    assert_eq!(create_status, StatusCode::OK, "payload: {created}");
    let agent_id = created["id"].as_u64().unwrap();
    let agent_admin_user_id = created["admin_user_id"].as_u64().unwrap();

    let initial_login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/agent/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "username": agent_username, "password": initial_password }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(initial_login.status(), StatusCode::OK);

    let missing_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/api/v1/agents/{agent_id}/password/reset"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "password": rotated_password }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing_reason.status(), StatusCode::BAD_REQUEST);

    let weak_password = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/api/v1/agents/{agent_id}/password/reset"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "password": "123", "reason": "too short" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(weak_password.status(), StatusCode::BAD_REQUEST);

    let reset = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/api/v1/agents/{agent_id}/password/reset"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "password": rotated_password, "reason": "credential leak" })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let reset_status = reset.status();
    let reset_payload = body_json(reset).await?;
    assert_eq!(reset_status, StatusCode::OK, "payload: {reset_payload}");
    assert_eq!(reset_payload["agent_id"], agent_id);
    assert_eq!(reset_payload["admin_user_id"], agent_admin_user_id);
    assert_eq!(reset_payload["admin_username"], agent_username);
    assert_eq!(reset_payload["requires_relogin"], true);

    let stale_login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/agent/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "username": agent_username, "password": initial_password }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(stale_login.status(), StatusCode::UNAUTHORIZED);

    let rotated_login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/agent/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "username": agent_username, "password": rotated_password }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(rotated_login.status(), StatusCode::OK);

    let revoked_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM refresh_tokens
           WHERE actor_type = 'agent' AND actor_id = ? AND revoked_at IS NOT NULL"#,
    )
    .bind(agent_admin_user_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(revoked_count, 1);

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND target_type = 'agent_admin_user'
           ORDER BY id"#,
    )
    .bind(admin_id)
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].action, "agent_admin_user.password.reset");
    assert_eq!(audits[0].target_id, agent_admin_user_id.to_string());
    assert!(audits[0].before_json.is_none());
    assert_eq!(
        audits[0].after_json.as_ref().unwrap()["admin_username"],
        agent_username
    );
    assert_eq!(audits[0].reason.as_deref(), Some("credential leak"));

    sqlx::query("DELETE FROM refresh_tokens WHERE actor_type = 'agent' AND actor_id = ?")
        .bind(agent_admin_user_id)
        .execute(&pool)
        .await?;
    delete_admin_agent_management_fixture(&pool, admin_id, role_id, &[agent_id], &[agent_owner_id])
        .await?;
    Ok(())
}

#[tokio::test]
async fn admin_user_status_update_blocks_login_and_audits() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let password = "user-status-pass-1";
    let (user_id, email) = create_user_with_login_password(&pool, password).await;
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let login_request = || {
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({ "email": email, "password": password }).to_string(),
            ))
            .unwrap()
    };

    let active_login = app.clone().oneshot(login_request()).await?;
    assert_eq!(active_login.status(), StatusCode::OK);

    let unsupported_status = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/users/{user_id}/status"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "status": "banned", "reason": "abuse" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(unsupported_status.status(), StatusCode::BAD_REQUEST);

    let missing_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/users/{user_id}/status"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "status": "disabled" }).to_string()))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing_reason.status(), StatusCode::BAD_REQUEST);

    let disable = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/users/{user_id}/status"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "status": "disabled", "reason": "account compromised" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let disable_status = disable.status();
    let disabled_payload = body_json(disable).await?;
    assert_eq!(
        disable_status,
        StatusCode::OK,
        "payload: {disabled_payload}"
    );
    assert_eq!(disabled_payload["id"], user_id);
    assert_eq!(disabled_payload["status"], "disabled");

    let blocked_login = app.clone().oneshot(login_request()).await?;
    assert_eq!(blocked_login.status(), StatusCode::UNAUTHORIZED);

    let revoked_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM refresh_tokens
           WHERE actor_type = 'user' AND actor_id = ? AND revoked_at IS NOT NULL"#,
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(revoked_count, 1);

    let restore = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/users/{user_id}/status"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "status": "active", "reason": "appeal approved" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let restore_status = restore.status();
    let restored_payload = body_json(restore).await?;
    assert_eq!(
        restore_status,
        StatusCode::OK,
        "payload: {restored_payload}"
    );
    assert_eq!(restored_payload["status"], "active");

    let restored_login = app.clone().oneshot(login_request()).await?;
    assert_eq!(restored_login.status(), StatusCode::OK);

    let audits = sqlx::query_as::<_, AdminAuditRow>(
        r#"SELECT action, target_type, target_id, before_json, after_json, reason
           FROM admin_audit_logs
           WHERE admin_id = ? AND action = 'user.status.update'
           ORDER BY id"#,
    )
    .bind(admin_id)
    .fetch_all(&pool)
    .await?;
    assert_eq!(audits.len(), 2);
    assert_eq!(audits[0].target_type, "user");
    assert_eq!(audits[0].target_id, user_id.to_string());
    assert_eq!(audits[0].before_json.as_ref().unwrap()["status"], "active");
    assert_eq!(audits[0].after_json.as_ref().unwrap()["status"], "disabled");
    assert_eq!(audits[0].reason.as_deref(), Some("account compromised"));
    assert_eq!(
        audits[1].before_json.as_ref().unwrap()["status"],
        "disabled"
    );
    assert_eq!(audits[1].after_json.as_ref().unwrap()["status"], "active");
    assert_eq!(audits[1].reason.as_deref(), Some("appeal approved"));

    sqlx::query("DELETE FROM refresh_tokens WHERE actor_type = 'user' AND actor_id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    delete_admin_agent_management_fixture(&pool, admin_id, role_id, &[], &[user_id]).await?;
    Ok(())
}

#[tokio::test]
async fn admin_rbac_returns_current_permissions_and_denies_ungranted_routes()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let suffix = Uuid::now_v7().simple().to_string();
    let role_id = sqlx::query(
        "INSERT INTO admin_roles (name, permissions) VALUES (?, JSON_ARRAY('dashboard.read'))",
    )
    .bind(format!("admin-rbac-role-{suffix}"))
    .execute(&pool)
    .await?
    .last_insert_id();
    let admin_id =
        sqlx::query("INSERT INTO admin_users (username, password_hash, role_id) VALUES (?, ?, ?)")
            .bind(format!("admin-rbac-user-{suffix}"))
            .bind("not-a-real-hash")
            .bind(role_id)
            .execute(&pool)
            .await?
            .last_insert_id();
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let access = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/access/me")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(access.status(), StatusCode::OK);
    let access_payload = body_json(access).await?;
    assert_eq!(access_payload["admin_id"], admin_id);
    assert_eq!(access_payload["permissions"], json!(["dashboard.read"]));
    assert_eq!(access_payload["is_super_admin"], false);

    let dashboard = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/dashboard")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(dashboard.status(), StatusCode::OK);

    let audit_logs = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/audit-logs")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(audit_logs.status(), StatusCode::FORBIDDEN);

    let country_write = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/countries")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from("{}"))?,
        )
        .await?;
    assert_eq!(country_write.status(), StatusCode::FORBIDDEN);

    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(role_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn admin_config_change_requires_two_people_and_replays_without_duplicate_audit()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (maker_role_id, maker_id) = create_admin_user(&pool).await;
    let (reviewer_role_id, reviewer_id) = create_admin_user(&pool).await;
    let (applier_role_id, applier_id) = create_admin_user(&pool).await;
    let maker_token = issue_token(
        &settings,
        format!("admin:{maker_id}"),
        TokenScope::Admin,
        900,
    )?;
    let reviewer_token = issue_token(
        &settings,
        format!("admin:{reviewer_id}"),
        TokenScope::Admin,
        900,
    )?;
    let applier_token = issue_token(
        &settings,
        format!("admin:{applier_id}"),
        TokenScope::Admin,
        900,
    )?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let request_id = format!("config-change-test-{}", Uuid::now_v7().simple());

    let created = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/v1/config-change-requests")
                .header(AUTHORIZATION, format!("Bearer {maker_token}"))
                .header("content-type", "application/json")
                .header("x-request-id", &request_id)
                .header("x-forwarded-for", "203.0.113.9, 10.0.0.1")
                .body(Body::from(
                    json!({
                        "config_domain": "system",
                        "target_type": "smtp",
                        "target_id": "default",
                        "action": "rotate",
                        "base_revision": 4,
                        "before_json": {"enabled": true, "password": "old-secret"},
                        "proposed_json": {"enabled": true, "password": "new-secret"},
                        "reason": "轮换邮件凭据",
                        "risk_level": "critical"
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(created.status(), StatusCode::OK);
    assert_eq!(
        created
            .headers()
            .get("x-request-id")
            .and_then(|value| value.to_str().ok()),
        Some(request_id.as_str())
    );
    let created_payload = body_json(created).await?;
    let change_id = created_payload["id"].as_u64().unwrap();
    assert_eq!(created_payload["status"], "pending");
    assert_eq!(
        created_payload["proposed_json"]["password"],
        "***REDACTED***"
    );

    let self_review = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/admin/api/v1/config-change-requests/{change_id}/review"
                ))
                .header(AUTHORIZATION, format!("Bearer {maker_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"decision": "approve", "reason": "本人确认"}).to_string(),
                ))?,
        )
        .await?;
    assert_eq!(self_review.status(), StatusCode::FORBIDDEN);

    for _ in 0..2 {
        let review = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/admin/api/v1/config-change-requests/{change_id}/review"
                    ))
                    .header(AUTHORIZATION, format!("Bearer {reviewer_token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"decision": "approve", "reason": "复核通过"}).to_string(),
                    ))?,
            )
            .await?;
        assert_eq!(review.status(), StatusCode::OK);
        assert_eq!(body_json(review).await?["status"], "approved");
    }

    for _ in 0..2 {
        let apply = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/admin/api/v1/config-change-requests/{change_id}/apply"
                    ))
                    .header(AUTHORIZATION, format!("Bearer {applier_token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"reason": "执行已复核变更"}).to_string()))?,
            )
            .await?;
        assert_eq!(apply.status(), StatusCode::OK);
        assert_eq!(body_json(apply).await?["status"], "applied");
    }

    let approved_audits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM admin_audit_logs WHERE target_type = 'admin_config_change_request' AND target_id = ? AND action = 'config_change.approved'",
    )
    .bind(change_id.to_string())
    .fetch_one(&pool)
    .await?;
    let applied_audits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM admin_audit_logs WHERE target_type = 'admin_config_change_request' AND target_id = ? AND action = 'config_change.applied'",
    )
    .bind(change_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(approved_audits, 1);
    assert_eq!(applied_audits, 1);

    let (ip, stored_request_id, after_json) = sqlx::query_as::<_, (
        Option<String>,
        Option<String>,
        Option<SqlxJson<Value>>,
    )>(
        "SELECT ip, request_id, after_json FROM admin_audit_logs WHERE admin_id = ? AND action = 'config_change.requested' AND target_id = ?",
    )
    .bind(maker_id)
    .bind(change_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(ip.as_deref(), Some("203.0.113.9"));
    assert_eq!(stored_request_id.as_deref(), Some(request_id.as_str()));
    assert_eq!(
        after_json.unwrap().0["proposed_json"]["password"],
        "***REDACTED***"
    );

    sqlx::query("DELETE FROM admin_audit_logs WHERE target_type = 'admin_config_change_request' AND target_id = ?")
        .bind(change_id.to_string())
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_config_change_requests WHERE id = ?")
        .bind(change_id)
        .execute(&pool)
        .await?;
    for (admin_id, role_id) in [
        (maker_id, maker_role_id),
        (reviewer_id, reviewer_role_id),
        (applier_id, applier_role_id),
    ] {
        sqlx::query("DELETE FROM admin_users WHERE id = ?")
            .bind(admin_id)
            .execute(&pool)
            .await?;
        sqlx::query("DELETE FROM admin_roles WHERE id = ?")
            .bind(role_id)
            .execute(&pool)
            .await?;
    }
    Ok(())
}

// 真实 HTTP -> 申购冻结 -> 管理员最终派发/退差额；每个数量案例使用独立订单和项目。
#[tokio::test]
async fn admin_new_coin_manual_distribution_settles_and_refunds_exactly_once()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    for (allocated, payment, refund, expected_status) in [
        ("4", "10", "15", "partial_allocated"),
        ("0", "0", "25", "refunded"),
        ("10", "25", "0", "allocated"),
    ] {
        let settings = test_settings();
        let (role_id, admin_id) = create_admin_user(&pool).await;
        let user_id = create_user(&pool).await;
        let asset = create_asset(&pool, "MSB").await;
        let quote = create_asset(&pool, "MSQ").await;
        let symbol = format!("MS{}", Uuid::now_v7().simple());
        let project = sqlx::query("INSERT INTO new_coin_projects (asset_id, symbol, lifecycle_status, total_supply, issue_price, quote_asset_id, remaining_supply, unlock_type, relative_unlock_seconds, status) VALUES (?, ?, 'subscription', 100, 2.5, ?, 100, 'relative_period', 86400, 'active')")
            .bind(asset).bind(&symbol).bind(quote).execute(&pool).await?.last_insert_id();
        sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available, frozen, locked) VALUES (?, ?, 100, 7, 0)")
            .bind(user_id).bind(quote).execute(&pool).await?;
        let token = issue_token(
            &settings,
            format!("admin:{admin_id}"),
            TokenScope::Admin,
            900,
        )?;
        let user_token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900)?;
        let app = build_router(AppState::new(settings.clone()).with_mysql(pool.clone()));
        let user_app = exchange_api::modules::new_coin::routes::user_routes()
            .with_state(AppState::new(settings).with_mysql(pool.clone()));
        let subscription_key = format!("manual-sub-{}", Uuid::now_v7());
        let submit = || {
            Request::builder().method("POST").uri(format!("/new-coins/{symbol}/subscriptions"))
            .header(AUTHORIZATION, format!("Bearer {user_token}")).header("content-type", "application/json")
            .body(Body::from(json!({"quote_asset_id": quote, "quote_amount": "25", "quantity": "10", "idempotency_key": subscription_key}).to_string())).unwrap()
        };
        let response = user_app.clone().oneshot(submit()).await?;
        let code = response.status();
        let payload = body_json(response).await?;
        assert_eq!(code, StatusCode::OK, "{payload}");
        assert_eq!(payload["status"], "pending");
        assert!(payload["lock_position_id"].is_null());
        assert_eq!(user_app.oneshot(submit()).await?.status(), StatusCode::OK);
        let (subscription,): (u64,) =
            sqlx::query_as("SELECT id FROM new_coin_subscriptions WHERE idempotency_key = ?")
                .bind(&subscription_key)
                .fetch_one(&pool)
                .await?;
        let (available, frozen, locked): (BigDecimal, BigDecimal, BigDecimal) = sqlx::query_as("SELECT available, frozen, locked FROM wallet_accounts WHERE user_id = ? AND asset_id = ?").bind(user_id).bind(quote).fetch_one(&pool).await?;
        assert_eq!(
            (available, frozen, locked),
            (decimal("75"), decimal("32"), decimal("0"))
        );
        let (base_available, base_locked): (BigDecimal, BigDecimal) = sqlx::query_as(
            "SELECT available, locked FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
        )
        .bind(user_id)
        .bind(asset)
        .fetch_one(&pool)
        .await?;
        assert_eq!((base_available, base_locked), (decimal("0"), decimal("0")));
        let lifecycle = |target: &str| {
            Request::builder()
                .method("PATCH")
                .uri(format!("/admin/api/v1/new-coins/{project}/lifecycle"))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"lifecycle_status": target, "reason": "manual settlement test"})
                        .to_string(),
                ))
                .unwrap()
        };
        assert_eq!(
            app.clone()
                .oneshot(lifecycle("distribution"))
                .await?
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            app.clone().oneshot(lifecycle("listed")).await?.status(),
            StatusCode::CONFLICT
        );
        let center_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/admin/api/v1/new-coins/{project}"))
                    .header(AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(center_response.status(), StatusCode::OK);
        let center = body_json(center_response).await?;
        assert_eq!(center["subscription_count"], 1);
        assert_eq!(center["pending_manual_count"], 1);
        assert_eq!(center["issuance_editable"], false);
        assert!(
            center["lifecycle_block_reason"]
                .as_str()
                .is_some_and(|reason| !reason.is_empty())
        );
        let key = format!("manual-dist-{}", Uuid::now_v7());
        let distribute = |quantity: &str, key: &str| {
            Request::builder().method("POST").uri(format!("/admin/api/v1/new-coins/{project}/distribute"))
            .header(AUTHORIZATION, format!("Bearer {token}")).header("content-type", "application/json")
            .body(Body::from(json!({"user_id": user_id, "subscription_id": subscription, "quantity": quantity, "idempotency_key": key, "reason": "final allocation and refund"}).to_string())).unwrap()
        };
        assert_eq!(
            app.clone().oneshot(distribute("11", &key)).await?.status(),
            StatusCode::BAD_REQUEST
        );
        if allocated == "4" {
            // 在最后的派发收据写入处制造失败，验证已执行的扣款、退款和锁仓全部回滚。
            let trigger = format!("manual_distribution_failure_{user_id}");
            sqlx::raw_sql(&format!("CREATE TRIGGER {trigger} BEFORE INSERT ON new_coin_distributions FOR EACH ROW BEGIN IF NEW.user_id = {user_id} THEN SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'test distribution receipt failure'; END IF; END"))
                .execute(&pool).await?;
            let failed = app.clone().oneshot(distribute(allocated, &key)).await?;
            sqlx::raw_sql(&format!("DROP TRIGGER {trigger}"))
                .execute(&pool)
                .await?;
            assert_eq!(failed.status(), StatusCode::INTERNAL_SERVER_ERROR);
            let obligation: (String, BigDecimal) = sqlx::query_as(
                "SELECT status, frozen_quote_amount FROM new_coin_subscriptions WHERE id = ?",
            )
            .bind(subscription)
            .fetch_one(&pool)
            .await?;
            assert_eq!(obligation, ("pending".into(), decimal("25")));
            let funds: (BigDecimal, BigDecimal) = sqlx::query_as(
                "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
            )
            .bind(user_id)
            .bind(quote)
            .fetch_one(&pool)
            .await?;
            assert_eq!(funds, (decimal("75"), decimal("32")));
            let supply: (BigDecimal, BigDecimal, BigDecimal) = sqlx::query_as("SELECT reserved_supply, allocated_supply, remaining_supply FROM new_coin_projects WHERE id = ?")
                .bind(project).fetch_one(&pool).await?;
            assert_eq!(supply, (decimal("10"), decimal("0"), decimal("90")));
            let (positions,): (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM asset_lock_positions WHERE user_id = ? AND asset_id = ?",
            )
            .bind(user_id)
            .bind(asset)
            .fetch_one(&pool)
            .await?;
            assert_eq!(positions, 0);
        }
        // 同键并发确认只产生一次派发和资金动作；失败的超量尝试没有占用幂等键。
        let (first, second) = tokio::join!(
            app.clone().oneshot(distribute(allocated, &key)),
            app.clone().oneshot(distribute(allocated, &key))
        );
        let first = first?;
        let code = first.status();
        let payload = body_json(first).await?;
        assert_eq!(code, StatusCode::OK, "{payload}");
        let second = second?;
        assert_eq!(second.status(), StatusCode::OK);
        assert_eq!(body_json(second).await?, payload);
        assert_eq!(
            app.clone()
                .oneshot(distribute(allocated, &format!("{key}-again")))
                .await?
                .status(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            app.clone().oneshot(distribute("9", &key)).await?.status(),
            StatusCode::CONFLICT
        );
        let order: (String, BigDecimal, BigDecimal, BigDecimal, BigDecimal) = sqlx::query_as("SELECT status, allocated_quantity, frozen_quote_amount, settled_quote_amount, refunded_quote_amount FROM new_coin_subscriptions WHERE id = ?").bind(subscription).fetch_one(&pool).await?;
        assert_eq!(
            order,
            (
                expected_status.into(),
                decimal(allocated),
                decimal("0"),
                decimal(payment),
                decimal(refund)
            )
        );
        let funds: (BigDecimal, BigDecimal) = sqlx::query_as(
            "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
        )
        .bind(user_id)
        .bind(quote)
        .fetch_one(&pool)
        .await?;
        assert_eq!(funds, (decimal("100") - decimal(payment), decimal("7"))); // 其他业务冻结款不受影响。
        let (base_locked,): (BigDecimal,) =
            sqlx::query_as("SELECT locked FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
                .bind(user_id)
                .bind(asset)
                .fetch_one(&pool)
                .await?;
        assert_eq!(base_locked, decimal(allocated));
        let supply: (BigDecimal, BigDecimal, BigDecimal) = sqlx::query_as("SELECT reserved_supply, allocated_supply, remaining_supply FROM new_coin_projects WHERE id = ?").bind(project).fetch_one(&pool).await?;
        assert_eq!(
            supply,
            (
                decimal("0"),
                decimal(allocated),
                decimal("100") - decimal(allocated)
            )
        );
        let (net_quote,): (BigDecimal,) = sqlx::query_as(
            "SELECT COALESCE(SUM(amount),0) FROM wallet_ledger WHERE user_id = ? AND asset_id = ?",
        )
        .bind(user_id)
        .bind(quote)
        .fetch_one(&pool)
        .await?;
        assert_eq!(net_quote, -decimal(payment));
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM new_coin_distributions WHERE subscription_id = ?")
                .bind(subscription)
                .fetch_one(&pool)
                .await?;
        assert_eq!(count, 1);
        assert_eq!(
            app.clone().oneshot(lifecycle("listed")).await?.status(),
            StatusCode::OK
        );
        assert_eq!(
            app.oneshot(distribute(allocated, &key)).await?.status(),
            StatusCode::OK
        ); // 上市后同参重放。
        sqlx::query("DELETE FROM wallet_ledger WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote)
            .execute(&pool)
            .await?;
        sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote)
            .execute(&pool)
            .await?;
        // 先移除派发外键，再删申购，最后复用现有锁仓/审计清理。
        sqlx::query("DELETE FROM new_coin_distributions WHERE project_id = ?")
            .bind(project)
            .execute(&pool)
            .await?;
        sqlx::query("DELETE FROM new_coin_subscriptions WHERE project_id = ?")
            .bind(project)
            .execute(&pool)
            .await?;
        sqlx::query(
            "DELETE FROM admin_audit_logs WHERE admin_id = ? AND target_type = 'new_coin_project'",
        )
        .bind(admin_id)
        .execute(&pool)
        .await?;
        delete_new_coin_distribution_fixture(&pool, project, asset, user_id, admin_id, role_id)
            .await?;
        sqlx::query("DELETE FROM assets WHERE id = ?")
            .bind(quote)
            .execute(&pool)
            .await?;
    }
    Ok(())
}

#[tokio::test]
async fn admin_new_coin_project_center_guards_issuance_and_stale_configuration()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin_user(&pool).await;
    let asset = create_asset(&pool, "PCB").await;
    let quote = create_asset(&pool, "PCQ").await;
    let symbol = format!("CENTER{}", Uuid::now_v7().simple());
    let id = sqlx::query("INSERT INTO new_coin_projects (asset_id, symbol, lifecycle_status, total_supply, issue_price, quote_asset_id, remaining_supply, unlock_type, relative_unlock_seconds, status) VALUES (?, ?, 'preheat', 100, 2.5, ?, 100, 'relative_period', 86400, 'active')")
        .bind(asset).bind(&symbol).bind(quote).execute(&pool).await?.last_insert_id();
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let request = |method: &str, uri: String, body: Value| {
        Request::builder()
            .method(method)
            .uri(uri)
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(if body.is_null() {
                Body::empty()
            } else {
                Body::from(body.to_string())
            })
            .unwrap()
    };
    let endpoint = format!("/admin/api/v1/new-coins/{id}");
    let response = app
        .clone()
        .oneshot(request("GET", endpoint.clone(), Value::Null))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let center = body_json(response).await?;
    assert_eq!(center["project"]["id"], id);
    assert_eq!(center["issuance_editable"], true);
    assert_eq!(center["pending_manual_count"], 0);
    assert_eq!(center["next_lifecycle_status"], "subscription");
    let original_version = center["configuration_version"].clone();
    let response = app.clone().oneshot(request("GET", format!("/admin/api/v1/new-coins?symbol={symbol}&lifecycle_status=preheat&status=active&limit=1"), Value::Null)).await?;
    let listing = body_json(response).await?;
    assert_eq!(listing["total"], 1);
    assert_eq!(listing["projects"][0]["id"], id);
    let payload = json!({"total_supply":"150", "issue_price":"3.25", "expected_total_supply":"100", "expected_issue_price":"2.5", "reason":"correct preheat issuance"});
    let mut blank_reason = payload.clone();
    blank_reason["reason"] = json!(" ");
    let missing_reason = app
        .clone()
        .oneshot(request(
            "PATCH",
            format!("{endpoint}/issuance"),
            blank_reason,
        ))
        .await?;
    assert_eq!(missing_reason.status(), StatusCode::BAD_REQUEST);
    sqlx::query(
        "UPDATE admin_roles SET permissions=JSON_ARRAY('new_coin.projects.read') WHERE id=?",
    )
    .bind(role_id)
    .execute(&pool)
    .await?;
    assert_eq!(
        app.clone()
            .oneshot(request("GET", endpoint.clone(), Value::Null))
            .await?
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        app.clone()
            .oneshot(request(
                "PATCH",
                format!("{endpoint}/issuance"),
                payload.clone()
            ))
            .await?
            .status(),
        StatusCode::FORBIDDEN
    );
    sqlx::query("UPDATE admin_roles SET permissions=JSON_ARRAY('*') WHERE id=?")
        .bind(role_id)
        .execute(&pool)
        .await?;
    sqlx::query("UPDATE assets SET precision_scale=2 WHERE id=?")
        .bind(quote)
        .execute(&pool)
        .await?;
    let mut excessive_precision = payload.clone();
    excessive_precision["issue_price"] = json!("3.251");
    assert_eq!(
        app.clone()
            .oneshot(request(
                "PATCH",
                format!("{endpoint}/issuance"),
                excessive_precision
            ))
            .await?
            .status(),
        StatusCode::BAD_REQUEST
    );
    let updated = app
        .clone()
        .oneshot(request(
            "PATCH",
            format!("{endpoint}/issuance"),
            payload.clone(),
        ))
        .await?;
    let status = updated.status();
    let body = body_json(updated).await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        decimal(body["remaining_supply"].as_str().unwrap()),
        decimal("150")
    );
    assert_eq!(
        app.clone()
            .oneshot(request(
                "PATCH",
                format!("{endpoint}/issuance"),
                payload.clone()
            ))
            .await?
            .status(),
        StatusCode::CONFLICT
    );
    let stale = json!({"unlock_type":"relative_period","relative_unlock_seconds":172800,"expected_config":original_version,"reason":"stale editor"});
    assert_eq!(
        app.clone()
            .oneshot(request("PATCH", format!("{endpoint}/unlock-rule"), stale))
            .await?
            .status(),
        StatusCode::CONFLICT
    );
    let center = body_json(
        app.clone()
            .oneshot(request("GET", endpoint.clone(), Value::Null))
            .await?,
    )
    .await?;
    assert_ne!(center["configuration_version"], original_version);
    let stage = json!({"lifecycle_status":"subscription","expected_config":center["configuration_version"],"reason":"open project"});
    assert_eq!(
        app.clone()
            .oneshot(request("PATCH", format!("{endpoint}/lifecycle"), stage))
            .await?
            .status(),
        StatusCode::OK
    );
    let payload = json!({"total_supply":"200","issue_price":"4","expected_total_supply":"150","expected_issue_price":"3.25","reason":"late edit"});
    assert_eq!(
        app.clone()
            .oneshot(request("PATCH", format!("{endpoint}/issuance"), payload))
            .await?
            .status(),
        StatusCode::CONFLICT
    );
    let center = body_json(
        app.clone()
            .oneshot(request("GET", endpoint.clone(), Value::Null))
            .await?,
    )
    .await?;
    assert_eq!(center["issuance_editable"], false);
    assert_eq!(center["next_lifecycle_status"], "distribution");
    let (audits,):(i64,)=sqlx::query_as("SELECT COUNT(*) FROM admin_audit_logs WHERE target_type='new_coin_project' AND target_id=? AND action='new_coin_project.issuance.update'").bind(id.to_string()).fetch_one(&pool).await?;
    assert_eq!(audits, 1);
    assert_eq!(
        app.oneshot(request(
            "GET",
            "/admin/api/v1/new-coins/18446744073709551614".into(),
            Value::Null
        ))
        .await?
        .status(),
        StatusCode::NOT_FOUND
    );
    delete_new_coin_project_fixture(&pool, id, asset, admin_id, role_id).await?;
    sqlx::query("DELETE FROM assets WHERE id=?")
        .bind(quote)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn admin_new_coin_actual_listing_gates_new_locks_without_rewriting_history()
-> Result<(), Box<dyn Error>> {
    use exchange_api::workers::unlock_scanner::release_due_unlock_positions;
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role, admin) = create_admin_user(&pool).await;
    let user = create_user(&pool).await;
    let asset = create_asset(&pool, "LGB").await;
    let quote = create_asset(&pool, "LGQ").await;
    let symbol = format!("LG{}", Uuid::now_v7().simple());
    let token = issue_token(&settings, format!("admin:{admin}"), TokenScope::Admin, 900)?;
    let user_token = issue_token(&settings, format!("user:{user}"), TokenScope::User, 900)?;
    let app = build_router(AppState::new(settings.clone()).with_mysql(pool.clone()));
    let user_app = exchange_api::modules::new_coin::routes::user_routes()
        .with_state(AppState::new(settings).with_mysql(pool.clone()));
    let request = |method: &str, path: String, payload: Value| {
        Request::builder()
            .method(method)
            .uri(path)
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap()
    };
    let past = 946684800000_i64;
    let future = chrono::Utc::now().timestamp_millis() + 86_400_000;
    let created=app.clone().oneshot(request("POST","/admin/api/v1/new-coins".into(),json!({"asset_id":asset,"symbol":symbol,"quote_asset_id":quote,"lifecycle_status":"preheat","total_supply":"100","issue_price":"2.5","unlock_type":"immediate_on_listing","listed_at":past,"unlock_fee_enabled":false,"reason":"actual listing regression"}))).await?;
    assert_eq!(created.status(), StatusCode::OK);
    let created = body_json(created).await?;
    let project = created["id"].as_u64().unwrap();
    assert_eq!(created["listed_at"], past);
    assert!(created["actual_listed_at"].is_null());
    let lifecycle = |stage: &str| {
        request(
            "PATCH",
            format!("/admin/api/v1/new-coins/{project}/lifecycle"),
            json!({"lifecycle_status":stage,"reason":"actual event"}),
        )
    };
    assert_eq!(
        app.clone()
            .oneshot(lifecycle("subscription"))
            .await?
            .status(),
        StatusCode::OK
    );
    sqlx::query("INSERT INTO wallet_accounts (user_id,asset_id,available) VALUES (?,?,100)")
        .bind(user)
        .bind(quote)
        .execute(&pool)
        .await?;
    let subscription_key = format!("listing-sub-{}", Uuid::now_v7());
    let sub=user_app.clone().oneshot(Request::builder().method("POST").uri(format!("/new-coins/{symbol}/subscriptions")).header(AUTHORIZATION,format!("Bearer {user_token}")).header("content-type","application/json").body(Body::from(json!({"quote_asset_id":quote,"quote_amount":"25","quantity":"10","idempotency_key":subscription_key}).to_string()))?).await?;
    assert_eq!(sub.status(), StatusCode::OK);
    let subscription: u64 =
        sqlx::query_scalar("SELECT id FROM new_coin_subscriptions WHERE idempotency_key=?")
            .bind(&subscription_key)
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        app.clone()
            .oneshot(lifecycle("distribution"))
            .await?
            .status(),
        StatusCode::OK
    );
    let allocate = |quantity: &str, subscription_id: Option<u64>| {
        request(
            "POST",
            format!("/admin/api/v1/new-coins/{project}/distribute"),
            json!({"user_id":user,"quantity":quantity,"subscription_id":subscription_id,"idempotency_key":Uuid::now_v7().to_string(),"reason":"listing-gated allocation"}),
        )
    };
    for payload in [allocate("4", Some(subscription)), allocate("2", None)] {
        let response = app.clone().oneshot(payload).await?;
        let status = response.status();
        let body = body_json(response).await?;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(body["lock_position_id"].is_number());
    }
    type ListingLockSnapshot = (
        u64,
        Option<u64>,
        chrono::DateTime<chrono::Utc>,
        String,
        BigDecimal,
    );
    let locks: Vec<ListingLockSnapshot> = sqlx::query_as("SELECT id,listing_project_id,unlock_at,merge_key,remaining_amount FROM asset_lock_positions WHERE user_id=? AND asset_id=?").bind(user).bind(asset).fetch_all(&pool).await?;
    assert_eq!(locks.len(), 1);
    assert_eq!(locks[0].1, Some(project));
    assert_eq!(locks[0].4, decimal("6"));
    let gate = locks[0].clone();
    let unlock_keys: Vec<String> = sqlx::query_scalar(
        "SELECT idempotency_key FROM asset_unlock_records WHERE lock_position_id=?",
    )
    .bind(gate.0)
    .fetch_all(&pool)
    .await?;
    assert_eq!(unlock_keys.len(), 2);
    let release = |key: &str| {
        Request::builder()
            .method("POST")
            .uri(format!("/new-coins/unlocks/{key}/release"))
            .header(AUTHORIZATION, format!("Bearer {user_token}"))
            .body(Body::empty())
            .unwrap()
    };
    for key in &unlock_keys {
        assert_eq!(
            user_app.clone().oneshot(release(key)).await?.status(),
            StatusCode::BAD_REQUEST
        );
    }
    release_due_unlock_positions(&pool, chrono::Utc::now(), 100).await?;
    let held: BigDecimal =
        sqlx::query_scalar("SELECT locked FROM wallet_accounts WHERE user_id=? AND asset_id=?")
            .bind(user)
            .bind(asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(held, decimal("6"));
    let list = app
        .clone()
        .oneshot(request(
            "GET",
            format!("/admin/api/v1/new-coins/lock-positions?user_id={user}&asset_id={asset}"),
            Value::Null,
        ))
        .await?;
    assert_eq!(list.status(), StatusCode::OK);
    let list = body_json(list).await?;
    assert_eq!(list["lock_positions"][0]["listing_project_id"], project);
    assert!(list["lock_positions"][0]["unlock_at"].is_null());
    assert!(list["lock_positions"][0]["actual_listing_at"].is_null());
    // Later fixed-time rule affects only a new allocation; it must not release the pinned listing gate.
    let fixed=app.clone().oneshot(request("PATCH",format!("/admin/api/v1/new-coins/{project}/unlock-rule"),json!({"unlock_type":"fixed_time","fixed_unlock_at":past,"reason":"future allocations only"}))).await?;
    assert_eq!(fixed.status(), StatusCode::OK);
    assert_eq!(
        app.clone().oneshot(allocate("3", None)).await?.status(),
        StatusCode::OK
    );
    release_due_unlock_positions(&pool, chrono::Utc::now(), 100).await?;
    let balances: (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available,locked FROM wallet_accounts WHERE user_id=? AND asset_id=?",
    )
    .bind(user)
    .bind(asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(balances, (decimal("3"), decimal("6")));
    let historic:(u64,chrono::DateTime<chrono::Utc>,Option<u64>,String)=sqlx::query_as("SELECT id,unlock_at,listing_project_id,status FROM asset_lock_positions WHERE user_id=? AND asset_id=? AND listing_project_id IS NULL").bind(user).bind(asset).fetch_one(&pool).await?;
    assert_eq!(historic.1.timestamp_millis(), past);
    assert_eq!(historic.3, "released");
    let plan=app.clone().oneshot(request("PATCH",format!("/admin/api/v1/new-coins/{project}/unlock-rule"),json!({"unlock_type":"immediate_on_listing","listed_at":future,"reason":"plan is not an event"}))).await?;
    assert_eq!(plan.status(), StatusCode::OK);
    let unchanged: ListingLockSnapshot = sqlx::query_as("SELECT id,listing_project_id,unlock_at,merge_key,remaining_amount FROM asset_lock_positions WHERE id=?").bind(gate.0).fetch_one(&pool).await?;
    assert_eq!(unchanged, gate);
    let spoof=app.clone().oneshot(request("PATCH",format!("/admin/api/v1/new-coins/{project}/lifecycle"),json!({"lifecycle_status":"listed","listed_at":past,"reason":"reject manual actual timestamp"}))).await?;
    assert_eq!(spoof.status(), StatusCode::BAD_REQUEST);
    let trigger = format!("listing_audit_failure_{admin}");
    sqlx::raw_sql(&format!("CREATE TRIGGER {trigger} BEFORE INSERT ON admin_audit_logs FOR EACH ROW BEGIN IF NEW.admin_id={admin} AND NEW.action='new_coin_project.lifecycle.update' THEN SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='test listing audit rollback'; END IF; END")).execute(&pool).await?;
    let failed = app.clone().oneshot(lifecycle("listed")).await?;
    sqlx::raw_sql(&format!("DROP TRIGGER {trigger}"))
        .execute(&pool)
        .await?;
    assert_eq!(failed.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let failed_event: (String, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as(
        "SELECT lifecycle_status,actual_listed_at FROM new_coin_projects WHERE id=?",
    )
    .bind(project)
    .fetch_one(&pool)
    .await?;
    assert_eq!(failed_event, ("distribution".into(), None));
    assert_eq!(
        user_app
            .clone()
            .oneshot(release(&unlock_keys[0]))
            .await?
            .status(),
        StatusCode::BAD_REQUEST
    );
    let started = chrono::Utc::now().timestamp_millis();
    let listed = app.clone().oneshot(lifecycle("listed")).await?;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed = body_json(listed).await?;
    assert_eq!(listed["listed_at"], future);
    let actual = listed["actual_listed_at"].as_i64().unwrap();
    assert!(actual >= started && actual <= chrono::Utc::now().timestamp_millis());
    let (manual, worker) = tokio::join!(
        user_app.clone().oneshot(release(&unlock_keys[0])),
        release_due_unlock_positions(&pool, chrono::Utc::now(), 100)
    );
    assert_eq!(manual?.status(), StatusCode::OK);
    worker?;
    release_due_unlock_positions(&pool, chrono::Utc::now(), 100).await?;
    // New grants remain distribution-only. Listed holdings enter through the separate purchase flow.
    assert_eq!(
        app.clone().oneshot(allocate("2", None)).await?.status(),
        StatusCode::BAD_REQUEST
    );
    let pair=sqlx::query("INSERT INTO trading_pairs (base_asset,quote_asset,symbol,price_precision,qty_precision,min_order_value,status,market_type) VALUES (?,?,?,8,8,1,'disabled','spot')").bind(asset).bind(quote).bind(format!("LGP{}",Uuid::now_v7().simple())).execute(&pool).await?.last_insert_id();
    let enabled = app
        .clone()
        .oneshot(request(
            "PATCH",
            format!("/admin/api/v1/new-coins/{project}/post-listing-purchase"),
            json!({"enabled":true,"pair_id":pair,"reason":"listed purchase test"}),
        ))
        .await?;
    assert_eq!(enabled.status(), StatusCode::OK);
    let purchase=user_app.clone().oneshot(Request::builder().method("POST").uri(format!("/new-coins/{symbol}/purchase")).header(AUTHORIZATION,format!("Bearer {user_token}")).header("content-type","application/json").body(Body::from(json!({"pair_id":pair,"price":"2.5","quantity":"2","idempotency_key":Uuid::now_v7().to_string()}).to_string()))?).await?;
    let purchase_status = purchase.status();
    let purchase = body_json(purchase).await?;
    assert_eq!(purchase_status, StatusCode::OK, "{purchase}");
    assert_eq!(purchase["status"], "available");
    assert!(purchase["lock_position_id"].is_null());
    let balances: (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available,locked FROM wallet_accounts WHERE user_id=? AND asset_id=?",
    )
    .bind(user)
    .bind(asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(balances, (decimal("11"), decimal("0")));
    let quote_balances: (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available,frozen FROM wallet_accounts WHERE user_id=? AND asset_id=?",
    )
    .bind(user)
    .bind(quote)
    .fetch_one(&pool)
    .await?;
    assert_eq!(quote_balances, (decimal("85"), decimal("0")));
    let historic_after: (u64, chrono::DateTime<chrono::Utc>, Option<u64>, String) = sqlx::query_as(
        "SELECT id,unlock_at,listing_project_id,status FROM asset_lock_positions WHERE id=?",
    )
    .bind(historic.0)
    .fetch_one(&pool)
    .await?;
    assert_eq!(historic_after, historic);
    for key in &unlock_keys {
        let count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM wallet_ledger WHERE change_type='new_coin_unlock_release' AND ref_id=?").bind(key).fetch_one(&pool).await?;
        assert_eq!(count, 2);
    }
    let list = body_json(
        app.clone()
            .oneshot(request(
                "GET",
                format!("/admin/api/v1/new-coins/lock-positions?user_id={user}&asset_id={asset}"),
                Value::Null,
            ))
            .await?,
    )
    .await?;
    let gated = list["lock_positions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == gate.0)
        .unwrap();
    assert_eq!(gated["unlock_at"], actual);
    assert_eq!(gated["actual_listing_at"], actual);
    sqlx::query("DELETE FROM new_coin_purchase_orders WHERE project_id=?")
        .bind(project)
        .execute(&pool)
        .await?;
    sqlx::query("UPDATE new_coin_projects SET post_listing_pair_id=NULL WHERE id=?")
        .bind(project)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM trading_pairs WHERE id=?")
        .bind(pair)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM new_coin_distributions WHERE project_id=?")
        .bind(project)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM new_coin_subscriptions WHERE project_id=?")
        .bind(project)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_ledger WHERE user_id=? AND asset_id=?")
        .bind(user)
        .bind(quote)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id=? AND asset_id=?")
        .bind(user)
        .bind(quote)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id=? AND target_type='new_coin_project'")
        .bind(admin)
        .execute(&pool)
        .await?;
    delete_new_coin_distribution_fixture(&pool, project, asset, user, admin, role).await?;
    sqlx::query("DELETE FROM assets WHERE id=?")
        .bind(quote)
        .execute(&pool)
        .await?;
    Ok(())
}
