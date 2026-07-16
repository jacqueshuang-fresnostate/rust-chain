//! spot bounded context domain layer.
//!
//! 领域层：放置领域实体、值对象、错误和纯业务规则。
//! 这部分代码不依赖数据库/网络/HTTP，便于被应用层直接复用和独立测试。

use crate::{architecture::DomainLayer, modules::wallet::WalletServiceError};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct DomainLayerMarker;

impl DomainLayer for DomainLayerMarker {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderType {
    Limit,
    Market,
    StopLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Pending,
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
}

#[derive(Debug, Clone)]
pub struct TradingPairRule {
    pub pair_id: String,
    pub price_precision: u32,
    pub quantity_precision: u32,
    pub min_order_value: BigDecimal,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewOrder {
    pub user_id: String,
    pub pair_id: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub price: Option<BigDecimal>,
    pub trigger_price: Option<BigDecimal>,
    pub quantity: BigDecimal,
    pub filled_quantity: BigDecimal,
    pub status: OrderStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotOrder {
    pub id: String,
    pub user_id: String,
    pub pair_id: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub price: Option<BigDecimal>,
    pub trigger_price: Option<BigDecimal>,
    pub quantity: BigDecimal,
    pub filled_quantity: BigDecimal,
    pub status: OrderStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSpotTrade {
    pub pair_id: String,
    pub buy_order_id: String,
    pub sell_order_id: String,
    pub price: BigDecimal,
    pub quantity: BigDecimal,
    pub fee: BigDecimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotTrade {
    pub id: String,
    pub pair_id: String,
    pub buy_order_id: String,
    pub sell_order_id: String,
    pub price: BigDecimal,
    pub quantity: BigDecimal,
    pub fee: BigDecimal,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpotDomainError {
    TradingPairDisabled,
    LimitOrderRequiresPrice,
    StopLimitOrderRequiresPrice,
    StopLimitOrderRequiresTriggerPrice,
    MarketOrderRejectsPrice,
    NonPositivePrice,
    NonPositiveQuantity,
    PricePrecisionExceeded {
        allowed: u32,
    },
    QuantityPrecisionExceeded {
        allowed: u32,
    },
    MinOrderValueNotMet {
        actual: BigDecimal,
        minimum: BigDecimal,
    },
    InvalidStatusTransition {
        from: OrderStatus,
        to: OrderStatus,
    },
    FillQuantityExceedsRemaining {
        remaining: BigDecimal,
        fill: BigDecimal,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpotServiceError {
    Domain(SpotDomainError),
    Wallet(WalletServiceError),
    MissingPriceForWalletReservation,
    MissingReferencePriceForMarketOrder,
    MissingTriggerPriceForStopLimitOrder,
    Repository(String),
}

impl From<SpotDomainError> for SpotServiceError {
    fn from(error: SpotDomainError) -> Self {
        Self::Domain(error)
    }
}

impl From<WalletServiceError> for SpotServiceError {
    fn from(error: WalletServiceError) -> Self {
        Self::Wallet(error)
    }
}

pub fn create_limit_order(
    user_id: impl Into<String>,
    side: OrderSide,
    price: BigDecimal,
    quantity: BigDecimal,
    pair: &TradingPairRule,
) -> Result<NewOrder, SpotDomainError> {
    validate_common(&quantity, pair)?;
    validate_price(&price, pair.price_precision)?;
    validate_min_order_value(price.clone() * quantity.clone(), pair)?;

    Ok(NewOrder {
        user_id: user_id.into(),
        pair_id: pair.pair_id.clone(),
        side,
        order_type: OrderType::Limit,
        price: Some(price),
        trigger_price: None,
        quantity,
        filled_quantity: BigDecimal::from(0),
        status: OrderStatus::Pending,
    })
}

pub fn create_market_order(
    user_id: impl Into<String>,
    side: OrderSide,
    quantity: BigDecimal,
    reference_price: BigDecimal,
    pair: &TradingPairRule,
) -> Result<NewOrder, SpotDomainError> {
    validate_common(&quantity, pair)?;
    validate_price(&reference_price, pair.price_precision)?;
    validate_min_order_value(reference_price * quantity.clone(), pair)?;

    Ok(NewOrder {
        user_id: user_id.into(),
        pair_id: pair.pair_id.clone(),
        side,
        order_type: OrderType::Market,
        price: None,
        trigger_price: None,
        quantity,
        filled_quantity: BigDecimal::from(0),
        status: OrderStatus::Pending,
    })
}

pub fn create_stop_limit_order(
    user_id: impl Into<String>,
    side: OrderSide,
    trigger_price: BigDecimal,
    price: BigDecimal,
    quantity: BigDecimal,
    pair: &TradingPairRule,
) -> Result<NewOrder, SpotDomainError> {
    validate_common(&quantity, pair)?;
    validate_price(&trigger_price, pair.price_precision)?;
    validate_price(&price, pair.price_precision)?;
    validate_min_order_value(price.clone() * quantity.clone(), pair)?;

    Ok(NewOrder {
        user_id: user_id.into(),
        pair_id: pair.pair_id.clone(),
        side,
        order_type: OrderType::StopLimit,
        price: Some(price),
        trigger_price: Some(trigger_price),
        quantity,
        filled_quantity: BigDecimal::from(0),
        status: OrderStatus::Pending,
    })
}

pub fn spot_reservation_amount(
    side: OrderSide,
    price: &BigDecimal,
    quantity: &BigDecimal,
) -> BigDecimal {
    reservation_amount(side, price, quantity)
}

pub fn spot_reserve_asset_id<'a>(
    side: OrderSide,
    base_asset_id: &'a str,
    quote_asset_id: &'a str,
) -> &'a str {
    reserve_asset_id(side, base_asset_id, quote_asset_id)
}

pub fn spot_remaining_reserved_amount(
    order: &SpotOrder,
    base_asset_id: &str,
    quote_asset_id: &str,
) -> Result<(String, BigDecimal), SpotServiceError> {
    remaining_reserved_amount(order, base_asset_id, quote_asset_id)
        .map(|reserved| (reserved.asset_id, reserved.amount))
}

pub fn validate_order_request(
    order_type: OrderType,
    price: Option<BigDecimal>,
    quantity: BigDecimal,
    pair: &TradingPairRule,
) -> Result<(), SpotDomainError> {
    match (order_type, price) {
        (OrderType::Limit, Some(price)) => {
            create_limit_order("validation", OrderSide::Buy, price, quantity, pair).map(|_| ())
        }
        (OrderType::Limit, None) => Err(SpotDomainError::LimitOrderRequiresPrice),
        (OrderType::Market, Some(_)) => Err(SpotDomainError::MarketOrderRejectsPrice),
        (OrderType::Market, None) => validate_common(&quantity, pair),
        (OrderType::StopLimit, Some(_)) => Err(SpotDomainError::StopLimitOrderRequiresTriggerPrice),
        (OrderType::StopLimit, None) => Err(SpotDomainError::StopLimitOrderRequiresPrice),
    }
}

pub fn transition_status(
    current: OrderStatus,
    next: OrderStatus,
) -> Result<OrderStatus, SpotDomainError> {
    if can_transition(current, next) {
        Ok(next)
    } else {
        Err(SpotDomainError::InvalidStatusTransition {
            from: current,
            to: next,
        })
    }
}

pub fn cancel_order(order: &mut SpotOrder) -> Result<bool, SpotDomainError> {
    match order.status {
        OrderStatus::Cancelled => Ok(false),
        OrderStatus::Pending | OrderStatus::Open | OrderStatus::PartiallyFilled => {
            order.status = OrderStatus::Cancelled;
            Ok(true)
        }
        status => Err(SpotDomainError::InvalidStatusTransition {
            from: status,
            to: OrderStatus::Cancelled,
        }),
    }
}

pub fn apply_fill(order: &mut SpotOrder, fill_quantity: BigDecimal) -> Result<(), SpotDomainError> {
    if fill_quantity <= BigDecimal::from(0) {
        return Err(SpotDomainError::NonPositiveQuantity);
    }

    let remaining = order.quantity.clone() - order.filled_quantity.clone();
    if fill_quantity > remaining {
        return Err(SpotDomainError::FillQuantityExceedsRemaining {
            remaining,
            fill: fill_quantity,
        });
    }

    let next_filled = order.filled_quantity.clone() + fill_quantity;
    let next_status = if next_filled == order.quantity {
        OrderStatus::Filled
    } else {
        OrderStatus::PartiallyFilled
    };

    transition_status(order.status, next_status)?;
    order.filled_quantity = next_filled;
    order.status = next_status;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReservedAmount {
    asset_id: String,
    amount: BigDecimal,
}

fn remaining_reserved_amount(
    order: &SpotOrder,
    base_asset_id: &str,
    quote_asset_id: &str,
) -> Result<ReservedAmount, SpotServiceError> {
    let remaining_quantity = order.quantity.clone() - order.filled_quantity.clone();
    match order.side {
        OrderSide::Buy => {
            let price = order
                .price
                .clone()
                .ok_or(SpotServiceError::MissingPriceForWalletReservation)?;
            Ok(ReservedAmount {
                asset_id: quote_asset_id.to_owned(),
                amount: price * remaining_quantity,
            })
        }
        OrderSide::Sell => Ok(ReservedAmount {
            asset_id: base_asset_id.to_owned(),
            amount: remaining_quantity,
        }),
    }
}

fn reserve_asset_id<'a>(
    side: OrderSide,
    base_asset_id: &'a str,
    quote_asset_id: &'a str,
) -> &'a str {
    match side {
        OrderSide::Buy => quote_asset_id,
        OrderSide::Sell => base_asset_id,
    }
}

fn reservation_amount(side: OrderSide, price: &BigDecimal, quantity: &BigDecimal) -> BigDecimal {
    match side {
        OrderSide::Buy => price.clone() * quantity.clone(),
        OrderSide::Sell => quantity.clone(),
    }
}

fn validate_common(quantity: &BigDecimal, pair: &TradingPairRule) -> Result<(), SpotDomainError> {
    if !pair.enabled {
        return Err(SpotDomainError::TradingPairDisabled);
    }
    if quantity <= &BigDecimal::from(0) {
        return Err(SpotDomainError::NonPositiveQuantity);
    }
    validate_precision(quantity, pair.quantity_precision).map_err(|()| {
        SpotDomainError::QuantityPrecisionExceeded {
            allowed: pair.quantity_precision,
        }
    })
}

fn validate_price(price: &BigDecimal, precision: u32) -> Result<(), SpotDomainError> {
    if price <= &BigDecimal::from(0) {
        return Err(SpotDomainError::NonPositivePrice);
    }
    validate_precision(price, precision)
        .map_err(|()| SpotDomainError::PricePrecisionExceeded { allowed: precision })
}

fn validate_min_order_value(
    actual: BigDecimal,
    pair: &TradingPairRule,
) -> Result<(), SpotDomainError> {
    if actual < pair.min_order_value {
        Err(SpotDomainError::MinOrderValueNotMet {
            actual,
            minimum: pair.min_order_value.clone(),
        })
    } else {
        Ok(())
    }
}

fn validate_precision(amount: &BigDecimal, precision: u32) -> Result<(), ()> {
    let (_, scale) = amount.normalized().as_bigint_and_exponent();
    if scale.max(0) as u32 <= precision {
        Ok(())
    } else {
        Err(())
    }
}

fn can_transition(current: OrderStatus, next: OrderStatus) -> bool {
    use OrderStatus::*;
    matches!(
        (current, next),
        (Pending, Open)
            | (Pending, PartiallyFilled)
            | (Pending, Filled)
            | (Pending, Cancelled)
            | (Pending, Rejected)
            | (Open, PartiallyFilled)
            | (Open, Filled)
            | (Open, Cancelled)
            | (PartiallyFilled, PartiallyFilled)
            | (PartiallyFilled, Filled)
            | (PartiallyFilled, Cancelled)
            | (Cancelled, Cancelled)
    )
}
