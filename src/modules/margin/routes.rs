use crate::{
    error::{AppError, AppResult},
    modules::{
        auth::{AdminAuth, UserAuth},
        events::EventBroadcastMessage,
        market::market_ticker_redis_key,
    },
    state::AppState,
    time::{option_unix_millis, unix_millis},
    workers::margin_liquidation::margin_liquidation_risk_state,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, patch, post},
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use redis::{AsyncCommands, aio::ConnectionManager};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{Value, json};
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};
use std::{collections::BTreeSet, str::FromStr};

pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/margin/products", get(list_active_products))
        .route("/margin/positions", get(list_positions).post(open_position))
        .route("/margin/positions/:id", get(get_position))
        .route("/margin/positions/:id/risk", get(get_position_risk))
        .route("/margin/positions/:id/close", post(close_position))
}

pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/margin/products",
            get(list_admin_products).post(create_product),
        )
        .route("/margin/products/:id", get(get_admin_product))
        .route("/margin/products/:id/status", patch(update_product_status))
        .route("/margin/positions", get(list_admin_positions))
        .route("/margin/positions/:id", get(get_admin_position))
        .route("/margin/interest/summary", get(list_admin_interest_summary))
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ListPositionsQuery {
    status: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminListPositionsQuery {
    user_id: Option<u64>,
    pair_id: Option<u64>,
    status: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminInterestSummaryQuery {
    user_id: Option<u64>,
    pair_id: Option<u64>,
    status: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OpenMarginPositionRequest {
    product_id: u64,
    direction: String,
    margin_amount: BigDecimal,
    leverage: BigDecimal,
    idempotency_key: String,
}

#[derive(Debug, Deserialize)]
struct CachedTickerPayload {
    last_price: BigDecimal,
    #[serde(with = "unix_millis")]
    observed_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct CreateMarginProductRequest {
    pair_id: u64,
    margin_asset: u64,
    margin_mode: Option<String>,
    leverage_levels: Option<Vec<BigDecimal>>,
    max_leverage: BigDecimal,
    min_margin: BigDecimal,
    max_margin: Option<BigDecimal>,
    maintenance_margin_rate: BigDecimal,
    hourly_interest_rate: Option<BigDecimal>,
    status: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateMarginProductStatusRequest {
    status: String,
    reason: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct MarginProductResponse {
    id: u64,
    pair_id: u64,
    symbol: String,
    margin_asset: u64,
    margin_asset_symbol: String,
    margin_mode: String,
    leverage_levels: SqlxJson<Vec<String>>,
    max_leverage: BigDecimal,
    min_margin: BigDecimal,
    max_margin: Option<BigDecimal>,
    maintenance_margin_rate: BigDecimal,
    hourly_interest_rate: BigDecimal,
    status: String,
}

#[derive(Debug, Serialize)]
struct MarginProductsResponse {
    products: Vec<MarginProductResponse>,
}

#[derive(Debug, Serialize)]
struct MarginPositionsResponse {
    positions: Vec<MarginPositionResponse>,
}

#[derive(Debug, Serialize)]
struct MarginPositionDetailResponse {
    position: MarginPositionResponse,
}

#[derive(Debug, Serialize)]
struct AdminMarginPositionsResponse {
    positions: Vec<AdminMarginPositionResponse>,
}

#[derive(Debug, Serialize)]
struct AdminInterestSummaryResponse {
    summaries: Vec<AdminInterestSummaryItem>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminInterestSummaryItem {
    margin_asset: u64,
    status: String,
    position_count: i64,
    #[serde(serialize_with = "serialize_decimal_amount")]
    borrowed_amount: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    interest_amount: BigDecimal,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminMarginPositionResponse {
    id: u64,
    user_id: u64,
    product_id: u64,
    pair_id: u64,
    margin_asset: u64,
    margin_mode: String,
    direction: String,
    margin_amount: BigDecimal,
    leverage: BigDecimal,
    notional_amount: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    borrowed_amount: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    interest_amount: BigDecimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    entry_price: Option<BigDecimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    exit_price: Option<BigDecimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    realized_pnl: Option<BigDecimal>,
    #[serde(default, with = "option_unix_millis")]
    closed_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    liquidated_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    liquidation_reason: Option<String>,
    status: String,
    idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
struct MarginPositionResponse {
    id: u64,
    user_id: u64,
    product_id: u64,
    pair_id: u64,
    margin_asset: u64,
    margin_mode: String,
    direction: String,
    margin_amount: BigDecimal,
    leverage: BigDecimal,
    notional_amount: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    borrowed_amount: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    interest_amount: BigDecimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    entry_price: Option<BigDecimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    exit_price: Option<BigDecimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    realized_pnl: Option<BigDecimal>,
    #[serde(
        default,
        with = "option_unix_millis",
        skip_serializing_if = "Option::is_none"
    )]
    closed_at: Option<DateTime<Utc>>,
    status: String,
    idempotency_key: String,
}

#[derive(Debug, Serialize)]
struct OpenMarginPositionResponse {
    position: MarginPositionResponse,
}

#[derive(Debug, Serialize)]
struct CloseMarginPositionResponse {
    position: MarginPositionResponse,
}

#[derive(Debug, sqlx::FromRow)]
struct MarginProductRuleRow {
    id: u64,
    pair_id: u64,
    symbol: String,
    margin_asset: u64,
    margin_mode: String,
    leverage_levels: SqlxJson<Vec<String>>,
    min_margin: BigDecimal,
    max_margin: Option<BigDecimal>,
    hourly_interest_rate: BigDecimal,
    status: String,
}

#[derive(Debug, sqlx::FromRow)]
struct MarginWalletRow {
    available: BigDecimal,
    frozen: BigDecimal,
    locked: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct LockedMarginPositionRow {
    id: u64,
    pair_id: u64,
    symbol: String,
    margin_asset: u64,
    direction: String,
    margin_amount: BigDecimal,
    notional_amount: BigDecimal,
    interest_amount: BigDecimal,
    entry_price: Option<BigDecimal>,
    status: String,
}

#[derive(Debug, sqlx::FromRow)]
struct MarginRiskPositionRow {
    id: u64,
    pair_id: u64,
    symbol: String,
    margin_asset: u64,
    direction: String,
    margin_amount: BigDecimal,
    notional_amount: BigDecimal,
    interest_amount: BigDecimal,
    entry_price: Option<BigDecimal>,
    maintenance_margin_rate: BigDecimal,
    status: String,
}

#[derive(Debug, Serialize)]
struct MarginRiskSnapshotResponse {
    risk: MarginRiskSnapshot,
}

#[derive(Debug, Serialize)]
struct MarginRiskSnapshot {
    position_id: u64,
    pair_id: u64,
    symbol: String,
    margin_asset: u64,
    direction: String,
    margin_amount: BigDecimal,
    notional_amount: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    interest_amount: BigDecimal,
    entry_price: BigDecimal,
    mark_price: BigDecimal,
    maintenance_margin_rate: BigDecimal,
    realized_pnl: BigDecimal,
    equity: BigDecimal,
    maintenance_margin: BigDecimal,
    should_liquidate: bool,
    #[serde(with = "unix_millis")]
    observed_at: DateTime<Utc>,
}

async fn list_active_products(
    UserAuth(_claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<MarginProductsResponse>> {
    list_products(
        mysql_pool(&state)?,
        Some("active"),
        route_limit(query.limit),
    )
    .await
}

async fn list_admin_products(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<MarginProductsResponse>> {
    list_products(mysql_pool(&state)?, None, route_limit(query.limit)).await
}

async fn get_admin_product(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
) -> AppResult<Json<MarginProductResponse>> {
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    let product = load_product_by_id(&mut tx, product_id).await?;
    tx.commit().await?;
    Ok(Json(product))
}

async fn create_product(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateMarginProductRequest>,
) -> AppResult<Json<MarginProductResponse>> {
    validate_create_product_request(&request)?;
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let status = normalized_product_status(request.status.as_deref().unwrap_or("active"))?;
    let margin_mode = normalized_margin_mode(request.margin_mode.as_deref().unwrap_or("isolated"))?;
    let leverage_levels =
        validated_leverage_levels(&request.max_leverage, request.leverage_levels.as_deref())?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    ensure_pair_exists(&mut tx, request.pair_id).await?;
    ensure_asset_exists(&mut tx, request.margin_asset).await?;
    let product_id = sqlx::query(
        r#"INSERT INTO margin_products
           (pair_id, margin_asset, margin_mode, leverage_levels, max_leverage, min_margin, max_margin,
            maintenance_margin_rate, hourly_interest_rate, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(request.pair_id)
    .bind(request.margin_asset)
    .bind(&margin_mode)
    .bind(SqlxJson(leverage_levels))
    .bind(&request.max_leverage)
    .bind(&request.min_margin)
    .bind(&request.max_margin)
    .bind(&request.maintenance_margin_rate)
    .bind(request.hourly_interest_rate.unwrap_or_else(zero_rate))
    .bind(&status)
    .execute(&mut *tx)
    .await?
    .last_insert_id();
    let product = load_product_by_id(&mut tx, product_id).await?;
    insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "margin_product.create",
        product.id,
        None,
        Some(product_audit_json(&product)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(product))
}

async fn update_product_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
    Json(request): Json<UpdateMarginProductStatusRequest>,
) -> AppResult<Json<MarginProductResponse>> {
    let status = normalized_product_status(&request.status)?;
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    let before = lock_product_by_id(&mut tx, product_id).await?;
    sqlx::query("UPDATE margin_products SET status = ? WHERE id = ?")
        .bind(&status)
        .bind(product_id)
        .execute(&mut *tx)
        .await?;
    let after = load_product_by_id(&mut tx, product_id).await?;
    insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "margin_product.update_status",
        product_id,
        Some(product_audit_json(&before)),
        Some(product_audit_json(&after)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(after))
}

async fn list_positions(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListPositionsQuery>,
) -> AppResult<Json<MarginPositionsResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let status = optional_string(query.status)
        .map(|status| normalized_position_status(&status))
        .transpose()?;
    let pool = mysql_pool(&state)?;
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, margin_mode, direction, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
                  exit_price, realized_pnl, closed_at, status, idempotency_key
           FROM margin_positions
           WHERE user_id = "#,
    );
    builder.push_bind(user_id);
    if let Some(status) = status.as_deref() {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);
    let positions = builder
        .build_query_as::<MarginPositionResponse>()
        .fetch_all(&pool)
        .await?;
    Ok(Json(MarginPositionsResponse { positions }))
}

async fn list_admin_positions(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminListPositionsQuery>,
) -> AppResult<Json<AdminMarginPositionsResponse>> {
    let status = optional_string(query.status)
        .map(|status| normalized_position_status(&status))
        .transpose()?;
    let pool = mysql_pool(&state)?;
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, margin_mode, direction, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
                  exit_price, realized_pnl, closed_at, liquidated_at, liquidation_reason, status,
                  idempotency_key
           FROM margin_positions
           WHERE 1 = 1"#,
    );
    if let Some(user_id) = query.user_id {
        builder.push(" AND user_id = ");
        builder.push_bind(user_id);
    }
    if let Some(pair_id) = query.pair_id {
        builder.push(" AND pair_id = ");
        builder.push_bind(pair_id);
    }
    if let Some(status) = status.as_deref() {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);
    let positions = builder
        .build_query_as::<AdminMarginPositionResponse>()
        .fetch_all(&pool)
        .await?;
    Ok(Json(AdminMarginPositionsResponse { positions }))
}

async fn get_admin_position(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(position_id): Path<u64>,
) -> AppResult<Json<AdminMarginPositionResponse>> {
    let position = load_admin_position_by_id(&mysql_pool(&state)?, position_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(position))
}

async fn list_admin_interest_summary(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminInterestSummaryQuery>,
) -> AppResult<Json<AdminInterestSummaryResponse>> {
    let status = optional_string(query.status)
        .map(|status| normalized_position_status(&status))
        .transpose()?;
    let pool = mysql_pool(&state)?;
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT margin_asset, status, COUNT(*) AS position_count,
                  COALESCE(SUM(borrowed_amount), 0) AS borrowed_amount,
                  COALESCE(SUM(interest_amount), 0) AS interest_amount
           FROM margin_positions
           WHERE 1 = 1"#,
    );
    if let Some(user_id) = query.user_id {
        builder.push(" AND user_id = ");
        builder.push_bind(user_id);
    }
    if let Some(pair_id) = query.pair_id {
        builder.push(" AND pair_id = ");
        builder.push_bind(pair_id);
    }
    if let Some(status) = status.as_deref() {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    builder.push(" GROUP BY margin_asset, status ORDER BY margin_asset ASC, status ASC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);
    let summaries = builder
        .build_query_as::<AdminInterestSummaryItem>()
        .fetch_all(&pool)
        .await?;
    Ok(Json(AdminInterestSummaryResponse { summaries }))
}

async fn get_position(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(position_id): Path<u64>,
) -> AppResult<Json<MarginPositionDetailResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let position = load_user_position_by_id(&mysql_pool(&state)?, user_id, position_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(MarginPositionDetailResponse { position }))
}

async fn get_position_risk(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(position_id): Path<u64>,
) -> AppResult<Json<MarginRiskSnapshotResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let position = load_user_risk_position_by_id(&mysql_pool(&state)?, user_id, position_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if position.status != "opened" {
        return Err(AppError::Validation(
            "margin risk snapshot requires an opened position".to_owned(),
        ));
    }
    let Some(entry_price) = position.entry_price.clone() else {
        return Err(AppError::Validation(
            "margin entry price is required for risk snapshot".to_owned(),
        ));
    };
    let ticker =
        cached_margin_risk_ticker(state.redis.as_ref(), position.pair_id, &position.symbol).await?;
    let risk_state = margin_liquidation_risk_state(
        &position.direction,
        &position.margin_amount,
        &position.notional_amount,
        &position.interest_amount,
        &entry_price,
        &ticker.last_price,
        &position.maintenance_margin_rate,
    )?;

    Ok(Json(MarginRiskSnapshotResponse {
        risk: MarginRiskSnapshot {
            position_id: position.id,
            pair_id: position.pair_id,
            symbol: position.symbol,
            margin_asset: position.margin_asset,
            direction: position.direction,
            margin_amount: position.margin_amount,
            notional_amount: position.notional_amount,
            interest_amount: position.interest_amount,
            entry_price,
            mark_price: ticker.last_price,
            maintenance_margin_rate: position.maintenance_margin_rate,
            realized_pnl: risk_state.realized_pnl,
            equity: risk_state.equity,
            maintenance_margin: risk_state.maintenance_margin,
            should_liquidate: risk_state.should_liquidate,
            observed_at: ticker.observed_at,
        },
    }))
}

async fn list_products(
    pool: Pool<MySql>,
    status: Option<&str>,
    limit: u32,
) -> AppResult<Json<MarginProductsResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT products.id, products.pair_id, pairs.symbol,
                  products.margin_asset, assets.symbol AS margin_asset_symbol,
                  products.margin_mode, products.leverage_levels, products.max_leverage,
                  products.min_margin, products.max_margin, products.maintenance_margin_rate,
                  products.hourly_interest_rate, products.status
           FROM margin_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           INNER JOIN assets ON assets.id = products.margin_asset"#,
    );

    if let Some(status) = status {
        builder.push(" WHERE products.status = ");
        builder.push_bind(status);
    }

    builder.push(" ORDER BY products.id DESC LIMIT ");
    builder.push_bind(limit as i64);

    let products = builder
        .build_query_as::<MarginProductResponse>()
        .fetch_all(&pool)
        .await?;
    Ok(Json(MarginProductsResponse { products }))
}

async fn open_position(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<OpenMarginPositionRequest>,
) -> AppResult<Json<OpenMarginPositionResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let idempotency_key = normalize_idempotency_key(&request.idempotency_key)?;
    let direction = normalize_direction(&request.direction)?;
    validate_positive_decimal(&request.margin_amount, "margin amount")?;
    validate_positive_decimal(&request.leverage, "leverage")?;
    let (position, is_new_position) = open_position_in_tx(
        &state,
        user_id,
        request.product_id,
        direction,
        request.margin_amount,
        request.leverage,
        idempotency_key,
    )
    .await?;
    let response = OpenMarginPositionResponse { position };
    if is_new_position && let Some(hub) = &state.event_broadcast_hub {
        hub.publish(EventBroadcastMessage::private_user(
            user_id,
            json!({
                "type": "margin.position.opened",
                "position_id": response.position.id,
                "product_id": response.position.product_id,
                "pair_id": response.position.pair_id,
                "margin_asset": response.position.margin_asset,
                "margin_mode": response.position.margin_mode,
                "direction": response.position.direction,
                "margin_amount": response.position.margin_amount,
                "leverage": response.position.leverage,
                "notional_amount": response.position.notional_amount,
                "borrowed_amount": decimal_amount_string(&response.position.borrowed_amount),
                "interest_amount": decimal_amount_string(&response.position.interest_amount),
                "entry_price": response.position.entry_price,
                "status": response.position.status,
            })
            .to_string(),
        ));
    }
    Ok(Json(response))
}

async fn close_position(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(position_id): Path<u64>,
) -> AppResult<Json<CloseMarginPositionResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let (position, is_new_close) = close_position_in_tx(&state, user_id, position_id).await?;
    let response = CloseMarginPositionResponse { position };
    if is_new_close && let Some(hub) = &state.event_broadcast_hub {
        hub.publish(EventBroadcastMessage::private_user(
            user_id,
            json!({
                "type": "margin.position.closed",
                "position_id": response.position.id,
                "product_id": response.position.product_id,
                "pair_id": response.position.pair_id,
                "margin_asset": response.position.margin_asset,
                "direction": response.position.direction,
                "margin_amount": response.position.margin_amount,
                "exit_price": response.position.exit_price,
                "realized_pnl": response.position.realized_pnl,
                "interest_amount": decimal_amount_string(&response.position.interest_amount),
                "payout_amount": margin_position_payout_amount(
                    &response.position.margin_amount,
                    response.position.realized_pnl.as_ref(),
                    &response.position.interest_amount,
                ),
                "closed_at": response.position.closed_at.map(|closed_at| closed_at.timestamp_millis()),
                "status": response.position.status,
            })
            .to_string(),
        ));
    }
    Ok(Json(response))
}

async fn open_position_in_tx(
    state: &AppState,
    user_id: u64,
    product_id: u64,
    direction: String,
    margin_amount: BigDecimal,
    leverage: BigDecimal,
    idempotency_key: String,
) -> AppResult<(MarginPositionResponse, bool)> {
    let pool = mysql_pool(state)?;
    if let Some(existing) =
        existing_position_for_idempotency_key_readonly(&pool, user_id, &idempotency_key).await?
    {
        ensure_existing_position_matches_request(
            &existing,
            product_id,
            &direction,
            &margin_amount,
            &leverage,
        )?;
        return Ok((existing, false));
    }

    let mut tx = pool.begin().await?;
    let product = match lock_active_product(&mut tx, product_id).await {
        Ok(product) => product,
        Err(AppError::NotFound) => {
            tx.rollback().await?;
            if let Some(existing) = replay_existing_position_if_present(
                &pool,
                user_id,
                product_id,
                &direction,
                &margin_amount,
                &leverage,
                &idempotency_key,
            )
            .await?
            {
                return Ok((existing, false));
            }
            return Err(AppError::NotFound);
        }
        Err(error) => return Err(error),
    };
    validate_product_margin(&margin_amount, &leverage, &product)?;
    let notional_amount = margin_amount.clone() * leverage.clone();
    let borrowed_amount = margin_borrowed_amount(&notional_amount, &margin_amount);
    let entry_price = cached_entry_price(
        state.redis.as_ref(),
        product.pair_id,
        product.symbol.as_str(),
    )
    .await?;
    // 先写入仓位占用用户幂等键，再锁定钱包扣保证金，避免同 key 并发重复扣款。
    let position_id = match sqlx::query(
        r#"INSERT INTO margin_positions
           (user_id, product_id, pair_id, margin_asset, margin_mode, direction, margin_amount,
            leverage, notional_amount, borrowed_amount, interest_amount, interest_accrued_at,
            entry_price, status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP(6), ?, 'opened', ?)"#,
    )
    .bind(user_id)
    .bind(product.id)
    .bind(product.pair_id)
    .bind(product.margin_asset)
    .bind(&product.margin_mode)
    .bind(&direction)
    .bind(&margin_amount)
    .bind(&leverage)
    .bind(&notional_amount)
    .bind(&borrowed_amount)
    .bind(zero_amount())
    .bind(&entry_price)
    .bind(&idempotency_key)
    .execute(&mut *tx)
    .await
    {
        Ok(result) => result.last_insert_id(),
        Err(error) if is_duplicate_key_error(&error) => {
            tx.rollback().await?;
            return replay_existing_position(
                &pool,
                user_id,
                product_id,
                &direction,
                &margin_amount,
                &leverage,
                &idempotency_key,
            )
            .await
            .map(|position| (position, false));
        }
        Err(error) => return Err(AppError::Database(error)),
    };

    let wallet = lock_wallet_row(&mut tx, user_id, product.margin_asset).await?;
    if wallet.available < margin_amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for margin position: requested {}, available {}, locked {}",
            margin_amount, wallet.available, wallet.locked
        )));
    }
    let available_after = wallet.available.clone() - margin_amount.clone();

    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
        .bind(user_id)
        .bind(product.margin_asset)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, 'margin_position_open', ?, 'available', ?, ?, ?, ?, 'margin_position', ?)"#,
    )
    .bind(user_id)
    .bind(product.margin_asset)
    .bind(-margin_amount.clone())
    .bind(&available_after)
    .bind(&available_after)
    .bind(&wallet.frozen)
    .bind(&wallet.locked)
    .bind(position_id.to_string())
    .execute(&mut *tx)
    .await?;

    let position = load_position_by_id(&mut tx, position_id).await?;
    tx.commit().await?;
    Ok((position, true))
}

async fn close_position_in_tx(
    state: &AppState,
    user_id: u64,
    position_id: u64,
) -> AppResult<(MarginPositionResponse, bool)> {
    let pool = mysql_pool(state)?;
    let mut tx = pool.begin().await?;
    let Some(position) = lock_user_position_by_id(&mut tx, user_id, position_id).await? else {
        return Err(AppError::NotFound);
    };
    if position.status != "opened" {
        let position = load_position_by_id(&mut tx, position.id).await?;
        tx.commit().await?;
        return Ok((position, false));
    }
    let Some(entry_price) = position.entry_price.as_ref() else {
        return Err(AppError::Validation(
            "margin entry price is required to close position".to_owned(),
        ));
    };
    let mark_price =
        cached_mark_price(state.redis.as_ref(), position.pair_id, &position.symbol).await?;
    let realized_pnl = margin_realized_pnl(
        &position.direction,
        &position.notional_amount,
        entry_price,
        &mark_price,
    )?;
    let payout_amount = margin_payout_amount(
        &position.margin_amount,
        &realized_pnl,
        &position.interest_amount,
    );
    if payout_amount > 0 {
        let wallet = lock_wallet_row(&mut tx, user_id, position.margin_asset).await?;
        let available_after = wallet.available.clone() + payout_amount.clone();
        sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
            .bind(&available_after)
            .bind(user_id)
            .bind(position.margin_asset)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            r#"INSERT INTO wallet_ledger
               (user_id, asset_id, change_type, amount, balance_type, balance_after,
                available_after, frozen_after, locked_after, ref_type, ref_id)
               VALUES (?, ?, 'margin_position_close', ?, 'available', ?, ?, ?, ?, 'margin_position', ?)"#,
        )
        .bind(user_id)
        .bind(position.margin_asset)
        .bind(&payout_amount)
        .bind(&available_after)
        .bind(&available_after)
        .bind(&wallet.frozen)
        .bind(&wallet.locked)
        .bind(position.id.to_string())
        .execute(&mut *tx)
        .await?;
    }
    let now = Utc::now();
    let update_position = sqlx::query(
        r#"UPDATE margin_positions
           SET status = 'closed', closed_at = ?, exit_price = ?, realized_pnl = ?,
               next_liquidation_attempt_at = NULL
           WHERE id = ? AND user_id = ? AND status = 'opened'"#,
    )
    .bind(now.naive_utc())
    .bind(&mark_price)
    .bind(&realized_pnl)
    .bind(position.id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    if update_position.rows_affected() != 1 {
        tx.rollback().await?;
        return Err(AppError::Conflict(
            "margin position close status changed concurrently".to_owned(),
        ));
    }
    let position = load_position_by_id(&mut tx, position.id).await?;
    tx.commit().await?;
    Ok((position, true))
}

async fn replay_existing_position(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    direction: &str,
    margin_amount: &BigDecimal,
    leverage: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<MarginPositionResponse> {
    replay_existing_position_if_present(
        pool,
        user_id,
        product_id,
        direction,
        margin_amount,
        leverage,
        idempotency_key,
    )
    .await?
    .ok_or_else(|| AppError::Conflict("margin idempotency key is being committed".to_owned()))
}

async fn replay_existing_position_if_present(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    direction: &str,
    margin_amount: &BigDecimal,
    leverage: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<Option<MarginPositionResponse>> {
    let mut tx = pool.begin().await?;
    let Some(existing) =
        existing_position_for_idempotency_key(&mut tx, user_id, idempotency_key).await?
    else {
        return Ok(None);
    };
    ensure_existing_position_matches_request(
        &existing,
        product_id,
        direction,
        margin_amount,
        leverage,
    )?;
    tx.commit().await?;
    Ok(Some(existing))
}

async fn existing_position_for_idempotency_key(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<MarginPositionResponse>> {
    sqlx::query_as::<_, MarginPositionResponse>(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, margin_mode, direction, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
                  exit_price, realized_pnl, closed_at, status, idempotency_key
           FROM margin_positions
           WHERE user_id = ? AND idempotency_key = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(idempotency_key)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

async fn existing_position_for_idempotency_key_readonly(
    pool: &Pool<MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<MarginPositionResponse>> {
    sqlx::query_as::<_, MarginPositionResponse>(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, margin_mode, direction, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
                  exit_price, realized_pnl, closed_at, status, idempotency_key
           FROM margin_positions
           WHERE user_id = ? AND idempotency_key = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

async fn lock_user_position_by_id(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    position_id: u64,
) -> AppResult<Option<LockedMarginPositionRow>> {
    sqlx::query_as::<_, LockedMarginPositionRow>(
        r#"SELECT positions.id, positions.pair_id, pairs.symbol,
                  positions.margin_asset, positions.direction, positions.margin_amount,
                  positions.notional_amount, positions.interest_amount, positions.entry_price,
                  positions.status
           FROM margin_positions positions
           INNER JOIN trading_pairs pairs ON pairs.id = positions.pair_id
           WHERE positions.id = ? AND positions.user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(position_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

async fn load_user_position_by_id(
    pool: &Pool<MySql>,
    user_id: u64,
    position_id: u64,
) -> AppResult<Option<MarginPositionResponse>> {
    sqlx::query_as::<_, MarginPositionResponse>(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, margin_mode, direction, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
                  exit_price, realized_pnl, closed_at, status, idempotency_key
           FROM margin_positions
           WHERE id = ? AND user_id = ?
           LIMIT 1"#,
    )
    .bind(position_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

async fn load_admin_position_by_id(
    pool: &Pool<MySql>,
    position_id: u64,
) -> AppResult<Option<AdminMarginPositionResponse>> {
    sqlx::query_as::<_, AdminMarginPositionResponse>(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, margin_mode, direction, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
                  exit_price, realized_pnl, closed_at, liquidated_at, liquidation_reason, status,
                  idempotency_key
           FROM margin_positions
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(position_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

async fn load_user_risk_position_by_id(
    pool: &Pool<MySql>,
    user_id: u64,
    position_id: u64,
) -> AppResult<Option<MarginRiskPositionRow>> {
    sqlx::query_as::<_, MarginRiskPositionRow>(
        r#"SELECT positions.id, positions.pair_id, pairs.symbol, positions.margin_asset,
                  positions.direction, positions.margin_amount, positions.notional_amount,
                  positions.interest_amount, positions.entry_price,
                  products.maintenance_margin_rate, positions.status
           FROM margin_positions positions
           INNER JOIN margin_products products ON products.id = positions.product_id
           INNER JOIN trading_pairs pairs ON pairs.id = positions.pair_id
           WHERE positions.id = ? AND positions.user_id = ?
           LIMIT 1"#,
    )
    .bind(position_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

async fn ensure_pair_exists(tx: &mut Transaction<'_, MySql>, pair_id: u64) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, u64>("SELECT id FROM trading_pairs WHERE id = ? LIMIT 1")
        .bind(pair_id)
        .fetch_optional(&mut **tx)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }
    Ok(())
}

async fn ensure_asset_exists(tx: &mut Transaction<'_, MySql>, asset_id: u64) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, u64>("SELECT id FROM assets WHERE id = ? LIMIT 1")
        .bind(asset_id)
        .fetch_optional(&mut **tx)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }
    Ok(())
}

async fn cached_entry_price(
    redis: Option<&ConnectionManager>,
    pair_id: u64,
    symbol: &str,
) -> AppResult<Option<BigDecimal>> {
    let Some(redis) = redis else {
        return Ok(None);
    };
    let ticker = cached_ticker_price(redis, symbol).await?;
    if ticker.last_price <= 0 {
        return Err(AppError::Validation(format!(
            "margin entry price must be positive for pair {pair_id}"
        )));
    }
    if ticker.observed_at < Utc::now() - chrono::TimeDelta::seconds(60) {
        return Err(AppError::Validation(format!(
            "margin entry ticker is stale for pair {pair_id}"
        )));
    }
    Ok(Some(ticker.last_price))
}

async fn cached_mark_price(
    redis: Option<&ConnectionManager>,
    pair_id: u64,
    symbol: &str,
) -> AppResult<BigDecimal> {
    let ticker = cached_valid_margin_ticker(
        redis,
        pair_id,
        symbol,
        "cached ticker is required to close margin position",
        "margin close ticker",
    )
    .await?;
    Ok(ticker.last_price)
}

async fn cached_margin_risk_ticker(
    redis: Option<&ConnectionManager>,
    pair_id: u64,
    symbol: &str,
) -> AppResult<CachedTickerPayload> {
    cached_valid_margin_ticker(
        redis,
        pair_id,
        symbol,
        "cached ticker is required for margin risk snapshot",
        "margin risk ticker",
    )
    .await
}

async fn cached_valid_margin_ticker(
    redis: Option<&ConnectionManager>,
    pair_id: u64,
    symbol: &str,
    missing_message: &str,
    label: &str,
) -> AppResult<CachedTickerPayload> {
    let Some(redis) = redis else {
        return Err(AppError::Validation(format!(
            "{missing_message} for pair {pair_id}"
        )));
    };
    let ticker = cached_ticker_price(redis, symbol).await?;
    if ticker.last_price <= 0 {
        return Err(AppError::Validation(format!(
            "{label} price must be positive for pair {pair_id}"
        )));
    }
    if ticker.observed_at < Utc::now() - chrono::TimeDelta::seconds(60) {
        return Err(AppError::Validation(format!(
            "{label} is stale for pair {pair_id}"
        )));
    }
    Ok(ticker)
}

async fn cached_ticker_price(
    redis: &ConnectionManager,
    symbol: &str,
) -> AppResult<CachedTickerPayload> {
    let mut connection = redis.clone();
    let payload: Option<String> = connection.get(market_ticker_redis_key(symbol)).await?;
    let payload = payload.ok_or_else(|| {
        AppError::Validation("cached ticker is required to open margin positions".to_owned())
    })?;
    serde_json::from_str::<CachedTickerPayload>(&payload).map_err(|error| {
        AppError::Internal(format!("invalid cached margin ticker payload: {error}"))
    })
}

async fn load_product_by_id(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<MarginProductResponse> {
    sqlx::query_as::<_, MarginProductResponse>(
        r#"SELECT products.id, products.pair_id, pairs.symbol,
                  products.margin_asset, assets.symbol AS margin_asset_symbol,
                  products.margin_mode, products.leverage_levels, products.max_leverage,
                  products.min_margin, products.max_margin, products.maintenance_margin_rate,
                  products.hourly_interest_rate, products.status
           FROM margin_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           INNER JOIN assets ON assets.id = products.margin_asset
           WHERE products.id = ?
           LIMIT 1"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

async fn lock_product_by_id(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<MarginProductResponse> {
    sqlx::query_as::<_, MarginProductResponse>(
        r#"SELECT products.id, products.pair_id, pairs.symbol,
                  products.margin_asset, assets.symbol AS margin_asset_symbol,
                  products.margin_mode, products.leverage_levels, products.max_leverage,
                  products.min_margin, products.max_margin, products.maintenance_margin_rate,
                  products.hourly_interest_rate, products.status
           FROM margin_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           INNER JOIN assets ON assets.id = products.margin_asset
           WHERE products.id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

async fn lock_active_product(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<MarginProductRuleRow> {
    let product = sqlx::query_as::<_, MarginProductRuleRow>(
        r#"SELECT products.id, products.pair_id, pairs.symbol, products.margin_asset,
                  products.margin_mode, products.leverage_levels, products.min_margin,
                  products.max_margin, products.hourly_interest_rate, products.status
           FROM margin_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           WHERE products.id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if product.status != "active" {
        return Err(AppError::NotFound);
    }
    Ok(product)
}

async fn lock_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<MarginWalletRow> {
    sqlx::query_as::<_, MarginWalletRow>(
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
    .ok_or_else(|| AppError::Validation("wallet account is required for margin".to_owned()))
}

async fn insert_admin_audit_log_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    action: &str,
    target_id: u64,
    before_json: Option<Value>,
    after_json: Option<Value>,
    reason: Option<String>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, before_json, after_json, reason)
           VALUES (?, ?, 'margin_product', ?, ?, ?, ?)"#,
    )
    .bind(admin_id)
    .bind(action)
    .bind(target_id.to_string())
    .bind(before_json.map(SqlxJson))
    .bind(after_json.map(SqlxJson))
    .bind(optional_string(reason))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn product_audit_json(product: &MarginProductResponse) -> Value {
    json!({
        "id": product.id,
        "pair_id": product.pair_id,
        "symbol": product.symbol,
        "margin_asset": product.margin_asset,
        "margin_asset_symbol": product.margin_asset_symbol,
        "margin_mode": product.margin_mode,
        "leverage_levels": product.leverage_levels.0,
        "max_leverage": product.max_leverage,
        "min_margin": product.min_margin,
        "max_margin": product.max_margin,
        "maintenance_margin_rate": product.maintenance_margin_rate,
        "hourly_interest_rate": product.hourly_interest_rate,
        "status": product.status,
    })
}

async fn load_position_by_id(
    tx: &mut Transaction<'_, MySql>,
    position_id: u64,
) -> AppResult<MarginPositionResponse> {
    sqlx::query_as::<_, MarginPositionResponse>(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, margin_mode, direction, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
                  exit_price, realized_pnl, closed_at, status, idempotency_key
           FROM margin_positions
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(position_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

fn margin_realized_pnl(
    direction: &str,
    notional_amount: &BigDecimal,
    entry_price: &BigDecimal,
    mark_price: &BigDecimal,
) -> AppResult<BigDecimal> {
    validate_positive_decimal(entry_price, "entry price")?;
    validate_positive_decimal(mark_price, "mark price")?;
    let price_delta = match direction {
        "long" => mark_price.clone() - entry_price.clone(),
        "short" => entry_price.clone() - mark_price.clone(),
        _ => {
            return Err(AppError::Validation(
                "margin direction must be long or short".to_owned(),
            ));
        }
    };
    Ok((notional_amount.clone() * price_delta / entry_price.clone()).with_scale(18))
}

fn margin_borrowed_amount(notional_amount: &BigDecimal, margin_amount: &BigDecimal) -> BigDecimal {
    non_negative_amount(&(notional_amount.clone() - margin_amount.clone()))
}

fn margin_position_payout_amount(
    margin_amount: &BigDecimal,
    realized_pnl: Option<&BigDecimal>,
    interest_amount: &BigDecimal,
) -> String {
    realized_pnl
        .map(|pnl| {
            decimal_amount_string(&margin_payout_amount(margin_amount, pnl, interest_amount))
        })
        .unwrap_or_else(|| decimal_amount_string(&zero_amount()))
}

fn margin_payout_amount(
    margin_amount: &BigDecimal,
    realized_pnl: &BigDecimal,
    interest_amount: &BigDecimal,
) -> BigDecimal {
    non_negative_amount(&(margin_amount.clone() + realized_pnl.clone() - interest_amount.clone()))
}

fn serialize_decimal_amount<S>(amount: &BigDecimal, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&decimal_amount_string(amount))
}

fn decimal_amount_string(amount: &BigDecimal) -> String {
    format!("{amount:.18}")
}

fn non_negative_amount(amount: &BigDecimal) -> BigDecimal {
    if amount > &BigDecimal::from(0) {
        amount.clone().with_scale(18)
    } else {
        BigDecimal::from(0).with_scale(18)
    }
}

fn ensure_existing_position_matches_request(
    existing: &MarginPositionResponse,
    product_id: u64,
    direction: &str,
    margin_amount: &BigDecimal,
    leverage: &BigDecimal,
) -> AppResult<()> {
    if existing.product_id != product_id
        || existing.direction != direction
        || existing.margin_amount != *margin_amount
        || existing.leverage != *leverage
    {
        return Err(AppError::Conflict(
            "margin idempotency key belongs to a different request".to_owned(),
        ));
    }
    Ok(())
}

fn validate_create_product_request(request: &CreateMarginProductRequest) -> AppResult<()> {
    let margin_mode = request.margin_mode.as_deref().unwrap_or("isolated");
    normalized_margin_mode(margin_mode)?;
    validated_leverage_levels(&request.max_leverage, request.leverage_levels.as_deref())?;
    if request.pair_id == 0 {
        return Err(AppError::Validation("pair_id is required".to_owned()));
    }
    if request.margin_asset == 0 {
        return Err(AppError::Validation("margin_asset is required".to_owned()));
    }
    validate_max_leverage(&request.max_leverage)?;
    validate_margin_amount(&request.min_margin)?;
    if let Some(max_margin) = &request.max_margin {
        validate_margin_amount(max_margin)?;
        if max_margin < &request.min_margin {
            return Err(AppError::Validation(
                "margin product max_margin must be greater than or equal to min_margin".to_owned(),
            ));
        }
    }
    validate_maintenance_margin_rate(&request.maintenance_margin_rate)?;
    if let Some(hourly_interest_rate) = &request.hourly_interest_rate {
        validate_hourly_interest_rate(hourly_interest_rate)?;
    }
    if let Some(status) = request.status.as_deref() {
        normalized_product_status(status)?;
    }
    validate_reason_len(request.reason.as_deref())?;
    Ok(())
}

fn validate_product_margin(
    margin_amount: &BigDecimal,
    leverage: &BigDecimal,
    product: &MarginProductRuleRow,
) -> AppResult<()> {
    if product.status != "active" {
        return Err(AppError::NotFound);
    }
    if margin_amount < &product.min_margin {
        return Err(AppError::Validation(
            "margin amount is below product minimum".to_owned(),
        ));
    }
    if let Some(max_margin) = &product.max_margin
        && margin_amount > max_margin
    {
        return Err(AppError::Validation(
            "margin amount exceeds product maximum".to_owned(),
        ));
    }
    if product.margin_mode == "cross" {
        return Err(AppError::Validation(
            "cross margin opening requires a dedicated margin wallet risk model".to_owned(),
        ));
    }
    if !product
        .leverage_levels
        .0
        .iter()
        .any(|level| decimal_matches_string(leverage, level))
    {
        return Err(AppError::Validation(
            "margin leverage must match a configured product level".to_owned(),
        ));
    }
    validate_hourly_interest_rate(&product.hourly_interest_rate)?;
    Ok(())
}

fn normalize_direction(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "long" => Ok("long".to_owned()),
        "short" => Ok("short".to_owned()),
        _ => Err(AppError::Validation(
            "margin direction must be long or short".to_owned(),
        )),
    }
}

fn validate_positive_decimal(amount: &BigDecimal, label: &str) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        return Err(AppError::Validation(format!(
            "margin {label} must be positive"
        )));
    }
    Ok(())
}

fn normalized_product_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "margin product status is required".to_owned(),
        ));
    };
    match status.as_str() {
        "active" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "margin product status must be active or disabled".to_owned(),
        )),
    }
}

fn normalized_margin_mode(value: &str) -> AppResult<String> {
    let Some(mode) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "margin product margin_mode is required".to_owned(),
        ));
    };
    match mode.as_str() {
        "isolated" | "cross" => Ok(mode),
        _ => Err(AppError::Validation(
            "margin product margin_mode must be isolated or cross".to_owned(),
        )),
    }
}

fn validated_leverage_levels(
    max_leverage: &BigDecimal,
    leverage_levels: Option<&[BigDecimal]>,
) -> AppResult<Vec<String>> {
    validate_max_leverage(max_leverage)?;
    let Some(levels) = leverage_levels else {
        return Ok(vec![decimal_config_string(max_leverage)]);
    };
    if levels.is_empty() {
        return Err(AppError::Validation(
            "margin product leverage_levels must not be empty".to_owned(),
        ));
    }

    let mut seen = BTreeSet::new();
    let mut normalized = Vec::with_capacity(levels.len());
    for level in levels {
        validate_max_leverage(level)?;
        let level_text = decimal_config_string(level);
        if !seen.insert(level_text.clone()) {
            return Err(AppError::Validation(
                "margin product leverage_levels must not contain duplicates".to_owned(),
            ));
        }
        normalized.push(level_text);
    }

    let max_level = levels
        .iter()
        .max_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal))
        .ok_or_else(|| {
            AppError::Validation("margin product leverage_levels must not be empty".to_owned())
        })?;
    if max_level != max_leverage {
        return Err(AppError::Validation(
            "margin product max_leverage must match maximum leverage level".to_owned(),
        ));
    }

    Ok(normalized)
}

fn decimal_config_string(value: &BigDecimal) -> String {
    let normalized = value.normalized().to_string();
    normalized
        .strip_suffix(".0")
        .unwrap_or(&normalized)
        .to_owned()
}

fn decimal_matches_string(value: &BigDecimal, expected: &str) -> bool {
    BigDecimal::from_str(expected)
        .map(|level| &level == value)
        .unwrap_or(false)
}

fn normalized_position_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "margin position status is required".to_owned(),
        ));
    };
    match status.as_str() {
        "opened" | "closed" | "liquidated" => Ok(status),
        _ => Err(AppError::Validation(
            "margin position status must be opened, closed, or liquidated".to_owned(),
        )),
    }
}

fn required_reason(reason: Option<String>) -> AppResult<String> {
    let Some(reason) = optional_string(reason) else {
        return Err(AppError::Validation(
            "margin product reason is required".to_owned(),
        ));
    };
    validate_reason_len(Some(reason.as_str()))?;
    Ok(reason)
}

fn validate_reason_len(reason: Option<&str>) -> AppResult<()> {
    if let Some(reason) = reason
        && reason.trim().chars().count() > MARGIN_AUDIT_REASON_MAX_LEN
    {
        return Err(AppError::Validation(
            "margin product reason is too long".to_owned(),
        ));
    }
    Ok(())
}

fn validate_max_leverage(leverage: &BigDecimal) -> AppResult<()> {
    if leverage <= &BigDecimal::from(1) {
        return Err(AppError::Validation(
            "margin product max_leverage must be greater than 1".to_owned(),
        ));
    }
    validate_decimal_storage(
        leverage,
        MARGIN_RATE_MAX_SCALE,
        MARGIN_RATE_MAX_INTEGER_DIGITS,
        "margin product max_leverage",
    )
}

fn validate_maintenance_margin_rate(rate: &BigDecimal) -> AppResult<()> {
    if rate < &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "margin product maintenance_margin_rate must be non-negative".to_owned(),
        ));
    }
    validate_decimal_storage(
        rate,
        MARGIN_RATE_MAX_SCALE,
        MARGIN_RATE_MAX_INTEGER_DIGITS,
        "margin product maintenance_margin_rate",
    )
}

fn validate_hourly_interest_rate(rate: &BigDecimal) -> AppResult<()> {
    if rate < &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "margin product hourly_interest_rate must be non-negative".to_owned(),
        ));
    }
    validate_decimal_storage(
        rate,
        MARGIN_RATE_MAX_SCALE,
        MARGIN_RATE_MAX_INTEGER_DIGITS,
        "margin product hourly_interest_rate",
    )
}

fn validate_margin_amount(amount: &BigDecimal) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "margin product margin amount must be positive".to_owned(),
        ));
    }
    validate_decimal_storage(
        amount,
        MARGIN_AMOUNT_MAX_SCALE,
        MARGIN_AMOUNT_MAX_INTEGER_DIGITS,
        "margin product margin amount",
    )
}

fn zero_rate() -> BigDecimal {
    BigDecimal::from(0).with_scale(8)
}

fn zero_amount() -> BigDecimal {
    BigDecimal::from(0).with_scale(18)
}

fn validate_decimal_storage(
    value: &BigDecimal,
    max_scale: i64,
    max_integer_digits: usize,
    label: &str,
) -> AppResult<()> {
    let (digits, scale) = value.as_bigint_and_exponent();
    if scale > max_scale {
        return Err(AppError::Validation(format!(
            "{label} supports at most {max_scale} decimal places"
        )));
    }

    let significant_digits = digits
        .to_str_radix(10)
        .trim_start_matches('-')
        .trim_start_matches('0')
        .len();
    let integer_digits = if scale >= 0 {
        significant_digits.saturating_sub(scale as usize)
    } else {
        significant_digits.saturating_add(scale.unsigned_abs() as usize)
    };
    if integer_digits > max_integer_digits {
        return Err(AppError::Validation(format!(
            "{label} exceeds decimal storage precision"
        )));
    }
    Ok(())
}

fn normalize_idempotency_key(value: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "idempotency_key is required for margin positions".to_owned(),
        ));
    }
    if trimmed.len() > 255 {
        return Err(AppError::Validation(
            "idempotency_key is too long for margin positions".to_owned(),
        ));
    }
    Ok(trimmed.to_owned())
}

const MARGIN_AUDIT_REASON_MAX_LEN: usize = 512;
const MARGIN_RATE_MAX_SCALE: i64 = 8;
const MARGIN_RATE_MAX_INTEGER_DIGITS: usize = 10;
const MARGIN_AMOUNT_MAX_SCALE: i64 = 18;
const MARGIN_AMOUNT_MAX_INTEGER_DIGITS: usize = 20;

fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for margin routes".to_owned())
    })
}

fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

fn admin_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("admin:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

fn is_duplicate_key_error(error: &sqlx::Error) -> bool {
    let Some(database_error) = error.as_database_error() else {
        return false;
    };
    matches!(database_error.code().as_deref(), Some("1062"))
        || database_error.message().contains("Duplicate entry")
}
