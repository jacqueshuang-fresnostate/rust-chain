use crate::{
    error::{AppError, AppResult},
    modules::{
        auth::{AdminAuth, UserAuth},
        events::EventBroadcastMessage,
        spot::{
            MySqlSpotRepository, NewOrder, NewSpotTrade, OrderSide, OrderStatus, OrderType,
            SpotOrder, SpotTrade, apply_fill, create_limit_order, create_market_order,
            spot_remaining_reserved_amount, spot_reservation_amount, spot_reserve_asset_id,
        },
    },
    state::AppState,
    time::unix_millis,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{delete, get, post},
};
use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/spot/orders", post(create_order).get(list_orders))
        .route("/spot/orders/:id", delete(cancel_order))
        .route("/spot/trades", get(list_trades))
}

pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/spot/orders", get(list_admin_orders))
        .route("/spot/orders/:id", get(get_admin_order))
        .route("/spot/orders/:id/cancel", post(cancel_admin_order))
        .route("/spot/trades", get(list_admin_trades))
        .route("/spot/fills", post(fill_orders))
}

#[derive(Debug, Deserialize)]
struct CreateSpotOrderRequest {
    pair_id: String,
    side: OrderSide,
    order_type: OrderType,
    price: Option<BigDecimal>,
    quantity: BigDecimal,
    reference_price: Option<BigDecimal>,
    idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SpotOrdersQuery {
    pair_id: Option<String>,
    status: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct FillSpotOrdersRequest {
    buy_order_id: String,
    sell_order_id: String,
    price: BigDecimal,
    quantity: BigDecimal,
    idempotency_key: String,
}

#[derive(Debug, Deserialize)]
struct AdminCancelSpotOrderRequest {
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SpotTradesQuery {
    pair_id: String,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminSpotOrdersQuery {
    pair_id: Option<String>,
    status: Option<String>,
    user_id: Option<u64>,
    email: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AdminSpotTradesQuery {
    pair_id: Option<String>,
    user_id: Option<u64>,
    email: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Serialize)]
struct SpotOrderResponse {
    id: String,
    user_id: String,
    pair_id: String,
    side: OrderSide,
    order_type: OrderType,
    price: Option<BigDecimal>,
    quantity: BigDecimal,
    filled_quantity: BigDecimal,
    status: OrderStatus,
}

#[derive(Debug, Serialize)]
struct SpotOrdersResponse {
    orders: Vec<SpotOrderResponse>,
}

#[derive(Debug, Serialize)]
struct SpotCancelResponse {
    order: SpotOrderResponse,
    cancelled: bool,
}

#[derive(Debug, Serialize)]
struct SpotFillResponse {
    buy_order: SpotOrderResponse,
    sell_order: SpotOrderResponse,
    trade: SpotTradeResponse,
}

#[derive(Debug, Serialize)]
struct SpotTradeResponse {
    id: String,
    pair_id: String,
    buy_order_id: String,
    sell_order_id: String,
    price: BigDecimal,
    quantity: BigDecimal,
    fee: BigDecimal,
    #[serde(with = "unix_millis")]
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
struct SpotTradesResponse {
    trades: Vec<SpotTradeResponse>,
}

#[derive(Debug, sqlx::FromRow)]
struct SpotOrderRow {
    id: u64,
    user_id: u64,
    pair_id: String,
    side: String,
    order_type: String,
    price: Option<BigDecimal>,
    quantity: BigDecimal,
    filled_quantity: BigDecimal,
    status: String,
}

#[derive(Debug, sqlx::FromRow)]
struct IdempotentSpotOrderRow {
    id: u64,
    user_id: u64,
    pair_db_id: u64,
    pair_id: String,
    side: String,
    order_type: String,
    price: Option<BigDecimal>,
    quantity: BigDecimal,
    filled_quantity: BigDecimal,
    status: String,
    reserved_amount: Option<BigDecimal>,
    request_reference_price: Option<BigDecimal>,
    request_price: Option<BigDecimal>,
}

#[derive(Debug, sqlx::FromRow)]
struct SpotOrderLockRow {
    id: u64,
    user_id: u64,
    pair_id: u64,
    side: String,
    order_type: String,
    price: Option<BigDecimal>,
    quantity: BigDecimal,
    filled_quantity: BigDecimal,
    status: String,
}

#[derive(Debug, sqlx::FromRow)]
struct SpotPairAssetRow {
    base_asset_id: u64,
    quote_asset_id: u64,
}

#[derive(Debug, sqlx::FromRow)]
struct SpotWalletRow {
    available: BigDecimal,
    frozen: BigDecimal,
    locked: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct SpotTradeRow {
    id: u64,
    pair_id: String,
    buy_order_id: u64,
    sell_order_id: u64,
    price: BigDecimal,
    quantity: BigDecimal,
    fee: BigDecimal,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct SpotOrderReservationRow {
    reserved_asset_id: Option<u64>,
    reserved_amount: Option<BigDecimal>,
}

#[derive(Debug, Clone)]
struct SpotOrderReservation {
    asset_id: u64,
    amount: BigDecimal,
}

#[derive(Debug, Clone, Copy)]
struct SpotLedgerMetadata<'a> {
    change_type: &'a str,
    ref_type: &'a str,
    ref_id: &'a str,
}

struct SpotAdminAuditEntry<'a> {
    action: &'a str,
    target_type: &'a str,
    target_id: &'a str,
    before_json: Option<Value>,
    after_json: Option<Value>,
    reason: Option<String>,
}

async fn create_order(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateSpotOrderRequest>,
) -> AppResult<Json<SpotOrderResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let repository = spot_repository(&state)?;
    if let Some(existing) =
        existing_spot_order_for_idempotency_key(repository.pool(), user_id, &request).await?
    {
        return Ok(Json(existing));
    }
    let pair = repository
        .load_pair_rule_async(&request.pair_id)
        .await
        .map_err(map_spot_error)?;
    let new_order = match request.order_type {
        OrderType::Limit => create_limit_order(
            user_id.to_string(),
            request.side,
            request.price.clone().ok_or_else(|| {
                AppError::Validation("price is required for limit orders".to_owned())
            })?,
            request.quantity,
            &pair,
        ),
        OrderType::Market => create_market_order(
            user_id.to_string(),
            request.side,
            request.quantity,
            request.reference_price.clone().ok_or_else(|| {
                AppError::Validation("reference_price is required for market orders".to_owned())
            })?,
            &pair,
        ),
    }
    .map_err(|error| AppError::Validation(format!("invalid spot order: {error:?}")))?;
    let (inserted, is_new_order) = insert_order_and_freeze_wallet(
        repository.pool(),
        new_order,
        request.idempotency_key.as_deref(),
        request.price.as_ref(),
        request.reference_price.as_ref(),
    )
    .await?;
    let response = SpotOrderResponse::from(inserted);

    if is_new_order && let Some(hub) = &state.event_broadcast_hub {
        hub.publish(EventBroadcastMessage::private_user(
            user_id,
            json!({
                "type": "spot.order.created",
                "order_id": response.id,
                "pair_id": response.pair_id,
                "side": response.side,
                "order_type": response.order_type,
                "status": response.status,
            })
            .to_string(),
        ));
    }

    Ok(Json(response))
}

async fn list_orders(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<SpotOrdersQuery>,
) -> AppResult<Json<SpotOrdersResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let mut builder = base_spot_orders_query();
    builder.push(" WHERE orders.user_id = ");
    builder.push_bind(user_id);
    push_spot_order_filters(&mut builder, query.pair_id, query.status, None, None, true);
    builder.push(" ORDER BY orders.id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);
    fetch_spot_orders(pool, builder).await
}

async fn list_admin_orders(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminSpotOrdersQuery>,
) -> AppResult<Json<SpotOrdersResponse>> {
    let pool = mysql_pool(&state)?;
    let mut builder = base_spot_orders_query();
    push_spot_order_filters(
        &mut builder,
        query.pair_id,
        query.status,
        query.user_id,
        query.email,
        false,
    );
    builder.push(" ORDER BY orders.created_at DESC, orders.id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);
    fetch_spot_orders(pool, builder).await
}

async fn get_admin_order(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<SpotOrderResponse>> {
    Ok(Json(
        load_spot_order_by_id(&mysql_pool(&state)?, order_id).await?,
    ))
}

async fn cancel_admin_order(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
    Json(request): Json<AdminCancelSpotOrderRequest>,
) -> AppResult<Json<SpotCancelResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let reason = required_reason(request.reason)?;
    let repository = spot_repository(&state)?;
    let (order, cancelled) =
        cancel_spot_order_by_admin(repository.pool(), order_id, admin_id, reason).await?;
    let response = SpotCancelResponse {
        order: order.into(),
        cancelled,
    };

    if cancelled && let Some(hub) = &state.event_broadcast_hub {
        let user_id = response
            .order
            .user_id
            .parse::<u64>()
            .map_err(|_| AppError::Unauthorized)?;
        publish_spot_cancel_private_event(hub, user_id, &response.order);
    }

    Ok(Json(response))
}

fn base_spot_orders_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT orders.id, orders.user_id, pairs.symbol AS pair_id, orders.side,
                  orders.order_type, orders.price, orders.quantity, orders.filled_quantity,
                  orders.status
           FROM spot_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id"#,
    )
}

fn push_spot_order_filters(
    builder: &mut QueryBuilder<'_, MySql>,
    pair_id: Option<String>,
    status: Option<String>,
    user_id: Option<u64>,
    email: Option<String>,
    mut has_filter: bool,
) -> bool {
    if let Some(pair_id) = optional_query_string(pair_id) {
        builder.push(if has_filter { " AND " } else { " WHERE " });
        builder.push("pairs.symbol = ");
        builder.push_bind(pair_id);
        has_filter = true;
    }
    if let Some(status) = optional_query_string(status) {
        builder.push(if has_filter { " AND " } else { " WHERE " });
        builder.push("orders.status = ");
        builder.push_bind(status);
        has_filter = true;
    }
    if let Some(user_id) = user_id {
        builder.push(if has_filter { " AND " } else { " WHERE " });
        builder.push("orders.user_id = ");
        builder.push_bind(user_id);
        has_filter = true;
    }
    if let Some(email) = optional_query_string(email) {
        builder.push(if has_filter { " AND " } else { " WHERE " });
        builder
            .push("EXISTS (SELECT 1 FROM users WHERE users.id = orders.user_id AND users.email = ");
        builder.push_bind(email);
        builder.push(")");
        has_filter = true;
    }
    has_filter
}

async fn fetch_spot_orders(
    pool: Pool<MySql>,
    mut builder: QueryBuilder<'_, MySql>,
) -> AppResult<Json<SpotOrdersResponse>> {
    let rows = builder
        .build_query_as::<SpotOrderRow>()
        .fetch_all(&pool)
        .await?;
    let orders = rows.into_iter().map(SpotOrderResponse::from).collect();
    Ok(Json(SpotOrdersResponse { orders }))
}

async fn load_spot_order_by_id(pool: &Pool<MySql>, order_id: u64) -> AppResult<SpotOrderResponse> {
    let mut builder = base_spot_orders_query();
    builder.push(" WHERE orders.id = ");
    builder.push_bind(order_id);
    builder
        .build_query_as::<SpotOrderRow>()
        .fetch_optional(pool)
        .await?
        .map(SpotOrderResponse::from)
        .ok_or(AppError::NotFound)
}

async fn cancel_order(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<SpotCancelResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let repository = spot_repository(&state)?;
    let (order, cancelled) =
        cancel_spot_order_and_unfreeze_wallet(repository.pool(), order_id, user_id).await?;
    let response = SpotCancelResponse {
        order: order.into(),
        cancelled,
    };

    if cancelled && let Some(hub) = &state.event_broadcast_hub {
        publish_spot_cancel_private_event(hub, user_id, &response.order);
    }

    Ok(Json(response))
}

fn publish_spot_cancel_private_event(
    hub: &crate::modules::events::EventBroadcastHub,
    user_id: u64,
    order: &SpotOrderResponse,
) {
    hub.publish(EventBroadcastMessage::private_user(
        user_id,
        json!({
            "type": "spot.order.cancelled",
            "order_id": order.id,
            "pair_id": order.pair_id,
            "status": order.status,
        })
        .to_string(),
    ));
}

async fn list_trades(
    UserAuth(_claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<SpotTradesQuery>,
) -> AppResult<Json<SpotTradesResponse>> {
    let repository = spot_repository(&state)?;
    let trades = repository
        .list_trades_by_pair_async(&query.pair_id, route_limit(query.limit))
        .await
        .map_err(map_spot_error)?
        .into_iter()
        .map(SpotTradeResponse::from)
        .collect();

    Ok(Json(SpotTradesResponse { trades }))
}

async fn list_admin_trades(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminSpotTradesQuery>,
) -> AppResult<Json<SpotTradesResponse>> {
    let pool = mysql_pool(&state)?;
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT trades.id, pairs.symbol AS pair_id, trades.buy_order_id, trades.sell_order_id,
                  trades.price, trades.quantity, trades.fee, trades.created_at
           FROM spot_trades trades
           INNER JOIN trading_pairs pairs ON pairs.id = trades.pair_id
           INNER JOIN spot_orders buy_orders ON buy_orders.id = trades.buy_order_id
           INNER JOIN spot_orders sell_orders ON sell_orders.id = trades.sell_order_id"#,
    );
    let mut has_filter = false;
    if let Some(pair_id) = optional_query_string(query.pair_id) {
        builder.push(" WHERE pairs.symbol = ");
        builder.push_bind(pair_id);
        has_filter = true;
    }
    if let Some(user_id) = query.user_id {
        builder.push(if has_filter { " AND " } else { " WHERE " });
        builder.push("(buy_orders.user_id = ");
        builder.push_bind(user_id);
        builder.push(" OR sell_orders.user_id = ");
        builder.push_bind(user_id);
        builder.push(")");
        has_filter = true;
    }
    if let Some(email) = optional_query_string(query.email) {
        builder.push(if has_filter { " AND " } else { " WHERE " });
        builder.push(
            r#"EXISTS (
                   SELECT 1 FROM users
                   WHERE users.email = "#,
        );
        builder.push_bind(email);
        builder.push(" AND (users.id = buy_orders.user_id OR users.id = sell_orders.user_id))");
    }
    builder.push(" ORDER BY trades.created_at DESC, trades.id DESC LIMIT ");
    builder.push_bind(route_limit(query.limit) as i64);

    let rows = builder
        .build_query_as::<SpotTradeRow>()
        .fetch_all(&pool)
        .await?;
    let trades = rows
        .into_iter()
        .map(SpotTrade::from)
        .map(SpotTradeResponse::from)
        .collect();
    Ok(Json(SpotTradesResponse { trades }))
}

async fn fill_orders(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<FillSpotOrdersRequest>,
) -> AppResult<Json<SpotFillResponse>> {
    ensure_positive_amount(&request.price, "price")?;
    ensure_positive_amount(&request.quantity, "quantity")?;
    let idempotency_key = optional_query_string(Some(request.idempotency_key))
        .ok_or_else(|| AppError::Validation("idempotency_key is required".to_owned()))?;
    let repository = spot_repository(&state)?;
    let (buy_order, sell_order, trade, is_new_trade) = settle_spot_fill(
        repository.pool(),
        &request.buy_order_id,
        &request.sell_order_id,
        &request.price,
        &request.quantity,
        &idempotency_key,
    )
    .await?;
    let response = SpotFillResponse {
        buy_order: buy_order.into(),
        sell_order: sell_order.into(),
        trade: trade.into(),
    };

    if is_new_trade && let Some(hub) = &state.event_broadcast_hub {
        publish_spot_fill_private_events(hub, &response)?;
    }

    Ok(Json(response))
}

fn publish_spot_fill_private_events(
    hub: &crate::modules::events::EventBroadcastHub,
    response: &SpotFillResponse,
) -> AppResult<()> {
    let trade = &response.trade;
    publish_spot_fill_private_event(hub, &response.buy_order, &response.sell_order, trade, "buy")?;
    publish_spot_fill_private_event(
        hub,
        &response.sell_order,
        &response.buy_order,
        trade,
        "sell",
    )?;
    Ok(())
}

fn publish_spot_fill_private_event(
    hub: &crate::modules::events::EventBroadcastHub,
    order: &SpotOrderResponse,
    counterparty_order: &SpotOrderResponse,
    trade: &SpotTradeResponse,
    side: &str,
) -> AppResult<()> {
    let user_id = order
        .user_id
        .parse::<u64>()
        .map_err(|_| AppError::Unauthorized)?;
    hub.publish(EventBroadcastMessage::private_user(
        user_id,
        json!({
            "type": "spot.trade.filled",
            "trade_id": trade.id,
            "order_id": order.id,
            "counterparty_order_id": counterparty_order.id,
            "pair_id": trade.pair_id,
            "side": side,
            "price": trade.price,
            "quantity": trade.quantity,
            "order_status": order.status,
        })
        .to_string(),
    ));
    Ok(())
}

fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for spot routes".to_owned())
    })
}

fn spot_repository(state: &AppState) -> AppResult<MySqlSpotRepository> {
    Ok(MySqlSpotRepository::new(mysql_pool(state)?))
}

async fn settle_spot_fill(
    pool: &Pool<MySql>,
    buy_order_id: &str,
    sell_order_id: &str,
    price: &BigDecimal,
    quantity: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<(SpotOrder, SpotOrder, SpotTrade, bool)> {
    let mut tx = pool.begin().await?;
    let (mut buy_order, mut sell_order) =
        lock_spot_fill_orders_in_order(&mut tx, buy_order_id, sell_order_id).await?;
    ensure_fill_orders_match(&buy_order, &sell_order)?;
    if let Some(trade) = existing_spot_trade(&mut tx, idempotency_key).await? {
        ensure_existing_spot_trade_matches_request(
            &trade,
            &buy_order.id,
            &sell_order.id,
            price,
            quantity,
        )?;
        tx.commit().await?;
        return Ok((buy_order, sell_order, trade, false));
    }
    ensure_fill_price_matches_limits(&buy_order, &sell_order, price)?;
    let (base_asset_id, quote_asset_id): (u64, u64) = sqlx::query_as(
        "SELECT base_asset, quote_asset FROM trading_pairs WHERE symbol = ? LIMIT 1",
    )
    .bind(&buy_order.pair_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;
    let buyer_id = buy_order
        .user_id
        .parse::<u64>()
        .map_err(|_| AppError::Unauthorized)?;
    let seller_id = sell_order
        .user_id
        .parse::<u64>()
        .map_err(|_| AppError::Unauthorized)?;
    let fill_quote_amount = price.clone() * quantity.clone();
    // 成交幂等键先占位再锁钱包，避免重复键事务和钱包结算互相等待造成死锁或 500。
    let trade = match insert_spot_trade(
        &mut tx,
        &buy_order,
        &sell_order,
        price,
        quantity,
        idempotency_key,
    )
    .await
    {
        Ok(trade) => trade,
        Err(AppError::Database(error)) if is_duplicate_key_error(&error) => {
            tx.rollback().await?;
            return replay_existing_spot_fill(
                pool,
                buy_order_id,
                sell_order_id,
                price,
                quantity,
                idempotency_key,
            )
            .await;
        }
        Err(error) => return Err(error),
    };
    lock_spot_fill_wallet_rows_in_order(
        &mut tx,
        buyer_id,
        seller_id,
        base_asset_id,
        quote_asset_id,
    )
    .await?;
    let buy_order_remaining_reservation =
        remaining_spot_fill_reservation_before_trade_in_tx(&mut tx, &buy_order, &trade.id).await?;
    ensure_spot_fill_within_order_reservation(
        &buy_order_remaining_reservation,
        &fill_quote_amount,
        OrderSide::Buy,
    )?;
    let sell_order_remaining_reservation =
        remaining_spot_fill_reservation_before_trade_in_tx(&mut tx, &sell_order, &trade.id).await?;
    ensure_spot_fill_within_order_reservation(
        &sell_order_remaining_reservation,
        quantity,
        OrderSide::Sell,
    )?;
    apply_fill(&mut buy_order, quantity.clone())
        .map_err(|error| AppError::Validation(format!("invalid spot buy fill: {error:?}")))?;
    apply_fill(&mut sell_order, quantity.clone())
        .map_err(|error| AppError::Validation(format!("invalid spot sell fill: {error:?}")))?;
    let ref_id = format!("{}:{}", buy_order.id, sell_order.id);
    let ledger = SpotLedgerMetadata {
        change_type: "spot_trade_settlement",
        ref_type: "spot_trade",
        ref_id: &ref_id,
    };

    apply_spot_wallet_settlement_leg(
        &mut tx,
        buyer_id,
        quote_asset_id,
        &fill_quote_amount,
        false,
        ledger,
    )
    .await?;
    apply_spot_wallet_settlement_leg(&mut tx, buyer_id, base_asset_id, quantity, true, ledger)
        .await?;
    apply_spot_wallet_settlement_leg(&mut tx, seller_id, base_asset_id, quantity, false, ledger)
        .await?;
    apply_spot_wallet_settlement_leg(
        &mut tx,
        seller_id,
        quote_asset_id,
        &fill_quote_amount,
        true,
        ledger,
    )
    .await?;
    release_buy_order_surplus_reservation_after_fill(
        &mut tx,
        buyer_id,
        &buy_order,
        &buy_order_remaining_reservation,
        &fill_quote_amount,
        &ref_id,
    )
    .await?;

    save_spot_order(&mut tx, &buy_order).await?;
    save_spot_order(&mut tx, &sell_order).await?;
    tx.commit().await?;
    Ok((buy_order, sell_order, trade, true))
}

async fn replay_existing_spot_fill(
    pool: &Pool<MySql>,
    buy_order_id: &str,
    sell_order_id: &str,
    price: &BigDecimal,
    quantity: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<(SpotOrder, SpotOrder, SpotTrade, bool)> {
    let mut tx = pool.begin().await?;
    let (buy_order, sell_order) =
        lock_spot_fill_orders_in_order(&mut tx, buy_order_id, sell_order_id).await?;
    let trade = existing_spot_trade(&mut tx, idempotency_key)
        .await?
        .ok_or_else(|| {
            AppError::Conflict("spot fill idempotency key is being committed".to_owned())
        })?;
    ensure_existing_spot_trade_matches_request(
        &trade,
        &buy_order.id,
        &sell_order.id,
        price,
        quantity,
    )?;
    tx.commit().await?;
    Ok((buy_order, sell_order, trade, false))
}

async fn existing_spot_trade(
    tx: &mut Transaction<'_, MySql>,
    idempotency_key: &str,
) -> AppResult<Option<SpotTrade>> {
    let trade = sqlx::query_as::<_, SpotTradeRow>(
        r#"SELECT trades.id, pairs.symbol AS pair_id, trades.buy_order_id, trades.sell_order_id,
                  trades.price, trades.quantity, trades.fee, trades.created_at
           FROM spot_trades trades
           INNER JOIN trading_pairs pairs ON pairs.id = trades.pair_id
           WHERE trades.idempotency_key = ?
           LIMIT 1"#,
    )
    .bind(idempotency_key)
    .fetch_optional(&mut **tx)
    .await?
    .map(SpotTrade::from);
    Ok(trade)
}

async fn lock_spot_fill_orders_in_order(
    tx: &mut Transaction<'_, MySql>,
    buy_order_id: &str,
    sell_order_id: &str,
) -> AppResult<(SpotOrder, SpotOrder)> {
    let buy_order_db_id = parse_spot_order_request_id(buy_order_id)?;
    let sell_order_db_id = parse_spot_order_request_id(sell_order_id)?;
    let mut buy_order = None;
    let mut sell_order = None;
    // 成交会同时锁买卖订单，先按订单主键稳定加锁，再映射回请求角色，避免 A/B 与 B/A 请求互相等待。
    for order_db_id in spot_fill_order_lock_keys(buy_order_id, sell_order_id)? {
        let order = lock_spot_order_by_db_id(tx, order_db_id).await?;
        if order_db_id == buy_order_db_id {
            buy_order = Some(order.clone());
        }
        if order_db_id == sell_order_db_id {
            sell_order = Some(order);
        }
    }
    Ok((
        buy_order.ok_or(AppError::NotFound)?,
        sell_order.ok_or(AppError::NotFound)?,
    ))
}

fn spot_fill_order_lock_keys(buy_order_id: &str, sell_order_id: &str) -> AppResult<Vec<u64>> {
    let mut keys = vec![
        parse_spot_order_request_id(buy_order_id)?,
        parse_spot_order_request_id(sell_order_id)?,
    ];
    keys.sort_unstable();
    keys.dedup();
    Ok(keys)
}

fn parse_spot_order_request_id(order_id: &str) -> AppResult<u64> {
    order_id
        .parse::<u64>()
        .map_err(|_| AppError::Validation("invalid spot order id".to_owned()))
}

async fn lock_spot_order(tx: &mut Transaction<'_, MySql>, order_id: &str) -> AppResult<SpotOrder> {
    let order_db_id = parse_spot_order_request_id(order_id)?;
    lock_spot_order_by_db_id(tx, order_db_id).await
}

async fn lock_spot_order_by_db_id(
    tx: &mut Transaction<'_, MySql>,
    order_db_id: u64,
) -> AppResult<SpotOrder> {
    let row = sqlx::query_as::<_, SpotOrderLockRow>(
        r#"SELECT id, user_id, pair_id, side, order_type, price, quantity,
                  filled_quantity, status
           FROM spot_orders
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(order_db_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    let pair_symbol = spot_pair_symbol_in_tx(tx, row.pair_id).await?;
    Ok(locked_spot_order_response(row, pair_symbol).into())
}

async fn spot_pair_symbol_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<String> {
    let (symbol,): (String,) = sqlx::query_as(
        r#"SELECT symbol
           FROM trading_pairs
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(pair_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(symbol)
}

fn locked_spot_order_response(order: SpotOrderLockRow, pair_symbol: String) -> SpotOrderResponse {
    SpotOrderResponse {
        id: order.id.to_string(),
        user_id: order.user_id.to_string(),
        pair_id: pair_symbol,
        side: parse_order_side(&order.side),
        order_type: parse_order_type(&order.order_type),
        price: order.price,
        quantity: order.quantity,
        filled_quantity: order.filled_quantity,
        status: parse_order_status(&order.status),
    }
}

fn ensure_fill_orders_match(buy_order: &SpotOrder, sell_order: &SpotOrder) -> AppResult<()> {
    if buy_order.side != OrderSide::Buy || sell_order.side != OrderSide::Sell {
        return Err(AppError::Validation(
            "spot fill requires buy_order_id to be buy and sell_order_id to be sell".to_owned(),
        ));
    }
    if buy_order.pair_id != sell_order.pair_id {
        return Err(AppError::Validation(
            "spot fill orders must belong to the same pair".to_owned(),
        ));
    }
    Ok(())
}

fn ensure_fill_price_matches_limits(
    buy_order: &SpotOrder,
    sell_order: &SpotOrder,
    fill_price: &BigDecimal,
) -> AppResult<()> {
    if let Some(buy_limit) = buy_order.price.as_ref()
        && fill_price > buy_limit
    {
        return Err(AppError::Validation(
            "spot fill price exceeds buy limit".to_owned(),
        ));
    }
    if let Some(sell_limit) = sell_order.price.as_ref()
        && fill_price < sell_limit
    {
        return Err(AppError::Validation(
            "spot fill price is below sell limit".to_owned(),
        ));
    }
    Ok(())
}

fn ensure_existing_spot_trade_matches_request(
    trade: &SpotTrade,
    buy_order_id: &str,
    sell_order_id: &str,
    price: &BigDecimal,
    quantity: &BigDecimal,
) -> AppResult<()> {
    if trade.buy_order_id != buy_order_id
        || trade.sell_order_id != sell_order_id
        || trade.price != *price
        || trade.quantity != *quantity
    {
        return Err(AppError::Conflict(
            "spot fill idempotency key belongs to a different fill request".to_owned(),
        ));
    }
    Ok(())
}

fn ensure_spot_fill_within_order_reservation(
    reservation: &SpotOrderReservation,
    requested_amount: &BigDecimal,
    side: OrderSide,
) -> AppResult<()> {
    if reservation.amount < *requested_amount {
        let reserve_name = match side {
            OrderSide::Buy => "quote",
            OrderSide::Sell => "base",
        };
        return Err(AppError::Validation(format!(
            "insufficient order reservation for spot fill: requested {}, reserved {} {}",
            requested_amount, reservation.amount, reserve_name
        )));
    }
    Ok(())
}

async fn release_buy_order_surplus_reservation_after_fill(
    tx: &mut Transaction<'_, MySql>,
    buyer_id: u64,
    buy_order: &SpotOrder,
    reservation_before_fill: &SpotOrderReservation,
    fill_quote_amount: &BigDecimal,
    ref_id: &str,
) -> AppResult<()> {
    let surplus_amount = reservation_before_fill.amount.clone() - fill_quote_amount.clone();
    if surplus_amount <= 0 || buy_order.status == OrderStatus::PartiallyFilled {
        return Ok(());
    }
    // 非继续挂单的买单成交后释放剩余订单级预留，避免市价单全成后价差长期冻结。
    apply_spot_wallet_unfreeze(
        tx,
        buyer_id,
        reservation_before_fill.asset_id,
        &surplus_amount,
        "spot_price_improvement_release",
        "spot_trade",
        ref_id,
    )
    .await
}

async fn save_spot_order(tx: &mut Transaction<'_, MySql>, order: &SpotOrder) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE spot_orders
           SET filled_quantity = ?, status = ?
           WHERE id = ?"#,
    )
    .bind(&order.filled_quantity)
    .bind(order_status_as_str(order.status))
    .bind(
        order
            .id
            .parse::<u64>()
            .map_err(|_| AppError::Validation("invalid spot order id".to_owned()))?,
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn insert_spot_trade(
    tx: &mut Transaction<'_, MySql>,
    buy_order: &SpotOrder,
    sell_order: &SpotOrder,
    price: &BigDecimal,
    quantity: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<SpotTrade> {
    let (pair_id,): (u64,) =
        sqlx::query_as("SELECT id FROM trading_pairs WHERE symbol = ? LIMIT 1")
            .bind(&buy_order.pair_id)
            .fetch_one(&mut **tx)
            .await?;
    let buy_order_id = buy_order
        .id
        .parse::<u64>()
        .map_err(|_| AppError::Validation("invalid buy order id".to_owned()))?;
    let sell_order_id = sell_order
        .id
        .parse::<u64>()
        .map_err(|_| AppError::Validation("invalid sell order id".to_owned()))?;
    let trade = NewSpotTrade {
        pair_id: buy_order.pair_id.clone(),
        buy_order_id: buy_order.id.clone(),
        sell_order_id: sell_order.id.clone(),
        price: price.clone(),
        quantity: quantity.clone(),
        fee: BigDecimal::from(0),
    };
    let result = sqlx::query(
        r#"INSERT INTO spot_trades
           (pair_id, buy_order_id, sell_order_id, price, quantity, fee, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(pair_id)
    .bind(buy_order_id)
    .bind(sell_order_id)
    .bind(&trade.price)
    .bind(&trade.quantity)
    .bind(&trade.fee)
    .bind(idempotency_key)
    .execute(&mut **tx)
    .await?;
    let (id, created_at): (u64, chrono::DateTime<chrono::Utc>) =
        sqlx::query_as("SELECT id, created_at FROM spot_trades WHERE id = ?")
            .bind(result.last_insert_id())
            .fetch_one(&mut **tx)
            .await?;
    Ok(SpotTrade {
        id: id.to_string(),
        pair_id: trade.pair_id,
        buy_order_id: trade.buy_order_id,
        sell_order_id: trade.sell_order_id,
        price: trade.price,
        quantity: trade.quantity,
        fee: trade.fee,
        created_at,
    })
}

async fn insert_order_and_freeze_wallet(
    pool: &Pool<MySql>,
    new_order: NewOrder,
    idempotency_key: Option<&str>,
    request_price: Option<&BigDecimal>,
    reference_price: Option<&BigDecimal>,
) -> AppResult<(SpotOrder, bool)> {
    let pair_db_id = spot_pair_db_id(pool, &new_order.pair_id).await?;
    let mut tx = pool.begin().await?;
    let reservation = spot_order_reservation_in_tx(&mut tx, &new_order, reference_price).await?;
    let (order, is_new_order) = insert_spot_order_in_tx(
        &mut tx,
        new_order,
        pair_db_id,
        normalize_idempotency_key(idempotency_key),
        request_price,
        reference_price,
        &reservation,
    )
    .await?;
    if is_new_order {
        freeze_wallet_for_inserted_order_in_tx(&mut tx, &order, &reservation).await?;
    }
    tx.commit().await?;
    Ok((order, is_new_order))
}

async fn existing_spot_order_for_idempotency_key(
    pool: &Pool<MySql>,
    user_id: u64,
    request: &CreateSpotOrderRequest,
) -> AppResult<Option<SpotOrderResponse>> {
    let Some(idempotency_key) = normalize_idempotency_key(request.idempotency_key.as_deref())
    else {
        return Ok(None);
    };
    let existing = select_spot_order_by_idempotency_key(pool, idempotency_key).await?;

    match existing {
        Some(order) if order.user_id == user_id => {
            ensure_idempotent_spot_order_matches_request(&order, request)?;
            Ok(Some(SpotOrderResponse::from(order)))
        }
        Some(_) => Err(AppError::Conflict(
            "spot order idempotency key belongs to another user".to_owned(),
        )),
        None => Ok(None),
    }
}

async fn select_spot_order_by_idempotency_key<'e, E>(
    executor: E,
    idempotency_key: &str,
) -> Result<Option<IdempotentSpotOrderRow>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = MySql>,
{
    sqlx::query_as::<_, IdempotentSpotOrderRow>(
        r#"SELECT orders.id, orders.user_id, orders.pair_id AS pair_db_id,
                  pairs.symbol AS pair_id, orders.side, orders.order_type, orders.price,
                  orders.quantity, orders.filled_quantity, orders.status, orders.reserved_amount,
                  orders.request_reference_price, orders.request_price
           FROM spot_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           WHERE orders.idempotency_key = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(idempotency_key)
    .fetch_optional(executor)
    .await
}

fn ensure_idempotent_spot_order_matches_insert(
    existing: &IdempotentSpotOrderRow,
    new_order: &NewOrder,
    request_price: Option<&BigDecimal>,
    reference_price: Option<&BigDecimal>,
    reservation: &SpotOrderReservation,
) -> AppResult<()> {
    let expected_reference_price = match new_order.order_type {
        OrderType::Limit => None,
        OrderType::Market => reference_price,
    };
    let matches = spot_pair_matches(existing, &new_order.pair_id)
        && parse_order_side(&existing.side) == new_order.side
        && parse_order_type(&existing.order_type) == new_order.order_type
        && existing.price == new_order.price
        && existing.quantity == new_order.quantity
        && existing.reserved_amount.as_ref() == Some(&reservation.amount)
        && request_reference_price_matches(
            existing,
            new_order.side,
            new_order.order_type,
            expected_reference_price,
        )
        && request_price_matches(existing, new_order.order_type, request_price);

    if matches {
        Ok(())
    } else {
        Err(AppError::Conflict(
            "spot order idempotency key was used with a different request".to_owned(),
        ))
    }
}

fn ensure_idempotent_spot_order_matches_request(
    existing: &IdempotentSpotOrderRow,
    request: &CreateSpotOrderRequest,
) -> AppResult<()> {
    let expected_reservation_price = match request.order_type {
        OrderType::Limit => request.price.as_ref(),
        OrderType::Market => request.reference_price.as_ref(),
    };
    let expected_reserved_amount = expected_reservation_price
        .map(|price| spot_reservation_amount(request.side, price, &request.quantity));
    let expected_price = match request.order_type {
        OrderType::Limit => request.price.as_ref(),
        OrderType::Market => None,
    };
    let expected_reference_price = match request.order_type {
        OrderType::Limit => None,
        OrderType::Market => request.reference_price.as_ref(),
    };
    let matches = spot_pair_matches(existing, &request.pair_id)
        && parse_order_side(&existing.side) == request.side
        && parse_order_type(&existing.order_type) == request.order_type
        && existing.price.as_ref() == expected_price
        && existing.quantity == request.quantity
        && existing.reserved_amount == expected_reserved_amount
        && request_reference_price_matches(
            existing,
            request.side,
            request.order_type,
            expected_reference_price,
        )
        && request_price_matches(existing, request.order_type, request.price.as_ref());

    if matches {
        Ok(())
    } else {
        Err(AppError::Conflict(
            "spot order idempotency key was used with a different request".to_owned(),
        ))
    }
}

fn spot_pair_matches(existing: &IdempotentSpotOrderRow, requested_pair_id: &str) -> bool {
    existing.pair_id.eq_ignore_ascii_case(requested_pair_id)
        || requested_pair_id.parse::<u64>().ok() == Some(existing.pair_db_id)
}

fn request_reference_price_matches(
    existing: &IdempotentSpotOrderRow,
    side: OrderSide,
    order_type: OrderType,
    expected: Option<&BigDecimal>,
) -> bool {
    match existing.request_reference_price.as_ref() {
        Some(stored) => Some(stored) == expected,
        None => match order_type {
            OrderType::Limit => expected.is_none(),
            OrderType::Market => side == OrderSide::Buy,
        },
    }
}

fn request_price_matches(
    existing: &IdempotentSpotOrderRow,
    order_type: OrderType,
    expected: Option<&BigDecimal>,
) -> bool {
    match existing.request_price.as_ref() {
        Some(stored) => Some(stored) == expected,
        None => match order_type {
            OrderType::Limit => existing.price.as_ref() == expected,
            OrderType::Market => expected.is_none(),
        },
    }
}

fn normalize_idempotency_key(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

async fn spot_pair_db_id(pool: &Pool<MySql>, pair_symbol: &str) -> AppResult<u64> {
    let (pair_db_id,): (u64,) = sqlx::query_as(
        r#"SELECT id
           FROM trading_pairs
           WHERE symbol = ? OR id = ?
           LIMIT 1"#,
    )
    .bind(pair_symbol)
    .bind(pair_symbol.parse::<u64>().ok())
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(pair_db_id)
}

async fn insert_spot_order_in_tx(
    tx: &mut Transaction<'_, MySql>,
    new_order: NewOrder,
    pair_db_id: u64,
    idempotency_key: Option<&str>,
    request_price: Option<&BigDecimal>,
    reference_price: Option<&BigDecimal>,
    reservation: &SpotOrderReservation,
) -> AppResult<(SpotOrder, bool)> {
    // 下单记录和钱包冻结必须同事务提交；重复幂等键命中时只锁定并返回原订单，不再次冻结钱包。
    let user_id = new_order
        .user_id
        .parse::<u64>()
        .map_err(|_| AppError::Unauthorized)?;
    let insert_result = sqlx::query(
        r#"INSERT INTO spot_orders
           (user_id, pair_id, side, order_type, price, quantity, filled_quantity, status,
            idempotency_key, reserved_asset, reserved_amount, request_reference_price, request_price)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(pair_db_id)
    .bind(route_order_side_as_str(new_order.side))
    .bind(route_order_type_as_str(new_order.order_type))
    .bind(&new_order.price)
    .bind(&new_order.quantity)
    .bind(&new_order.filled_quantity)
    .bind(order_status_as_str(new_order.status))
    .bind(idempotency_key)
    .bind(reservation.asset_id)
    .bind(&reservation.amount)
    .bind(match new_order.order_type {
        OrderType::Limit => None,
        OrderType::Market => reference_price,
    })
    .bind(request_price)
    .execute(&mut **tx)
    .await;

    let (order_id, is_new_order) = match insert_result {
        Ok(result) => (result.last_insert_id(), true),
        Err(error) if is_duplicate_key_error(&error) => {
            let Some(idempotency_key) = idempotency_key else {
                return Err(error.into());
            };
            let existing = select_spot_order_by_idempotency_key(&mut **tx, idempotency_key)
                .await?
                .ok_or(AppError::NotFound)?;
            if existing.user_id != user_id {
                return Err(AppError::Conflict(
                    "spot order idempotency key belongs to another user".to_owned(),
                ));
            }
            ensure_idempotent_spot_order_matches_insert(
                &existing,
                &new_order,
                request_price,
                reference_price,
                reservation,
            )?;
            return Ok((SpotOrderResponse::from(existing).into(), false));
        }
        Err(error) => return Err(error.into()),
    };

    let row = sqlx::query_as::<_, SpotOrderRow>(
        r#"SELECT orders.id, orders.user_id, pairs.symbol AS pair_id, orders.side,
                  orders.order_type, orders.price, orders.quantity, orders.filled_quantity,
                  orders.status
           FROM spot_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           WHERE orders.id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(order_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok((SpotOrderResponse::from(row).into(), is_new_order))
}

fn is_duplicate_key_error(error: &sqlx::Error) -> bool {
    error.as_database_error().is_some_and(|database_error| {
        database_error.code().as_deref() == Some("1062")
            || database_error.code().as_deref() == Some("23000")
    })
}

async fn spot_order_reservation_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &NewOrder,
    reference_price: Option<&BigDecimal>,
) -> AppResult<SpotOrderReservation> {
    let assets = pair_assets_in_tx(tx, &order.pair_id).await?;
    let price = match order.order_type {
        OrderType::Limit => order.price.as_ref().ok_or_else(|| {
            AppError::Validation("price is required for wallet reservation".to_owned())
        })?,
        OrderType::Market => reference_price.ok_or_else(|| {
            AppError::Validation("reference_price is required for market orders".to_owned())
        })?,
    };
    let amount = spot_reservation_amount(order.side, price, &order.quantity);
    let asset_id = spot_reserve_asset_id(
        order.side,
        &assets.base_asset_id.to_string(),
        &assets.quote_asset_id.to_string(),
    )
    .parse::<u64>()
    .map_err(|_| AppError::Internal("invalid reserve asset id".to_owned()))?;
    Ok(SpotOrderReservation { asset_id, amount })
}

async fn freeze_wallet_for_inserted_order_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &SpotOrder,
    reservation: &SpotOrderReservation,
) -> AppResult<()> {
    let user_id = order
        .user_id
        .parse::<u64>()
        .map_err(|_| AppError::Unauthorized)?;
    apply_spot_wallet_freeze(
        tx,
        user_id,
        reservation.asset_id,
        &reservation.amount,
        "spot_freeze",
        "spot_order",
        &order.id,
    )
    .await
}

async fn cancel_spot_order_and_unfreeze_wallet(
    pool: &Pool<MySql>,
    order_id: u64,
    user_id: u64,
) -> AppResult<(SpotOrder, bool)> {
    let mut tx = pool.begin().await?;
    let order = lock_spot_order(&mut tx, &order_id.to_string()).await?;
    if order.user_id != user_id.to_string() {
        return Err(AppError::NotFound);
    }
    let result = cancel_locked_spot_order_and_unfreeze_wallet(&mut tx, order, user_id).await?;
    tx.commit().await?;
    Ok(result)
}

async fn cancel_spot_order_by_admin(
    pool: &Pool<MySql>,
    order_id: u64,
    admin_id: u64,
    reason: String,
) -> AppResult<(SpotOrder, bool)> {
    let mut tx = pool.begin().await?;
    let order = lock_spot_order(&mut tx, &order_id.to_string()).await?;
    let owner_user_id = order
        .user_id
        .parse::<u64>()
        .map_err(|_| AppError::Unauthorized)?;
    let before = spot_order_audit_json(&order);
    let (order, cancelled) =
        cancel_locked_spot_order_and_unfreeze_wallet(&mut tx, order, owner_user_id).await?;
    if cancelled {
        insert_spot_admin_audit_log_in_tx(
            &mut tx,
            admin_id,
            SpotAdminAuditEntry {
                action: "spot_order.cancel",
                target_type: "spot_order",
                target_id: &order.id,
                before_json: Some(before),
                after_json: Some(spot_order_audit_json(&order)),
                reason: Some(reason),
            },
        )
        .await?;
    }
    tx.commit().await?;
    Ok((order, cancelled))
}

async fn cancel_locked_spot_order_and_unfreeze_wallet(
    tx: &mut Transaction<'_, MySql>,
    mut order: SpotOrder,
    user_id: u64,
) -> AppResult<(SpotOrder, bool)> {
    let cancelled = crate::modules::spot::cancel_order(&mut order)
        .map_err(|error| AppError::Validation(format!("invalid spot cancel: {error:?}")))?;
    if !cancelled {
        return Ok((order, false));
    }

    // 撤单状态和钱包解冻必须同事务提交，避免订单仍可成交但资金已经提前解冻。
    let reservation = remaining_spot_order_reservation_in_tx(tx, &order).await?;
    if reservation.amount > 0 {
        apply_spot_wallet_unfreeze(
            tx,
            user_id,
            reservation.asset_id,
            &reservation.amount,
            "spot_unfreeze",
            "spot_order",
            &order.id,
        )
        .await?;
    }
    update_spot_order_in_tx(tx, &order).await?;
    Ok((order, true))
}

async fn remaining_spot_fill_reservation_before_trade_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &SpotOrder,
    current_trade_id: &str,
) -> AppResult<SpotOrderReservation> {
    let order_db_id = parse_spot_order_db_id(order)?;
    let stored = sqlx::query_as::<_, SpotOrderReservationRow>(
        r#"SELECT reserved_asset AS reserved_asset_id, reserved_amount
           FROM spot_orders
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(order_db_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if let (Some(asset_id), Some(total_amount)) = (stored.reserved_asset_id, stored.reserved_amount)
    {
        let total_amount = if total_amount > 0 {
            Some(total_amount)
        } else {
            ledger_freeze_reservation_in_tx(tx, order, asset_id).await?
        };
        if let Some(total_amount) = total_amount {
            let trade_id = current_trade_id
                .parse::<u64>()
                .map_err(|_| AppError::Validation("invalid spot trade id".to_owned()))?;
            return remaining_tracked_reservation_excluding_trade_in_tx(
                tx,
                order,
                asset_id,
                total_amount,
                Some(trade_id),
            )
            .await;
        }
        return Ok(SpotOrderReservation {
            asset_id,
            amount: BigDecimal::from(0),
        });
    }

    remaining_legacy_spot_reservation_in_tx(tx, order).await
}

async fn remaining_spot_order_reservation_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &SpotOrder,
) -> AppResult<SpotOrderReservation> {
    let order_db_id = order
        .id
        .parse::<u64>()
        .map_err(|_| AppError::Internal("invalid spot order id".to_owned()))?;
    let stored = sqlx::query_as::<_, SpotOrderReservationRow>(
        r#"SELECT reserved_asset AS reserved_asset_id, reserved_amount
           FROM spot_orders
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(order_db_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if let (Some(asset_id), Some(total_amount)) = (stored.reserved_asset_id, stored.reserved_amount)
    {
        let total_amount = if total_amount > 0 {
            total_amount
        } else {
            ledger_freeze_reservation_in_tx(tx, order, asset_id)
                .await?
                .unwrap_or(total_amount)
        };
        return remaining_tracked_reservation_in_tx(tx, order, asset_id, total_amount).await;
    }

    remaining_legacy_spot_reservation_in_tx(tx, order).await
}

async fn remaining_legacy_spot_reservation_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &SpotOrder,
) -> AppResult<SpotOrderReservation> {
    let assets = pair_assets_in_tx(tx, &order.pair_id).await?;
    let (reserve_asset_id, reserve_amount) = spot_remaining_reserved_amount(
        order,
        &assets.base_asset_id.to_string(),
        &assets.quote_asset_id.to_string(),
    )
    .map_err(map_spot_error)?;
    let asset_id = reserve_asset_id
        .parse::<u64>()
        .map_err(|_| AppError::Internal("invalid reserve asset id".to_owned()))?;
    let amount = match order.side {
        OrderSide::Buy => {
            let wallet = lock_wallet_row(
                tx,
                order
                    .user_id
                    .parse::<u64>()
                    .map_err(|_| AppError::Unauthorized)?,
                asset_id,
            )
            .await?;
            if wallet.frozen > reserve_amount {
                wallet.frozen
            } else {
                reserve_amount
            }
        }
        OrderSide::Sell => reserve_amount,
    };
    Ok(SpotOrderReservation { asset_id, amount })
}

async fn ledger_freeze_reservation_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &SpotOrder,
    asset_id: u64,
) -> AppResult<Option<BigDecimal>> {
    let (frozen_amount,): (Option<BigDecimal>,) = sqlx::query_as(
        r#"SELECT SUM(amount)
           FROM wallet_ledger
           WHERE ref_type = 'spot_order'
             AND ref_id = ?
             AND asset_id = ?
             AND change_type = 'spot_freeze'
             AND balance_type = 'frozen'
             AND amount > 0"#,
    )
    .bind(&order.id)
    .bind(asset_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(frozen_amount.filter(|amount| amount > &BigDecimal::from(0)))
}

async fn remaining_tracked_reservation_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &SpotOrder,
    asset_id: u64,
    total_amount: BigDecimal,
) -> AppResult<SpotOrderReservation> {
    remaining_tracked_reservation_excluding_trade_in_tx(tx, order, asset_id, total_amount, None)
        .await
}

async fn remaining_tracked_reservation_excluding_trade_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &SpotOrder,
    asset_id: u64,
    total_amount: BigDecimal,
    excluded_trade_id: Option<u64>,
) -> AppResult<SpotOrderReservation> {
    let spent_amount =
        filled_spot_order_reservation_excluding_trade_in_tx(tx, order, excluded_trade_id).await?;
    let released_amount = released_spot_order_reservation_in_tx(tx, order).await?;
    let remaining_amount = total_amount - spent_amount - released_amount;
    Ok(SpotOrderReservation {
        asset_id,
        amount: if remaining_amount > 0 {
            remaining_amount
        } else {
            BigDecimal::from(0)
        },
    })
}

async fn released_spot_order_reservation_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &SpotOrder,
) -> AppResult<BigDecimal> {
    let (released_amount,): (Option<BigDecimal>,) = sqlx::query_as(
        r#"SELECT COALESCE(SUM(amount), 0)
           FROM wallet_ledger
           WHERE ref_type = 'spot_trade'
             AND change_type = 'spot_price_improvement_release'
             AND balance_type = 'frozen'
             AND amount < 0
             AND ref_id LIKE ?"#,
    )
    .bind(format!("{}:%", order.id))
    .fetch_one(&mut **tx)
    .await?;
    Ok(-released_amount.unwrap_or_else(|| BigDecimal::from(0)))
}

async fn filled_spot_order_reservation_excluding_trade_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &SpotOrder,
    excluded_trade_id: Option<u64>,
) -> AppResult<BigDecimal> {
    let order_id = parse_spot_order_db_id(order)?;
    let (filled_amount,): (Option<BigDecimal>,) = match order.side {
        OrderSide::Buy => {
            sqlx::query_as(
                r#"SELECT COALESCE(SUM(price * quantity), 0)
                   FROM spot_trades
                   WHERE buy_order_id = ?
                     AND (? IS NULL OR id <> ?)"#,
            )
            .bind(order_id)
            .bind(excluded_trade_id)
            .bind(excluded_trade_id)
            .fetch_one(&mut **tx)
            .await?
        }
        OrderSide::Sell => {
            sqlx::query_as(
                r#"SELECT COALESCE(SUM(quantity), 0)
                   FROM spot_trades
                   WHERE sell_order_id = ?
                     AND (? IS NULL OR id <> ?)"#,
            )
            .bind(order_id)
            .bind(excluded_trade_id)
            .bind(excluded_trade_id)
            .fetch_one(&mut **tx)
            .await?
        }
    };
    Ok(filled_amount.unwrap_or_else(|| BigDecimal::from(0)))
}

fn parse_spot_order_db_id(order: &SpotOrder) -> AppResult<u64> {
    order
        .id
        .parse::<u64>()
        .map_err(|_| AppError::Validation("invalid spot order id".to_owned()))
}

async fn update_spot_order_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &SpotOrder,
) -> AppResult<()> {
    let order_db_id = order
        .id
        .parse::<u64>()
        .map_err(|_| AppError::Internal("invalid spot order id".to_owned()))?;
    let pair_db_id = spot_pair_db_id_in_tx(tx, &order.pair_id).await?;
    sqlx::query(
        r#"UPDATE spot_orders
           SET pair_id = ?, side = ?, order_type = ?, price = ?, quantity = ?,
               filled_quantity = ?, status = ?
           WHERE id = ?"#,
    )
    .bind(pair_db_id)
    .bind(route_order_side_as_str(order.side))
    .bind(route_order_type_as_str(order.order_type))
    .bind(&order.price)
    .bind(&order.quantity)
    .bind(&order.filled_quantity)
    .bind(order_status_as_str(order.status))
    .bind(order_db_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn spot_pair_db_id_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_symbol: &str,
) -> AppResult<u64> {
    let (pair_db_id,): (u64,) = sqlx::query_as(
        r#"SELECT id
           FROM trading_pairs
           WHERE symbol = ? OR id = ?
           LIMIT 1"#,
    )
    .bind(pair_symbol)
    .bind(pair_symbol.parse::<u64>().ok())
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(pair_db_id)
}

async fn pair_assets_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_symbol: &str,
) -> AppResult<SpotPairAssetRow> {
    sqlx::query_as::<_, SpotPairAssetRow>(
        r#"SELECT base_asset AS base_asset_id, quote_asset AS quote_asset_id
           FROM trading_pairs
           WHERE symbol = ?
           LIMIT 1"#,
    )
    .bind(pair_symbol)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

async fn lock_spot_fill_wallet_rows_in_order(
    tx: &mut Transaction<'_, MySql>,
    buyer_id: u64,
    seller_id: u64,
    base_asset_id: u64,
    quote_asset_id: u64,
) -> AppResult<()> {
    // 成交结算会触达买卖双方的 base/quote 钱包，先按固定顺序锁行，避免交叉方向成交互相等待。
    for (user_id, asset_id) in
        spot_fill_wallet_lock_keys(buyer_id, seller_id, base_asset_id, quote_asset_id)
    {
        lock_wallet_row(tx, user_id, asset_id).await?;
    }
    Ok(())
}

fn spot_fill_wallet_lock_keys(
    buyer_id: u64,
    seller_id: u64,
    base_asset_id: u64,
    quote_asset_id: u64,
) -> Vec<(u64, u64)> {
    let mut keys = vec![
        (buyer_id, quote_asset_id),
        (buyer_id, base_asset_id),
        (seller_id, base_asset_id),
        (seller_id, quote_asset_id),
    ];
    keys.sort_unstable();
    keys.dedup();
    keys
}

async fn lock_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<SpotWalletRow> {
    sqlx::query_as::<_, SpotWalletRow>(
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
    .ok_or_else(|| AppError::Validation("wallet account is required for spot order".to_owned()))
}

async fn apply_spot_wallet_freeze(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    change_type: &str,
    ref_type: &str,
    ref_id: &str,
) -> AppResult<()> {
    let wallet = lock_wallet_row(tx, user_id, asset_id).await?;
    if wallet.available < *amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for spot order: requested {}, available {}, locked {}",
            amount, wallet.available, wallet.locked
        )));
    }
    let available_after = wallet.available.clone() - amount.clone();
    let frozen_after = wallet.frozen.clone() + amount.clone();
    sqlx::query(
        "UPDATE wallet_accounts SET available = ?, frozen = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&available_after)
    .bind(&frozen_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    insert_spot_wallet_ledger(
        tx,
        user_id,
        asset_id,
        -amount.clone(),
        "available",
        &available_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
        change_type,
        ref_type,
        ref_id,
    )
    .await?;
    insert_spot_wallet_ledger(
        tx,
        user_id,
        asset_id,
        amount.clone(),
        "frozen",
        &frozen_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
        change_type,
        ref_type,
        ref_id,
    )
    .await
}

async fn apply_spot_wallet_settlement_leg(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    credit_available: bool,
    ledger: SpotLedgerMetadata<'_>,
) -> AppResult<()> {
    let wallet = lock_wallet_row(tx, user_id, asset_id).await?;
    let (amount_change, available_after, frozen_after, balance_type, balance_after) =
        if credit_available {
            let available_after = wallet.available.clone() + amount.clone();
            (
                amount.clone(),
                available_after.clone(),
                wallet.frozen.clone(),
                "available",
                available_after,
            )
        } else {
            if wallet.frozen < *amount {
                return Err(AppError::Validation(format!(
                    "insufficient frozen balance for spot fill: requested {}, frozen {}",
                    amount, wallet.frozen
                )));
            }
            let frozen_after = wallet.frozen.clone() - amount.clone();
            (
                -amount.clone(),
                wallet.available.clone(),
                frozen_after.clone(),
                "frozen",
                frozen_after,
            )
        };
    sqlx::query(
        "UPDATE wallet_accounts SET available = ?, frozen = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&available_after)
    .bind(&frozen_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    insert_spot_wallet_ledger(
        tx,
        user_id,
        asset_id,
        amount_change,
        balance_type,
        &balance_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
        ledger.change_type,
        ledger.ref_type,
        ledger.ref_id,
    )
    .await
}

async fn insert_spot_admin_audit_log_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    entry: SpotAdminAuditEntry<'_>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, before_json, after_json, reason)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(admin_id)
    .bind(entry.action)
    .bind(entry.target_type)
    .bind(entry.target_id)
    .bind(entry.before_json.map(SqlxJson))
    .bind(entry.after_json.map(SqlxJson))
    .bind(optional_query_string(entry.reason))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn apply_spot_wallet_unfreeze(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    change_type: &str,
    ref_type: &str,
    ref_id: &str,
) -> AppResult<()> {
    let wallet = lock_wallet_row(tx, user_id, asset_id).await?;
    if wallet.frozen < *amount {
        return Err(AppError::Validation(format!(
            "insufficient frozen balance for spot cancel: requested {}, frozen {}",
            amount, wallet.frozen
        )));
    }
    let available_after = wallet.available.clone() + amount.clone();
    let frozen_after = wallet.frozen.clone() - amount.clone();
    sqlx::query(
        "UPDATE wallet_accounts SET available = ?, frozen = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&available_after)
    .bind(&frozen_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    insert_spot_wallet_ledger(
        tx,
        user_id,
        asset_id,
        amount.clone(),
        "available",
        &available_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
        change_type,
        ref_type,
        ref_id,
    )
    .await?;
    insert_spot_wallet_ledger(
        tx,
        user_id,
        asset_id,
        -amount.clone(),
        "frozen",
        &frozen_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
        change_type,
        ref_type,
        ref_id,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn insert_spot_wallet_ledger(
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
    ref_type: &str,
    ref_id: &str,
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
    .bind(ref_type)
    .bind(ref_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
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

fn optional_query_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn required_reason(value: Option<String>) -> AppResult<String> {
    optional_query_string(value)
        .ok_or_else(|| AppError::Validation("reason is required".to_owned()))
}

fn spot_order_audit_json(order: &SpotOrder) -> Value {
    json!({
        "id": order.id,
        "user_id": order.user_id,
        "pair_id": order.pair_id,
        "side": order.side,
        "order_type": order.order_type,
        "price": order.price,
        "quantity": order.quantity,
        "filled_quantity": order.filled_quantity,
        "status": order.status,
    })
}

fn ensure_positive_amount(amount: &BigDecimal, field: &str) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        Err(AppError::Validation(format!("{field} must be positive")))
    } else {
        Ok(())
    }
}

fn order_status_as_str(status: OrderStatus) -> &'static str {
    match status {
        OrderStatus::Pending => "pending",
        OrderStatus::Open => "open",
        OrderStatus::PartiallyFilled => "partially_filled",
        OrderStatus::Filled => "filled",
        OrderStatus::Cancelled => "cancelled",
        OrderStatus::Rejected => "rejected",
    }
}

fn route_order_side_as_str(side: OrderSide) -> &'static str {
    match side {
        OrderSide::Buy => "buy",
        OrderSide::Sell => "sell",
    }
}

fn route_order_type_as_str(order_type: OrderType) -> &'static str {
    match order_type {
        OrderType::Limit => "limit",
        OrderType::Market => "market",
    }
}

fn map_spot_error(error: crate::modules::spot::SpotServiceError) -> AppError {
    match error {
        crate::modules::spot::SpotServiceError::Repository(message)
            if message.starts_with("missing") =>
        {
            AppError::NotFound
        }
        crate::modules::spot::SpotServiceError::Repository(message) => AppError::Internal(message),
        crate::modules::spot::SpotServiceError::Domain(error) => {
            AppError::Validation(format!("invalid spot order: {error:?}"))
        }
        crate::modules::spot::SpotServiceError::Wallet(error) => {
            AppError::Validation(format!("wallet error: {error:?}"))
        }
        crate::modules::spot::SpotServiceError::MissingPriceForWalletReservation => {
            AppError::Validation("price is required for wallet reservation".to_owned())
        }
        crate::modules::spot::SpotServiceError::MissingReferencePriceForMarketOrder => {
            AppError::Validation("reference_price is required for market orders".to_owned())
        }
    }
}

impl From<SpotOrder> for SpotOrderResponse {
    fn from(order: SpotOrder) -> Self {
        Self {
            id: order.id,
            user_id: order.user_id,
            pair_id: order.pair_id,
            side: order.side,
            order_type: order.order_type,
            price: order.price,
            quantity: order.quantity,
            filled_quantity: order.filled_quantity,
            status: order.status,
        }
    }
}

impl From<SpotOrderRow> for SpotOrderResponse {
    fn from(order: SpotOrderRow) -> Self {
        Self {
            id: order.id.to_string(),
            user_id: order.user_id.to_string(),
            pair_id: order.pair_id,
            side: parse_order_side(&order.side),
            order_type: parse_order_type(&order.order_type),
            price: order.price,
            quantity: order.quantity,
            filled_quantity: order.filled_quantity,
            status: parse_order_status(&order.status),
        }
    }
}

impl From<IdempotentSpotOrderRow> for SpotOrderResponse {
    fn from(order: IdempotentSpotOrderRow) -> Self {
        Self {
            id: order.id.to_string(),
            user_id: order.user_id.to_string(),
            pair_id: order.pair_id,
            side: parse_order_side(&order.side),
            order_type: parse_order_type(&order.order_type),
            price: order.price,
            quantity: order.quantity,
            filled_quantity: order.filled_quantity,
            status: parse_order_status(&order.status),
        }
    }
}

impl From<SpotOrderResponse> for SpotOrder {
    fn from(order: SpotOrderResponse) -> Self {
        Self {
            id: order.id,
            user_id: order.user_id,
            pair_id: order.pair_id,
            side: order.side,
            order_type: order.order_type,
            price: order.price,
            quantity: order.quantity,
            filled_quantity: order.filled_quantity,
            status: order.status,
        }
    }
}

impl From<SpotTradeRow> for SpotTrade {
    fn from(row: SpotTradeRow) -> Self {
        Self {
            id: row.id.to_string(),
            pair_id: row.pair_id,
            buy_order_id: row.buy_order_id.to_string(),
            sell_order_id: row.sell_order_id.to_string(),
            price: row.price,
            quantity: row.quantity,
            fee: row.fee,
            created_at: row.created_at,
        }
    }
}

impl From<SpotTrade> for SpotTradeResponse {
    fn from(trade: SpotTrade) -> Self {
        Self {
            id: trade.id,
            pair_id: trade.pair_id,
            buy_order_id: trade.buy_order_id,
            sell_order_id: trade.sell_order_id,
            price: trade.price,
            quantity: trade.quantity,
            fee: trade.fee,
            created_at: trade.created_at,
        }
    }
}

fn parse_order_side(value: &str) -> OrderSide {
    match value {
        "sell" => OrderSide::Sell,
        _ => OrderSide::Buy,
    }
}

fn parse_order_type(value: &str) -> OrderType {
    match value {
        "market" => OrderType::Market,
        _ => OrderType::Limit,
    }
}

fn parse_order_status(value: &str) -> OrderStatus {
    match value {
        "open" => OrderStatus::Open,
        "partially_filled" => OrderStatus::PartiallyFilled,
        "filled" => OrderStatus::Filled,
        "cancelled" => OrderStatus::Cancelled,
        "rejected" => OrderStatus::Rejected,
        _ => OrderStatus::Pending,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteNewOrder {
    pub user_id: String,
    pub pair_id: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub price: Option<BigDecimal>,
    pub quantity: BigDecimal,
    pub reference_price: Option<BigDecimal>,
}

pub fn build_route_new_order(
    user_id: u64,
    request: RouteNewOrder,
    pair: &crate::modules::spot::TradingPairRule,
) -> AppResult<NewOrder> {
    match request.order_type {
        OrderType::Limit => create_limit_order(
            user_id.to_string(),
            request.side,
            request.price.ok_or_else(|| {
                AppError::Validation("price is required for limit orders".to_owned())
            })?,
            request.quantity,
            pair,
        ),
        OrderType::Market => create_market_order(
            user_id.to_string(),
            request.side,
            request.quantity,
            request.reference_price.ok_or_else(|| {
                AppError::Validation("reference_price is required for market orders".to_owned())
            })?,
            pair,
        ),
    }
    .map_err(|error| AppError::Validation(format!("invalid spot order: {error:?}")))
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
    use std::str::FromStr;
    use tower::ServiceExt;

    fn decimal(value: &str) -> BigDecimal {
        BigDecimal::from_str(value).unwrap()
    }

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
    async fn spot_orders_route_requires_user_auth() {
        let app = routes().with_state(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/spot/orders")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn spot_orders_route_returns_clear_error_without_mysql() {
        let state = test_state();
        let token = bearer_token(&state);
        let app = routes().with_state(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/spot/orders")
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
            "internal error: mysql pool is not configured for spot routes"
        );
    }

    #[test]
    fn route_limit_is_clamped() {
        assert_eq!(route_limit(None), 50);
        assert_eq!(route_limit(Some(0)), 1);
        assert_eq!(route_limit(Some(500)), 100);
    }

    #[test]
    fn spot_fill_wallet_lock_keys_are_sorted_and_unique() {
        assert_eq!(
            spot_fill_wallet_lock_keys(20, 10, 2, 1),
            vec![(10, 1), (10, 2), (20, 1), (20, 2)]
        );
        assert_eq!(spot_fill_wallet_lock_keys(7, 7, 2, 1), vec![(7, 1), (7, 2)]);
    }

    #[test]
    fn spot_fill_order_lock_keys_are_canonical_sorted_and_unique() {
        assert_eq!(spot_fill_order_lock_keys("20", "10").unwrap(), vec![10, 20]);
        assert_eq!(spot_fill_order_lock_keys("0010", "10").unwrap(), vec![10]);
    }

    #[test]
    fn locked_spot_order_response_keeps_pair_id_without_locking_pair_row() {
        let order = locked_spot_order_response(
            SpotOrderLockRow {
                id: 7,
                user_id: 42,
                pair_id: 99,
                side: "buy".to_owned(),
                order_type: "limit".to_owned(),
                price: Some(decimal("10.000000000000000000")),
                quantity: decimal("2.000000000000000000"),
                filled_quantity: decimal("0.000000000000000000"),
                status: "open".to_owned(),
            },
            "BTC-USDT".to_owned(),
        );

        assert_eq!(order.id, "7");
        assert_eq!(order.pair_id, "BTC-USDT");
    }

    #[test]
    fn route_new_order_requires_market_reference_price() {
        let pair = crate::modules::spot::TradingPairRule {
            pair_id: "BTC-USDT".to_owned(),
            price_precision: 2,
            quantity_precision: 4,
            min_order_value: decimal("10"),
            enabled: true,
        };
        let result = build_route_new_order(
            42,
            RouteNewOrder {
                user_id: "ignored".to_owned(),
                pair_id: "BTC-USDT".to_owned(),
                side: OrderSide::Buy,
                order_type: OrderType::Market,
                price: None,
                quantity: decimal("0.1"),
                reference_price: None,
            },
            &pair,
        );

        assert!(matches!(result, Err(AppError::Validation(_))));
    }
}
