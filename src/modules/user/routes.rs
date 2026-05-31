use crate::{
    error::{AppError, AppResult},
    modules::auth::UserAuth,
    state::AppState,
    time::unix_millis,
};
use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{MySql, Pool, Transaction};
use uuid::Uuid;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/user/profile", get(profile))
        .route("/referral/my-code", get(my_referral_code))
        .route("/referral/bind", post(bind_referral_code))
        .route("/referral/my-invites", get(my_invites))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct UserProfileResponse {
    id: u64,
    email: Option<String>,
    phone: Option<String>,
    status: String,
    kyc_level: i32,
    #[serde(with = "unix_millis")]
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct BindReferralCodeRequest {
    code: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct ReferralBindingResponse {
    user_id: u64,
    direct_inviter_id: Option<u64>,
    direct_inviter_type: Option<String>,
    root_agent_id: Option<u64>,
    depth: i32,
    path: String,
    #[serde(with = "unix_millis")]
    created_at: DateTime<Utc>,
    bound: bool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct ReferralCodeResponse {
    id: u64,
    owner_type: String,
    owner_id: u64,
    code: String,
    usage_limit: Option<i32>,
    used_count: i32,
    status: String,
    root_agent_id: Option<u64>,
    #[serde(with = "unix_millis")]
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct MyInvitesResponse {
    users: Vec<MyInviteUserResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct MyInviteUserResponse {
    user_id: u64,
    email: Option<String>,
    phone: Option<String>,
    status: String,
    direct_inviter_type: Option<String>,
    direct_inviter_id: Option<u64>,
    root_agent_id: Option<u64>,
    depth: i32,
    path: String,
    #[serde(with = "unix_millis")]
    created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct InviteCodeRow {
    id: u64,
    owner_type: String,
    owner_id: u64,
    usage_limit: Option<i32>,
    used_count: i32,
}

#[derive(Debug, sqlx::FromRow)]
struct ReferralLinkRow {
    root_agent_id: Option<u64>,
    depth: i32,
    path: String,
}

async fn profile(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<UserProfileResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let profile = sqlx::query_as::<_, UserProfileResponse>(
        r#"SELECT id, email, phone, status, kyc_level, created_at
           FROM users
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    Ok(Json(profile))
}

async fn my_referral_code(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<ReferralCodeResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    ensure_user_exists(&pool, user_id).await?;

    if let Some(code) = load_user_invite_code(&pool, user_id).await? {
        return Ok(Json(code));
    }

    let code = format!("USR{}", Uuid::now_v7().simple());
    sqlx::query(
        r#"INSERT INTO invite_codes (owner_type, owner_id, code, status)
           VALUES ('user', ?, ?, 'active')"#,
    )
    .bind(user_id)
    .bind(&code)
    .execute(&pool)
    .await?;

    load_user_invite_code(&pool, user_id)
        .await?
        .map(Json)
        .ok_or_else(|| AppError::Internal("failed to create user invite code".to_owned()))
}

async fn bind_referral_code(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<BindReferralCodeRequest>,
) -> AppResult<Json<ReferralBindingResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let code = normalize_invite_code(&request.code)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;

    ensure_user_exists_in_tx(&mut tx, user_id).await?;
    if let Some(existing) = lock_user_referral_in_tx(&mut tx, user_id).await? {
        tx.commit().await?;
        return Ok(Json(existing));
    }

    let invite = lock_active_invite_code_in_tx(&mut tx, &code).await?;
    if invite
        .usage_limit
        .is_some_and(|usage_limit| invite.used_count >= usage_limit)
    {
        return Err(AppError::Validation("invite code is exhausted".to_owned()));
    }

    let (direct_inviter_type, direct_inviter_id, root_agent_id, depth, path) =
        match invite.owner_type.as_str() {
            "agent" => {
                ensure_active_agent_in_tx(&mut tx, invite.owner_id).await?;
                (
                    "agent".to_owned(),
                    invite.owner_id,
                    Some(invite.owner_id),
                    1,
                    format!("/agent:{}/user:{}", invite.owner_id, user_id),
                )
            }
            "user" => {
                if invite.owner_id == user_id {
                    return Err(AppError::Validation(
                        "user cannot bind own invite code".to_owned(),
                    ));
                }
                let inviter = load_referral_link_in_tx(&mut tx, invite.owner_id).await?;
                (
                    "user".to_owned(),
                    invite.owner_id,
                    inviter.root_agent_id,
                    inviter.depth + 1,
                    format!("{}/user:{}", inviter.path, user_id),
                )
            }
            _ => {
                return Err(AppError::Validation(
                    "unsupported invite code owner".to_owned(),
                ));
            }
        };

    sqlx::query(
        r#"INSERT INTO user_referrals
              (user_id, direct_inviter_id, direct_inviter_type, root_agent_id, depth, path)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(direct_inviter_id)
    .bind(direct_inviter_type)
    .bind(root_agent_id)
    .bind(depth)
    .bind(path)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE invite_codes SET used_count = used_count + 1 WHERE id = ?")
        .bind(invite.id)
        .execute(&mut *tx)
        .await?;

    let binding = load_user_referral_in_tx(&mut tx, user_id).await?;
    tx.commit().await?;

    Ok(Json(binding))
}

async fn my_invites(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<MyInvitesResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let users = sqlx::query_as::<_, MyInviteUserResponse>(
        r#"SELECT referrals.user_id, users.email, users.phone, users.status,
                  referrals.direct_inviter_type, referrals.direct_inviter_id,
                  referrals.root_agent_id, referrals.depth, referrals.path,
                  referrals.created_at
           FROM user_referrals referrals
           INNER JOIN users ON users.id = referrals.user_id
           WHERE referrals.direct_inviter_type = 'user'
             AND referrals.direct_inviter_id = ?
           ORDER BY referrals.created_at ASC, referrals.user_id ASC
           LIMIT 100"#,
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await?;

    Ok(Json(MyInvitesResponse { users }))
}

async fn ensure_user_exists(pool: &Pool<MySql>, user_id: u64) -> AppResult<()> {
    sqlx::query_as::<_, (u64,)>("SELECT id FROM users WHERE id = ? LIMIT 1")
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::Unauthorized)?;
    Ok(())
}

async fn ensure_user_exists_in_tx(tx: &mut Transaction<'_, MySql>, user_id: u64) -> AppResult<()> {
    sqlx::query_as::<_, (u64,)>("SELECT id FROM users WHERE id = ? LIMIT 1 FOR UPDATE")
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::Unauthorized)?;
    Ok(())
}

async fn load_user_invite_code(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Option<ReferralCodeResponse>> {
    sqlx::query_as::<_, ReferralCodeResponse>(
        r#"SELECT codes.id, codes.owner_type, codes.owner_id, codes.code,
                  codes.usage_limit, codes.used_count, codes.status,
                  referrals.root_agent_id, codes.created_at
           FROM invite_codes codes
           LEFT JOIN user_referrals referrals ON referrals.user_id = codes.owner_id
           WHERE codes.owner_type = 'user' AND codes.owner_id = ?
           ORDER BY codes.id ASC
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

async fn lock_user_referral_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<Option<ReferralBindingResponse>> {
    sqlx::query_as::<_, ReferralBindingResponse>(
        r#"SELECT user_id, direct_inviter_id, direct_inviter_type,
                  root_agent_id, depth, path, created_at,
                  true AS bound
           FROM user_referrals
           WHERE user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

async fn load_user_referral_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<ReferralBindingResponse> {
    sqlx::query_as::<_, ReferralBindingResponse>(
        r#"SELECT user_id, direct_inviter_id, direct_inviter_type,
                  root_agent_id, depth, path, created_at,
                  true AS bound
           FROM user_referrals
           WHERE user_id = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

async fn ensure_active_agent_in_tx(
    tx: &mut Transaction<'_, MySql>,
    agent_id: u64,
) -> AppResult<()> {
    sqlx::query_as::<_, (u64,)>(
        r#"SELECT id
           FROM agents
           WHERE id = ? AND status = 'active'
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(agent_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Validation("agent is inactive or not found".to_owned()))?;
    Ok(())
}

async fn load_referral_link_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<ReferralLinkRow> {
    sqlx::query_as::<_, ReferralLinkRow>(
        r#"SELECT root_agent_id, depth, path
           FROM user_referrals
           WHERE user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Validation("inviter has not bound an agent".to_owned()))
}

async fn lock_active_invite_code_in_tx(
    tx: &mut Transaction<'_, MySql>,
    code: &str,
) -> AppResult<InviteCodeRow> {
    sqlx::query_as::<_, InviteCodeRow>(
        r#"SELECT id, owner_type, owner_id, usage_limit, used_count
           FROM invite_codes
           WHERE code = ? AND status = 'active'
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(code)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Validation("invite code is inactive or not found".to_owned()))
}

fn normalize_invite_code(code: &str) -> AppResult<String> {
    let code = code.trim();
    if code.is_empty() {
        return Err(AppError::Validation("code is required".to_owned()));
    }
    Ok(code.to_owned())
}

fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for user routes".to_owned())
    })
}

fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::Settings,
        modules::auth::{TokenScope, issue_token},
    };
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use secrecy::SecretString;
    use serde_json::Value;
    use tower::ServiceExt;

    fn test_state() -> AppState {
        AppState::new(Settings {
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
        })
    }

    #[tokio::test]
    async fn profile_requires_mysql_after_user_auth() {
        let state = test_state();
        let token = issue_token(&state.settings, "user:42", TokenScope::User, 900).unwrap();
        let response = routes()
            .with_state(state)
            .oneshot(
                Request::builder()
                    .uri("/user/profile")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["code"], "INTERNAL_ERROR");
        assert!(
            payload["message"]
                .as_str()
                .unwrap()
                .contains("mysql pool is not configured for user routes")
        );
    }
}
