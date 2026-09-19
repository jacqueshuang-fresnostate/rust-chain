//! 现货成交记录、订单成交状态与订单级冻结额核算。
//!
//! 本模块不开始或提交事务；应用层是成交事务 owner。调用方必须先按稳定顺序锁订单，
//! 再写入成交幂等占位，并在同一事务内完成钱包四条资金腿、价差释放和订单状态写回。
//! 冻结额核算兼容新订单快照、历史账本反推，并可排除当前成交以防重复扣减。

use super::{
    common::{order_status_as_str, parse_spot_order_db_id},
    read_models::SpotTradeQueryRow,
};
use crate::{
    error::{AppError, AppResult},
    modules::spot::{
        NewOrder, NewSpotTrade, OrderSide, SpotOrder, SpotTrade,
        service::{
            SpotOrderReservation as CreateSpotOrderReservation, ensure_spot_asset_amount,
            spot_order_reservation, spot_quote_amount,
        },
    },
};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool, Transaction};

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct SpotPairAssetRow {
    pub(crate) base_asset_id: u64,
    pub(crate) quote_asset_id: u64,
    pub(crate) base_precision: i32,
    pub(crate) quote_precision: i32,
    price_precision: i32,
    qty_precision: i32,
}

impl SpotPairAssetRow {
    /// 数量必须为正并符合交易对步长和真实基础资产精度，不受价格是否存在影响。
    pub(super) fn ensure_quantity(&self, quantity: &BigDecimal) -> AppResult<()> {
        if !(0..=18).contains(&self.quote_precision)
            || self.qty_precision > self.base_precision
            || self.base_asset_id == self.quote_asset_id
        {
            return Err(AppError::Validation(
                "invalid spot pair asset precision configuration".to_owned(),
            ));
        }
        ensure_spot_asset_amount(quantity, self.qty_precision, "spot quantity")?;
        ensure_spot_asset_amount(quantity, self.base_precision, "spot base quantity")?;
        if quantity <= &BigDecimal::from(0) {
            return Err(AppError::Validation(
                "spot quantity must be positive".to_owned(),
            ));
        }
        Ok(())
    }

    /// 新成交必须符合当前交易对步长与真实资产精度，拒绝舍入价格或源数量。
    pub(crate) fn ensure_fill(&self, price: &BigDecimal, quantity: &BigDecimal) -> AppResult<()> {
        self.ensure_quantity(quantity)?;
        ensure_spot_asset_amount(price, self.price_precision, "spot price")?;
        if price <= &BigDecimal::from(0) {
            return Err(AppError::Validation(
                "spot price must be positive".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, sqlx::FromRow)]
struct SpotOrderReservationRow {
    reserved_asset_id: Option<u64>,
    reserved_amount: Option<BigDecimal>,
}

#[derive(Debug, Clone)]
pub(super) struct SpotOrderReservation {
    pub(super) asset_id: u64,
    pub(super) amount: BigDecimal,
}

/// 从 MySQL 持久化数据读取逐笔成交，保持现货既有归属过滤、可见性及排序条件。
/// 在成交重放路径按幂等键读取既有交易，命中后必须逐字段核对订单、价格与数量。
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

/// 在已锁定双方订单的手工成交重放事务中核对原始管理员、原因和精确请求键。
/// 缺少审计的历史/自动成交或身份、原因不一致均拒绝重放，不补写审计，也不改变资金。
pub(crate) async fn ensure_manual_spot_fill_audit_matches_in_tx(
    tx: &mut Transaction<'_, MySql>,
    trade_id: &str,
    admin_id: u64,
    reason: &str,
    idempotency_key: &str,
) -> AppResult<()> {
    let stored: Option<(u64, Option<String>, Option<serde_json::Value>)> = sqlx::query_as(
        r#"SELECT admin_id, reason, after_json
           FROM admin_audit_logs
           WHERE action = 'spot.fill' AND target_type = 'spot_trade' AND target_id = ?
           ORDER BY id LIMIT 1"#,
    )
    .bind(trade_id)
    .fetch_optional(&mut **tx)
    .await?;
    if !matches!(stored, Some((actor, Some(stored_reason), Some(snapshot)))
        if actor == admin_id && stored_reason == reason
            && snapshot.get("idempotency_key").and_then(serde_json::Value::as_str) == Some(idempotency_key))
    {
        return Err(AppError::Conflict(
            "spot fill audit does not match original actor, reason or request key".to_owned(),
        ));
    }
    Ok(())
}

/// 在调用方持有的成交事务中写入唯一成交记录，并回读数据库生成的 ID 与时间。
/// 买卖订单、价格、数量和幂等键必须已由应用层校验；本函数不开始或提交事务，也不修改钱包和订单状态。
/// 幂等键冲突原样返回数据库错误，由上层回滚并核对既有成交，禁止在当前事务内继续资金结算。
pub(crate) async fn insert_spot_trade(
    tx: &mut Transaction<'_, MySql>,
    buy_order: &SpotOrder,
    sell_order: &SpotOrder,
    price: &BigDecimal,
    quantity: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<SpotTrade> {
    pair_assets_in_tx(tx, &buy_order.pair_id)
        .await?
        .ensure_fill(price, quantity)?;
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

/// 在成交事务内保存订单累计成交、平均价和终态，钱包结算失败时一并回滚。
/// 数据库失败由调用方回滚；涉及资金时余额、流水与业务状态必须同事务且幂等重放不重复入账。
pub(crate) async fn save_spot_order_fill_state(
    tx: &mut Transaction<'_, MySql>,
    order: &SpotOrder,
) -> AppResult<()> {
    crate::numeric::ensure_amount_storage(&order.filled_quantity, "filled_quantity")?;
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

/// 从 MySQL 持久化数据读取数据库订单标识，保持现货既有归属过滤、可见性及排序条件。
/// 读取交易对数据库主键，缺失时停止成交事务且不触碰钱包。
pub(crate) async fn load_spot_pair_db_id(pool: &Pool<MySql>, pair_symbol: &str) -> AppResult<u64> {
    let (pair_db_id,): (u64,) = sqlx::query_as(
        r#"SELECT id
           FROM trading_pairs
           WHERE symbol = ?
           LIMIT 1"#,
    )
    .bind(pair_symbol)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(pair_db_id)
}

/// 处理现货订单预留资金的现货基础设施适配逻辑，保持存储或外部协议的既有边界。
/// 按订单方向与已成交量计算事务内剩余预留，负值或异常状态返回错误。
pub(super) async fn remaining_spot_order_reservation_in_tx(
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
        let assets = pair_assets_in_tx(tx, &order.pair_id).await?;
        let expected_asset = match order.side {
            OrderSide::Buy => assets.quote_asset_id,
            OrderSide::Sell => assets.base_asset_id,
        };
        if asset_id != expected_asset {
            return Err(AppError::Conflict(
                "spot reservation asset does not match order side".to_owned(),
            ));
        }
        let total_amount = ledger_freeze_reservation_in_tx(tx, order, asset_id)
            .await?
            .unwrap_or(total_amount);
        return remaining_tracked_reservation_in_tx(tx, order, asset_id, total_amount).await;
    }

    remaining_legacy_spot_reservation_in_tx(tx, order).await
}

/// 在成交写入后计算“排除当前成交”的订单剩余冻结额，供当前资金腿校验与买单价差释放使用。
/// 调用方必须已锁定订单并处于同一成交事务；函数再次以 `FOR UPDATE` 读取保留快照，兼容历史订单的账本反推路径。
/// 必须在当前成交资金腿写入前调用；成交占位没有冻结扣款流水，天然不计入已消耗金额。
pub(crate) async fn remaining_spot_fill_reservation_before_trade_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &SpotOrder,
    _current_trade_id: &str,
) -> AppResult<CreateSpotOrderReservation> {
    let reservation = remaining_spot_order_reservation_in_tx(tx, order).await?;
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
    let asset_id = match order.side {
        OrderSide::Buy => assets.quote_asset_id,
        OrderSide::Sell => assets.base_asset_id,
    };
    let total = ledger_freeze_reservation_in_tx(tx, order, asset_id)
        .await?
        .ok_or_else(|| {
            AppError::Conflict("legacy spot reservation has no stored freeze evidence".to_owned())
        })?;
    remaining_tracked_reservation_in_tx(tx, order, asset_id, total).await
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
             AND user_id = ?
             AND change_type = 'spot_freeze'
             AND balance_type = 'frozen'
             AND amount > 0"#,
    )
    .bind(&order.id)
    .bind(asset_id)
    .bind(
        order
            .user_id
            .parse::<u64>()
            .map_err(|_| AppError::Unauthorized)?,
    )
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
    crate::numeric::ensure_amount_storage(&total_amount, "stored spot reservation")?;
    if total_amount <= 0 {
        return Err(AppError::Conflict(
            "spot reservation has no positive stored freeze evidence".to_owned(),
        ));
    }
    // 当前成交此时仅有 trade 占位，尚未写资金腿，因此实际流水天然排除当前成交。
    let spent_amount = filled_spot_order_reservation_in_tx(tx, order, asset_id).await?;
    if order.filled_quantity > 0 && spent_amount <= 0 {
        return Err(AppError::Conflict(
            "partial spot fill has no stored debit evidence".to_owned(),
        ));
    }
    let released_amount = released_spot_order_reservation_in_tx(tx, order, asset_id).await?;
    let remaining_amount = total_amount - spent_amount - released_amount;
    if remaining_amount < 0 {
        return Err(AppError::Conflict(
            "spot reservation accounting is negative".to_owned(),
        ));
    }
    crate::numeric::ensure_amount_storage(&remaining_amount, "remaining spot reservation")?;
    Ok(SpotOrderReservation {
        asset_id,
        amount: remaining_amount,
    })
}

async fn released_spot_order_reservation_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &SpotOrder,
    asset_id: u64,
) -> AppResult<BigDecimal> {
    let (released_amount,): (Option<BigDecimal>,) = sqlx::query_as(
        r#"SELECT COALESCE(SUM(amount), 0)
           FROM wallet_ledger
           WHERE ref_type = 'spot_trade'
             AND change_type = 'spot_price_improvement_release'
             AND balance_type = 'frozen'
             AND amount < 0
             AND user_id = ?
             AND asset_id = ?
             AND ref_id LIKE ?"#,
    )
    .bind(
        order
            .user_id
            .parse::<u64>()
            .map_err(|_| AppError::Unauthorized)?,
    )
    .bind(asset_id)
    .bind(format!("{}:%", order.id))
    .fetch_one(&mut **tx)
    .await?;
    Ok(-released_amount.unwrap_or_else(|| BigDecimal::from(0)))
}

async fn filled_spot_order_reservation_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &SpotOrder,
    asset_id: u64,
) -> AppResult<BigDecimal> {
    let reference = match order.side {
        OrderSide::Buy => format!("{}:%", order.id),
        OrderSide::Sell => format!("%:{}", order.id),
    };
    let (spent, debit_count): (BigDecimal, i64) = sqlx::query_as(
        r#"SELECT COALESCE(-SUM(amount), 0), COUNT(*) FROM wallet_ledger
           WHERE user_id = ? AND asset_id = ? AND balance_type = 'frozen'
             AND change_type = 'spot_trade_settlement' AND ref_type = 'spot_trade'
             AND amount < 0 AND ref_id LIKE ?"#,
    )
    .bind(
        order
            .user_id
            .parse::<u64>()
            .map_err(|_| AppError::Unauthorized)?,
    )
    .bind(asset_id)
    .bind(&reference)
    .fetch_one(&mut **tx)
    .await?;
    if order.side == OrderSide::Sell {
        if spent != order.filled_quantity {
            return Err(AppError::Conflict(
                "spot sell debit evidence differs from filled quantity".to_owned(),
            ));
        }
    } else {
        let (received, credit_count): (BigDecimal, i64) = sqlx::query_as(
            r#"SELECT COALESCE(SUM(amount), 0), COUNT(*) FROM wallet_ledger
               WHERE user_id = ? AND asset_id <> ? AND balance_type = 'available'
                 AND change_type = 'spot_trade_settlement' AND ref_type = 'spot_trade'
                 AND amount > 0 AND ref_id LIKE ?"#,
        )
        .bind(
            order
                .user_id
                .parse::<u64>()
                .map_err(|_| AppError::Unauthorized)?,
        )
        .bind(asset_id)
        .bind(reference)
        .fetch_one(&mut **tx)
        .await?;
        if received != order.filled_quantity || debit_count != credit_count {
            return Err(AppError::Conflict(
                "spot buy debit evidence differs from filled quantity".to_owned(),
            ));
        }
    }
    Ok(spent)
}

/// 处理事务内交易对基础与计价资产的现货基础设施适配逻辑，保持存储或外部协议的既有边界。
/// 读取成交事务使用的基础与计价资产，交易对不存在时终止资金结算。
pub(crate) async fn pair_assets_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_symbol: &str,
) -> AppResult<SpotPairAssetRow> {
    let assets = sqlx::query_as::<_, SpotPairAssetRow>(
        r#"SELECT p.base_asset AS base_asset_id, p.quote_asset AS quote_asset_id,
                  b.precision_scale AS base_precision, q.precision_scale AS quote_precision,
                  p.price_precision, p.qty_precision
           FROM trading_pairs p
           INNER JOIN assets b ON b.id = p.base_asset
           INNER JOIN assets q ON q.id = p.quote_asset
           WHERE p.symbol = ?
           LIMIT 1 FOR SHARE"#,
    )
    .bind(pair_symbol)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(assets)
}

/// 处理现货订单预留资金的现货基础设施适配逻辑，保持存储或外部协议的既有边界。
/// 从锁定订单快照重建预留资产与金额，供撤单和成交校验资金守恒。
pub(crate) async fn spot_order_reservation_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: &NewOrder,
    reference_price: Option<&BigDecimal>,
) -> AppResult<CreateSpotOrderReservation> {
    let assets = pair_assets_in_tx(tx, &order.pair_id).await?;
    ensure_spot_asset_amount(&order.quantity, assets.base_precision, "quantity")?;
    let mut reservation = spot_order_reservation(
        order,
        reference_price,
        assets.base_asset_id,
        assets.quote_asset_id,
    )?;
    let price = order
        .price
        .as_ref()
        .or(reference_price)
        .ok_or_else(|| AppError::Validation("spot reservation price is required".to_owned()))?;
    assets.ensure_fill(price, &order.quantity)?;
    if let Some(trigger) = order.trigger_price.as_ref() {
        ensure_spot_asset_amount(trigger, assets.price_precision, "trigger_price")?;
    }
    let quote = spot_quote_amount(price, &order.quantity, assets.quote_precision)?;
    if order.side == OrderSide::Buy {
        reservation.amount = quote;
    }
    Ok(reservation)
}

/// 处理数据库订单标识的现货基础设施适配逻辑，保持存储或外部协议的既有边界。
/// 解析并读取现货交易对数据库主键，未找到时不创建成交或钱包流水。
pub(crate) async fn spot_pair_db_id_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_symbol: &str,
) -> AppResult<u64> {
    let (pair_db_id,): (u64,) = sqlx::query_as(
        r#"SELECT id
           FROM trading_pairs
           WHERE symbol = ?
           LIMIT 1"#,
    )
    .bind(pair_symbol)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(pair_db_id)
}
