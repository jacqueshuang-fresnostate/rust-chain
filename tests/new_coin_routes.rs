use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use bigdecimal::BigDecimal;
use exchange_api::{
    config::Settings,
    modules::{
        auth::{TokenScope, issue_token},
        events::{EventBroadcastHub, WebSocketChannel},
        new_coin::routes::user_routes,
    },
    state::AppState,
};
use secrecy::SecretString;
use serde_json::Value;
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::{error::Error, str::FromStr, time::Duration};
use tokio::time::timeout;
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
            eprintln!("skipping MySQL new coin route test because DATABASE_URL is not set");
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
    let email = format!("new-coin-route-{}@example.test", Uuid::now_v7().simple());
    sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(email)
        .bind("not-a-real-hash")
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id()
}

async fn create_asset(pool: &MySqlPool, prefix: &str) -> (u64, String) {
    let suffix = Uuid::now_v7().simple().to_string();
    let symbol = format!("{prefix}{}", &suffix[..12]);
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

async fn create_new_coin_project(
    pool: &MySqlPool,
    asset_id: u64,
    symbol: &str,
    quote_asset_id: Option<u64>,
) -> u64 {
    create_new_coin_project_with_status(pool, asset_id, symbol, quote_asset_id, "listed").await
}

async fn create_new_coin_project_with_status(
    pool: &MySqlPool,
    asset_id: u64,
    symbol: &str,
    quote_asset_id: Option<u64>,
    lifecycle_status: &str,
) -> u64 {
    let unlock_fee_enabled = quote_asset_id.is_some();
    let unlock_fee_rate = unlock_fee_enabled.then(|| decimal("0.04000000"));
    let unlock_fee_basis = unlock_fee_enabled.then_some("market_value");
    sqlx::query(
        r#"INSERT INTO new_coin_projects
           (asset_id, symbol, lifecycle_status, total_supply, issue_price, quote_asset_id,
            reserved_supply, allocated_supply, remaining_supply, listed_at,
            unlock_type, fixed_unlock_at, unlock_fee_enabled, unlock_fee_rate,
            unlock_fee_basis, unlock_fee_asset, status)
           VALUES (?, ?, ?, ?, ?, ?, 0, 0, ?, CURRENT_TIMESTAMP(6), 'fixed_time',
                   DATE_ADD(CURRENT_TIMESTAMP(6), INTERVAL 7 DAY), ?, ?, ?, ?, 'active')"#,
    )
    .bind(asset_id)
    .bind(symbol)
    .bind(lifecycle_status)
    .bind(decimal("1000000.000000000000000000"))
    .bind(decimal("1.000000000000000000"))
    .bind(quote_asset_id)
    .bind(decimal("1000000.000000000000000000"))
    .bind(unlock_fee_enabled)
    .bind(unlock_fee_rate)
    .bind(unlock_fee_basis)
    .bind(quote_asset_id)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id()
}

async fn create_pair(
    pool: &MySqlPool,
    base_asset: u64,
    quote_asset: u64,
    base_symbol: &str,
    quote_symbol: &str,
) -> u64 {
    sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, 2, 4, ?, 'active', 'spot')"#,
    )
    .bind(base_asset)
    .bind(quote_asset)
    .bind(format!("{base_symbol}-{quote_symbol}"))
    .bind(decimal("1.000000000000000000"))
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id()
}

async fn seed_unlock_record(
    pool: &MySqlPool,
    user_id: u64,
    asset_id: u64,
    unlock_after_days: i64,
) -> String {
    let unlock_key = format!("unlock-route-{}", Uuid::now_v7().simple());
    let merge_key = format!("lock-route-{}", Uuid::now_v7().simple());
    let lock_position_id = sqlx::query(
        r#"INSERT INTO asset_lock_positions
           (user_id, asset_id, unlock_type, unlock_at, locked_amount, released_amount,
            remaining_amount, merge_key, status)
           VALUES (?, ?, 'fixed_time', DATE_ADD(CURRENT_TIMESTAMP(6), INTERVAL ? DAY), ?, 0, ?, ?, 'active')"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(unlock_after_days)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("10.000000000000000000"))
    .bind(merge_key)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id();

    sqlx::query(
        r#"INSERT INTO asset_unlock_records
           (user_id, asset_id, lock_position_id, unlock_quantity, unlock_price,
            unlock_fee_enabled, unlock_fee_rate, unlock_fee_basis, unlock_fee_asset,
            unlock_fee_amount, fee_paid_status, status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, true, ?, 'market_value', ?, ?, 'pending', 'pending', ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(lock_position_id)
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("5.000000000000000000"))
    .bind(decimal("0.04000000"))
    .bind(asset_id)
    .bind(decimal("2.000000000000000000"))
    .bind(&unlock_key)
    .execute(pool)
    .await
    .unwrap();

    unlock_key
}

#[tokio::test]
async fn new_coin_routes_require_auth_for_user_unlocks() {
    let response = user_routes()
        .with_state(AppState::new(test_settings()))
        .oneshot(
            Request::builder()
                .uri("/new-coins/unlocks")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn new_coin_routes_return_clear_error_without_mysql() {
    let settings = test_settings();
    let token = issue_token(&settings, "user:42", TokenScope::User, 900).unwrap();
    let response = user_routes()
        .with_state(AppState::new(settings))
        .oneshot(
            Request::builder()
                .uri("/new-coins/unlocks")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert_eq!(
        payload["message"],
        "internal error: mysql pool is not configured for new coin routes"
    );
}

#[tokio::test]
async fn public_new_coin_routes_return_authoritative_asset_metadata() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let (asset_id, symbol) = create_asset(&pool, "NM").await;
    let (quote_asset_id, quote_symbol) = create_asset(&pool, "NQ").await;
    let project_id = create_new_coin_project(&pool, asset_id, &symbol, Some(quote_asset_id)).await;
    let project_name = format!("{symbol} launch");
    let project_logo = format!("https://assets.example.test/{symbol}.png");
    let quote_logo = format!("https://assets.example.test/{quote_symbol}.png");
    sqlx::query("UPDATE assets SET name = ?, logo_url = ? WHERE id = ?")
        .bind(&project_name)
        .bind(&project_logo)
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("UPDATE assets SET logo_url = ? WHERE id = ?")
        .bind(&quote_logo)
        .bind(quote_asset_id)
        .execute(&pool)
        .await?;

    let app = user_routes().with_state(AppState::new(test_settings()).with_mysql(pool.clone()));
    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/new-coins?limit=100")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = axum::body::to_bytes(list_response.into_body(), 131072).await?;
    let list_payload: Value = serde_json::from_slice(&list_body)?;
    let listed_project = list_payload["projects"]
        .as_array()
        .and_then(|projects| projects.iter().find(|project| project["id"] == project_id))
        .expect("seeded project must be present")
        .clone();

    let detail_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/new-coins/{symbol}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(detail_response.status(), StatusCode::OK);
    let detail_body = axum::body::to_bytes(detail_response.into_body(), 131072).await?;
    let detail_project: Value = serde_json::from_slice(&detail_body)?;

    assert_eq!(detail_project, listed_project);
    assert_eq!(detail_project["name"], project_name);
    assert_eq!(detail_project["logo_url"], project_logo);
    assert_eq!(detail_project["quote_asset_id"], quote_asset_id);
    assert_eq!(detail_project["quote_asset_symbol"], quote_symbol);
    assert_eq!(detail_project["quote_asset_logo_url"], quote_logo);

    sqlx::query("DELETE FROM new_coin_projects WHERE id = ?")
        .bind(project_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id IN (?, ?)")
        .bind(asset_id)
        .bind(quote_asset_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn new_coin_routes_list_projects_and_allow_fee_payment() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (asset_id, symbol) = create_asset(&pool, "NC").await;
    let project_id = create_new_coin_project(&pool, asset_id, &symbol, None).await;
    let unlock_key = seed_unlock_record(&pool, user_id, asset_id, 0).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(asset_id)
        .bind(decimal("10.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = user_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/new-coins?limit=100")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = axum::body::to_bytes(list_response.into_body(), 131072).await?;
    let projects: Value = serde_json::from_slice(&list_body)?;
    assert!(
        projects["projects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|project| {
                project["id"] == project_id
                    && project["symbol"] == symbol
                    && project["lifecycle_status"] == "listed"
                    && project["name"] == symbol
                    && project["logo_url"].is_null()
                    && project["quote_asset_id"].is_null()
                    && project["quote_asset_symbol"].is_null()
                    && project["quote_asset_logo_url"].is_null()
                    && project["post_listing_purchase_enabled"] == false
                    && project["post_listing_pair_id"].is_null()
            }),
        "payload: {projects}"
    );

    let unlocks_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/new-coins/unlocks")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unlocks_response.status(), StatusCode::OK);
    let unlocks_body = axum::body::to_bytes(unlocks_response.into_body(), 8192).await?;
    let unlocks: Value = serde_json::from_slice(&unlocks_body)?;
    assert!(unlocks["unlocks"].as_array().unwrap().iter().any(|unlock| {
        unlock["idempotency_key"] == unlock_key && unlock["fee_paid_status"] == "pending"
    }));

    // Existing immutable fee receivables remain payable after an asset is disabled. This avoids
    // stranding a locked allocation merely because the asset lifecycle changed after issuance.
    sqlx::query("UPDATE assets SET status = 'disabled' WHERE id = ?")
        .bind(asset_id)
        .execute(&pool)
        .await?;

    let first_payment = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(format!("/new-coins/unlocks/{unlock_key}/pay-fee"))
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(format!(
                r#"{{"payment_asset_id":{asset_id},"amount":"2.000000000000000000"}}"#
            )))
            .unwrap(),
    );
    let second_payment = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(format!("/new-coins/unlocks/{unlock_key}/pay-fee"))
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(format!(
                r#"{{"payment_asset_id":{asset_id},"amount":"2.000000000000000000"}}"#
            )))
            .unwrap(),
    );
    let (first_payment, second_payment) = tokio::join!(first_payment, second_payment);
    let mut paid_flags = Vec::new();
    for response in [first_payment?, second_payment?] {
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 8192).await?;
        let payload: Value = serde_json::from_slice(&body)?;
        assert_eq!(payload["unlock_idempotency_key"], unlock_key);
        paid_flags.push(payload["paid"].as_bool().expect("paid boolean"));
    }
    paid_flags.sort_unstable();
    assert_eq!(paid_flags, vec![false, true]);

    let (available, fee_status, fee_paid_at, payment_ledger_id): (
        BigDecimal,
        String,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<u64>,
    ) = sqlx::query_as(
        r#"SELECT wallets.available, unlocks.fee_paid_status, unlocks.fee_paid_at,
                  unlocks.unlock_fee_payment_ledger_id
           FROM asset_unlock_records unlocks
           INNER JOIN wallet_accounts wallets
             ON wallets.user_id = unlocks.user_id AND wallets.asset_id = unlocks.unlock_fee_asset
           WHERE unlocks.idempotency_key = ?"#,
    )
    .bind(&unlock_key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(available.normalized(), decimal("8").normalized());
    assert_eq!(fee_status, "paid");
    assert!(fee_paid_at.is_some());
    assert!(payment_ledger_id.is_some());
    let (ledger_count, journal_count, journal_sum): (i64, i64, BigDecimal) = sqlx::query_as(
        r#"SELECT
             (SELECT COUNT(*) FROM wallet_ledger
               WHERE ref_type = 'new_coin_unlock' AND ref_id = ?
                 AND change_type = 'new_coin_unlock_fee_payment'),
             (SELECT COUNT(*) FROM platform_financial_journal
               WHERE transaction_key = CONCAT('new_coin_unlock_fee:', ?)),
             (SELECT COALESCE(SUM(amount), 0) FROM platform_financial_journal
               WHERE transaction_key = CONCAT('new_coin_unlock_fee:', ?))"#,
    )
    .bind(&unlock_key)
    .bind(
        sqlx::query_scalar::<_, u64>(
            "SELECT id FROM asset_unlock_records WHERE idempotency_key = ?",
        )
        .bind(&unlock_key)
        .fetch_one(&pool)
        .await?,
    )
    .bind(
        sqlx::query_scalar::<_, u64>(
            "SELECT id FROM asset_unlock_records WHERE idempotency_key = ?",
        )
        .bind(&unlock_key)
        .fetch_one(&pool)
        .await?,
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 1);
    assert_eq!(journal_count, 2);
    assert_eq!(journal_sum, decimal("0"));

    let paid_replay = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/unlocks/{unlock_key}/pay-fee"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"payment_asset_id":{asset_id},"amount":"2.000000000000000000"}}"#
                )))?,
        )
        .await?;
    assert_eq!(paid_replay.status(), StatusCode::OK);
    let paid_replay = axum::body::to_bytes(paid_replay.into_body(), 8192).await?;
    let paid_replay: Value = serde_json::from_slice(&paid_replay)?;
    assert_eq!(paid_replay["paid"], false);

    cleanup_fixture(&pool, user_id, asset_id, project_id, &unlock_key).await?;
    Ok(())
}

#[tokio::test]
async fn new_coin_routes_reject_invalid_fee_payment_and_early_release() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (asset_id, symbol) = create_asset(&pool, "NF").await;
    let project_id = create_new_coin_project(&pool, asset_id, &symbol, None).await;
    let unlock_key = seed_unlock_record(&pool, user_id, asset_id, 7).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(asset_id)
        .bind(decimal("1.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = user_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let invalid_fee_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/unlocks/{unlock_key}/pay-fee"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"payment_asset_id":{asset_id},"amount":"0.000000000000000000"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_fee_response.status(), StatusCode::BAD_REQUEST);

    let insufficient_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/unlocks/{unlock_key}/pay-fee"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"payment_asset_id":{asset_id},"amount":"2.000000000000000000"}}"#
                )))
                .unwrap(),
        )
        .await?;
    assert_eq!(insufficient_response.status(), StatusCode::BAD_REQUEST);

    let release_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/unlocks/{unlock_key}/release"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(release_response.status(), StatusCode::BAD_REQUEST);

    // 单独篡改状态位不构成缴费证据：正数应收不能伪装成免费，
    // 也不能只写 paid 而缺少钱包流水和平台双腿分录。
    sqlx::query(
        "UPDATE asset_unlock_records SET fee_paid_status = 'not_required' WHERE idempotency_key = ?",
    )
    .bind(&unlock_key)
    .execute(&pool)
    .await?;
    let forged_not_required = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/unlocks/{unlock_key}/release"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(forged_not_required.status(), StatusCode::BAD_REQUEST);
    sqlx::query(
        r#"UPDATE asset_unlock_records
           SET unlock_fee_amount = NULL, fee_paid_status = 'not_required'
           WHERE idempotency_key = ?"#,
    )
    .bind(&unlock_key)
    .execute(&pool)
    .await?;
    let forged_null_snapshot = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/unlocks/{unlock_key}/release"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(forged_null_snapshot.status(), StatusCode::BAD_REQUEST);
    sqlx::query(
        r#"UPDATE asset_unlock_records
           SET unlock_fee_amount = 2, fee_paid_status = 'paid', fee_paid_at = CURRENT_TIMESTAMP(6),
               unlock_fee_payment_ledger_id = NULL
           WHERE idempotency_key = ?"#,
    )
    .bind(&unlock_key)
    .execute(&pool)
    .await?;
    let forged_paid = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/unlocks/{unlock_key}/release"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(forged_paid.status(), StatusCode::BAD_REQUEST);
    sqlx::query(
        r#"UPDATE asset_unlock_records
           SET fee_paid_status = 'pending', fee_paid_at = NULL,
               unlock_fee_payment_ledger_id = NULL
           WHERE idempotency_key = ?"#,
    )
    .bind(&unlock_key)
    .execute(&pool)
    .await?;

    let (fee_status, status): (String, String) = sqlx::query_as(
        "SELECT fee_paid_status, status FROM asset_unlock_records WHERE idempotency_key = ?",
    )
    .bind(&unlock_key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(fee_status, "pending");
    assert_eq!(status, "pending");
    let (available, ledger_count, journal_count): (BigDecimal, i64, i64) = sqlx::query_as(
        r#"SELECT wallets.available,
             (SELECT COUNT(*) FROM wallet_ledger
               WHERE ref_type = 'new_coin_unlock' AND ref_id = ?),
             (SELECT COUNT(*) FROM platform_financial_journal journal
               INNER JOIN asset_unlock_records unlocks
                 ON journal.transaction_key = CONCAT('new_coin_unlock_fee:', unlocks.id)
               WHERE unlocks.idempotency_key = ?)
           FROM wallet_accounts wallets
           WHERE wallets.user_id = ? AND wallets.asset_id = ?"#,
    )
    .bind(&unlock_key)
    .bind(&unlock_key)
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(available.normalized(), decimal("1").normalized());
    assert_eq!(ledger_count, 0);
    assert_eq!(journal_count, 0);

    cleanup_fixture(&pool, user_id, asset_id, project_id, &unlock_key).await?;
    Ok(())
}

#[tokio::test]
async fn new_coin_unlock_fee_rolls_back_wallet_and_paid_state_when_journal_write_fails()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (asset_id, symbol) = create_asset(&pool, "NB").await;
    let project_id = create_new_coin_project(&pool, asset_id, &symbol, None).await;
    let unlock_key = seed_unlock_record(&pool, user_id, asset_id, 0).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(asset_id)
        .bind(decimal("10.000000000000000000"))
        .execute(&pool)
        .await?;
    let unlock_id: u64 =
        sqlx::query_scalar("SELECT id FROM asset_unlock_records WHERE idempotency_key = ?")
            .bind(&unlock_key)
            .fetch_one(&pool)
            .await?;
    let transaction_key = format!("new_coin_unlock_fee:{unlock_id}");
    sqlx::query(
        r#"INSERT INTO platform_financial_journal
           (transaction_key, context, account_code, asset_id, amount, ref_type, ref_id)
           VALUES (?, 'new_coin_unlock_fee', 'user_unlock_fee_expense', ?, ?,
                   'new_coin_unlock', ?)"#,
    )
    .bind(&transaction_key)
    .bind(asset_id)
    .bind(decimal("-2.000000000000000000"))
    .bind(unlock_id.to_string())
    .execute(&pool)
    .await?;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900)?;
    let app = user_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/unlocks/{unlock_key}/pay-fee"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"payment_asset_id":{asset_id},"amount":"2.000000000000000000"}}"#
                )))
                .unwrap(),
        )
        .await?;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let (available, fee_status, fee_paid_at, payment_ledger_id, ledger_count): (
        BigDecimal,
        String,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<u64>,
        i64,
    ) = sqlx::query_as(
        r#"SELECT wallets.available, unlocks.fee_paid_status, unlocks.fee_paid_at,
                  unlocks.unlock_fee_payment_ledger_id,
                  (SELECT COUNT(*) FROM wallet_ledger ledger
                    WHERE ledger.ref_type = 'new_coin_unlock' AND ledger.ref_id = ?)
           FROM asset_unlock_records unlocks
           INNER JOIN wallet_accounts wallets
             ON wallets.user_id = unlocks.user_id AND wallets.asset_id = unlocks.unlock_fee_asset
           WHERE unlocks.idempotency_key = ?"#,
    )
    .bind(&unlock_key)
    .bind(&unlock_key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(available.normalized(), decimal("10").normalized());
    assert_eq!(fee_status, "pending");
    assert!(fee_paid_at.is_none());
    assert!(payment_ledger_id.is_none());
    assert_eq!(ledger_count, 0);

    let release = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/unlocks/{unlock_key}/release"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(release.status(), StatusCode::BAD_REQUEST);

    cleanup_fixture(&pool, user_id, asset_id, project_id, &unlock_key).await?;
    Ok(())
}

#[tokio::test]
async fn concurrent_new_coin_subscriptions_never_allocate_beyond_remaining_supply()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "NX").await;
    let (quote_asset, _quote_symbol) = create_asset(&pool, "NY").await;
    let project_id = create_new_coin_project_with_status(
        &pool,
        base_asset,
        &base_symbol,
        Some(quote_asset),
        "subscription",
    )
    .await;
    sqlx::query(
        r#"UPDATE new_coin_projects
           SET total_supply = 15, reserved_supply = 0,
               allocated_supply = 0, remaining_supply = 15
           WHERE id = ?"#,
    )
    .bind(project_id)
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, 100)")
        .bind(user_id)
        .bind(quote_asset)
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id) VALUES (?, ?)")
        .bind(user_id)
        .bind(base_asset)
        .execute(&pool)
        .await?;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900)?;
    let app = user_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let first_key = format!("supply-a-{}", Uuid::now_v7().simple());
    let second_key = format!("supply-b-{}", Uuid::now_v7().simple());
    let request = |key: &str| {
        Request::builder()
            .method("POST")
            .uri(format!("/new-coins/{base_symbol}/subscriptions"))
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(format!(
                r#"{{"quote_asset_id":{quote_asset},"quote_amount":"10","quantity":"10","idempotency_key":"{key}"}}"#
            )))
            .unwrap()
    };
    let (first, second) = tokio::join!(
        app.clone().oneshot(request(&first_key)),
        app.clone().oneshot(request(&second_key)),
    );
    let first = first?;
    let second = second?;
    let mut statuses = [first.status(), second.status()];
    statuses.sort_unstable();
    assert_eq!(statuses, [StatusCode::OK, StatusCode::BAD_REQUEST]);
    let winner_key = if first.status() == StatusCode::OK {
        &first_key
    } else {
        &second_key
    };

    let (reserved, allocated, remaining, order_count, quote_available, base_locked): (
        BigDecimal,
        BigDecimal,
        BigDecimal,
        i64,
        BigDecimal,
        BigDecimal,
    ) = sqlx::query_as(
        r#"SELECT projects.reserved_supply, projects.allocated_supply,
                  projects.remaining_supply,
                  (SELECT COUNT(*) FROM new_coin_subscriptions WHERE project_id = projects.id),
                  quote_wallet.available, base_wallet.locked
           FROM new_coin_projects projects
           INNER JOIN wallet_accounts quote_wallet
             ON quote_wallet.user_id = ? AND quote_wallet.asset_id = ?
           INNER JOIN wallet_accounts base_wallet
             ON base_wallet.user_id = ? AND base_wallet.asset_id = ?
           WHERE projects.id = ?"#,
    )
    .bind(user_id)
    .bind(quote_asset)
    .bind(user_id)
    .bind(base_asset)
    .bind(project_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(reserved, decimal("10"));
    assert_eq!(allocated.normalized(), decimal("0").normalized());
    assert_eq!(remaining.normalized(), decimal("5").normalized());
    assert_eq!(order_count, 1);
    assert_eq!(quote_available.normalized(), decimal("90").normalized());
    assert_eq!(base_locked.normalized(), decimal("0").normalized());

    cleanup_order_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        project_id,
        None,
        winner_key,
        "new_coin_subscription",
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn new_coin_subscription_freezes_quote_without_allocating_coins() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "NS").await;
    let (quote_asset, _quote_symbol) = create_asset(&pool, "NQ").await;
    let project_id = create_new_coin_project_with_status(
        &pool,
        base_asset,
        &base_symbol,
        Some(quote_asset),
        "subscription",
    )
    .await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, locked) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(base_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("new-sub-{}", Uuid::now_v7().simple());
    let hub = EventBroadcastHub::new(16);
    let _keepalive_hub = hub.clone();
    let mut private_events = hub.subscribe(&WebSocketChannel::private_user(user_id));
    let app = user_routes().with_state(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_event_broadcast_hub(hub),
    );

    let tampered_key = format!("new-sub-tampered-{}", Uuid::now_v7().simple());
    let tampered_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/{base_symbol}/subscriptions"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"quote_asset_id":{quote_asset},"quote_amount":"19.000000000000000000","quantity":"20.000000000000000000","idempotency_key":"{tampered_key}"}}"#
                )))
                .unwrap(),
        )
        .await?;
    assert_eq!(tampered_response.status(), StatusCode::BAD_REQUEST);
    assert!(
        timeout(Duration::from_millis(25), private_events.recv())
            .await
            .is_err()
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/{base_symbol}/subscriptions"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"quote_asset_id":{quote_asset},"quote_amount":"20.000000000000000000","quantity":"20.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
                )))
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
    let subscription: Value = serde_json::from_slice(&body)?;
    assert_eq!(subscription["idempotency_key"], idempotency_key);
    assert_eq!(subscription["status"], "pending");
    assert!(subscription["lock_position_id"].is_null());
    let event_message = timeout(Duration::from_millis(100), private_events.recv()).await??;
    let event: Value = serde_json::from_str(event_message.payload())?;
    assert_eq!(event["type"], "new_coin.subscription.created");
    assert_eq!(event["idempotency_key"], idempotency_key);
    assert_eq!(event["project_id"], project_id);
    assert_eq!(event["asset_id"], base_asset);
    assert_eq!(event["quote_asset_id"], quote_asset);
    assert_eq!(event["quote_amount"], "20.000000000000000000");
    assert_eq!(event["quantity"], "20.000000000000000000");
    assert_eq!(event["status"], "pending");
    assert!(event["lock_position_id"].is_null());

    let (quote_available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        quote_available.normalized(),
        decimal("80.000000000000000000").normalized()
    );

    let (base_locked,): (BigDecimal,) =
        sqlx::query_as("SELECT locked FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(base_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(base_locked.normalized(), decimal("0").normalized());

    let (quote_frozen,): (BigDecimal,) =
        sqlx::query_as("SELECT frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(quote_frozen, decimal("20"));
    let (source_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM asset_lock_position_sources WHERE source_type = 'new_coin_subscription' AND source_id = ?",
    )
    .bind(&idempotency_key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(source_count, 0);

    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'new_coin_subscription' AND ref_id = ?",
    )
    .bind(&idempotency_key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 2);

    let (total_supply, reserved_supply, allocated_supply, remaining_supply): (
        BigDecimal,
        BigDecimal,
        BigDecimal,
        BigDecimal,
    ) = sqlx::query_as(
        "SELECT total_supply, reserved_supply, allocated_supply, remaining_supply FROM new_coin_projects WHERE id = ?",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(reserved_supply, decimal("20"));
    assert_eq!(allocated_supply.normalized(), decimal("0").normalized());
    assert_eq!(
        remaining_supply.normalized(),
        decimal("999980").normalized()
    );
    assert_eq!(
        (reserved_supply + allocated_supply + remaining_supply).normalized(),
        total_supply.normalized()
    );

    // 迁移前订单没有运行时指纹；即使项目随后停用并推进生命周期，同参重试也必须回吐原结果。
    sqlx::query(
        "UPDATE new_coin_subscriptions SET request_fingerprint = NULL WHERE idempotency_key = ?",
    )
    .bind(&idempotency_key)
    .execute(&pool)
    .await?;
    sqlx::query(
        "UPDATE new_coin_projects SET status = 'disabled', lifecycle_status = 'listed' WHERE id = ?",
    )
    .bind(project_id)
    .execute(&pool)
    .await?;

    let duplicate_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/{base_symbol}/subscriptions"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"quote_asset_id":{quote_asset},"quote_amount":"20.000000000000000000","quantity":"20.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let duplicate_status = duplicate_response.status();
    let duplicate_body = axum::body::to_bytes(duplicate_response.into_body(), 8192).await?;
    assert_eq!(
        duplicate_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&duplicate_body)
    );
    let (quote_after_duplicate,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        quote_after_duplicate.normalized(),
        quote_available.normalized()
    );
    let (ledger_count_after_duplicate,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'new_coin_subscription' AND ref_id = ?",
    )
    .bind(&idempotency_key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count_after_duplicate, 2);
    assert!(
        timeout(Duration::from_millis(25), private_events.recv())
            .await
            .is_err()
    );

    let conflict_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/{base_symbol}/subscriptions"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"quote_asset_id":{quote_asset},"quote_amount":"21.000000000000000000","quantity":"21.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
                )))
                .unwrap(),
        )
        .await?;
    assert_eq!(conflict_response.status(), StatusCode::CONFLICT);

    cleanup_order_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        project_id,
        None,
        &idempotency_key,
        "new_coin_subscription",
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn new_coin_purchase_debits_quote_wallet_and_locks_fixed_time_allocation()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "NP").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "NT").await;
    let project_id =
        create_new_coin_project(&pool, base_asset, &base_symbol, Some(quote_asset)).await;
    let pair_id = create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query(
        "UPDATE new_coin_projects SET issue_price = 2, post_listing_purchase_enabled = TRUE, post_listing_pair_id = ? WHERE id = ?",
    )
    .bind(pair_id)
    .bind(project_id)
    .execute(&pool)
    .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, locked) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(base_asset)
        .bind(decimal("0.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("new-pur-{}", Uuid::now_v7().simple());
    let hub = EventBroadcastHub::new(16);
    let _keepalive_hub = hub.clone();
    let mut private_events = hub.subscribe(&WebSocketChannel::private_user(user_id));
    let app = user_routes().with_state(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_event_broadcast_hub(hub),
    );

    let tampered_key = format!("new-pur-tampered-{}", Uuid::now_v7().simple());
    let tampered_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/{base_symbol}/purchase"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"pair_id":{pair_id},"price":"1.990000000000000000","quantity":"10.000000000000000000","idempotency_key":"{tampered_key}"}}"#
                )))
                .unwrap(),
        )
        .await?;
    assert_eq!(tampered_response.status(), StatusCode::BAD_REQUEST);
    assert!(
        timeout(Duration::from_millis(25), private_events.recv())
            .await
            .is_err()
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/{base_symbol}/purchase"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"pair_id":{pair_id},"price":"2.000000000000000000","quantity":"10.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
                )))
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
    let purchase: Value = serde_json::from_slice(&body)?;
    assert_eq!(purchase["idempotency_key"], idempotency_key);
    assert_eq!(purchase["status"], "locked");
    let lock_position_id = purchase["lock_position_id"].as_u64().unwrap();
    let event_message = timeout(Duration::from_millis(100), private_events.recv()).await??;
    let event: Value = serde_json::from_str(event_message.payload())?;
    assert_eq!(event["type"], "new_coin.purchase.created");
    assert_eq!(event["idempotency_key"], idempotency_key);
    assert_eq!(event["project_id"], project_id);
    assert_eq!(event["pair_id"], pair_id);
    assert_eq!(event["asset_id"], base_asset);
    assert_eq!(event["quote_asset_id"], quote_asset);
    assert_eq!(event["price"], "2.000000000000000000");
    assert_eq!(event["quantity"], "10.000000000000000000");
    assert_eq!(event["quote_amount"], "20.000000000000000000");
    assert_eq!(event["status"], "locked");
    assert_eq!(event["lock_position_id"], lock_position_id);

    let (quote_available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        quote_available.normalized(),
        decimal("80.000000000000000000").normalized()
    );

    let (base_locked,): (BigDecimal,) =
        sqlx::query_as("SELECT locked FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(base_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        base_locked.normalized(),
        decimal("10.000000000000000000").normalized()
    );

    let (remaining,): (BigDecimal,) =
        sqlx::query_as("SELECT remaining_amount FROM asset_lock_positions WHERE id = ?")
            .bind(lock_position_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        remaining.normalized(),
        decimal("10.000000000000000000").normalized()
    );

    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'new_coin_purchase' AND ref_id = ?",
    )
    .bind(&idempotency_key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 2);

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
    assert_eq!(reserved_supply, decimal("0"));
    assert_eq!(allocated_supply.normalized(), decimal("10").normalized());
    assert_eq!(
        remaining_supply.normalized(),
        decimal("999990").normalized()
    );

    let (unlock_count, fee_status): (i64, String) = sqlx::query_as(
        r#"SELECT COUNT(*), MIN(fee_paid_status)
           FROM asset_unlock_records
           WHERE user_id = ? AND asset_id = ? AND lock_position_id = ? AND status = 'pending'"#,
    )
    .bind(user_id)
    .bind(base_asset)
    .bind(lock_position_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(unlock_count, 1);
    assert_eq!(fee_status, "pending");

    // 历史 NULL 指纹、项目停用和交易对下架都不得破坏成功订单的同参重放。
    sqlx::query(
        "UPDATE new_coin_purchase_orders SET request_fingerprint = NULL WHERE idempotency_key = ?",
    )
    .bind(&idempotency_key)
    .execute(&pool)
    .await?;
    sqlx::query("UPDATE new_coin_projects SET status = 'disabled' WHERE id = ?")
        .bind(project_id)
        .execute(&pool)
        .await?;
    sqlx::query("UPDATE trading_pairs SET status = 'disabled' WHERE id = ?")
        .bind(pair_id)
        .execute(&pool)
        .await?;

    let duplicate_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/{base_symbol}/purchase"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"pair_id":{pair_id},"price":"2.000000000000000000","quantity":"10.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let duplicate_status = duplicate_response.status();
    let duplicate_body = axum::body::to_bytes(duplicate_response.into_body(), 8192).await?;
    assert_eq!(
        duplicate_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&duplicate_body)
    );
    let (quote_after_duplicate,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(quote_asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(
        quote_after_duplicate.normalized(),
        quote_available.normalized()
    );
    let (ledger_count_after_duplicate,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'new_coin_purchase' AND ref_id = ?",
    )
    .bind(&idempotency_key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count_after_duplicate, 2);
    assert!(
        timeout(Duration::from_millis(25), private_events.recv())
            .await
            .is_err()
    );

    let conflict_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/{base_symbol}/purchase"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"pair_id":{pair_id},"price":"2.000000000000000000","quantity":"11.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
                )))
                .unwrap(),
        )
        .await?;
    assert_eq!(conflict_response.status(), StatusCode::CONFLICT);

    cleanup_order_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        project_id,
        Some(pair_id),
        &idempotency_key,
        "new_coin_purchase",
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn new_coin_purchase_requires_enabled_post_listing_pair() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (base_asset, base_symbol) = create_asset(&pool, "NE").await;
    let (quote_asset, quote_symbol) = create_asset(&pool, "NU").await;
    let project_id =
        create_new_coin_project(&pool, base_asset, &base_symbol, Some(quote_asset)).await;
    let pair_id = create_pair(&pool, base_asset, quote_asset, &base_symbol, &quote_symbol).await;
    sqlx::query("UPDATE new_coin_projects SET issue_price = 2 WHERE id = ?")
        .bind(project_id)
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(quote_asset)
        .bind(decimal("100.000000000000000000"))
        .execute(&pool)
        .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = user_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let disabled_key = format!("new-pur-disabled-{}", Uuid::now_v7().simple());
    let disabled_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/{base_symbol}/purchase"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"pair_id":{pair_id},"price":"2.000000000000000000","quantity":"10.000000000000000000","idempotency_key":"{disabled_key}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let disabled_status = disabled_response.status();
    let disabled_body = axum::body::to_bytes(disabled_response.into_body(), 8192).await?;
    assert_eq!(
        disabled_status,
        StatusCode::BAD_REQUEST,
        "payload: {}",
        String::from_utf8_lossy(&disabled_body)
    );
    let disabled_payload: Value = serde_json::from_slice(&disabled_body)?;
    assert_eq!(disabled_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        disabled_payload["message"],
        "validation error: post-listing new coin purchase is not open for this project"
    );

    sqlx::query(
        "UPDATE new_coin_projects SET post_listing_purchase_enabled = TRUE, post_listing_pair_id = ? WHERE id = ?",
    )
    .bind(pair_id)
    .bind(project_id)
    .execute(&pool)
    .await?;

    let mismatched_pair_id = u64::MAX;
    let mismatched_key = format!("new-pur-mismatch-{}", Uuid::now_v7().simple());
    let mismatched_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/{base_symbol}/purchase"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"pair_id":{mismatched_pair_id},"price":"2.000000000000000000","quantity":"10.000000000000000000","idempotency_key":"{mismatched_key}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let mismatched_status = mismatched_response.status();
    let mismatched_body = axum::body::to_bytes(mismatched_response.into_body(), 8192).await?;
    assert_eq!(
        mismatched_status,
        StatusCode::BAD_REQUEST,
        "payload: {}",
        String::from_utf8_lossy(&mismatched_body)
    );

    cleanup_order_fixture(
        &pool,
        user_id,
        base_asset,
        quote_asset,
        project_id,
        Some(pair_id),
        &mismatched_key,
        "new_coin_purchase",
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn new_coin_routes_release_due_paid_unlock_updates_wallet_and_lock_state()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let user_id = create_user(&pool).await;
    let (asset_id, symbol) = create_asset(&pool, "NR").await;
    let project_id = create_new_coin_project(&pool, asset_id, &symbol, None).await;
    let unlock_key = seed_unlock_record(&pool, user_id, asset_id, 0).await;
    sqlx::query(
        "INSERT INTO wallet_accounts (user_id, asset_id, available, locked) VALUES (?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(decimal("2.000000000000000000"))
    .bind(decimal("10.000000000000000000"))
    .execute(&pool)
    .await?;
    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let hub = EventBroadcastHub::new(16);
    let mut private_events = hub.subscribe(&WebSocketChannel::private_user(user_id));
    let _keepalive_hub = hub.clone();
    let app = user_routes().with_state(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_event_broadcast_hub(hub),
    );

    let pay_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/new-coins/unlocks/{unlock_key}/pay-fee"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"payment_asset_id":{asset_id},"amount":"2.000000000000000000"}}"#
                )))
                .unwrap(),
        )
        .await?;
    assert_eq!(pay_response.status(), StatusCode::OK);

    let release_request = || {
        Request::builder()
            .method("POST")
            .uri(format!("/new-coins/unlocks/{unlock_key}/release"))
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap()
    };
    let (first_release, second_release) = tokio::join!(
        app.clone().oneshot(release_request()),
        app.clone().oneshot(release_request())
    );
    for release_response in [first_release?, second_release?] {
        assert_eq!(release_response.status(), StatusCode::OK);
        let release_body = axum::body::to_bytes(release_response.into_body(), 8192).await?;
        let released: Value = serde_json::from_slice(&release_body)?;
        assert_eq!(released["released"], true);
    }

    let event_message = timeout(Duration::from_millis(100), private_events.recv()).await??;
    let event: Value = serde_json::from_str(event_message.payload())?;
    assert_eq!(event["type"], "new_coin.unlock.released");
    assert_eq!(event["unlock_idempotency_key"], unlock_key);
    assert_eq!(event["asset_id"], asset_id);
    assert_eq!(event["unlock_quantity"], "10.000000000000000000");
    assert_eq!(event["released"], true);
    assert!(
        timeout(Duration::from_millis(25), private_events.recv())
            .await
            .is_err(),
        "concurrent new coin unlock replay must not publish duplicate private event"
    );

    let replay_response = app.oneshot(release_request()).await?;
    assert_eq!(replay_response.status(), StatusCode::OK);
    let replay_body = axum::body::to_bytes(replay_response.into_body(), 8192).await?;
    let replayed: Value = serde_json::from_slice(&replay_body)?;
    assert_eq!(replayed["released"], true);
    assert!(
        timeout(Duration::from_millis(25), private_events.recv())
            .await
            .is_err(),
        "idempotent new coin unlock replay must not publish a private event"
    );

    let (available, locked): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, locked FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(available, decimal("10.000000000000000000"));
    assert_eq!(locked, decimal("0.000000000000000000"));

    let (remaining, lock_status): (BigDecimal, String) = sqlx::query_as(
        r#"SELECT remaining_amount, status
           FROM asset_lock_positions
           WHERE user_id = ? AND asset_id = ?"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(remaining, decimal("0.000000000000000000"));
    assert_eq!(lock_status, "released");

    let (unlock_status,): (String,) =
        sqlx::query_as("SELECT status FROM asset_unlock_records WHERE idempotency_key = ?")
            .bind(&unlock_key)
            .fetch_one(&pool)
            .await?;
    assert_eq!(unlock_status, "released");

    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'new_coin_unlock' AND ref_id = ?",
    )
    .bind(&unlock_key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 3);

    cleanup_fixture(&pool, user_id, asset_id, project_id, &unlock_key).await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn cleanup_order_fixture(
    pool: &MySqlPool,
    user_id: u64,
    base_asset: u64,
    quote_asset: u64,
    project_id: u64,
    pair_id: Option<u64>,
    idempotency_key: &str,
    ref_type: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM wallet_ledger WHERE ref_type = ? AND ref_id = ?")
        .bind(ref_type)
        .bind(idempotency_key)
        .execute(pool)
        .await?;
    sqlx::query(
        "DELETE FROM asset_unlock_records WHERE idempotency_key = ? OR idempotency_key = CONCAT(?, ':', ?)",
    )
        .bind(idempotency_key)
        .bind(ref_type)
        .bind(idempotency_key)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM asset_lock_position_sources WHERE source_id = ?")
        .bind(idempotency_key)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM new_coin_subscriptions WHERE idempotency_key = ?")
        .bind(idempotency_key)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM new_coin_purchase_orders WHERE idempotency_key = ?")
        .bind(idempotency_key)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM asset_lock_positions WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(base_asset)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ? AND asset_id IN (?, ?)")
        .bind(user_id)
        .bind(base_asset)
        .bind(quote_asset)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM new_coin_projects WHERE id = ?")
        .bind(project_id)
        .execute(pool)
        .await?;
    if let Some(pair_id) = pair_id {
        sqlx::query("DELETE FROM trading_pairs WHERE id = ?")
            .bind(pair_id)
            .execute(pool)
            .await?;
    }
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(base_asset)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(quote_asset)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

async fn cleanup_fixture(
    pool: &MySqlPool,
    user_id: u64,
    asset_id: u64,
    project_id: u64,
    unlock_key: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"DELETE FROM platform_financial_journal
           WHERE context = 'new_coin_unlock_fee'
             AND ref_id IN (
                 SELECT CAST(id AS CHAR) FROM asset_unlock_records WHERE idempotency_key = ?
             )"#,
    )
    .bind(unlock_key)
    .execute(pool)
    .await?;
    sqlx::query("DELETE FROM asset_unlock_records WHERE idempotency_key = ?")
        .bind(unlock_key)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM wallet_ledger WHERE ref_type = 'new_coin_unlock' AND ref_id = ?")
        .bind(unlock_key)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM asset_lock_positions WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
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
    Ok(())
}
