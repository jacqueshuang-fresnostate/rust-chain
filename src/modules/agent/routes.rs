use crate::{
    error::{AppError, AppResult},
    modules::auth::AgentAuth,
    state::AppState,
    time::{option_unix_millis, unix_millis},
};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, patch},
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{MySql, Pool};
use uuid::Uuid;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/me", get(me))
        .route("/dashboard", get(dashboard))
        .route("/users", get(list_users))
        .route(
            "/invite-codes",
            get(list_invite_codes).post(create_invite_code),
        )
        .route("/invite-codes/:id/status", patch(update_invite_code_status))
        .route("/commissions", get(list_commissions))
        .route("/convert/stats", get(convert_stats))
        .route("/team-tree", get(team_tree))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AgentMeResponse {
    agent_admin_id: u64,
    agent_id: u64,
    username: String,
    agent_code: String,
    level: i32,
    agent_status: String,
    admin_status: String,
    #[serde(default, with = "option_unix_millis")]
    last_login_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
struct AgentUsersResponse {
    users: Vec<AgentTeamUserResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AgentTeamUserResponse {
    user_id: u64,
    email: Option<String>,
    phone: Option<String>,
    status: String,
    kyc_level: i32,
    root_agent_id: u64,
    depth: i32,
    #[serde(with = "unix_millis")]
    referred_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct CreateInviteCodeRequest {
    usage_limit: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct UpdateInviteCodeStatusRequest {
    status: String,
}

#[derive(Debug, Serialize)]
struct AgentInviteCodesResponse {
    invite_codes: Vec<AgentInviteCodeResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AgentInviteCodeResponse {
    id: u64,
    owner_id: u64,
    code: String,
    usage_limit: Option<i32>,
    used_count: i32,
    status: String,
    #[serde(with = "unix_millis")]
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct AgentTeamTreeResponse {
    root_agent_id: u64,
    nodes: Vec<AgentTeamTreeNodeResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AgentTeamTreeNodeResponse {
    user_id: u64,
    email: Option<String>,
    phone: Option<String>,
    status: String,
    direct_inviter_id: Option<u64>,
    direct_inviter_type: Option<String>,
    depth: i32,
    path: String,
    #[serde(with = "unix_millis")]
    referred_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct AgentCommissionsResponse {
    agent_id: u64,
    total_records: u64,
    total_commission_amount: BigDecimal,
    commissions: Vec<AgentCommissionResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AgentCommissionResponse {
    id: u64,
    user_id: u64,
    email: Option<String>,
    source_type: String,
    source_id: String,
    source_amount: BigDecimal,
    commission_amount: BigDecimal,
    status: String,
    depth: i32,
    payout_ledger_id: Option<u64>,
    payout_asset_id: Option<u64>,
    payout_amount: Option<BigDecimal>,
    payout_balance_after: Option<BigDecimal>,
    #[serde(default, with = "option_unix_millis")]
    payout_created_at: Option<DateTime<Utc>>,
    #[serde(with = "unix_millis")]
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AgentDashboardResponse {
    agent_id: u64,
    team_user_count: i64,
    active_invite_code_count: i64,
    commission_record_count: i64,
    pending_commission_amount: BigDecimal,
    settled_commission_amount: BigDecimal,
    total_commission_amount: BigDecimal,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AgentConvertStatsQueryRow {
    agent_id: u64,
    total_orders: i64,
    pending_orders: BigDecimal,
    completed_orders: BigDecimal,
    total_from_amount: BigDecimal,
    total_to_amount: BigDecimal,
}

#[derive(Debug, Serialize)]
struct AgentConvertStatsResponse {
    agent_id: u64,
    total_orders: i64,
    pending_orders: i64,
    completed_orders: i64,
    total_from_amount: BigDecimal,
    total_to_amount: BigDecimal,
}

async fn me(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
) -> AppResult<Json<AgentMeResponse>> {
    let agent_admin_id = agent_admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;

    sqlx::query_as::<_, AgentMeResponse>(
        r#"SELECT agent_admins.id AS agent_admin_id,
                  agents.id AS agent_id,
                  agent_admins.username,
                  agents.agent_code,
                  agents.level,
                  agents.status AS agent_status,
                  agent_admins.status AS admin_status,
                  agent_admins.last_login_at
           FROM agent_admin_users agent_admins
           INNER JOIN agents ON agents.id = agent_admins.agent_id
           WHERE agent_admins.id = ?
             AND agent_admins.status = 'active'
             AND agents.status = 'active'
           LIMIT 1"#,
    )
    .bind(agent_admin_id)
    .fetch_optional(&pool)
    .await?
    .map(Json)
    .ok_or(AppError::Unauthorized)
}

async fn dashboard(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
) -> AppResult<Json<AgentDashboardResponse>> {
    let (pool, agent_id) = agent_context(&state, &claims.sub).await?;
    let dashboard = sqlx::query_as::<_, AgentDashboardResponse>(
        r#"SELECT ? AS agent_id,
                  (SELECT COUNT(*)
                   FROM user_referrals
                   WHERE root_agent_id = ?) AS team_user_count,
                  (SELECT COUNT(*)
                   FROM invite_codes
                   WHERE owner_type = 'agent' AND owner_id = ? AND status = 'active')
                   AS active_invite_code_count,
                  COUNT(records.id) AS commission_record_count,
                  COALESCE(SUM(CASE WHEN records.status = 'pending'
                                    THEN records.commission_amount ELSE 0 END), 0)
                   AS pending_commission_amount,
                  COALESCE(SUM(CASE WHEN records.status = 'settled'
                                    THEN records.commission_amount ELSE 0 END), 0)
                   AS settled_commission_amount,
                  COALESCE(SUM(records.commission_amount), 0) AS total_commission_amount
           FROM agent_commission_records records
           INNER JOIN user_referrals referrals ON referrals.user_id = records.user_id
           WHERE records.agent_id = ? AND referrals.root_agent_id = ?"#,
    )
    .bind(agent_id)
    .bind(agent_id)
    .bind(agent_id)
    .bind(agent_id)
    .bind(agent_id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(dashboard))
}

async fn convert_stats(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
) -> AppResult<Json<AgentConvertStatsResponse>> {
    let (pool, agent_id) = agent_context(&state, &claims.sub).await?;
    let row = sqlx::query_as::<_, AgentConvertStatsQueryRow>(
        r#"SELECT ? AS agent_id,
                  COUNT(orders.id) AS total_orders,
                  COALESCE(SUM(CASE WHEN orders.status = 'pending' THEN 1 ELSE 0 END), 0)
                   AS pending_orders,
                  COALESCE(SUM(CASE WHEN orders.status = 'completed' THEN 1 ELSE 0 END), 0)
                   AS completed_orders,
                  COALESCE(SUM(orders.from_amount), 0) AS total_from_amount,
                  COALESCE(SUM(orders.to_amount), 0) AS total_to_amount
           FROM convert_orders orders
           INNER JOIN user_referrals referrals ON referrals.user_id = orders.user_id
           WHERE referrals.root_agent_id = ?"#,
    )
    .bind(agent_id)
    .bind(agent_id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(AgentConvertStatsResponse {
        agent_id: row.agent_id,
        total_orders: row.total_orders,
        pending_orders: row.pending_orders.to_string().parse().map_err(|_| {
            AppError::Internal("failed to decode pending convert order count".to_owned())
        })?,
        completed_orders: row.completed_orders.to_string().parse().map_err(|_| {
            AppError::Internal("failed to decode completed convert order count".to_owned())
        })?,
        total_from_amount: row.total_from_amount,
        total_to_amount: row.total_to_amount,
    }))
}

async fn list_users(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
) -> AppResult<Json<AgentUsersResponse>> {
    let (pool, agent_id) = agent_context(&state, &claims.sub).await?;
    let users = sqlx::query_as::<_, AgentTeamUserResponse>(
        r#"SELECT u.id AS user_id, u.email, u.phone, u.status, u.kyc_level,
                  ur.root_agent_id, ur.depth, ur.created_at AS referred_at
           FROM user_referrals ur
           INNER JOIN users u ON u.id = ur.user_id
           WHERE ur.root_agent_id = ?
           ORDER BY u.id ASC
           LIMIT 100"#,
    )
    .bind(agent_id)
    .fetch_all(&pool)
    .await?;

    Ok(Json(AgentUsersResponse { users }))
}

async fn team_tree(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
) -> AppResult<Json<AgentTeamTreeResponse>> {
    let (pool, agent_id) = agent_context(&state, &claims.sub).await?;
    let nodes = sqlx::query_as::<_, AgentTeamTreeNodeResponse>(
        r#"SELECT u.id AS user_id, u.email, u.phone, u.status,
                  ur.direct_inviter_id, ur.direct_inviter_type, ur.depth,
                  ur.path, ur.created_at AS referred_at
           FROM user_referrals ur
           INNER JOIN users u ON u.id = ur.user_id
           WHERE ur.root_agent_id = ?
           ORDER BY ur.depth ASC, u.id ASC
           LIMIT 500"#,
    )
    .bind(agent_id)
    .fetch_all(&pool)
    .await?;

    Ok(Json(AgentTeamTreeResponse {
        root_agent_id: agent_id,
        nodes,
    }))
}

async fn list_commissions(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
) -> AppResult<Json<AgentCommissionsResponse>> {
    let (pool, agent_id) = agent_context(&state, &claims.sub).await?;
    let commissions = sqlx::query_as::<_, AgentCommissionResponse>(
        r#"SELECT records.id, records.user_id, users.email, records.source_type,
                  records.source_id, records.source_amount, records.commission_amount,
                  records.status, referrals.depth,
                  payout.id AS payout_ledger_id,
                  payout.asset_id AS payout_asset_id,
                  payout.amount AS payout_amount,
                  payout.balance_after AS payout_balance_after,
                  payout.created_at AS payout_created_at,
                  records.created_at
           FROM agent_commission_records records
           INNER JOIN user_referrals referrals ON referrals.user_id = records.user_id
           INNER JOIN users ON users.id = records.user_id
           LEFT JOIN agents ON agents.id = records.agent_id
           LEFT JOIN wallet_ledger payout
             ON payout.user_id = agents.user_id
            AND payout.ref_type = 'agent_commission'
            AND CAST(payout.ref_id AS UNSIGNED) = records.id
            AND payout.change_type = 'agent_commission_payout'
            AND records.status = 'settled'
           WHERE records.agent_id = ? AND referrals.root_agent_id = ?
           ORDER BY records.id ASC
           LIMIT 100"#,
    )
    .bind(agent_id)
    .bind(agent_id)
    .fetch_all(&pool)
    .await?;
    let total_commission_amount = commissions
        .iter()
        .map(|record| record.commission_amount.clone())
        .sum();

    Ok(Json(AgentCommissionsResponse {
        agent_id,
        total_records: commissions.len() as u64,
        total_commission_amount,
        commissions,
    }))
}

async fn list_invite_codes(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
) -> AppResult<Json<AgentInviteCodesResponse>> {
    let (pool, agent_id) = agent_context(&state, &claims.sub).await?;
    let invite_codes = sqlx::query_as::<_, AgentInviteCodeResponse>(
        r#"SELECT id, owner_id, code, usage_limit, used_count, status, created_at
           FROM invite_codes
           WHERE owner_type = 'agent' AND owner_id = ?
           ORDER BY id ASC
           LIMIT 100"#,
    )
    .bind(agent_id)
    .fetch_all(&pool)
    .await?;

    Ok(Json(AgentInviteCodesResponse { invite_codes }))
}

async fn create_invite_code(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateInviteCodeRequest>,
) -> AppResult<Json<AgentInviteCodeResponse>> {
    let (pool, agent_id) = agent_context(&state, &claims.sub).await?;
    if request.usage_limit.is_some_and(|limit| limit <= 0) {
        return Err(AppError::Validation(
            "usage_limit must be positive".to_owned(),
        ));
    }

    let code = format!("AGT{}", Uuid::now_v7().simple());
    let insert = sqlx::query(
        r#"INSERT INTO invite_codes (owner_type, owner_id, code, usage_limit)
           VALUES ('agent', ?, ?, ?)"#,
    )
    .bind(agent_id)
    .bind(&code)
    .bind(request.usage_limit)
    .execute(&pool)
    .await?;

    invite_code_by_id(&pool, agent_id, insert.last_insert_id()).await
}

async fn update_invite_code_status(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
    Path(invite_code_id): Path<u64>,
    Json(request): Json<UpdateInviteCodeStatusRequest>,
) -> AppResult<Json<AgentInviteCodeResponse>> {
    let status = invite_code_status(&request.status)?;
    let (pool, agent_id) = agent_context(&state, &claims.sub).await?;
    let result = sqlx::query(
        r#"UPDATE invite_codes
           SET status = ?
           WHERE id = ? AND owner_type = 'agent' AND owner_id = ?"#,
    )
    .bind(status)
    .bind(invite_code_id)
    .bind(agent_id)
    .execute(&pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    invite_code_by_id(&pool, agent_id, invite_code_id).await
}

async fn invite_code_by_id(
    pool: &Pool<MySql>,
    agent_id: u64,
    invite_code_id: u64,
) -> AppResult<Json<AgentInviteCodeResponse>> {
    sqlx::query_as::<_, AgentInviteCodeResponse>(
        r#"SELECT id, owner_id, code, usage_limit, used_count, status, created_at
           FROM invite_codes
           WHERE id = ? AND owner_type = 'agent' AND owner_id = ?
           LIMIT 1"#,
    )
    .bind(invite_code_id)
    .bind(agent_id)
    .fetch_optional(pool)
    .await?
    .map(Json)
    .ok_or(AppError::NotFound)
}

async fn agent_context(state: &AppState, subject: &str) -> AppResult<(Pool<MySql>, u64)> {
    let agent_admin_id = agent_admin_id_from_subject(subject)?;
    let pool = mysql_pool(state)?;
    let agent_id = root_agent_id_for_admin(&pool, agent_admin_id).await?;
    Ok((pool, agent_id))
}

fn invite_code_status(status: &str) -> AppResult<&'static str> {
    match status.trim() {
        "active" => Ok("active"),
        "disabled" => Ok("disabled"),
        _ => Err(AppError::Validation(
            "status must be active or disabled".to_owned(),
        )),
    }
}

async fn root_agent_id_for_admin(pool: &Pool<MySql>, agent_admin_id: u64) -> AppResult<u64> {
    sqlx::query_as::<_, (u64,)>(
        r#"SELECT agent_admins.agent_id
           FROM agent_admin_users agent_admins
           INNER JOIN agents ON agents.id = agent_admins.agent_id
           WHERE agent_admins.id = ?
             AND agent_admins.status = 'active'
             AND agents.status = 'active'
           LIMIT 1"#,
    )
    .bind(agent_admin_id)
    .fetch_optional(pool)
    .await?
    .map(|(agent_id,)| agent_id)
    .ok_or(AppError::Unauthorized)
}

fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for agent routes".to_owned())
    })
}

fn agent_admin_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("agent:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::Settings,
        modules::auth::{TokenScope, issue_token},
        state::AppState,
    };
    use axum::{
        body::Body,
        http::{Request, StatusCode, header::AUTHORIZATION},
    };
    use secrecy::SecretString;
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

    async fn get_dashboard(app: Router, token: Option<&str>) -> StatusCode {
        let mut request = Request::builder().uri("/dashboard");
        if let Some(token) = token {
            request = request.header(AUTHORIZATION, format!("Bearer {token}"));
        }

        app.oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
    }

    #[tokio::test]
    async fn agent_routes_require_agent_scope() {
        let state = test_state();
        let user_token = issue_token(
            &state.settings,
            "user:1",
            TokenScope::User,
            state.settings.jwt_access_ttl_seconds,
        )
        .unwrap();
        let admin_token = issue_token(
            &state.settings,
            "admin:1",
            TokenScope::Admin,
            state.settings.jwt_access_ttl_seconds,
        )
        .unwrap();
        let agent_token = issue_token(
            &state.settings,
            "agent:1",
            TokenScope::Agent,
            state.settings.jwt_access_ttl_seconds,
        )
        .unwrap();
        let app = routes().with_state(state);

        assert_eq!(
            get_dashboard(app.clone(), None).await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            get_dashboard(app.clone(), Some(&user_token)).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            get_dashboard(app.clone(), Some(&admin_token)).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            get_dashboard(app, Some(&agent_token)).await,
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
