use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use bigdecimal::BigDecimal;
use exchange_api::{
    build_router,
    config::Settings,
    modules::{
        auth::{TokenScope, issue_token},
        prediction::routes::{admin_routes, user_routes},
    },
    state::AppState,
};
use secrecy::SecretString;
use serde_json::{Value, json};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions, types::Json as SqlxJson};
use std::{error::Error, str::FromStr};
use tower::ServiceExt;
use uuid::Uuid;

mod support;

static PREDICTION_CONFIG_GOVERNANCE_LOCK: tokio::sync::Mutex<()> =
    tokio::sync::Mutex::const_new(());

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).expect("valid decimal")
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
            eprintln!("skipping prediction commission test because DATABASE_URL is not set");
            return None;
        }
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("connect test mysql");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("run migrations");
    Some(pool)
}

#[tokio::test]
async fn prediction_order_creates_precise_idempotent_agent_commission() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let suffix = Uuid::now_v7().simple().to_string();
    let user_id = sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(format!("prediction-commission-{suffix}@example.test"))
        .bind("not-a-real-hash")
        .execute(&pool)
        .await?
        .last_insert_id();
    let commission_fixture =
        support::seed_direct_agent_commission(&pool, user_id, "prediction", "0.05000000").await?;
    let asset_id = sqlx::query(
        "INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, 8, 'coin', 'active')",
    )
    .bind(format!("PC{}", &suffix[..12]))
    .bind(format!("Prediction commission {suffix}"))
    .execute(&pool)
    .await?
    .last_insert_id();
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(asset_id)
        .bind(decimal("20.00000000"))
        .execute(&pool)
        .await?;
    let market_id = sqlx::query(
        r#"INSERT INTO prediction_markets
           (external_market_id, title, tags_json, yes_price, no_price)
           VALUES (?, ?, JSON_ARRAY(), 0.50000000, 0.50000000)"#,
    )
    .bind(format!("prediction-commission-{suffix}"))
    .bind("Prediction commission market")
    .execute(&pool)
    .await?
    .last_insert_id();
    let quote_id = format!("prediction-quote-{suffix}");
    sqlx::query(
        r#"INSERT INTO prediction_quotes
           (quote_id, user_id, market_id, outcome, asset_id, stake_amount, fee_amount,
            accepted_price, shares, theoretical_payout, effective_payout_cap, expires_at)
           VALUES (?, ?, ?, 'yes', ?, ?, ?, ?, ?, ?, ?, DATE_ADD(NOW(6), INTERVAL 1 HOUR))"#,
    )
    .bind(&quote_id)
    .bind(user_id)
    .bind(market_id)
    .bind(asset_id)
    .bind(decimal("10.12345678"))
    .bind(decimal("0.10000000"))
    .bind(decimal("0.50000000"))
    .bind(decimal("20.24691356"))
    .bind(decimal("20.24691356"))
    .bind(decimal("100.00000000"))
    .execute(&pool)
    .await?;

    let token = issue_token(
        &test_settings(),
        format!("user:{user_id}"),
        TokenScope::User,
        900,
    )?;
    let app = user_routes().with_state(AppState::new(test_settings()).with_mysql(pool.clone()));
    let idempotency_key = format!("prediction-order-{suffix}");
    let request_body = serde_json::json!({
        "quote_id": quote_id,
        "idempotency_key": idempotency_key,
    })
    .to_string();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/prediction/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body.clone()))?,
        )
        .await?;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 65_536).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );
    let payload: Value = serde_json::from_slice(&body)?;
    let order_id = payload["order"]["id"]
        .as_u64()
        .expect("prediction order id");

    let replay = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/prediction/orders")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))?,
        )
        .await?;
    assert_eq!(replay.status(), StatusCode::OK);

    let records: Vec<(u64, BigDecimal, BigDecimal, BigDecimal, u64, String)> = sqlx::query_as(
        r#"SELECT agent_id, source_amount, commission_rate, commission_amount,
                  payout_asset_id, status
           FROM agent_commission_records
           WHERE user_id = ? AND source_type = 'prediction_order' AND source_id = ?"#,
    )
    .bind(user_id)
    .bind(order_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].0, commission_fixture.agent_id);
    assert_eq!(records[0].1, decimal("10.123456780000000000"));
    assert_eq!(records[0].2, decimal("0.05000000"));
    assert_eq!(records[0].3, decimal("0.50617283"));
    assert_eq!(records[0].4, asset_id);
    assert_eq!(records[0].5, "pending");

    support::cleanup_direct_agent_commission(&pool, user_id, commission_fixture).await?;
    sqlx::query("DELETE FROM wallet_ledger WHERE ref_type = 'prediction_order' AND ref_id = ?")
        .bind(order_id.to_string())
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM prediction_orders WHERE id = ?")
        .bind(order_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM prediction_quotes WHERE quote_id = ?")
        .bind(&quote_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM prediction_markets WHERE id = ?")
        .bind(market_id)
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

async fn create_prediction_admin(pool: &MySqlPool) -> (u64, u64) {
    let suffix = Uuid::now_v7().simple().to_string();
    let role_id =
        sqlx::query("INSERT INTO admin_roles (name, permissions) VALUES (?, JSON_ARRAY('*'))")
            .bind(format!("prediction-page-role-{}", &suffix[16..32]))
            .execute(pool)
            .await
            .unwrap()
            .last_insert_id();
    let admin_id =
        sqlx::query("INSERT INTO admin_users (username, password_hash, role_id) VALUES (?, ?, ?)")
            .bind(format!("prediction-page-admin-{}", &suffix[16..32]))
            .bind("not-a-real-hash")
            .bind(role_id)
            .execute(pool)
            .await
            .unwrap()
            .last_insert_id();
    (role_id, admin_id)
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct PredictionSettingsGovernanceSnapshot {
    sync_enabled: bool,
    sync_interval_seconds: u32,
    sync_tags_json: SqlxJson<Value>,
    allowed_asset_ids_json: SqlxJson<Value>,
    default_fee_rate: BigDecimal,
    default_settlement_mode: String,
    default_invalid_refund_policy: String,
    quote_ttl_seconds: u32,
    revision: u64,
}

#[derive(Debug, sqlx::FromRow)]
struct PredictionConfigAuditRow {
    action: String,
    before_json: SqlxJson<Value>,
    after_json: SqlxJson<Value>,
    reason: Option<String>,
    ip: Option<String>,
    request_id: Option<String>,
}

fn prediction_settings_write_body(
    asset_id: u64,
    revision: u64,
    reason: &str,
    sync_interval_seconds: u32,
) -> Value {
    json!({
        "sync_enabled": true,
        "sync_interval_seconds": sync_interval_seconds,
        "sync_tags": ["governance", sync_interval_seconds.to_string()],
        "allowed_asset_ids": [asset_id],
        "default_fee_rate": "0.015",
        "default_settlement_mode": "manual_confirm",
        "default_invalid_refund_policy": "refund_stake_and_fee",
        "quote_ttl_seconds": 30,
        "revision": revision,
        "reason": reason,
    })
}

async fn write_prediction_admin_config(
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
                .header("cf-connecting-ip", "203.0.113.84")
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
    let bytes = axum::body::to_bytes(response.into_body(), 1_048_576).await?;
    Ok((status, response_request_id, serde_json::from_slice(&bytes)?))
}

fn assert_prediction_audit_has_no_sensitive_keys(value: &Value) {
    match value {
        Value::Array(values) => values
            .iter()
            .for_each(assert_prediction_audit_has_no_sensitive_keys),
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
                    "sensitive key leaked into prediction audit snapshot: {key}"
                );
                assert_prediction_audit_has_no_sensitive_keys(value);
            }
        }
        _ => {}
    }
}

async fn restore_prediction_settings(
    pool: &MySqlPool,
    snapshot: &PredictionSettingsGovernanceSnapshot,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"UPDATE prediction_settings
           SET sync_enabled = ?, sync_interval_seconds = ?, sync_tags_json = ?,
               allowed_asset_ids_json = ?, default_fee_rate = ?,
               default_settlement_mode = ?, default_invalid_refund_policy = ?,
               quote_ttl_seconds = ?, revision = ?
           WHERE id = 1"#,
    )
    .bind(snapshot.sync_enabled)
    .bind(snapshot.sync_interval_seconds)
    .bind(snapshot.sync_tags_json.clone())
    .bind(snapshot.allowed_asset_ids_json.clone())
    .bind(&snapshot.default_fee_rate)
    .bind(&snapshot.default_settlement_mode)
    .bind(&snapshot.default_invalid_refund_policy)
    .bind(snapshot.quote_ttl_seconds)
    .bind(snapshot.revision)
    .execute(pool)
    .await?;
    Ok(())
}

#[tokio::test]
async fn prediction_configuration_writes_are_revision_guarded_and_audited_atomically()
-> Result<(), Box<dyn Error>> {
    let _guard = PREDICTION_CONFIG_GOVERNANCE_LOCK.lock().await;
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let original_settings = sqlx::query_as::<_, PredictionSettingsGovernanceSnapshot>(
        r#"SELECT sync_enabled, sync_interval_seconds, sync_tags_json,
                  allowed_asset_ids_json, default_fee_rate, default_settlement_mode,
                  default_invalid_refund_policy, quote_ttl_seconds, revision
           FROM prediction_settings WHERE id = 1"#,
    )
    .fetch_one(&pool)
    .await?;
    let (role_id, admin_id) = create_prediction_admin(&pool).await;
    let suffix = Uuid::now_v7().simple().to_string();
    let asset_id = sqlx::query(
        "INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, 8, 'coin', 'active')",
    )
    .bind(format!("PG{}", &suffix[..12]))
    .bind(format!("Prediction governance {suffix}"))
    .execute(&pool)
    .await?
    .last_insert_id();
    let settings = test_settings();
    let token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let next_settings_revision = original_settings
        .revision
        .checked_add(1)
        .expect("prediction settings revision test fixture must not be exhausted");
    let settings_request_a = format!("prediction-settings-a-{}", &suffix[24..]);
    let settings_request_b = format!("prediction-settings-b-{}", &suffix[24..]);
    let asset_request_a = format!("prediction-asset-a-{}", &suffix[24..]);
    let asset_request_b = format!("prediction-asset-b-{}", &suffix[24..]);
    let asset_update_request = format!("prediction-asset-update-{}", &suffix[24..]);

    let outcome: Result<(), Box<dyn Error>> = async {
        let (blank_settings_status, _, blank_settings_payload) =
            write_prediction_admin_config(
                app.clone(),
                "PATCH",
                "/admin/api/v1/prediction/settings",
                &token,
                "prediction-settings-blank-reason",
                prediction_settings_write_body(
                    asset_id,
                    original_settings.revision,
                    "   ",
                    330,
                ),
            )
            .await?;
        assert_eq!(
            blank_settings_status,
            StatusCode::BAD_REQUEST,
            "{blank_settings_payload}"
        );
        assert_eq!(blank_settings_payload["code"], "VALIDATION_ERROR");

        let (settings_update_a, settings_update_b) = tokio::join!(
            write_prediction_admin_config(
                app.clone(),
                "PATCH",
                "/admin/api/v1/prediction/settings",
                &token,
                &settings_request_a,
                prediction_settings_write_body(
                    asset_id,
                    original_settings.revision,
                    "  并发设置更新A  ",
                    331,
                ),
            ),
            write_prediction_admin_config(
                app.clone(),
                "PATCH",
                "/admin/api/v1/prediction/settings",
                &token,
                &settings_request_b,
                prediction_settings_write_body(
                    asset_id,
                    original_settings.revision,
                    "  并发设置更新B  ",
                    332,
                ),
            )
        );
        let settings_updates = [settings_update_a?, settings_update_b?];
        assert_eq!(
            settings_updates
                .iter()
                .filter(|(status, _, _)| *status == StatusCode::OK)
                .count(),
            1
        );
        assert_eq!(
            settings_updates
                .iter()
                .filter(|(status, _, _)| *status == StatusCode::CONFLICT)
                .count(),
            1
        );
        let successful_settings = settings_updates
            .iter()
            .find(|(status, _, _)| *status == StatusCode::OK)
            .expect("one settings update succeeds");
        assert_eq!(successful_settings.2["revision"], next_settings_revision);
        let rejected_settings = settings_updates
            .iter()
            .find(|(status, _, _)| *status == StatusCode::CONFLICT)
            .expect("one settings update conflicts");
        assert_eq!(rejected_settings.2["code"], "CONFLICT");

        let settings_audits = sqlx::query_as::<_, PredictionConfigAuditRow>(
            r#"SELECT action, before_json, after_json, reason, ip, request_id
               FROM admin_audit_logs
               WHERE admin_id = ? AND target_type = 'prediction_settings' AND target_id = '1'
               ORDER BY id ASC"#,
        )
        .bind(admin_id)
        .fetch_all(&pool)
        .await?;
        assert_eq!(
            settings_audits.len(),
            1,
            "blank and conflicting settings writes must not be audited"
        );
        let settings_audit = &settings_audits[0];
        assert_eq!(settings_audit.action, "prediction_settings.update");
        assert_eq!(
            settings_audit.before_json.0["revision"],
            original_settings.revision
        );
        assert_eq!(
            settings_audit.after_json.0["revision"],
            next_settings_revision
        );
        let expected_settings_request_id = match settings_audit.reason.as_deref() {
            Some("并发设置更新A") => settings_request_a.as_str(),
            Some("并发设置更新B") => settings_request_b.as_str(),
            reason => panic!("unexpected settings audit reason: {reason:?}"),
        };
        assert_eq!(
            settings_audit.request_id.as_deref(),
            Some(expected_settings_request_id)
        );
        assert_eq!(settings_audit.ip.as_deref(), Some("203.0.113.84"));

        let asset_list = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/admin/api/v1/prediction/asset-configs")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(asset_list.status(), StatusCode::OK);
        let asset_list_body = axum::body::to_bytes(asset_list.into_body(), 1_048_576).await?;
        let asset_list_payload: Value = serde_json::from_slice(&asset_list_body)?;
        let listed_asset = asset_list_payload["configs"]
            .as_array()
            .and_then(|configs| {
                configs
                    .iter()
                    .find(|config| config["asset_id"].as_u64() == Some(asset_id))
            })
            .expect("new active asset must be listed for prediction configuration");
        assert_eq!(listed_asset["revision"], 0);

        let (blank_asset_status, _, blank_asset_payload) = write_prediction_admin_config(
            app.clone(),
            "POST",
            "/admin/api/v1/prediction/asset-configs",
            &token,
            "prediction-asset-blank-reason",
            json!({
                "asset_id": asset_id,
                "enabled": true,
                "max_payout_amount": "1000",
                "revision": 0,
                "reason": "   ",
            }),
        )
        .await?;
        assert_eq!(
            blank_asset_status,
            StatusCode::BAD_REQUEST,
            "{blank_asset_payload}"
        );

        let (asset_create_a, asset_create_b) = tokio::join!(
            write_prediction_admin_config(
                app.clone(),
                "POST",
                "/admin/api/v1/prediction/asset-configs",
                &token,
                &asset_request_a,
                json!({
                    "asset_id": asset_id,
                    "enabled": true,
                    "max_payout_amount": "1200",
                    "revision": 0,
                    "reason": "  首次资产配置A  ",
                }),
            ),
            write_prediction_admin_config(
                app.clone(),
                "POST",
                "/admin/api/v1/prediction/asset-configs",
                &token,
                &asset_request_b,
                json!({
                    "asset_id": asset_id,
                    "enabled": true,
                    "max_payout_amount": "1300",
                    "revision": 0,
                    "reason": "  首次资产配置B  ",
                }),
            )
        );
        let asset_creates = [asset_create_a?, asset_create_b?];
        assert_eq!(
            asset_creates
                .iter()
                .filter(|(status, _, _)| *status == StatusCode::OK)
                .count(),
            1
        );
        assert_eq!(
            asset_creates
                .iter()
                .filter(|(status, _, _)| *status == StatusCode::CONFLICT)
                .count(),
            1
        );
        assert_eq!(
            asset_creates
                .iter()
                .find(|(status, _, _)| *status == StatusCode::OK)
                .expect("one asset create succeeds")
                .2["revision"],
            1
        );

        let asset_uri = format!("/admin/api/v1/prediction/asset-configs/{asset_id}");
        let (asset_update_status, response_request_id, asset_update_payload) =
            write_prediction_admin_config(
                app.clone(),
                "PATCH",
                &asset_uri,
                &token,
                &asset_update_request,
                json!({
                    "enabled": false,
                    "max_payout_amount": "900",
                    "revision": 1,
                    "reason": "  收紧资产配置  ",
                }),
            )
            .await?;
        assert_eq!(asset_update_status, StatusCode::OK, "{asset_update_payload}");
        assert_eq!(asset_update_payload["revision"], 2);
        assert_eq!(
            response_request_id.as_deref(),
            Some(asset_update_request.as_str())
        );

        let (stale_asset_status, _, stale_asset_payload) = write_prediction_admin_config(
            app.clone(),
            "PATCH",
            &asset_uri,
            &token,
            "prediction-asset-stale",
            json!({
                "enabled": true,
                "max_payout_amount": "9999",
                "revision": 1,
                "reason": "过期资产版本",
            }),
        )
        .await?;
        assert_eq!(
            stale_asset_status,
            StatusCode::CONFLICT,
            "{stale_asset_payload}"
        );
        assert_eq!(stale_asset_payload["code"], "CONFLICT");

        let stored_asset: (bool, BigDecimal, u64) = sqlx::query_as(
            "SELECT enabled, max_payout_amount, revision FROM prediction_asset_configs WHERE asset_id = ?",
        )
        .bind(asset_id)
        .fetch_one(&pool)
        .await?;
        assert!(!stored_asset.0);
        assert_eq!(stored_asset.1, decimal("900"));
        assert_eq!(stored_asset.2, 2);

        let asset_audits = sqlx::query_as::<_, PredictionConfigAuditRow>(
            r#"SELECT action, before_json, after_json, reason, ip, request_id
               FROM admin_audit_logs
               WHERE admin_id = ? AND target_type = 'prediction_asset_config' AND target_id = ?
               ORDER BY id ASC"#,
        )
        .bind(admin_id)
        .bind(asset_id.to_string())
        .fetch_all(&pool)
        .await?;
        assert_eq!(
            asset_audits.len(),
            2,
            "blank, concurrent loser and stale writes must not be audited"
        );
        assert_eq!(asset_audits[0].action, "prediction_asset_config.create");
        assert_eq!(asset_audits[0].before_json.0["revision"], 0);
        assert_eq!(asset_audits[0].after_json.0["revision"], 1);
        let expected_asset_request_id = match asset_audits[0].reason.as_deref() {
            Some("首次资产配置A") => asset_request_a.as_str(),
            Some("首次资产配置B") => asset_request_b.as_str(),
            reason => panic!("unexpected asset audit reason: {reason:?}"),
        };
        assert_eq!(
            asset_audits[0].request_id.as_deref(),
            Some(expected_asset_request_id)
        );
        assert_eq!(asset_audits[1].action, "prediction_asset_config.update");
        assert_eq!(asset_audits[1].before_json.0["revision"], 1);
        assert_eq!(asset_audits[1].after_json.0["revision"], 2);
        assert_eq!(asset_audits[1].reason.as_deref(), Some("收紧资产配置"));
        assert_eq!(
            asset_audits[1].request_id.as_deref(),
            Some(asset_update_request.as_str())
        );

        for audit in settings_audits.iter().chain(asset_audits.iter()) {
            assert_eq!(audit.ip.as_deref(), Some("203.0.113.84"));
            assert_prediction_audit_has_no_sensitive_keys(&audit.before_json.0);
            assert_prediction_audit_has_no_sensitive_keys(&audit.after_json.0);
        }
        Ok(())
    }
    .await;

    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM prediction_asset_configs WHERE asset_id = ?")
        .bind(asset_id)
        .execute(&pool)
        .await?;
    restore_prediction_settings(&pool, &original_settings).await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(asset_id)
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
    outcome
}

#[tokio::test]
async fn admin_prediction_orders_offset_paging_returns_disjoint_pages_and_filtered_total()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let suffix = Uuid::now_v7().simple().to_string();
    let (role_id, admin_id) = create_prediction_admin(&pool).await;
    let user_id = sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(format!("prediction-page-{suffix}@example.test"))
        .bind("not-a-real-hash")
        .execute(&pool)
        .await?
        .last_insert_id();
    let asset_id = sqlx::query(
        "INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, 8, 'coin', 'active')",
    )
    .bind(format!("PP{}", &suffix[..12]))
    .bind(format!("Prediction paging {suffix}"))
    .execute(&pool)
    .await?
    .last_insert_id();
    let market_id = sqlx::query(
        r#"INSERT INTO prediction_markets
           (external_market_id, title, tags_json, yes_price, no_price)
           VALUES (?, ?, JSON_ARRAY(), 0.50000000, 0.50000000)"#,
    )
    .bind(format!("prediction-paging-{suffix}"))
    .bind("Prediction paging market")
    .execute(&pool)
    .await?
    .last_insert_id();

    let mut open_order_ids = Vec::new();
    for (index, status) in ["open", "open", "open", "settled"].into_iter().enumerate() {
        let id = sqlx::query(
            r#"INSERT INTO prediction_orders
               (order_no, user_id, market_id, quote_id, idempotency_key, outcome, asset_id,
                stake_amount, fee_amount, accepted_price, shares, theoretical_payout,
                effective_payout_cap, status)
               VALUES (?, ?, ?, ?, ?, 'yes', ?, 1, 0, 0.50000000, 2, 2, 100, ?)"#,
        )
        .bind(format!("PM{suffix}{index}"))
        .bind(user_id)
        .bind(market_id)
        .bind(format!("prediction-paging-quote-{suffix}-{index}"))
        .bind(format!("prediction-paging-key-{suffix}-{index}"))
        .bind(asset_id)
        .bind(status)
        .execute(&pool)
        .await?
        .last_insert_id();
        if status == "open" {
            open_order_ids.push(id);
        }
    }

    let token = issue_token(
        &test_settings(),
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )?;
    let app = admin_routes().with_state(AppState::new(test_settings()).with_mysql(pool.clone()));

    let mut page_ids = Vec::new();
    for offset in [0, 2] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/prediction/orders?market_id={market_id}&status=open&limit=2&offset={offset}"
                    ))
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())?,
            )
            .await?;
        let status_code = response.status();
        let body = axum::body::to_bytes(response.into_body(), 65_536).await?;
        let payload: Value = serde_json::from_slice(&body)?;
        assert_eq!(status_code, StatusCode::OK, "payload: {payload}");
        // 总数必须反映筛选条件本身，而不是当前页行数。
        assert_eq!(payload["total"], 3, "payload: {payload}");
        page_ids.extend(
            payload["orders"]
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
    open_order_ids.sort_unstable();
    assert_eq!(unique_ids, open_order_ids);

    let all_statuses = app
        .oneshot(
            Request::builder()
                .uri(format!("/prediction/orders?market_id={market_id}&limit=1"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    let all_statuses_code = all_statuses.status();
    let all_statuses_body = axum::body::to_bytes(all_statuses.into_body(), 65_536).await?;
    let all_statuses_payload: Value = serde_json::from_slice(&all_statuses_body)?;
    assert_eq!(
        all_statuses_code,
        StatusCode::OK,
        "payload: {all_statuses_payload}"
    );
    assert_eq!(all_statuses_payload["orders"].as_array().unwrap().len(), 1);
    assert_eq!(all_statuses_payload["total"], 4);

    sqlx::query("DELETE FROM prediction_orders WHERE market_id = ?")
        .bind(market_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM prediction_markets WHERE id = ?")
        .bind(market_id)
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
