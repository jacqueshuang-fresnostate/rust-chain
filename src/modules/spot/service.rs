//! spot bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务服务逐步迁入。

use crate::modules::spot::repository::SpotRepository;
use crate::{
    architecture::ServiceLayer,
    error::{AppError, AppResult},
    modules::events::{EventBroadcastHub, EventBroadcastMessage},
    modules::spot::{
        NewOrder, OrderSide, OrderStatus, OrderType, SpotOrder, SpotTrade,
        presentation::{SpotFillResponse, SpotOrderResponse, SpotTradeResponse},
        repository::SpotIdempotentOrderRecord,
        spot_remaining_reserved_amount, spot_reservation_amount, spot_reserve_asset_id,
    },
    modules::wallet::{WalletRepository, WalletService},
};
use bigdecimal::BigDecimal;
use serde_json::{Value, json};

const MARKET_REFERENCE_PRICE_TOLERANCE_BPS: i64 = 10;
const BASIS_POINTS_DENOMINATOR: i64 = 10_000;

#[derive(Debug)]
pub struct ServiceLayerMarker;

impl ServiceLayer for ServiceLayerMarker {}

/// 推送现货订单取消事件到用户私有频道（订单侧只做结构化通知）。
pub(crate) fn publish_spot_cancel_private_event_by_order(
    hub: &EventBroadcastHub,
    order: &SpotOrderResponse,
) -> AppResult<()> {
    let user_id = order
        .user_id
        .parse::<u64>()
        .map_err(|_| AppError::Unauthorized)?;
    publish_spot_cancel_private_event(hub, user_id, order);
    Ok(())
}

pub(crate) fn publish_spot_cancel_private_event(
    hub: &EventBroadcastHub,
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

/// 推送现货订单创建事件到用户私有频道。
pub(crate) fn publish_spot_created_private_event(
    hub: &EventBroadcastHub,
    user_id: u64,
    order: &SpotOrderResponse,
) {
    hub.publish(EventBroadcastMessage::private_user(
        user_id,
        json!({
            "type": "spot.order.created",
            "order_id": order.id,
            "pair_id": order.pair_id,
            "side": order.side,
            "order_type": order.order_type,
            "status": order.status,
        })
        .to_string(),
    ));
}

/// 在有新订单且已开启事件广播时推送创建与撮合事件。
pub(crate) fn publish_spot_created_private_events_if_needed(
    hub: Option<&EventBroadcastHub>,
    user_id: u64,
    response: &SpotOrderResponse,
    fill_event: Option<(&SpotOrderResponse, &SpotTradeResponse)>,
    is_new_order: bool,
) -> AppResult<()> {
    if !is_new_order {
        return Ok(());
    }
    let Some(hub) = hub else {
        return Ok(());
    };
    publish_spot_created_private_event(hub, user_id, response);
    if let Some((counterparty_order, trade)) = fill_event {
        match response.side {
            OrderSide::Buy => {
                publish_spot_fill_private_events(hub, response, counterparty_order, trade)?;
            }
            OrderSide::Sell => {
                publish_spot_fill_private_events(hub, counterparty_order, response, trade)?;
            }
        }
    }
    Ok(())
}

/// 在订单状态变化为已取消时推送取消事件。
pub(crate) fn publish_spot_cancel_private_event_if_needed(
    hub: Option<&EventBroadcastHub>,
    user_id: u64,
    order: &SpotOrderResponse,
    is_cancelled: bool,
) {
    if is_cancelled {
        if let Some(hub) = hub {
            publish_spot_cancel_private_event(hub, user_id, order);
        }
    }
}

/// 在订单侧已取消且广播可用时，以订单对象反查 user id 并推送取消事件。
pub(crate) fn publish_spot_cancel_private_event_by_order_if_needed(
    hub: Option<&EventBroadcastHub>,
    order: &SpotOrderResponse,
    is_cancelled: bool,
) -> AppResult<()> {
    if !is_cancelled {
        return Ok(());
    }
    if let Some(hub) = hub {
        return publish_spot_cancel_private_event_by_order(hub, order);
    }
    Ok(())
}

/// 推送单笔现货成交事件到成交双方的私有频道。
pub(crate) fn publish_spot_fill_private_events(
    hub: &EventBroadcastHub,
    buy_order: &SpotOrderResponse,
    sell_order: &SpotOrderResponse,
    trade: &SpotTradeResponse,
) -> AppResult<()> {
    publish_spot_fill_private_event(hub, buy_order, sell_order, trade, "buy")?;
    publish_spot_fill_private_event(hub, sell_order, buy_order, trade, "sell")?;
    Ok(())
}

/// 在有新成交且广播可用时推送成交事件。
pub(crate) fn publish_spot_fill_private_events_if_needed(
    hub: Option<&EventBroadcastHub>,
    response: &SpotFillResponse,
    is_new_trade: bool,
) -> AppResult<()> {
    if !is_new_trade {
        return Ok(());
    }
    let Some(hub) = hub else {
        return Ok(());
    };
    publish_spot_fill_private_events(
        hub,
        &response.buy_order,
        &response.sell_order,
        &response.trade,
    )
}

fn publish_spot_fill_private_event(
    hub: &EventBroadcastHub,
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

#[derive(Debug, Clone)]
pub struct CreateSpotOrderCommand {
    pub user_id: String,
    pub pair_id: String,
    pub base_asset_id: String,
    pub quote_asset_id: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub price: Option<BigDecimal>,
    pub trigger_price: Option<BigDecimal>,
    pub quantity: BigDecimal,
    pub reference_price: Option<BigDecimal>,
    pub idempotency_key: Option<String>,
    pub wallet_ledger: crate::modules::wallet::LedgerMetadata,
}

#[derive(Debug, Clone)]
pub struct CancelSpotOrderCommand {
    pub order_id: String,
    pub base_asset_id: String,
    pub quote_asset_id: String,
    pub wallet_ledger: crate::modules::wallet::LedgerMetadata,
}

#[derive(Debug, Clone)]
pub struct FillSpotOrderCommand {
    pub order_id: String,
    pub base_asset_id: String,
    pub quote_asset_id: String,
    pub fill_price: BigDecimal,
    pub fill_quantity: BigDecimal,
    pub wallet_ledger: crate::modules::wallet::LedgerMetadata,
}

#[derive(Debug, Clone)]
pub struct SpotService<S, W> {
    spot_repository: S,
    wallet_service: WalletService<W>,
}

impl<S, W> SpotService<S, W> {
    pub fn new(spot_repository: S, wallet_repository: W) -> Self {
        Self {
            spot_repository,
            wallet_service: WalletService::new(wallet_repository),
        }
    }

    pub fn into_repositories(self) -> (S, W) {
        (self.spot_repository, self.wallet_service.into_repository())
    }
}

impl<S: SpotRepository, W: WalletRepository> SpotService<S, W> {
    pub fn create_order(
        &mut self,
        command: CreateSpotOrderCommand,
    ) -> Result<crate::modules::spot::SpotOrder, crate::modules::spot::SpotServiceError> {
        let pair = self.spot_repository.load_pair_rule(&command.pair_id)?;

        let new_order = match command.order_type {
            OrderType::Limit => crate::modules::spot::create_limit_order(
                command.user_id.clone(),
                command.side,
                command.price.clone().ok_or(
                    crate::modules::spot::SpotServiceError::MissingPriceForWalletReservation,
                )?,
                command.quantity.clone(),
                &pair,
            )?,
            OrderType::Market => crate::modules::spot::create_market_order(
                command.user_id.clone(),
                command.side,
                command.quantity.clone(),
                command.reference_price.clone().ok_or(
                    crate::modules::spot::SpotServiceError::MissingReferencePriceForMarketOrder,
                )?,
                &pair,
            )?,
            OrderType::StopLimit => crate::modules::spot::create_stop_limit_order(
                command.user_id.clone(),
                command.side,
                command.trigger_price.clone().ok_or(
                    crate::modules::spot::SpotServiceError::MissingTriggerPriceForStopLimitOrder,
                )?,
                command.price.clone().ok_or(
                    crate::modules::spot::SpotServiceError::MissingPriceForWalletReservation,
                )?,
                command.quantity.clone(),
                &pair,
            )?,
        };

        let reservation_price =
            command
                .price
                .or(command.reference_price)
                .ok_or(match command.order_type {
                    OrderType::Limit => {
                        crate::modules::spot::SpotServiceError::MissingPriceForWalletReservation
                    }
                    OrderType::Market => {
                        crate::modules::spot::SpotServiceError::MissingReferencePriceForMarketOrder
                    }
                    OrderType::StopLimit => {
                        crate::modules::spot::SpotServiceError::MissingPriceForWalletReservation
                    }
                })?;
        let reserve_amount =
            spot_reservation_amount(command.side, &reservation_price, &command.quantity);
        let reserve_asset_id = spot_reserve_asset_id(
            command.side,
            &command.base_asset_id,
            &command.quote_asset_id,
        );
        self.wallet_service
            .freeze(crate::modules::wallet::FreezeBalanceCommand {
                user_id: command.user_id,
                asset_id: reserve_asset_id.to_owned(),
                amount: reserve_amount,
                ledger: command.wallet_ledger,
            })?;

        self.spot_repository
            .insert_order(new_order, command.idempotency_key.as_deref())
    }

    pub fn cancel_order(
        &mut self,
        command: CancelSpotOrderCommand,
    ) -> Result<bool, crate::modules::spot::SpotServiceError> {
        let mut order = self.spot_repository.load_order(&command.order_id)?;
        let was_cancelled = crate::modules::spot::cancel_order(&mut order)?;
        if !was_cancelled {
            return Ok(false);
        }

        let remaining_reservation = spot_remaining_reserved_amount(
            &order,
            &command.base_asset_id,
            &command.quote_asset_id,
        )?;
        if remaining_reservation.1 > BigDecimal::from(0) {
            self.wallet_service
                .unfreeze(crate::modules::wallet::UnfreezeBalanceCommand {
                    user_id: order.user_id.clone(),
                    asset_id: remaining_reservation.0,
                    amount: remaining_reservation.1,
                    ledger: command.wallet_ledger,
                })?;
        }

        self.spot_repository.save_order(order)?;
        Ok(true)
    }

    pub fn fill_order(
        &mut self,
        command: FillSpotOrderCommand,
    ) -> Result<crate::modules::spot::SpotOrder, crate::modules::spot::SpotServiceError> {
        let mut order = self.spot_repository.load_order(&command.order_id)?;
        crate::modules::spot::apply_fill(&mut order, command.fill_quantity.clone())?;

        match order.side {
            OrderSide::Buy => {
                self.wallet_service
                    .settle(crate::modules::wallet::SettleBalanceCommand {
                        user_id: order.user_id.clone(),
                        debit_frozen_asset_id: command.quote_asset_id,
                        debit_frozen_amount: command.fill_price * command.fill_quantity.clone(),
                        credit_available_asset_id: command.base_asset_id,
                        credit_available_amount: command.fill_quantity,
                        ledger: command.wallet_ledger,
                    })?;
            }
            OrderSide::Sell => {
                self.wallet_service
                    .settle(crate::modules::wallet::SettleBalanceCommand {
                        user_id: order.user_id.clone(),
                        debit_frozen_asset_id: command.base_asset_id,
                        debit_frozen_amount: command.fill_quantity.clone(),
                        credit_available_asset_id: command.quote_asset_id,
                        credit_available_amount: command.fill_price * command.fill_quantity,
                        ledger: command.wallet_ledger,
                    })?;
            }
        }

        self.spot_repository.save_order(order.clone())?;
        Ok(order)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SpotOrderIdempotencyCheck {
    pub(crate) pair_id: String,
    pub(crate) side: OrderSide,
    pub(crate) order_type: OrderType,
    pub(crate) price: Option<BigDecimal>,
    pub(crate) trigger_price: Option<BigDecimal>,
    pub(crate) quantity: BigDecimal,
    pub(crate) reserved_amount: Option<BigDecimal>,
    pub(crate) request_reference_price: Option<BigDecimal>,
    pub(crate) request_price: Option<BigDecimal>,
}

#[derive(Debug, Clone)]
pub(crate) struct SpotOrderReservation {
    pub(crate) asset_id: u64,
    pub(crate) amount: BigDecimal,
}

pub(crate) fn normalize_idempotency_key(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

pub(crate) fn spot_order_idempotency_check_for_insert(
    new_order: &NewOrder,
    request_price: Option<&BigDecimal>,
    reference_price: Option<&BigDecimal>,
    reserved_amount: &BigDecimal,
) -> SpotOrderIdempotencyCheck {
    SpotOrderIdempotencyCheck {
        pair_id: new_order.pair_id.clone(),
        side: new_order.side,
        order_type: new_order.order_type,
        price: new_order.price.clone(),
        trigger_price: new_order.trigger_price.clone(),
        quantity: new_order.quantity.clone(),
        reserved_amount: Some(reserved_amount.clone()),
        request_reference_price: match new_order.order_type {
            OrderType::Limit | OrderType::StopLimit => None,
            OrderType::Market => reference_price.cloned(),
        },
        request_price: request_price.cloned(),
    }
}

pub(crate) fn ensure_spot_order_idempotency_matches(
    existing: &SpotIdempotentOrderRecord,
    expected: &SpotOrderIdempotencyCheck,
) -> AppResult<()> {
    let matches = spot_pair_matches(existing, &expected.pair_id)
        && existing.side == expected.side
        && existing.order_type == expected.order_type
        && existing.price == expected.price
        && existing.trigger_price == expected.trigger_price
        && existing.quantity == expected.quantity
        && existing.reserved_amount == expected.reserved_amount
        && request_reference_price_matches(
            existing,
            expected.side,
            expected.order_type,
            expected.request_reference_price.as_ref(),
        )
        && request_price_matches(
            existing,
            expected.order_type,
            expected.request_price.as_ref(),
        );

    if matches {
        Ok(())
    } else {
        Err(AppError::Conflict(
            "spot order idempotency key was used with a different request".to_owned(),
        ))
    }
}

pub(crate) fn ensure_spot_order_idempotency_matches_insert(
    existing: &SpotIdempotentOrderRecord,
    new_order: &NewOrder,
    request_price: Option<&BigDecimal>,
    reference_price: Option<&BigDecimal>,
    reservation: &SpotOrderReservation,
) -> AppResult<()> {
    let expected = spot_order_idempotency_check_for_insert(
        new_order,
        request_price,
        reference_price,
        &reservation.amount,
    );
    ensure_spot_order_idempotency_matches(existing, &expected)
}

pub(crate) fn spot_fill_order_lock_keys(
    buy_order_id: &str,
    sell_order_id: &str,
) -> AppResult<Vec<u64>> {
    let mut keys = vec![
        parse_spot_order_request_id(buy_order_id)?,
        parse_spot_order_request_id(sell_order_id)?,
    ];
    // 成交会同时锁买卖订单，统一按主键升序加锁，避免相反方向请求互相等待。
    keys.sort_unstable();
    keys.dedup();
    Ok(keys)
}

/// 解析 JWT subject 中的管理员标识。
pub(crate) fn admin_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("admin:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 将底层 spot 服务错误统一转换为对外错误码与提示文案。
pub(crate) fn map_spot_error(error: crate::modules::spot::SpotServiceError) -> AppError {
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
        crate::modules::spot::SpotServiceError::MissingTriggerPriceForStopLimitOrder => {
            AppError::Validation("trigger_price is required for stop limit orders".to_owned())
        }
    }
}

pub(crate) fn spot_fill_wallet_lock_keys(
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
    // 钱包锁顺序必须稳定，避免买卖双方 base/quote 交叉锁导致死锁。
    keys.sort_unstable();
    keys.dedup();
    keys
}

pub(crate) fn parse_spot_order_request_id(order_id: &str) -> AppResult<u64> {
    order_id
        .parse::<u64>()
        .map_err(|_| AppError::Validation("invalid spot order id".to_owned()))
}

pub(crate) fn ensure_fill_orders_match(
    buy_order: &SpotOrder,
    sell_order: &SpotOrder,
) -> AppResult<()> {
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

pub(crate) fn ensure_fill_price_matches_limits(
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

pub(crate) fn ensure_existing_spot_trade_matches_request(
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

pub(crate) fn ensure_spot_fill_within_order_reservation(
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

pub(crate) fn cancel_spot_order_state(mut order: SpotOrder) -> AppResult<(SpotOrder, bool)> {
    let cancelled = crate::modules::spot::cancel_order(&mut order)
        .map_err(|error| AppError::Validation(format!("invalid spot cancel: {error:?}")))?;
    Ok((order, cancelled))
}

pub(crate) fn spot_order_audit_json(order: &SpotOrder) -> Value {
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

pub(crate) fn ensure_market_price_within_reference(
    side: OrderSide,
    execution_price: &BigDecimal,
    reference_price: &BigDecimal,
) -> AppResult<()> {
    match side {
        OrderSide::Buy => {
            let ceiling =
                reference_price.clone() + market_reference_price_tolerance(reference_price);
            if execution_price > &ceiling {
                Err(AppError::Validation(
                    "market price exceeds submitted reference price; please retry".to_owned(),
                ))
            } else {
                Ok(())
            }
        }
        OrderSide::Sell => {
            let floor = reference_price.clone() - market_reference_price_tolerance(reference_price);
            if execution_price < &floor {
                Err(AppError::Validation(
                    "market price is below submitted reference price; please retry".to_owned(),
                ))
            } else {
                Ok(())
            }
        }
    }
}

pub(crate) fn limit_order_reaches_execution_price(
    side: OrderSide,
    execution_price: &BigDecimal,
    limit_price: &BigDecimal,
) -> bool {
    match side {
        OrderSide::Buy => execution_price <= limit_price,
        OrderSide::Sell => execution_price >= limit_price,
    }
}

pub(crate) fn stop_limit_order_reaches_execution_price(
    side: OrderSide,
    execution_price: &BigDecimal,
    trigger_price: &BigDecimal,
    limit_price: &BigDecimal,
) -> bool {
    // 止限价单必须同时满足触发价和限价，避免只触发但以超出用户限价的价格成交。
    let trigger_reached = match side {
        OrderSide::Buy => execution_price <= trigger_price,
        OrderSide::Sell => execution_price >= trigger_price,
    };
    let limit_reached = limit_order_reaches_execution_price(side, execution_price, limit_price);
    trigger_reached && limit_reached
}

pub(crate) fn is_triggerable_limit_buy_order(order: &SpotOrder, market_price: &BigDecimal) -> bool {
    order.side == OrderSide::Buy
        && order.order_type == OrderType::Limit
        && is_triggerable_order_status(order.status)
        && order
            .price
            .as_ref()
            .is_some_and(|limit_price| market_price <= limit_price)
        && order.quantity > order.filled_quantity
}

pub(crate) fn is_triggerable_limit_sell_order(
    order: &SpotOrder,
    market_price: &BigDecimal,
) -> bool {
    order.side == OrderSide::Sell
        && order.order_type == OrderType::Limit
        && is_triggerable_order_status(order.status)
        && order
            .price
            .as_ref()
            .is_some_and(|limit_price| market_price >= limit_price)
        && order.quantity > order.filled_quantity
}

pub(crate) fn is_triggerable_stop_limit_buy_order(
    order: &SpotOrder,
    market_price: &BigDecimal,
) -> bool {
    order.side == OrderSide::Buy
        && order.order_type == OrderType::StopLimit
        && is_triggerable_order_status(order.status)
        && order
            .trigger_price
            .as_ref()
            .is_some_and(|trigger_price| market_price <= trigger_price)
        && order
            .price
            .as_ref()
            .is_some_and(|limit_price| market_price <= limit_price)
        && order.quantity > order.filled_quantity
}

pub(crate) fn is_triggerable_stop_limit_sell_order(
    order: &SpotOrder,
    market_price: &BigDecimal,
) -> bool {
    order.side == OrderSide::Sell
        && order.order_type == OrderType::StopLimit
        && is_triggerable_order_status(order.status)
        && order
            .trigger_price
            .as_ref()
            .is_some_and(|trigger_price| market_price >= trigger_price)
        && order
            .price
            .as_ref()
            .is_some_and(|limit_price| market_price >= limit_price)
        && order.quantity > order.filled_quantity
}

pub(crate) fn market_buy_reservation_price<'a>(
    request_reference_price: Option<&'a BigDecimal>,
    execution_price: &'a BigDecimal,
) -> Option<&'a BigDecimal> {
    // 市价买单即时成交时按 reference/execution 较高者冻结，避免成交金额超过已冻结报价资产。
    request_reference_price.map(|reference_price| {
        if execution_price > reference_price {
            execution_price
        } else {
            reference_price
        }
    })
}

pub(crate) fn spot_order_reservation(
    order: &NewOrder,
    reference_price: Option<&BigDecimal>,
    base_asset_id: u64,
    quote_asset_id: u64,
) -> AppResult<SpotOrderReservation> {
    let price = match order.order_type {
        OrderType::Limit | OrderType::StopLimit => order.price.as_ref().ok_or_else(|| {
            AppError::Validation("price is required for wallet reservation".to_owned())
        })?,
        OrderType::Market => reference_price.ok_or_else(|| {
            AppError::Validation("reference_price is required for market orders".to_owned())
        })?,
    };
    let amount = spot_reservation_amount(order.side, price, &order.quantity);
    let base_asset_id = base_asset_id.to_string();
    let quote_asset_id = quote_asset_id.to_string();
    let asset_id = spot_reserve_asset_id(order.side, &base_asset_id, &quote_asset_id)
        .parse::<u64>()
        .map_err(|_| AppError::Internal("invalid reserve asset id".to_owned()))?;
    Ok(SpotOrderReservation { asset_id, amount })
}

fn market_reference_price_tolerance(reference_price: &BigDecimal) -> BigDecimal {
    reference_price.clone() * BigDecimal::from(MARKET_REFERENCE_PRICE_TOLERANCE_BPS)
        / BigDecimal::from(BASIS_POINTS_DENOMINATOR)
}

fn is_triggerable_order_status(status: OrderStatus) -> bool {
    matches!(
        status,
        OrderStatus::Pending | OrderStatus::Open | OrderStatus::PartiallyFilled
    )
}

fn spot_pair_matches(existing: &SpotIdempotentOrderRecord, requested_pair_id: &str) -> bool {
    existing.pair_id.eq_ignore_ascii_case(requested_pair_id)
        || requested_pair_id.parse::<u64>().ok() == Some(existing.pair_db_id)
}

fn request_reference_price_matches(
    existing: &SpotIdempotentOrderRecord,
    side: OrderSide,
    order_type: OrderType,
    expected: Option<&BigDecimal>,
) -> bool {
    match existing.request_reference_price.as_ref() {
        Some(stored) => Some(stored) == expected,
        None => match order_type {
            OrderType::Limit | OrderType::StopLimit => expected.is_none(),
            OrderType::Market => side == OrderSide::Buy,
        },
    }
}

fn request_price_matches(
    existing: &SpotIdempotentOrderRecord,
    order_type: OrderType,
    expected: Option<&BigDecimal>,
) -> bool {
    match existing.request_price.as_ref() {
        Some(stored) => Some(stored) == expected,
        None => match order_type {
            OrderType::Limit | OrderType::StopLimit => existing.price.as_ref() == expected,
            OrderType::Market => expected.is_none(),
        },
    }
}
