use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use bigdecimal::BigDecimal;
use chrono::Utc;
use exchange_api::{
    config::Settings,
    modules::{
        auth::{TokenScope, issue_token},
        earn::routes::{admin_routes, user_routes},
        events::{EventBroadcastHub, WebSocketChannel},
    },
    state::AppState,
};
use secrecy::SecretString;
use serde_json::{Value, json};
use sqlx::{MySql, MySqlPool, Transaction, mysql::MySqlPoolOptions};
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
            eprintln!("skipping MySQL earn route test because DATABASE_URL is not set");
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

async fn create_user(tx: &mut Transaction<'_, MySql>) -> u64 {
    let email = format!("earn-route-{}@example.test", Uuid::now_v7().simple());
    create_user_with_email(tx, email).await
}

async fn create_user_with_email(tx: &mut Transaction<'_, MySql>, email: String) -> u64 {
    sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(email)
        .bind("not-a-real-hash")
        .execute(&mut **tx)
        .await
        .unwrap()
        .last_insert_id()
}

async fn create_admin(pool: &MySqlPool) -> u64 {
    let suffix = Uuid::now_v7().simple().to_string();
    let role_id = sqlx::query(
        "INSERT INTO admin_roles (name, permissions) VALUES (?, JSON_ARRAY('earn:read'))",
    )
    .bind(format!("earn-role-{}", &suffix[16..32]))
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id();
    sqlx::query("INSERT INTO admin_users (username, password_hash, role_id) VALUES (?, ?, ?)")
        .bind(format!("earn-admin-{}", &suffix[16..32]))
        .bind("not-a-real-hash")
        .bind(role_id)
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id()
}

async fn create_asset(tx: &mut Transaction<'_, MySql>, prefix: &str) -> (u64, String) {
    let suffix = Uuid::now_v7().simple().to_string();
    let symbol = format!("{prefix}{}", &suffix[16..32]);
    let id = sqlx::query(
        "INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, 18, 'coin', 'active')",
    )
    .bind(&symbol)
    .bind(&symbol)
    .execute(&mut **tx)
    .await
    .unwrap()
    .last_insert_id();
    (id, symbol)
}

fn default_introduction_json(name: &str) -> Value {
    json!({
        "version": 1,
        "default_locale": "zh-CN",
        "items": [
            {
                "locale": "zh-CN",
                "country": "CN",
                "title": name,
                "content": [
                    { "type": "p", "children": [{ "text": name }] }
                ]
            }
        ]
    })
}

async fn seed_earn_product(tx: &mut Transaction<'_, MySql>, asset_id: u64) -> u64 {
    let suffix = Uuid::now_v7().simple().to_string();
    let name = format!("Earn {}", &suffix[16..32]);
    sqlx::query(
        r#"INSERT INTO earn_products
           (asset_id, name, category, introduction_json, term_days, apr_rate, min_subscribe, max_subscribe, status)
           VALUES (?, ?, 'fixed_term', ?, 30, ?, ?, ?, 'active')"#,
    )
    .bind(asset_id)
    .bind(&name)
    .bind(sqlx::types::Json(default_introduction_json(&name)))
    .bind(decimal("0.12000000"))
    .bind(decimal("10.000000000000000000"))
    .bind(decimal("1000.000000000000000000"))
    .execute(&mut **tx)
    .await
    .unwrap()
    .last_insert_id()
}

async fn body_json(response: axum::response::Response) -> Result<Value, Box<dyn Error>> {
    let body = axum::body::to_bytes(response.into_body(), 65_536).await?;
    Ok(serde_json::from_slice(&body)?)
}

#[tokio::test]
async fn earn_routes_require_expected_scope() {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:42", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:7", TokenScope::Admin, 900).unwrap();

    let unauthenticated_subscribe = user_routes()
        .with_state(AppState::new(settings.clone()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/subscriptions")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"product_id":1,"amount":"20.000000000000000000","idempotency_key":"scope-test"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthenticated_subscribe.status(), StatusCode::UNAUTHORIZED);

    let user_on_admin = admin_routes()
        .with_state(AppState::new(settings.clone()))
        .oneshot(
            Request::builder()
                .uri("/earn/products")
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(user_on_admin.status(), StatusCode::FORBIDDEN);

    let admin_on_user_subscribe = user_routes()
        .with_state(AppState::new(settings.clone()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/subscriptions")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"product_id":1,"amount":"20.000000000000000000","idempotency_key":"scope-test"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(admin_on_user_subscribe.status(), StatusCode::FORBIDDEN);

    let admin_on_user_redeem = user_routes()
        .with_state(AppState::new(settings.clone()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/subscriptions/1/redeem")
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(admin_on_user_redeem.status(), StatusCode::FORBIDDEN);

    let user_on_admin_create = admin_routes()
        .with_state(AppState::new(settings))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/products")
                .header("authorization", format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"asset_id":1,"name":"Earn","term_days":30,"apr_rate":"0.12000000","min_subscribe":"10.000000000000000000"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(user_on_admin_create.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn earn_routes_return_clear_error_without_mysql() {
    let settings = test_settings();
    let token = issue_token(&settings, "user:42", TokenScope::User, 900).unwrap();
    let response = user_routes()
        .with_state(AppState::new(settings))
        .oneshot(
            Request::builder()
                .uri("/earn/products")
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
        "internal error: mysql pool is not configured for earn routes"
    );
}

#[tokio::test]
async fn admin_earn_product_routes_require_admin_scope_mysql_and_validation()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = admin_routes().with_state(AppState::new(settings));

    let invalid = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"asset_id":1,"name":" ","term_days":30,"apr_rate":"0.12000000","min_subscribe":"10.000000000000000000"}"#,
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let invalid_payload = body_json(invalid).await?;
    assert_eq!(invalid_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        invalid_payload["message"],
        "validation error: earn product name is required"
    );

    let missing_mysql = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"asset_id":1,"name":"Earn 30D","term_days":30,"apr_rate":"0.12000000","min_subscribe":"10.000000000000000000","reason":"create earn product"}"#,
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(missing_mysql.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let missing_payload = body_json(missing_mysql).await?;
    assert_eq!(missing_payload["code"], "INTERNAL_ERROR");
    assert_eq!(
        missing_payload["message"],
        "internal error: mysql pool is not configured for earn routes"
    );

    let invalid_status = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/earn/products/1/status")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"status":"archived"}"#))
                .unwrap(),
        )
        .await?;
    assert_eq!(invalid_status.status(), StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn admin_earn_detail_routes_require_admin_scope_mysql_and_reason()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = admin_routes().with_state(AppState::new(settings));

    let product_missing = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/earn/products/1")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(product_missing.status(), StatusCode::UNAUTHORIZED);

    let product_user = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/earn/products/1")
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(product_user.status(), StatusCode::FORBIDDEN);

    let product_admin = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/earn/products/1")
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(product_admin.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let subscription_admin = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/earn/subscriptions/1")
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(
        subscription_admin.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );

    let blank_create_reason = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"asset_id":1,"name":"Earn 30D","term_days":30,"apr_rate":"0.12000000","min_subscribe":"10.000000000000000000","reason":"   "}"#,
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(blank_create_reason.status(), StatusCode::BAD_REQUEST);
    let blank_create_payload = body_json(blank_create_reason).await?;
    assert_eq!(
        blank_create_payload["message"],
        "validation error: earn product reason is required"
    );

    let blank_status_reason = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/earn/products/1/status")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"status":"disabled","reason":"   "}"#))
                .unwrap(),
        )
        .await?;
    assert_eq!(blank_status_reason.status(), StatusCode::BAD_REQUEST);
    let blank_status_payload = body_json(blank_status_reason).await?;
    assert_eq!(
        blank_status_payload["message"],
        "validation error: earn product reason is required"
    );

    Ok(())
}

#[tokio::test]
async fn admin_earn_product_rejects_unsafe_term_name_and_apr_before_mysql()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = admin_routes().with_state(AppState::new(settings));

    let unsafe_term = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"asset_id":1,"name":"Unsafe Term","term_days":4294967295,"apr_rate":"0.12000000","min_subscribe":"10.000000000000000000"}"#,
                ))
                .unwrap(),
        )
        .await?;
    let unsafe_term_status = unsafe_term.status();
    let unsafe_term_payload = body_json(unsafe_term).await?;
    assert_eq!(unsafe_term_status, StatusCode::BAD_REQUEST);
    assert_eq!(unsafe_term_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        unsafe_term_payload["message"],
        "validation error: earn product term_days exceeds supported maximum"
    );

    let long_name = "A".repeat(129);
    let long_name_body = format!(
        r#"{{"asset_id":1,"name":"{long_name}","term_days":30,"apr_rate":"0.12000000","min_subscribe":"10.000000000000000000"}}"#
    );
    let long_name_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(long_name_body))
                .unwrap(),
        )
        .await?;
    let long_name_status = long_name_response.status();
    let long_name_payload = body_json(long_name_response).await?;
    assert_eq!(long_name_status, StatusCode::BAD_REQUEST);
    assert_eq!(long_name_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        long_name_payload["message"],
        "validation error: earn product name is too long"
    );

    let overflow_apr = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"asset_id":1,"name":"Overflow APR","term_days":30,"apr_rate":"10000000000.00000000","min_subscribe":"10.000000000000000000"}"#,
                ))
                .unwrap(),
        )
        .await?;
    let overflow_apr_status = overflow_apr.status();
    let overflow_apr_payload = body_json(overflow_apr).await?;
    assert_eq!(overflow_apr_status, StatusCode::BAD_REQUEST);
    assert_eq!(overflow_apr_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        overflow_apr_payload["message"],
        "validation error: earn product apr_rate exceeds decimal storage precision"
    );

    let scale_apr = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"asset_id":1,"name":"Scale APR","term_days":30,"apr_rate":"0.123456789","min_subscribe":"10.000000000000000000"}"#,
                ))
                .unwrap(),
        )
        .await?;
    let scale_apr_status = scale_apr.status();
    let scale_apr_payload = body_json(scale_apr).await?;
    assert_eq!(scale_apr_status, StatusCode::BAD_REQUEST);
    assert_eq!(scale_apr_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        scale_apr_payload["message"],
        "validation error: earn product apr_rate supports at most 8 decimal places"
    );

    let long_reason = "R".repeat(513);
    let long_reason_body = format!(
        r#"{{"asset_id":1,"name":"Long Reason","term_days":30,"apr_rate":"0.12000000","min_subscribe":"10.000000000000000000","reason":"{long_reason}"}}"#
    );
    let invalid_category_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"asset_id":1,"name":"Invalid Category","category":"fixed term","term_days":30,"apr_rate":"0.12000000","min_subscribe":"10.000000000000000000"}"#,
                ))
                .unwrap(),
        )
        .await?;
    let invalid_category_status = invalid_category_response.status();
    let invalid_category_payload = body_json(invalid_category_response).await?;
    assert_eq!(invalid_category_status, StatusCode::BAD_REQUEST);
    assert_eq!(invalid_category_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        invalid_category_payload["message"],
        "validation error: earn product category supports only letters, numbers, underscore, and hyphen"
    );

    let invalid_intro_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"asset_id":1,"name":"Invalid Intro","category":"fixed_term","introduction_json":{"version":1,"default_locale":"en-US","items":[{"locale":"zh-CN","country":"CN","title":"介绍","content":[{"type":"p","children":[{"text":"内容"}]}]}]},"term_days":30,"apr_rate":"0.12000000","min_subscribe":"10.000000000000000000"}"#,
                ))
                .unwrap(),
        )
        .await?;
    let invalid_intro_status = invalid_intro_response.status();
    let invalid_intro_payload = body_json(invalid_intro_response).await?;
    assert_eq!(invalid_intro_status, StatusCode::BAD_REQUEST);
    assert_eq!(invalid_intro_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        invalid_intro_payload["message"],
        "validation error: earn product introduction default_locale must exist in items"
    );

    let invalid_plate_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"asset_id":1,"name":"Invalid Plate","category":"fixed_term","introduction_json":{"version":1,"default_locale":"zh-CN","items":[{"locale":"zh-CN","country":"CN","title":"介绍","content":[123,{"unexpected":true}]}]},"term_days":30,"apr_rate":"0.12000000","min_subscribe":"10.000000000000000000","reason":"invalid plate"}"#,
                ))
                .unwrap(),
        )
        .await?;
    let invalid_plate_status = invalid_plate_response.status();
    let invalid_plate_payload = body_json(invalid_plate_response).await?;
    assert_eq!(invalid_plate_status, StatusCode::BAD_REQUEST);
    assert_eq!(invalid_plate_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        invalid_plate_payload["message"],
        "validation error: earn product introduction content node is invalid"
    );

    let unexpected_plate_field_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"asset_id":1,"name":"Plate Extra Field","category":"fixed_term","introduction_json":{"version":1,"default_locale":"zh-CN","items":[{"locale":"zh-CN","country":"CN","title":"介绍","content":[{"type":"p","children":[{"text":"内容","html":"raw html","children":[123]}]}]}]},"term_days":30,"apr_rate":"0.12000000","min_subscribe":"10.000000000000000000","reason":"unexpected plate field"}"#,
                ))
                .unwrap(),
        )
        .await?;
    let unexpected_plate_field_status = unexpected_plate_field_response.status();
    let unexpected_plate_field_payload = body_json(unexpected_plate_field_response).await?;
    assert_eq!(unexpected_plate_field_status, StatusCode::BAD_REQUEST);
    assert_eq!(unexpected_plate_field_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        unexpected_plate_field_payload["message"],
        "validation error: earn product introduction content node is invalid"
    );

    let long_reason_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(long_reason_body))
                .unwrap(),
        )
        .await?;
    let long_reason_status = long_reason_response.status();
    let long_reason_payload = body_json(long_reason_response).await?;
    assert_eq!(long_reason_status, StatusCode::BAD_REQUEST);
    assert_eq!(long_reason_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        long_reason_payload["message"],
        "validation error: earn product reason is too long"
    );

    Ok(())
}

#[tokio::test]
async fn admin_disabling_earn_product_blocks_concurrent_subscription_after_commit()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let (asset_id, _asset_symbol) = create_asset(&mut fixture_tx, "DA").await;
    let product_id = seed_earn_product(&mut fixture_tx, asset_id).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(asset_id)
        .bind(decimal("100.000000000000000000"))
        .execute(&mut *fixture_tx)
        .await?;
    fixture_tx.commit().await?;

    let mut disable_tx = pool.begin().await?;
    sqlx::query("SELECT id FROM earn_products WHERE id = ? FOR UPDATE")
        .bind(product_id)
        .execute(&mut *disable_tx)
        .await?;
    sqlx::query("UPDATE earn_products SET status = 'disabled' WHERE id = ?")
        .bind(product_id)
        .execute(&mut *disable_tx)
        .await?;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = user_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let idempotency_key = format!("earn-disable-race-{}", Uuid::now_v7().simple());
    let request_body = format!(
        r#"{{"product_id":{product_id},"amount":"20.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );
    let mut subscribe_task = tokio::spawn(async move {
        app.oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/subscriptions")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
    });

    assert!(
        timeout(Duration::from_millis(200), &mut subscribe_task)
            .await
            .is_err()
    );
    disable_tx.commit().await?;
    let response = subscribe_task.await??;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 65_536).await?;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );

    let (subscription_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM earn_subscriptions WHERE user_id = ? AND product_id = ?",
    )
    .bind(user_id)
    .bind(product_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(subscription_count, 0);

    let (available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(asset_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(available, decimal("100.000000000000000000"));

    Ok(())
}

#[tokio::test]
async fn earn_subscribe_replays_existing_key_after_concurrent_disable() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let (asset_id, _asset_symbol) = create_asset(&mut fixture_tx, "DR").await;
    let product_id = seed_earn_product(&mut fixture_tx, asset_id).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(asset_id)
        .bind(decimal("100.000000000000000000"))
        .execute(&mut *fixture_tx)
        .await?;
    fixture_tx.commit().await?;

    let idempotency_key = format!("earn-disable-replay-{}", Uuid::now_v7().simple());
    let amount = decimal("20.000000000000000000");
    let mut first_tx = pool.begin().await?;
    sqlx::query("SELECT id FROM earn_products WHERE id = ? FOR UPDATE")
        .bind(product_id)
        .execute(&mut *first_tx)
        .await?;
    let subscription_id = sqlx::query(
        r#"INSERT INTO earn_subscriptions
           (user_id, product_id, asset_id, amount, apr_rate, term_days, status, idempotency_key, matures_at)
           VALUES (?, ?, ?, ?, ?, 30, 'subscribed', ?, ?)"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(asset_id)
    .bind(&amount)
    .bind(decimal("0.12000000"))
    .bind(&idempotency_key)
    .bind(Utc::now() + chrono::TimeDelta::days(30))
    .execute(&mut *first_tx)
    .await?
    .last_insert_id();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(decimal("80.000000000000000000"))
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut *first_tx)
        .await?;
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, 'earn_subscribe', ?, 'available', ?, ?, 0, 0, 'earn_subscription', ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(-amount.clone())
    .bind(decimal("80.000000000000000000"))
    .bind(decimal("80.000000000000000000"))
    .bind(subscription_id.to_string())
    .execute(&mut *first_tx)
    .await?;

    let disable_pool = pool.clone();
    let disable_task = tokio::spawn(async move {
        let mut tx = disable_pool.begin().await?;
        sqlx::query("SELECT id FROM earn_products WHERE id = ? FOR UPDATE")
            .bind(product_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE earn_products SET status = 'disabled' WHERE id = ?")
            .bind(product_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await
    });
    assert!(
        timeout(Duration::from_millis(200), disable_task)
            .await
            .is_err()
    );

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = user_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let request_body = format!(
        r#"{{"product_id":{product_id},"amount":"30.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );
    let mut replay_task = tokio::spawn(async move {
        app.oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/subscriptions")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
    });
    assert!(
        timeout(Duration::from_millis(200), &mut replay_task)
            .await
            .is_err()
    );

    first_tx.commit().await?;
    let response = replay_task.await??;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 65_536).await?;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );
    let payload: Value = serde_json::from_slice(&body)?;
    assert_eq!(payload["code"], "CONFLICT");
    assert_eq!(
        payload["message"],
        "conflict: earn idempotency key belongs to a different request"
    );

    let (subscription_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM earn_subscriptions WHERE user_id = ? AND product_id = ?",
    )
    .bind(user_id)
    .bind(product_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(subscription_count, 1);

    Ok(())
}

#[tokio::test]
async fn admin_earn_product_create_update_status_and_audit() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let admin_id = create_admin(&pool).await;
    let (asset_id, asset_symbol) = create_asset(&mut fixture_tx, "AP").await;
    fixture_tx.commit().await?;

    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let create_body = format!(
        r#"{{"asset_id":{asset_id},"name":"Admin Earn {asset_symbol}","category":"structured","introduction_json":{{"version":1,"default_locale":"zh-CN","items":[{{"locale":"zh-CN","country":"CN","title":"USDT 稳健理财","content":[{{"type":"h3","children":[{{"text":"USDT 稳健理财"}}]}},{{"type":"p","children":[{{"text":"适合稳健型用户。"}}]}}]}},{{"locale":"en-US","country":"US","title":"USDT Earn","content":[{{"type":"p","children":[{{"text":"For stable users."}}]}}]}}]}} ,"term_days":60,"apr_rate":"0.15000000","min_subscribe":"25.000000000000000000","max_subscribe":"2500.000000000000000000","status":"active","reason":"launch earn product"}}"#
    );

    let missing_asset_body = r#"{"asset_id":999999999999,"name":"Missing Asset Earn","term_days":30,"apr_rate":"0.12000000","min_subscribe":"10.000000000000000000","reason":"missing asset"}"#;
    let missing_asset_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(missing_asset_body))
                .unwrap(),
        )
        .await?;
    let missing_asset_status = missing_asset_response.status();
    let missing_asset_payload = body_json(missing_asset_response).await?;
    assert_eq!(missing_asset_status, StatusCode::NOT_FOUND);
    assert_eq!(missing_asset_payload["code"], "NOT_FOUND");

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(create_body))
                .unwrap(),
        )
        .await?;
    let create_status = create_response.status();
    let create_payload = body_json(create_response).await?;
    assert_eq!(create_status, StatusCode::OK, "payload: {create_payload}");
    let product_id = create_payload["id"].as_u64().unwrap();
    assert_eq!(create_payload["asset_id"], asset_id);
    assert_eq!(create_payload["asset_symbol"], asset_symbol);
    assert_eq!(create_payload["term_days"], 60);
    assert_eq!(create_payload["apr_rate"], "0.15000000");
    assert_eq!(create_payload["category"], "structured");
    assert_eq!(create_payload["status"], "active");
    assert_eq!(create_payload["introduction_json"]["version"], 1);
    assert_eq!(
        create_payload["introduction_json"]["items"][0]["content"][0]["type"],
        "h3"
    );

    let (stored_category, stored_introduction): (String, sqlx::types::Json<Value>) =
        sqlx::query_as("SELECT category, introduction_json FROM earn_products WHERE id = ?")
            .bind(product_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(stored_category, "structured");
    assert_eq!(stored_introduction["default_locale"], "zh-CN");

    let long_update_reason = "R".repeat(513);
    let long_update_reason_body =
        format!(r#"{{"status":"disabled","reason":"{long_update_reason}"}}"#);
    let long_update_reason_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/earn/products/{product_id}/status"))
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(long_update_reason_body))
                .unwrap(),
        )
        .await?;
    let long_update_reason_status = long_update_reason_response.status();
    let long_update_reason_payload = body_json(long_update_reason_response).await?;
    assert_eq!(long_update_reason_status, StatusCode::BAD_REQUEST);
    assert_eq!(long_update_reason_payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        long_update_reason_payload["message"],
        "validation error: earn product reason is too long"
    );

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/earn/products/{product_id}/status"))
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"status":"disabled","reason":"pause product"}"#,
                ))
                .unwrap(),
        )
        .await?;
    let update_status = update_response.status();
    let update_payload = body_json(update_response).await?;
    assert_eq!(update_status, StatusCode::OK, "payload: {update_payload}");
    assert_eq!(update_payload["id"], product_id);
    assert_eq!(update_payload["status"], "disabled");

    let admin_list = app
        .oneshot(
            Request::builder()
                .uri("/earn/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let admin_list_status = admin_list.status();
    let admin_list_payload = body_json(admin_list).await?;
    assert_eq!(
        admin_list_status,
        StatusCode::OK,
        "payload: {admin_list_payload}"
    );
    assert!(
        admin_list_payload["products"]
            .as_array()
            .unwrap()
            .iter()
            .any(|product| product["id"] == product_id
                && product["status"] == "disabled"
                && product["category"] == "structured"
                && product["introduction_json"]["default_locale"] == "zh-CN")
    );

    let audit_rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT action, target_type, reason FROM admin_audit_logs WHERE admin_id = ? AND target_id = ? ORDER BY id",
    )
    .bind(admin_id)
    .bind(product_id.to_string())
    .fetch_all(&pool)
    .await?;
    assert_eq!(audit_rows.len(), 2);
    assert_eq!(audit_rows[0].0, "earn_product.create");
    assert_eq!(audit_rows[0].1, "earn_product");
    assert_eq!(audit_rows[0].2, "launch earn product");
    assert_eq!(audit_rows[1].0, "earn_product.update_status");
    assert_eq!(audit_rows[1].1, "earn_product");
    assert_eq!(audit_rows[1].2, "pause product");

    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ? AND target_type = 'earn_product'")
        .bind(admin_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM earn_products WHERE id = ?")
        .bind(product_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await?;

    Ok(())
}

#[tokio::test]
async fn admin_earn_product_create_rolls_back_when_audit_fails() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let (asset_id, _asset_symbol) = create_asset(&mut fixture_tx, "AR").await;
    fixture_tx.commit().await?;

    let missing_admin_id = 999_999_999_u64;
    let admin_token = issue_token(
        &settings,
        format!("admin:{missing_admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let product_name = format!("Audit Rollback {}", Uuid::now_v7().simple());
    let create_body = format!(
        r#"{{"asset_id":{asset_id},"name":"{product_name}","term_days":30,"apr_rate":"0.12000000","min_subscribe":"10.000000000000000000","reason":"audit should fail"}}"#
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(create_body))
                .unwrap(),
        )
        .await?;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let (product_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM earn_products WHERE name = ?")
            .bind(&product_name)
            .fetch_one(&pool)
            .await?;
    assert_eq!(product_count, 0);

    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(asset_id)
        .execute(&pool)
        .await?;

    Ok(())
}

#[tokio::test]
async fn earn_lists_active_products_for_user_and_all_products_for_admin()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let admin_id = create_admin(&pool).await;
    let (asset_id, asset_symbol) = create_asset(&mut fixture_tx, "EA").await;
    let active_product_id = seed_earn_product(&mut fixture_tx, asset_id).await;
    let disabled_product_id = seed_earn_product(&mut fixture_tx, asset_id).await;
    sqlx::query("UPDATE earn_products SET status = 'disabled' WHERE id = ?")
        .bind(disabled_product_id)
        .execute(&mut *fixture_tx)
        .await?;
    fixture_tx.commit().await?;

    let user_token =
        issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let state = AppState::new(settings).with_mysql(pool.clone());

    let user_response = user_routes()
        .with_state(state.clone())
        .oneshot(
            Request::builder()
                .uri("/earn/products")
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let user_status = user_response.status();
    let user_body = axum::body::to_bytes(user_response.into_body(), 65_536).await?;
    assert_eq!(
        user_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&user_body)
    );
    let user_payload: Value = serde_json::from_slice(&user_body)?;
    assert!(
        user_payload["products"]
            .as_array()
            .unwrap()
            .iter()
            .any(|product| {
                product["id"] == active_product_id
                    && product["asset_symbol"] == asset_symbol
                    && product["category"] == "fixed_term"
                    && product["introduction_json"]["default_locale"] == "zh-CN"
            })
    );
    assert!(
        !user_payload["products"]
            .as_array()
            .unwrap()
            .iter()
            .any(|product| { product["id"] == disabled_product_id })
    );

    let admin_response = admin_routes()
        .with_state(state)
        .oneshot(
            Request::builder()
                .uri("/earn/products")
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let admin_status = admin_response.status();
    let admin_body = axum::body::to_bytes(admin_response.into_body(), 65_536).await?;
    assert_eq!(
        admin_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&admin_body)
    );
    let admin_payload: Value = serde_json::from_slice(&admin_body)?;
    assert!(
        admin_payload["products"]
            .as_array()
            .unwrap()
            .iter()
            .any(|product| { product["id"] == active_product_id })
    );
    assert!(
        admin_payload["products"]
            .as_array()
            .unwrap()
            .iter()
            .any(|product| { product["id"] == disabled_product_id })
    );

    Ok(())
}

#[tokio::test]
async fn earn_lists_current_user_subscriptions_with_timestamp() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let other_user_id = create_user(&mut fixture_tx).await;
    let (asset_id, _asset_symbol) = create_asset(&mut fixture_tx, "LA").await;
    let product_id = seed_earn_product(&mut fixture_tx, asset_id).await;

    let first_subscription_id = sqlx::query(
        r#"INSERT INTO earn_subscriptions
           (user_id, product_id, asset_id, amount, apr_rate, term_days, status,
            idempotency_key, subscribed_at, matures_at, created_at)
           VALUES (?, ?, ?, ?, ?, 30, 'subscribed', ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(asset_id)
    .bind(decimal("25.000000000000000000"))
    .bind(decimal("0.12000000"))
    .bind(format!("earn-list-first-{}", Uuid::now_v7().simple()))
    .bind("2026-05-30 04:00:00.000000")
    .bind("2026-06-29 04:00:00.000000")
    .bind("2026-05-30 04:00:00.000000")
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    let second_subscription_id = sqlx::query(
        r#"INSERT INTO earn_subscriptions
           (user_id, product_id, asset_id, amount, apr_rate, term_days, status,
            idempotency_key, subscribed_at, matures_at, created_at)
           VALUES (?, ?, ?, ?, ?, 30, 'subscribed', ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(asset_id)
    .bind(decimal("50.000000000000000000"))
    .bind(decimal("0.12000000"))
    .bind(format!("earn-list-second-{}", Uuid::now_v7().simple()))
    .bind("2026-05-30 05:00:00.000000")
    .bind("2026-06-29 05:00:00.000000")
    .bind("2026-05-30 05:00:00.000000")
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    let other_subscription_id = sqlx::query(
        r#"INSERT INTO earn_subscriptions
           (user_id, product_id, asset_id, amount, apr_rate, term_days, status,
            idempotency_key, subscribed_at, matures_at, created_at)
           VALUES (?, ?, ?, ?, ?, 30, 'subscribed', ?, ?, ?, ?)"#,
    )
    .bind(other_user_id)
    .bind(product_id)
    .bind(asset_id)
    .bind(decimal("75.000000000000000000"))
    .bind(decimal("0.12000000"))
    .bind(format!("earn-list-other-{}", Uuid::now_v7().simple()))
    .bind("2026-05-30 06:00:00.000000")
    .bind("2026-06-29 06:00:00.000000")
    .bind("2026-05-30 06:00:00.000000")
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    fixture_tx.commit().await?;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let response = user_routes()
        .with_state(AppState::new(settings).with_mysql(pool.clone()))
        .oneshot(
            Request::builder()
                .uri("/earn/subscriptions?limit=10")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
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
    let subscriptions = payload["subscriptions"].as_array().unwrap();
    let subscription_ids: Vec<u64> = subscriptions
        .iter()
        .map(|subscription| subscription["id"].as_u64().unwrap())
        .collect();
    assert_eq!(
        subscription_ids,
        vec![second_subscription_id, first_subscription_id]
    );
    assert!(!subscription_ids.contains(&other_subscription_id));
    assert!(subscriptions[0]["matures_at"].is_number());
    assert_eq!(subscriptions[0]["amount"], "50.000000000000000000");

    Ok(())
}

#[tokio::test]
async fn admin_earn_lists_subscriptions_with_filters_and_timestamp() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let admin_id = create_admin(&pool).await;
    let mut fixture_tx = pool.begin().await?;
    sqlx::query("DELETE FROM earn_subscriptions WHERE idempotency_key LIKE 'earn-admin-list-%'")
        .execute(&mut *fixture_tx)
        .await?;
    let user_email = format!("earn-admin-filter-{}@example.test", Uuid::now_v7().simple());
    let user_id = create_user_with_email(&mut fixture_tx, user_email.clone()).await;
    let other_user_id = create_user(&mut fixture_tx).await;
    let (asset_id, _asset_symbol) = create_asset(&mut fixture_tx, "AL").await;
    let product_id = seed_earn_product(&mut fixture_tx, asset_id).await;

    let user_redeemed_id = sqlx::query(
        r#"INSERT INTO earn_subscriptions
           (user_id, product_id, asset_id, amount, apr_rate, term_days, status,
            idempotency_key, subscribed_at, matures_at, created_at)
           VALUES (?, ?, ?, ?, ?, 30, 'redeemed', ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(asset_id)
    .bind(decimal("15.000000000000000000"))
    .bind(decimal("0.12000000"))
    .bind(format!(
        "earn-admin-list-redeemed-{}",
        Uuid::now_v7().simple()
    ))
    .bind("2037-05-30 04:00:00.000000")
    .bind("2037-06-29 04:00:00.000000")
    .bind("2037-05-30 04:00:00.000000")
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    let _user_subscribed_id = sqlx::query(
        r#"INSERT INTO earn_subscriptions
           (user_id, product_id, asset_id, amount, apr_rate, term_days, status,
            idempotency_key, subscribed_at, matures_at, created_at)
           VALUES (?, ?, ?, ?, ?, 30, 'subscribed', ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(asset_id)
    .bind(decimal("25.000000000000000000"))
    .bind(decimal("0.12000000"))
    .bind(format!("earn-admin-list-user-{}", Uuid::now_v7().simple()))
    .bind("2037-05-30 05:00:00.000000")
    .bind("2037-06-29 05:00:00.000000")
    .bind("2037-05-30 05:00:00.000000")
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    let other_subscribed_id = sqlx::query(
        r#"INSERT INTO earn_subscriptions
           (user_id, product_id, asset_id, amount, apr_rate, term_days, status,
            idempotency_key, subscribed_at, matures_at, created_at)
           VALUES (?, ?, ?, ?, ?, 30, 'subscribed', ?, ?, ?, ?)"#,
    )
    .bind(other_user_id)
    .bind(product_id)
    .bind(asset_id)
    .bind(decimal("50.000000000000000000"))
    .bind(decimal("0.12000000"))
    .bind(format!("earn-admin-list-other-{}", Uuid::now_v7().simple()))
    .bind("2037-05-30 06:00:00.000000")
    .bind("2037-06-29 06:00:00.000000")
    .bind("2037-05-30 06:00:00.000000")
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    let other_redeemed_id = sqlx::query(
        r#"INSERT INTO earn_subscriptions
           (user_id, product_id, asset_id, amount, apr_rate, term_days, status,
            idempotency_key, subscribed_at, matures_at, created_at)
           VALUES (?, ?, ?, ?, ?, 30, 'redeemed', ?, ?, ?, ?)"#,
    )
    .bind(other_user_id)
    .bind(product_id)
    .bind(asset_id)
    .bind(decimal("55.000000000000000000"))
    .bind(decimal("0.12000000"))
    .bind(format!(
        "earn-admin-list-other-redeemed-{}",
        Uuid::now_v7().simple()
    ))
    .bind("2037-05-30 06:30:00.000000")
    .bind("2037-06-29 06:30:00.000000")
    .bind("2037-05-30 06:30:00.000000")
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    let user_later_subscribed_id = sqlx::query(
        r#"INSERT INTO earn_subscriptions
           (user_id, product_id, asset_id, amount, apr_rate, term_days, status,
            idempotency_key, subscribed_at, matures_at, created_at)
           VALUES (?, ?, ?, ?, ?, 30, 'subscribed', ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(asset_id)
    .bind(decimal("75.000000000000000000"))
    .bind(decimal("0.12000000"))
    .bind(format!("earn-admin-list-later-{}", Uuid::now_v7().simple()))
    .bind("2037-05-30 07:00:00.000000")
    .bind("2037-06-29 07:00:00.000000")
    .bind("2037-05-30 07:00:00.000000")
    .execute(&mut *fixture_tx)
    .await?
    .last_insert_id();
    fixture_tx.commit().await?;

    let admin_token = issue_token(
        &settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )
    .unwrap();
    let user_token =
        issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = admin_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/earn/subscriptions?status=subscribed&limit=2")
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let list_status = list_response.status();
    let list_body = axum::body::to_bytes(list_response.into_body(), 65_536).await?;
    assert_eq!(
        list_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&list_body)
    );
    let list_payload: Value = serde_json::from_slice(&list_body)?;
    let subscriptions = list_payload["subscriptions"].as_array().unwrap();
    let subscription_ids: Vec<u64> = subscriptions
        .iter()
        .map(|subscription| subscription["id"].as_u64().unwrap())
        .collect();
    assert_eq!(
        subscription_ids,
        vec![user_later_subscribed_id, other_subscribed_id]
    );
    assert_eq!(subscriptions[0]["user_id"], user_id);
    assert!(subscriptions[0]["matures_at"].is_number());

    let filtered_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/earn/subscriptions?email={user_email}&status=redeemed&limit=10"
                ))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let filtered_status = filtered_response.status();
    let filtered_payload = body_json(filtered_response).await?;
    assert_eq!(
        filtered_status,
        StatusCode::OK,
        "payload: {filtered_payload}"
    );
    let filtered_subscriptions = filtered_payload["subscriptions"].as_array().unwrap();
    let filtered_ids: Vec<u64> = filtered_subscriptions
        .iter()
        .map(|subscription| subscription["id"].as_u64().unwrap())
        .collect();
    assert_eq!(filtered_ids, vec![user_redeemed_id]);
    assert!(!filtered_ids.contains(&other_redeemed_id));

    let user_scope_response = app
        .oneshot(
            Request::builder()
                .uri("/earn/subscriptions")
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(user_scope_response.status(), StatusCode::FORBIDDEN);

    Ok(())
}

#[tokio::test]
async fn earn_subscribe_debits_wallet_and_writes_ledger() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let (asset_id, _asset_symbol) = create_asset(&mut fixture_tx, "SA").await;
    let product_id = seed_earn_product(&mut fixture_tx, asset_id).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(asset_id)
        .bind(decimal("100.000000000000000000"))
        .execute(&mut *fixture_tx)
        .await?;
    fixture_tx.commit().await?;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("earn-subscribe-{}", Uuid::now_v7().simple());
    let hub = EventBroadcastHub::new(16);
    let _keepalive_hub = hub.clone();
    let mut private_events = hub.subscribe(&WebSocketChannel::private_user(user_id));
    let app = user_routes().with_state(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_event_broadcast_hub(hub),
    );
    let request_body = format!(
        r#"{{"product_id":{product_id},"amount":"20.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/subscriptions")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
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
    let subscription_id = payload["subscription"]["id"].as_u64().unwrap();
    assert_eq!(payload["subscription"]["user_id"], user_id);
    assert_eq!(payload["subscription"]["product_id"], product_id);
    assert_eq!(payload["subscription"]["asset_id"], asset_id);
    assert_eq!(payload["subscription"]["amount"], "20.000000000000000000");
    assert_eq!(payload["subscription"]["status"], "subscribed");

    let event_message =
        tokio::time::timeout(Duration::from_millis(100), private_events.recv()).await??;
    let event: Value = serde_json::from_str(event_message.payload())?;
    assert_eq!(event["type"], "earn.subscription.created");
    assert_eq!(event["subscription_id"], subscription_id);
    assert_eq!(event["product_id"], product_id);
    assert_eq!(event["asset_id"], asset_id);
    assert_eq!(event["amount"], "20.000000000000000000");
    assert_eq!(event["status"], "subscribed");

    let (available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(asset_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(available, decimal("80.000000000000000000"));

    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'earn_subscription' AND ref_id = ? AND change_type = 'earn_subscribe'",
    )
    .bind(subscription_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 1);

    Ok(())
}

#[tokio::test]
async fn earn_subscribe_rejects_amount_scale_above_decimal_storage() -> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let token = issue_token(&settings, "user:42", TokenScope::User, 900).unwrap();
    let response = user_routes()
        .with_state(AppState::new(settings))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/subscriptions")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"product_id":1,"amount":"20.0000000000000000001","idempotency_key":"scale-test"}"#,
                ))
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 4096).await?;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );
    let payload: Value = serde_json::from_slice(&body)?;
    assert_eq!(payload["code"], "VALIDATION_ERROR");
    assert_eq!(
        payload["message"],
        "validation error: earn subscription amount supports at most 18 decimal places"
    );

    Ok(())
}

#[tokio::test]
async fn earn_redeem_matured_subscription_credits_principal_yield_and_writes_ledger()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let (asset_id, _asset_symbol) = create_asset(&mut fixture_tx, "RA").await;
    let product_id = seed_earn_product(&mut fixture_tx, asset_id).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(asset_id)
        .bind(decimal("500.000000000000000000"))
        .execute(&mut *fixture_tx)
        .await?;
    fixture_tx.commit().await?;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let subscribe_app =
        user_routes().with_state(AppState::new(settings.clone()).with_mysql(pool.clone()));
    let idempotency_key = format!("earn-redeem-{}", Uuid::now_v7().simple());
    let request_body = format!(
        r#"{{"product_id":{product_id},"amount":"365.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );
    let subscribe_response = subscribe_app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/subscriptions")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await?;
    let subscribe_status = subscribe_response.status();
    let subscribe_body = axum::body::to_bytes(subscribe_response.into_body(), 65_536).await?;
    assert_eq!(
        subscribe_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&subscribe_body)
    );
    let subscribe_payload: Value = serde_json::from_slice(&subscribe_body)?;
    let subscription_id = subscribe_payload["subscription"]["id"].as_u64().unwrap();
    sqlx::query("UPDATE earn_subscriptions SET matures_at = ? WHERE id = ?")
        .bind(Utc::now() - chrono::TimeDelta::seconds(1))
        .bind(subscription_id)
        .execute(&pool)
        .await?;

    let hub = EventBroadcastHub::new(16);
    let _keepalive_hub = hub.clone();
    let mut private_events = hub.subscribe(&WebSocketChannel::private_user(user_id));
    let redeem_app = user_routes().with_state(
        AppState::new(settings)
            .with_mysql(pool.clone())
            .with_event_broadcast_hub(hub),
    );
    for attempt in 0..2 {
        let response = redeem_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/earn/subscriptions/{subscription_id}/redeem"))
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
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
        assert_eq!(payload["subscription"]["id"], subscription_id);
        assert_eq!(payload["subscription"]["status"], "redeemed");
        assert_eq!(payload["principal_amount"], "365.000000000000000000");
        assert_eq!(payload["yield_amount"], "3.600000000000000000");
        assert_eq!(payload["redeem_amount"], "368.600000000000000000");
        if attempt == 0 {
            let event_message =
                tokio::time::timeout(Duration::from_millis(100), private_events.recv()).await??;
            let event: Value = serde_json::from_str(event_message.payload())?;
            assert_eq!(event["type"], "earn.subscription.redeemed");
            assert_eq!(event["subscription_id"], subscription_id);
            assert_eq!(event["product_id"], product_id);
            assert_eq!(event["asset_id"], asset_id);
            assert_eq!(event["principal_amount"], "365.000000000000000000");
            assert_eq!(event["yield_amount"], "3.600000000000000000");
            assert_eq!(event["redeem_amount"], "368.600000000000000000");
            assert_eq!(event["status"], "redeemed");
        } else {
            assert!(
                tokio::time::timeout(Duration::from_millis(25), private_events.recv())
                    .await
                    .is_err(),
                "idempotent earn redeem replay must not publish duplicate private event"
            );
        }
    }

    let (available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(asset_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(available, decimal("503.600000000000000000"));

    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'earn_subscription' AND ref_id = ? AND change_type = 'earn_redeem'",
    )
    .bind(subscription_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 1);

    sqlx::query(
        "UPDATE earn_subscriptions SET amount = ?, apr_rate = ?, term_days = ? WHERE id = ?",
    )
    .bind(decimal("100.000000000000000000"))
    .bind(decimal("0.01000000"))
    .bind(1_u32)
    .bind(subscription_id)
    .execute(&pool)
    .await?;
    let replay_response = redeem_app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/earn/subscriptions/{subscription_id}/redeem"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let replay_status = replay_response.status();
    let replay_body = axum::body::to_bytes(replay_response.into_body(), 65_536).await?;
    assert_eq!(
        replay_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&replay_body)
    );
    let replay_payload: Value = serde_json::from_slice(&replay_body)?;
    assert_eq!(replay_payload["principal_amount"], "365.000000000000000000");
    assert_eq!(replay_payload["yield_amount"], "3.600000000000000000");
    assert_eq!(replay_payload["redeem_amount"], "368.600000000000000000");

    let (available_after_replay,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(asset_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(available_after_replay, decimal("503.600000000000000000"));

    let (ledger_count_after_replay,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'earn_subscription' AND ref_id = ? AND change_type = 'earn_redeem'",
    )
    .bind(subscription_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count_after_replay, 1);

    Ok(())
}

#[tokio::test]
async fn earn_redeem_rejects_early_subscription() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let (asset_id, _asset_symbol) = create_asset(&mut fixture_tx, "EA").await;
    let product_id = seed_earn_product(&mut fixture_tx, asset_id).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(asset_id)
        .bind(decimal("100.000000000000000000"))
        .execute(&mut *fixture_tx)
        .await?;
    fixture_tx.commit().await?;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let app = user_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let idempotency_key = format!("earn-early-{}", Uuid::now_v7().simple());
    let request_body = format!(
        r#"{{"product_id":{product_id},"amount":"20.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );
    let subscribe_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/earn/subscriptions")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await?;
    let subscribe_status = subscribe_response.status();
    let subscribe_body = axum::body::to_bytes(subscribe_response.into_body(), 65_536).await?;
    assert_eq!(
        subscribe_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&subscribe_body)
    );
    let subscribe_payload: Value = serde_json::from_slice(&subscribe_body)?;
    let subscription_id = subscribe_payload["subscription"]["id"].as_u64().unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/earn/subscriptions/{subscription_id}/redeem"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 65_536).await?;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );
    let payload: Value = serde_json::from_slice(&body)?;
    assert_eq!(payload["code"], "VALIDATION_ERROR");

    Ok(())
}

#[tokio::test]
async fn earn_subscribe_is_idempotent_for_repeated_key() -> Result<(), Box<dyn Error>> {
    assert_earn_subscribe_idempotent(false).await
}

#[tokio::test]
async fn earn_subscribe_concurrent_idempotency_key_debits_once() -> Result<(), Box<dyn Error>> {
    assert_earn_subscribe_idempotent(true).await
}

async fn assert_earn_subscribe_idempotent(concurrent: bool) -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let mut fixture_tx = pool.begin().await?;
    let user_id = create_user(&mut fixture_tx).await;
    let (asset_id, _asset_symbol) = create_asset(&mut fixture_tx, "IA").await;
    let product_id = seed_earn_product(&mut fixture_tx, asset_id).await;
    sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(asset_id)
        .bind(decimal("100.000000000000000000"))
        .execute(&mut *fixture_tx)
        .await?;
    fixture_tx.commit().await?;

    let token = issue_token(&settings, format!("user:{user_id}"), TokenScope::User, 900).unwrap();
    let idempotency_key = format!("earn-repeat-{}", Uuid::now_v7().simple());
    let app = user_routes().with_state(AppState::new(settings).with_mysql(pool.clone()));
    let request_body = format!(
        r#"{{"product_id":{product_id},"amount":"20.000000000000000000","idempotency_key":"{idempotency_key}"}}"#
    );

    let first_request = Request::builder()
        .method("POST")
        .uri("/earn/subscriptions")
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(request_body.clone()))
        .unwrap();
    let second_request = Request::builder()
        .method("POST")
        .uri("/earn/subscriptions")
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(request_body))
        .unwrap();

    let (first, second) = if concurrent {
        tokio::join!(
            app.clone().oneshot(first_request),
            app.oneshot(second_request)
        )
    } else {
        (
            app.clone().oneshot(first_request).await,
            app.oneshot(second_request).await,
        )
    };
    let first = first?;
    let second = second?;
    assert_eq!(first.status(), StatusCode::OK);
    let first_body = axum::body::to_bytes(first.into_body(), 65_536).await?;
    let first_payload: Value = serde_json::from_slice(&first_body)?;
    let first_subscription_id = first_payload["subscription"]["id"].as_u64().unwrap();

    let second_status = second.status();
    let second_body = axum::body::to_bytes(second.into_body(), 65_536).await?;
    let second_payload: Value = serde_json::from_slice(&second_body)?;
    assert_eq!(
        second_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&second_body)
    );
    assert_eq!(second_payload["subscription"]["id"], first_subscription_id);

    let (available,): (BigDecimal,) =
        sqlx::query_as("SELECT available FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
            .bind(user_id)
            .bind(asset_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(available, decimal("80.000000000000000000"));

    let (ledger_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'earn_subscription' AND ref_id = ?",
    )
    .bind(first_subscription_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(ledger_count, 1);

    Ok(())
}
