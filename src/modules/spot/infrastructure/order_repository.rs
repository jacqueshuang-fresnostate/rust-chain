//! 现货订单写仓储、幂等插入、行锁与取消事务实现。
//!
//! 用户/后台撤单事务由 `SqlxSpotOrderCancelRepository` 持有：先锁订单，再核算剩余冻结额，
//! 随后释放钱包并更新订单；后台审计与订单状态在同一事务提交。下单调用方仍持有创建事务，
//! 仅当本模块返回新订单时才可继续执行钱包冻结，幂等重放不得产生第二次资金变动。

use super::{
    common::{
        order_side_as_str, order_status_as_str, order_type_as_str, parse_order_side,
        parse_order_status, parse_order_type, parse_spot_order_db_id,
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
            SpotOrderRequestIdentity, SpotOrderReservation as CreateSpotOrderReservation,
            cancel_spot_order_state, ensure_spot_order_idempotency_matches_insert,
            parse_spot_order_request_id, spot_fill_order_lock_keys, spot_order_audit_json,
            spot_order_idempotency_response,
        },
    },
};
use axum::async_trait;
use bigdecimal::BigDecimal;
use serde_json::Value;
use sqlx::{MySql, Pool, Transaction, types::Json as SqlxJson};
use uuid::Uuid;

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
    request_fingerprint: Option<String>,
    idempotency_attempt_token: Option<String>,
    idempotency_response_json: Option<SqlxJson<Value>>,
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
    /// 保存 MySQL 池供现货撤单仓储开启独立事务并锁定订单、钱包和预留流水。
    /// 构造时不获取连接；撤单幂等与解冻一致性由仓储方法的事务实现保证。
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

/// 从 MySQL 持久化数据读取现货订单，保持现货既有归属过滤、可见性及排序条件。
/// 按用户和幂等键读取订单快照，供建单前重放核对而不再次冻结余额。
/// 锁定范围显式限定为 `orders`：并发失败方在 RR 事务中能看见唯一键胜者，
/// 同时不会锁住 JOIN 的交易对行而与 INSERT 外键共享锁形成倒序。
pub(crate) async fn load_spot_order_by_idempotency_key<'e, E>(
    executor: E,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<SpotIdempotentOrderRecord>>
where
    E: sqlx::Executor<'e, Database = MySql>,
{
    let row = sqlx::query_as::<_, IdempotentSpotOrderRow>(
        r#"SELECT orders.id, orders.user_id, orders.pair_id AS pair_db_id,
                  pairs.symbol AS pair_id, orders.side, orders.order_type, orders.price, orders.trigger_price,
                  orders.quantity, orders.filled_quantity, orders.status, orders.created_at,
                  orders.reserved_amount, orders.request_reference_price, orders.request_price,
                  orders.request_fingerprint, orders.idempotency_attempt_token,
                  orders.idempotency_response_json
           FROM spot_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           WHERE orders.user_id = ? AND orders.idempotency_key = ?
           LIMIT 1
           FOR UPDATE OF orders"#,
    )
    .bind(user_id)
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
    request_identity: SpotOrderRequestIdentity<'_>,
    reservation: &CreateSpotOrderReservation,
) -> AppResult<(SpotOrder, bool)> {
    // 下单记录和钱包冻结必须同事务提交；ON DUPLICATE KEY 会串行化同键竞争，
    // 避免多个失败 INSERT 持有外键共享锁后再升级订单锁所形成的死锁环。
    let user_id = new_order
        .user_id
        .parse::<u64>()
        .map_err(|_| AppError::Unauthorized)?;
    let insert_attempt_token = Uuid::now_v7().to_string();
    let insert_result = sqlx::query(
        r#"INSERT INTO spot_orders
           (user_id, pair_id, side, order_type, price, trigger_price, quantity, filled_quantity, status,
            idempotency_key, request_fingerprint, idempotency_attempt_token,
            reserved_asset, reserved_amount,
            request_reference_price, request_price)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
    .bind(request_identity.idempotency_key)
    .bind(request_identity.request_fingerprint)
    .bind(&insert_attempt_token)
    .bind(reservation.asset_id)
    .bind(&reservation.amount)
    .bind(match new_order.order_type {
        OrderType::Limit | OrderType::StopLimit => None,
        OrderType::Market => request_identity.request_reference_price,
    })
    .bind(request_identity.request_price)
    .execute(&mut **tx)
    .await;

    let result = insert_result.map_err(AppError::from)?;
    let order_id = result.last_insert_id();
    if let Some(idempotency_key) = request_identity.idempotency_key {
        let existing = load_spot_order_by_idempotency_key(&mut **tx, user_id, idempotency_key)
            .await?
            .ok_or(AppError::NotFound)?;
        if existing.idempotency_attempt_token.as_deref() != Some(insert_attempt_token.as_str()) {
            ensure_spot_order_idempotency_matches_insert(
                &existing,
                &new_order,
                request_identity.request_price,
                request_identity.request_reference_price,
                reservation,
                request_identity.request_fingerprint,
            )?;
            return Ok((spot_order_idempotency_response(existing)?.into(), false));
        }
    }

    let mut builder = base_spot_orders_query(true);
    builder.push(" WHERE orders.id = ");
    builder.push_bind(order_id);
    // INSERT/ON DUPLICATE KEY 已经持有目标订单记录锁；这里保持普通一致性读，
    // 避免带 JOIN 的 FOR UPDATE 额外锁住 users/trading_pairs 并与并发外键检查倒序。
    builder.push(" LIMIT 1");
    let row = builder
        .build_query_as::<SpotOrderQueryRow>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok((SpotOrderResponse::from(row).into(), true))
}

/// 在建单事务提交前保存首次响应，重放始终返回该快照而不是订单当前状态。
pub(crate) async fn store_spot_order_idempotency_response_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order_id: &str,
    response: &SpotOrderResponse,
) -> AppResult<()> {
    let order_id = parse_spot_order_request_id(order_id)?;
    let snapshot = serde_json::to_value(response)
        .map_err(|error| AppError::Internal(format!("serialize spot order snapshot: {error}")))?;
    let result = sqlx::query(
        r#"UPDATE spot_orders
           SET idempotency_response_json = ?
           WHERE id = ? AND request_fingerprint IS NOT NULL"#,
    )
    .bind(SqlxJson(snapshot))
    .bind(order_id)
    .execute(&mut **tx)
    .await?;
    if result.rows_affected() != 1 {
        return Err(AppError::Internal(
            "spot order idempotency snapshot update affected an unexpected row count".to_owned(),
        ));
    }
    Ok(())
}

/// 在成交事务内创建做市卖单快照，数量和价格与用户买单成交腿严格对应。
/// 数据库失败由调用方回滚；涉及资金时余额、流水与业务状态必须同事务且幂等重放不重复入账。
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
        SpotOrderRequestIdentity {
            idempotency_key: Some(&system_order_key),
            request_fingerprint: None,
            request_price: Some(execution_price),
            request_reference_price: None,
        },
        &reservation,
    )
    .await?;
    Ok(order)
}

/// 在成交事务内创建做市买单快照，数量和价格与用户卖单成交腿严格对应。
/// 数据库失败由调用方回滚；涉及资金时余额、流水与业务状态必须同事务且幂等重放不重复入账。
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
        SpotOrderRequestIdentity {
            idempotency_key: Some(&system_order_key),
            request_fingerprint: None,
            request_price: Some(execution_price),
            request_reference_price: None,
        },
        &reservation,
    )
    .await?;
    Ok(order)
}

/// 在成交事务内按稳定订单主键顺序锁定买卖单，避免并发撮合形成反向锁等待。
/// 任一订单不存在或状态不允许成交时终止事务，尚未产生钱包或成交记录副作用。
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

/// 在调用方事务内锁定现货订单，固定现货后续校验与写入所依据的并发快照。
/// 调用方负责稳定锁序及提交回滚；记录缺失或锁失败时不得继续资金和状态写入。
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
    let request_context = crate::infra::admin_request_context::current_admin_request_context();
    sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, before_json, after_json, reason, ip, request_id)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(admin_id)
    .bind(entry.action)
    .bind(entry.target_type)
    .bind(entry.target_id)
    .bind(entry.before_json.map(SqlxJson))
    .bind(entry.after_json.map(SqlxJson))
    .bind(entry.reason)
    .bind(
        request_context
            .as_ref()
            .and_then(|context| context.source_ip.as_deref()),
    )
    .bind(
        request_context
            .as_ref()
            .map(|context| context.request_id.as_str()),
    )
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
            request_fingerprint: order.request_fingerprint,
            idempotency_attempt_token: order.idempotency_attempt_token,
            idempotency_response_json: order.idempotency_response_json.map(|value| value.0),
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
