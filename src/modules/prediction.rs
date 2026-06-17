use crate::{
    error::{AppError, AppResult},
    modules::{
        auth::{AdminAuth, UserAuth},
        wallet::{amount_fits_asset_precision, truncate_amount_to_asset_precision},
    },
    state::AppState,
    time::{option_unix_millis, unix_millis},
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, patch, post},
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, TimeDelta, Utc};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};
use std::{collections::HashSet, str::FromStr, time::Duration};
use tokio::time::sleep;
use uuid::Uuid;

const STATUS_ACTIVE: &str = "active";
const STATUS_HIDDEN: &str = "hidden";
const SETTLEMENT_OPEN: &str = "open";
const SETTLEMENT_PENDING_CONFIRMATION: &str = "pending_confirmation";
const SETTLEMENT_SETTLED: &str = "settled";
const SETTLEMENT_REFUNDED: &str = "refunded";
const ORDER_STATUS_OPEN: &str = "open";
const OUTCOME_YES: &str = "yes";
const OUTCOME_NO: &str = "no";
const OUTCOME_INVALID: &str = "invalid";
const SETTLEMENT_MODE_MANUAL: &str = "manual_confirm";
const SETTLEMENT_MODE_AUTO: &str = "auto";
const REFUND_STAKE_AND_FEE: &str = "refund_stake_and_fee";
const REFUND_STAKE_ONLY: &str = "refund_stake_only";
const REFUND_MANUAL: &str = "manual";
const REF_TYPE_PREDICTION_ORDER: &str = "prediction_order";
const POLYMARKET_GAMMA_EVENTS_URL: &str = "https://gamma-api.polymarket.com/events";
const DEFAULT_SYNC_POLL_SECONDS: u64 = 30;
const DEFAULT_SYNC_LIMIT: &str = "100";

pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/prediction/config", get(get_user_config))
        .route("/prediction/markets", get(list_user_markets))
        .route("/prediction/markets/:id", get(get_user_market))
        .route("/prediction/quotes", post(create_quote))
        .route(
            "/prediction/orders",
            get(list_user_orders).post(create_order),
        )
}

pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/prediction/settings",
            get(get_admin_settings).patch(save_admin_settings),
        )
        .route(
            "/prediction/asset-configs",
            get(list_admin_asset_configs).post(upsert_admin_asset_config),
        )
        .route(
            "/prediction/asset-configs/:asset_id",
            patch(update_admin_asset_config),
        )
        .route("/prediction/markets", get(list_admin_markets))
        .route(
            "/prediction/markets/:id",
            get(get_admin_market).patch(update_admin_market),
        )
        .route("/prediction/markets/:id/settle", post(settle_admin_market))
        .route("/prediction/orders", get(list_admin_orders))
        .route("/prediction/orders/:id", get(get_admin_order))
        .route("/prediction/sync", post(trigger_admin_sync))
        .route("/prediction/sync/logs", get(list_admin_sync_logs))
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminMarketQuery {
    limit: Option<u32>,
    display_status: Option<String>,
    settlement_status: Option<String>,
    keyword: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OrdersQuery {
    limit: Option<u32>,
    status: Option<String>,
    market_id: Option<u64>,
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SavePredictionSettingsRequest {
    sync_enabled: bool,
    sync_interval_seconds: u32,
    sync_tags: Vec<String>,
    allowed_asset_ids: Vec<u64>,
    default_fee_rate: BigDecimal,
    default_settlement_mode: String,
    default_invalid_refund_policy: String,
    quote_ttl_seconds: u32,
}

#[derive(Debug, Deserialize)]
struct UpsertPredictionAssetConfigRequest {
    asset_id: u64,
    enabled: bool,
    max_payout_amount: BigDecimal,
}

#[derive(Debug, Deserialize)]
struct UpdatePredictionAssetConfigRequest {
    enabled: bool,
    max_payout_amount: BigDecimal,
}

#[derive(Debug, Deserialize)]
struct UpdatePredictionMarketRequest {
    display_status: String,
    settlement_mode_override: Option<String>,
    allowed_asset_ids_override: Option<Vec<u64>>,
    payout_cap_overrides: Option<Value>,
    fee_rate_override: Option<BigDecimal>,
}

#[derive(Debug, Deserialize)]
struct CreatePredictionQuoteRequest {
    market_id: u64,
    outcome: String,
    asset_id: u64,
    stake_amount: BigDecimal,
}

#[derive(Debug, Deserialize)]
struct CreatePredictionOrderRequest {
    quote_id: String,
    idempotency_key: String,
}

#[derive(Debug, Deserialize)]
struct SettlePredictionMarketRequest {
    result: String,
    invalid_refund_policy: Option<String>,
}

#[derive(Debug, Serialize)]
struct PredictionSettingsResponse {
    sync_enabled: bool,
    sync_interval_seconds: u32,
    sync_tags: Vec<String>,
    allowed_asset_ids: Vec<u64>,
    default_fee_rate: BigDecimal,
    default_settlement_mode: String,
    default_invalid_refund_policy: String,
    quote_ttl_seconds: u32,
    last_sync_status: Option<String>,
    last_sync_error: Option<String>,
    #[serde(default, with = "option_unix_millis")]
    last_sync_started_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    last_sync_finished_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    last_successful_sync_at: Option<DateTime<Utc>>,
    last_sync_imported_count: u32,
    last_sync_updated_count: u32,
}

#[derive(Debug, Serialize)]
struct PredictionStakeAssetResponse {
    asset_id: u64,
    asset_symbol: String,
    max_payout_amount: BigDecimal,
}

#[derive(Debug, Serialize)]
struct PredictionUserConfigResponse {
    allowed_assets: Vec<PredictionStakeAssetResponse>,
    default_fee_rate: BigDecimal,
    quote_ttl_seconds: u32,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct PredictionSettingsRow {
    sync_enabled: bool,
    sync_interval_seconds: u32,
    sync_tags_json: SqlxJson<Value>,
    allowed_asset_ids_json: SqlxJson<Value>,
    default_fee_rate: BigDecimal,
    default_settlement_mode: String,
    default_invalid_refund_policy: String,
    quote_ttl_seconds: u32,
    last_sync_status: Option<String>,
    last_sync_error: Option<String>,
    last_sync_started_at: Option<DateTime<Utc>>,
    last_sync_finished_at: Option<DateTime<Utc>>,
    last_successful_sync_at: Option<DateTime<Utc>>,
    last_sync_imported_count: u32,
    last_sync_updated_count: u32,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
struct PredictionAssetConfigResponse {
    asset_id: u64,
    asset_symbol: String,
    enabled: bool,
    max_payout_amount: BigDecimal,
    #[serde(with = "unix_millis")]
    created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct PredictionStakeAssetRow {
    asset_id: u64,
    asset_symbol: String,
    max_payout_amount: BigDecimal,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
struct PredictionMarketResponse {
    id: u64,
    source: String,
    external_event_id: Option<String>,
    external_market_id: String,
    slug: Option<String>,
    title: String,
    description: Option<String>,
    image_url: Option<String>,
    category: Option<String>,
    tags_json: SqlxJson<Value>,
    outcome_yes_label: String,
    outcome_no_label: String,
    yes_price: BigDecimal,
    no_price: BigDecimal,
    volume: Option<BigDecimal>,
    liquidity: Option<BigDecimal>,
    #[serde(default, with = "option_unix_millis")]
    end_at: Option<DateTime<Utc>>,
    source_status: String,
    display_status: String,
    external_resolution: Option<String>,
    local_resolution: Option<String>,
    settlement_status: String,
    settlement_mode_override: Option<String>,
    allowed_asset_ids_override_json: Option<SqlxJson<Value>>,
    payout_cap_overrides_json: Option<SqlxJson<Value>>,
    fee_rate_override: Option<BigDecimal>,
    #[serde(default, with = "option_unix_millis")]
    last_synced_at: Option<DateTime<Utc>>,
    #[serde(with = "unix_millis")]
    created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct PredictionMarketsResponse {
    markets: Vec<PredictionMarketResponse>,
}

#[derive(Debug, Serialize)]
struct PredictionAssetConfigsResponse {
    configs: Vec<PredictionAssetConfigResponse>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
struct PredictionQuoteResponse {
    quote_id: String,
    market_id: u64,
    outcome: String,
    asset_id: u64,
    asset_symbol: String,
    stake_amount: BigDecimal,
    fee_amount: BigDecimal,
    accepted_price: BigDecimal,
    shares: BigDecimal,
    theoretical_payout: BigDecimal,
    effective_payout_cap: BigDecimal,
    #[serde(with = "unix_millis")]
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
struct PredictionOrderResponse {
    id: u64,
    order_no: Option<String>,
    user_id: u64,
    user_email: Option<String>,
    market_id: u64,
    market_title: String,
    outcome: String,
    asset_id: u64,
    asset_symbol: String,
    stake_amount: BigDecimal,
    fee_amount: BigDecimal,
    accepted_price: BigDecimal,
    shares: BigDecimal,
    theoretical_payout: BigDecimal,
    effective_payout_cap: BigDecimal,
    status: String,
    result: Option<String>,
    payout_amount: BigDecimal,
    refund_amount: BigDecimal,
    fee_refund_amount: BigDecimal,
    invalid_refund_policy_used: Option<String>,
    #[serde(default, with = "option_unix_millis")]
    settled_at: Option<DateTime<Utc>>,
    #[serde(with = "unix_millis")]
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct PredictionOrdersResponse {
    orders: Vec<PredictionOrderResponse>,
}

#[derive(Debug, Serialize)]
struct PredictionOrderActionResponse {
    order: PredictionOrderResponse,
    changed: bool,
}

#[derive(Debug, Serialize)]
struct PredictionSettlementResponse {
    market: PredictionMarketResponse,
    settled_orders: u32,
    changed: bool,
}

#[derive(Debug, Serialize)]
struct PredictionSyncResponse {
    imported_count: u32,
    updated_count: u32,
    status: String,
    error_message: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct PredictionSyncLogResponse {
    id: u64,
    trigger_type: String,
    status: String,
    imported_count: u32,
    updated_count: u32,
    error_message: Option<String>,
    #[serde(with = "unix_millis")]
    started_at: DateTime<Utc>,
    #[serde(default, with = "option_unix_millis")]
    finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
struct PredictionSyncLogsResponse {
    logs: Vec<PredictionSyncLogResponse>,
}

#[derive(Debug, sqlx::FromRow)]
struct AssetMetaRow {
    symbol: String,
    precision_scale: i32,
    status: String,
}

#[derive(Debug, sqlx::FromRow)]
struct QuoteLockRow {
    quote_id: String,
    user_id: u64,
    market_id: u64,
    outcome: String,
    asset_id: u64,
    stake_amount: BigDecimal,
    fee_amount: BigDecimal,
    accepted_price: BigDecimal,
    shares: BigDecimal,
    theoretical_payout: BigDecimal,
    effective_payout_cap: BigDecimal,
    expires_at: DateTime<Utc>,
    consumed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, sqlx::FromRow)]
struct WalletRow {
    available: BigDecimal,
    frozen: BigDecimal,
    locked: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct OrderSettlementRow {
    id: u64,
    user_id: u64,
    asset_id: u64,
    outcome: String,
    stake_amount: BigDecimal,
    fee_amount: BigDecimal,
    theoretical_payout: BigDecimal,
    effective_payout_cap: BigDecimal,
    status: String,
}

#[derive(Debug, Clone)]
struct EffectiveMarketConfig {
    allowed_asset_ids: Vec<u64>,
    fee_rate: BigDecimal,
    payout_cap_overrides: Option<Value>,
}

#[derive(Debug)]
struct ParsedPolymarketMarket {
    external_event_id: Option<String>,
    external_market_id: String,
    slug: Option<String>,
    title: String,
    description: Option<String>,
    image_url: Option<String>,
    category: Option<String>,
    tags_json: Value,
    outcome_yes_label: String,
    outcome_no_label: String,
    yes_price: BigDecimal,
    no_price: BigDecimal,
    volume: Option<BigDecimal>,
    liquidity: Option<BigDecimal>,
    end_at: Option<DateTime<Utc>>,
    source_status: String,
    external_resolution: Option<String>,
    payload: Value,
}

#[derive(Debug, Default)]
struct SyncCounts {
    imported_count: u32,
    updated_count: u32,
}

pub async fn run_sync_loop(state: AppState) -> AppResult<()> {
    loop {
        if let Err(error) = run_due_sync_once(&state).await {
            tracing::warn!(%error, "prediction market sync tick failed");
        }
        sleep(Duration::from_secs(DEFAULT_SYNC_POLL_SECONDS)).await;
    }
}

async fn run_due_sync_once(state: &AppState) -> AppResult<()> {
    let pool = mysql_pool(state)?;
    let settings = load_settings(&pool).await?;
    if !settings.sync_enabled {
        return Ok(());
    }
    let now = Utc::now();
    if let Some(last_started) = settings.last_sync_started_at {
        let elapsed = now.signed_duration_since(last_started).num_seconds();
        if elapsed >= 0 && elapsed < i64::from(settings.sync_interval_seconds.max(30)) {
            return Ok(());
        }
    }
    sync_polymarket_markets(&pool, "scheduled").await?;
    Ok(())
}

async fn get_admin_settings(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<PredictionSettingsResponse>> {
    Ok(Json(settings_response(
        load_settings(&mysql_pool(&state)?).await?,
    )))
}

async fn save_admin_settings(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<SavePredictionSettingsRequest>,
) -> AppResult<Json<PredictionSettingsResponse>> {
    let pool = mysql_pool(&state)?;
    let settlement_mode = normalize_settlement_mode(&request.default_settlement_mode)?;
    let refund_policy = normalize_invalid_refund_policy(&request.default_invalid_refund_policy)?;
    ensure_non_negative_decimal(&request.default_fee_rate, "default_fee_rate")?;
    if request.sync_interval_seconds < 30 {
        return Err(AppError::Validation(
            "sync_interval_seconds must be at least 30".to_owned(),
        ));
    }
    if request.quote_ttl_seconds == 0 || request.quote_ttl_seconds > 120 {
        return Err(AppError::Validation(
            "quote_ttl_seconds must be between 1 and 120".to_owned(),
        ));
    }
    validate_asset_ids_exist(&pool, &request.allowed_asset_ids).await?;
    let sync_tags = normalize_string_list(request.sync_tags);
    let allowed_asset_ids = unique_u64_list(request.allowed_asset_ids);

    sqlx::query(
        r#"UPDATE prediction_settings
           SET sync_enabled = ?, sync_interval_seconds = ?, sync_tags_json = ?,
               allowed_asset_ids_json = ?, default_fee_rate = ?,
               default_settlement_mode = ?, default_invalid_refund_policy = ?,
               quote_ttl_seconds = ?
           WHERE id = 1"#,
    )
    .bind(request.sync_enabled)
    .bind(request.sync_interval_seconds)
    .bind(SqlxJson(json!(sync_tags)))
    .bind(SqlxJson(json!(allowed_asset_ids)))
    .bind(&request.default_fee_rate)
    .bind(settlement_mode)
    .bind(refund_policy)
    .bind(request.quote_ttl_seconds)
    .execute(&pool)
    .await?;

    Ok(Json(settings_response(load_settings(&pool).await?)))
}

async fn list_admin_asset_configs(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<PredictionAssetConfigsResponse>> {
    let configs = sqlx::query_as::<_, PredictionAssetConfigResponse>(
        r#"SELECT assets.id AS asset_id, assets.symbol AS asset_symbol,
                  COALESCE(configs.enabled, FALSE) AS enabled,
                  COALESCE(configs.max_payout_amount, 0) AS max_payout_amount,
                  COALESCE(configs.created_at, assets.created_at) AS created_at,
                  COALESCE(configs.updated_at, assets.updated_at) AS updated_at
           FROM assets
           LEFT JOIN prediction_asset_configs configs ON configs.asset_id = assets.id
           WHERE assets.status = 'active'
           ORDER BY assets.symbol ASC"#,
    )
    .fetch_all(&mysql_pool(&state)?)
    .await?;
    Ok(Json(PredictionAssetConfigsResponse { configs }))
}

async fn get_user_config(
    State(state): State<AppState>,
) -> AppResult<Json<PredictionUserConfigResponse>> {
    let pool = mysql_pool(&state)?;
    let settings = load_settings(&pool).await?;
    let allowed_ids = json_u64_array(&settings.allowed_asset_ids_json.0);
    if allowed_ids.is_empty() {
        return Ok(Json(PredictionUserConfigResponse {
            allowed_assets: Vec::new(),
            default_fee_rate: settings.default_fee_rate,
            quote_ttl_seconds: settings.quote_ttl_seconds,
        }));
    }
    let allowed_set = allowed_ids.into_iter().collect::<HashSet<_>>();
    let rows = sqlx::query_as::<_, PredictionStakeAssetRow>(
        r#"SELECT configs.asset_id, assets.symbol AS asset_symbol, configs.max_payout_amount
           FROM prediction_asset_configs configs
           INNER JOIN assets ON assets.id = configs.asset_id
           WHERE configs.enabled = TRUE AND assets.status = 'active'
           ORDER BY assets.symbol ASC"#,
    )
    .fetch_all(&pool)
    .await?;
    let allowed_assets = rows
        .into_iter()
        .filter(|row| allowed_set.contains(&row.asset_id))
        .map(|row| PredictionStakeAssetResponse {
            asset_id: row.asset_id,
            asset_symbol: row.asset_symbol,
            max_payout_amount: row.max_payout_amount,
        })
        .collect();
    Ok(Json(PredictionUserConfigResponse {
        allowed_assets,
        default_fee_rate: settings.default_fee_rate,
        quote_ttl_seconds: settings.quote_ttl_seconds,
    }))
}

async fn upsert_admin_asset_config(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<UpsertPredictionAssetConfigRequest>,
) -> AppResult<Json<PredictionAssetConfigResponse>> {
    upsert_asset_config(
        &mysql_pool(&state)?,
        request.asset_id,
        request.enabled,
        request.max_payout_amount,
    )
    .await
    .map(Json)
}

async fn update_admin_asset_config(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(asset_id): Path<u64>,
    Json(request): Json<UpdatePredictionAssetConfigRequest>,
) -> AppResult<Json<PredictionAssetConfigResponse>> {
    upsert_asset_config(
        &mysql_pool(&state)?,
        asset_id,
        request.enabled,
        request.max_payout_amount,
    )
    .await
    .map(Json)
}

async fn list_user_markets(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<PredictionMarketsResponse>> {
    let mut builder = prediction_market_query_builder();
    builder.push(" WHERE markets.display_status = ");
    builder.push_bind(STATUS_ACTIVE);
    builder.push(" AND markets.settlement_status IN ('open', 'pending_confirmation')");
    builder.push(" ORDER BY markets.last_synced_at DESC, markets.id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);
    let markets = builder
        .build_query_as::<PredictionMarketResponse>()
        .fetch_all(&mysql_pool(&state)?)
        .await?;
    Ok(Json(PredictionMarketsResponse { markets }))
}

async fn get_user_market(
    State(state): State<AppState>,
    Path(market_id): Path<u64>,
) -> AppResult<Json<PredictionMarketResponse>> {
    let market = load_market_response(&mysql_pool(&state)?, market_id).await?;
    if market.display_status != STATUS_ACTIVE {
        return Err(AppError::NotFound);
    }
    Ok(Json(market))
}

async fn list_admin_markets(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminMarketQuery>,
) -> AppResult<Json<PredictionMarketsResponse>> {
    let mut builder = prediction_market_query_builder();
    builder.push(" WHERE 1 = 1");
    if let Some(status) = optional_string(query.display_status) {
        builder.push(" AND markets.display_status = ");
        builder.push_bind(status);
    }
    if let Some(status) = optional_string(query.settlement_status) {
        builder.push(" AND markets.settlement_status = ");
        builder.push_bind(status);
    }
    if let Some(keyword) = optional_string(query.keyword) {
        builder.push(" AND markets.title LIKE ");
        builder.push_bind(format!("%{keyword}%"));
    }
    builder.push(" ORDER BY markets.last_synced_at DESC, markets.id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);
    let markets = builder
        .build_query_as::<PredictionMarketResponse>()
        .fetch_all(&mysql_pool(&state)?)
        .await?;
    Ok(Json(PredictionMarketsResponse { markets }))
}

async fn get_admin_market(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(market_id): Path<u64>,
) -> AppResult<Json<PredictionMarketResponse>> {
    Ok(Json(
        load_market_response(&mysql_pool(&state)?, market_id).await?,
    ))
}

async fn update_admin_market(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(market_id): Path<u64>,
    Json(request): Json<UpdatePredictionMarketRequest>,
) -> AppResult<Json<PredictionMarketResponse>> {
    let pool = mysql_pool(&state)?;
    let display_status = normalize_display_status(&request.display_status)?;
    let settlement_mode_override = match request.settlement_mode_override {
        Some(value) if !value.trim().is_empty() => Some(normalize_settlement_mode(&value)?),
        _ => None,
    };
    let allowed_override = request.allowed_asset_ids_override.map(unique_u64_list);
    if let Some(ids) = allowed_override.as_ref() {
        validate_asset_ids_exist(&pool, ids).await?;
    }
    if let Some(rate) = request.fee_rate_override.as_ref() {
        ensure_non_negative_decimal(rate, "fee_rate_override")?;
    }

    let updated = sqlx::query(
        r#"UPDATE prediction_markets
           SET display_status = ?, settlement_mode_override = ?,
               allowed_asset_ids_override_json = ?, payout_cap_overrides_json = ?,
               fee_rate_override = ?
           WHERE id = ?"#,
    )
    .bind(display_status)
    .bind(settlement_mode_override)
    .bind(allowed_override.map(|ids| SqlxJson(json!(ids))))
    .bind(request.payout_cap_overrides.map(SqlxJson))
    .bind(request.fee_rate_override)
    .bind(market_id)
    .execute(&pool)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(load_market_response(&pool, market_id).await?))
}

async fn create_quote(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<CreatePredictionQuoteRequest>,
) -> AppResult<Json<PredictionQuoteResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let quote = create_quote_in_db(&mysql_pool(&state)?, user_id, request).await?;
    Ok(Json(quote))
}

async fn create_order(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<CreatePredictionOrderRequest>,
) -> AppResult<Json<PredictionOrderActionResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let (order, changed) = create_order_in_tx(&mysql_pool(&state)?, user_id, request).await?;
    Ok(Json(PredictionOrderActionResponse { order, changed }))
}

async fn list_user_orders(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<OrdersQuery>,
) -> AppResult<Json<PredictionOrdersResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let mut builder = prediction_order_query_builder();
    builder.push(" WHERE orders.user_id = ");
    builder.push_bind(user_id);
    if let Some(status) = optional_string(query.status) {
        builder.push(" AND orders.status = ");
        builder.push_bind(status);
    }
    if let Some(market_id) = query.market_id {
        builder.push(" AND orders.market_id = ");
        builder.push_bind(market_id);
    }
    builder.push(" ORDER BY orders.created_at DESC, orders.id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);
    let orders = builder
        .build_query_as::<PredictionOrderResponse>()
        .fetch_all(&mysql_pool(&state)?)
        .await?;
    Ok(Json(PredictionOrdersResponse { orders }))
}

async fn list_admin_orders(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<OrdersQuery>,
) -> AppResult<Json<PredictionOrdersResponse>> {
    let mut builder = prediction_order_query_builder();
    builder.push(" WHERE 1 = 1");
    if let Some(status) = optional_string(query.status) {
        builder.push(" AND orders.status = ");
        builder.push_bind(status);
    }
    if let Some(market_id) = query.market_id {
        builder.push(" AND orders.market_id = ");
        builder.push_bind(market_id);
    }
    if let Some(email) = optional_string(query.email) {
        builder.push(" AND users.email LIKE ");
        builder.push_bind(format!("%{email}%"));
    }
    builder.push(" ORDER BY orders.created_at DESC, orders.id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);
    let orders = builder
        .build_query_as::<PredictionOrderResponse>()
        .fetch_all(&mysql_pool(&state)?)
        .await?;
    Ok(Json(PredictionOrdersResponse { orders }))
}

async fn get_admin_order(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<PredictionOrderResponse>> {
    Ok(Json(
        load_order_response(&mysql_pool(&state)?, order_id).await?,
    ))
}

async fn settle_admin_market(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(market_id): Path<u64>,
    Json(request): Json<SettlePredictionMarketRequest>,
) -> AppResult<Json<PredictionSettlementResponse>> {
    let result = normalize_settlement_result(&request.result)?;
    let refund_policy = match request.invalid_refund_policy {
        Some(value) if !value.trim().is_empty() => Some(normalize_invalid_refund_policy(&value)?),
        _ => None,
    };
    let (market, settled_orders, changed) =
        settle_market_in_tx(&mysql_pool(&state)?, market_id, result, refund_policy).await?;
    Ok(Json(PredictionSettlementResponse {
        market,
        settled_orders,
        changed,
    }))
}

async fn trigger_admin_sync(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<PredictionSyncResponse>> {
    let response = sync_polymarket_markets(&mysql_pool(&state)?, "manual").await?;
    Ok(Json(response))
}

async fn list_admin_sync_logs(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<PredictionSyncLogsResponse>> {
    let logs = sqlx::query_as::<_, PredictionSyncLogResponse>(
        r#"SELECT id, trigger_type, status, imported_count, updated_count,
                  error_message, started_at, finished_at
           FROM prediction_sync_logs
           ORDER BY id DESC
           LIMIT ?"#,
    )
    .bind(route_limit(query.limit) as i64)
    .fetch_all(&mysql_pool(&state)?)
    .await?;
    Ok(Json(PredictionSyncLogsResponse { logs }))
}

async fn create_quote_in_db(
    pool: &Pool<MySql>,
    user_id: u64,
    request: CreatePredictionQuoteRequest,
) -> AppResult<PredictionQuoteResponse> {
    let outcome = normalize_binary_outcome(&request.outcome)?;
    ensure_positive_amount(&request.stake_amount, "stake_amount")?;
    let settings = load_settings(pool).await?;
    let market = load_market_response(pool, request.market_id).await?;
    if market.display_status != STATUS_ACTIVE || market.settlement_status != SETTLEMENT_OPEN {
        return Err(AppError::Validation(
            "prediction market is not open for quotes".to_owned(),
        ));
    }
    let asset = load_active_asset(pool, request.asset_id).await?;
    ensure_amount_precision(&request.stake_amount, asset.precision_scale, "stake_amount")?;
    let effective = effective_market_config(&settings, &market);
    if !effective.allowed_asset_ids.contains(&request.asset_id) {
        return Err(AppError::Validation(
            "asset is not allowed for this prediction market".to_owned(),
        ));
    }
    ensure_prediction_asset_enabled(pool, request.asset_id).await?;

    let accepted_price = if outcome == OUTCOME_YES {
        market.yes_price.clone()
    } else {
        market.no_price.clone()
    };
    ensure_probability_price(&accepted_price)?;
    let raw_shares = request.stake_amount.clone() / accepted_price.clone();
    let shares = truncate_amount_to_asset_precision(&raw_shares, asset.precision_scale);
    let theoretical_payout = shares.clone();
    let fee_amount = truncate_amount_to_asset_precision(
        &(request.stake_amount.clone() * effective.fee_rate.clone()),
        asset.precision_scale,
    );
    let effective_payout_cap =
        effective_payout_cap(pool, request.asset_id, &effective.payout_cap_overrides).await?;
    if effective_payout_cap > BigDecimal::from(0) && theoretical_payout > effective_payout_cap {
        return Err(AppError::Validation(
            "prediction quote exceeds configured payout cap".to_owned(),
        ));
    }
    let quote_id = format!("pq_{}", Uuid::now_v7().simple());
    let ttl_seconds = i64::from(settings.quote_ttl_seconds.max(1));
    let expires_at = Utc::now()
        .checked_add_signed(TimeDelta::seconds(ttl_seconds))
        .ok_or_else(|| AppError::Validation("quote expiry is outside valid range".to_owned()))?;

    sqlx::query(
        r#"INSERT INTO prediction_quotes
           (quote_id, user_id, market_id, outcome, asset_id, stake_amount, fee_amount,
            accepted_price, shares, theoretical_payout, effective_payout_cap, expires_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&quote_id)
    .bind(user_id)
    .bind(request.market_id)
    .bind(&outcome)
    .bind(request.asset_id)
    .bind(&request.stake_amount)
    .bind(&fee_amount)
    .bind(&accepted_price)
    .bind(&shares)
    .bind(&theoretical_payout)
    .bind(&effective_payout_cap)
    .bind(expires_at)
    .execute(pool)
    .await?;

    Ok(PredictionQuoteResponse {
        quote_id,
        market_id: request.market_id,
        outcome,
        asset_id: request.asset_id,
        asset_symbol: asset.symbol,
        stake_amount: request.stake_amount,
        fee_amount,
        accepted_price,
        shares,
        theoretical_payout,
        effective_payout_cap,
        expires_at,
    })
}

async fn create_order_in_tx(
    pool: &Pool<MySql>,
    user_id: u64,
    request: CreatePredictionOrderRequest,
) -> AppResult<(PredictionOrderResponse, bool)> {
    let quote_id = required_text(request.quote_id, "quote_id", 64)?;
    let idempotency_key = required_text(request.idempotency_key, "idempotency_key", 128)?;
    if let Some(existing) = load_order_by_idempotency(pool, user_id, &idempotency_key).await? {
        if existing.status.is_empty() {
            return Err(AppError::Conflict(
                "prediction order idempotency key is invalid".to_owned(),
            ));
        }
        return Ok((existing, false));
    }

    let mut tx = pool.begin().await?;
    let quote = lock_quote(&mut tx, &quote_id).await?;
    if quote.user_id != user_id {
        return Err(AppError::Forbidden);
    }
    if quote.consumed_at.is_some() {
        return Err(AppError::Conflict(
            "prediction quote was already used".to_owned(),
        ));
    }
    if quote.expires_at <= Utc::now() {
        return Err(AppError::Validation("prediction quote expired".to_owned()));
    }
    let market = lock_market(&mut tx, quote.market_id).await?;
    if market.display_status != STATUS_ACTIVE || market.settlement_status != SETTLEMENT_OPEN {
        return Err(AppError::Validation(
            "prediction market is not open for orders".to_owned(),
        ));
    }
    let asset = load_active_asset_in_tx(&mut tx, quote.asset_id).await?;
    ensure_amount_precision(&quote.stake_amount, asset.precision_scale, "stake_amount")?;
    ensure_amount_precision(&quote.fee_amount, asset.precision_scale, "fee_amount")?;

    let insert = sqlx::query(
        r#"INSERT INTO prediction_orders
           (user_id, market_id, quote_id, idempotency_key, outcome, asset_id,
            stake_amount, fee_amount, accepted_price, shares, theoretical_payout,
            effective_payout_cap, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'open')"#,
    )
    .bind(user_id)
    .bind(quote.market_id)
    .bind(&quote.quote_id)
    .bind(&idempotency_key)
    .bind(&quote.outcome)
    .bind(quote.asset_id)
    .bind(&quote.stake_amount)
    .bind(&quote.fee_amount)
    .bind(&quote.accepted_price)
    .bind(&quote.shares)
    .bind(&quote.theoretical_payout)
    .bind(&quote.effective_payout_cap)
    .execute(&mut *tx)
    .await;

    let order_id = match insert {
        Ok(result) => result.last_insert_id(),
        Err(error) if is_duplicate_key_error(&error) => {
            tx.rollback().await?;
            let order = load_order_by_idempotency(pool, user_id, &idempotency_key)
                .await?
                .ok_or_else(|| {
                    AppError::Conflict("prediction idempotency key is being committed".to_owned())
                })?;
            return Ok((order, false));
        }
        Err(error) => return Err(AppError::Database(error)),
    };
    let order_no = prediction_order_no(order_id);
    sqlx::query("UPDATE prediction_orders SET order_no = ? WHERE id = ?")
        .bind(&order_no)
        .bind(order_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE prediction_quotes SET consumed_at = CURRENT_TIMESTAMP(6) WHERE quote_id = ?",
    )
    .bind(&quote.quote_id)
    .execute(&mut *tx)
    .await?;
    apply_wallet_prediction_open(
        &mut tx,
        user_id,
        quote.asset_id,
        &quote.stake_amount,
        &quote.fee_amount,
        order_id,
    )
    .await?;
    tx.commit().await?;
    Ok((load_order_response(pool, order_id).await?, true))
}

async fn settle_market_in_tx(
    pool: &Pool<MySql>,
    market_id: u64,
    result: String,
    requested_refund_policy: Option<String>,
) -> AppResult<(PredictionMarketResponse, u32, bool)> {
    let mut tx = pool.begin().await?;
    let market = lock_market(&mut tx, market_id).await?;
    if market.settlement_status == SETTLEMENT_SETTLED
        || market.settlement_status == SETTLEMENT_REFUNDED
    {
        tx.commit().await?;
        return Ok((load_market_response(pool, market_id).await?, 0, false));
    }
    let settings = load_settings_in_tx(&mut tx).await?;
    let refund_policy = if result == OUTCOME_INVALID {
        match requested_refund_policy {
            Some(policy) => policy,
            None => settings.default_invalid_refund_policy.clone(),
        }
    } else {
        settings.default_invalid_refund_policy.clone()
    };
    if result == OUTCOME_INVALID && refund_policy == REFUND_MANUAL {
        return Err(AppError::Validation(
            "manual invalid refund policy requires an explicit concrete refund policy".to_owned(),
        ));
    }
    let orders = sqlx::query_as::<_, OrderSettlementRow>(
        r#"SELECT id, user_id, asset_id, outcome, stake_amount, fee_amount,
                  theoretical_payout, effective_payout_cap, status
           FROM prediction_orders
           WHERE market_id = ? AND status = 'open'
           ORDER BY id ASC
           FOR UPDATE"#,
    )
    .bind(market_id)
    .fetch_all(&mut *tx)
    .await?;

    let mut settled_orders = 0u32;
    for order in orders {
        if order.status != ORDER_STATUS_OPEN {
            continue;
        }
        if result == OUTCOME_INVALID {
            let fee_refund_amount = if refund_policy == REFUND_STAKE_AND_FEE {
                order.fee_amount.clone()
            } else {
                BigDecimal::from(0)
            };
            apply_wallet_prediction_refund(
                &mut tx,
                order.user_id,
                order.asset_id,
                &order.stake_amount,
                &fee_refund_amount,
                order.id,
            )
            .await?;
            sqlx::query(
                r#"UPDATE prediction_orders
                   SET status = 'refunded', result = ?, refund_amount = ?,
                       fee_refund_amount = ?, invalid_refund_policy_used = ?,
                       settled_at = CURRENT_TIMESTAMP(6)
                   WHERE id = ?"#,
            )
            .bind(&result)
            .bind(&order.stake_amount)
            .bind(&fee_refund_amount)
            .bind(&refund_policy)
            .bind(order.id)
            .execute(&mut *tx)
            .await?;
        } else {
            let payout_amount = if order.outcome == result {
                capped_payout(&order.theoretical_payout, &order.effective_payout_cap)
            } else {
                BigDecimal::from(0)
            };
            apply_wallet_prediction_settlement(
                &mut tx,
                order.user_id,
                order.asset_id,
                &order.stake_amount,
                &payout_amount,
                order.id,
                order.outcome == result,
            )
            .await?;
            sqlx::query(
                r#"UPDATE prediction_orders
                   SET status = 'settled', result = ?, payout_amount = ?,
                       settled_at = CURRENT_TIMESTAMP(6)
                   WHERE id = ?"#,
            )
            .bind(&result)
            .bind(&payout_amount)
            .bind(order.id)
            .execute(&mut *tx)
            .await?;
        }
        settled_orders += 1;
    }

    let settlement_status = if result == OUTCOME_INVALID {
        SETTLEMENT_REFUNDED
    } else {
        SETTLEMENT_SETTLED
    };
    let invalid_policy_used = if result == OUTCOME_INVALID {
        Some(refund_policy.clone())
    } else {
        None
    };
    sqlx::query(
        r#"UPDATE prediction_markets
           SET local_resolution = ?, settlement_status = ?,
               invalid_refund_policy_used = ?
           WHERE id = ?"#,
    )
    .bind(&result)
    .bind(settlement_status)
    .bind(invalid_policy_used)
    .bind(market_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((
        load_market_response(pool, market_id).await?,
        settled_orders,
        true,
    ))
}

async fn sync_polymarket_markets(
    pool: &Pool<MySql>,
    trigger_type: &str,
) -> AppResult<PredictionSyncResponse> {
    let started_at = Utc::now();
    let log_id = sqlx::query(
        r#"INSERT INTO prediction_sync_logs (trigger_type, status, started_at)
           VALUES (?, 'running', ?)"#,
    )
    .bind(trigger_type)
    .bind(started_at)
    .execute(pool)
    .await?
    .last_insert_id();
    sqlx::query(
        r#"UPDATE prediction_settings
           SET last_sync_status = 'running',
               last_sync_error = NULL,
               last_sync_started_at = ?
           WHERE id = 1"#,
    )
    .bind(started_at)
    .execute(pool)
    .await?;

    let result = sync_polymarket_markets_inner(pool).await;
    let finished_at = Utc::now();
    match result {
        Ok(counts) => {
            sqlx::query(
                r#"UPDATE prediction_sync_logs
                   SET status = 'success', imported_count = ?, updated_count = ?,
                       finished_at = ?
                   WHERE id = ?"#,
            )
            .bind(counts.imported_count)
            .bind(counts.updated_count)
            .bind(finished_at)
            .bind(log_id)
            .execute(pool)
            .await?;
            sqlx::query(
                r#"UPDATE prediction_settings
                   SET last_sync_status = 'success', last_sync_error = NULL,
                       last_sync_finished_at = ?, last_successful_sync_at = ?,
                       last_sync_imported_count = ?, last_sync_updated_count = ?
                   WHERE id = 1"#,
            )
            .bind(finished_at)
            .bind(finished_at)
            .bind(counts.imported_count)
            .bind(counts.updated_count)
            .execute(pool)
            .await?;
            Ok(PredictionSyncResponse {
                imported_count: counts.imported_count,
                updated_count: counts.updated_count,
                status: "success".to_owned(),
                error_message: None,
            })
        }
        Err(error) => {
            let message = compact_error_message(&error.to_string());
            sqlx::query(
                r#"UPDATE prediction_sync_logs
                   SET status = 'failed', error_message = ?, finished_at = ?
                   WHERE id = ?"#,
            )
            .bind(&message)
            .bind(finished_at)
            .bind(log_id)
            .execute(pool)
            .await?;
            sqlx::query(
                r#"UPDATE prediction_settings
                   SET last_sync_status = 'failed', last_sync_error = ?,
                       last_sync_finished_at = ?
                   WHERE id = 1"#,
            )
            .bind(&message)
            .bind(finished_at)
            .execute(pool)
            .await?;
            Err(error)
        }
    }
}

async fn sync_polymarket_markets_inner(pool: &Pool<MySql>) -> AppResult<SyncCounts> {
    let settings = load_settings(pool).await?;
    let tags = json_string_array(&settings.sync_tags_json.0);
    let remote_markets = fetch_polymarket_markets(&tags).await?;
    let parsed_markets = remote_markets
        .iter()
        .filter_map(|value| parse_polymarket_market(value).ok())
        .collect::<Vec<_>>();
    let mut counts = SyncCounts::default();
    for market in parsed_markets {
        let result = sqlx::query(
            r#"INSERT INTO prediction_markets
               (source, external_event_id, external_market_id, slug, title, description,
                image_url, category, tags_json, outcome_yes_label, outcome_no_label,
                yes_price, no_price, volume, liquidity, end_at, source_status,
                display_status, external_resolution, settlement_status, sync_payload_json,
                last_synced_at)
               VALUES ('polymarket', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, 'open', ?, CURRENT_TIMESTAMP(6))
               ON DUPLICATE KEY UPDATE
                   external_event_id = VALUES(external_event_id),
                   slug = VALUES(slug),
                   title = VALUES(title),
                   description = VALUES(description),
                   image_url = VALUES(image_url),
                   category = VALUES(category),
                   tags_json = VALUES(tags_json),
                   outcome_yes_label = VALUES(outcome_yes_label),
                   outcome_no_label = VALUES(outcome_no_label),
                   yes_price = VALUES(yes_price),
                   no_price = VALUES(no_price),
                   volume = VALUES(volume),
                   liquidity = VALUES(liquidity),
                   end_at = VALUES(end_at),
                   source_status = VALUES(source_status),
                   external_resolution = VALUES(external_resolution),
                   sync_payload_json = VALUES(sync_payload_json),
                   last_synced_at = CURRENT_TIMESTAMP(6)"#,
        )
        .bind(&market.external_event_id)
        .bind(&market.external_market_id)
        .bind(&market.slug)
        .bind(&market.title)
        .bind(&market.description)
        .bind(&market.image_url)
        .bind(&market.category)
        .bind(SqlxJson(market.tags_json))
        .bind(&market.outcome_yes_label)
        .bind(&market.outcome_no_label)
        .bind(&market.yes_price)
        .bind(&market.no_price)
        .bind(&market.volume)
        .bind(&market.liquidity)
        .bind(market.end_at)
        .bind(&market.source_status)
        .bind(&market.external_resolution)
        .bind(SqlxJson(market.payload))
        .execute(pool)
        .await?;
        let is_insert = result.last_insert_id() > 0;
        if is_insert {
            counts.imported_count += 1;
        } else {
            counts.updated_count += 1;
        }
        reconcile_synced_resolution(
            pool,
            &settings,
            &market.external_market_id,
            &market.external_resolution,
        )
        .await?;
    }
    Ok(counts)
}

async fn reconcile_synced_resolution(
    pool: &Pool<MySql>,
    settings: &PredictionSettingsRow,
    external_market_id: &str,
    external_resolution: &Option<String>,
) -> AppResult<()> {
    let Some(result) = external_resolution.as_ref() else {
        return Ok(());
    };
    let market = load_market_by_source_external(pool, "polymarket", external_market_id).await?;
    if market.local_resolution.is_some()
        || market.settlement_status == SETTLEMENT_SETTLED
        || market.settlement_status == SETTLEMENT_REFUNDED
    {
        return Ok(());
    }

    let settlement_mode = market
        .settlement_mode_override
        .clone()
        .unwrap_or_else(|| settings.default_settlement_mode.clone());
    let invalid_requires_manual_policy =
        result == OUTCOME_INVALID && settings.default_invalid_refund_policy == REFUND_MANUAL;
    if settlement_mode == SETTLEMENT_MODE_AUTO && !invalid_requires_manual_policy {
        settle_market_in_tx(pool, market.id, result.clone(), None).await?;
        return Ok(());
    }

    if market.settlement_status == SETTLEMENT_OPEN {
        sqlx::query(
            "UPDATE prediction_markets SET settlement_status = ? WHERE id = ? AND settlement_status = ?",
        )
        .bind(SETTLEMENT_PENDING_CONFIRMATION)
        .bind(market.id)
        .bind(SETTLEMENT_OPEN)
        .execute(pool)
        .await?;
    }
    Ok(())
}

async fn fetch_polymarket_markets(tags: &[String]) -> AppResult<Vec<Value>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("rust-chain-prediction-sync/1.0")
        .build()
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let tags_to_fetch = if tags.is_empty() {
        vec![String::new()]
    } else {
        tags.to_vec()
    };
    let mut values = Vec::new();
    for tag in tags_to_fetch {
        let mut params = vec![
            ("active".to_owned(), "true".to_owned()),
            ("closed".to_owned(), "false".to_owned()),
            ("limit".to_owned(), DEFAULT_SYNC_LIMIT.to_owned()),
        ];
        if !tag.is_empty() {
            if tag.chars().all(|ch| ch.is_ascii_digit()) {
                params.push(("tag_id".to_owned(), tag));
            } else {
                params.push(("tag_slug".to_owned(), tag));
            }
        }
        let url = Url::parse_with_params(POLYMARKET_GAMMA_EVENTS_URL, &params)
            .map_err(|error| AppError::Internal(error.to_string()))?;
        let response = client
            .get(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|error| upstream_sync_error(error.to_string()))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| upstream_sync_error(error.to_string()))?;
        if !status.is_success() {
            return Err(upstream_sync_error(format!(
                "polymarket returned status {status}: {}",
                compact_error_message(&body)
            )));
        }
        let payload: Value = serde_json::from_str(&body).map_err(|error| {
            upstream_sync_error(format!("polymarket returned invalid json: {error}"))
        })?;
        values.extend(extract_market_values(payload));
    }
    Ok(values)
}

fn parse_polymarket_market(value: &Value) -> AppResult<ParsedPolymarketMarket> {
    let external_market_id = first_string(value, &["id", "conditionId", "questionID"])
        .ok_or_else(|| AppError::Validation("polymarket market id is missing".to_owned()))?;
    let external_event_id = first_string(value, &["eventId", "event_id", "groupItemTitle"]);
    let title = first_string(value, &["question", "title", "name"])
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| AppError::Validation("polymarket market title is missing".to_owned()))?;
    let outcome_labels = json_string_array(
        &first_jsonish_value(value, &["outcomes", "tokens"]).unwrap_or(Value::Null),
    );
    let outcome_yes_label = outcome_labels
        .first()
        .cloned()
        .unwrap_or_else(|| "Yes".to_owned());
    let outcome_no_label = outcome_labels
        .get(1)
        .cloned()
        .unwrap_or_else(|| "No".to_owned());
    let prices = json_decimal_array(
        &first_jsonish_value(value, &["outcomePrices", "prices"]).unwrap_or(Value::Null),
    );
    let yes_price = prices
        .first()
        .cloned()
        .unwrap_or_else(|| decimal_str("0.5"));
    let no_price = prices
        .get(1)
        .cloned()
        .unwrap_or_else(|| decimal_str("1") - yes_price.clone());
    let source_status = if bool_field(value, "closed") || bool_field(value, "archived") {
        "closed".to_owned()
    } else {
        STATUS_ACTIVE.to_owned()
    };
    let external_resolution =
        first_string(value, &["resolutionOutcome", "outcome", "resolvedOutcome"])
            .and_then(|outcome| normalize_external_resolution(&outcome));

    Ok(ParsedPolymarketMarket {
        external_event_id,
        external_market_id,
        slug: first_string(value, &["slug"]),
        title,
        description: first_string(value, &["description"]),
        image_url: first_string(value, &["image", "icon", "imageUrl"]),
        category: first_string(value, &["category", "categorySlug"]),
        tags_json: first_jsonish_value(value, &["tags"]).unwrap_or_else(|| json!([])),
        outcome_yes_label,
        outcome_no_label,
        yes_price: clamp_probability(yes_price),
        no_price: clamp_probability(no_price),
        volume: first_decimal(value, &["volume", "volumeNum", "volume24hr"]),
        liquidity: first_decimal(value, &["liquidity", "liquidityNum"]),
        end_at: first_string(value, &["endDate", "end_date"])
            .and_then(|text| parse_datetime(&text)),
        source_status,
        external_resolution,
        payload: value.clone(),
    })
}

fn prediction_market_query_builder() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT markets.id, markets.source, markets.external_event_id, markets.external_market_id,
                  markets.slug, markets.title, markets.description, markets.image_url,
                  markets.category, markets.tags_json, markets.outcome_yes_label,
                  markets.outcome_no_label, markets.yes_price, markets.no_price,
                  markets.volume, markets.liquidity, markets.end_at, markets.source_status,
                  markets.display_status, markets.external_resolution, markets.local_resolution,
                  markets.settlement_status, markets.settlement_mode_override,
                  markets.allowed_asset_ids_override_json, markets.payout_cap_overrides_json,
                  markets.fee_rate_override, markets.last_synced_at,
                  markets.created_at, markets.updated_at
           FROM prediction_markets markets"#,
    )
}

fn prediction_order_query_builder() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT orders.id, orders.order_no, orders.user_id, users.email AS user_email,
                  orders.market_id, markets.title AS market_title, orders.outcome,
                  orders.asset_id, assets.symbol AS asset_symbol, orders.stake_amount,
                  orders.fee_amount, orders.accepted_price, orders.shares,
                  orders.theoretical_payout, orders.effective_payout_cap,
                  orders.status, orders.result, orders.payout_amount, orders.refund_amount,
                  orders.fee_refund_amount, orders.invalid_refund_policy_used,
                  orders.settled_at, orders.created_at
           FROM prediction_orders orders
           INNER JOIN users ON users.id = orders.user_id
           INNER JOIN prediction_markets markets ON markets.id = orders.market_id
           INNER JOIN assets ON assets.id = orders.asset_id"#,
    )
}

async fn load_settings(pool: &Pool<MySql>) -> AppResult<PredictionSettingsRow> {
    sqlx::query_as::<_, PredictionSettingsRow>(
        r#"SELECT sync_enabled, sync_interval_seconds, sync_tags_json, allowed_asset_ids_json,
                  default_fee_rate, default_settlement_mode, default_invalid_refund_policy,
                  quote_ttl_seconds, last_sync_status, last_sync_error,
                  last_sync_started_at, last_sync_finished_at, last_successful_sync_at,
                  last_sync_imported_count, last_sync_updated_count
           FROM prediction_settings
           WHERE id = 1"#,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Internal("prediction settings are missing".to_owned()))
}

async fn load_settings_in_tx(tx: &mut Transaction<'_, MySql>) -> AppResult<PredictionSettingsRow> {
    sqlx::query_as::<_, PredictionSettingsRow>(
        r#"SELECT sync_enabled, sync_interval_seconds, sync_tags_json, allowed_asset_ids_json,
                  default_fee_rate, default_settlement_mode, default_invalid_refund_policy,
                  quote_ttl_seconds, last_sync_status, last_sync_error,
                  last_sync_started_at, last_sync_finished_at, last_successful_sync_at,
                  last_sync_imported_count, last_sync_updated_count
           FROM prediction_settings
           WHERE id = 1
           FOR UPDATE"#,
    )
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Internal("prediction settings are missing".to_owned()))
}

fn settings_response(row: PredictionSettingsRow) -> PredictionSettingsResponse {
    PredictionSettingsResponse {
        sync_enabled: row.sync_enabled,
        sync_interval_seconds: row.sync_interval_seconds,
        sync_tags: json_string_array(&row.sync_tags_json.0),
        allowed_asset_ids: json_u64_array(&row.allowed_asset_ids_json.0),
        default_fee_rate: row.default_fee_rate,
        default_settlement_mode: row.default_settlement_mode,
        default_invalid_refund_policy: row.default_invalid_refund_policy,
        quote_ttl_seconds: row.quote_ttl_seconds,
        last_sync_status: row.last_sync_status,
        last_sync_error: row.last_sync_error,
        last_sync_started_at: row.last_sync_started_at,
        last_sync_finished_at: row.last_sync_finished_at,
        last_successful_sync_at: row.last_successful_sync_at,
        last_sync_imported_count: row.last_sync_imported_count,
        last_sync_updated_count: row.last_sync_updated_count,
    }
}

async fn upsert_asset_config(
    pool: &Pool<MySql>,
    asset_id: u64,
    enabled: bool,
    max_payout_amount: BigDecimal,
) -> AppResult<PredictionAssetConfigResponse> {
    ensure_non_negative_decimal(&max_payout_amount, "max_payout_amount")?;
    load_active_asset(pool, asset_id).await?;
    sqlx::query(
        r#"INSERT INTO prediction_asset_configs (asset_id, enabled, max_payout_amount)
           VALUES (?, ?, ?)
           ON DUPLICATE KEY UPDATE enabled = VALUES(enabled),
                                   max_payout_amount = VALUES(max_payout_amount)"#,
    )
    .bind(asset_id)
    .bind(enabled)
    .bind(&max_payout_amount)
    .execute(pool)
    .await?;
    sqlx::query_as::<_, PredictionAssetConfigResponse>(
        r#"SELECT configs.asset_id, assets.symbol AS asset_symbol, configs.enabled,
                  configs.max_payout_amount, configs.created_at, configs.updated_at
           FROM prediction_asset_configs configs
           INNER JOIN assets ON assets.id = configs.asset_id
           WHERE configs.asset_id = ?"#,
    )
    .bind(asset_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)
}

async fn load_market_response(
    pool: &Pool<MySql>,
    market_id: u64,
) -> AppResult<PredictionMarketResponse> {
    let mut builder = prediction_market_query_builder();
    builder.push(" WHERE markets.id = ");
    builder.push_bind(market_id);
    builder
        .build_query_as::<PredictionMarketResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

async fn load_market_by_source_external(
    pool: &Pool<MySql>,
    source: &str,
    external_market_id: &str,
) -> AppResult<PredictionMarketResponse> {
    let mut builder = prediction_market_query_builder();
    builder.push(" WHERE markets.source = ");
    builder.push_bind(source.to_owned());
    builder.push(" AND markets.external_market_id = ");
    builder.push_bind(external_market_id.to_owned());
    builder
        .build_query_as::<PredictionMarketResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

async fn load_order_response(
    pool: &Pool<MySql>,
    order_id: u64,
) -> AppResult<PredictionOrderResponse> {
    let mut builder = prediction_order_query_builder();
    builder.push(" WHERE orders.id = ");
    builder.push_bind(order_id);
    builder
        .build_query_as::<PredictionOrderResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

async fn load_order_by_idempotency(
    pool: &Pool<MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<PredictionOrderResponse>> {
    let mut builder = prediction_order_query_builder();
    builder.push(" WHERE orders.user_id = ");
    builder.push_bind(user_id);
    builder.push(" AND orders.idempotency_key = ");
    builder.push_bind(idempotency_key.to_owned());
    Ok(builder
        .build_query_as::<PredictionOrderResponse>()
        .fetch_optional(pool)
        .await?)
}

async fn lock_quote(tx: &mut Transaction<'_, MySql>, quote_id: &str) -> AppResult<QuoteLockRow> {
    sqlx::query_as::<_, QuoteLockRow>(
        r#"SELECT quote_id, user_id, market_id, outcome, asset_id, stake_amount,
                  fee_amount, accepted_price, shares, theoretical_payout,
                  effective_payout_cap, expires_at, consumed_at
           FROM prediction_quotes
           WHERE quote_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(quote_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

async fn lock_market(
    tx: &mut Transaction<'_, MySql>,
    market_id: u64,
) -> AppResult<PredictionMarketResponse> {
    let market = sqlx::query_as::<_, PredictionMarketResponse>(
        r#"SELECT id, source, external_event_id, external_market_id, slug, title, description,
                  image_url, category, tags_json, outcome_yes_label, outcome_no_label,
                  yes_price, no_price, volume, liquidity, end_at, source_status,
                  display_status, external_resolution, local_resolution, settlement_status,
                  settlement_mode_override, allowed_asset_ids_override_json,
                  payout_cap_overrides_json, fee_rate_override, last_synced_at,
                  created_at, updated_at
           FROM prediction_markets
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(market_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(market)
}

async fn load_active_asset(pool: &Pool<MySql>, asset_id: u64) -> AppResult<AssetMetaRow> {
    let asset = sqlx::query_as::<_, AssetMetaRow>(
        "SELECT symbol, precision_scale, status FROM assets WHERE id = ? LIMIT 1",
    )
    .bind(asset_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    if asset.status != STATUS_ACTIVE {
        return Err(AppError::Validation("asset must be active".to_owned()));
    }
    Ok(asset)
}

async fn load_active_asset_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<AssetMetaRow> {
    let asset = sqlx::query_as::<_, AssetMetaRow>(
        "SELECT symbol, precision_scale, status FROM assets WHERE id = ? LIMIT 1",
    )
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if asset.status != STATUS_ACTIVE {
        return Err(AppError::Validation("asset must be active".to_owned()));
    }
    Ok(asset)
}

async fn ensure_prediction_asset_enabled(pool: &Pool<MySql>, asset_id: u64) -> AppResult<()> {
    let enabled = sqlx::query_as::<_, (bool,)>(
        "SELECT enabled FROM prediction_asset_configs WHERE asset_id = ? LIMIT 1",
    )
    .bind(asset_id)
    .fetch_optional(pool)
    .await?
    .map(|row| row.0)
    .unwrap_or(false);
    if !enabled {
        return Err(AppError::Validation(
            "asset is not enabled for prediction betting".to_owned(),
        ));
    }
    Ok(())
}

fn effective_market_config(
    settings: &PredictionSettingsRow,
    market: &PredictionMarketResponse,
) -> EffectiveMarketConfig {
    let allowed_asset_ids = market
        .allowed_asset_ids_override_json
        .as_ref()
        .map(|value| json_u64_array(&value.0))
        .filter(|ids| !ids.is_empty())
        .unwrap_or_else(|| json_u64_array(&settings.allowed_asset_ids_json.0));
    let fee_rate = market
        .fee_rate_override
        .clone()
        .unwrap_or_else(|| settings.default_fee_rate.clone());
    let payout_cap_overrides = market
        .payout_cap_overrides_json
        .as_ref()
        .map(|value| value.0.clone());
    EffectiveMarketConfig {
        allowed_asset_ids,
        fee_rate,
        payout_cap_overrides,
    }
}

async fn effective_payout_cap(
    pool: &Pool<MySql>,
    asset_id: u64,
    overrides: &Option<Value>,
) -> AppResult<BigDecimal> {
    let asset_key = asset_id.to_string();
    if let Some(value) = overrides
        && let Some(cap) = value.get(asset_key.as_str()).and_then(decimal_from_json)
    {
        return Ok(cap);
    }
    let cap = sqlx::query_as::<_, (BigDecimal,)>(
        "SELECT max_payout_amount FROM prediction_asset_configs WHERE asset_id = ? LIMIT 1",
    )
    .bind(asset_id)
    .fetch_optional(pool)
    .await?
    .map(|row| row.0)
    .unwrap_or_else(|| BigDecimal::from(0));
    Ok(cap)
}

async fn validate_asset_ids_exist(pool: &Pool<MySql>, ids: &[u64]) -> AppResult<()> {
    for id in unique_u64_list(ids.to_vec()) {
        load_active_asset(pool, id).await?;
    }
    Ok(())
}

async fn apply_wallet_prediction_open(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    stake_amount: &BigDecimal,
    fee_amount: &BigDecimal,
    order_id: u64,
) -> AppResult<()> {
    let wallet = lock_or_create_wallet_row(tx, user_id, asset_id).await?;
    let total_required = stake_amount.clone() + fee_amount.clone();
    if wallet.available < total_required {
        return Err(AppError::Validation(format!(
            "insufficient available balance for prediction order: requested {}, available {}",
            stake_amount.clone() + fee_amount.clone(),
            wallet.available
        )));
    }
    let available_after_stake = wallet.available.clone() - stake_amount.clone();
    let frozen_after = wallet.frozen.clone() + stake_amount.clone();
    let available_after_fee = available_after_stake.clone() - fee_amount.clone();
    sqlx::query(
        "UPDATE wallet_accounts SET available = ?, frozen = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&available_after_fee)
    .bind(&frozen_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        -stake_amount.clone(),
        "available",
        &available_after_stake,
        &available_after_stake,
        &frozen_after,
        &wallet.locked,
        "prediction_stake_freeze",
        order_id,
    )
    .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        stake_amount.clone(),
        "frozen",
        &frozen_after,
        &available_after_stake,
        &frozen_after,
        &wallet.locked,
        "prediction_stake_freeze",
        order_id,
    )
    .await?;
    if fee_amount > &BigDecimal::from(0) {
        insert_wallet_ledger(
            tx,
            user_id,
            asset_id,
            -fee_amount.clone(),
            "available",
            &available_after_fee,
            &available_after_fee,
            &frozen_after,
            &wallet.locked,
            "prediction_fee",
            order_id,
        )
        .await?;
    }
    Ok(())
}

async fn apply_wallet_prediction_settlement(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    stake_amount: &BigDecimal,
    payout_amount: &BigDecimal,
    order_id: u64,
    won: bool,
) -> AppResult<()> {
    let wallet = lock_or_create_wallet_row(tx, user_id, asset_id).await?;
    if wallet.frozen < *stake_amount {
        return Err(AppError::Validation(format!(
            "insufficient frozen balance for prediction settlement: requested {}, frozen {}",
            stake_amount, wallet.frozen
        )));
    }
    let frozen_after = wallet.frozen.clone() - stake_amount.clone();
    let available_after = wallet.available.clone() + payout_amount.clone();
    sqlx::query(
        "UPDATE wallet_accounts SET available = ?, frozen = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&available_after)
    .bind(&frozen_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        -stake_amount.clone(),
        "frozen",
        &frozen_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
        if won {
            "prediction_settle_win"
        } else {
            "prediction_settle_loss"
        },
        order_id,
    )
    .await?;
    if payout_amount > &BigDecimal::from(0) {
        insert_wallet_ledger(
            tx,
            user_id,
            asset_id,
            payout_amount.clone(),
            "available",
            &available_after,
            &available_after,
            &frozen_after,
            &wallet.locked,
            "prediction_payout",
            order_id,
        )
        .await?;
    }
    Ok(())
}

async fn apply_wallet_prediction_refund(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    stake_amount: &BigDecimal,
    fee_refund_amount: &BigDecimal,
    order_id: u64,
) -> AppResult<()> {
    let wallet = lock_or_create_wallet_row(tx, user_id, asset_id).await?;
    if wallet.frozen < *stake_amount {
        return Err(AppError::Validation(format!(
            "insufficient frozen balance for prediction refund: requested {}, frozen {}",
            stake_amount, wallet.frozen
        )));
    }
    let available_after_stake = wallet.available.clone() + stake_amount.clone();
    let frozen_after = wallet.frozen.clone() - stake_amount.clone();
    let available_after_fee = available_after_stake.clone() + fee_refund_amount.clone();
    sqlx::query(
        "UPDATE wallet_accounts SET available = ?, frozen = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&available_after_fee)
    .bind(&frozen_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        stake_amount.clone(),
        "available",
        &available_after_stake,
        &available_after_stake,
        &frozen_after,
        &wallet.locked,
        "prediction_stake_refund",
        order_id,
    )
    .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        -stake_amount.clone(),
        "frozen",
        &frozen_after,
        &available_after_stake,
        &frozen_after,
        &wallet.locked,
        "prediction_stake_refund",
        order_id,
    )
    .await?;
    if fee_refund_amount > &BigDecimal::from(0) {
        insert_wallet_ledger(
            tx,
            user_id,
            asset_id,
            fee_refund_amount.clone(),
            "available",
            &available_after_fee,
            &available_after_fee,
            &frozen_after,
            &wallet.locked,
            "prediction_fee_refund",
            order_id,
        )
        .await?;
    }
    Ok(())
}

async fn lock_or_create_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<WalletRow> {
    sqlx::query(
        r#"INSERT IGNORE INTO wallet_accounts (user_id, asset_id, available, frozen, locked)
           VALUES (?, ?, 0, 0, 0)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query_as::<_, WalletRow>(
        r#"SELECT available, frozen, locked
           FROM wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Validation("wallet account is required".to_owned()))
}

#[allow(clippy::too_many_arguments)]
async fn insert_wallet_ledger(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: BigDecimal,
    balance_type: &str,
    balance_after: &BigDecimal,
    available_after: &BigDecimal,
    frozen_after: &BigDecimal,
    locked_after: &BigDecimal,
    change_type: &str,
    order_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(change_type)
    .bind(amount)
    .bind(balance_type)
    .bind(balance_after)
    .bind(available_after)
    .bind(frozen_after)
    .bind(locked_after)
    .bind(REF_TYPE_PREDICTION_ORDER)
    .bind(order_id.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 200)
}

fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_owned())
        .filter(|item| !item.is_empty())
}

fn required_text(value: String, field: &str, max_len: usize) -> AppResult<String> {
    let normalized = value.trim().to_owned();
    if normalized.is_empty() {
        return Err(AppError::Validation(format!("{field} is required")));
    }
    if normalized.len() > max_len {
        return Err(AppError::Validation(format!("{field} is too long")));
    }
    Ok(normalized)
}

fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject.parse::<u64>().map_err(|_| AppError::Unauthorized)
}

fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state
        .mysql
        .clone()
        .ok_or_else(|| AppError::Internal("mysql pool is not configured".to_owned()))
}

fn ensure_positive_amount(amount: &BigDecimal, field: &str) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        return Err(AppError::Validation(format!("{field} must be positive")));
    }
    Ok(())
}

fn ensure_non_negative_decimal(value: &BigDecimal, field: &str) -> AppResult<()> {
    if value < &BigDecimal::from(0) {
        return Err(AppError::Validation(format!(
            "{field} must not be negative"
        )));
    }
    Ok(())
}

fn ensure_amount_precision(
    amount: &BigDecimal,
    precision_scale: i32,
    field: &str,
) -> AppResult<()> {
    if !amount_fits_asset_precision(amount, precision_scale) {
        return Err(AppError::Validation(format!(
            "{field} exceeds asset precision scale {precision_scale}"
        )));
    }
    Ok(())
}

fn ensure_probability_price(price: &BigDecimal) -> AppResult<()> {
    if price <= &BigDecimal::from(0) || price >= &BigDecimal::from(1) {
        return Err(AppError::Validation(
            "prediction probability price must be between 0 and 1".to_owned(),
        ));
    }
    Ok(())
}

fn normalize_binary_outcome(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" => Ok(OUTCOME_YES.to_owned()),
        "no" => Ok(OUTCOME_NO.to_owned()),
        _ => Err(AppError::Validation(
            "prediction outcome must be yes or no".to_owned(),
        )),
    }
}

fn normalize_settlement_result(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" => Ok(OUTCOME_YES.to_owned()),
        "no" => Ok(OUTCOME_NO.to_owned()),
        "invalid" | "cancelled" | "canceled" => Ok(OUTCOME_INVALID.to_owned()),
        _ => Err(AppError::Validation(
            "prediction settlement result must be yes, no, or invalid".to_owned(),
        )),
    }
}

fn normalize_settlement_mode(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        SETTLEMENT_MODE_MANUAL => Ok(SETTLEMENT_MODE_MANUAL.to_owned()),
        SETTLEMENT_MODE_AUTO => Ok(SETTLEMENT_MODE_AUTO.to_owned()),
        _ => Err(AppError::Validation(
            "settlement mode must be manual_confirm or auto".to_owned(),
        )),
    }
}

fn normalize_invalid_refund_policy(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        REFUND_STAKE_AND_FEE => Ok(REFUND_STAKE_AND_FEE.to_owned()),
        REFUND_STAKE_ONLY => Ok(REFUND_STAKE_ONLY.to_owned()),
        REFUND_MANUAL => Ok(REFUND_MANUAL.to_owned()),
        _ => Err(AppError::Validation(
            "invalid refund policy is unsupported".to_owned(),
        )),
    }
}

fn normalize_display_status(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        STATUS_ACTIVE => Ok(STATUS_ACTIVE.to_owned()),
        STATUS_HIDDEN => Ok(STATUS_HIDDEN.to_owned()),
        _ => Err(AppError::Validation(
            "display_status must be active or hidden".to_owned(),
        )),
    }
}

fn normalize_external_resolution(value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" => Some(OUTCOME_YES.to_owned()),
        "no" => Some(OUTCOME_NO.to_owned()),
        "invalid" | "canceled" | "cancelled" => Some(OUTCOME_INVALID.to_owned()),
        _ => None,
    }
}

fn unique_u64_list(values: Vec<u64>) -> Vec<u64> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter(|value| *value > 0 && seen.insert(*value))
        .collect()
}

fn normalize_string_list(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty() && seen.insert(value.clone()))
        .collect()
}

fn json_u64_array(value: &Value) -> Vec<u64> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_u64().or_else(|| item.as_str()?.parse::<u64>().ok()))
                .collect()
        })
        .unwrap_or_default()
}

fn json_string_array(value: &Value) -> Vec<String> {
    let parsed = match value {
        Value::String(text) => {
            serde_json::from_str::<Value>(text).unwrap_or_else(|_| json!([text]))
        }
        other => other.clone(),
    };
    parsed
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    if let Some(text) = item.as_str() {
                        Some(text.to_owned())
                    } else {
                        item.get("label")
                            .or_else(|| item.get("name"))
                            .and_then(Value::as_str)
                            .map(str::to_owned)
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn json_decimal_array(value: &Value) -> Vec<BigDecimal> {
    let parsed = match value {
        Value::String(text) => serde_json::from_str::<Value>(text).unwrap_or(Value::Null),
        other => other.clone(),
    };
    parsed
        .as_array()
        .map(|items| items.iter().filter_map(decimal_from_json).collect())
        .unwrap_or_default()
}

fn decimal_from_json(value: &Value) -> Option<BigDecimal> {
    match value {
        Value::Number(number) => BigDecimal::from_str(&number.to_string()).ok(),
        Value::String(text) => BigDecimal::from_str(text.trim()).ok(),
        _ => None,
    }
}

fn first_jsonish_value(value: &Value, keys: &[&str]) -> Option<Value> {
    for key in keys {
        let Some(candidate) = value.get(*key) else {
            continue;
        };
        if let Some(text) = candidate.as_str()
            && let Ok(parsed) = serde_json::from_str::<Value>(text)
        {
            return Some(parsed);
        }
        return Some(candidate.clone());
    }
    None
}

fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        let Some(candidate) = value.get(*key) else {
            continue;
        };
        if let Some(text) = candidate.as_str() {
            return Some(text.to_owned());
        }
        if candidate.is_number() || candidate.is_boolean() {
            return Some(candidate.to_string());
        }
    }
    None
}

fn first_decimal(value: &Value, keys: &[&str]) -> Option<BigDecimal> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(decimal_from_json))
}

fn bool_field(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn extract_market_values(payload: Value) -> Vec<Value> {
    if let Some(items) = payload.as_array() {
        return items
            .iter()
            .flat_map(extract_market_values_from_item)
            .collect();
    }
    if let Some(markets) = payload.get("markets").and_then(Value::as_array) {
        return markets
            .iter()
            .map(|market| merge_event_context(&payload, market))
            .collect();
    }
    for key in ["events", "data", "items"] {
        if let Some(items) = payload.get(key).and_then(Value::as_array) {
            return items
                .iter()
                .flat_map(extract_market_values_from_item)
                .collect();
        }
    }
    Vec::new()
}

fn extract_market_values_from_item(item: &Value) -> Vec<Value> {
    if let Some(markets) = item.get("markets").and_then(Value::as_array) {
        return markets
            .iter()
            .map(|market| merge_event_context(item, market))
            .collect();
    }
    vec![item.clone()]
}

fn merge_event_context(event: &Value, market: &Value) -> Value {
    let mut merged = market.clone();
    let Some(object) = merged.as_object_mut() else {
        return merged;
    };
    for (target, keys) in [
        ("eventId", &["id", "eventId", "event_id"][..]),
        ("eventSlug", &["slug", "eventSlug", "event_slug"][..]),
        ("category", &["category", "categorySlug"][..]),
        ("image", &["image", "icon", "imageUrl"][..]),
    ] {
        if object.get(target).is_none()
            && let Some(value) = keys.iter().find_map(|key| event.get(*key)).cloned()
        {
            object.insert(target.to_owned(), value);
        }
    }
    if object.get("tags").is_none()
        && let Some(tags) = event.get("tags").cloned()
    {
        object.insert("tags".to_owned(), tags);
    }
    merged
}

fn parse_datetime(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|datetime| datetime.with_timezone(&Utc))
}

fn decimal_str(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap_or_else(|_| BigDecimal::from(0))
}

fn clamp_probability(value: BigDecimal) -> BigDecimal {
    if value <= BigDecimal::from(0) {
        return decimal_str("0.01");
    }
    if value >= BigDecimal::from(1) {
        return decimal_str("0.99");
    }
    value.with_scale(8)
}

fn capped_payout(theoretical_payout: &BigDecimal, cap: &BigDecimal) -> BigDecimal {
    if cap > &BigDecimal::from(0) && theoretical_payout > cap {
        cap.clone()
    } else {
        theoretical_payout.clone()
    }
}

fn prediction_order_no(order_id: u64) -> String {
    format!("PM{}{:08}", Utc::now().format("%Y%m%d"), order_id)
}

fn is_duplicate_key_error(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(database_error) if database_error.is_unique_violation())
}

fn upstream_sync_error(message: String) -> AppError {
    AppError::Api {
        status: StatusCode::BAD_GATEWAY,
        code: "POLYMARKET_SYNC_FAILED",
        message: compact_error_message(&message),
    }
}

fn compact_error_message(value: &str) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    compact.chars().take(512).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_markets_from_polymarket_events_with_context() {
        let payload = json!({
            "events": [
                {
                    "id": "event-1",
                    "slug": "sample-event",
                    "category": "crypto",
                    "tags": [{"label": "Bitcoin"}],
                    "markets": [
                        {
                            "id": "market-1",
                            "question": "Will BTC close above 100k?",
                            "outcomes": "[\"Yes\",\"No\"]",
                            "outcomePrices": "[\"0.42\",\"0.58\"]"
                        }
                    ]
                }
            ]
        });

        let markets = extract_market_values(payload);

        assert_eq!(markets.len(), 1);
        assert_eq!(
            markets[0].get("eventId").and_then(Value::as_str),
            Some("event-1")
        );
        assert_eq!(
            markets[0].get("category").and_then(Value::as_str),
            Some("crypto")
        );
        assert!(markets[0].get("tags").and_then(Value::as_array).is_some());

        let parsed = parse_polymarket_market(&markets[0]).expect("market should parse");
        assert_eq!(parsed.external_event_id.as_deref(), Some("event-1"));
        assert_eq!(parsed.external_market_id, "market-1");
        assert_eq!(parsed.outcome_yes_label, "Yes");
        assert_eq!(parsed.outcome_no_label, "No");
        assert_eq!(parsed.yes_price, decimal_str("0.42"));
    }
}
