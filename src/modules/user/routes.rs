use crate::{
    error::{AppError, AppResult},
    infra::email::EmailMessage,
    modules::{
        admin::smtp_config::load_enabled_smtp_config,
        auth::{
            TokenScope, UserAuth, hash_password, hash_refresh_token, issue_token, verify_password,
        },
    },
    state::AppState,
    time::{option_unix_millis, unix_millis},
};
use axum::{
    Json, Router,
    extract::State,
    routing::{get, patch, post},
};
use chrono::{DateTime, Duration, Utc};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{MySql, Pool, Transaction, types::Json as SqlxJson};
use uuid::Uuid;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/user/profile", get(profile))
        .route("/user/email/bind-code", post(send_email_bind_code))
        .route("/user/email/bind", post(bind_email))
        .route("/user/password", patch(change_password))
        .route(
            "/user/fund-password",
            post(create_fund_password).patch(change_fund_password),
        )
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
    #[serde(default, with = "option_unix_millis")]
    email_verified_at: Option<DateTime<Utc>>,
    fund_password_set: bool,
    #[serde(with = "unix_millis")]
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct BindEmailCodeRequest {
    email: String,
}

#[derive(Debug, Serialize)]
struct BindEmailCodeResponse {
    sent: bool,
    #[serde(with = "unix_millis")]
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct BindEmailRequest {
    email: String,
    code: String,
}

#[derive(Debug, Serialize)]
struct BindEmailResponse {
    email: String,
    #[serde(with = "unix_millis")]
    email_verified_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct ChangePasswordRequest {
    old_password: String,
    new_password: String,
}

#[derive(Debug, Serialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    token_type: &'static str,
    scope: TokenScope,
}

#[derive(Debug, Deserialize)]
struct CreateFundPasswordRequest {
    login_password: String,
    fund_password: String,
}

#[derive(Debug, Deserialize)]
struct ChangeFundPasswordRequest {
    old_fund_password: String,
    new_fund_password: String,
}

#[derive(Debug, Serialize)]
struct FundPasswordResponse {
    fund_password_set: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct UserPasswordRow {
    id: u64,
    password_hash: String,
    status: String,
}

#[derive(Debug, sqlx::FromRow)]
struct EmailVerificationRow {
    id: u64,
    code_hash: String,
    attempt_count: i32,
    expires_at: DateTime<Utc>,
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
        r#"SELECT users.id, users.email, users.phone, users.status, users.kyc_level,
                  users.email_verified_at,
                  CASE WHEN security.fund_password_hash IS NULL THEN FALSE ELSE TRUE END AS fund_password_set,
                  users.created_at
           FROM users
           LEFT JOIN user_security security ON security.user_id = users.id
           WHERE users.id = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    Ok(Json(profile))
}

async fn send_email_bind_code(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<BindEmailCodeRequest>,
) -> AppResult<Json<BindEmailCodeResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let email = validate_email(&request.email, "email")?;
    let pool = mysql_pool(&state)?;
    let now = Utc::now();
    let expires_at = now + Duration::minutes(10);
    let code = generate_email_code()?;
    let code_hash = hash_password(&code)?;

    let sender = state
        .email_sender
        .clone()
        .ok_or_else(|| AppError::Internal("email sender is not configured".to_owned()))?;
    let smtp_config =
        load_enabled_smtp_config(&pool, state.settings.exposed_credential_encryption_key())
            .await?
            .ok_or_else(|| {
                AppError::Internal("enabled smtp config is not configured".to_owned())
            })?;

    let mut tx = pool.begin().await?;
    ensure_active_user_in_tx(&mut tx, user_id).await?;
    ensure_email_available_in_tx(&mut tx, user_id, &email).await?;
    ensure_email_bind_not_cooling_down_in_tx(&mut tx, user_id, &email, now).await?;
    sqlx::query(
        r#"UPDATE user_email_verifications
           SET status = 'superseded'
           WHERE user_id = ? AND purpose = 'bind' AND status = 'pending'"#,
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"INSERT INTO user_email_verifications
           (user_id, email, purpose, code_hash, status, expires_at, sent_at)
           VALUES (?, ?, 'bind', ?, 'pending', ?, ?)"#,
    )
    .bind(user_id)
    .bind(&email)
    .bind(&code_hash)
    .bind(expires_at.naive_utc())
    .bind(now.naive_utc())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    sender
        .send(
            smtp_config,
            EmailMessage {
                to: email.clone(),
                subject: "绑定邮箱验证码".to_owned(),
                text_body: format!("您的绑定邮箱验证码是 {code}，10 分钟内有效。"),
            },
        )
        .await?;

    Ok(Json(BindEmailCodeResponse {
        sent: true,
        expires_at,
    }))
}

async fn bind_email(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<BindEmailRequest>,
) -> AppResult<Json<BindEmailResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let email = validate_email(&request.email, "email")?;
    let code = validate_email_code(&request.code)?;
    let pool = mysql_pool(&state)?;
    let verified_at = Utc::now();
    let mut tx = pool.begin().await?;

    ensure_active_user_in_tx(&mut tx, user_id).await?;
    ensure_email_available_in_tx(&mut tx, user_id, &email).await?;
    let verification = lock_latest_pending_email_verification_in_tx(&mut tx, user_id, &email)
        .await?
        .ok_or_else(|| AppError::Validation("email verification code is invalid".to_owned()))?;
    if verification.expires_at <= verified_at || verification.attempt_count >= 5 {
        return Err(AppError::Validation(
            "email verification code is expired".to_owned(),
        ));
    }
    if !verify_password(&verification.code_hash, &code)? {
        sqlx::query(
            r#"UPDATE user_email_verifications
               SET attempt_count = attempt_count + 1
               WHERE id = ?"#,
        )
        .bind(verification.id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        return Err(AppError::Validation(
            "email verification code is invalid".to_owned(),
        ));
    }

    sqlx::query(
        r#"UPDATE users
           SET email = ?, email_verified_at = ?
           WHERE id = ?"#,
    )
    .bind(&email)
    .bind(verified_at.naive_utc())
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(map_duplicate_email)?;
    sqlx::query(
        r#"UPDATE user_email_verifications
           SET status = 'verified', verified_at = ?
           WHERE id = ?"#,
    )
    .bind(verified_at.naive_utc())
    .bind(verification.id)
    .execute(&mut *tx)
    .await?;
    insert_user_audit_event_in_tx(
        &mut tx,
        user_id,
        "user.email.bind",
        "user",
        user_id.to_string(),
        None,
        Some(json!({ "email": email })),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(BindEmailResponse {
        email,
        email_verified_at: verified_at,
    }))
}

async fn change_password(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<ChangePasswordRequest>,
) -> AppResult<Json<TokenResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let old_password = required_string(Some(request.old_password), "old_password")?;
    let new_password = validate_login_password(&request.new_password, "new_password")?;
    if old_password == new_password {
        return Err(AppError::Validation(
            "new_password must be different from old_password".to_owned(),
        ));
    }
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    let user = lock_user_password_in_tx(&mut tx, user_id).await?;
    if user.status != "active" || !verify_password(&user.password_hash, &old_password)? {
        return Err(AppError::Unauthorized);
    }
    let password_hash = hash_password(&new_password)?;
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(password_hash)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"UPDATE refresh_tokens
           SET revoked_at = CURRENT_TIMESTAMP(6)
           WHERE actor_type = 'user' AND actor_id = ? AND revoked_at IS NULL"#,
    )
    .bind(user.id)
    .execute(&mut *tx)
    .await?;
    insert_user_audit_event_in_tx(
        &mut tx,
        user.id,
        "user.password.change",
        "user",
        user.id.to_string(),
        None,
        Some(json!({ "changed": true })),
    )
    .await?;
    let tokens = issue_user_tokens_in_tx(&mut tx, &state, user.id).await?;
    tx.commit().await?;

    Ok(Json(tokens))
}

async fn create_fund_password(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateFundPasswordRequest>,
) -> AppResult<Json<FundPasswordResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let login_password = required_string(Some(request.login_password), "login_password")?;
    let fund_password = validate_fund_password(&request.fund_password, "fund_password")?;
    if login_password == fund_password {
        return Err(AppError::Validation(
            "fund_password must be different from login_password".to_owned(),
        ));
    }
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    let user = lock_user_password_in_tx(&mut tx, user_id).await?;
    if user.status != "active" || !verify_password(&user.password_hash, &login_password)? {
        return Err(AppError::Unauthorized);
    }
    let existing: Option<(Option<String>,)> = sqlx::query_as(
        r#"SELECT fund_password_hash
           FROM user_security
           WHERE user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user.id)
    .fetch_optional(&mut *tx)
    .await?;
    if existing.and_then(|row| row.0).is_some() {
        return Err(AppError::Conflict(
            "fund password already exists".to_owned(),
        ));
    }
    let fund_password_hash = hash_password(&fund_password)?;
    sqlx::query(
        r#"INSERT INTO user_security (user_id, fund_password_hash)
           VALUES (?, ?)
           ON DUPLICATE KEY UPDATE fund_password_hash = VALUES(fund_password_hash)"#,
    )
    .bind(user.id)
    .bind(fund_password_hash)
    .execute(&mut *tx)
    .await?;
    insert_user_audit_event_in_tx(
        &mut tx,
        user.id,
        "user.fund_password.create",
        "user_security",
        user.id.to_string(),
        None,
        Some(json!({ "fund_password_set": true })),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(FundPasswordResponse {
        fund_password_set: true,
    }))
}

async fn change_fund_password(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<ChangeFundPasswordRequest>,
) -> AppResult<Json<FundPasswordResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let old_fund_password =
        validate_fund_password(&request.old_fund_password, "old_fund_password")?;
    let new_fund_password =
        validate_fund_password(&request.new_fund_password, "new_fund_password")?;
    if old_fund_password == new_fund_password {
        return Err(AppError::Validation(
            "new_fund_password must be different from old_fund_password".to_owned(),
        ));
    }
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    ensure_active_user_in_tx(&mut tx, user_id).await?;
    let existing_hash: Option<String> = sqlx::query_scalar(
        r#"SELECT fund_password_hash
           FROM user_security
           WHERE user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    .flatten();
    let existing_hash = existing_hash.ok_or(AppError::NotFound)?;
    if !verify_password(&existing_hash, &old_fund_password)? {
        return Err(AppError::Unauthorized);
    }
    let new_hash = hash_password(&new_fund_password)?;
    sqlx::query("UPDATE user_security SET fund_password_hash = ? WHERE user_id = ?")
        .bind(new_hash)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    insert_user_audit_event_in_tx(
        &mut tx,
        user_id,
        "user.fund_password.change",
        "user_security",
        user_id.to_string(),
        None,
        Some(json!({ "fund_password_set": true })),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(FundPasswordResponse {
        fund_password_set: true,
    }))
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

async fn ensure_active_user_in_tx(tx: &mut Transaction<'_, MySql>, user_id: u64) -> AppResult<()> {
    sqlx::query_as::<_, (u64,)>(
        r#"SELECT id
           FROM users
           WHERE id = ? AND status = 'active'
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::Unauthorized)?;
    Ok(())
}

async fn ensure_email_available_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    email: &str,
) -> AppResult<()> {
    let existing_user_id: Option<u64> = sqlx::query_scalar(
        r#"SELECT id
           FROM users
           WHERE email = ? AND id <> ?
           LIMIT 1"#,
    )
    .bind(email)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?;
    if existing_user_id.is_some() {
        return Err(AppError::Conflict("email already exists".to_owned()));
    }
    Ok(())
}

async fn ensure_email_bind_not_cooling_down_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    email: &str,
    now: DateTime<Utc>,
) -> AppResult<()> {
    let sent_at: Option<DateTime<Utc>> = sqlx::query_scalar(
        r#"SELECT sent_at
           FROM user_email_verifications
           WHERE user_id = ? AND email = ? AND purpose = 'bind' AND status = 'pending'
           ORDER BY id DESC
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(email)
    .fetch_optional(&mut **tx)
    .await?;
    if sent_at.is_some_and(|sent_at| sent_at + Duration::seconds(60) > now) {
        return Err(AppError::Validation(
            "email verification code was sent recently".to_owned(),
        ));
    }
    Ok(())
}

async fn lock_latest_pending_email_verification_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    email: &str,
) -> AppResult<Option<EmailVerificationRow>> {
    sqlx::query_as::<_, EmailVerificationRow>(
        r#"SELECT id, code_hash, attempt_count, expires_at
           FROM user_email_verifications
           WHERE user_id = ? AND email = ? AND purpose = 'bind' AND status = 'pending'
           ORDER BY id DESC
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(email)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

async fn lock_user_password_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<UserPasswordRow> {
    sqlx::query_as::<_, UserPasswordRow>(
        r#"SELECT id, password_hash, status
           FROM users
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::Unauthorized)
}

async fn issue_user_tokens_in_tx(
    tx: &mut Transaction<'_, MySql>,
    state: &AppState,
    user_id: u64,
) -> AppResult<TokenResponse> {
    let subject = format!("user:{user_id}");
    let access_token = issue_token(
        &state.settings,
        subject.clone(),
        TokenScope::User,
        state.settings.jwt_access_ttl_seconds,
    )?;
    let refresh_token = issue_token(
        &state.settings,
        subject,
        TokenScope::User,
        state.settings.jwt_refresh_ttl_seconds,
    )?;
    let token_hash = hash_refresh_token(&refresh_token)?;
    let expires_at =
        Utc::now().naive_utc() + Duration::seconds(state.settings.jwt_refresh_ttl_seconds as i64);

    sqlx::query(
        r#"INSERT INTO refresh_tokens (user_id, actor_type, actor_id, token_hash, expires_at)
           VALUES (?, 'user', ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(user_id)
    .bind(token_hash)
    .bind(expires_at)
    .execute(&mut **tx)
    .await?;

    Ok(TokenResponse {
        access_token,
        refresh_token,
        token_type: "Bearer",
        scope: TokenScope::User,
    })
}

async fn insert_user_audit_event_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    action: &'static str,
    target_type: &'static str,
    target_id: String,
    before_json: Option<Value>,
    after_json: Option<Value>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO audit_events
           (actor_type, actor_id, action, target_type, target_id, before_json, after_json)
           VALUES ('user', ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(before_json.map(SqlxJson))
    .bind(after_json.map(SqlxJson))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn validate_email(value: &str, field: &str) -> AppResult<String> {
    let email = required_string(Some(value.to_owned()), field)?;
    let mut parts = email.split('@');
    let local = parts.next().unwrap_or_default();
    let domain = parts.next().unwrap_or_default();
    if email.len() > 255
        || local.is_empty()
        || domain.is_empty()
        || parts.next().is_some()
        || email.chars().any(char::is_whitespace)
    {
        return Err(AppError::Validation(format!("{field} is invalid")));
    }
    Ok(email)
}

fn validate_email_code(value: &str) -> AppResult<String> {
    let code = required_string(Some(value.to_owned()), "code")?;
    if code.len() != 6 || !code.chars().all(|char| char.is_ascii_digit()) {
        return Err(AppError::Validation("code is invalid".to_owned()));
    }
    Ok(code)
}

fn validate_login_password(value: &str, field: &str) -> AppResult<String> {
    let password = required_string(Some(value.to_owned()), field)?;
    if password.chars().count() < 8 {
        return Err(AppError::Validation(format!("{field} is too short")));
    }
    Ok(password)
}

fn validate_fund_password(value: &str, field: &str) -> AppResult<String> {
    let password = required_string(Some(value.to_owned()), field)?;
    if password.len() != 6 || !password.chars().all(|char| char.is_ascii_digit()) {
        return Err(AppError::Validation(format!("{field} must be 6 digits")));
    }
    Ok(password)
}

fn generate_email_code() -> AppResult<String> {
    let rng = SystemRandom::new();
    let mut bytes = [0_u8; 4];
    rng.fill(&mut bytes)
        .map_err(|_| AppError::Internal("email verification code generation failed".to_owned()))?;
    let value = u32::from_be_bytes(bytes) % 1_000_000;
    Ok(format!("{value:06}"))
}

fn required_string(value: Option<String>, field: &str) -> AppResult<String> {
    optional_string(value).ok_or_else(|| AppError::Validation(format!("{field} is required")))
}

fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn map_duplicate_email(error: sqlx::Error) -> AppError {
    if is_duplicate_key(&error) {
        AppError::Conflict("email already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}

fn is_duplicate_key(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(database_error) if database_error.code().as_deref() == Some("1062"))
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
