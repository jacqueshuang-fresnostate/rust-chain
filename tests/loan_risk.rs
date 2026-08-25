use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, TimeDelta, Utc};
use exchange_api::{
    config::Settings,
    modules::{
        auth::{TokenScope, issue_token},
        loan::{
            admin_routes,
            liquidation::{LoanLiquidationOutcome, liquidate_loan_order_if_required},
            oracle::LoanOraclePrice,
            user_routes,
        },
        market::market_ticker_redis_key,
    },
    state::AppState,
    workers::loan_health,
};
use redis::AsyncCommands;
use secrecy::SecretString;
use serde_json::{Value, json};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions, types::Json as SqlxJson};
use std::{error::Error, str::FromStr};
use tower::ServiceExt;
use uuid::Uuid;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

fn response_decimal(value: &Value) -> BigDecimal {
    match value.as_str() {
        Some(value) => decimal(value),
        None => decimal(&value.to_string()),
    }
}

fn assert_decimal(actual: &BigDecimal, expected: &str) {
    assert_eq!(actual.normalized(), decimal(expected).normalized());
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

async fn dependencies() -> Result<Option<(MySqlPool, redis::aio::ConnectionManager)>, Box<dyn Error>>
{
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!("skipping loan risk integration test because DATABASE_URL is not set");
            return Ok(None);
        }
    };
    let redis_url = match std::env::var("REDIS_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!("skipping loan risk integration test because REDIS_URL is not set");
            return Ok(None);
        }
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    let redis = redis::aio::ConnectionManager::new(redis::Client::open(redis_url)?).await?;
    Ok(Some((pool, redis)))
}

async fn body_json(response: axum::response::Response) -> Result<Value, Box<dyn Error>> {
    let body = axum::body::to_bytes(response.into_body(), 1_048_576).await?;
    Ok(serde_json::from_slice(&body)?)
}

async fn request_json(
    app: Router,
    method: &'static str,
    uri: String,
    token: &str,
    body: Option<Value>,
) -> Result<(StatusCode, Value), Box<dyn Error>> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"));
    let body = match body {
        Some(body) => {
            builder = builder.header("content-type", "application/json");
            Body::from(body.to_string())
        }
        None => Body::empty(),
    };
    let response = app.oneshot(builder.body(body)?).await?;
    let status = response.status();
    Ok((status, body_json(response).await?))
}

async fn create_asset(pool: &MySqlPool, prefix: &str) -> Result<(u64, String), sqlx::Error> {
    let suffix = Uuid::now_v7().simple().to_string();
    let symbol = format!("{prefix}{}", &suffix[suffix.len() - 8..]).to_ascii_uppercase();
    let id = sqlx::query(
        r#"INSERT INTO assets (symbol, name, precision_scale, asset_type, status)
           VALUES (?, ?, 18, 'coin', 'active')"#,
    )
    .bind(&symbol)
    .bind(&symbol)
    .execute(pool)
    .await?
    .last_insert_id();
    Ok((id, symbol))
}

async fn create_user(pool: &MySqlPool, label: &str) -> Result<u64, sqlx::Error> {
    Ok(
        sqlx::query("INSERT INTO users (email, password_hash, kyc_level) VALUES (?, ?, 2)")
            .bind(format!(
                "loan-risk-{label}-{}@example.test",
                Uuid::now_v7().simple()
            ))
            .bind("not-a-real-hash")
            .execute(pool)
            .await?
            .last_insert_id(),
    )
}

async fn seed_collateral_wallet(
    pool: &MySqlPool,
    user_id: u64,
    collateral_asset_id: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO wallet_accounts (user_id, asset_id, available, frozen, locked)
           VALUES (?, ?, 100, 0, 0)"#,
    )
    .bind(user_id)
    .bind(collateral_asset_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn cache_ticker(
    redis: &redis::aio::ConnectionManager,
    symbol: &str,
    price: &str,
    observed_at: DateTime<Utc>,
) -> Result<(), redis::RedisError> {
    let mut connection = redis.clone();
    let payload = json!({
        "symbol": symbol,
        "last_price": price,
        "volume_24h": "1.000000000000000000",
        "observed_at": observed_at.timestamp_millis(),
    })
    .to_string();
    let _: () = connection
        .set(market_ticker_redis_key(symbol), payload)
        .await?;
    Ok(())
}

fn order_body(product_id: u64, collateral_asset_id: u64, idempotency_key: &str) -> Value {
    json!({
        "product_id": product_id,
        "amount": "50.000000000000000000",
        "collateral_asset_id": collateral_asset_id,
        "collateral_amount": "100.000000000000000000",
        "idempotency_key": idempotency_key,
    })
}

struct LoanRiskFixture {
    role_id: u64,
    admin_id: u64,
    user_ids: [u64; 3],
    asset_ids: [u64; 3],
    product_id: u64,
    order_ids: Vec<u64>,
    oracle_symbol: String,
}

async fn cleanup_fixture(
    pool: &MySqlPool,
    redis: &redis::aio::ConnectionManager,
    fixture: &LoanRiskFixture,
) -> Result<(), Box<dyn Error>> {
    let mut connection = redis.clone();
    let _: usize = connection
        .del(market_ticker_redis_key(&fixture.oracle_symbol))
        .await?;
    for order_id in &fixture.order_ids {
        sqlx::query(
            "DELETE FROM platform_financial_journal WHERE ref_type = 'loan_order' AND ref_id = ?",
        )
        .bind(order_id.to_string())
        .execute(pool)
        .await?;
        sqlx::query("DELETE FROM loan_liquidations WHERE order_id = ?")
            .bind(order_id)
            .execute(pool)
            .await?;
    }
    for user_id in fixture.user_ids {
        sqlx::query("DELETE FROM wallet_ledger WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM loan_orders WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;
    }
    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(fixture.admin_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM loan_product_collateral_assets WHERE product_id = ?")
        .bind(fixture.product_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM loan_products WHERE id = ?")
        .bind(fixture.product_id)
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
    for user_id in fixture.user_ids {
        sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;
    }
    for asset_id in fixture.asset_ids {
        sqlx::query("DELETE FROM assets WHERE id = ?")
            .bind(asset_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

#[tokio::test]
async fn collateral_loan_risk_is_authoritative_atomic_and_single_terminal()
-> Result<(), Box<dyn Error>> {
    let Some((pool, redis)) = dependencies().await? else {
        return Ok(());
    };
    let settings = test_settings();
    let suffix = Uuid::now_v7().simple().to_string();
    let role_id = sqlx::query("INSERT INTO admin_roles (name, permissions) VALUES (?, ?)")
        .bind(format!("loan-risk-role-{suffix}"))
        .bind(SqlxJson(json!(["*"])))
        .execute(&pool)
        .await?
        .last_insert_id();
    let admin_id = sqlx::query(
        "INSERT INTO admin_users (username, password_hash, role_id, status) VALUES (?, ?, ?, 'active')",
    )
    .bind(format!("loan-risk-{suffix}"))
    .bind("not-a-real-hash")
    .bind(role_id)
    .execute(&pool)
    .await?
    .last_insert_id();
    let (loan_asset_id, loan_symbol) = create_asset(&pool, "LRQ").await?;
    let (collateral_asset_id, collateral_symbol) = create_asset(&pool, "LRC").await?;
    let (other_collateral_asset_id, _) = create_asset(&pool, "LRO").await?;
    let user_ids = [
        create_user(&pool, "authority").await?,
        create_user(&pool, "race").await?,
        create_user(&pool, "rollback").await?,
    ];
    for user_id in user_ids {
        seed_collateral_wallet(&pool, user_id, collateral_asset_id).await?;
    }
    let oracle_symbol = format!("{collateral_symbol}{loan_symbol}");
    let state = AppState::new(settings.clone())
        .with_mysql(pool.clone())
        .with_redis(redis.clone());
    let admin_app = admin_routes().with_state(state.clone());
    let user_app = user_routes().with_state(state);
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )?;
    let user_tokens = [
        issue_token(
            &settings,
            format!("user:{}", user_ids[0]),
            TokenScope::User,
            900,
        )?,
        issue_token(
            &settings,
            format!("user:{}", user_ids[1]),
            TokenScope::User,
            900,
        )?,
        issue_token(
            &settings,
            format!("user:{}", user_ids[2]),
            TokenScope::User,
            900,
        )?,
    ];

    let (product_status, product_payload) = request_json(
        admin_app.clone(),
        "POST",
        "/loan/products".to_owned(),
        &admin_token,
        Some(json!({
            "loan_type": "collateralized",
            "asset_id": loan_asset_id,
            "name": "P0 collateral risk fixture",
            "term_days": 30,
            "interest_rate": "0.02",
            "interest_calculation_mode": "full_term",
            "min_kyc_level": 1,
            "min_amount": "1",
            "max_amount": "1000",
            "initial_ltv": "0.5",
            "maintenance_ltv": "0.7",
            "liquidation_ltv": "0.85",
            "collateral_assets": [{
                "collateral_asset_id": collateral_asset_id,
                "oracle_symbol": oracle_symbol,
                "oracle_source": "market_ticker_redis",
                "oracle_max_age_seconds": 30,
            }],
            "status": "active",
            "reason": "P0 loan risk integration fixture",
        })),
    )
    .await?;
    assert_eq!(product_status, StatusCode::OK, "{product_payload}");
    let product_id = product_payload["id"].as_u64().expect("product id");
    assert_eq!(
        product_payload["collateral_assets"][0]["oracle_symbol"],
        oracle_symbol
    );
    assert_decimal(&response_decimal(&product_payload["initial_ltv"]), "0.5");
    let mut fixture = LoanRiskFixture {
        role_id,
        admin_id,
        user_ids,
        asset_ids: [
            loan_asset_id,
            collateral_asset_id,
            other_collateral_asset_id,
        ],
        product_id,
        order_ids: Vec::new(),
        oracle_symbol: oracle_symbol.clone(),
    };

    let authority_key = format!("loan-risk-authority-{suffix}");
    cache_ticker(&redis, &oracle_symbol, "1", Utc::now()).await?;
    let authority_body = order_body(product_id, collateral_asset_id, &authority_key);
    let (create_status, create_payload) = request_json(
        user_app.clone(),
        "POST",
        "/loan/orders".to_owned(),
        &user_tokens[0],
        Some(authority_body.clone()),
    )
    .await?;
    assert_eq!(create_status, StatusCode::OK, "{create_payload}");
    assert_eq!(create_payload["changed"], true);
    assert_decimal(
        &response_decimal(&create_payload["order"]["application_collateral_price"]),
        "1",
    );
    assert_decimal(
        &response_decimal(&create_payload["order"]["application_ltv"]),
        "0.5",
    );
    let authority_order_id = create_payload["order"]["id"]
        .as_u64()
        .expect("authority order id");
    fixture.order_ids.push(authority_order_id);

    let (replay_status, replay_payload) = request_json(
        user_app.clone(),
        "POST",
        "/loan/orders".to_owned(),
        &user_tokens[0],
        Some(authority_body.clone()),
    )
    .await?;
    assert_eq!(replay_status, StatusCode::OK, "{replay_payload}");
    assert_eq!(replay_payload["changed"], false);
    assert_eq!(replay_payload["order"]["id"], authority_order_id);

    let mut changed_parameters = authority_body.clone();
    changed_parameters["amount"] = json!("40");
    let (conflict_status, conflict_payload) = request_json(
        user_app.clone(),
        "POST",
        "/loan/orders".to_owned(),
        &user_tokens[0],
        Some(changed_parameters),
    )
    .await?;
    assert_eq!(conflict_status, StatusCode::CONFLICT, "{conflict_payload}");

    let non_whitelist_key = format!("loan-risk-non-whitelist-{suffix}");
    let (non_whitelist_status, non_whitelist_payload) = request_json(
        user_app.clone(),
        "POST",
        "/loan/orders".to_owned(),
        &user_tokens[0],
        Some(order_body(
            product_id,
            other_collateral_asset_id,
            &non_whitelist_key,
        )),
    )
    .await?;
    assert_eq!(
        non_whitelist_status,
        StatusCode::BAD_REQUEST,
        "{non_whitelist_payload}"
    );

    let mut connection = redis.clone();
    let _: usize = connection
        .del(market_ticker_redis_key(&oracle_symbol))
        .await?;
    let missing_key = format!("loan-risk-missing-{suffix}");
    let (missing_status, missing_payload) = request_json(
        user_app.clone(),
        "POST",
        "/loan/orders".to_owned(),
        &user_tokens[0],
        Some(order_body(product_id, collateral_asset_id, &missing_key)),
    )
    .await?;
    assert_eq!(missing_status, StatusCode::BAD_REQUEST, "{missing_payload}");

    cache_ticker(
        &redis,
        &oracle_symbol,
        "1",
        Utc::now() - TimeDelta::seconds(31),
    )
    .await?;
    let stale_key = format!("loan-risk-stale-{suffix}");
    let (stale_status, stale_payload) = request_json(
        user_app.clone(),
        "POST",
        "/loan/orders".to_owned(),
        &user_tokens[0],
        Some(order_body(product_id, collateral_asset_id, &stale_key)),
    )
    .await?;
    assert_eq!(stale_status, StatusCode::BAD_REQUEST, "{stale_payload}");

    cache_ticker(&redis, &oracle_symbol, "0.9", Utc::now()).await?;
    let under_collateralized_key = format!("loan-risk-low-collateral-{suffix}");
    let (low_status, low_payload) = request_json(
        user_app.clone(),
        "POST",
        "/loan/orders".to_owned(),
        &user_tokens[0],
        Some(order_body(
            product_id,
            collateral_asset_id,
            &under_collateralized_key,
        )),
    )
    .await?;
    assert_eq!(low_status, StatusCode::BAD_REQUEST, "{low_payload}");

    cache_ticker(&redis, &oracle_symbol, "1", Utc::now()).await?;
    let insufficient_key = format!("loan-risk-insufficient-{suffix}");
    let (insufficient_status, insufficient_payload) = request_json(
        user_app.clone(),
        "POST",
        "/loan/orders".to_owned(),
        &user_tokens[0],
        Some(order_body(
            product_id,
            collateral_asset_id,
            &insufficient_key,
        )),
    )
    .await?;
    assert_eq!(
        insufficient_status,
        StatusCode::BAD_REQUEST,
        "{insufficient_payload}"
    );
    let failed_order_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM loan_orders WHERE user_id = ? AND idempotency_key IN (?, ?, ?, ?, ?)",
    )
    .bind(user_ids[0])
    .bind(non_whitelist_key)
    .bind(missing_key)
    .bind(stale_key)
    .bind(under_collateralized_key)
    .bind(insufficient_key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(failed_order_count, 0);
    let (available, frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_ids[0])
    .bind(collateral_asset_id)
    .fetch_one(&pool)
    .await?;
    assert_decimal(&available, "0");
    assert_decimal(&frozen, "100");
    let freeze_ledger_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'loan_order' AND ref_id = ? AND change_type = 'loan_collateral_freeze'",
    )
    .bind(authority_order_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(freeze_ledger_count, 2);

    cache_ticker(
        &redis,
        &oracle_symbol,
        "1",
        Utc::now() - TimeDelta::seconds(31),
    )
    .await?;
    let (stale_approval_status, stale_approval_payload) = request_json(
        admin_app.clone(),
        "POST",
        format!("/loan/orders/{authority_order_id}/approve"),
        &admin_token,
        None,
    )
    .await?;
    assert_eq!(
        stale_approval_status,
        StatusCode::BAD_REQUEST,
        "{stale_approval_payload}"
    );
    let (status_after_stale, loan_wallet_count): (String, i64) = sqlx::query_as(
        r#"SELECT orders.status,
                  (SELECT COUNT(*) FROM wallet_accounts wallets
                   WHERE wallets.user_id = orders.user_id AND wallets.asset_id = orders.asset_id)
           FROM loan_orders orders WHERE orders.id = ?"#,
    )
    .bind(authority_order_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(status_after_stale, "pending");
    assert_eq!(loan_wallet_count, 0);

    cache_ticker(&redis, &oracle_symbol, "1", Utc::now()).await?;
    let (approval_status, approval_payload) = request_json(
        admin_app.clone(),
        "POST",
        format!("/loan/orders/{authority_order_id}/approve"),
        &admin_token,
        None,
    )
    .await?;
    assert_eq!(approval_status, StatusCode::OK, "{approval_payload}");
    assert_eq!(approval_payload["changed"], true);
    assert_decimal(
        &response_decimal(&approval_payload["order"]["approval_collateral_price"]),
        "1",
    );
    assert_decimal(
        &response_decimal(&approval_payload["order"]["approval_ltv"]),
        "0.5",
    );
    let due_alignment_micros: i64 = sqlx::query_scalar(
        r#"SELECT TIMESTAMPDIFF(
                  MICROSECOND,
                  TIMESTAMPADD(DAY, term_days, disbursed_at),
                  due_at
               )
           FROM loan_orders WHERE id = ?"#,
    )
    .bind(authority_order_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(due_alignment_micros, 0);
    let (approval_replay_status, approval_replay_payload) = request_json(
        admin_app.clone(),
        "POST",
        format!("/loan/orders/{authority_order_id}/approve"),
        &admin_token,
        None,
    )
    .await?;
    assert_eq!(
        approval_replay_status,
        StatusCode::OK,
        "{approval_replay_payload}"
    );
    assert_eq!(approval_replay_payload["changed"], false);
    let disbursement_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'loan_order' AND ref_id = ? AND change_type = 'loan_disbursement'",
    )
    .bind(authority_order_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(disbursement_count, 1);
    let (disbursement_journal_count, disbursement_journal_sum): (i64, BigDecimal) = sqlx::query_as(
        r#"SELECT COUNT(*), COALESCE(SUM(amount), 0)
               FROM platform_financial_journal WHERE transaction_key = ?"#,
    )
    .bind(format!("loan_disbursement:{authority_order_id}"))
    .fetch_one(&pool)
    .await?;
    assert_eq!(disbursement_journal_count, 2);
    assert_decimal(&disbursement_journal_sum, "0");

    let (health_status, health_payload) = request_json(
        user_app.clone(),
        "GET",
        format!("/loan/orders/{authority_order_id}/health"),
        &user_tokens[0],
        None,
    )
    .await?;
    assert_eq!(health_status, StatusCode::OK, "{health_payload}");
    assert_eq!(health_payload["risk_state"], "healthy");
    assert_decimal(&response_decimal(&health_payload["debt_amount"]), "51");
    assert_decimal(&response_decimal(&health_payload["current_ltv"]), "0.51");

    let liquidation_now = Utc::now();
    cache_ticker(&redis, &oracle_symbol, "0.5", liquidation_now).await?;
    let first_scan =
        loan_health::run_once_with_dependencies(&pool, &redis, liquidation_now, 100).await?;
    assert_eq!(first_scan.liquidated, 1, "summary: {first_scan:?}");
    let second_scan = loan_health::run_once_with_dependencies(
        &pool,
        &redis,
        liquidation_now + TimeDelta::seconds(1),
        100,
    )
    .await?;
    assert_eq!(second_scan.liquidated, 0, "summary: {second_scan:?}");
    let liquidation_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM loan_liquidations WHERE order_id = ?")
            .bind(authority_order_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(liquidation_count, 1);
    let (debt, seized, returned, recovered, bad_debt): (
        BigDecimal,
        BigDecimal,
        BigDecimal,
        BigDecimal,
        BigDecimal,
    ) = sqlx::query_as(
        r#"SELECT debt_amount, collateral_seized, collateral_returned,
                  recovered_amount, bad_debt_amount
           FROM loan_liquidations WHERE order_id = ?"#,
    )
    .bind(authority_order_id)
    .fetch_one(&pool)
    .await?;
    assert_decimal(&debt, "51");
    assert_decimal(&seized, "100");
    assert_decimal(&returned, "0");
    assert_decimal(&recovered, "50");
    assert_decimal(&bad_debt, "1");
    let journal_sums: Vec<(u64, BigDecimal)> = sqlx::query_as(
        r#"SELECT asset_id, SUM(amount)
           FROM platform_financial_journal
           WHERE transaction_key = ?
           GROUP BY asset_id"#,
    )
    .bind(format!("loan_liquidation:{authority_order_id}"))
    .fetch_all(&pool)
    .await?;
    assert_eq!(journal_sums.len(), 2);
    for (_, sum) in journal_sums {
        assert_decimal(&sum, "0");
    }
    let (bad_debt_legs, liquidation_mode): (i64, String) = sqlx::query_as(
        r#"SELECT COUNT(*), CAST(MAX(JSON_UNQUOTE(JSON_EXTRACT(metadata_json, '$.mode'))) AS CHAR)
           FROM platform_financial_journal
           WHERE transaction_key = ? AND account_code = 'platform_bad_debt_expense'"#,
    )
    .bind(format!("loan_liquidation:{authority_order_id}"))
    .fetch_one(&pool)
    .await?;
    assert_eq!(bad_debt_legs, 1);
    assert_eq!(
        liquidation_mode,
        "platform_collateral_clearing_no_external_sale"
    );
    let liquidation_wallet_legs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'loan_liquidation' AND ref_id = ? AND change_type = 'loan_collateral_liquidation'",
    )
    .bind(authority_order_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(liquidation_wallet_legs, 1);

    sqlx::query("UPDATE loan_products SET interest_rate = 0 WHERE id = ?")
        .bind(product_id)
        .execute(&pool)
        .await?;
    let race_key = format!("loan-risk-race-{suffix}");
    cache_ticker(&redis, &oracle_symbol, "1", Utc::now()).await?;
    let (race_create_status, race_create_payload) = request_json(
        user_app.clone(),
        "POST",
        "/loan/orders".to_owned(),
        &user_tokens[1],
        Some(order_body(product_id, collateral_asset_id, &race_key)),
    )
    .await?;
    assert_eq!(race_create_status, StatusCode::OK, "{race_create_payload}");
    let race_order_id = race_create_payload["order"]["id"]
        .as_u64()
        .expect("race order id");
    fixture.order_ids.push(race_order_id);
    let (race_approval_status, race_approval_payload) = request_json(
        admin_app.clone(),
        "POST",
        format!("/loan/orders/{race_order_id}/approve"),
        &admin_token,
        None,
    )
    .await?;
    assert_eq!(
        race_approval_status,
        StatusCode::OK,
        "{race_approval_payload}"
    );
    let race_now = Utc::now();
    cache_ticker(&redis, &oracle_symbol, "0.4", race_now).await?;
    let worker_future = loan_health::run_once_with_dependencies(&pool, &redis, race_now, 100);
    let repayment_future = request_json(
        user_app.clone(),
        "POST",
        format!("/loan/orders/{race_order_id}/repay"),
        &user_tokens[1],
        None,
    );
    let (race_scan, repayment_response) = tokio::join!(worker_future, repayment_future);
    let race_scan = race_scan?;
    let (repayment_status, repayment_payload) = repayment_response?;
    let (terminal_status, repaid_at_set, liquidated_at_set): (String, bool, bool) =
        sqlx::query_as(
            "SELECT status, repaid_at IS NOT NULL, liquidated_at IS NOT NULL FROM loan_orders WHERE id = ?",
        )
        .bind(race_order_id)
        .fetch_one(&pool)
        .await?;
    assert_ne!(repaid_at_set, liquidated_at_set);
    let race_liquidation_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM loan_liquidations WHERE order_id = ?")
            .bind(race_order_id)
            .fetch_one(&pool)
            .await?;
    let race_repayment_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'loan_order' AND ref_id = ? AND change_type = 'loan_repayment'",
    )
    .bind(race_order_id.to_string())
    .fetch_one(&pool)
    .await?;
    let (race_repayment_journals, race_repayment_journal_sum): (i64, BigDecimal) = sqlx::query_as(
        r#"SELECT COUNT(*), COALESCE(SUM(amount), 0)
               FROM platform_financial_journal WHERE transaction_key = ?"#,
    )
    .bind(format!("loan_repayment:{race_order_id}"))
    .fetch_one(&pool)
    .await?;
    match terminal_status.as_str() {
        "repaid" => {
            assert_eq!(repayment_status, StatusCode::OK, "{repayment_payload}");
            assert_eq!(race_liquidation_count, 0);
            assert_eq!(race_repayment_count, 1);
            assert_eq!(race_repayment_journals, 2);
            assert_decimal(&race_repayment_journal_sum, "0");
            assert_eq!(race_scan.liquidated, 0);
        }
        "liquidated" => {
            assert_eq!(
                repayment_status,
                StatusCode::CONFLICT,
                "{repayment_payload}"
            );
            assert_eq!(race_liquidation_count, 1);
            assert_eq!(race_repayment_count, 0);
            assert_eq!(race_repayment_journals, 0);
            assert_decimal(&race_repayment_journal_sum, "0");
            assert_eq!(race_scan.liquidated, 1);
        }
        other => panic!("unexpected concurrent terminal status: {other}"),
    }

    let rollback_key = format!("loan-risk-rollback-{suffix}");
    cache_ticker(&redis, &oracle_symbol, "1", Utc::now()).await?;
    let (rollback_create_status, rollback_create_payload) = request_json(
        user_app.clone(),
        "POST",
        "/loan/orders".to_owned(),
        &user_tokens[2],
        Some(order_body(product_id, collateral_asset_id, &rollback_key)),
    )
    .await?;
    assert_eq!(
        rollback_create_status,
        StatusCode::OK,
        "{rollback_create_payload}"
    );
    let rollback_order_id = rollback_create_payload["order"]["id"]
        .as_u64()
        .expect("rollback order id");
    fixture.order_ids.push(rollback_order_id);
    let rollback_disbursement_key = format!("loan_disbursement:{rollback_order_id}");
    sqlx::query(
        r#"INSERT INTO platform_financial_journal
           (transaction_key, context, account_code, asset_id, amount, ref_type, ref_id)
           VALUES (?, 'loan_disbursement', 'platform_loan_funding', ?, -50, 'loan_order', ?)"#,
    )
    .bind(&rollback_disbursement_key)
    .bind(loan_asset_id)
    .bind(rollback_order_id.to_string())
    .execute(&pool)
    .await?;
    let (failed_approval_status, failed_approval_payload) = request_json(
        admin_app.clone(),
        "POST",
        format!("/loan/orders/{rollback_order_id}/approve"),
        &admin_token,
        None,
    )
    .await?;
    assert_eq!(
        failed_approval_status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "{failed_approval_payload}"
    );
    let (failed_approval_order_status, failed_approval_wallet_count): (String, i64) =
        sqlx::query_as(
            r#"SELECT orders.status,
                      (SELECT COUNT(*) FROM wallet_accounts wallets
                       WHERE wallets.user_id = orders.user_id AND wallets.asset_id = orders.asset_id)
               FROM loan_orders orders WHERE orders.id = ?"#,
        )
        .bind(rollback_order_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(failed_approval_order_status, "pending");
    assert_eq!(failed_approval_wallet_count, 0);
    sqlx::query("DELETE FROM platform_financial_journal WHERE transaction_key = ?")
        .bind(&rollback_disbursement_key)
        .execute(&pool)
        .await?;
    let (rollback_approval_status, rollback_approval_payload) = request_json(
        admin_app,
        "POST",
        format!("/loan/orders/{rollback_order_id}/approve"),
        &admin_token,
        None,
    )
    .await?;
    assert_eq!(
        rollback_approval_status,
        StatusCode::OK,
        "{rollback_approval_payload}"
    );
    let rollback_repayment_key = format!("loan_repayment:{rollback_order_id}");
    sqlx::query(
        r#"INSERT INTO platform_financial_journal
           (transaction_key, context, account_code, asset_id, amount, ref_type, ref_id)
           VALUES (?, 'loan_repayment', 'loan_principal_receivable_close', ?, -50, 'loan_order', ?)"#,
    )
    .bind(&rollback_repayment_key)
    .bind(loan_asset_id)
    .bind(rollback_order_id.to_string())
    .execute(&pool)
    .await?;
    let (failed_repayment_status, failed_repayment_payload) = request_json(
        user_app,
        "POST",
        format!("/loan/orders/{rollback_order_id}/repay"),
        &user_tokens[2],
        None,
    )
    .await?;
    assert_eq!(
        failed_repayment_status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "{failed_repayment_payload}"
    );
    let (status_after_failed_repayment, loan_available_after_failed_repayment): (
        String,
        BigDecimal,
    ) = sqlx::query_as(
        r#"SELECT orders.status, wallets.available
           FROM loan_orders orders
           INNER JOIN wallet_accounts wallets
             ON wallets.user_id = orders.user_id AND wallets.asset_id = orders.asset_id
           WHERE orders.id = ?"#,
    )
    .bind(rollback_order_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(status_after_failed_repayment, "disbursed");
    assert_decimal(&loan_available_after_failed_repayment, "50");
    sqlx::query("DELETE FROM platform_financial_journal WHERE transaction_key = ?")
        .bind(&rollback_repayment_key)
        .execute(&pool)
        .await?;
    sqlx::query("UPDATE wallet_accounts SET frozen = 99 WHERE user_id = ? AND asset_id = ?")
        .bind(user_ids[2])
        .bind(collateral_asset_id)
        .execute(&pool)
        .await?;
    let rollback_now = Utc::now();
    let rollback_ticker = LoanOraclePrice {
        symbol: oracle_symbol.clone(),
        source: "market_ticker_redis".to_owned(),
        price: decimal("0.4"),
        observed_at: rollback_now,
    };
    let rollback_outcome =
        liquidate_loan_order_if_required(&pool, rollback_order_id, &rollback_ticker, rollback_now)
            .await;
    assert!(rollback_outcome.is_err());
    assert!(!matches!(
        rollback_outcome,
        Ok(LoanLiquidationOutcome::Liquidated(_))
    ));
    let (rollback_status, rollback_frozen): (String, BigDecimal) = sqlx::query_as(
        r#"SELECT orders.status, wallets.frozen
           FROM loan_orders orders
           INNER JOIN wallet_accounts wallets
             ON wallets.user_id = orders.user_id AND wallets.asset_id = orders.collateral_asset_id
           WHERE orders.id = ?"#,
    )
    .bind(rollback_order_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(rollback_status, "disbursed");
    assert_decimal(&rollback_frozen, "99");
    let rollback_side_effect_count: i64 = sqlx::query_scalar(
        r#"SELECT
              (SELECT COUNT(*) FROM loan_liquidations WHERE order_id = ?)
            + (SELECT COUNT(*) FROM platform_financial_journal WHERE transaction_key = ?)
            + (SELECT COUNT(*) FROM wallet_ledger
               WHERE ref_type = 'loan_liquidation' AND ref_id = ?)"#,
    )
    .bind(rollback_order_id)
    .bind(format!("loan_liquidation:{rollback_order_id}"))
    .bind(rollback_order_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(rollback_side_effect_count, 0);

    cleanup_fixture(&pool, &redis, &fixture).await?;
    Ok(())
}
