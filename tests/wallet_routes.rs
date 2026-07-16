use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use bigdecimal::BigDecimal;
use exchange_api::{
    config::Settings,
    modules::{
        auth::{TokenScope, hash_password, issue_token},
        wallet::routes::routes,
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
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

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
    assert_eq!(ledger["entries"][0]["fee"], "0.000000000000000000");

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
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
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
        serde_json::from_slice(&axum::body::to_bytes(listed.into_body(), 8192).await?)?;
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
    let withdraw_listed_payload: Value =
        serde_json::from_slice(&axum::body::to_bytes(withdraw_listed.into_body(), 8192).await?)?;
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
        serde_json::from_slice(&axum::body::to_bytes(rejected.into_body(), 8192).await?)?;
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
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

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
                        "fee": "0.100000000000000000"
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
    assert_eq!(created["status"], "pending");
    assert_eq!(created["security_method"], "fund_password");

    let stored: (
        u64,
        String,
        Option<String>,
        String,
        BigDecimal,
        BigDecimal,
        String,
        String,
    ) = sqlx::query_as(
        r#"SELECT user_id, asset_symbol, network, address, amount, fee, status, security_method
               FROM wallet_withdrawal_requests
               WHERE id = ?"#,
    )
    .bind(withdrawal_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(stored.0, user_id);
    assert_eq!(stored.1, asset_symbol);
    assert_eq!(stored.2.as_deref(), Some("trc20"));
    assert_eq!(stored.3, "TWithdrawAddress");
    assert_eq!(stored.4, decimal("2.000000000000000000"));
    assert_eq!(stored.5, decimal("0.250000000000000000"));
    assert_eq!(stored.6, "pending");
    assert_eq!(stored.7, "fund_password");

    cleanup_wallet_route_fixture(&pool, user_id, asset_id).await?;
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
    let app = routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

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

    cleanup_wallet_route_fixture(&pool, user_id, asset_id).await?;
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
