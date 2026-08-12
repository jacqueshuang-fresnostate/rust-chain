//! 现货订单写仓储、幂等插入、行锁与取消事务实现。
//!
//! 用户/后台撤单事务由 `SqlxSpotOrderCancelRepository` 持有：先锁订单，再核算剩余冻结额，
//! 随后释放钱包并更新订单；后台审计与订单状态在同一事务提交。下单调用方仍持有创建事务，
//! 仅当本模块返回新订单时才可继续执行钱包冻结，幂等重放不得产生第二次资金变动。

use super::{
    common::{
        is_duplicate_key_error, order_side_as_str, order_status_as_str, order_type_as_str,
        parse_order_side, parse_order_status, parse_order_type, parse_spot_order_db_id,
    },
    read_models::{SpotOrderQueryRow, base_spot_orders_query},
    trade_settlement::{
        pair_assets_in_tx, remaining_spot_order_reservation_in_tx, spot_pair_db_id_in_tx,
    },
    wallet_accounts::apply_spot_wallet_unfreeze,
};
use crate::{
    error::{AppError, AppResult},
    modules::spot::{
        NewOrder, OrderSide, OrderStatus, OrderType, SpotOrder,
        presentation::SpotOrderResponse,
        repository::{
            SpotAdminCancelCommand, SpotCancelRepositoryResult, SpotIdempotentOrderRecord,
            SpotOrderCancelRepository, SpotUserCancelCommand,
        },
        service::{
            SpotOrderReservation as CreateSpotOrderReservation, cancel_spot_order_state,
            ensure_spot_order_idempotency_matches_insert, parse_spot_order_request_id,
            spot_fill_order_lock_keys, spot_order_audit_json,
        },
    },
};
use axum::async_trait;
use bigdecimal::BigDecimal;
use serde_json::Value;
use sqlx::{MySql, Pool, Transaction, types::Json as SqlxJson};

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

/// 在调用方事务中插入并锁定现货订单；订单保留资产与金额必须已按交易对和订单类型计算完成。
/// 唯一幂等键命中时仅允许同一用户且请求价、参考价和保留金额完全一致的重放，并返回既有订单而不触发第二次冻结。
/// 本函数不提交事务且不写钱包；新订单返回 `is_new_order=true` 后，调用方必须在同一事务内完成冻结再提交。
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
