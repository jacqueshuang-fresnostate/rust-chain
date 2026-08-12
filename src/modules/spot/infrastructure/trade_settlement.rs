//! 现货成交记录、订单成交状态与订单级冻结额核算。
//!
//! 本模块不开始或提交事务；应用层是成交事务 owner。调用方必须先按稳定顺序锁订单，
//! 再写入成交幂等占位，并在同一事务内完成钱包四条资金腿、价差释放和订单状态写回。
//! 冻结额核算兼容新订单快照、历史账本反推，并可排除当前成交以防重复扣减。

use super::{
    common::{map_spot_service_error, order_status_as_str, parse_spot_order_db_id},
    read_models::SpotTradeQueryRow,
    wallet_accounts::lock_wallet_row,
};
use crate::{
    error::{AppError, AppResult},
    modules::spot::{
        NewOrder, NewSpotTrade, OrderSide, SpotOrder, SpotTrade,
        service::{SpotOrderReservation as CreateSpotOrderReservation, spot_order_reservation},
        spot_remaining_reserved_amount,
    },
};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool, Transaction};

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct SpotPairAssetRow {
    pub(crate) base_asset_id: u64,
    pub(crate) quote_asset_id: u64,
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

/// 在成交写入后计算“排除当前成交”的订单剩余冻结额，供当前资金腿校验与买单价差释放使用。
/// 调用方必须已锁定订单并处于同一成交事务；函数再次以 `FOR UPDATE` 读取保留快照，兼容历史订单的账本反推路径。
/// 当前成交 ID 只从历史扣减计算中排除，避免刚插入的成交被重复计入已用额度；本函数不修改余额或账本。
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
