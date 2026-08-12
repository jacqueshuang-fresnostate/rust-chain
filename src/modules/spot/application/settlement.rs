//! 现货成交结算应用用例：手工填单的事务、锁序、资金腿、佣金、幂等与提交后事件。

use crate::{
    error::{AppError, AppResult},
    modules::{
        agent::{
            infrastructure::insert_agent_business_commission_in_tx,
            repository::AgentBusinessCommissionWrite, service::AGENT_COMMISSION_PRODUCT_SPOT,
        },
        spot::{
            OrderSide, SpotOrder, SpotTrade, apply_fill,
            infrastructure::{
                SpotLedgerMetadata, apply_spot_wallet_settlement_leg, insert_spot_trade,
                is_duplicate_key_error, load_existing_spot_trade_by_idempotency_key,
                lock_spot_fill_orders_in_order, lock_spot_fill_wallet_rows_in_order,
                pair_assets_in_tx, release_buy_order_surplus_reservation_after_fill,
                remaining_spot_fill_reservation_before_trade_in_tx, save_spot_order_fill_state,
            },
            presentation::{FillSpotOrdersRequest, SpotFillResponse},
            service::{
                ensure_existing_spot_trade_matches_request, ensure_fill_orders_match,
                ensure_fill_price_matches_limits, ensure_spot_fill_within_order_reservation,
                publish_spot_fill_private_events_if_needed,
            },
        },
    },
};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool, Transaction};

/// 在单个 MySQL 事务中结算一笔现货成交，调用方必须传入同交易对、方向相反且满足限价约束的买卖订单。
/// 事务先按固定顺序锁订单，再以成交幂等键占位，随后按用户/资产稳定顺序锁钱包，避免双向撮合形成死锁。
/// 买方报价资产与卖方基础资产只能从 frozen 扣减，对手资产等额进入 available；每条资金腿、佣金及订单状态均同步写账本。
/// 同一幂等键重放只接受完全一致的订单、价格和数量并返回既有成交；唯一键竞态会回滚后走只读重放，不重复结算。
/// 本函数不发布外部事件；调用者只能在事务提交成功且 `is_new_trade` 为真时广播成交结果。
pub(crate) async fn settle_spot_fill(
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
    if let Some(trade) =
        load_existing_spot_trade_by_idempotency_key(&mut tx, idempotency_key).await?
    {
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
    let assets = pair_assets_in_tx(&mut tx, &buy_order.pair_id).await?;
    let base_asset_id = assets.base_asset_id;
    let quote_asset_id = assets.quote_asset_id;
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

    insert_spot_fill_commissions_in_tx(
        &mut tx,
        &trade,
        buyer_id,
        seller_id,
        base_asset_id,
        quote_asset_id,
        &fill_quote_amount,
    )
    .await?;

    save_spot_order_fill_state(&mut tx, &buy_order).await?;
    save_spot_order_fill_state(&mut tx, &sell_order).await?;
    tx.commit().await?;
    Ok((buy_order, sell_order, trade, true))
}

/// 结算指定买卖订单并在事务提交后发布私有成交事件；调用方必须提供正数价格/数量及稳定幂等键，并确保订单方向与交易对匹配。
/// 具体事务遵循“稳定订单锁→成交幂等占位→稳定钱包锁”顺序，以预留额约束 frozen 扣减，四条资金腿、佣金、流水和订单状态原子提交。
/// 相同幂等键仅在参数完全一致时返回既有成交，且不会重复结算或发事件；结算失败无事件，事件仅对首次成交在提交后发布。
pub(crate) async fn fill_spot_orders_with_events(
    pool: &Pool<MySql>,
    buy_order_id: &str,
    sell_order_id: &str,
    price: &BigDecimal,
    quantity: &BigDecimal,
    idempotency_key: &str,
    hub: Option<&crate::modules::events::EventBroadcastHub>,
) -> AppResult<SpotFillResponse> {
    // 成交处理后的事件发布收口到应用层，路由只返回统一的填单响应。
    let (buy_order, sell_order, trade, is_new_trade) = settle_spot_fill(
        pool,
        buy_order_id,
        sell_order_id,
        price,
        quantity,
        idempotency_key,
    )
    .await?;
    let response = SpotFillResponse {
        buy_order: buy_order.into(),
        sell_order: sell_order.into(),
        trade: trade.into(),
    };

    publish_spot_fill_private_events_if_needed(hub, &response, is_new_trade)?;
    Ok(response)
}

/// 接收管理员填单请求，先校验成交价、数量为正并标准化非空幂等键，再进入成交结算与提交后事件流程。
/// 调用方必须已完成管理员鉴权；事务、订单/钱包锁序、预留额、冻结扣减、流水及佣金合同由 [`settle_spot_fill`] 保证。
/// 参数失败时不启动事务；重放只返回一致成交且不重复资金或事件副作用，数据库失败则整笔回滚。
pub(crate) async fn fill_spot_orders_with_events_with_request(
    pool: &Pool<MySql>,
    request: FillSpotOrdersRequest,
    hub: Option<&crate::modules::events::EventBroadcastHub>,
) -> AppResult<SpotFillResponse> {
    // 成交请求的参数边界（幂等键与数量、价格正数）放在应用服务层，避免路由重复校验。
    let request = validate_fill_spot_order_request(request)?;

    fill_spot_orders_with_events(
        pool,
        &request.buy_order_id,
        &request.sell_order_id,
        &request.price,
        &request.quantity,
        &request.idempotency_key,
        hub,
    )
    .await
}

/// 校验并标准化手工成交请求：成交价和数量必须为正，幂等键去除首尾空白后仍须非空。
/// 该边界在开启结算事务前执行，不锁订单或钱包；失败时不占用幂等键、不改变冻结额/流水，也不发布事件。
pub(super) fn validate_fill_spot_order_request(
    mut request: FillSpotOrdersRequest,
) -> AppResult<FillSpotOrdersRequest> {
    validate_positive_amount(&request.price, "price")?;
    validate_positive_amount(&request.quantity, "quantity")?;
    request.idempotency_key = request.idempotency_key.trim().to_owned();
    if request.idempotency_key.is_empty() {
        return Err(AppError::Validation(
            "idempotency_key is required".to_owned(),
        ));
    }
    Ok(request)
}

fn validate_positive_amount(amount: &BigDecimal, field: &str) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        Err(AppError::Validation(format!("{field} must be positive")))
    } else {
        Ok(())
    }
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
    let trade = load_existing_spot_trade_by_idempotency_key(&mut tx, idempotency_key)
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
/// 在既有成交事务中为买卖双方写代理佣金：买方以报价成交额/报价资产为基数，卖方以成交数量/基础资产为基数。
/// 调用方必须已完成成交幂等占位和钱包结算锁序；本函数不自行提交或发布事件，来源类型与成交 ID 保证重放不产生重复佣金。
/// 任一佣金写入失败会向上传播并回滚同事务内的成交、订单、钱包及流水，禁止在事务外单独调用。
#[allow(clippy::too_many_arguments)]
pub(super) async fn insert_spot_fill_commissions_in_tx(
    tx: &mut Transaction<'_, MySql>,
    trade: &SpotTrade,
    buyer_id: u64,
    seller_id: u64,
    base_asset_id: u64,
    quote_asset_id: u64,
    fill_quote_amount: &BigDecimal,
) -> AppResult<()> {
    // 买卖双方的返佣基数和结算资产不同，来源类型分开后同代理撮合也不会撞幂等键。
    insert_agent_business_commission_in_tx(
        tx,
        AgentBusinessCommissionWrite {
            user_id: buyer_id,
            product_type: AGENT_COMMISSION_PRODUCT_SPOT,
            source_type: "spot_trade_buy",
            source_id: &trade.id,
            source_amount: fill_quote_amount,
            payout_asset_id: quote_asset_id,
        },
    )
    .await?;
    insert_agent_business_commission_in_tx(
        tx,
        AgentBusinessCommissionWrite {
            user_id: seller_id,
            product_type: AGENT_COMMISSION_PRODUCT_SPOT,
            source_type: "spot_trade_sell",
            source_id: &trade.id,
            source_amount: &trade.quantity,
            payout_asset_id: base_asset_id,
        },
    )
    .await
}
