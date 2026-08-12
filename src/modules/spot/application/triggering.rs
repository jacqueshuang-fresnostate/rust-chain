//! 行情触发现货撮合应用用例：扫描可触发委托并在逐单事务内创建流动性对手单与结算。

use crate::{
    error::{AppError, AppResult},
    modules::spot::{
        NewOrder, OrderSide, OrderStatus, OrderType, SpotOrder, SpotTrade, apply_fill,
        infrastructure::{
            SpotLedgerMetadata, apply_spot_wallet_settlement_leg,
            ensure_spot_liquidity_inventory_in_tx, ensure_spot_liquidity_user_in_tx,
            ensure_wallet_account_in_tx, freeze_wallet_for_inserted_order_in_tx,
            insert_spot_liquidity_buy_order_in_tx, insert_spot_liquidity_sell_order_in_tx,
            insert_spot_order_in_tx, insert_spot_trade, load_spot_pair_db_id,
            lock_spot_fill_wallet_rows_in_order, lock_spot_order_by_db_id, pair_assets_in_tx,
            release_buy_order_surplus_reservation_after_fill,
            remaining_spot_fill_reservation_before_trade_in_tx, save_spot_order_fill_state,
            spot_order_reservation_in_tx, triggered_limit_buy_order_ids,
            triggered_limit_sell_order_ids, triggered_stop_limit_buy_order_ids,
            triggered_stop_limit_sell_order_ids,
        },
        presentation::{SpotOrderResponse, SpotTradeResponse},
        service::{
            SpotOrderReservation, ensure_market_price_within_reference,
            ensure_spot_fill_within_order_reservation, is_triggerable_limit_buy_order,
            is_triggerable_limit_sell_order, is_triggerable_stop_limit_buy_order,
            is_triggerable_stop_limit_sell_order, market_buy_reservation_price,
            normalize_idempotency_key, publish_spot_fill_private_events_if_needed,
        },
    },
};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool, Transaction};

use super::settlement::insert_spot_fill_commissions_in_tx;

/// 按服务端行情价扫描并执行一批可触发现货限价/止损限价订单；价格必须为正，交易对符号必须对应持久化委托。
/// 四类候选各最多读取 20 笔，每笔使用独立事务并复核触发条件；单笔失败仅回滚自身并记录告警，不阻塞同批后续订单。
/// 每笔新成交在事务内完成订单、流动性对手单、固定钱包锁序、冻结扣减、资金流水和佣金；本函数返回成交但不发布事件。
pub(crate) async fn execute_triggered_spot_limit_orders(
    pool: &Pool<MySql>,
    pair_symbol: &str,
    market_price: &BigDecimal,
) -> AppResult<Vec<(SpotOrder, SpotOrder, SpotTrade, &'static str)>> {
    if market_price <= &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "market price must be positive".to_owned(),
        ));
    }

    let mut fills = Vec::new();

    let order_ids = triggered_limit_buy_order_ids(pool, pair_symbol, market_price, 20).await?;
    for order_id in order_ids {
        let result = execute_triggered_limit_buy_order(pool, order_id, market_price).await;
        collect_triggered_spot_fill(&mut fills, result, "buy", order_id, pair_symbol);
    }

    let order_ids = triggered_limit_sell_order_ids(pool, pair_symbol, market_price, 20).await?;
    for order_id in order_ids {
        let result = execute_triggered_limit_sell_order(pool, order_id, market_price).await;
        collect_triggered_spot_fill(&mut fills, result, "sell", order_id, pair_symbol);
    }

    let order_ids = triggered_stop_limit_buy_order_ids(pool, pair_symbol, market_price, 20).await?;
    for order_id in order_ids {
        let result = execute_triggered_stop_limit_buy_order(pool, order_id, market_price).await;
        collect_triggered_spot_fill(&mut fills, result, "buy", order_id, pair_symbol);
    }

    let order_ids =
        triggered_stop_limit_sell_order_ids(pool, pair_symbol, market_price, 20).await?;
    for order_id in order_ids {
        let result = execute_triggered_stop_limit_sell_order(pool, order_id, market_price).await;
        collect_triggered_spot_fill(&mut fills, result, "sell", order_id, pair_symbol);
    }

    Ok(fills)
}

/// 单个触发订单失败只回滚并跳过自身，防止头部坏单阻塞同批其他订单撮合。
fn collect_triggered_spot_fill(
    fills: &mut Vec<(SpotOrder, SpotOrder, SpotTrade, &'static str)>,
    result: AppResult<Option<(SpotOrder, SpotOrder, SpotTrade)>>,
    side: &'static str,
    order_id: u64,
    pair_symbol: &str,
) {
    match result {
        Ok(Some((order, counterparty_order, trade))) => {
            fills.push((order, counterparty_order, trade, side));
        }
        Ok(None) => {}
        Err(error) => {
            tracing::warn!(order_id, pair_symbol, side, %error, "触发订单撮合失败，跳过该订单");
        }
    }
}

/// 供行情摄取链调用的价格驱动撮合入口；`market_price` 必须是服务端市场源的正数价格，不接受客户端价格作为权威值。
/// 每个触发订单独立持有事务并遵循订单后钱包的稳定锁序，按原预留额结算 frozen、资金流水与佣金；失败订单跳过且不影响已提交项。
/// 仅对成功提交的新成交逐笔发布私有事件并返回成交数量；重复行情不会再次触发已成交订单，事件失败不会回滚已提交结算。
pub async fn execute_triggered_spot_limit_orders_with_hub(
    pool: &Pool<MySql>,
    pair_symbol: &str,
    market_price: &BigDecimal,
    hub: Option<&crate::modules::events::EventBroadcastHub>,
) -> AppResult<u32> {
    let fills = execute_triggered_spot_limit_orders(pool, pair_symbol, market_price).await?;
    let mut filled_count = 0_u32;

    for (order, counterparty_order, trade, side) in fills {
        filled_count += 1;
        if let Some(hub) = hub {
            let (buy_order, sell_order) = match side {
                "sell" => (counterparty_order, order),
                _ => (order, counterparty_order),
            };
            publish_spot_fill_private_events_if_needed(
                Some(hub),
                &crate::modules::spot::presentation::SpotFillResponse {
                    buy_order: SpotOrderResponse::from(buy_order),
                    sell_order: SpotOrderResponse::from(sell_order),
                    trade: SpotTradeResponse::from(trade),
                },
                true,
            )?;
        }
    }

    Ok(filled_count)
}
/// 插入并立即执行已触发的买单；调用前执行价必须来自服务端 Redis 行情，订单须为买向且已通过领域和风控校验。
/// 市价买单以 `max(reference_price, execution_price)` 预留报价资产，限价/止损限价按委托价约束；同一事务内依次插单、冻结、创建对手单并结算。
/// 幂等命中返回原订单且不重复冻结/成交；滑点、触发条件、余额或结算失败整体回滚，本函数不发布事件，提交后由创建用例广播。
pub(crate) async fn insert_triggered_buy_order_freeze_and_execute(
    pool: &Pool<MySql>,
    new_order: NewOrder,
    idempotency_key: Option<&str>,
    request_price: Option<&BigDecimal>,
    request_reference_price: Option<&BigDecimal>,
    execution_price: &BigDecimal,
) -> AppResult<(SpotOrder, bool, Option<(SpotOrder, SpotTrade)>)> {
    if let Some(request_reference_price) = request_reference_price {
        ensure_market_price_within_reference(
            OrderSide::Buy,
            execution_price,
            request_reference_price,
        )?;
    } else {
        ensure_limit_buy_price_reached(&new_order, execution_price)?;
    }
    let pair_db_id = load_spot_pair_db_id(pool, &new_order.pair_id).await?;
    let mut tx = pool.begin().await?;
    let reservation_reference_price =
        market_buy_reservation_price(request_reference_price, execution_price);
    let reservation =
        spot_order_reservation_in_tx(&mut tx, &new_order, reservation_reference_price).await?;
    let (order, is_new_order) = insert_spot_order_in_tx(
        &mut tx,
        new_order,
        pair_db_id,
        normalize_idempotency_key(idempotency_key),
        request_price,
        request_reference_price,
        &reservation,
    )
    .await?;
    if !is_new_order {
        tx.commit().await?;
        return Ok((order, false, None));
    }
    // 即时触发成交会同时冻结用户订单、生成系统对手单并结算，必须共享事务。
    freeze_wallet_for_inserted_order_in_tx(&mut tx, &order, &reservation).await?;
    let (order, counterparty_order, trade) =
        execute_triggered_buy_order_in_tx(&mut tx, order, execution_price).await?;
    tx.commit().await?;
    Ok((order, true, Some((counterparty_order, trade))))
}

/// 插入并立即执行已触发的卖单；调用前执行价必须来自服务端 Redis 行情，订单须为卖向且已通过领域和风控校验。
/// 卖单预留额恒为剩余基础资产数量；同一事务内插单、锁钱包并冻结、创建流动性买单，再按稳定钱包锁序完成双边结算与流水。
/// 幂等命中不重复冻结/成交；滑点、触发价/限价、库存或结算失败整体回滚，本函数不发布事件，提交成功后由创建用例广播。
pub(crate) async fn insert_triggered_sell_order_freeze_and_execute(
    pool: &Pool<MySql>,
    new_order: NewOrder,
    idempotency_key: Option<&str>,
    request_price: Option<&BigDecimal>,
    request_reference_price: Option<&BigDecimal>,
    execution_price: &BigDecimal,
) -> AppResult<(SpotOrder, bool, Option<(SpotOrder, SpotTrade)>)> {
    if let Some(request_reference_price) = request_reference_price {
        ensure_market_price_within_reference(
            OrderSide::Sell,
            execution_price,
            request_reference_price,
        )?;
    } else {
        ensure_limit_sell_price_reached(&new_order, execution_price)?;
    }
    let pair_db_id = load_spot_pair_db_id(pool, &new_order.pair_id).await?;
    let mut tx = pool.begin().await?;
    let reservation =
        spot_order_reservation_in_tx(&mut tx, &new_order, request_reference_price).await?;
    let (order, is_new_order) = insert_spot_order_in_tx(
        &mut tx,
        new_order,
        pair_db_id,
        normalize_idempotency_key(idempotency_key),
        request_price,
        request_reference_price,
        &reservation,
    )
    .await?;
    if !is_new_order {
        tx.commit().await?;
        return Ok((order, false, None));
    }
    // 卖单即时触发同样需要订单冻结、系统买单和成交结算原子提交。
    freeze_wallet_for_inserted_order_in_tx(&mut tx, &order, &reservation).await?;
    let (order, counterparty_order, trade) =
        execute_triggered_sell_order_in_tx(&mut tx, order, execution_price).await?;
    tx.commit().await?;
    Ok((order, true, Some((counterparty_order, trade))))
}

/// 在独立事务中尝试执行一笔行情触发的限价买单；先锁订单并在锁内复核状态、方向和 `market_price <= limit_price`。
/// 条件不再满足时提交只读事务并返回 `None`；满足时创建流动性卖单，按稳定钱包锁序结算原报价预留、资金流水和佣金。
/// 已成交/撤销订单构成幂等跳过，任一步失败回滚该订单全部副作用；本函数不发布事件。
pub(crate) async fn execute_triggered_limit_buy_order(
    pool: &Pool<MySql>,
    order_id: u64,
    market_price: &BigDecimal,
) -> AppResult<Option<(SpotOrder, SpotOrder, SpotTrade)>> {
    let mut tx = pool.begin().await?;
    let order = lock_spot_order_by_db_id(&mut tx, order_id).await?;
    if !is_triggerable_limit_buy_order(&order, market_price) {
        tx.commit().await?;
        return Ok(None);
    }
    let result = execute_triggered_buy_order_in_tx(&mut tx, order, market_price).await?;
    tx.commit().await?;
    Ok(Some(result))
}

/// 在独立事务中尝试执行一笔行情触发的限价卖单；先锁订单并在锁内复核状态、方向和 `market_price >= limit_price`。
/// 条件不再满足时无资金变更并返回 `None`；满足时创建流动性买单，按稳定钱包锁序扣减基础资产 frozen、贷记报价资产并写流水/佣金。
/// 已成交/撤销订单幂等跳过，失败整体回滚且不发布事件，提交后的广播由上层批处理入口承担。
pub(crate) async fn execute_triggered_limit_sell_order(
    pool: &Pool<MySql>,
    order_id: u64,
    market_price: &BigDecimal,
) -> AppResult<Option<(SpotOrder, SpotOrder, SpotTrade)>> {
    let mut tx = pool.begin().await?;
    let order = lock_spot_order_by_db_id(&mut tx, order_id).await?;
    if !is_triggerable_limit_sell_order(&order, market_price) {
        tx.commit().await?;
        return Ok(None);
    }
    let result = execute_triggered_sell_order_in_tx(&mut tx, order, market_price).await?;
    tx.commit().await?;
    Ok(Some(result))
}

/// 在独立事务中尝试执行止损限价买单；锁定订单后必须同时满足 `market_price <= trigger_price` 与 `market_price <= limit_price`。
/// 条件不满足或状态已变化时返回 `None` 且不触碰钱包；满足时复用买向流动性对手单和稳定钱包锁序，以既有报价预留完成结算。
/// 重复行情对已处理订单幂等，库存/余额/结算失败回滚订单、钱包、流水和佣金；事件由事务提交后的上层入口发布。
pub(crate) async fn execute_triggered_stop_limit_buy_order(
    pool: &Pool<MySql>,
    order_id: u64,
    market_price: &BigDecimal,
) -> AppResult<Option<(SpotOrder, SpotOrder, SpotTrade)>> {
    let mut tx = pool.begin().await?;
    let order = lock_spot_order_by_db_id(&mut tx, order_id).await?;
    if !is_triggerable_stop_limit_buy_order(&order, market_price) {
        tx.commit().await?;
        return Ok(None);
    }
    let result = execute_triggered_buy_order_in_tx(&mut tx, order, market_price).await?;
    tx.commit().await?;
    Ok(Some(result))
}

/// 在独立事务中尝试执行止损限价卖单；锁定订单后必须同时满足 `market_price >= trigger_price` 与 `market_price >= limit_price`。
/// 条件不满足或订单不可触发时返回 `None` 且不解冻/结算；满足时按基础资产预留和稳定钱包锁序完成流动性买单及双边资金流水。
/// 重复行情幂等跳过已处理订单，任一失败整体回滚且不发布事件，事件仅由上层在提交成功后广播。
pub(crate) async fn execute_triggered_stop_limit_sell_order(
    pool: &Pool<MySql>,
    order_id: u64,
    market_price: &BigDecimal,
) -> AppResult<Option<(SpotOrder, SpotOrder, SpotTrade)>> {
    let mut tx = pool.begin().await?;
    let order = lock_spot_order_by_db_id(&mut tx, order_id).await?;
    if !is_triggerable_stop_limit_sell_order(&order, market_price) {
        tx.commit().await?;
        return Ok(None);
    }
    let result = execute_triggered_sell_order_in_tx(&mut tx, order, market_price).await?;
    tx.commit().await?;
    Ok(Some(result))
}

async fn execute_triggered_buy_order_in_tx(
    tx: &mut Transaction<'_, MySql>,
    mut buy_order: SpotOrder,
    execution_price: &BigDecimal,
) -> AppResult<(SpotOrder, SpotOrder, SpotTrade)> {
    if buy_order.side != OrderSide::Buy {
        return Err(AppError::Internal(
            "triggered spot execution requires a buy order".to_owned(),
        ));
    }
    if let Some(limit_price) = buy_order.price.as_ref()
        && execution_price > limit_price
    {
        return Err(AppError::Validation(
            "market price is above buy limit".to_owned(),
        ));
    }
    let fill_quantity = buy_order.quantity.clone() - buy_order.filled_quantity.clone();
    if fill_quantity <= 0 {
        return Err(AppError::Validation(
            "spot buy order has no remaining quantity".to_owned(),
        ));
    }
    let assets = pair_assets_in_tx(tx, &buy_order.pair_id).await?;
    let buyer_id = buy_order
        .user_id
        .parse::<u64>()
        .map_err(|_| AppError::Unauthorized)?;
    ensure_wallet_account_in_tx(tx, buyer_id, assets.base_asset_id).await?;
    let liquidity_user_id = ensure_spot_liquidity_user_in_tx(tx).await?;
    ensure_spot_liquidity_inventory_in_tx(
        tx,
        liquidity_user_id,
        assets.base_asset_id,
        &fill_quantity,
    )
    .await?;
    ensure_wallet_account_in_tx(tx, liquidity_user_id, assets.quote_asset_id).await?;

    let mut sell_order = insert_spot_liquidity_sell_order_in_tx(
        tx,
        liquidity_user_id,
        &buy_order,
        execution_price,
        &fill_quantity,
    )
    .await?;
    let sell_reservation = SpotOrderReservation {
        asset_id: assets.base_asset_id,
        amount: fill_quantity.clone(),
    };
    freeze_wallet_for_inserted_order_in_tx(tx, &sell_order, &sell_reservation).await?;

    buy_order.status = OrderStatus::Open;
    sell_order.status = OrderStatus::Open;
    let trade_idempotency_key = format!("spot_triggered_buy:{}", buy_order.id);
    let trade = insert_spot_trade(
        tx,
        &buy_order,
        &sell_order,
        execution_price,
        &fill_quantity,
        &trade_idempotency_key,
    )
    .await?;
    lock_spot_fill_wallet_rows_in_order(
        tx,
        buyer_id,
        liquidity_user_id,
        assets.base_asset_id,
        assets.quote_asset_id,
    )
    .await?;

    let fill_quote_amount = execution_price.clone() * fill_quantity.clone();
    let buy_order_remaining_reservation =
        remaining_spot_fill_reservation_before_trade_in_tx(tx, &buy_order, &trade.id).await?;
    ensure_spot_fill_within_order_reservation(
        &buy_order_remaining_reservation,
        &fill_quote_amount,
        OrderSide::Buy,
    )?;
    let sell_order_remaining_reservation =
        remaining_spot_fill_reservation_before_trade_in_tx(tx, &sell_order, &trade.id).await?;
    ensure_spot_fill_within_order_reservation(
        &sell_order_remaining_reservation,
        &trade.quantity,
        OrderSide::Sell,
    )?;

    apply_fill(&mut buy_order, trade.quantity.clone())
        .map_err(|error| AppError::Validation(format!("invalid spot buy fill: {error:?}")))?;
    apply_fill(&mut sell_order, trade.quantity.clone())
        .map_err(|error| AppError::Validation(format!("invalid spot sell fill: {error:?}")))?;
    let ref_id = format!("{}:{}", buy_order.id, sell_order.id);
    let ledger = SpotLedgerMetadata {
        change_type: "spot_trade_settlement",
        ref_type: "spot_trade",
        ref_id: &ref_id,
    };
    apply_spot_wallet_settlement_leg(
        tx,
        buyer_id,
        assets.quote_asset_id,
        &fill_quote_amount,
        false,
        ledger,
    )
    .await?;
    apply_spot_wallet_settlement_leg(
        tx,
        buyer_id,
        assets.base_asset_id,
        &trade.quantity,
        true,
        ledger,
    )
    .await?;
    apply_spot_wallet_settlement_leg(
        tx,
        liquidity_user_id,
        assets.base_asset_id,
        &trade.quantity,
        false,
        ledger,
    )
    .await?;
    apply_spot_wallet_settlement_leg(
        tx,
        liquidity_user_id,
        assets.quote_asset_id,
        &fill_quote_amount,
        true,
        ledger,
    )
    .await?;
    release_buy_order_surplus_reservation_after_fill(
        tx,
        buyer_id,
        &buy_order,
        &buy_order_remaining_reservation,
        &fill_quote_amount,
        &ref_id,
    )
    .await?;
    insert_spot_fill_commissions_in_tx(
        tx,
        &trade,
        buyer_id,
        liquidity_user_id,
        assets.base_asset_id,
        assets.quote_asset_id,
        &fill_quote_amount,
    )
    .await?;
    save_spot_order_fill_state(tx, &buy_order).await?;
    save_spot_order_fill_state(tx, &sell_order).await?;
    Ok((buy_order, sell_order, trade))
}

async fn execute_triggered_sell_order_in_tx(
    tx: &mut Transaction<'_, MySql>,
    mut sell_order: SpotOrder,
    execution_price: &BigDecimal,
) -> AppResult<(SpotOrder, SpotOrder, SpotTrade)> {
    if sell_order.side != OrderSide::Sell {
        return Err(AppError::Internal(
            "triggered spot execution requires a sell order".to_owned(),
        ));
    }
    if let Some(limit_price) = sell_order.price.as_ref()
        && execution_price < limit_price
    {
        return Err(AppError::Validation(
            "market price is below sell limit".to_owned(),
        ));
    }
    let fill_quantity = sell_order.quantity.clone() - sell_order.filled_quantity.clone();
    if fill_quantity <= 0 {
        return Err(AppError::Validation(
            "spot sell order has no remaining quantity".to_owned(),
        ));
    }
    let assets = pair_assets_in_tx(tx, &sell_order.pair_id).await?;
    let seller_id = sell_order
        .user_id
        .parse::<u64>()
        .map_err(|_| AppError::Unauthorized)?;
    ensure_wallet_account_in_tx(tx, seller_id, assets.quote_asset_id).await?;
    let liquidity_user_id = ensure_spot_liquidity_user_in_tx(tx).await?;
    let fill_quote_amount = execution_price.clone() * fill_quantity.clone();
    ensure_spot_liquidity_inventory_in_tx(
        tx,
        liquidity_user_id,
        assets.quote_asset_id,
        &fill_quote_amount,
    )
    .await?;
    ensure_wallet_account_in_tx(tx, liquidity_user_id, assets.base_asset_id).await?;

    let mut buy_order = insert_spot_liquidity_buy_order_in_tx(
        tx,
        liquidity_user_id,
        &sell_order,
        execution_price,
        &fill_quantity,
    )
    .await?;
    let buy_reservation = SpotOrderReservation {
        asset_id: assets.quote_asset_id,
        amount: fill_quote_amount.clone(),
    };
    freeze_wallet_for_inserted_order_in_tx(tx, &buy_order, &buy_reservation).await?;

    buy_order.status = OrderStatus::Open;
    sell_order.status = OrderStatus::Open;
    let trade_idempotency_key = format!("spot_triggered_sell:{}", sell_order.id);
    let trade = insert_spot_trade(
        tx,
        &buy_order,
        &sell_order,
        execution_price,
        &fill_quantity,
        &trade_idempotency_key,
    )
    .await?;
    lock_spot_fill_wallet_rows_in_order(
        tx,
        liquidity_user_id,
        seller_id,
        assets.base_asset_id,
        assets.quote_asset_id,
    )
    .await?;

    let buy_order_remaining_reservation =
        remaining_spot_fill_reservation_before_trade_in_tx(tx, &buy_order, &trade.id).await?;
    ensure_spot_fill_within_order_reservation(
        &buy_order_remaining_reservation,
        &fill_quote_amount,
        OrderSide::Buy,
    )?;
    let sell_order_remaining_reservation =
        remaining_spot_fill_reservation_before_trade_in_tx(tx, &sell_order, &trade.id).await?;
    ensure_spot_fill_within_order_reservation(
        &sell_order_remaining_reservation,
        &trade.quantity,
        OrderSide::Sell,
    )?;

    apply_fill(&mut buy_order, trade.quantity.clone())
        .map_err(|error| AppError::Validation(format!("invalid spot buy fill: {error:?}")))?;
    apply_fill(&mut sell_order, trade.quantity.clone())
        .map_err(|error| AppError::Validation(format!("invalid spot sell fill: {error:?}")))?;
    let ref_id = format!("{}:{}", buy_order.id, sell_order.id);
    let ledger = SpotLedgerMetadata {
        change_type: "spot_trade_settlement",
        ref_type: "spot_trade",
        ref_id: &ref_id,
    };
    apply_spot_wallet_settlement_leg(
        tx,
        liquidity_user_id,
        assets.quote_asset_id,
        &fill_quote_amount,
        false,
        ledger,
    )
    .await?;
    apply_spot_wallet_settlement_leg(
        tx,
        liquidity_user_id,
        assets.base_asset_id,
        &trade.quantity,
        true,
        ledger,
    )
    .await?;
    apply_spot_wallet_settlement_leg(
        tx,
        seller_id,
        assets.base_asset_id,
        &trade.quantity,
        false,
        ledger,
    )
    .await?;
    apply_spot_wallet_settlement_leg(
        tx,
        seller_id,
        assets.quote_asset_id,
        &fill_quote_amount,
        true,
        ledger,
    )
    .await?;
    release_buy_order_surplus_reservation_after_fill(
        tx,
        liquidity_user_id,
        &buy_order,
        &buy_order_remaining_reservation,
        &fill_quote_amount,
        &ref_id,
    )
    .await?;
    insert_spot_fill_commissions_in_tx(
        tx,
        &trade,
        liquidity_user_id,
        seller_id,
        assets.base_asset_id,
        assets.quote_asset_id,
        &fill_quote_amount,
    )
    .await?;
    save_spot_order_fill_state(tx, &buy_order).await?;
    save_spot_order_fill_state(tx, &sell_order).await?;
    Ok((sell_order, buy_order, trade))
}
fn ensure_limit_buy_price_reached(order: &NewOrder, execution_price: &BigDecimal) -> AppResult<()> {
    if order.side != OrderSide::Buy
        || !matches!(order.order_type, OrderType::Limit | OrderType::StopLimit)
    {
        return Err(AppError::Internal(
            "price trigger requires a buy limit order".to_owned(),
        ));
    }
    let limit_price = order
        .price
        .as_ref()
        .ok_or_else(|| AppError::Validation("price is required for limit orders".to_owned()))?;
    if execution_price > limit_price {
        return Err(AppError::Validation(
            "market price is above buy limit".to_owned(),
        ));
    }
    if order.order_type == OrderType::StopLimit {
        let trigger_price = order.trigger_price.as_ref().ok_or_else(|| {
            AppError::Validation("trigger_price is required for stop limit orders".to_owned())
        })?;
        if execution_price > trigger_price {
            return Err(AppError::Validation(
                "market price is above buy trigger".to_owned(),
            ));
        }
    }
    Ok(())
}

fn ensure_limit_sell_price_reached(
    order: &NewOrder,
    execution_price: &BigDecimal,
) -> AppResult<()> {
    if order.side != OrderSide::Sell
        || !matches!(order.order_type, OrderType::Limit | OrderType::StopLimit)
    {
        return Err(AppError::Internal(
            "price trigger requires a sell limit order".to_owned(),
        ));
    }
    let limit_price = order
        .price
        .as_ref()
        .ok_or_else(|| AppError::Validation("price is required for limit orders".to_owned()))?;
    if execution_price < limit_price {
        return Err(AppError::Validation(
            "market price is below sell limit".to_owned(),
        ));
    }
    if order.order_type == OrderType::StopLimit {
        let trigger_price = order.trigger_price.as_ref().ok_or_else(|| {
            AppError::Validation("trigger_price is required for stop limit orders".to_owned())
        })?;
        if execution_price < trigger_price {
            return Err(AppError::Validation(
                "market price is below sell trigger".to_owned(),
            ));
        }
    }
    Ok(())
}
