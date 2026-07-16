//! spot bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。
//! 只读查询和持久化细节集中在这里，路由层不直接拼装 SQL。

use crate::{
    error::{AppError, AppResult},
    modules::{
        market::market_ticker_redis_key,
        spot::{
            NewOrder, NewSpotTrade, OrderSide, OrderStatus, OrderType, SpotOrder, SpotServiceError,
            SpotTrade,
            presentation::{SpotOrderResponse, SpotTradeResponse},
            repository::{
                SpotAdminCancelCommand, SpotCancelRepositoryResult, SpotIdempotentOrderRecord,
                SpotOrderCancelRepository, SpotUserCancelCommand,
            },
            service::{
                SpotOrderReservation as CreateSpotOrderReservation, cancel_spot_order_state,
                ensure_spot_order_idempotency_matches_insert, parse_spot_order_request_id,
                spot_fill_order_lock_keys, spot_fill_wallet_lock_keys, spot_order_audit_json,
                spot_order_reservation,
            },
            spot_remaining_reserved_amount,
        },
    },
};
use axum::async_trait;
use bigdecimal::BigDecimal;
use redis::{AsyncCommands, aio::ConnectionManager};
use serde_json::Value;
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};
use std::str::FromStr;

const SYSTEM_SPOT_LIQUIDITY_EMAIL: &str = "__system_spot_liquidity@internal.local";
const SYSTEM_SPOT_LIQUIDITY_PASSWORD_HASH: &str = "system-liquidity";

pub(crate) struct SpotOrderListFilter {
    pub(crate) user_id: Option<u64>,
    pub(crate) pair_id: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) email: Option<String>,
    pub(crate) include_internal: bool,
    pub(crate) limit: u32,
}

pub(crate) struct SpotTradeListFilter {
    pub(crate) pair_id: Option<String>,
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) include_internal: bool,
    pub(crate) limit: u32,
}

#[derive(Debug, sqlx::FromRow)]
struct SpotOrderQueryRow {
    id: u64,
    user_id: u64,
    user_email: Option<String>,
    pair_id: String,
    side: String,
    order_type: String,
    price: Option<BigDecimal>,
    trigger_price: Option<BigDecimal>,
    quantity: BigDecimal,
    filled_quantity: BigDecimal,
    average_price: Option<BigDecimal>,
    status: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct SpotTradeQueryRow {
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
struct SpotOrderLockRow {
    id: u64,
    user_id: u64,
    pair_id: u64,
    side: String,
    order_type: String,
    price: Option<BigDecimal>,
    trigger_price: Option<BigDecimal>,
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
    trigger_price: Option<BigDecimal>,
    quantity: BigDecimal,
    filled_quantity: BigDecimal,
    status: String,
    created_at: chrono::DateTime<chrono::Utc>,
    reserved_amount: Option<BigDecimal>,
    request_reference_price: Option<BigDecimal>,
    request_price: Option<BigDecimal>,
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct SpotPairAssetRow {
    pub(crate) base_asset_id: u64,
    pub(crate) quote_asset_id: u64,
}

#[derive(Debug, sqlx::FromRow)]
struct SpotWalletRow {
    available: BigDecimal,
    frozen: BigDecimal,
    locked: BigDecimal,
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
pub(crate) struct SpotLedgerMetadata<'a> {
    pub(crate) change_type: &'a str,
    pub(crate) ref_type: &'a str,
    pub(crate) ref_id: &'a str,
}

struct SpotAdminAuditEntry<'a> {
    action: &'a str,
    target_type: &'a str,
    target_id: &'a str,
    before_json: Option<Value>,
    after_json: Option<Value>,
    reason: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct SqlxSpotOrderCancelRepository {
    pool: Pool<MySql>,
}

impl SqlxSpotOrderCancelRepository {
    pub(crate) fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SpotOrderCancelRepository for SqlxSpotOrderCancelRepository {
    async fn cancel_user_order(
        &self,
        command: SpotUserCancelCommand,
    ) -> AppResult<SpotCancelRepositoryResult> {
        let mut tx = self.pool.begin().await?;
        let order = lock_spot_order_by_db_id(&mut tx, command.order_id).await?;
        if order.user_id != command.user_id.to_string() {
            return Err(AppError::NotFound);
        }
        let result =
            cancel_locked_spot_order_and_unfreeze_wallet(&mut tx, order, command.user_id).await?;
        tx.commit().await?;
        Ok(result)
    }

    async fn cancel_admin_order(
        &self,
        command: SpotAdminCancelCommand,
    ) -> AppResult<SpotCancelRepositoryResult> {
        let mut tx = self.pool.begin().await?;
        let order = lock_spot_order_by_db_id(&mut tx, command.order_id).await?;
        let owner_user_id = order
            .user_id
            .parse::<u64>()
            .map_err(|_| AppError::Unauthorized)?;
        let before = spot_order_audit_json(&order);
        let result =
            cancel_locked_spot_order_and_unfreeze_wallet(&mut tx, order, owner_user_id).await?;
        if result.cancelled {
            insert_spot_admin_audit_log_in_tx(
                &mut tx,
                command.admin_id,
                SpotAdminAuditEntry {
                    action: "spot_order.cancel",
                    target_type: "spot_order",
                    target_id: &result.order.id,
                    before_json: Some(before),
                    after_json: Some(spot_order_audit_json(&result.order)),
                    reason: Some(command.reason),
                },
            )
            .await?;
        }
        tx.commit().await?;
        Ok(result)
    }
}

#[derive(Debug, Clone)]
pub struct MySqlSpotRepository {
    pool: Pool<MySql>,
}

impl MySqlSpotRepository {
    pub fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &Pool<MySql> {
        &self.pool
    }

    pub async fn load_pair_rule_async(
        &self,
        pair_id: &str,
    ) -> Result<crate::modules::spot::TradingPairRule, crate::modules::spot::SpotServiceError> {
        let row = sqlx::query_as::<_, (u64, String, i32, i32, BigDecimal, String)>(
            r#"SELECT id, symbol, price_precision, qty_precision, min_order_value, status
               FROM trading_pairs
               WHERE symbol = ? OR id = ?
               LIMIT 1"#,
        )
        .bind(pair_id)
        .bind(pair_id.parse::<u64>().ok())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_spot_sqlx_error)?
        .ok_or_else(|| {
            crate::modules::spot::SpotServiceError::Repository(format!(
                "missing trading pair: {pair_id}"
            ))
        })?;

        let (_id, symbol, price_precision, quantity_precision, min_order_value, status) = row;
        Ok(crate::modules::spot::TradingPairRule {
            pair_id: symbol,
            price_precision: price_precision as u32,
            quantity_precision: quantity_precision as u32,
            min_order_value,
            enabled: status == "active",
        })
    }

    pub async fn insert_order_async(
        &self,
        new_order: NewOrder,
        idempotency_key: Option<&str>,
    ) -> Result<SpotOrder, crate::modules::spot::SpotServiceError> {
        let user_id = parse_spot_u64_identifier("user_id", &new_order.user_id)?;
        let pair_db_id = resolve_pair_id(&self.pool, &new_order.pair_id).await?;
        let result = sqlx::query(
            r#"INSERT INTO spot_orders
               (user_id, pair_id, side, order_type, price, trigger_price, quantity, filled_quantity, status, idempotency_key)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON DUPLICATE KEY UPDATE id = LAST_INSERT_ID(id)"#,
        )
        .bind(user_id)
        .bind(pair_db_id)
        .bind(order_side_as_str(new_order.side))
        .bind(order_type_as_str(new_order.order_type))
        .bind(&new_order.price)
        .bind(&new_order.trigger_price)
        .bind(&new_order.quantity)
        .bind(&new_order.filled_quantity)
        .bind(order_status_as_str(new_order.status))
        .bind(idempotency_key)
        .execute(&self.pool)
        .await
        .map_err(map_spot_sqlx_error)?;
        self.load_order_async(&result.last_insert_id().to_string())
            .await
    }

    pub async fn load_order_async(
        &self,
        order_id: &str,
    ) -> Result<SpotOrder, crate::modules::spot::SpotServiceError> {
        let row = sqlx::query_as::<
            _,
            (
                u64,
                u64,
                String,
                String,
                String,
                Option<BigDecimal>,
                Option<BigDecimal>,
                BigDecimal,
                BigDecimal,
                String,
            ),
        >(
            r#"SELECT orders.id, orders.user_id, pairs.symbol, orders.side, orders.order_type,
                      orders.price, orders.trigger_price, orders.quantity, orders.filled_quantity, orders.status
               FROM spot_orders orders
               INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
               WHERE orders.id = ?
               LIMIT 1"#,
        )
        .bind(parse_spot_u64_identifier("order_id", order_id)?)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_spot_sqlx_error)?
        .ok_or_else(|| {
            crate::modules::spot::SpotServiceError::Repository(format!("missing spot order: {order_id}"))
        })?;

        Ok(SpotOrder {
            id: row.0.to_string(),
            user_id: row.1.to_string(),
            pair_id: row.2,
            side: parse_order_side(&row.3),
            order_type: parse_order_type(&row.4),
            price: row.5,
            trigger_price: row.6,
            quantity: row.7,
            filled_quantity: row.8,
            status: parse_order_status(&row.9),
        })
    }

    pub async fn save_order_async(
        &self,
        order: SpotOrder,
    ) -> Result<(), crate::modules::spot::SpotServiceError> {
        let order_db_id = order.id.parse::<u64>().map_err(|_| {
            crate::modules::spot::SpotServiceError::Repository(format!("invalid spot order id"))
        })?;
        let pair_db_id = resolve_pair_id(&self.pool, &order.pair_id).await?;
        sqlx::query(
            r#"UPDATE spot_orders
               SET pair_id = ?, side = ?, order_type = ?, price = ?, trigger_price = ?, quantity = ?,
                   filled_quantity = ?, status = ?
               WHERE id = ?"#,
        )
        .bind(pair_db_id)
        .bind(order_side_as_str(order.side))
        .bind(order_type_as_str(order.order_type))
        .bind(order.price)
        .bind(order.trigger_price)
        .bind(order.quantity)
        .bind(order.filled_quantity)
        .bind(order_status_as_str(order.status))
        .bind(order_db_id)
        .execute(&self.pool)
        .await
        .map_err(map_spot_sqlx_error)?;
        Ok(())
    }

    pub async fn insert_trade_async(
        &self,
        trade: NewSpotTrade,
    ) -> Result<SpotTrade, crate::modules::spot::SpotServiceError> {
        let pair_db_id = resolve_pair_id(&self.pool, &trade.pair_id).await?;
        let buy_order_id = parse_spot_u64_identifier("buy_order_id", &trade.buy_order_id)?;
        let sell_order_id = parse_spot_u64_identifier("sell_order_id", &trade.sell_order_id)?;
        let result = sqlx::query(
            r#"INSERT INTO spot_trades
               (pair_id, buy_order_id, sell_order_id, price, quantity, fee)
               VALUES (?, ?, ?, ?, ?, ?)"#,
        )
        .bind(pair_db_id)
        .bind(buy_order_id)
        .bind(sell_order_id)
        .bind(&trade.price)
        .bind(&trade.quantity)
        .bind(&trade.fee)
        .execute(&self.pool)
        .await
        .map_err(map_spot_sqlx_error)?;
        load_trade_by_id_async(&self.pool, result.last_insert_id()).await
    }

    pub async fn list_trades_by_pair_async(
        &self,
        pair_id: &str,
        limit: u32,
    ) -> Result<Vec<SpotTrade>, crate::modules::spot::SpotServiceError> {
        let pair_db_id = resolve_pair_id(&self.pool, pair_id).await?;
        let rows = sqlx::query_as::<
            _,
            (
                u64,
                String,
                u64,
                u64,
                BigDecimal,
                BigDecimal,
                BigDecimal,
                chrono::DateTime<chrono::Utc>,
            ),
        >(
            r#"SELECT trades.id, pairs.symbol, trades.buy_order_id, trades.sell_order_id,
                      trades.price, trades.quantity, trades.fee, trades.created_at
               FROM spot_trades trades
               INNER JOIN trading_pairs pairs ON pairs.id = trades.pair_id
               WHERE trades.pair_id = ?
               ORDER BY trades.id DESC
               LIMIT ?"#,
        )
        .bind(pair_db_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(map_spot_sqlx_error)?;

        let mut trades = Vec::with_capacity(rows.len());
        for row in rows {
            let (id, pair_id, buy_order_id, sell_order_id, price, quantity, fee, created_at) = row;
            trades.push(SpotTrade {
                id: id.to_string(),
                pair_id,
                buy_order_id: buy_order_id.to_string(),
                sell_order_id: sell_order_id.to_string(),
                price,
                quantity,
                fee,
                created_at,
            });
        }
        Ok(trades)
    }
}

pub(crate) async fn list_spot_orders(
    pool: &Pool<MySql>,
    filter: SpotOrderListFilter,
) -> AppResult<Vec<SpotOrderResponse>> {
    let mut builder = base_spot_orders_query(filter.include_internal);
    let has_filter = push_spot_order_filters(
        &mut builder,
        filter.user_id,
        filter.pair_id,
        filter.status,
        filter.email,
        false,
    );
    if !filter.include_internal {
        builder.push(if has_filter { " AND " } else { " WHERE " });
        builder.push("users.email <> ");
        builder.push_bind(SYSTEM_SPOT_LIQUIDITY_EMAIL);
    }
    builder.push(" ORDER BY orders.created_at DESC, orders.id DESC LIMIT ");
    builder.push_bind(i64::from(filter.limit));

    let rows = builder
        .build_query_as::<SpotOrderQueryRow>()
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(SpotOrderResponse::from).collect())
}

pub(crate) async fn load_spot_order_by_id(
    pool: &Pool<MySql>,
    order_id: u64,
) -> AppResult<SpotOrderResponse> {
    let mut builder = base_spot_orders_query(true);
    builder.push(" WHERE orders.id = ");
    builder.push_bind(order_id);
    builder
        .build_query_as::<SpotOrderQueryRow>()
        .fetch_optional(pool)
        .await?
        .map(SpotOrderResponse::from)
        .ok_or(AppError::NotFound)
}

pub(crate) async fn list_user_cancellable_spot_order_ids(
    pool: &Pool<MySql>,
    user_id: u64,
    pair_id: Option<String>,
) -> AppResult<Vec<u64>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT orders.id
           FROM spot_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           WHERE orders.user_id = "#,
    );
    builder.push_bind(user_id);
    builder.push(" AND orders.status IN ('pending', 'open', 'partially_filled')");
    if let Some(pair_id) = pair_id {
        let pair_db_id = pair_id.parse::<u64>().ok();
        builder.push(" AND (pairs.symbol = ");
        builder.push_bind(pair_id);
        builder.push(" OR pairs.id = ");
        builder.push_bind(pair_db_id);
        builder.push(")");
    }
    builder.push(" ORDER BY orders.id ASC");
    builder
        .build_query_scalar::<u64>()
        .fetch_all(pool)
        .await
        .map_err(AppError::from)
}

pub(crate) async fn list_spot_trades(
    pool: &Pool<MySql>,
    filter: SpotTradeListFilter,
) -> AppResult<Vec<SpotTradeResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT trades.id, pairs.symbol AS pair_id, trades.buy_order_id, trades.sell_order_id,
                  trades.price, trades.quantity, trades.fee, trades.created_at
           FROM spot_trades trades
           INNER JOIN trading_pairs pairs ON pairs.id = trades.pair_id
           INNER JOIN spot_orders buy_orders ON buy_orders.id = trades.buy_order_id
           INNER JOIN spot_orders sell_orders ON sell_orders.id = trades.sell_order_id"#,
    );
    let mut has_filter = false;
    if let Some(pair_id) = filter.pair_id {
        builder.push(" WHERE pairs.symbol = ");
        builder.push_bind(pair_id);
        has_filter = true;
    }
    if let Some(user_id) = filter.user_id {
        builder.push(if has_filter { " AND " } else { " WHERE " });
        builder.push("(buy_orders.user_id = ");
        builder.push_bind(user_id);
        builder.push(" OR sell_orders.user_id = ");
        builder.push_bind(user_id);
        builder.push(")");
        has_filter = true;
    }
    if let Some(email) = filter.email {
        builder.push(if has_filter { " AND " } else { " WHERE " });
        builder.push(
            r#"EXISTS (
                   SELECT 1 FROM users
                   WHERE users.email = "#,
        );
        builder.push_bind(email);
        builder.push(" AND (users.id = buy_orders.user_id OR users.id = sell_orders.user_id))");
        has_filter = true;
    }
    if !filter.include_internal {
        builder.push(if has_filter { " AND " } else { " WHERE " });
        builder.push(
            r#"NOT EXISTS (
                   SELECT 1 FROM users
                   WHERE users.email = "#,
        );
        builder.push_bind(SYSTEM_SPOT_LIQUIDITY_EMAIL);
        builder.push(" AND (users.id = buy_orders.user_id OR users.id = sell_orders.user_id))");
    }
    builder.push(" ORDER BY trades.created_at DESC, trades.id DESC LIMIT ");
    builder.push_bind(i64::from(filter.limit));

    let rows = builder
        .build_query_as::<SpotTradeQueryRow>()
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(SpotTradeResponse::from).collect())
}

pub(crate) async fn load_existing_spot_trade_by_idempotency_key(
    tx: &mut Transaction<'_, MySql>,
    idempotency_key: &str,
) -> AppResult<Option<SpotTrade>> {
    let trade = sqlx::query_as::<_, SpotTradeQueryRow>(
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

pub(crate) async fn insert_spot_trade(
    tx: &mut Transaction<'_, MySql>,
    buy_order: &SpotOrder,
    sell_order: &SpotOrder,
    price: &BigDecimal,
    quantity: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<SpotTrade> {
    let pair_id = spot_pair_db_id_in_tx(tx, &buy_order.pair_id).await?;
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

pub(crate) async fn save_spot_order_fill_state(
    tx: &mut Transaction<'_, MySql>,
    order: &SpotOrder,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE spot_orders
           SET filled_quantity = ?, status = ?
           WHERE id = ?"#,
    )
    .bind(&order.filled_quantity)
    .bind(order_status_as_str(order.status))
    .bind(parse_spot_order_db_id(order)?)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn latest_spot_market_price(
    redis: Option<&ConnectionManager>,
    pair_symbol: &str,
) -> AppResult<Option<BigDecimal>> {
    let Some(redis) = redis else {
        return Ok(None);
    };
    let mut connection = redis.clone();
    let payload: Option<String> = connection
        .get(market_ticker_redis_key(pair_symbol))
        .await
        .map_err(AppError::from)?;
    let Some(payload) = payload else {
        return Ok(None);
    };
    let value = serde_json::from_str::<Value>(&payload)
        .map_err(|error| AppError::Internal(format!("invalid cached ticker payload: {error}")))?;
    let last_price = value
        .get("last_price")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Internal("cached ticker is missing last_price".to_owned()))?;
    let price = BigDecimal::from_str(last_price)
        .map_err(|_| AppError::Internal("cached ticker last_price is invalid".to_owned()))?;
    if price <= BigDecimal::from(0) {
        return Err(AppError::Validation(
            "market price must be positive".to_owned(),
        ));
    }
    let observed_at = value
        .get("observed_at")
        .and_then(Value::as_i64)
        .ok_or_else(|| AppError::Internal("cached ticker is missing observed_at".to_owned()))?;
    let stale_before = chrono::Utc::now().timestamp_millis() - 60_000;
    if observed_at < stale_before {
        return Err(AppError::Validation("spot ticker is stale".to_owned()));
    }
    Ok(Some(price))
}

pub(crate) async fn load_spot_order_by_idempotency_key<'e, E>(
    executor: E,
    idempotency_key: &str,
) -> AppResult<Option<SpotIdempotentOrderRecord>>
where
    E: sqlx::Executor<'e, Database = MySql>,
{
    let row = sqlx::query_as::<_, IdempotentSpotOrderRow>(
        r#"SELECT orders.id, orders.user_id, orders.pair_id AS pair_db_id,
                  pairs.symbol AS pair_id, orders.side, orders.order_type, orders.price, orders.trigger_price,
                  orders.quantity, orders.filled_quantity, orders.status, orders.created_at,
                  orders.reserved_amount, orders.request_reference_price, orders.request_price
           FROM spot_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           WHERE orders.idempotency_key = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(idempotency_key)
    .fetch_optional(executor)
    .await?;
    Ok(row.map(SpotIdempotentOrderRecord::from))
}

pub(crate) async fn load_spot_pair_db_id(pool: &Pool<MySql>, pair_symbol: &str) -> AppResult<u64> {
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

pub(crate) async fn triggered_limit_buy_order_ids(
    pool: &Pool<MySql>,
    pair_symbol: &str,
    market_price: &BigDecimal,
    limit: u32,
) -> AppResult<Vec<u64>> {
    let rows = sqlx::query_as::<_, (u64,)>(
        r#"SELECT orders.id
           FROM spot_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           WHERE REPLACE(REPLACE(REPLACE(UPPER(pairs.symbol), '-', ''), '/', ''), '_', '') =
                 REPLACE(REPLACE(REPLACE(UPPER(?), '-', ''), '/', ''), '_', '')
             AND orders.side = 'buy'
             AND orders.order_type = 'limit'
             AND orders.status IN ('pending', 'open', 'partially_filled')
             AND orders.price >= ?
           ORDER BY orders.price DESC, orders.id ASC
           LIMIT ?"#,
    )
    .bind(pair_symbol)
    .bind(market_price)
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| row.0).collect())
}

pub(crate) async fn triggered_limit_sell_order_ids(
    pool: &Pool<MySql>,
    pair_symbol: &str,
    market_price: &BigDecimal,
    limit: u32,
) -> AppResult<Vec<u64>> {
    let rows = sqlx::query_as::<_, (u64,)>(
        r#"SELECT orders.id
           FROM spot_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           WHERE REPLACE(REPLACE(REPLACE(UPPER(pairs.symbol), '-', ''), '/', ''), '_', '') =
                 REPLACE(REPLACE(REPLACE(UPPER(?), '-', ''), '/', ''), '_', '')
             AND orders.side = 'sell'
             AND orders.order_type = 'limit'
             AND orders.status IN ('pending', 'open', 'partially_filled')
             AND orders.price <= ?
           ORDER BY orders.price ASC, orders.id ASC
           LIMIT ?"#,
    )
    .bind(pair_symbol)
    .bind(market_price)
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| row.0).collect())
}

pub(crate) async fn triggered_stop_limit_buy_order_ids(
    pool: &Pool<MySql>,
    pair_symbol: &str,
    market_price: &BigDecimal,
    limit: u32,
) -> AppResult<Vec<u64>> {
    let rows = sqlx::query_as::<_, (u64,)>(
        r#"SELECT orders.id
           FROM spot_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           WHERE REPLACE(REPLACE(REPLACE(UPPER(pairs.symbol), '-', ''), '/', ''), '_', '') =
                 REPLACE(REPLACE(REPLACE(UPPER(?), '-', ''), '/', ''), '_', '')
             AND orders.side = 'buy'
             AND orders.order_type = 'stop_limit'
             AND orders.status IN ('pending', 'open', 'partially_filled')
             AND orders.trigger_price >= ?
             AND orders.price >= ?
           ORDER BY orders.trigger_price DESC, orders.price DESC, orders.id ASC
           LIMIT ?"#,
    )
    .bind(pair_symbol)
    .bind(market_price)
    .bind(market_price)
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| row.0).collect())
}

pub(crate) async fn triggered_stop_limit_sell_order_ids(
    pool: &Pool<MySql>,
    pair_symbol: &str,
    market_price: &BigDecimal,
    limit: u32,
) -> AppResult<Vec<u64>> {
    let rows = sqlx::query_as::<_, (u64,)>(
        r#"SELECT orders.id
           FROM spot_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           WHERE REPLACE(REPLACE(REPLACE(UPPER(pairs.symbol), '-', ''), '/', ''), '_', '') =
                 REPLACE(REPLACE(REPLACE(UPPER(?), '-', ''), '/', ''), '_', '')
             AND orders.side = 'sell'
             AND orders.order_type = 'stop_limit'
             AND orders.status IN ('pending', 'open', 'partially_filled')
             AND orders.trigger_price <= ?
             AND orders.price <= ?
           ORDER BY orders.trigger_price ASC, orders.price ASC, orders.id ASC
           LIMIT ?"#,
    )
    .bind(pair_symbol)
    .bind(market_price)
    .bind(market_price)
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| row.0).collect())
}

pub(crate) fn is_duplicate_key_error(error: &sqlx::Error) -> bool {
    error.as_database_error().is_some_and(|database_error| {
        database_error.code().as_deref() == Some("1062")
            || database_error.code().as_deref() == Some("23000")
    })
}

pub(crate) async fn insert_spot_order_in_tx(
    tx: &mut Transaction<'_, MySql>,
    new_order: NewOrder,
    pair_db_id: u64,
    idempotency_key: Option<&str>,
    request_price: Option<&BigDecimal>,
    reference_price: Option<&BigDecimal>,
    reservation: &CreateSpotOrderReservation,
) -> AppResult<(SpotOrder, bool)> {
    // 下单记录和钱包冻结必须同事务提交；重复幂等键命中时只返回原订单，避免再次冻结钱包。
    let user_id = new_order
        .user_id
        .parse::<u64>()
        .map_err(|_| AppError::Unauthorized)?;
    let insert_result = sqlx::query(
        r#"INSERT INTO spot_orders
           (user_id, pair_id, side, order_type, price, trigger_price, quantity, filled_quantity, status,
            idempotency_key, reserved_asset, reserved_amount, request_reference_price, request_price)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(pair_db_id)
    .bind(order_side_as_str(new_order.side))
    .bind(order_type_as_str(new_order.order_type))
    .bind(&new_order.price)
    .bind(&new_order.trigger_price)
    .bind(&new_order.quantity)
    .bind(&new_order.filled_quantity)
    .bind(order_status_as_str(new_order.status))
    .bind(idempotency_key)
    .bind(reservation.asset_id)
    .bind(&reservation.amount)
    .bind(match new_order.order_type {
        OrderType::Limit | OrderType::StopLimit => None,
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
            let existing = load_spot_order_by_idempotency_key(&mut **tx, idempotency_key)
                .await?
                .ok_or(AppError::NotFound)?;
            if existing.user_id != user_id {
                return Err(AppError::Conflict(
                    "spot order idempotency key belongs to another user".to_owned(),
                ));
            }
            ensure_spot_order_idempotency_matches_insert(
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

    let mut builder = base_spot_orders_query(true);
    builder.push(" WHERE orders.id = ");
    builder.push_bind(order_id);
    builder.push(" LIMIT 1 FOR UPDATE");
    let row = builder
        .build_query_as::<SpotOrderQueryRow>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok((SpotOrderResponse::from(row).into(), is_new_order))
}

pub(crate) async fn insert_spot_liquidity_sell_order_in_tx(
    tx: &mut Transaction<'_, MySql>,
    liquidity_user_id: u64,
    buy_order: &SpotOrder,
    execution_price: &BigDecimal,
    fill_quantity: &BigDecimal,
) -> AppResult<SpotOrder> {
    let new_order = NewOrder {
        user_id: liquidity_user_id.to_string(),
        pair_id: buy_order.pair_id.clone(),
        side: OrderSide::Sell,
        order_type: OrderType::Limit,
        price: Some(execution_price.clone()),
        trigger_price: None,
        quantity: fill_quantity.clone(),
        filled_quantity: BigDecimal::from(0),
        status: OrderStatus::Pending,
    };
    let reservation = CreateSpotOrderReservation {
        asset_id: pair_assets_in_tx(tx, &buy_order.pair_id)
            .await?
            .base_asset_id,
        amount: fill_quantity.clone(),
    };
    let pair_db_id = spot_pair_db_id_in_tx(tx, &buy_order.pair_id).await?;
    let system_order_key = format!("spot_system_liquidity:{}", buy_order.id);
    let (order, _) = insert_spot_order_in_tx(
        tx,
        new_order,
        pair_db_id,
        Some(&system_order_key),
        Some(execution_price),
        None,
        &reservation,
    )
    .await?;
    Ok(order)
}

pub(crate) async fn insert_spot_liquidity_buy_order_in_tx(
    tx: &mut Transaction<'_, MySql>,
    liquidity_user_id: u64,
    sell_order: &SpotOrder,
    execution_price: &BigDecimal,
    fill_quantity: &BigDecimal,
) -> AppResult<SpotOrder> {
    let fill_quote_amount = execution_price.clone() * fill_quantity.clone();
    let new_order = NewOrder {
        user_id: liquidity_user_id.to_string(),
        pair_id: sell_order.pair_id.clone(),
        side: OrderSide::Buy,
        order_type: OrderType::Limit,
        price: Some(execution_price.clone()),
        trigger_price: None,
        quantity: fill_quantity.clone(),
        filled_quantity: BigDecimal::from(0),
        status: OrderStatus::Pending,
    };
    let reservation = CreateSpotOrderReservation {
        asset_id: pair_assets_in_tx(tx, &sell_order.pair_id)
            .await?
            .quote_asset_id,
        amount: fill_quote_amount,
    };
    let pair_db_id = spot_pair_db_id_in_tx(tx, &sell_order.pair_id).await?;
    let system_order_key = format!("spot_system_liquidity_buy:{}", sell_order.id);
    let (order, _) = insert_spot_order_in_tx(
        tx,
        new_order,
        pair_db_id,
        Some(&system_order_key),
        Some(execution_price),
        None,
        &reservation,
    )
    .await?;
    Ok(order)
}

pub(crate) async fn lock_spot_fill_orders_in_order(
    tx: &mut Transaction<'_, MySql>,
    buy_order_id: &str,
    sell_order_id: &str,
) -> AppResult<(SpotOrder, SpotOrder)> {
    let buy_order_db_id = parse_spot_order_request_id(buy_order_id)?;
    let sell_order_db_id = parse_spot_order_request_id(sell_order_id)?;
    let mut buy_order = None;
    let mut sell_order = None;
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

pub(crate) async fn lock_spot_order_by_db_id(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
) -> AppResult<SpotOrder> {
    let row = sqlx::query_as::<_, SpotOrderLockRow>(
        r#"SELECT id, user_id, pair_id, side, order_type, price, trigger_price, quantity,
                  filled_quantity, status
           FROM spot_orders
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(order_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    let pair_symbol = spot_pair_symbol_in_tx(tx, row.pair_id).await?;
    Ok(SpotOrder {
        id: row.id.to_string(),
        user_id: row.user_id.to_string(),
        pair_id: pair_symbol,
        side: parse_order_side(&row.side),
        order_type: parse_order_type(&row.order_type),
        price: row.price,
        trigger_price: row.trigger_price,
        quantity: row.quantity,
        filled_quantity: row.filled_quantity,
        status: parse_order_status(&row.status),
    })
}

async fn cancel_locked_spot_order_and_unfreeze_wallet(
    tx: &mut Transaction<'_, MySql>,
    order: SpotOrder,
    user_id: u64,
) -> AppResult<SpotCancelRepositoryResult> {
    let (order, cancelled) = cancel_spot_order_state(order)?;
    if !cancelled {
        return Ok(SpotCancelRepositoryResult { order, cancelled });
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
    Ok(SpotCancelRepositoryResult { order, cancelled })
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

async fn remaining_spot_order_reservation_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &SpotOrder,
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

pub(crate) async fn remaining_spot_fill_reservation_before_trade_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &SpotOrder,
    current_trade_id: &str,
) -> AppResult<CreateSpotOrderReservation> {
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
            let reservation = remaining_tracked_reservation_excluding_trade_in_tx(
                tx,
                order,
                asset_id,
                total_amount,
                Some(trade_id),
            )
            .await?;
            return Ok(CreateSpotOrderReservation {
                asset_id: reservation.asset_id,
                amount: reservation.amount,
            });
        }
        return Ok(CreateSpotOrderReservation {
            asset_id,
            amount: BigDecimal::from(0),
        });
    }

    let reservation = remaining_legacy_spot_reservation_in_tx(tx, order).await?;
    Ok(CreateSpotOrderReservation {
        asset_id: reservation.asset_id,
        amount: reservation.amount,
    })
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
    .map_err(map_spot_service_error)?;
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
    let spent_amount = filled_spot_order_reservation_in_tx(tx, order).await?;
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

async fn filled_spot_order_reservation_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &SpotOrder,
) -> AppResult<BigDecimal> {
    let order_id = parse_spot_order_db_id(order)?;
    let (filled_amount,): (Option<BigDecimal>,) = match order.side {
        OrderSide::Buy => {
            sqlx::query_as(
                r#"SELECT COALESCE(SUM(price * quantity), 0)
                   FROM spot_trades
                   WHERE buy_order_id = ?"#,
            )
            .bind(order_id)
            .fetch_one(&mut **tx)
            .await?
        }
        OrderSide::Sell => {
            sqlx::query_as(
                r#"SELECT COALESCE(SUM(quantity), 0)
                   FROM spot_trades
                   WHERE sell_order_id = ?"#,
            )
            .bind(order_id)
            .fetch_one(&mut **tx)
            .await?
        }
    };
    Ok(filled_amount.unwrap_or_else(|| BigDecimal::from(0)))
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

pub(crate) async fn pair_assets_in_tx(
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

pub(crate) async fn spot_order_reservation_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &NewOrder,
    reference_price: Option<&BigDecimal>,
) -> AppResult<CreateSpotOrderReservation> {
    let assets = pair_assets_in_tx(tx, &order.pair_id).await?;
    spot_order_reservation(
        order,
        reference_price,
        assets.base_asset_id,
        assets.quote_asset_id,
    )
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

pub(crate) async fn apply_spot_wallet_freeze(
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

pub(crate) async fn freeze_wallet_for_inserted_order_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &SpotOrder,
    reservation: &CreateSpotOrderReservation,
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

pub(crate) async fn lock_spot_fill_wallet_rows_in_order(
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

pub(crate) async fn apply_spot_wallet_settlement_leg(
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

pub(crate) async fn ensure_spot_liquidity_user_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO users (email, password_hash, status)
           VALUES (?, ?, 'active')
           ON DUPLICATE KEY UPDATE id = LAST_INSERT_ID(id)"#,
    )
    .bind(SYSTEM_SPOT_LIQUIDITY_EMAIL)
    .bind(SYSTEM_SPOT_LIQUIDITY_PASSWORD_HASH)
    .execute(&mut **tx)
    .await?;
    let user_id = result.last_insert_id();
    if user_id > 0 {
        return Ok(user_id);
    }
    let (user_id,): (u64,) = sqlx::query_as("SELECT id FROM users WHERE email = ? LIMIT 1")
        .bind(SYSTEM_SPOT_LIQUIDITY_EMAIL)
        .fetch_one(&mut **tx)
        .await?;
    Ok(user_id)
}

pub(crate) async fn ensure_wallet_account_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT IGNORE INTO wallet_accounts (user_id, asset_id, available, frozen, locked)
           VALUES (?, ?, 0, 0, 0)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn credit_spot_liquidity_wallet_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    change_type: &str,
    ref_type: &str,
    ref_id: &str,
) -> AppResult<()> {
    ensure_wallet_account_in_tx(tx, user_id, asset_id).await?;
    let wallet = lock_wallet_row(tx, user_id, asset_id).await?;
    let available_after = wallet.available.clone() + amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
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
        &wallet.frozen,
        &wallet.locked,
        change_type,
        ref_type,
        ref_id,
    )
    .await
}

pub(crate) async fn release_buy_order_surplus_reservation_after_fill(
    tx: &mut Transaction<'_, MySql>,
    buyer_id: u64,
    buy_order: &SpotOrder,
    reservation_before_fill: &CreateSpotOrderReservation,
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

async fn update_spot_order_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &SpotOrder,
) -> AppResult<()> {
    let pair_db_id = spot_pair_db_id_in_tx(tx, &order.pair_id).await?;
    sqlx::query(
        r#"UPDATE spot_orders
           SET pair_id = ?, side = ?, order_type = ?, price = ?, trigger_price = ?, quantity = ?,
               filled_quantity = ?, status = ?
           WHERE id = ?"#,
    )
    .bind(pair_db_id)
    .bind(order_side_as_str(order.side))
    .bind(order_type_as_str(order.order_type))
    .bind(&order.price)
    .bind(&order.trigger_price)
    .bind(&order.quantity)
    .bind(&order.filled_quantity)
    .bind(order_status_as_str(order.status))
    .bind(parse_spot_order_db_id(order)?)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn spot_pair_db_id_in_tx(
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
    .bind(entry.reason)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn parse_spot_order_db_id(order: &SpotOrder) -> AppResult<u64> {
    order
        .id
        .parse::<u64>()
        .map_err(|_| AppError::Validation("invalid spot order id".to_owned()))
}

fn base_spot_orders_query(include_internal_trades: bool) -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(format!(
        r#"SELECT orders.id, orders.user_id, users.email AS user_email, pairs.symbol AS pair_id, orders.side,
                  orders.order_type, orders.price, orders.trigger_price, orders.quantity, orders.filled_quantity,
                  orders.status, orders.created_at,
                  {} AS average_price
           FROM spot_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           LEFT JOIN users ON users.id = orders.user_id"#,
        spot_order_average_price_sql(include_internal_trades)
    ))
}

fn spot_order_average_price_sql(include_internal_trades: bool) -> &'static str {
    if include_internal_trades {
        return r#"CAST((
             SELECT SUM(trades.price * trades.quantity) / NULLIF(SUM(trades.quantity), 0)
             FROM spot_trades trades
             WHERE trades.buy_order_id = orders.id OR trades.sell_order_id = orders.id
           ) AS DECIMAL(38,18))"#;
    }
    r#"CAST((
             SELECT SUM(trades.price * trades.quantity) / NULLIF(SUM(trades.quantity), 0)
             FROM spot_trades trades
             INNER JOIN spot_orders average_buy_orders ON average_buy_orders.id = trades.buy_order_id
             INNER JOIN users average_buy_users ON average_buy_users.id = average_buy_orders.user_id
             INNER JOIN spot_orders average_sell_orders ON average_sell_orders.id = trades.sell_order_id
             INNER JOIN users average_sell_users ON average_sell_users.id = average_sell_orders.user_id
             WHERE (trades.buy_order_id = orders.id OR trades.sell_order_id = orders.id)
               AND average_buy_users.email <> '__system_spot_liquidity@internal.local'
               AND average_sell_users.email <> '__system_spot_liquidity@internal.local'
           ) AS DECIMAL(38,18))"#
}

fn push_spot_order_filters(
    builder: &mut QueryBuilder<'_, MySql>,
    user_id: Option<u64>,
    pair_id: Option<String>,
    status: Option<String>,
    email: Option<String>,
    mut has_filter: bool,
) -> bool {
    if let Some(user_id) = user_id {
        builder.push(if has_filter { " AND " } else { " WHERE " });
        builder.push("orders.user_id = ");
        builder.push_bind(user_id);
        has_filter = true;
    }
    if let Some(pair_id) = pair_id {
        builder.push(if has_filter { " AND " } else { " WHERE " });
        builder.push("pairs.symbol = ");
        builder.push_bind(pair_id);
        has_filter = true;
    }
    if let Some(status) = status {
        builder.push(if has_filter { " AND " } else { " WHERE " });
        builder.push("orders.status = ");
        builder.push_bind(status);
        has_filter = true;
    }
    if let Some(email) = email {
        builder.push(if has_filter { " AND " } else { " WHERE " });
        builder.push("users.email = ");
        builder.push_bind(email);
        has_filter = true;
    }
    has_filter
}

impl From<SpotOrderQueryRow> for SpotOrderResponse {
    fn from(order: SpotOrderQueryRow) -> Self {
        Self {
            id: order.id.to_string(),
            user_id: order.user_id.to_string(),
            user_email: order.user_email,
            pair_id: order.pair_id,
            side: parse_order_side(&order.side),
            order_type: parse_order_type(&order.order_type),
            price: order.price,
            trigger_price: order.trigger_price,
            quantity: order.quantity,
            filled_quantity: order.filled_quantity,
            average_price: order.average_price,
            status: parse_order_status(&order.status),
            created_at: Some(order.created_at),
        }
    }
}

impl From<SpotTradeQueryRow> for SpotTradeResponse {
    fn from(row: SpotTradeQueryRow) -> Self {
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

impl From<SpotTradeQueryRow> for SpotTrade {
    fn from(row: SpotTradeQueryRow) -> Self {
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

impl From<IdempotentSpotOrderRow> for SpotIdempotentOrderRecord {
    fn from(order: IdempotentSpotOrderRow) -> Self {
        Self {
            id: order.id,
            user_id: order.user_id,
            pair_db_id: order.pair_db_id,
            pair_id: order.pair_id,
            side: parse_order_side(&order.side),
            order_type: parse_order_type(&order.order_type),
            price: order.price,
            trigger_price: order.trigger_price,
            quantity: order.quantity,
            filled_quantity: order.filled_quantity,
            status: parse_order_status(&order.status),
            created_at: order.created_at,
            reserved_amount: order.reserved_amount,
            request_reference_price: order.request_reference_price,
            request_price: order.request_price,
        }
    }
}

impl From<SpotIdempotentOrderRecord> for SpotOrderResponse {
    fn from(order: SpotIdempotentOrderRecord) -> Self {
        Self {
            id: order.id.to_string(),
            user_id: order.user_id.to_string(),
            user_email: None,
            pair_id: order.pair_id,
            side: order.side,
            order_type: order.order_type,
            price: order.price,
            trigger_price: order.trigger_price,
            quantity: order.quantity,
            filled_quantity: order.filled_quantity,
            average_price: None,
            status: order.status,
            created_at: Some(order.created_at),
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
        "stop_limit" => OrderType::StopLimit,
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

fn order_side_as_str(side: OrderSide) -> &'static str {
    match side {
        OrderSide::Buy => "buy",
        OrderSide::Sell => "sell",
    }
}

fn order_type_as_str(order_type: OrderType) -> &'static str {
    match order_type {
        OrderType::Limit => "limit",
        OrderType::Market => "market",
        OrderType::StopLimit => "stop_limit",
    }
}

fn map_spot_service_error(error: SpotServiceError) -> AppError {
    match error {
        SpotServiceError::Repository(message) if message.starts_with("missing") => {
            AppError::NotFound
        }
        SpotServiceError::Repository(message) => AppError::Internal(message),
        SpotServiceError::Domain(error) => {
            AppError::Validation(format!("invalid spot order: {error:?}"))
        }
        SpotServiceError::Wallet(error) => AppError::Validation(format!("wallet error: {error:?}")),
        SpotServiceError::MissingPriceForWalletReservation => {
            AppError::Validation("price is required for wallet reservation".to_owned())
        }
        SpotServiceError::MissingReferencePriceForMarketOrder => {
            AppError::Validation("reference_price is required for market orders".to_owned())
        }
        SpotServiceError::MissingTriggerPriceForStopLimitOrder => {
            AppError::Validation("trigger_price is required for stop limit orders".to_owned())
        }
    }
}

fn map_spot_sqlx_error(error: sqlx::Error) -> crate::modules::spot::SpotServiceError {
    crate::modules::spot::SpotServiceError::Repository(error.to_string())
}

fn parse_spot_u64_identifier(
    field: &str,
    value: &str,
) -> Result<u64, crate::modules::spot::SpotServiceError> {
    value.parse::<u64>().map_err(|error| {
        crate::modules::spot::SpotServiceError::Repository(format!(
            "invalid numeric {field} `{value}`: {error}"
        ))
    })
}

async fn resolve_pair_id(
    pool: &Pool<MySql>,
    pair_id: &str,
) -> Result<u64, crate::modules::spot::SpotServiceError> {
    if let Ok(pair_db_id) = pair_id.parse::<u64>() {
        return Ok(pair_db_id);
    }

    sqlx::query_as::<_, (u64,)>(r#"SELECT id FROM trading_pairs WHERE symbol = ? LIMIT 1"#)
        .bind(pair_id)
        .fetch_optional(pool)
        .await
        .map_err(map_spot_sqlx_error)?
        .map(|(id,)| id)
        .ok_or_else(|| {
            crate::modules::spot::SpotServiceError::Repository(format!(
                "missing trading pair: {pair_id}"
            ))
        })
}

async fn load_trade_by_id_async(
    pool: &Pool<MySql>,
    trade_id: u64,
) -> Result<SpotTrade, crate::modules::spot::SpotServiceError> {
    let row = sqlx::query_as::<
        _,
        (
            u64,
            String,
            u64,
            u64,
            BigDecimal,
            BigDecimal,
            BigDecimal,
            chrono::DateTime<chrono::Utc>,
        ),
    >(
        r#"SELECT trades.id, pairs.symbol, trades.buy_order_id, trades.sell_order_id,
                      trades.price, trades.quantity, trades.fee, trades.created_at
               FROM spot_trades trades
               INNER JOIN trading_pairs pairs ON pairs.id = trades.pair_id
               WHERE trades.id = ?
               LIMIT 1"#,
    )
    .bind(trade_id)
    .fetch_optional(pool)
    .await
    .map_err(map_spot_sqlx_error)?
    .ok_or_else(|| {
        crate::modules::spot::SpotServiceError::Repository(format!(
            "missing spot trade: {trade_id}"
        ))
    })?;

    Ok(SpotTrade {
        id: row.0.to_string(),
        pair_id: row.1,
        buy_order_id: row.2.to_string(),
        sell_order_id: row.3.to_string(),
        price: row.4,
        quantity: row.5,
        fee: row.6,
        created_at: row.7,
    })
}
