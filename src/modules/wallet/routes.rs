use crate::{
    error::{AppError, AppResult},
    modules::auth::UserAuth,
    state::AppState,
    time::unix_millis,
};
use axum::{Json, Router, extract::Query, extract::State, routing::get};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{MySql, Pool, QueryBuilder};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/wallet/accounts", get(list_accounts))
        .route("/wallet/ledger", get(list_ledger))
}

#[derive(Debug, Serialize)]
struct WalletAccountsResponse {
    accounts: Vec<WalletAccountResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct WalletAccountResponse {
    user_id: u64,
    asset_id: u64,
    symbol: String,
    available: BigDecimal,
    frozen: BigDecimal,
    locked: BigDecimal,
}

#[derive(Debug, Deserialize)]
struct WalletLedgerQuery {
    asset_id: Option<u64>,
    ref_type: Option<String>,
    ref_id: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Serialize)]
struct WalletLedgerResponse {
    entries: Vec<WalletLedgerEntryResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct WalletLedgerEntryResponse {
    id: u64,
    user_id: u64,
    asset_id: u64,
    symbol: String,
    change_type: String,
    amount: BigDecimal,
    balance_type: String,
    balance_after: BigDecimal,
    available_after: BigDecimal,
    frozen_after: BigDecimal,
    locked_after: BigDecimal,
    ref_type: String,
    ref_id: String,
    #[serde(with = "unix_millis")]
    created_at: DateTime<Utc>,
}

async fn list_accounts(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<WalletAccountsResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let accounts = sqlx::query_as::<_, WalletAccountResponse>(
        r#"SELECT wa.user_id, wa.asset_id, a.symbol, wa.available, wa.frozen, wa.locked
           FROM wallet_accounts wa
           JOIN assets a ON a.id = wa.asset_id
           WHERE wa.user_id = ?
           ORDER BY a.symbol ASC"#,
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await?;

    Ok(Json(WalletAccountsResponse { accounts }))
}

async fn list_ledger(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<WalletLedgerQuery>,
) -> AppResult<Json<WalletLedgerResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let limit = ledger_limit(query.limit);
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT wl.id, wl.user_id, wl.asset_id, a.symbol, wl.change_type, wl.amount,
                  wl.balance_type, wl.balance_after, wl.available_after, wl.frozen_after,
                  wl.locked_after, wl.ref_type, wl.ref_id, wl.created_at
           FROM wallet_ledger wl
           JOIN assets a ON a.id = wl.asset_id
           WHERE wl.user_id = "#,
    );
    builder.push_bind(user_id);

    if let Some(asset_id) = query.asset_id {
        builder.push(" AND wl.asset_id = ");
        builder.push_bind(asset_id);
    }
    if let Some(ref_type) = optional_query_string(query.ref_type) {
        builder.push(" AND wl.ref_type = ");
        builder.push_bind(ref_type);
    }
    if let Some(ref_id) = optional_query_string(query.ref_id) {
        builder.push(" AND wl.ref_id = ");
        builder.push_bind(ref_id);
    }

    builder.push(" ORDER BY wl.id DESC LIMIT ");
    builder.push_bind(limit as i64);

    let entries = builder
        .build_query_as::<WalletLedgerEntryResponse>()
        .fetch_all(&pool)
        .await?;

    Ok(Json(WalletLedgerResponse { entries }))
}

fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for wallet routes".to_owned())
    })
}

fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

fn ledger_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

fn optional_query_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
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

    fn bearer_token(state: &AppState) -> String {
        issue_token(&state.settings, "user:42", TokenScope::User, 900).unwrap()
    }

    #[tokio::test]
    async fn wallet_accounts_route_requires_user_auth() {
        let app = routes().with_state(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/wallet/accounts")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn wallet_accounts_route_returns_clear_error_without_mysql() {
        let state = test_state();
        let token = bearer_token(&state);
        let app = routes().with_state(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/wallet/accounts")
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
        assert_eq!(
            payload["message"],
            "internal error: mysql pool is not configured for wallet routes"
        );
    }

    #[test]
    fn wallet_ledger_limit_is_clamped() {
        assert_eq!(ledger_limit(None), 50);
        assert_eq!(ledger_limit(Some(0)), 1);
        assert_eq!(ledger_limit(Some(500)), 100);
    }
}
