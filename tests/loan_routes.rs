use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, TimeDelta, Utc};
use exchange_api::{
    build_router,
    config::Settings,
    modules::{
        auth::{TokenScope, issue_token},
        loan::routes::user_routes,
    },
    state::AppState,
};
use secrecy::SecretString;
use serde_json::{Value, json};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions, types::Json as SqlxJson};
use std::{error::Error, str::FromStr, time::Duration};
use tower::ServiceExt;
use uuid::Uuid;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

async fn body_json(response: axum::response::Response) -> Result<Value, Box<dyn Error>> {
    let body = axum::body::to_bytes(response.into_body(), 1_048_576).await?;
    Ok(serde_json::from_slice(&body)?)
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
            eprintln!("skipping MySQL loan route test because DATABASE_URL is not set");
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

struct LoanFixture {
    user_id: u64,
    asset_id: u64,
    collateral_asset_id: u64,
    product_id: u64,
    order_id: u64,
}

struct LoanProductFilterFixture {
    asset_id: u64,
    product_ids: [u64; 4],
}

struct LoanProductGovernanceFixture {
    admin_id: u64,
    role_id: u64,
    asset_id: u64,
}

#[derive(Debug, sqlx::FromRow)]
struct LoanProductAuditRow {
    action: String,
    before_json: Option<SqlxJson<Value>>,
    after_json: Option<SqlxJson<Value>>,
    reason: Option<String>,
    ip: Option<String>,
    request_id: Option<String>,
}

async fn create_asset(pool: &MySqlPool, prefix: &str) -> Result<u64, sqlx::Error> {
    let suffix = Uuid::now_v7().simple().to_string();
    let symbol = format!("{prefix}{}", &suffix[suffix.len() - 10..]).to_ascii_uppercase();
    Ok(sqlx::query(
        "INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, 18, 'coin', 'active')",
    )
    .bind(&symbol)
    .bind(&symbol)
    .execute(pool)
    .await?
    .last_insert_id())
}

async fn seed_loan_product_governance_fixture(
    pool: &MySqlPool,
) -> Result<LoanProductGovernanceFixture, sqlx::Error> {
    let suffix = Uuid::now_v7().simple().to_string();
    let role_id = sqlx::query("INSERT INTO admin_roles (name, permissions) VALUES (?, ?)")
        .bind(format!("loan-governance-role-{suffix}"))
        .bind(SqlxJson(json!(["*"])))
        .execute(pool)
        .await?
        .last_insert_id();
    let admin_id = sqlx::query(
        "INSERT INTO admin_users (username, password_hash, role_id, status) VALUES (?, ?, ?, 'active')",
    )
    .bind(format!("loan-governance-{suffix}"))
    .bind("not-a-real-hash")
    .bind(role_id)
    .execute(pool)
    .await?
    .last_insert_id();
    let asset_id = create_asset(pool, "LNG").await?;
    Ok(LoanProductGovernanceFixture {
        admin_id,
        role_id,
        asset_id,
    })
}

async fn cleanup_loan_product_governance_fixture(
    pool: &MySqlPool,
    fixture: &LoanProductGovernanceFixture,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(fixture.admin_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM loan_products WHERE asset_id = ?")
        .bind(fixture.asset_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(fixture.admin_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(fixture.role_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(fixture.asset_id)
        .execute(pool)
        .await?;
    Ok(())
}

fn loan_product_write_body(asset_id: u64, name: &str, reason: &str) -> Value {
    json!({
        "loan_type": "credit",
        "asset_id": asset_id,
        "name": name,
        "name_json": {
            "version": 1,
            "default_locale": "zh-CN",
            "items": [{ "locale": "zh-CN", "country": "CN", "title": name }]
        },
        "term_days": 30,
        "interest_rate": "0.02",
        "interest_calculation_mode": "full_term",
        "min_kyc_level": 0,
        "min_amount": "1",
        "max_amount": "1000",
        "status": "active",
        "reason": reason
    })
}

async fn write_admin_loan_product(
    app: axum::Router,
    method: &str,
    uri: &str,
    token: &str,
    request_id: &str,
    body: Value,
) -> Result<(StatusCode, Option<String>, Value), Box<dyn Error>> {
    let response = app
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .header("cf-connecting-ip", "203.0.113.42")
                .header("x-request-id", request_id)
                .body(Body::from(body.to_string()))?,
        )
        .await?;
    let status = response.status();
    let response_request_id = response
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    Ok((status, response_request_id, body_json(response).await?))
}

fn assert_no_sensitive_audit_keys(value: &Value) {
    match value {
        Value::Array(values) => values.iter().for_each(assert_no_sensitive_audit_keys),
        Value::Object(values) => {
            for (key, value) in values {
                assert!(
                    !matches!(
                        key.as_str(),
                        "password"
                            | "password_hash"
                            | "secret"
                            | "token"
                            | "credential"
                            | "api_key"
                            | "private_key"
                    ),
                    "sensitive key leaked into loan audit snapshot: {key}"
                );
                assert_no_sensitive_audit_keys(value);
            }
        }
        _ => {}
    }
}

async fn seed_loan_product_filter_fixture(
    pool: &MySqlPool,
) -> Result<LoanProductFilterFixture, sqlx::Error> {
    let asset_id = create_asset(pool, "LNF").await?;
    let suffix = Uuid::now_v7().simple().to_string();
    let mut product_ids = [0_u64; 4];
    for (index, (loan_type, status)) in [
        ("credit", "active"),
        ("credit", "disabled"),
        ("collateralized", "active"),
        ("collateralized", "disabled"),
    ]
    .into_iter()
    .enumerate()
    {
        let name = format!("loan-filter-{suffix}-{loan_type}-{status}");
        product_ids[index] = sqlx::query(
            r#"INSERT INTO loan_products
               (loan_type, asset_id, name, name_json, term_days, interest_rate,
                interest_calculation_mode, min_kyc_level, min_amount, max_amount, status)
               VALUES (?, ?, ?, ?, 30, 0.02, 'full_term', 0, 1, NULL, ?)"#,
        )
        .bind(loan_type)
        .bind(asset_id)
        .bind(&name)
        .bind(SqlxJson(json!({
            "version": 1,
            "default_locale": "zh-CN",
            "items": [{ "locale": "zh-CN", "country": "CN", "title": name }]
        })))
        .bind(status)
        .execute(pool)
        .await?
        .last_insert_id();
    }
    Ok(LoanProductFilterFixture {
        asset_id,
        product_ids,
    })
}

async fn cleanup_loan_product_filter_fixture(
    pool: &MySqlPool,
    fixture: &LoanProductFilterFixture,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM loan_products WHERE asset_id = ?")
        .bind(fixture.asset_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(fixture.asset_id)
        .execute(pool)
        .await?;
    Ok(())
}

async fn loan_product_count(
    pool: &MySqlPool,
    loan_type: Option<&str>,
    status: Option<&str>,
) -> Result<i64, sqlx::Error> {
    let mut builder =
        sqlx::QueryBuilder::<sqlx::MySql>::new("SELECT COUNT(*) FROM loan_products WHERE 1 = 1");
    if let Some(loan_type) = loan_type {
        builder.push(" AND loan_type = ");
        builder.push_bind(loan_type.to_owned());
    }
    if let Some(status) = status {
        builder.push(" AND status = ");
        builder.push_bind(status.to_owned());
    }
    builder.build_query_scalar().fetch_one(pool).await
}

async fn admin_loan_products(
    app: axum::Router,
    query: &str,
) -> Result<(StatusCode, Value), Box<dyn Error>> {
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/admin/api/v1/loan/products?{query}"))
                .body(Body::empty())?,
        )
        .await?;
    let status = response.status();
    Ok((status, body_json(response).await?))
}

async fn public_loan_products(
    app: axum::Router,
    query: &str,
) -> Result<(StatusCode, Value), Box<dyn Error>> {
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/loan/products?{query}"))
                .body(Body::empty())?,
        )
        .await?;
    let status = response.status();
    Ok((status, body_json(response).await?))
}

/// 播种一笔抵押贷订单：状态与放款时间由调用方决定，用于分别覆盖逾期还款与非法状态还款。
async fn seed_fixture(pool: &MySqlPool, status: &str) -> Result<LoanFixture, sqlx::Error> {
    let suffix = Uuid::now_v7().simple().to_string();
    let user_id = sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(format!("loan-route-{suffix}@example.test"))
        .bind("not-a-real-hash")
        .execute(pool)
        .await?
        .last_insert_id();
    let asset_id = create_asset(pool, "LNA").await?;
    let collateral_asset_id = create_asset(pool, "LNC").await?;
    let name = format!("loan-route-{suffix}");
    let product_id = sqlx::query(
        r#"INSERT INTO loan_products
           (loan_type, asset_id, name, name_json, term_days, interest_rate,
            interest_calculation_mode, min_kyc_level, min_amount, max_amount, status)
           VALUES ('collateralized', ?, ?, ?, 30, 0.02, 'full_term', 0, 1, NULL, 'active')"#,
    )
    .bind(asset_id)
    .bind(&name)
    .bind(SqlxJson(json!({
        "version": 1,
        "default_locale": "zh-CN",
        "items": [{ "locale": "zh-CN", "country": "CN", "title": name }]
    })))
    .execute(pool)
    .await?
    .last_insert_id();

    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, ?, 0)",
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(decimal("200.000000000000000000"))
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen) VALUES (?, ?, 0, ?)",
    )
    .bind(user_id)
    .bind(collateral_asset_id)
    .bind(decimal("50.000000000000000000"))
    .execute(pool)
    .await?;

    let now = Utc::now();
    let order_id = sqlx::query(
        r#"INSERT INTO loan_orders
           (user_id, product_id, loan_type, asset_id, amount, interest_rate,
            interest_calculation_mode, term_days, min_kyc_level, collateral_asset_id,
            collateral_amount, status, idempotency_key, disbursed_at, due_at, overdue_at)
           VALUES (?, ?, 'collateralized', ?, ?, 0.02, 'full_term', 30, 0, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(asset_id)
    .bind(decimal("100.000000000000000000"))
    .bind(collateral_asset_id)
    .bind(decimal("50.000000000000000000"))
    .bind(status)
    .bind(format!("loan-route-{suffix}"))
    .bind((status != "pending").then(|| (now - TimeDelta::days(30)).naive_utc()))
    .bind((status != "pending").then(|| (now - TimeDelta::days(1)).naive_utc()))
    .bind((status == "overdue").then(|| (now - TimeDelta::days(1)).naive_utc()))
    .execute(pool)
    .await?
    .last_insert_id();

    Ok(LoanFixture {
        user_id,
        asset_id,
        collateral_asset_id,
        product_id,
        order_id,
    })
}

async fn cleanup_fixture(pool: &MySqlPool, fixture: &LoanFixture) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM wallet_ledger WHERE user_id = ?")
        .bind(fixture.user_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM loan_orders WHERE user_id = ?")
        .bind(fixture.user_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ?")
        .bind(fixture.user_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM loan_products WHERE id = ?")
        .bind(fixture.product_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(fixture.user_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id IN (?, ?)")
        .bind(fixture.asset_id)
        .bind(fixture.collateral_asset_id)
        .execute(pool)
        .await?;
    Ok(())
}

async fn wallet_balances(
    pool: &MySqlPool,
    user_id: u64,
    asset_id: u64,
) -> Result<(BigDecimal, BigDecimal), sqlx::Error> {
    sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(pool)
    .await
}

async fn order_ledger(
    pool: &MySqlPool,
    order_id: u64,
) -> Result<Vec<(String, String, BigDecimal, u64)>, sqlx::Error> {
    sqlx::query_as(
        r#"SELECT change_type, balance_type, amount, asset_id
           FROM wallet_ledger
           WHERE ref_type = 'loan_order' AND ref_id = ?
           ORDER BY id ASC"#,
    )
    .bind(order_id.to_string())
    .fetch_all(pool)
    .await
}

async fn repay(
    pool: &MySqlPool,
    settings: &Settings,
    fixture: &LoanFixture,
) -> Result<(StatusCode, Value), Box<dyn Error>> {
    let token = issue_token(
        settings,
        format!("user:{}", fixture.user_id),
        TokenScope::User,
        900,
    )
    .unwrap();
    let app = user_routes().with_state(AppState::new(settings.clone()).with_mysql(pool.clone()));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/loan/orders/{}/repay", fixture.order_id))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::empty())?,
        )
        .await?;
    let status = response.status();
    Ok((status, body_json(response).await?))
}

#[tokio::test]
async fn admin_loan_products_filter_rows_and_total_by_type_and_status() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let fixture = seed_loan_product_filter_fixture(&pool).await?;
    let app = build_router(AppState::new(test_settings()).with_mysql(pool.clone()));

    let outcome: Result<(), Box<dyn Error>> = async {
        let (status, credit_payload) =
            admin_loan_products(app.clone(), "loan_type=%20credit%20&limit=200").await?;
        assert_eq!(status, StatusCode::OK, "payload: {credit_payload}");
        assert_eq!(
            credit_payload["total"],
            loan_product_count(&pool, Some("credit"), None).await?
        );
        let credit_products = credit_payload["products"].as_array().unwrap();
        assert!(
            credit_products
                .iter()
                .all(|product| product["loan_type"] == "credit")
        );
        let credit_ids = credit_products
            .iter()
            .filter_map(|product| product["id"].as_u64())
            .collect::<Vec<_>>();
        assert!(credit_ids.contains(&fixture.product_ids[0]));
        assert!(credit_ids.contains(&fixture.product_ids[1]));
        assert!(!credit_ids.contains(&fixture.product_ids[2]));
        assert!(!credit_ids.contains(&fixture.product_ids[3]));

        let (status, disabled_payload) =
            admin_loan_products(app.clone(), "status=disabled&limit=200").await?;
        assert_eq!(status, StatusCode::OK, "payload: {disabled_payload}");
        assert_eq!(
            disabled_payload["total"],
            loan_product_count(&pool, None, Some("disabled")).await?
        );
        let disabled_products = disabled_payload["products"].as_array().unwrap();
        assert!(
            disabled_products
                .iter()
                .all(|product| product["status"] == "disabled")
        );
        let disabled_ids = disabled_products
            .iter()
            .filter_map(|product| product["id"].as_u64())
            .collect::<Vec<_>>();
        assert!(!disabled_ids.contains(&fixture.product_ids[0]));
        assert!(disabled_ids.contains(&fixture.product_ids[1]));
        assert!(!disabled_ids.contains(&fixture.product_ids[2]));
        assert!(disabled_ids.contains(&fixture.product_ids[3]));

        let (status, combined_payload) =
            admin_loan_products(app.clone(), "loan_type=credit&status=disabled&limit=200").await?;
        assert_eq!(status, StatusCode::OK, "payload: {combined_payload}");
        assert_eq!(
            combined_payload["total"],
            loan_product_count(&pool, Some("credit"), Some("disabled")).await?
        );
        let combined_products = combined_payload["products"].as_array().unwrap();
        assert!(combined_products.iter().all(|product| {
            product["loan_type"] == "credit" && product["status"] == "disabled"
        }));
        let combined_ids = combined_products
            .iter()
            .filter_map(|product| product["id"].as_u64())
            .collect::<Vec<_>>();
        assert!(!combined_ids.contains(&fixture.product_ids[0]));
        assert!(combined_ids.contains(&fixture.product_ids[1]));
        assert!(!combined_ids.contains(&fixture.product_ids[2]));
        assert!(!combined_ids.contains(&fixture.product_ids[3]));

        let (status, blank_payload) =
            admin_loan_products(app.clone(), "loan_type=%20%20&status=&limit=200").await?;
        assert_eq!(status, StatusCode::OK, "payload: {blank_payload}");
        assert_eq!(
            blank_payload["total"],
            loan_product_count(&pool, None, None).await?
        );
        let blank_ids = blank_payload["products"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|product| product["id"].as_u64())
            .collect::<Vec<_>>();
        for product_id in fixture.product_ids {
            assert!(blank_ids.contains(&product_id));
        }

        let (status, public_payload) = public_loan_products(app, "limit=200").await?;
        assert_eq!(status, StatusCode::OK, "payload: {public_payload}");
        let public_products = public_payload["products"].as_array().unwrap();
        assert!(
            public_products
                .iter()
                .all(|product| product["status"] == "active")
        );
        let public_ids = public_products
            .iter()
            .filter_map(|product| product["id"].as_u64())
            .collect::<Vec<_>>();
        assert!(public_ids.contains(&fixture.product_ids[0]));
        assert!(!public_ids.contains(&fixture.product_ids[1]));
        assert!(public_ids.contains(&fixture.product_ids[2]));
        assert!(!public_ids.contains(&fixture.product_ids[3]));
        Ok(())
    }
    .await;

    cleanup_loan_product_filter_fixture(&pool, &fixture).await?;
    outcome
}

#[tokio::test]
async fn admin_loan_products_reject_invalid_enum_filters_before_query() -> Result<(), Box<dyn Error>>
{
    // 不可连接的惰性连接池可证明非法枚举在执行 SQL 前即被拒绝。
    let pool = MySqlPoolOptions::new()
        .acquire_timeout(Duration::from_millis(100))
        .connect_lazy("mysql://test:test@127.0.0.1:1/test")?;
    let app = build_router(AppState::new(test_settings()).with_mysql(pool));

    for (query, expected_message) in [
        ("loan_type=margin", "unsupported loan_type"),
        ("status=pending", "unsupported loan product status"),
    ] {
        let (status, payload) = admin_loan_products(app.clone(), query).await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "payload: {payload}");
        assert_eq!(payload["code"], "VALIDATION_ERROR");
        assert_eq!(
            payload["message"],
            format!("validation error: {expected_message}")
        );
    }
    Ok(())
}

#[tokio::test]
async fn admin_loan_product_writes_are_revision_guarded_and_audited_in_one_transaction()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let fixture = seed_loan_product_governance_fixture(&pool).await?;
    let token = issue_token(
        &settings,
        format!("admin:{}", fixture.admin_id),
        TokenScope::Admin,
        900,
    )?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let suffix = Uuid::now_v7().simple().to_string();
    let create_request_id = format!("loan-create-{}", &suffix[suffix.len() - 8..]);
    let update_a_request_id = format!("loan-update-a-{}", &suffix[suffix.len() - 8..]);
    let update_b_request_id = format!("loan-update-b-{}", &suffix[suffix.len() - 8..]);
    let status_request_id = format!("loan-status-{}", &suffix[suffix.len() - 8..]);

    let outcome: Result<(), Box<dyn Error>> = async {
        let (blank_status, _, blank_payload) = write_admin_loan_product(
            app.clone(),
            "POST",
            "/admin/api/v1/loan/products",
            &token,
            "loan-create-blank",
            loan_product_write_body(fixture.asset_id, "空原因产品", "   "),
        )
        .await?;
        assert_eq!(blank_status, StatusCode::BAD_REQUEST, "{blank_payload}");
        assert_eq!(blank_payload["code"], "VALIDATION_ERROR");

        let mut create_body =
            loan_product_write_body(fixture.asset_id, "并发治理贷款", "  创建贷款产品  ");
        // 产品名称结构允许前向兼容扩展键，审计快照仍必须深层白名单化，不能记录这些明文。
        create_body["name_json"]["token"] = json!("must-not-enter-audit");
        create_body["name_json"]["items"][0]["private_key"] = json!("must-not-enter-audit");
        let (create_status, response_request_id, create_payload) = write_admin_loan_product(
            app.clone(),
            "POST",
            "/admin/api/v1/loan/products",
            &token,
            &create_request_id,
            create_body,
        )
        .await?;
        assert_eq!(create_status, StatusCode::OK, "{create_payload}");
        assert_eq!(
            response_request_id.as_deref(),
            Some(create_request_id.as_str())
        );
        assert_eq!(create_payload["revision"], 1);
        let product_id = create_payload["id"].as_u64().expect("created product id");
        let product_uri = format!("/admin/api/v1/loan/products/{product_id}");
        let product_status_uri = format!("{product_uri}/status");

        let missing_revision_body =
            loan_product_write_body(fixture.asset_id, "缺版本更新", "缺少版本");
        let (missing_revision_status, _, missing_revision_payload) = write_admin_loan_product(
            app.clone(),
            "PATCH",
            &product_uri,
            &token,
            "loan-update-missing-revision",
            missing_revision_body,
        )
        .await?;
        assert_eq!(
            missing_revision_status,
            StatusCode::BAD_REQUEST,
            "{missing_revision_payload}"
        );
        assert_eq!(missing_revision_payload["code"], "VALIDATION_ERROR");

        let mut blank_update_body = loan_product_write_body(fixture.asset_id, "空原因更新", "   ");
        blank_update_body["revision"] = json!(1);
        let (blank_update_status, _, blank_update_payload) = write_admin_loan_product(
            app.clone(),
            "PATCH",
            &product_uri,
            &token,
            "loan-update-blank-reason",
            blank_update_body,
        )
        .await?;
        assert_eq!(
            blank_update_status,
            StatusCode::BAD_REQUEST,
            "{blank_update_payload}"
        );

        let mut update_a_body =
            loan_product_write_body(fixture.asset_id, "并发更新结果A", "  并发更新A  ");
        update_a_body["revision"] = json!(1);
        let mut update_b_body =
            loan_product_write_body(fixture.asset_id, "并发更新结果B", "  并发更新B  ");
        update_b_body["revision"] = json!(1);
        let (update_a, update_b) = tokio::join!(
            write_admin_loan_product(
                app.clone(),
                "PATCH",
                &product_uri,
                &token,
                &update_a_request_id,
                update_a_body,
            ),
            write_admin_loan_product(
                app.clone(),
                "PATCH",
                &product_uri,
                &token,
                &update_b_request_id,
                update_b_body,
            )
        );
        let updates = [update_a?, update_b?];
        assert_eq!(
            updates
                .iter()
                .filter(|(status, _, _)| *status == StatusCode::OK)
                .count(),
            1
        );
        assert_eq!(
            updates
                .iter()
                .filter(|(status, _, _)| *status == StatusCode::CONFLICT)
                .count(),
            1
        );
        let successful_update = updates
            .iter()
            .find(|(status, _, _)| *status == StatusCode::OK)
            .expect("one concurrent update succeeds");
        assert_eq!(successful_update.2["revision"], 2);
        let rejected_update = updates
            .iter()
            .find(|(status, _, _)| *status == StatusCode::CONFLICT)
            .expect("one concurrent update conflicts");
        assert_eq!(rejected_update.2["code"], "CONFLICT");

        let (missing_status_revision, _, missing_status_payload) = write_admin_loan_product(
            app.clone(),
            "PATCH",
            &product_status_uri,
            &token,
            "loan-status-missing-revision",
            json!({ "status": "disabled", "reason": "缺少状态版本" }),
        )
        .await?;
        assert_eq!(
            missing_status_revision,
            StatusCode::BAD_REQUEST,
            "{missing_status_payload}"
        );

        let (blank_status_reason, _, blank_status_payload) = write_admin_loan_product(
            app.clone(),
            "PATCH",
            &product_status_uri,
            &token,
            "loan-status-blank-reason",
            json!({ "status": "disabled", "reason": "   ", "revision": 2 }),
        )
        .await?;
        assert_eq!(
            blank_status_reason,
            StatusCode::BAD_REQUEST,
            "{blank_status_payload}"
        );

        let (status_status, response_request_id, status_payload) = write_admin_loan_product(
            app.clone(),
            "PATCH",
            &product_status_uri,
            &token,
            &status_request_id,
            json!({ "status": "disabled", "reason": "  下架贷款产品  ", "revision": 2 }),
        )
        .await?;
        assert_eq!(status_status, StatusCode::OK, "{status_payload}");
        assert_eq!(
            response_request_id.as_deref(),
            Some(status_request_id.as_str())
        );
        assert_eq!(status_payload["revision"], 3);
        assert_eq!(status_payload["status"], "disabled");

        let stored: (String, u64) =
            sqlx::query_as("SELECT status, revision FROM loan_products WHERE id = ?")
                .bind(product_id)
                .fetch_one(&pool)
                .await?;
        assert_eq!(stored, ("disabled".to_owned(), 3));

        let audits = sqlx::query_as::<_, LoanProductAuditRow>(
            r#"SELECT action, before_json, after_json, reason, ip, request_id
               FROM admin_audit_logs
               WHERE admin_id = ? AND target_type = 'loan_product' AND target_id = ?
               ORDER BY id ASC"#,
        )
        .bind(fixture.admin_id)
        .bind(product_id.to_string())
        .fetch_all(&pool)
        .await?;
        assert_eq!(
            audits.len(),
            3,
            "conflicts and invalid requests must not write audit rows"
        );
        assert_eq!(audits[0].action, "loan_product.create");
        assert!(audits[0].before_json.is_none());
        assert_eq!(audits[0].reason.as_deref(), Some("创建贷款产品"));
        assert_eq!(audits[0].ip.as_deref(), Some("203.0.113.42"));
        assert_eq!(
            audits[0].request_id.as_deref(),
            Some(create_request_id.as_str())
        );
        assert_eq!(audits[0].after_json.as_ref().unwrap().0["revision"], 1);

        assert_eq!(audits[1].action, "loan_product.update");
        assert_eq!(audits[1].before_json.as_ref().unwrap().0["revision"], 1);
        assert_eq!(audits[1].after_json.as_ref().unwrap().0["revision"], 2);
        let (expected_update_request_id, expected_update_reason) = match audits[1].reason.as_deref()
        {
            Some("并发更新A") => (update_a_request_id.as_str(), "并发更新A"),
            Some("并发更新B") => (update_b_request_id.as_str(), "并发更新B"),
            reason => panic!("unexpected update audit reason: {reason:?}"),
        };
        assert_eq!(audits[1].reason.as_deref(), Some(expected_update_reason));
        assert_eq!(
            audits[1].request_id.as_deref(),
            Some(expected_update_request_id)
        );

        assert_eq!(audits[2].action, "loan_product.update_status");
        assert_eq!(audits[2].before_json.as_ref().unwrap().0["revision"], 2);
        assert_eq!(audits[2].after_json.as_ref().unwrap().0["revision"], 3);
        assert_eq!(audits[2].reason.as_deref(), Some("下架贷款产品"));
        assert_eq!(
            audits[2].request_id.as_deref(),
            Some(status_request_id.as_str())
        );

        for audit in &audits {
            assert_eq!(audit.ip.as_deref(), Some("203.0.113.42"));
            if let Some(before) = audit.before_json.as_ref() {
                assert_no_sensitive_audit_keys(&before.0);
            }
            if let Some(after) = audit.after_json.as_ref() {
                assert_no_sensitive_audit_keys(&after.0);
            }
        }
        Ok(())
    }
    .await;

    cleanup_loan_product_governance_fixture(&pool, &fixture).await?;
    outcome
}

#[tokio::test]
async fn loan_repay_settles_overdue_order_releases_collateral_and_is_idempotent()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let fixture = seed_fixture(&pool, "overdue").await?;

    // 断言体单独求值，保证夹具在任何提前返回路径上都会被清理。
    let outcome: Result<(), Box<dyn Error>> = async {
        let (status, payload) = repay(&pool, &settings, &fixture).await?;
        assert_eq!(status, StatusCode::OK, "payload: {payload}");
        assert_eq!(payload["changed"], true);
        assert_eq!(payload["order"]["status"], "repaid");
        assert_eq!(payload["order"]["interest_amount"], "2.000000000000000000");
        assert_eq!(
            payload["order"]["repayment_amount"],
            "102.000000000000000000"
        );
        assert!(payload["order"]["repaid_at"].is_number());
        assert!(payload["order"]["collateral_released_at"].is_number());
        assert!(payload["order"]["overdue_at"].is_number());

        // 逾期订单还款后，借款资产按本息扣减，抵押资产从冻结全额解冻。
        let (loan_available, loan_frozen) =
            wallet_balances(&pool, fixture.user_id, fixture.asset_id).await?;
        assert_eq!(loan_available, decimal("98.000000000000000000"));
        assert_eq!(loan_frozen, decimal("0.000000000000000000"));
        let (collateral_available, collateral_frozen) =
            wallet_balances(&pool, fixture.user_id, fixture.collateral_asset_id).await?;
        assert_eq!(collateral_available, decimal("50.000000000000000000"));
        assert_eq!(collateral_frozen, decimal("0.000000000000000000"));

        let ledger = order_ledger(&pool, fixture.order_id).await?;
        assert_eq!(
            ledger,
            vec![
                (
                    "loan_repayment".to_owned(),
                    "available".to_owned(),
                    decimal("-102.000000000000000000"),
                    fixture.asset_id,
                ),
                (
                    "loan_collateral_release".to_owned(),
                    "available".to_owned(),
                    decimal("50.000000000000000000"),
                    fixture.collateral_asset_id,
                ),
                (
                    "loan_collateral_release".to_owned(),
                    "frozen".to_owned(),
                    decimal("-50.000000000000000000"),
                    fixture.collateral_asset_id,
                ),
            ]
        );

        // 重复还款必须是无副作用的幂等返回，否则会二次扣款并重复解冻抵押。
        let (repeat_status, repeat_payload) = repay(&pool, &settings, &fixture).await?;
        assert_eq!(repeat_status, StatusCode::OK, "payload: {repeat_payload}");
        assert_eq!(repeat_payload["changed"], false);
        assert_eq!(repeat_payload["order"]["status"], "repaid");
        assert_eq!(
            repeat_payload["order"]["repayment_amount"],
            "102.000000000000000000"
        );
        assert_eq!(
            wallet_balances(&pool, fixture.user_id, fixture.asset_id).await?,
            (
                decimal("98.000000000000000000"),
                decimal("0.000000000000000000")
            )
        );
        assert_eq!(order_ledger(&pool, fixture.order_id).await?.len(), 3);
        Ok(())
    }
    .await;

    cleanup_fixture(&pool, &fixture).await?;
    outcome
}

#[tokio::test]
async fn loan_repay_rejects_order_that_was_never_disbursed() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let fixture = seed_fixture(&pool, "pending").await?;

    let outcome: Result<(), Box<dyn Error>> = async {
        let (status, payload) = repay(&pool, &settings, &fixture).await?;
        assert_eq!(status, StatusCode::CONFLICT, "payload: {payload}");

        let (order_status, collateral_released_at): (String, Option<DateTime<Utc>>) =
            sqlx::query_as("SELECT status, collateral_released_at FROM loan_orders WHERE id = ?")
                .bind(fixture.order_id)
                .fetch_one(&pool)
                .await?;
        assert_eq!(order_status, "pending");
        assert!(collateral_released_at.is_none());
        assert!(order_ledger(&pool, fixture.order_id).await?.is_empty());
        Ok(())
    }
    .await;

    cleanup_fixture(&pool, &fixture).await?;
    outcome
}
