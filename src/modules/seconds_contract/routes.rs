use crate::{
    error::{AppError, AppResult},
    modules::{
        auth::{AdminAuth, UserAuth},
        events::EventBroadcastMessage,
        market::market_ticker_redis_key,
    },
    state::AppState,
    time::unix_millis,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, patch, post},
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use redis::{AsyncCommands, aio::ConnectionManager};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};

pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/seconds-contracts/products", get(list_active_products))
        .route(
            "/seconds-contracts/orders",
            get(list_orders).post(open_order),
        )
}

pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/seconds-contracts/products",
            get(list_admin_products).post(create_product),
        )
        .route("/seconds-contracts/products/:id", get(get_admin_product))
        .route(
            "/seconds-contracts/products/:id/status",
            patch(update_product_status),
        )
        .route("/seconds-contracts/orders", get(list_admin_orders))
        .route("/seconds-contracts/orders/:id", get(get_admin_order))
        .route("/seconds-contracts/orders/:id/settle", post(settle_order))
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminOrdersQuery {
    limit: Option<u32>,
    user_id: Option<u64>,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenSecondsContractOrderRequest {
    product_id: u64,
    direction: String,
    stake_amount: BigDecimal,
    idempotency_key: String,
}

#[derive(Debug, Deserialize)]
struct CreateSecondsContractProductRequest {
    pair_id: u64,
    stake_asset: u64,
    duration_seconds: u32,
    payout_rate: BigDecimal,
    min_stake: BigDecimal,
    max_stake: Option<BigDecimal>,
    status: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateSecondsContractProductStatusRequest {
    status: String,
    reason: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct SecondsContractProductResponse {
    id: u64,
    pair_id: u64,
    symbol: String,
    stake_asset: u64,
    stake_asset_symbol: String,
    duration_seconds: u32,
    payout_rate: BigDecimal,
    min_stake: BigDecimal,
    max_stake: Option<BigDecimal>,
    status: String,
}

#[derive(Debug, Deserialize)]
struct CachedTickerPayload {
    last_price: BigDecimal,
    #[serde(with = "unix_millis")]
    observed_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct SecondsContractProductsResponse {
    products: Vec<SecondsContractProductResponse>,
}

#[derive(Debug, Serialize)]
struct SecondsContractOrdersResponse {
    orders: Vec<SecondsContractOrderResponse>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
struct SecondsContractOrderResponse {
    id: u64,
    user_id: u64,
    product_id: u64,
    pair_id: u64,
    stake_asset: u64,
    direction: String,
    stake_amount: BigDecimal,
    payout_rate: BigDecimal,
    entry_price: Option<BigDecimal>,
    status: String,
    result: Option<String>,
    idempotency_key: String,
    #[serde(with = "unix_millis")]
    expires_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct OpenSecondsContractOrderResponse {
    order: SecondsContractOrderResponse,
}

#[derive(Debug, Deserialize)]
struct SettleSecondsContractOrderRequest {
    result: String,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct SettleSecondsContractOrderResponse {
    order: SecondsContractOrderResponse,
    payout_amount: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct SecondsContractProductRuleRow {
    id: u64,
    pair_id: u64,
    symbol: String,
    stake_asset: u64,
    duration_seconds: u32,
    payout_rate: BigDecimal,
    min_stake: BigDecimal,
    max_stake: Option<BigDecimal>,
    status: String,
}

#[derive(Debug, sqlx::FromRow)]
struct SecondsContractWalletRow {
    available: BigDecimal,
    frozen: BigDecimal,
    locked: BigDecimal,
}

#[derive(Debug)]
struct AdminSettlementAudit {
    admin_id: u64,
    reason: String,
}

struct AdminAuditEntry<'a> {
    action: &'a str,
    target_type: &'a str,
    target_id: u64,
    before_json: Option<Value>,
    after_json: Option<Value>,
    reason: Option<String>,
}

async fn list_active_products(
    UserAuth(_claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<SecondsContractProductsResponse>> {
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
) -> AppResult<Json<SecondsContractProductsResponse>> {
    list_products(mysql_pool(&state)?, None, route_limit(query.limit)).await
}

async fn get_admin_product(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
) -> AppResult<Json<SecondsContractProductResponse>> {
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    let product = load_product_by_id(&mut tx, product_id).await?;
    tx.commit().await?;
    Ok(Json(product))
}

async fn list_orders(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<SecondsContractOrdersResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let orders = sqlx::query_as::<_, SecondsContractOrderResponse>(
        r#"SELECT id, user_id, product_id, pair_id, stake_asset, direction, stake_amount,
                  payout_rate, entry_price, status, result, idempotency_key, expires_at
           FROM seconds_contract_orders
           WHERE user_id = ?
           ORDER BY created_at DESC, id DESC
           LIMIT ?"#,
    )
    .bind(user_id)
    .bind(route_limit(query.limit) as i64)
    .fetch_all(&mysql_pool(&state)?)
    .await?;
    Ok(Json(SecondsContractOrdersResponse { orders }))
}

async fn list_admin_orders(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminOrdersQuery>,
) -> AppResult<Json<SecondsContractOrdersResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, user_id, product_id, pair_id, stake_asset, direction, stake_amount,
                  payout_rate, entry_price, status, result, idempotency_key, expires_at
           FROM seconds_contract_orders"#,
    );
    let mut has_filter = false;
    if let Some(user_id) = query.user_id {
        builder.push(" WHERE user_id = ");
        builder.push_bind(user_id);
        has_filter = true;
    }
    if let Some(status) = optional_string(query.status) {
        builder.push(if has_filter {
            " AND status = "
        } else {
            " WHERE status = "
        });
        builder.push_bind(status);
    }
    builder.push(" ORDER BY created_at DESC, id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);

    let orders = builder
        .build_query_as::<SecondsContractOrderResponse>()
        .fetch_all(&mysql_pool(&state)?)
        .await?;
    Ok(Json(SecondsContractOrdersResponse { orders }))
}

async fn get_admin_order(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<SecondsContractOrderResponse>> {
    Ok(Json(
        load_order_by_id_from_pool(&mysql_pool(&state)?, order_id).await?,
    ))
}

async fn create_product(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateSecondsContractProductRequest>,
) -> AppResult<Json<SecondsContractProductResponse>> {
    validate_create_product_request(&request)?;
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let status = normalized_product_status(request.status.as_deref().unwrap_or("active"))?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    ensure_pair_exists(&mut tx, request.pair_id).await?;
    ensure_asset_exists(&mut tx, request.stake_asset).await?;
    let product_id = sqlx::query(
        r#"INSERT INTO seconds_contract_products
           (pair_id, stake_asset, duration_seconds, payout_rate, min_stake, max_stake, status)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(request.pair_id)
    .bind(request.stake_asset)
    .bind(request.duration_seconds)
    .bind(&request.payout_rate)
    .bind(&request.min_stake)
    .bind(&request.max_stake)
    .bind(&status)
    .execute(&mut *tx)
    .await?
    .last_insert_id();
    let product = load_product_by_id(&mut tx, product_id).await?;
    insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "seconds_contract_product.create",
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
    Json(request): Json<UpdateSecondsContractProductStatusRequest>,
) -> AppResult<Json<SecondsContractProductResponse>> {
    let status = normalized_product_status(&request.status)?;
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let mut tx = pool.begin().await?;
    let before = lock_product_by_id(&mut tx, product_id).await?;
    sqlx::query("UPDATE seconds_contract_products SET status = ? WHERE id = ?")
        .bind(&status)
        .bind(product_id)
        .execute(&mut *tx)
        .await?;
    let after = load_product_by_id(&mut tx, product_id).await?;
    insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "seconds_contract_product.update_status",
        product_id,
        Some(product_audit_json(&before)),
        Some(product_audit_json(&after)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(after))
}

async fn list_products(
    pool: Pool<MySql>,
    status: Option<&str>,
    limit: u32,
) -> AppResult<Json<SecondsContractProductsResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT products.id, products.pair_id, pairs.symbol,
                  products.stake_asset, assets.symbol AS stake_asset_symbol,
                  products.duration_seconds, products.payout_rate, products.min_stake,
                  products.max_stake, products.status
           FROM seconds_contract_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           INNER JOIN assets ON assets.id = products.stake_asset"#,
    );

    if let Some(status) = status {
        builder.push(" WHERE products.status = ");
        builder.push_bind(status);
    }

    builder.push(" ORDER BY products.id DESC LIMIT ");
    builder.push_bind(limit as i64);

    let products = builder
        .build_query_as::<SecondsContractProductResponse>()
        .fetch_all(&pool)
        .await?;
    Ok(Json(SecondsContractProductsResponse { products }))
}

async fn open_order(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<OpenSecondsContractOrderRequest>,
) -> AppResult<Json<OpenSecondsContractOrderResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let idempotency_key = normalize_idempotency_key(&request.idempotency_key)?;
    let direction = normalize_direction(&request.direction)?;
    validate_stake_amount(&request.stake_amount)?;
    let (order, is_new_order) = open_order_in_tx(
        &state,
        user_id,
        request.product_id,
        direction,
        request.stake_amount,
        idempotency_key,
    )
    .await?;
    let response = OpenSecondsContractOrderResponse { order };
    if is_new_order && let Some(hub) = &state.event_broadcast_hub {
        hub.publish(EventBroadcastMessage::private_user(
            user_id,
            json!({
                "type": "seconds_contract.order.opened",
                "order_id": response.order.id,
                "product_id": response.order.product_id,
                "pair_id": response.order.pair_id,
                "stake_asset": response.order.stake_asset,
                "direction": response.order.direction,
                "stake_amount": response.order.stake_amount,
                "payout_rate": response.order.payout_rate,
                "entry_price": response.order.entry_price,
                "expires_at": response.order.expires_at.timestamp_millis(),
                "status": response.order.status,
            })
            .to_string(),
        ));
    }
    Ok(Json(response))
}

async fn settle_order(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
    Json(request): Json<SettleSecondsContractOrderRequest>,
) -> AppResult<Json<SettleSecondsContractOrderResponse>> {
    let result = normalize_settlement_result(&request.result)?;
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let (response, is_new_settlement) = settle_order_in_tx(
        &mysql_pool(&state)?,
        order_id,
        result,
        Some(AdminSettlementAudit { admin_id, reason }),
    )
    .await?;
    if is_new_settlement && let Some(hub) = &state.event_broadcast_hub {
        hub.publish(EventBroadcastMessage::private_user(
            response.order.user_id,
            json!({
                "type": "seconds_contract.order.settled",
                "order_id": response.order.id,
                "product_id": response.order.product_id,
                "pair_id": response.order.pair_id,
                "stake_asset": response.order.stake_asset,
                "direction": response.order.direction,
                "stake_amount": response.order.stake_amount,
                "payout_amount": response.payout_amount,
                "result": response.order.result,
                "status": response.order.status,
            })
            .to_string(),
        ));
    }
    Ok(Json(response))
}

async fn open_order_in_tx(
    state: &AppState,
    user_id: u64,
    product_id: u64,
    direction: String,
    stake_amount: BigDecimal,
    idempotency_key: String,
) -> AppResult<(SecondsContractOrderResponse, bool)> {
    let pool = mysql_pool(state)?;
    if let Some(existing) =
        existing_order_for_idempotency_key_readonly(&pool, user_id, &idempotency_key).await?
    {
        ensure_existing_order_matches_request(&existing, product_id, &direction, &stake_amount)?;
        return Ok((existing, false));
    }

    let mut tx = pool.begin().await?;
    let product = match lock_active_product(&mut tx, product_id).await {
        Ok(product) => product,
        Err(AppError::NotFound) => {
            tx.rollback().await?;
            if let Some(existing) = replay_existing_order_if_present(
                &pool,
                user_id,
                product_id,
                &direction,
                &stake_amount,
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
    validate_product_stake(&stake_amount, &product)?;
    let entry_price = cached_entry_price(
        state.redis.as_ref(),
        product.pair_id,
        product.symbol.as_str(),
    )
    .await?;
    let expires_at = Utc::now() + chrono::TimeDelta::seconds(product.duration_seconds as i64);
    // 先占用用户幂等键，再锁钱包扣款；并发同 key 请求只会有一个进入扣款路径。
    let order_id = match sqlx::query(
        r#"INSERT INTO seconds_contract_orders
           (user_id, product_id, pair_id, stake_asset, direction, stake_amount,
            payout_rate, entry_price, status, idempotency_key, expires_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'opened', ?, ?)"#,
    )
    .bind(user_id)
    .bind(product.id)
    .bind(product.pair_id)
    .bind(product.stake_asset)
    .bind(&direction)
    .bind(&stake_amount)
    .bind(&product.payout_rate)
    .bind(&entry_price)
    .bind(&idempotency_key)
    .bind(expires_at)
    .execute(&mut *tx)
    .await
    {
        Ok(result) => result.last_insert_id(),
        Err(error) if is_duplicate_key_error(&error) => {
            tx.rollback().await?;
            return replay_existing_order(
                &pool,
                user_id,
                product_id,
                &direction,
                &stake_amount,
                &idempotency_key,
            )
            .await
            .map(|order| (order, false));
        }
        Err(error) => return Err(AppError::Database(error)),
    };

    let wallet = lock_wallet_row(&mut tx, user_id, product.stake_asset).await?;
    if wallet.available < stake_amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for seconds contract: requested {}, available {}, locked {}",
            stake_amount, wallet.available, wallet.locked
        )));
    }
    let available_after = wallet.available.clone() - stake_amount.clone();

    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
        .bind(user_id)
        .bind(product.stake_asset)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, 'seconds_contract_open', ?, 'available', ?, ?, ?, ?, 'seconds_contract_order', ?)"#,
    )
    .bind(user_id)
    .bind(product.stake_asset)
    .bind(-stake_amount.clone())
    .bind(&available_after)
    .bind(&available_after)
    .bind(&wallet.frozen)
    .bind(&wallet.locked)
    .bind(order_id.to_string())
    .execute(&mut *tx)
    .await?;

    let order = load_order_by_id(&mut tx, order_id).await?;
    tx.commit().await?;
    Ok((order, true))
}

async fn settle_order_in_tx(
    pool: &Pool<MySql>,
    order_id: u64,
    result: String,
    admin_audit: Option<AdminSettlementAudit>,
) -> AppResult<(SettleSecondsContractOrderResponse, bool)> {
    let mut tx = pool.begin().await?;
    let order = lock_order_by_id(&mut tx, order_id).await?;
    if order.status == "settled" {
        ensure_existing_settlement_matches(&order, &result)?;
        let payout_amount = settlement_payout_amount(&order, &result);
        tx.commit().await?;
        return Ok((
            SettleSecondsContractOrderResponse {
                order,
                payout_amount,
            },
            false,
        ));
    }
    if order.status != "opened" {
        return Err(AppError::Conflict(
            "seconds contract order is not open for settlement".to_owned(),
        ));
    }

    let before_json = admin_audit
        .as_ref()
        .map(|_| order_audit_json(&order, BigDecimal::from(0)));
    let payout_amount = settlement_payout_amount(&order, &result);

    if payout_amount > 0 {
        let wallet = lock_wallet_row(&mut tx, order.user_id, order.stake_asset).await?;
        let available_after = wallet.available.clone() + payout_amount.clone();
        sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
            .bind(&available_after)
            .bind(order.user_id)
            .bind(order.stake_asset)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            r#"INSERT INTO wallet_ledger
               (user_id, asset_id, change_type, amount, balance_type, balance_after,
                available_after, frozen_after, locked_after, ref_type, ref_id)
               VALUES (?, ?, 'seconds_contract_settle_win', ?, 'available', ?, ?, ?, ?, 'seconds_contract_order', ?)"#,
        )
        .bind(order.user_id)
        .bind(order.stake_asset)
        .bind(&payout_amount)
        .bind(&available_after)
        .bind(&available_after)
        .bind(&wallet.frozen)
        .bind(&wallet.locked)
        .bind(order.id.to_string())
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        "UPDATE seconds_contract_orders SET status = 'settled', result = ?, settled_at = CURRENT_TIMESTAMP(6) WHERE id = ?",
    )
    .bind(&result)
    .bind(order.id)
    .execute(&mut *tx)
    .await?;
    let settled_order = load_order_by_id(&mut tx, order.id).await?;
    if let Some(admin_audit) = admin_audit {
        insert_admin_order_audit_log_in_tx(
            &mut tx,
            admin_audit.admin_id,
            order.id,
            before_json,
            Some(order_audit_json(&settled_order, payout_amount.clone())),
            Some(admin_audit.reason),
        )
        .await?;
    }
    tx.commit().await?;
    Ok((
        SettleSecondsContractOrderResponse {
            order: settled_order,
            payout_amount,
        },
        true,
    ))
}

async fn replay_existing_order(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    direction: &str,
    stake_amount: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<SecondsContractOrderResponse> {
    replay_existing_order_if_present(
        pool,
        user_id,
        product_id,
        direction,
        stake_amount,
        idempotency_key,
    )
    .await?
    .ok_or_else(|| {
        AppError::Conflict("seconds contract idempotency key is being committed".to_owned())
    })
}

async fn replay_existing_order_if_present(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    direction: &str,
    stake_amount: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<Option<SecondsContractOrderResponse>> {
    let mut tx = pool.begin().await?;
    let Some(existing) =
        existing_order_for_idempotency_key(&mut tx, user_id, idempotency_key).await?
    else {
        return Ok(None);
    };
    ensure_existing_order_matches_request(&existing, product_id, direction, stake_amount)?;
    tx.commit().await?;
    Ok(Some(existing))
}

async fn existing_order_for_idempotency_key(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<SecondsContractOrderResponse>> {
    sqlx::query_as::<_, SecondsContractOrderResponse>(
        r#"SELECT id, user_id, product_id, pair_id, stake_asset, direction, stake_amount,
                  payout_rate, entry_price, status, result, idempotency_key, expires_at
           FROM seconds_contract_orders
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

async fn existing_order_for_idempotency_key_readonly(
    pool: &Pool<MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<SecondsContractOrderResponse>> {
    sqlx::query_as::<_, SecondsContractOrderResponse>(
        r#"SELECT id, user_id, product_id, pair_id, stake_asset, direction, stake_amount,
                  payout_rate, entry_price, status, result, idempotency_key, expires_at
           FROM seconds_contract_orders
           WHERE user_id = ? AND idempotency_key = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(idempotency_key)
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
            "seconds contract entry price must be positive for pair {pair_id}"
        )));
    }
    if ticker.observed_at < Utc::now() - chrono::TimeDelta::seconds(60) {
        return Err(AppError::Validation(format!(
            "seconds contract entry ticker is stale for pair {pair_id}"
        )));
    }
    Ok(Some(ticker.last_price))
}

async fn cached_ticker_price(
    redis: &ConnectionManager,
    symbol: &str,
) -> AppResult<CachedTickerPayload> {
    let mut connection = redis.clone();
    let payload: Option<String> = connection.get(market_ticker_redis_key(symbol)).await?;
    let payload = payload.ok_or_else(|| {
        AppError::Validation("cached ticker is required to open seconds contract orders".to_owned())
    })?;
    serde_json::from_str::<CachedTickerPayload>(&payload)
        .map_err(|error| AppError::Internal(format!("invalid cached ticker payload: {error}")))
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

async fn load_product_by_id(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<SecondsContractProductResponse> {
    sqlx::query_as::<_, SecondsContractProductResponse>(
        r#"SELECT products.id, products.pair_id, pairs.symbol,
                  products.stake_asset, assets.symbol AS stake_asset_symbol,
                  products.duration_seconds, products.payout_rate, products.min_stake,
                  products.max_stake, products.status
           FROM seconds_contract_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           INNER JOIN assets ON assets.id = products.stake_asset
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
) -> AppResult<SecondsContractProductResponse> {
    sqlx::query_as::<_, SecondsContractProductResponse>(
        r#"SELECT products.id, products.pair_id, pairs.symbol,
                  products.stake_asset, assets.symbol AS stake_asset_symbol,
                  products.duration_seconds, products.payout_rate, products.min_stake,
                  products.max_stake, products.status
           FROM seconds_contract_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           INNER JOIN assets ON assets.id = products.stake_asset
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
) -> AppResult<SecondsContractProductRuleRow> {
    let product = sqlx::query_as::<_, SecondsContractProductRuleRow>(
        r#"SELECT products.id, products.pair_id, pairs.symbol, products.stake_asset,
                  products.duration_seconds, products.payout_rate, products.min_stake,
                  products.max_stake, products.status
           FROM seconds_contract_products products
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
) -> AppResult<SecondsContractWalletRow> {
    sqlx::query_as::<_, SecondsContractWalletRow>(
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
    .ok_or_else(|| {
        AppError::Validation("wallet account is required for seconds contract".to_owned())
    })
}

async fn load_order_by_id(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
) -> AppResult<SecondsContractOrderResponse> {
    sqlx::query_as::<_, SecondsContractOrderResponse>(seconds_contract_order_by_id_sql())
        .bind(order_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

async fn load_order_by_id_from_pool(
    pool: &Pool<MySql>,
    order_id: u64,
) -> AppResult<SecondsContractOrderResponse> {
    sqlx::query_as::<_, SecondsContractOrderResponse>(seconds_contract_order_by_id_sql())
        .bind(order_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

fn seconds_contract_order_by_id_sql() -> &'static str {
    r#"SELECT id, user_id, product_id, pair_id, stake_asset, direction, stake_amount,
              payout_rate, entry_price, status, result, idempotency_key, expires_at
       FROM seconds_contract_orders
       WHERE id = ?
       LIMIT 1"#
}

async fn lock_order_by_id(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
) -> AppResult<SecondsContractOrderResponse> {
    sqlx::query_as::<_, SecondsContractOrderResponse>(
        r#"SELECT id, user_id, product_id, pair_id, stake_asset, direction, stake_amount,
                  payout_rate, entry_price, status, result, idempotency_key, expires_at
           FROM seconds_contract_orders
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(order_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
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
    insert_admin_audit_log_with_target_in_tx(
        tx,
        admin_id,
        AdminAuditEntry {
            action,
            target_type: "seconds_contract_product",
            target_id,
            before_json,
            after_json,
            reason,
        },
    )
    .await
}

async fn insert_admin_order_audit_log_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    order_id: u64,
    before_json: Option<Value>,
    after_json: Option<Value>,
    reason: Option<String>,
) -> AppResult<()> {
    insert_admin_audit_log_with_target_in_tx(
        tx,
        admin_id,
        AdminAuditEntry {
            action: "seconds_contract_order.settle",
            target_type: "seconds_contract_order",
            target_id: order_id,
            before_json,
            after_json,
            reason,
        },
    )
    .await
}

async fn insert_admin_audit_log_with_target_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    entry: AdminAuditEntry<'_>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, before_json, after_json, reason)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(admin_id)
    .bind(entry.action)
    .bind(entry.target_type)
    .bind(entry.target_id.to_string())
    .bind(entry.before_json.map(SqlxJson))
    .bind(entry.after_json.map(SqlxJson))
    .bind(optional_string(entry.reason))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn product_audit_json(product: &SecondsContractProductResponse) -> Value {
    json!({
        "id": product.id,
        "pair_id": product.pair_id,
        "symbol": product.symbol,
        "stake_asset": product.stake_asset,
        "stake_asset_symbol": product.stake_asset_symbol,
        "duration_seconds": product.duration_seconds,
        "payout_rate": product.payout_rate,
        "min_stake": product.min_stake,
        "max_stake": product.max_stake,
        "status": product.status,
    })
}

fn order_audit_json(order: &SecondsContractOrderResponse, payout_amount: BigDecimal) -> Value {
    json!({
        "id": order.id,
        "user_id": order.user_id,
        "product_id": order.product_id,
        "pair_id": order.pair_id,
        "stake_asset": order.stake_asset,
        "direction": order.direction,
        "stake_amount": order.stake_amount,
        "payout_rate": order.payout_rate,
        "entry_price": order.entry_price,
        "status": order.status,
        "result": order.result,
        "payout_amount": payout_amount,
        "expires_at": order.expires_at.timestamp_millis(),
    })
}

fn ensure_existing_order_matches_request(
    existing: &SecondsContractOrderResponse,
    product_id: u64,
    direction: &str,
    stake_amount: &BigDecimal,
) -> AppResult<()> {
    if existing.product_id != product_id
        || existing.direction != direction
        || existing.stake_amount != *stake_amount
    {
        return Err(AppError::Conflict(
            "seconds contract idempotency key belongs to a different request".to_owned(),
        ));
    }
    Ok(())
}

fn ensure_existing_settlement_matches(
    existing: &SecondsContractOrderResponse,
    result: &str,
) -> AppResult<()> {
    if existing.result.as_deref() != Some(result) {
        return Err(AppError::Conflict(
            "seconds contract order was settled with a different result".to_owned(),
        ));
    }
    Ok(())
}

fn settlement_payout_amount(order: &SecondsContractOrderResponse, result: &str) -> BigDecimal {
    if result == "win" {
        order.stake_amount.clone() + order.stake_amount.clone() * order.payout_rate.clone()
    } else {
        BigDecimal::from(0)
    }
}

fn validate_create_product_request(request: &CreateSecondsContractProductRequest) -> AppResult<()> {
    if request.pair_id == 0 {
        return Err(AppError::Validation("pair_id is required".to_owned()));
    }
    if request.stake_asset == 0 {
        return Err(AppError::Validation("stake_asset is required".to_owned()));
    }
    if request.duration_seconds == 0 {
        return Err(AppError::Validation(
            "seconds contract duration_seconds must be positive".to_owned(),
        ));
    }
    validate_payout_rate(&request.payout_rate)?;
    validate_stake_amount(&request.min_stake)?;
    if let Some(max_stake) = &request.max_stake {
        validate_stake_amount(max_stake)?;
        if max_stake < &request.min_stake {
            return Err(AppError::Validation(
                "seconds contract max_stake must be greater than or equal to min_stake".to_owned(),
            ));
        }
    }
    if let Some(status) = request.status.as_deref() {
        normalized_product_status(status)?;
    }
    validate_reason_len(request.reason.as_deref())?;
    Ok(())
}

fn validate_product_stake(
    stake_amount: &BigDecimal,
    product: &SecondsContractProductRuleRow,
) -> AppResult<()> {
    if product.status != "active" {
        return Err(AppError::NotFound);
    }
    if stake_amount < &product.min_stake {
        return Err(AppError::Validation(
            "seconds contract stake is below product minimum".to_owned(),
        ));
    }
    if let Some(max_stake) = &product.max_stake
        && stake_amount > max_stake
    {
        return Err(AppError::Validation(
            "seconds contract stake exceeds product maximum".to_owned(),
        ));
    }
    Ok(())
}

fn normalize_direction(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "up" => Ok("up".to_owned()),
        "down" => Ok("down".to_owned()),
        _ => Err(AppError::Validation(
            "seconds contract direction must be up or down".to_owned(),
        )),
    }
}

fn normalize_settlement_result(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "win" => Ok("win".to_owned()),
        "loss" => Ok("loss".to_owned()),
        _ => Err(AppError::Validation(
            "seconds contract settlement result must be win or loss".to_owned(),
        )),
    }
}

fn normalized_product_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "seconds contract product status is required".to_owned(),
        ));
    };
    match status.as_str() {
        "active" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "seconds contract product status must be active or disabled".to_owned(),
        )),
    }
}

fn required_reason(reason: Option<String>) -> AppResult<String> {
    let Some(reason) = optional_string(reason) else {
        return Err(AppError::Validation(
            "seconds contract reason is required".to_owned(),
        ));
    };
    validate_reason_len(Some(reason.as_str()))?;
    Ok(reason)
}

fn validate_reason_len(reason: Option<&str>) -> AppResult<()> {
    if let Some(reason) = reason
        && reason.trim().chars().count() > SECONDS_AUDIT_REASON_MAX_LEN
    {
        return Err(AppError::Validation(
            "seconds contract reason is too long".to_owned(),
        ));
    }
    Ok(())
}

fn validate_payout_rate(payout_rate: &BigDecimal) -> AppResult<()> {
    if payout_rate < &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "seconds contract payout_rate must be non-negative".to_owned(),
        ));
    }
    validate_decimal_storage(
        payout_rate,
        SECONDS_RATE_MAX_SCALE,
        SECONDS_RATE_MAX_INTEGER_DIGITS,
        "seconds contract payout_rate",
    )
}

fn validate_stake_amount(amount: &BigDecimal) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "seconds contract stake amount must be positive".to_owned(),
        ));
    }
    validate_decimal_storage(
        amount,
        SECONDS_AMOUNT_MAX_SCALE,
        SECONDS_AMOUNT_MAX_INTEGER_DIGITS,
        "seconds contract stake amount",
    )
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
            "idempotency_key is required for seconds contract orders".to_owned(),
        ));
    }
    if trimmed.len() > 255 {
        return Err(AppError::Validation(
            "idempotency_key is too long for seconds contract orders".to_owned(),
        ));
    }
    Ok(trimmed.to_owned())
}

const SECONDS_AUDIT_REASON_MAX_LEN: usize = 512;
const SECONDS_RATE_MAX_SCALE: i64 = 8;
const SECONDS_RATE_MAX_INTEGER_DIGITS: usize = 10;
const SECONDS_AMOUNT_MAX_SCALE: i64 = 18;
const SECONDS_AMOUNT_MAX_INTEGER_DIGITS: usize = 20;

fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for seconds contract routes".to_owned())
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
