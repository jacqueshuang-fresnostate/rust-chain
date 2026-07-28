use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use chrono::{Duration, NaiveDateTime, Utc};
use exchange_api::{
    build_router,
    config::Settings,
    modules::{
        auth::hash_password,
        security::{TOTP_DIGITS, TOTP_STEP_SECONDS, base32_decode_no_padding, totp_code_for_time},
    },
    state::AppState,
};
use secrecy::SecretString;
use serde_json::{Value, json};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::error::Error;
use tower::ServiceExt;
use uuid::Uuid;

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
            eprintln!(
                "skipping login 2FA setup route integration test because DATABASE_URL is not set"
            );
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
    let suffix = Uuid::now_v7().simple().to_string();
    sqlx::query("INSERT INTO users (email, phone, password_hash) VALUES (?, ?, ?)")
        .bind(format!("login-setup-{suffix}@example.test"))
        .bind(format!("176{}", &suffix[16..27]))
        .bind(hash_password("CorrectPassword123!").unwrap())
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id()
}

async fn create_challenge(
    pool: &MySqlPool,
    user_id: u64,
    challenge_type: &str,
    expires_at: chrono::DateTime<Utc>,
    consumed_at: Option<chrono::DateTime<Utc>>,
) -> String {
    let challenge_id = Uuid::now_v7().to_string();
    sqlx::query(
        r#"INSERT INTO login_two_factor_challenges
              (challenge_id, user_id, challenge_type, expires_at, consumed_at)
           VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(&challenge_id)
    .bind(user_id)
    .bind(challenge_type)
    .bind(expires_at.naive_utc())
    .bind(consumed_at.map(|value| value.naive_utc()))
    .execute(pool)
    .await
    .unwrap();
    challenge_id
}

async fn cleanup_user(pool: &MySqlPool, user_id: u64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM refresh_tokens WHERE actor_type = 'user' AND actor_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM login_two_factor_challenges WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM user_two_factor_settings WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

async fn post_json(app: Router, path: &str, payload: Value) -> Response {
    app.oneshot(
        Request::builder()
            .method("POST")
            .uri(path)
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn response_json(response: Response) -> Result<Value, Box<dyn Error>> {
    let body = axum::body::to_bytes(response.into_body(), 65_536).await?;
    Ok(serde_json::from_slice(&body)?)
}

fn current_totp_code(secret: &str) -> String {
    let bytes = base32_decode_no_padding(secret).unwrap();
    totp_code_for_time(
        &bytes,
        Utc::now().timestamp().max(0) as u64,
        TOTP_STEP_SECONDS,
        TOTP_DIGITS,
    )
}

fn invalid_totp_code(secret: &str) -> String {
    let bytes = base32_decode_no_padding(secret).unwrap();
    let timestamp = Utc::now().timestamp().max(0) as u64;
    let accepted = [
        totp_code_for_time(
            &bytes,
            timestamp.saturating_sub(TOTP_STEP_SECONDS),
            TOTP_STEP_SECONDS,
            TOTP_DIGITS,
        ),
        totp_code_for_time(&bytes, timestamp, TOTP_STEP_SECONDS, TOTP_DIGITS),
        totp_code_for_time(
            &bytes,
            timestamp.saturating_add(TOTP_STEP_SECONDS),
            TOTP_STEP_SECONDS,
            TOTP_DIGITS,
        ),
    ];

    (0..10)
        .map(|value| format!("{value:06}"))
        .find(|candidate| !accepted.contains(candidate))
        .unwrap()
}

#[tokio::test]
async fn login_setup_challenge_enrolls_totp_and_issues_tokens_once() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let user_id = create_user(&pool).await;
    let challenge_id = create_challenge(
        &pool,
        user_id,
        "setup_2fa",
        Utc::now() + Duration::minutes(5),
        None,
    )
    .await;
    let app = build_router(AppState::new(test_settings()).with_mysql(pool.clone()));

    let setup = post_json(
        app.clone(),
        "/api/v1/auth/login/2fa/setup",
        json!({ "setup_challenge_id": challenge_id }),
    )
    .await;
    assert_eq!(setup.status(), StatusCode::OK);
    let setup_payload = response_json(setup).await?;
    let secret = setup_payload["secret"].as_str().unwrap().to_owned();
    assert!(
        setup_payload["otpauth_uri"]
            .as_str()
            .unwrap()
            .starts_with("otpauth://totp/")
    );
    assert!(
        setup_payload["otpauth_uri"]
            .as_str()
            .unwrap()
            .contains(&format!("secret={secret}"))
    );
    assert!(setup_payload["expires_in_seconds"].as_i64().unwrap() > 0);

    let invalid = post_json(
        app.clone(),
        "/api/v1/auth/login/2fa/setup/confirm",
        json!({
            "setup_challenge_id": challenge_id,
            "totp_code": invalid_totp_code(&secret),
        }),
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response_json(invalid).await?["code"], "invalid_2fa_code");
    let consumed_after_invalid: Option<NaiveDateTime> = sqlx::query_scalar(
        "SELECT consumed_at FROM login_two_factor_challenges WHERE challenge_id = ?",
    )
    .bind(&challenge_id)
    .fetch_one(&pool)
    .await?;
    assert!(consumed_after_invalid.is_none());
    let enabled_after_invalid: bool =
        sqlx::query_scalar("SELECT totp_enabled FROM user_two_factor_settings WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&pool)
            .await?;
    assert!(!enabled_after_invalid);

    let confirmed = post_json(
        app.clone(),
        "/api/v1/auth/login/2fa/setup/confirm",
        json!({
            "setup_challenge_id": challenge_id,
            "totp_code": current_totp_code(&secret),
        }),
    )
    .await;
    assert_eq!(confirmed.status(), StatusCode::OK);
    let confirmed_payload = response_json(confirmed).await?;
    assert!(confirmed_payload["access_token"].is_string());
    assert!(confirmed_payload["refresh_token"].is_string());
    assert_eq!(confirmed_payload["token_type"], "Bearer");
    assert_eq!(confirmed_payload["scope"], "user");

    let (totp_enabled, secret_persisted): (bool, bool) = sqlx::query_as(
        r#"SELECT totp_enabled, totp_secret_encrypted IS NOT NULL
           FROM user_two_factor_settings
           WHERE user_id = ?"#,
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;
    assert!(totp_enabled);
    assert!(secret_persisted);

    let replayed = post_json(
        app,
        "/api/v1/auth/login/2fa/setup/confirm",
        json!({
            "setup_challenge_id": challenge_id,
            "totp_code": current_totp_code(&secret),
        }),
    )
    .await;
    assert_eq!(replayed.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(replayed).await?["code"],
        "login_2fa_challenge_expired"
    );

    cleanup_user(&pool, user_id).await?;
    Ok(())
}

#[tokio::test]
async fn login_setup_routes_reject_wrong_type_expired_and_consumed_challenges_before_mutation()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let user_id = create_user(&pool).await;
    let wrong_type = create_challenge(
        &pool,
        user_id,
        "login_2fa",
        Utc::now() + Duration::minutes(5),
        None,
    )
    .await;
    let expired = create_challenge(
        &pool,
        user_id,
        "setup_2fa",
        Utc::now() - Duration::seconds(1),
        None,
    )
    .await;
    let consumed = create_challenge(
        &pool,
        user_id,
        "setup_2fa",
        Utc::now() + Duration::minutes(5),
        Some(Utc::now()),
    )
    .await;
    let app = build_router(AppState::new(test_settings()).with_mysql(pool.clone()));

    for challenge_id in [&wrong_type, &expired, &consumed] {
        let setup = post_json(
            app.clone(),
            "/api/v1/auth/login/2fa/setup",
            json!({ "setup_challenge_id": challenge_id }),
        )
        .await;
        assert_eq!(setup.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response_json(setup).await?["code"],
            "login_2fa_challenge_expired"
        );
    }

    let wrong_type_confirm = post_json(
        app,
        "/api/v1/auth/login/2fa/setup/confirm",
        json!({
            "setup_challenge_id": wrong_type,
            "totp_code": "000000",
        }),
    )
    .await;
    assert_eq!(wrong_type_confirm.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(wrong_type_confirm).await?["code"],
        "login_2fa_challenge_expired"
    );

    let settings_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM user_two_factor_settings WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(settings_count, 0);

    cleanup_user(&pool, user_id).await?;
    Ok(())
}
