//! prediction bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。

use crate::{
    error::{AppError, AppResult},
    modules::{
        auth::{AdminAuth, UserAuth},
        prediction::{
            infrastructure, presentation, repository,
            service::{self, DEFAULT_SYNC_POLL_SECONDS},
        },
    },
    state::AppState,
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use chrono::Utc;
use std::collections::HashSet;
use tokio::time::sleep;

pub async fn run_sync_loop(state: AppState) -> AppResult<()> {
    loop {
        if let Err(error) = run_due_sync_once(&state).await {
            tracing::warn!(%error, "prediction market sync tick failed");
        }
        sleep(tokio::time::Duration::from_secs(DEFAULT_SYNC_POLL_SECONDS)).await;
    }
}

async fn run_due_sync_once(state: &AppState) -> AppResult<()> {
    let pool = infrastructure::mysql_pool(state)?;
    let settings = infrastructure::load_settings(&pool).await?;
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
    infrastructure::sync_polymarket_markets(&pool, "scheduled").await?;
    Ok(())
}

pub(crate) async fn get_admin_settings(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<presentation::PredictionSettingsResponse>> {
    Ok(Json(presentation::PredictionSettingsResponse::from(
        infrastructure::load_settings(&infrastructure::mysql_pool(&state)?).await?,
    )))
}

pub(crate) async fn save_admin_settings(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<presentation::SavePredictionSettingsRequest>,
) -> AppResult<Json<presentation::PredictionSettingsResponse>> {
    let pool = infrastructure::mysql_pool(&state)?;
    let settlement_mode = service::normalize_settlement_mode(&request.default_settlement_mode)?;
    let refund_policy =
        service::normalize_invalid_refund_policy(&request.default_invalid_refund_policy)?;
    service::ensure_non_negative_decimal(&request.default_fee_rate, "default_fee_rate")?;
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
    infrastructure::validate_asset_ids_exist(&pool, &request.allowed_asset_ids).await?;
    let sync_tags = service::normalize_string_list(request.sync_tags);
    let allowed_asset_ids = service::unique_u64_list(request.allowed_asset_ids);

    infrastructure::save_admin_settings(
        &pool,
        request.sync_enabled,
        request.sync_interval_seconds,
        &sync_tags,
        &allowed_asset_ids,
        request.default_fee_rate,
        settlement_mode,
        refund_policy,
        request.quote_ttl_seconds,
    )
    .await?;

    Ok(Json(presentation::PredictionSettingsResponse::from(
        infrastructure::load_settings(&pool).await?,
    )))
}

pub(crate) async fn list_admin_asset_configs(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<presentation::PredictionAssetConfigsResponse>> {
    let configs = infrastructure::list_admin_asset_configs(&infrastructure::mysql_pool(&state)?)
        .await?
        .into_iter()
        .map(presentation::PredictionAssetConfigResponse::from)
        .collect();
    Ok(Json(presentation::PredictionAssetConfigsResponse {
        configs,
    }))
}

pub(crate) async fn get_user_config(
    State(state): State<AppState>,
) -> AppResult<Json<presentation::PredictionUserConfigResponse>> {
    let pool = infrastructure::mysql_pool(&state)?;
    let settings = infrastructure::load_settings(&pool).await?;
    let allowed_ids = service::json_u64_array(&settings.allowed_asset_ids_json);
    if allowed_ids.is_empty() {
        return Ok(Json(presentation::PredictionUserConfigResponse {
            allowed_assets: Vec::new(),
            default_fee_rate: settings.default_fee_rate,
            quote_ttl_seconds: settings.quote_ttl_seconds,
        }));
    }
    let allowed_set = allowed_ids.into_iter().collect::<HashSet<_>>();
    let allowed_assets = infrastructure::list_stake_assets(&pool)
        .await?
        .into_iter()
        .filter(|row| allowed_set.contains(&row.asset_id))
        .map(presentation::PredictionStakeAssetResponse::from)
        .collect();
    Ok(Json(presentation::PredictionUserConfigResponse {
        allowed_assets,
        default_fee_rate: settings.default_fee_rate,
        quote_ttl_seconds: settings.quote_ttl_seconds,
    }))
}

pub(crate) async fn upsert_admin_asset_config(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<presentation::UpsertPredictionAssetConfigRequest>,
) -> AppResult<Json<presentation::PredictionAssetConfigResponse>> {
    infrastructure::upsert_asset_config(
        &infrastructure::mysql_pool(&state)?,
        request.asset_id,
        request.enabled,
        request.max_payout_amount,
    )
    .await
    .map(Json)
}

pub(crate) async fn update_admin_asset_config(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(asset_id): Path<u64>,
    Json(request): Json<presentation::UpdatePredictionAssetConfigRequest>,
) -> AppResult<Json<presentation::PredictionAssetConfigResponse>> {
    infrastructure::upsert_asset_config(
        &infrastructure::mysql_pool(&state)?,
        asset_id,
        request.enabled,
        request.max_payout_amount,
    )
    .await
    .map(Json)
}

pub(crate) async fn list_user_markets(
    State(state): State<AppState>,
    Query(query): Query<presentation::ListQuery>,
) -> AppResult<Json<presentation::PredictionMarketsResponse>> {
    let mut builder = infrastructure::prediction_market_query_builder();
    builder.push(" WHERE markets.display_status = ");
    builder.push_bind(service::STATUS_ACTIVE);
    builder.push(" AND markets.settlement_status IN ('open', 'pending_confirmation')");
    builder.push(" ORDER BY markets.last_synced_at DESC, markets.id DESC LIMIT ");
    builder.push_bind(service::route_limit(query.limit) as i64);
    let rows = builder
        .build_query_as::<repository::PredictionMarketRow>()
        .fetch_all(&infrastructure::mysql_pool(&state)?)
        .await?;
    let markets = rows
        .into_iter()
        .map(presentation::PredictionMarketResponse::from)
        .collect();
    Ok(Json(presentation::PredictionMarketsResponse { markets }))
}

pub(crate) async fn get_user_market(
    State(state): State<AppState>,
    Path(market_id): Path<u64>,
) -> AppResult<Json<presentation::PredictionMarketResponse>> {
    let market =
        infrastructure::load_market_response(&infrastructure::mysql_pool(&state)?, market_id)
            .await?;
    if market.display_status != service::STATUS_ACTIVE {
        return Err(AppError::NotFound);
    }
    Ok(Json(market))
}

pub(crate) async fn list_admin_markets(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<presentation::AdminMarketQuery>,
) -> AppResult<Json<presentation::PredictionMarketsResponse>> {
    let mut builder = infrastructure::prediction_market_query_builder();
    builder.push(" WHERE 1 = 1");
    if let Some(status) = service::optional_text(query.display_status) {
        builder.push(" AND markets.display_status = ");
        builder.push_bind(status);
    }
    if let Some(status) = service::optional_text(query.settlement_status) {
        builder.push(" AND markets.settlement_status = ");
        builder.push_bind(status);
    }
    if let Some(keyword) = service::optional_text(query.keyword) {
        builder.push(" AND markets.title LIKE ");
        builder.push_bind(format!("%{keyword}%"));
    }
    builder.push(" ORDER BY markets.last_synced_at DESC, markets.id DESC LIMIT ");
    builder.push_bind(service::route_limit(query.limit) as i64);
    let rows = builder
        .build_query_as::<repository::PredictionMarketRow>()
        .fetch_all(&infrastructure::mysql_pool(&state)?)
        .await?;
    let markets = rows
        .into_iter()
        .map(presentation::PredictionMarketResponse::from)
        .collect();
    Ok(Json(presentation::PredictionMarketsResponse { markets }))
}

pub(crate) async fn get_admin_market(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(market_id): Path<u64>,
) -> AppResult<Json<presentation::PredictionMarketResponse>> {
    Ok(Json(
        infrastructure::load_market_response(&infrastructure::mysql_pool(&state)?, market_id)
            .await?,
    ))
}

pub(crate) async fn update_admin_market(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(market_id): Path<u64>,
    Json(request): Json<presentation::UpdatePredictionMarketRequest>,
) -> AppResult<Json<presentation::PredictionMarketResponse>> {
    let pool = infrastructure::mysql_pool(&state)?;
    let display_status = service::normalize_display_status(&request.display_status)?;
    let settlement_mode_override = match request.settlement_mode_override {
        Some(value) if !value.trim().is_empty() => {
            Some(service::normalize_settlement_mode(&value)?)
        }
        _ => None,
    };
    let allowed_override = request
        .allowed_asset_ids_override
        .map(service::unique_u64_list);
    if let Some(ids) = allowed_override.as_ref() {
        infrastructure::validate_asset_ids_exist(&pool, ids).await?;
    }
    if let Some(rate) = request.fee_rate_override.as_ref() {
        service::ensure_non_negative_decimal(rate, "fee_rate_override")?;
    }

    let updated = infrastructure::update_admin_market(
        &pool,
        market_id,
        &display_status,
        settlement_mode_override.as_deref(),
        allowed_override.as_deref(),
        request.payout_cap_overrides.as_ref(),
        request.fee_rate_override.as_ref(),
    )
    .await?;
    if !updated {
        return Err(AppError::NotFound);
    }
    Ok(Json(
        infrastructure::load_market_response(&pool, market_id).await?,
    ))
}

pub(crate) async fn create_quote(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<presentation::CreatePredictionQuoteRequest>,
) -> AppResult<Json<presentation::PredictionQuoteResponse>> {
    let user_id = service::user_id_from_subject(&claims.sub)?;
    let quote =
        infrastructure::create_quote_in_db(&infrastructure::mysql_pool(&state)?, user_id, request)
            .await?;
    Ok(Json(quote))
}

pub(crate) async fn create_order(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<presentation::CreatePredictionOrderRequest>,
) -> AppResult<Json<presentation::PredictionOrderActionResponse>> {
    let user_id = service::user_id_from_subject(&claims.sub)?;
    let (order, changed) =
        infrastructure::create_order_in_tx(&infrastructure::mysql_pool(&state)?, user_id, request)
            .await?;
    Ok(Json(presentation::PredictionOrderActionResponse {
        order,
        changed,
    }))
}

pub(crate) async fn list_user_orders(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<presentation::OrdersQuery>,
) -> AppResult<Json<presentation::PredictionOrdersResponse>> {
    let user_id = service::user_id_from_subject(&claims.sub)?;
    let mut builder = infrastructure::prediction_order_query_builder();
    builder.push(" WHERE orders.user_id = ");
    builder.push_bind(user_id);
    if let Some(status) = service::optional_text(query.status) {
        builder.push(" AND orders.status = ");
        builder.push_bind(status);
    }
    if let Some(market_id) = query.market_id {
        builder.push(" AND orders.market_id = ");
        builder.push_bind(market_id);
    }
    builder.push(" ORDER BY orders.created_at DESC, orders.id DESC LIMIT ");
    builder.push_bind(service::route_limit(query.limit) as i64);
    let rows = builder
        .build_query_as::<repository::PredictionOrderRow>()
        .fetch_all(&infrastructure::mysql_pool(&state)?)
        .await?;
    let orders = rows
        .into_iter()
        .map(presentation::PredictionOrderResponse::from)
        .collect();
    Ok(Json(presentation::PredictionOrdersResponse { orders }))
}

pub(crate) async fn list_admin_orders(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<presentation::OrdersQuery>,
) -> AppResult<Json<presentation::PredictionOrdersResponse>> {
    let mut builder = infrastructure::prediction_order_query_builder();
    builder.push(" WHERE 1 = 1");
    if let Some(status) = service::optional_text(query.status) {
        builder.push(" AND orders.status = ");
        builder.push_bind(status);
    }
    if let Some(market_id) = query.market_id {
        builder.push(" AND orders.market_id = ");
        builder.push_bind(market_id);
    }
    if let Some(email) = service::optional_text(query.email) {
        builder.push(" AND users.email LIKE ");
        builder.push_bind(format!("%{email}%"));
    }
    builder.push(" ORDER BY orders.created_at DESC, orders.id DESC LIMIT ");
    builder.push_bind(service::route_limit(query.limit) as i64);
    let rows = builder
        .build_query_as::<repository::PredictionOrderRow>()
        .fetch_all(&infrastructure::mysql_pool(&state)?)
        .await?;
    let orders = rows
        .into_iter()
        .map(presentation::PredictionOrderResponse::from)
        .collect();
    Ok(Json(presentation::PredictionOrdersResponse { orders }))
}

pub(crate) async fn get_admin_order(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<presentation::PredictionOrderResponse>> {
    Ok(Json(
        infrastructure::load_order_response(&infrastructure::mysql_pool(&state)?, order_id).await?,
    ))
}

pub(crate) async fn settle_admin_market(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(market_id): Path<u64>,
    Json(request): Json<presentation::SettlePredictionMarketRequest>,
) -> AppResult<Json<presentation::PredictionSettlementResponse>> {
    let result = service::normalize_settlement_result(&request.result)?;
    let refund_policy = match request.invalid_refund_policy {
        Some(value) if !value.trim().is_empty() => {
            Some(service::normalize_invalid_refund_policy(&value)?)
        }
        _ => None,
    };
    let (market, settled_orders, changed) = infrastructure::settle_market_in_tx(
        &infrastructure::mysql_pool(&state)?,
        market_id,
        result,
        refund_policy,
    )
    .await?;
    Ok(Json(presentation::PredictionSettlementResponse {
        market,
        settled_orders,
        changed,
    }))
}

pub(crate) async fn trigger_admin_sync(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<presentation::PredictionSyncResponse>> {
    let response =
        infrastructure::sync_polymarket_markets(&infrastructure::mysql_pool(&state)?, "manual")
            .await?;
    Ok(Json(response))
}

pub(crate) async fn list_admin_sync_logs(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<presentation::ListQuery>,
) -> AppResult<Json<presentation::PredictionSyncLogsResponse>> {
    let rows = infrastructure::list_admin_sync_logs(
        &infrastructure::mysql_pool(&state)?,
        service::route_limit(query.limit) as i64,
    )
    .await?;
    let logs = rows
        .into_iter()
        .map(presentation::PredictionSyncLogResponse::from)
        .collect();
    Ok(Json(presentation::PredictionSyncLogsResponse { logs }))
}
