use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use bigdecimal::BigDecimal;
use chrono::Utc;
use exchange_api::{
    config::Settings,
    modules::{
        auth::{TokenScope, hash_password, issue_token},
        wallet::routes::{admin_routes, routes},
    },
    state::AppState,
};
use secrecy::SecretString;
use serde_json::{Value, json};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::{error::Error, str::FromStr};
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

async fn create_user(pool: &MySqlPool) -> u64 {
    let email = format!("wallet-route-{}@example.test", Uuid::now_v7().simple());
    sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(email)
        .bind("not-a-real-hash")
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id()
}

async fn create_admin(pool: &MySqlPool) -> (u64, u64) {
    let suffix = Uuid::now_v7().simple().to_string();
    let role_id = sqlx::query(
        "INSERT INTO admin_roles (name, permissions) VALUES (?, JSON_ARRAY('wallet:manage'))",
    )
    .bind(format!("wallet-role-{}", &suffix[16..32]))
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id();
    let admin_id =
        sqlx::query("INSERT INTO admin_users (username, password_hash, role_id) VALUES (?, ?, ?)")
            .bind(format!("wallet-admin-{}", &suffix[16..32]))
            .bind("not-a-real-hash")
            .bind(role_id)
            .execute(pool)
            .await
            .unwrap()
            .last_insert_id();
    (role_id, admin_id)
}

async fn seed_fund_password(pool: &MySqlPool, user_id: u64, password: &str) {
    sqlx::query("INSERT INTO user_security (user_id, fund_password_hash) VALUES (?, ?)")
        .bind(user_id)
        .bind(hash_password(password).unwrap())
        .execute(pool)
        .await
        .unwrap();
}

async fn create_asset(pool: &MySqlPool) -> (u64, String) {
    let suffix = Uuid::now_v7().simple().to_string();
    let symbol = format!("WR{}", &suffix[suffix.len() - 12..]).to_ascii_uppercase();
    let logo_url = format!("https://cdn.example.test/assets/{symbol}.png");
    let asset_id = sqlx::query(
        "INSERT INTO assets (symbol, name, logo_url, precision_scale, asset_type, status) VALUES (?, ?, ?, 18, 'coin', 'active')",
    )
        .bind(&symbol)
        .bind(&symbol)
        .bind(&logo_url)
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id();
    (asset_id, logo_url)
}

async fn create_deposit_asset(pool: &MySqlPool) -> (u64, String) {
    let suffix = Uuid::now_v7().simple().to_string();
    let symbol = format!("WD{}", &suffix[suffix.len() - 12..]).to_ascii_uppercase();
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

async fn seed_wallet(pool: &MySqlPool, user_id: u64, asset_id: u64, ref_id: &str) {
    sqlx::query(
        r#"INSERT INTO wallet_accounts (user_id, asset_id, available, frozen, locked)
           VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(decimal("12.500000000000000000"))
    .bind(decimal("1.500000000000000000"))
    .bind(decimal("3.000000000000000000"))
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind("deposit_credit")
    .bind(decimal("12.500000000000000000"))
    .bind("available")
    .bind(decimal("12.500000000000000000"))
    .bind(decimal("12.500000000000000000"))
    .bind(decimal("1.500000000000000000"))
    .bind(decimal("3.000000000000000000"))
    .bind("deposit_record")
    .bind(ref_id)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_convert_fee_ledger(pool: &MySqlPool, user_id: u64, asset_id: u64, quote_id: &str) {
    let pair_id = sqlx::query(
        r#"INSERT INTO convert_pairs
           (from_asset, to_asset, pricing_mode, spread_rate, min_amount, max_amount, enabled)
           VALUES (?, ?, 'fixed', ?, ?, NULL, TRUE)"#,
    )
    .bind(asset_id)
    .bind(asset_id)
    .bind(decimal("0.00000000"))
    .bind(decimal("0.000000000000000000"))
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id();

    sqlx::query(
        r#"INSERT INTO convert_orders
           (quote_id, convert_pair_id, user_id, from_asset, to_asset, from_amount,
            to_amount, rate, fee_rate, fee_amount, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'completed')"#,
    )
    .bind(quote_id)
    .bind(pair_id)
    .bind(user_id)
    .bind(asset_id)
    .bind(asset_id)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("9.750000000000000000"))
    .bind(decimal("1.000000000000000000"))
    .bind(decimal("0.02500000"))
    .bind(decimal("0.250000000000000000"))
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind("convert_settlement")
    .bind(decimal("-10.000000000000000000"))
    .bind("available")
    .bind(decimal("2.500000000000000000"))
    .bind(decimal("2.500000000000000000"))
    .bind(decimal("1.500000000000000000"))
    .bind(decimal("3.000000000000000000"))
    .bind("convert_order")
    .bind(quote_id)
    .execute(pool)
    .await
    .unwrap();
}

async fn cleanup_wallet_route_fixture(
    pool: &MySqlPool,
    user_id: u64,
    asset_id: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM wallet_ledger WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(pool)
        .await?;
    sqlx::query(
        "DELETE FROM convert_orders WHERE user_id = ? AND (from_asset = ? OR to_asset = ?)",
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(asset_id)
    .execute(pool)
    .await?;
    sqlx::query("DELETE FROM convert_pairs WHERE from_asset = ? OR to_asset = ?")
        .bind(asset_id)
        .bind(asset_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM wallet_withdrawal_requests WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM user_security WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
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
    Ok(())
}

#[tokio::test]
async fn wallet_routes_return_authenticated_user_accounts_and_ledger() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (asset_id, asset_logo_url) = create_asset(&pool).await;
    let ref_id = format!("wallet-route-{}", Uuid::now_v7().simple());
    let convert_quote_id = format!("wallet-convert-{}", Uuid::now_v7().simple());
    seed_wallet(&pool, user_id, asset_id, &ref_id).await;
    seed_convert_fee_ledger(&pool, user_id, asset_id, &convert_quote_id).await;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = routes().with_state(AppState::new(settings.clone()).with_mysql(pool.clone()));

    let accounts_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/wallet/accounts")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(accounts_response.status(), StatusCode::OK);
    let accounts_body = axum::body::to_bytes(accounts_response.into_body(), 8192).await?;
    let accounts: Value = serde_json::from_slice(&accounts_body)?;
    assert_eq!(accounts["accounts"].as_array().unwrap().len(), 1);
    assert_eq!(accounts["accounts"][0]["user_id"], user_id);
    assert_eq!(accounts["accounts"][0]["asset_id"], asset_id);
    assert_eq!(accounts["accounts"][0]["logo_url"], asset_logo_url);
    assert_eq!(
        accounts["accounts"][0]["available"],
        "12.500000000000000000"
    );

    let ledger_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/wallet/ledger?asset_id={asset_id}&ref_type=deposit_record&ref_id={ref_id}&limit=10"
                ))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let ledger_status = ledger_response.status();
    let ledger_body = axum::body::to_bytes(ledger_response.into_body(), 8192).await?;
    assert_eq!(
        ledger_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&ledger_body)
    );
    let ledger: Value = serde_json::from_slice(&ledger_body)?;
    assert_eq!(ledger["entries"].as_array().unwrap().len(), 1);
    assert_eq!(ledger["entries"][0]["user_id"], user_id);
    assert_eq!(ledger["entries"][0]["ref_id"], ref_id);
    assert_eq!(ledger["entries"][0]["amount"], "12.500000000000000000");
    assert_eq!(
        decimal(ledger["entries"][0]["fee"].as_str().unwrap()),
        decimal("0")
    );

    let convert_ledger_response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/wallet/ledger?asset_id={asset_id}&ref_type=convert_order&ref_id={convert_quote_id}&limit=10"
                ))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let convert_ledger_status = convert_ledger_response.status();
    let convert_ledger_body =
        axum::body::to_bytes(convert_ledger_response.into_body(), 8192).await?;
    assert_eq!(
        convert_ledger_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&convert_ledger_body)
    );
    let convert_ledger: Value = serde_json::from_slice(&convert_ledger_body)?;
    assert_eq!(convert_ledger["entries"].as_array().unwrap().len(), 1);
    assert_eq!(convert_ledger["entries"][0]["ref_id"], convert_quote_id);
    assert_eq!(convert_ledger["entries"][0]["fee"], "0.250000000000000000");

    cleanup_wallet_route_fixture(&pool, user_id, asset_id).await?;
    Ok(())
}

#[tokio::test]
async fn wallet_today_return_aggregates_realized_sources_and_marks_missing_ticker_partial()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let other_user_id = create_user(&pool).await;
    sqlx::query(
        r#"INSERT INTO assets (symbol, name, precision_scale, asset_type, status)
           VALUES ('USDT', 'Tether', 18, 'coin', 'active')
           ON DUPLICATE KEY UPDATE symbol = VALUES(symbol)"#,
    )
    .execute(&pool)
    .await?;
    let usdt_asset_id: u64 = sqlx::query_scalar("SELECT id FROM assets WHERE symbol = 'USDT'")
        .fetch_one(&pool)
        .await?;
    let (base_asset_id, _) = create_asset(&pool).await;
    let base_symbol: String = sqlx::query_scalar("SELECT symbol FROM assets WHERE id = ?")
        .bind(base_asset_id)
        .fetch_one(&pool)
        .await?;
    let suffix = Uuid::now_v7().simple().to_string();
    let pair_symbol = format!("{base_symbol}USDT");
    let pair_id = sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision,
            min_order_value, status, market_type)
           VALUES (?, ?, ?, 18, 18, 1, 'active', 'external')"#,
    )
    .bind(base_asset_id)
    .bind(usdt_asset_id)
    .bind(&pair_symbol)
    .execute(&pool)
    .await?
    .last_insert_id();
    let seconds_product_id = sqlx::query(
        r#"INSERT INTO seconds_contract_products
           (pair_id, stake_asset, duration_seconds, payout_rate, min_stake, max_stake, status)
           VALUES (?, ?, 60, 0.80000000, 1, 1000, 'active')"#,
    )
    .bind(pair_id)
    .bind(usdt_asset_id)
    .execute(&pool)
    .await?
    .last_insert_id();
    let margin_product_id = sqlx::query(
        r#"INSERT INTO margin_products
           (pair_id, margin_asset, margin_mode, margin_modes, leverage_levels,
            max_leverage, min_margin, max_margin, maintenance_margin_rate, status)
           VALUES (?, ?, 'isolated', JSON_ARRAY('isolated'), JSON_ARRAY('2'),
                   2, 1, 1000, 0.05000000, 'active')"#,
    )
    .bind(pair_id)
    .bind(usdt_asset_id)
    .execute(&pool)
    .await?
    .last_insert_id();
    let prediction_market_id = sqlx::query(
        r#"INSERT INTO prediction_markets
           (external_market_id, title, tags_json, yes_price, no_price)
           VALUES (?, 'Today return fixture', JSON_ARRAY(), 0.5, 0.5)"#,
    )
    .bind(format!("today-return-{suffix}"))
    .execute(&pool)
    .await?
    .last_insert_id();
    let earn_product_id = sqlx::query(
        r#"INSERT INTO earn_products
           (asset_id, name, category, introduction_json, term_days, apr_rate,
            min_subscribe, max_subscribe, status)
           VALUES (?, 'Today return earn', 'fixed_term',
                   JSON_OBJECT('version', 1, 'default_locale', 'en', 'items', JSON_ARRAY()),
                   30, 0.12000000, 1, 1000, 'active')"#,
    )
    .bind(usdt_asset_id)
    .execute(&pool)
    .await?
    .last_insert_id();

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let zero_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/wallet/today-return")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(zero_response.status(), StatusCode::OK);
    let zero_payload = body_json(zero_response).await?;
    assert_eq!(zero_payload["amount"], "0.000000000000000000");
    assert_eq!(zero_payload["basis_amount"], "0.000000000000000000");
    assert_eq!(zero_payload["status"], "complete");

    let period_start_at = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
    for (stake, result, key) in [("10", "win", "win"), ("4", "loss", "loss")] {
        sqlx::query(
            r#"INSERT INTO seconds_contract_orders
               (user_id, product_id, pair_id, stake_asset, direction, stake_amount,
                duration_seconds, payout_rate, status, result, idempotency_key,
                opened_at, expires_at, settled_at, created_at)
               VALUES (?, ?, ?, ?, 'up', ?, 60, 0.80000000, 'settled', ?, ?,
                       ?, ?, ?, ?)"#,
        )
        .bind(user_id)
        .bind(seconds_product_id)
        .bind(pair_id)
        .bind(usdt_asset_id)
        .bind(decimal(stake))
        .bind(result)
        .bind(format!("today-return-seconds-{key}-{suffix}"))
        .bind(period_start_at)
        .bind(period_start_at)
        .bind(period_start_at)
        .bind(period_start_at)
        .execute(&pool)
        .await?;
    }
    sqlx::query(
        r#"INSERT INTO seconds_contract_orders
           (user_id, product_id, pair_id, stake_asset, direction, stake_amount,
            duration_seconds, payout_rate, status, result, idempotency_key,
            opened_at, expires_at, settled_at, created_at)
           VALUES (?, ?, ?, ?, 'up', 100, 60, 0.80000000, 'settled', 'win', ?,
                   ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(seconds_product_id)
    .bind(pair_id)
    .bind(usdt_asset_id)
    .bind(format!("today-return-yesterday-{suffix}"))
    .bind(period_start_at - chrono::TimeDelta::minutes(1))
    .bind(period_start_at - chrono::TimeDelta::minutes(1))
    .bind(period_start_at - chrono::TimeDelta::microseconds(1))
    .bind(period_start_at - chrono::TimeDelta::minutes(1))
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"INSERT INTO seconds_contract_orders
           (user_id, product_id, pair_id, stake_asset, direction, stake_amount,
            duration_seconds, payout_rate, status, result, idempotency_key,
            opened_at, expires_at, settled_at, created_at)
           VALUES (?, ?, ?, ?, 'up', 100, 60, 0.80000000, 'settled', 'win', ?,
                   ?, ?, ?, ?)"#,
    )
    .bind(other_user_id)
    .bind(seconds_product_id)
    .bind(pair_id)
    .bind(usdt_asset_id)
    .bind(format!("today-return-other-user-{suffix}"))
    .bind(period_start_at)
    .bind(period_start_at)
    .bind(period_start_at)
    .bind(period_start_at)
    .execute(&pool)
    .await?;
    for (stake, fee, refund, fee_refund, key) in [
        ("7", "1", "7", "1", "full-refund"),
        ("6", "2", "6", "0", "stake-only-refund"),
    ] {
        sqlx::query(
            r#"INSERT INTO prediction_orders
               (user_id, market_id, quote_id, idempotency_key, outcome, asset_id,
                stake_amount, fee_amount, accepted_price, shares, theoretical_payout,
                effective_payout_cap, status, result, payout_amount, refund_amount,
                fee_refund_amount, settled_at)
               VALUES (?, ?, ?, ?, 'yes', ?, ?, ?, 0.5, 20, 20, 20,
                       'refunded', 'invalid', 0, ?, ?, ?)"#,
        )
        .bind(user_id)
        .bind(prediction_market_id)
        .bind(format!("today-return-refund-quote-{key}-{suffix}"))
        .bind(format!("today-return-refund-{key}-{suffix}"))
        .bind(usdt_asset_id)
        .bind(decimal(stake))
        .bind(decimal(fee))
        .bind(decimal(refund))
        .bind(decimal(fee_refund))
        .bind(period_start_at)
        .execute(&pool)
        .await?;
    }
    sqlx::query(
        r#"INSERT INTO prediction_orders
           (user_id, market_id, quote_id, idempotency_key, outcome, asset_id,
            stake_amount, fee_amount, accepted_price, shares, theoretical_payout,
            effective_payout_cap, status, result, payout_amount, settled_at)
           VALUES (?, ?, ?, ?, 'yes', ?, 10, 1, 0.5, 20, 20, 20,
                   'settled', 'yes', 15, ?)"#,
    )
    .bind(user_id)
    .bind(prediction_market_id)
    .bind(format!("today-return-quote-{suffix}"))
    .bind(format!("today-return-prediction-{suffix}"))
    .bind(usdt_asset_id)
    .bind(period_start_at)
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"INSERT INTO prediction_orders
           (user_id, market_id, quote_id, idempotency_key, outcome, asset_id,
            stake_amount, fee_amount, accepted_price, shares, theoretical_payout,
            effective_payout_cap, status, result, payout_amount, settled_at)
           VALUES (?, ?, ?, ?, 'yes', ?, 100, 0, 0.5, 200, 1000, 1000,
                   'settled', 'yes', 1000, ?)"#,
    )
    .bind(other_user_id)
    .bind(prediction_market_id)
    .bind(format!("today-return-other-prediction-quote-{suffix}"))
    .bind(format!("today-return-other-prediction-{suffix}"))
    .bind(usdt_asset_id)
    .bind(period_start_at)
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"INSERT INTO margin_positions
           (user_id, product_id, pair_id, margin_asset, direction, margin_amount,
            leverage, notional_amount, interest_amount, status, idempotency_key,
            closed_at, liquidated_at, exit_price, realized_pnl)
           VALUES (?, ?, ?, ?, 'long', 10, 2, 20, 1.5, 'liquidated', ?, ?, ?, 80, -5)"#,
    )
    .bind(user_id)
    .bind(margin_product_id)
    .bind(pair_id)
    .bind(usdt_asset_id)
    .bind(format!("today-return-liquidated-margin-{suffix}"))
    .bind(period_start_at)
    .bind(period_start_at)
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"INSERT INTO margin_positions
           (user_id, product_id, pair_id, margin_asset, direction, margin_amount,
            leverage, notional_amount, interest_amount, status, idempotency_key,
            closed_at, exit_price, realized_pnl)
           VALUES (?, ?, ?, ?, 'long', 20, 2, 40, 1, 'closed', ?, ?, 110, 3)"#,
    )
    .bind(user_id)
    .bind(margin_product_id)
    .bind(pair_id)
    .bind(usdt_asset_id)
    .bind(format!("today-return-margin-{suffix}"))
    .bind(period_start_at)
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"INSERT INTO margin_positions
           (user_id, product_id, pair_id, margin_asset, direction, margin_amount,
            leverage, notional_amount, interest_amount, status, idempotency_key,
            closed_at, exit_price, realized_pnl)
           VALUES (?, ?, ?, ?, 'long', 100, 2, 200, 10, 'closed', ?, ?, 200, 1000)"#,
    )
    .bind(other_user_id)
    .bind(margin_product_id)
    .bind(pair_id)
    .bind(usdt_asset_id)
    .bind(format!("today-return-other-margin-{suffix}"))
    .bind(period_start_at)
    .execute(&pool)
    .await?;
    let earn_subscription_id = sqlx::query(
        r#"INSERT INTO earn_subscriptions
           (user_id, product_id, asset_id, amount, apr_rate, term_days, status,
            idempotency_key, subscribed_at, matures_at, redeemed_at)
           VALUES (?, ?, ?, 100, 0.12000000, 30, 'redeemed', ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(earn_product_id)
    .bind(usdt_asset_id)
    .bind(format!("today-return-earn-{suffix}"))
    .bind(period_start_at)
    .bind(period_start_at)
    .bind(period_start_at)
    .execute(&pool)
    .await?
    .last_insert_id();
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id, created_at)
           VALUES (?, ?, 'earn_redeem', 105, 'available', 105, 105, 0, 0,
                   'earn_subscription', ?, ?)"#,
    )
    .bind(user_id)
    .bind(usdt_asset_id)
    .bind(earn_subscription_id.to_string())
    .bind(period_start_at)
    .execute(&pool)
    .await?;
    // 同一订阅的历史重复赎回流水不能把本金和收益重复计入首页聚合。
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id, created_at)
           VALUES (?, ?, 'earn_redeem', 105, 'available', 210, 210, 0, 0,
                   'earn_subscription', ?, ?)"#,
    )
    .bind(user_id)
    .bind(usdt_asset_id)
    .bind(earn_subscription_id.to_string())
    .bind(period_start_at)
    .execute(&pool)
    .await?;
    let other_earn_subscription_id = sqlx::query(
        r#"INSERT INTO earn_subscriptions
           (user_id, product_id, asset_id, amount, apr_rate, term_days, status,
            idempotency_key, subscribed_at, matures_at, redeemed_at)
           VALUES (?, ?, ?, 100, 0.12000000, 30, 'redeemed', ?, ?, ?, ?)"#,
    )
    .bind(other_user_id)
    .bind(earn_product_id)
    .bind(usdt_asset_id)
    .bind(format!("today-return-other-earn-{suffix}"))
    .bind(period_start_at)
    .bind(period_start_at)
    .bind(period_start_at)
    .execute(&pool)
    .await?
    .last_insert_id();
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id, created_at)
           VALUES (?, ?, 'earn_redeem', 1000, 'available', 1000, 1000, 0, 0,
                   'earn_subscription', ?, ?)"#,
    )
    .bind(other_user_id)
    .bind(usdt_asset_id)
    .bind(other_earn_subscription_id.to_string())
    .bind(period_start_at)
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id, created_at)
           VALUES (?, ?, 'deposit_credit', 9999, 'available', 9999, 9999, 0, 0,
                   'deposit_record', ?, ?)"#,
    )
    .bind(user_id)
    .bind(usdt_asset_id)
    .bind(format!("ignored-deposit-{suffix}"))
    .bind(period_start_at)
    .execute(&pool)
    .await?;

    let complete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/wallet/today-return")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    let complete_payload = body_json(complete_response).await?;
    assert_eq!(complete_payload["scope"], "realized");
    assert_eq!(complete_payload["reporting_asset"], "USDT");
    assert_eq!(complete_payload["amount"], "6.500000000000000000");
    assert_eq!(complete_payload["basis_amount"], "171.000000000000000000");
    assert_eq!(complete_payload["rate"], "0.038011695906432748");
    assert_eq!(complete_payload["status"], "complete");
    assert_eq!(complete_payload["missing_price_assets"], json!([]));

    let history_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/wallet/return-history?days=7")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(history_response.status(), StatusCode::OK);
    let history_payload = body_json(history_response).await?;
    let history_points = history_payload["points"].as_array().unwrap();
    assert_eq!(history_payload["scope"], "realized");
    assert_eq!(history_payload["reporting_asset"], "USDT");
    assert_eq!(history_payload["period_days"], 7);
    assert_eq!(history_payload["status"], "complete");
    assert_eq!(
        history_payload["summary"]["amount"],
        complete_payload["amount"]
    );
    assert_eq!(
        history_payload["summary"]["basis_amount"],
        complete_payload["basis_amount"]
    );
    assert_eq!(history_payload["summary"]["rate"], complete_payload["rate"]);
    assert_eq!(history_points.len(), 7);
    for points in history_points.windows(2) {
        assert_eq!(
            points[1]["day_start_at"].as_i64().unwrap()
                - points[0]["day_start_at"].as_i64().unwrap(),
            86_400_000
        );
    }
    for point in &history_points[..6] {
        assert_eq!(point["amount"], "0.000000000000000000");
        assert_eq!(point["status"], "complete");
    }
    assert_eq!(history_points[6]["amount"], complete_payload["amount"]);
    assert_eq!(
        history_points[6]["cumulative_amount"],
        complete_payload["amount"]
    );

    sqlx::query(
        r#"INSERT INTO prediction_orders
           (user_id, market_id, quote_id, idempotency_key, outcome, asset_id,
            stake_amount, fee_amount, accepted_price, shares, theoretical_payout,
            effective_payout_cap, status, result, payout_amount, settled_at)
           VALUES (?, ?, ?, ?, 'yes', ?, 2, 0, 0.5, 4, 4, 4,
                   'settled', 'yes', 3, ?)"#,
    )
    .bind(user_id)
    .bind(prediction_market_id)
    .bind(format!("today-return-missing-quote-{suffix}"))
    .bind(format!("today-return-missing-{suffix}"))
    .bind(base_asset_id)
    .bind(period_start_at)
    .execute(&pool)
    .await?;
    let partial_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/wallet/today-return")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    let partial_payload = body_json(partial_response).await?;
    assert_eq!(partial_payload["status"], "partial");
    assert_eq!(partial_payload["amount"], "6.500000000000000000");
    assert_eq!(
        partial_payload["missing_price_assets"],
        json!([base_symbol.clone()])
    );

    let partial_history_response = app
        .oneshot(
            Request::builder()
                .uri("/wallet/return-history?days=1")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    let partial_history_payload = body_json(partial_history_response).await?;
    assert_eq!(partial_history_payload["status"], "partial");
    assert_eq!(partial_history_payload["summary"]["amount"], Value::Null);
    assert_eq!(
        partial_history_payload["summary"]["basis_amount"],
        Value::Null
    );
    assert_eq!(partial_history_payload["summary"]["rate"], Value::Null);
    assert_eq!(partial_history_payload["points"][0]["amount"], Value::Null);
    assert_eq!(
        partial_history_payload["points"][0]["cumulative_amount"],
        Value::Null
    );
    assert_eq!(partial_history_payload["points"][0]["status"], "partial");
    assert_eq!(
        partial_history_payload["points"][0]["missing_price_assets"],
        json!([base_symbol])
    );

    sqlx::query("DELETE FROM wallet_ledger WHERE user_id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_ledger WHERE user_id = ?")
        .bind(other_user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM seconds_contract_orders WHERE user_id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM seconds_contract_orders WHERE user_id = ?")
        .bind(other_user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM prediction_orders WHERE user_id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM prediction_orders WHERE user_id = ?")
        .bind(other_user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM margin_positions WHERE user_id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM margin_positions WHERE user_id = ?")
        .bind(other_user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM earn_subscriptions WHERE user_id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM earn_subscriptions WHERE user_id = ?")
        .bind(other_user_id)
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
    sqlx::query("DELETE FROM earn_products WHERE id = ?")
        .bind(earn_product_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM prediction_markets WHERE id = ?")
        .bind(prediction_market_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM trading_pairs WHERE id = ?")
        .bind(pair_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(base_asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(other_user_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn wallet_deposit_address_is_assigned_from_pool_and_reused() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let other_user_id = create_user(&pool).await;
    let (asset_id, symbol) = create_deposit_asset(&pool).await;
    upsert_deposit_network_config(&pool, "tron", "C").await;
    let first_suffix = Uuid::now_v7().simple().to_string();
    let second_suffix = Uuid::now_v7().simple().to_string();
    let first_address = format!("TDeposit{}", &first_suffix[..24]);
    let second_address = format!("TDeposit{}", &second_suffix[..24]);
    sqlx::query(
        r#"INSERT INTO deposit_address_pool (network, address_group_code, address, asset_symbols_json, status)
           VALUES ('tron', 'C', ?, JSON_ARRAY(?), 'available')"#,
    )
    .bind(&first_address)
    .bind(&symbol)
    .execute(&pool)
    .await?;
    sqlx::query(
        "INSERT INTO deposit_address_pool (network, address_group_code, address, status) VALUES ('tron', 'C', ?, 'available')",
    )
    .bind(&second_address)
    .execute(&pool)
    .await?;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let other_token = issue_token(
        &settings,
        format!("user:{other_user_id}"),
        TokenScope::User,
        900,
    )
    .unwrap();
    let app = routes().with_state(AppState::new(settings.clone()).with_mysql(pool.clone()));
    let request_body =
        json!({ "asset_symbol": symbol.to_ascii_lowercase(), "network": "trc20" }).to_string();

    let created = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallet/deposit-address")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body.clone()))
                .unwrap(),
        )
        .await?;
    let created_status = created.status();
    let created_body = axum::body::to_bytes(created.into_body(), 8192).await?;
    assert_eq!(
        created_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&created_body)
    );
    let created_payload: Value = serde_json::from_slice(&created_body)?;
    assert_eq!(created_payload["network"], "tron");
    assert_eq!(created_payload["asset_symbol"], symbol);
    assert_eq!(created_payload["address"], first_address);

    let reused = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallet/deposit-address")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body.clone()))
                .unwrap(),
        )
        .await?;
    let reused_payload: Value =
        serde_json::from_slice(&axum::body::to_bytes(reused.into_body(), 8192).await?)?;
    assert_eq!(reused_payload["id"], created_payload["id"]);
    assert_eq!(reused_payload["address"], first_address);

    let other = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallet/deposit-address")
                .header("authorization", format!("Bearer {other_token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await?;
    let other_status = other.status();
    let other_payload: Value =
        serde_json::from_slice(&axum::body::to_bytes(other.into_body(), 8192).await?)?;
    assert_eq!(other_status, StatusCode::OK, "payload: {other_payload}");
    assert_eq!(other_payload["address"], second_address);

    let assigned_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM deposit_address_pool WHERE network = 'tron' AND assigned_asset_symbol = ? AND status = 'assigned'",
    )
    .bind(&symbol)
    .fetch_one(&pool)
    .await?;
    assert_eq!(assigned_count, 2);

    sqlx::query("DELETE FROM deposit_address_pool WHERE address IN (?, ?)")
        .bind(&first_address)
        .bind(&second_address)
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
    Ok(())
}

#[tokio::test]
async fn wallet_base_deposit_can_use_eth_address_pool() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (asset_id, symbol) = create_deposit_asset(&pool).await;
    upsert_deposit_network_config(&pool, "eth", "A").await;
    upsert_deposit_network_config(&pool, "base", "A").await;
    let suffix = Uuid::now_v7().simple().to_string();
    let address_hex = format!("{suffix}{suffix}");
    let eth_address = format!("0x{}", &address_hex[..40]);
    sqlx::query(
        r#"INSERT INTO deposit_address_pool (network, address_group_code, address, asset_symbols_json, status)
           VALUES ('eth', 'A', ?, JSON_ARRAY(?), 'available')"#,
    )
    .bind(&eth_address)
    .bind(&symbol)
    .execute(&pool)
    .await?;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let base_request =
        json!({ "asset_symbol": symbol.to_ascii_lowercase(), "network": "base" }).to_string();

    let base_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallet/deposit-address")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(base_request.clone()))
                .unwrap(),
        )
        .await?;
    let base_status = base_response.status();
    let base_payload: Value =
        serde_json::from_slice(&axum::body::to_bytes(base_response.into_body(), 8192).await?)?;
    assert_eq!(base_status, StatusCode::OK, "payload: {base_payload}");
    assert_eq!(base_payload["network"], "base");
    assert_eq!(base_payload["asset_symbol"], symbol);
    assert_eq!(base_payload["address"], eth_address);

    let eth_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallet/deposit-address")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "asset_symbol": symbol, "network": "eth" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let eth_payload: Value =
        serde_json::from_slice(&axum::body::to_bytes(eth_response.into_body(), 8192).await?)?;
    assert_eq!(eth_payload["id"], base_payload["id"]);
    assert_eq!(eth_payload["network"], "eth");
    assert_eq!(eth_payload["address"], eth_address);

    sqlx::query("DELETE FROM deposit_address_pool WHERE address = ?")
        .bind(&eth_address)
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
async fn wallet_deposit_assets_only_include_enabled_assets_and_reject_disabled_deposits()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (enabled_asset_id, enabled_symbol) = create_deposit_asset(&pool).await;
    let (disabled_asset_id, disabled_symbol) = create_deposit_asset(&pool).await;
    let (withdraw_only_asset_id, withdraw_only_symbol) = create_deposit_asset(&pool).await;
    upsert_deposit_network_config(&pool, "tron", "C").await;
    sqlx::query(
        r#"UPDATE assets
           SET min_deposit_amount = ?,
               deposit_fee = ?,
               withdraw_fee = ?
           WHERE id = ?"#,
    )
    .bind(decimal("5.000000000000000000"))
    .bind(decimal("0.100000000000000000"))
    .bind(decimal("0.250000000000000000"))
    .bind(enabled_asset_id)
    .execute(&pool)
    .await?;
    sqlx::query("UPDATE assets SET deposit_enabled = FALSE WHERE id = ?")
        .bind(disabled_asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("UPDATE assets SET deposit_enabled = FALSE, withdraw_enabled = TRUE WHERE id = ?")
        .bind(withdraw_only_asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("UPDATE assets SET withdraw_enabled = FALSE WHERE id = ?")
        .bind(enabled_asset_id)
        .execute(&pool)
        .await?;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let listed = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/wallet/deposit-assets")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let listed_status = listed.status();
    let listed_payload: Value =
        serde_json::from_slice(&axum::body::to_bytes(listed.into_body(), 1_048_576).await?)?;
    assert_eq!(listed_status, StatusCode::OK, "payload: {listed_payload}");
    let symbols: Vec<&str> = listed_payload["assets"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|asset| asset["symbol"].as_str())
        .collect();
    assert!(symbols.contains(&enabled_symbol.as_str()));
    assert!(!symbols.contains(&disabled_symbol.as_str()));
    assert!(!symbols.contains(&withdraw_only_symbol.as_str()));
    let listed_enabled = listed_payload["assets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|asset| asset["symbol"].as_str() == Some(enabled_symbol.as_str()))
        .unwrap();
    assert_eq!(listed_enabled["min_deposit_amount"], "5.000000000000000000");
    assert_eq!(listed_enabled["deposit_fee"], "0.100000000000000000");
    assert_eq!(listed_enabled["withdraw_fee"], "0.250000000000000000");
    assert!(
        listed_enabled["withdraw_fee_tiers"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(listed_enabled["deposit_enabled"], true);
    assert_eq!(listed_enabled["withdraw_enabled"], false);

    let withdraw_listed = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/wallet/withdraw-assets")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let withdraw_listed_status = withdraw_listed.status();
    let withdraw_listed_payload: Value = serde_json::from_slice(
        &axum::body::to_bytes(withdraw_listed.into_body(), 1_048_576).await?,
    )?;
    assert_eq!(
        withdraw_listed_status,
        StatusCode::OK,
        "payload: {withdraw_listed_payload}"
    );
    let withdraw_symbols: Vec<&str> = withdraw_listed_payload["assets"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|asset| asset["symbol"].as_str())
        .collect();
    assert!(!withdraw_symbols.contains(&enabled_symbol.as_str()));
    assert!(withdraw_symbols.contains(&disabled_symbol.as_str()));
    assert!(withdraw_symbols.contains(&withdraw_only_symbol.as_str()));
    let withdraw_enabled_asset = withdraw_listed_payload["assets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|asset| asset["symbol"].as_str() == Some(withdraw_only_symbol.as_str()))
        .unwrap();
    assert!(
        withdraw_enabled_asset["withdraw_fee_tiers"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let rejected = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallet/deposit-address")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "asset_symbol": disabled_symbol, "network": "trc20" }).to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let rejected_status = rejected.status();
    let rejected_payload: Value =
        serde_json::from_slice(&axum::body::to_bytes(rejected.into_body(), 1_048_576).await?)?;
    assert_eq!(
        rejected_status,
        StatusCode::BAD_REQUEST,
        "payload: {rejected_payload}"
    );
    assert_eq!(rejected_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        rejected_payload["message"],
        "validation error: asset does not support deposit"
    );

    sqlx::query("DELETE FROM assets WHERE id IN (?, ?, ?)")
        .bind(enabled_asset_id)
        .bind(disabled_asset_id)
        .bind(withdraw_only_asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn wallet_deposit_observation_credits_once_and_reorg_reverses_once()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (asset_id, asset_symbol) = create_deposit_asset(&pool).await;
    upsert_deposit_network_config(&pool, "tron", "C").await;
    let address = format!("T{}", Uuid::now_v7().simple());
    sqlx::query(
        r#"INSERT INTO deposit_address_pool
           (network, address_group_code, address, status, assigned_user_id,
            assigned_asset_symbol, assigned_at)
           VALUES ('tron', 'C', ?, 'assigned', ?, ?, CURRENT_TIMESTAMP(6))"#,
    )
    .bind(&address)
    .bind(user_id)
    .bind(&asset_symbol)
    .execute(&pool)
    .await?;
    let (admin_role_id, admin_id) = create_admin(&pool).await;
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let tx_hash = format!("0x{}", Uuid::now_v7().simple());
    let observe_body = |confirmations: u32| {
        json!({
            "asset_symbol": asset_symbol,
            "network": "tron",
            "address": address,
            "tx_hash": tx_hash,
            "event_index": 0,
            "amount": "3.000000000000000000",
            "block_height": 100,
            "confirmations": confirmations
        })
        .to_string()
    };

    let observed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallet/deposits/observe")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(observe_body(11)))
                .unwrap(),
        )
        .await?;
    assert_eq!(observed.status(), StatusCode::OK);
    let observed_payload = body_json(observed).await?;
    let deposit_id = observed_payload["id"].as_u64().unwrap();
    assert_eq!(observed_payload["status"], "observed");

    for confirmations in [12, 20] {
        let credited = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/wallet/deposits/observe")
                    .header("authorization", format!("Bearer {admin_token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(observe_body(confirmations)))
                    .unwrap(),
            )
            .await?;
        assert_eq!(credited.status(), StatusCode::OK);
        let credited_payload = body_json(credited).await?;
        assert_eq!(credited_payload["status"], "credited");
        assert_eq!(credited_payload["id"], deposit_id);
    }
    let credited_available: BigDecimal = sqlx::query_scalar(
        "SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(credited_available, decimal("3.000000000000000000"));
    let credit_ledger_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'wallet_deposit_event' AND ref_id = ? AND change_type = 'deposit_confirm'",
    )
    .bind(deposit_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(credit_ledger_count, 1);

    for _ in 0..2 {
        let reversed = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/wallet/deposits/{deposit_id}/reverse"))
                    .header("authorization", format!("Bearer {admin_token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"reason":"chain reorganization"}"#))
                    .unwrap(),
            )
            .await?;
        assert_eq!(reversed.status(), StatusCode::OK);
        let reversed_payload = body_json(reversed).await?;
        assert_eq!(reversed_payload["status"], "reversed");
    }
    let reversed_available: BigDecimal = sqlx::query_scalar(
        "SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(reversed_available, decimal("0.000000000000000000"));
    let reverse_ledger_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'wallet_deposit_event' AND ref_id = ? AND change_type = 'deposit_reorg_reverse'",
    )
    .bind(deposit_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(reverse_ledger_count, 1);

    sqlx::query("DELETE FROM wallet_ledger WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_deposit_events WHERE id = ?")
        .bind(deposit_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM deposit_address_pool WHERE address = ?")
        .bind(&address)
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
        .bind(admin_role_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn wallet_withdrawal_requires_fund_password_and_records_pending_request()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (asset_id, _) = create_asset(&pool).await;
    let asset_symbol: String = sqlx::query_scalar("SELECT symbol FROM assets WHERE id = ?")
        .bind(asset_id)
        .fetch_one(&pool)
        .await?;
    sqlx::query("UPDATE assets SET withdraw_fee = ? WHERE id = ?")
        .bind(decimal("0.250000000000000000"))
        .bind(asset_id)
        .execute(&pool)
        .await?;
    let ref_id = format!("wallet-withdraw-route-{}", Uuid::now_v7().simple());
    seed_wallet(&pool, user_id, asset_id, &ref_id).await;
    seed_fund_password(&pool, user_id, "123456").await;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = routes().with_state(AppState::new(settings.clone()).with_mysql(pool.clone()));
    let withdrawal_key = format!("withdraw-create-{}", Uuid::now_v7().simple());

    let missing_security_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallet/withdrawals")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "asset_symbol": asset_symbol.to_ascii_lowercase(),
                        "network": "trc20",
                        "address": "TWithdrawAddress",
                        "amount": "2.000000000000000000",
                        "fee": "0.100000000000000000",
                        "idempotency_key": format!("withdraw-missing-security-{}", Uuid::now_v7().simple())
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_security_response.status(), StatusCode::BAD_REQUEST);
    let missing_body = axum::body::to_bytes(missing_security_response.into_body(), 8192).await?;
    let missing_payload: Value = serde_json::from_slice(&missing_body)?;
    assert_eq!(missing_payload["code"], "security_verification_required");

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallet/withdrawals")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "asset_symbol": asset_symbol.to_ascii_lowercase(),
                        "network": "trc20",
                        "address": "TWithdrawAddress",
                        "amount": "2.000000000000000000",
                        "fee": "0.100000000000000000",
                        "idempotency_key": withdrawal_key.clone(),
                        "fund_password": "123456"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let create_status = create_response.status();
    let create_body = axum::body::to_bytes(create_response.into_body(), 8192).await?;
    assert_eq!(
        create_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&create_body)
    );
    let created: Value = serde_json::from_slice(&create_body)?;
    let withdrawal_id = created["id"].as_u64().unwrap();
    assert_eq!(created["status"], "pending_review");
    assert_eq!(created["total_reserved"], "2.250000000000000000");
    assert_eq!(created["security_method"], "fund_password");

    let stored: (
        u64,
        String,
        Option<String>,
        String,
        BigDecimal,
        BigDecimal,
        BigDecimal,
        String,
        String,
    ) = sqlx::query_as(
        r#"SELECT user_id, asset_symbol, network, address, amount, fee, total_reserved,
                  status, security_method
               FROM wallet_withdrawal_requests
               WHERE id = ?"#,
    )
    .bind(withdrawal_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(stored.0, user_id);
    assert_eq!(stored.1, asset_symbol);
    assert_eq!(stored.2.as_deref(), Some("tron"));
    assert_eq!(stored.3, "TWithdrawAddress");
    assert_eq!(stored.4, decimal("2.000000000000000000"));
    assert_eq!(stored.5, decimal("0.250000000000000000"));
    assert_eq!(stored.6, decimal("2.250000000000000000"));
    assert_eq!(stored.7, "pending_review");
    assert_eq!(stored.8, "fund_password");

    let replay_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallet/withdrawals")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "asset_symbol": asset_symbol.to_ascii_lowercase(),
                        "network": "tron",
                        "address": "TWithdrawAddress",
                        "amount": "2.000000000000000000",
                        "fee": "0.000000000000000000",
                        "idempotency_key": withdrawal_key
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(replay_response.status(), StatusCode::OK);
    let replay_payload: Value =
        serde_json::from_slice(&axum::body::to_bytes(replay_response.into_body(), 8192).await?)?;
    assert_eq!(replay_payload["id"], withdrawal_id);
    let (available_after_reserve, frozen_after_reserve): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(available_after_reserve, decimal("10.250000000000000000"));
    assert_eq!(frozen_after_reserve, decimal("3.750000000000000000"));
    let reserve_ledger_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'wallet_withdrawal_request' AND ref_id = ? AND change_type = 'withdrawal_reserve'",
    )
    .bind(withdrawal_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(reserve_ledger_count, 1);

    let (admin_role_id, admin_id) = create_admin(&pool).await;
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let admin_app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    for _ in 0..2 {
        let rejected = admin_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/wallet/withdrawals/{withdrawal_id}/reject"))
                    .header("authorization", format!("Bearer {admin_token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"reason":"risk review rejected"}"#))
                    .unwrap(),
            )
            .await?;
        assert_eq!(rejected.status(), StatusCode::OK);
    }
    let (available_after_reject, frozen_after_reject): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(available_after_reject, decimal("12.500000000000000000"));
    assert_eq!(frozen_after_reject, decimal("1.500000000000000000"));
    let release_ledger_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'wallet_withdrawal_request' AND ref_id = ? AND change_type = 'withdrawal_release'",
    )
    .bind(withdrawal_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(release_ledger_count, 1);

    cleanup_wallet_route_fixture(&pool, user_id, asset_id).await?;
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(admin_role_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn wallet_withdrawal_uses_tiered_withdraw_fee_when_amount_matches()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (asset_id, _) = create_asset(&pool).await;
    let asset_symbol: String = sqlx::query_scalar("SELECT symbol FROM assets WHERE id = ?")
        .bind(asset_id)
        .fetch_one(&pool)
        .await?;
    sqlx::query(
        r#"UPDATE assets
           SET withdraw_fee = ?,
               withdraw_fee_tiers_json = ?
           WHERE id = ?"#,
    )
    .bind(decimal("0.250000000000000000"))
    .bind(
        r#"[
          {"min_amount":"1","max_amount":"10","fee_rate_percent":"2"},
          {"min_amount":"10","fee_rate_percent":"3"}
        ]"#,
    )
    .bind(asset_id)
    .execute(&pool)
    .await?;
    let ref_id = format!("wallet-withdraw-tier-{}", Uuid::now_v7().simple());
    seed_wallet(&pool, user_id, asset_id, &ref_id).await;
    seed_fund_password(&pool, user_id, "123456").await;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = routes().with_state(AppState::new(settings.clone()).with_mysql(pool.clone()));

    let create_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallet/withdrawals")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "asset_symbol": asset_symbol.to_ascii_lowercase(),
                        "network": "trc20",
                        "address": "TWithdrawAddress",
                        "amount": "2.000000000000000000",
                        "fee": "0.000000000000000000",
                        "idempotency_key": format!("withdraw-tier-{}", Uuid::now_v7().simple()),
                        "fund_password": "123456"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let create_status = create_response.status();
    let create_body = axum::body::to_bytes(create_response.into_body(), 8192).await?;
    assert_eq!(
        create_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&create_body)
    );
    let created: Value = serde_json::from_slice(&create_body)?;
    let withdrawal_id = created["id"].as_u64().unwrap();

    let stored_fee: BigDecimal =
        sqlx::query_scalar("SELECT fee FROM wallet_withdrawal_requests WHERE id = ?")
            .bind(withdrawal_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(stored_fee, decimal("0.040000000000000000"));

    let (admin_role_id, admin_id) = create_admin(&pool).await;
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let admin_app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let approved = admin_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/wallet/withdrawals/{withdrawal_id}/approve"))
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"reason":"approved"}"#))
                .unwrap(),
        )
        .await?;
    assert_eq!(approved.status(), StatusCode::OK);
    let broadcasted = admin_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/wallet/withdrawals/{withdrawal_id}/broadcast"))
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"tx_hash":"0xwithdrawalconfirmed","block_height":101,"confirmations":1}"#,
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(broadcasted.status(), StatusCode::OK);
    let broadcasted_payload = body_json(broadcasted).await?;
    assert_eq!(broadcasted_payload["broadcasted_by"], admin_id);
    for _ in 0..2 {
        let confirmed = admin_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/wallet/withdrawals/{withdrawal_id}/confirm"))
                    .header("authorization", format!("Bearer {admin_token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"block_height":102,"confirmations":12}"#))
                    .unwrap(),
            )
            .await?;
        assert_eq!(confirmed.status(), StatusCode::OK);
        let confirmed_payload = body_json(confirmed).await?;
        assert_eq!(confirmed_payload["confirmed_by"], admin_id);
    }
    let (withdrawal_status, broadcasted_by, confirmed_by): (String, Option<u64>, Option<u64>) =
        sqlx::query_as(
            "SELECT status, broadcasted_by, confirmed_by FROM wallet_withdrawal_requests WHERE id = ?",
        )
        .bind(withdrawal_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(withdrawal_status, "confirmed");
    assert_eq!(broadcasted_by, Some(admin_id));
    assert_eq!(confirmed_by, Some(admin_id));
    let (available, frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(available, decimal("10.460000000000000000"));
    assert_eq!(frozen, decimal("1.500000000000000000"));
    let confirm_ledger_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'wallet_withdrawal_request' AND ref_id = ? AND change_type = 'withdrawal_confirm'",
    )
    .bind(withdrawal_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(confirm_ledger_count, 1);

    cleanup_wallet_route_fixture(&pool, user_id, asset_id).await?;
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(admin_role_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn wallet_withdrawal_rejects_assets_with_withdraw_disabled() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (asset_id, _) = create_asset(&pool).await;
    let asset_symbol: String = sqlx::query_scalar("SELECT symbol FROM assets WHERE id = ?")
        .bind(asset_id)
        .fetch_one(&pool)
        .await?;
    sqlx::query("UPDATE assets SET withdraw_enabled = FALSE WHERE id = ?")
        .bind(asset_id)
        .execute(&pool)
        .await?;
    let ref_id = format!("wallet-withdraw-disabled-{}", Uuid::now_v7().simple());
    seed_wallet(&pool, user_id, asset_id, &ref_id).await;
    seed_fund_password(&pool, user_id, "123456").await;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallet/withdrawals")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "asset_symbol": asset_symbol.to_ascii_lowercase(),
                        "network": "trc20",
                        "address": "TWithdrawAddress",
                        "amount": "2.000000000000000000",
                        "fee": "0.100000000000000000",
                        "idempotency_key": format!("withdraw-disabled-{}", Uuid::now_v7().simple()),
                        "fund_password": "123456"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let response_status = response.status();
    let payload: Value =
        serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 8192).await?)?;
    assert_eq!(
        response_status,
        StatusCode::BAD_REQUEST,
        "payload: {payload}"
    );
    assert_eq!(payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        payload["message"],
        "validation error: asset does not support withdraw"
    );

    cleanup_wallet_route_fixture(&pool, user_id, asset_id).await?;
    Ok(())
}

#[tokio::test]
async fn wallet_withdrawal_risk_rule_rejects_over_limit_amount_without_reserving_balance()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (asset_id, _) = create_asset(&pool).await;
    let asset_symbol: String = sqlx::query_scalar("SELECT symbol FROM assets WHERE id = ?")
        .bind(asset_id)
        .fetch_one(&pool)
        .await?;
    sqlx::query("UPDATE assets SET withdraw_fee = ? WHERE id = ?")
        .bind(decimal("0.250000000000000000"))
        .bind(asset_id)
        .execute(&pool)
        .await?;
    let ref_id = format!("wallet-withdraw-risk-{}", Uuid::now_v7().simple());
    seed_wallet(&pool, user_id, asset_id, &ref_id).await;
    seed_fund_password(&pool, user_id, "123456").await;

    let rule_id = sqlx::query(
        r#"INSERT INTO risk_rules (rule_type, target_type, target_id, config_json, enabled)
           VALUES ('amount_limit', 'asset', ?, ?, TRUE)"#,
    )
    .bind(&asset_symbol)
    .bind(r#"{"operations":["wallet.withdrawal.create"],"max_amount":"1"}"#)
    .execute(&pool)
    .await?
    .last_insert_id();

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let rejected = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallet/withdrawals")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "asset_symbol": asset_symbol.clone(),
                        "network": "trc20",
                        "address": "TWithdrawAddress",
                        "amount": "2.000000000000000000",
                        "fee": "0.100000000000000000",
                        "idempotency_key": format!("withdraw-risk-{}", Uuid::now_v7().simple()),
                        "fund_password": "123456"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let rejected_status = rejected.status();
    let rejected_payload = body_json(rejected).await?;
    assert_eq!(
        rejected_status,
        StatusCode::FORBIDDEN,
        "payload: {rejected_payload}"
    );
    assert_eq!(rejected_payload["code"], "risk_amount_limit");
    assert_eq!(rejected_payload["message"], "金额超出风控限额");

    let (available, frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(available, decimal("12.500000000000000000"));
    assert_eq!(frozen, decimal("1.500000000000000000"));
    let withdrawal_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM wallet_withdrawal_requests WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(withdrawal_count, 0);

    let (event_type, decision, risk_level, reason): (String, String, String, Option<String>) =
        sqlx::query_as(
            "SELECT event_type, decision, risk_level, reason FROM risk_events WHERE user_id = ?",
        )
        .bind(user_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(event_type, "wallet.withdrawal.create");
    assert_eq!(decision, "reject");
    assert_eq!(risk_level, "high");
    assert_eq!(reason.as_deref(), Some("金额超出风控限额"));

    // 规则停用后必须恢复接入风控之前的行为。
    sqlx::query("UPDATE risk_rules SET enabled = FALSE WHERE id = ?")
        .bind(rule_id)
        .execute(&pool)
        .await?;
    let allowed = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/wallet/withdrawals")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "asset_symbol": asset_symbol,
                        "network": "trc20",
                        "address": "TWithdrawAddress",
                        "amount": "2.000000000000000000",
                        "fee": "0.100000000000000000",
                        "idempotency_key": format!("withdraw-risk-allowed-{}", Uuid::now_v7().simple()),
                        "fund_password": "123456"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    let allowed_status = allowed.status();
    let allowed_payload = body_json(allowed).await?;
    assert_eq!(allowed_status, StatusCode::OK, "payload: {allowed_payload}");
    assert_eq!(allowed_payload["total_reserved"], "2.250000000000000000");

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

#[tokio::test]
async fn admin_wallet_withdrawals_offset_paging_returns_disjoint_pages_and_filtered_total()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let (role_id, admin_id) = create_admin(&pool).await;
    let user_id = create_user(&pool).await;
    let (asset_id, _logo_url) = create_asset(&pool).await;
    let asset_symbol = sqlx::query_scalar::<_, String>("SELECT symbol FROM assets WHERE id = ?")
        .bind(asset_id)
        .fetch_one(&pool)
        .await?;

    let mut pending_ids = Vec::new();
    for (index, status) in [
        "pending_review",
        "pending_review",
        "pending_review",
        "rejected",
    ]
    .into_iter()
    .enumerate()
    {
        let id = sqlx::query(
            r#"INSERT INTO wallet_withdrawal_requests
               (user_id, asset_id, asset_symbol, network, address, amount, fee, total_reserved,
                status, security_method, idempotency_key, gateway_request_id)
               VALUES (?, ?, ?, 'ETH', '0xpaging', ?, 0, ?, ?, 'fund_password', ?, UUID())"#,
        )
        .bind(user_id)
        .bind(asset_id)
        .bind(&asset_symbol)
        .bind(decimal("1.000000000000000000"))
        .bind(decimal("1.000000000000000000"))
        .bind(status)
        .bind(format!("wallet-page-{index}-{}", Uuid::now_v7().simple()))
        .execute(&pool)
        .await?
        .last_insert_id();
        if status == "pending_review" {
            pending_ids.push(id);
        }
    }

    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let mut page_ids = Vec::new();
    for offset in [0, 2] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/wallet/withdrawals?user_id={user_id}&status=pending_review&limit=2&offset={offset}"
                    ))
                    .header("authorization", format!("Bearer {admin_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await?;
        let status_code = response.status();
        let payload = body_json(response).await?;
        assert_eq!(status_code, StatusCode::OK, "payload: {payload}");
        // 总数必须反映筛选条件本身，而不是当前页行数。
        assert_eq!(payload["total"], 3, "payload: {payload}");
        page_ids.extend(
            payload["withdrawals"]
                .as_array()
                .unwrap()
                .iter()
                .map(|entry| entry["id"].as_u64().unwrap()),
        );
    }
    assert_eq!(page_ids.len(), 3);
    let mut unique_ids = page_ids.clone();
    unique_ids.sort_unstable();
    unique_ids.dedup();
    assert_eq!(unique_ids.len(), 3, "pages must not overlap: {page_ids:?}");
    pending_ids.sort_unstable();
    assert_eq!(unique_ids, pending_ids);

    let all_statuses = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/wallet/withdrawals?user_id={user_id}&limit=1"))
                .header("authorization", format!("Bearer {admin_token}"))
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
        all_statuses_payload["withdrawals"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(all_statuses_payload["total"], 4);

    cleanup_wallet_route_fixture(&pool, user_id, asset_id).await?;
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
