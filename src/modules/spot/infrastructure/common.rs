//! 现货基础设施共享的数据库枚举编解码与错误映射。
//!
//! 本模块不拥有事务；它只保证旧表字符串、领域枚举和错误语义在拆分后保持一致。

use crate::{
    error::{AppError, AppResult},
    modules::spot::{OrderSide, OrderStatus, OrderType, SpotOrder, SpotServiceError},
};

pub(super) const SYSTEM_SPOT_LIQUIDITY_EMAIL: &str = "__system_spot_liquidity@internal.local";

pub(crate) fn is_duplicate_key_error(error: &sqlx::Error) -> bool {
    error.as_database_error().is_some_and(|database_error| {
        database_error.code().as_deref() == Some("1062")
            || database_error.code().as_deref() == Some("23000")
    })
}

pub(super) fn parse_spot_order_db_id(order: &SpotOrder) -> AppResult<u64> {
    order
        .id
        .parse::<u64>()
        .map_err(|_| AppError::Validation("invalid spot order id".to_owned()))
}

pub(super) fn parse_order_side(value: &str) -> OrderSide {
    match value {
        "sell" => OrderSide::Sell,
        _ => OrderSide::Buy,
    }
}

pub(super) fn parse_order_type(value: &str) -> OrderType {
    match value {
        "market" => OrderType::Market,
        "stop_limit" => OrderType::StopLimit,
        _ => OrderType::Limit,
    }
}

pub(super) fn parse_order_status(value: &str) -> OrderStatus {
    match value {
        "open" => OrderStatus::Open,
        "partially_filled" => OrderStatus::PartiallyFilled,
        "filled" => OrderStatus::Filled,
        "cancelled" => OrderStatus::Cancelled,
        "rejected" => OrderStatus::Rejected,
        _ => OrderStatus::Pending,
    }
}

pub(super) fn order_status_as_str(status: OrderStatus) -> &'static str {
    match status {
        OrderStatus::Pending => "pending",
        OrderStatus::Open => "open",
        OrderStatus::PartiallyFilled => "partially_filled",
        OrderStatus::Filled => "filled",
        OrderStatus::Cancelled => "cancelled",
        OrderStatus::Rejected => "rejected",
    }
}

pub(super) fn order_side_as_str(side: OrderSide) -> &'static str {
    match side {
        OrderSide::Buy => "buy",
        OrderSide::Sell => "sell",
    }
}

pub(super) fn order_type_as_str(order_type: OrderType) -> &'static str {
    match order_type {
        OrderType::Limit => "limit",
        OrderType::Market => "market",
        OrderType::StopLimit => "stop_limit",
    }
}

pub(super) fn map_spot_service_error(error: SpotServiceError) -> AppError {
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

pub(super) fn map_spot_sqlx_error(error: sqlx::Error) -> crate::modules::spot::SpotServiceError {
    crate::modules::spot::SpotServiceError::Repository(error.to_string())
}

pub(super) fn parse_spot_u64_identifier(
    field: &str,
    value: &str,
) -> Result<u64, crate::modules::spot::SpotServiceError> {
    value.parse::<u64>().map_err(|error| {
        crate::modules::spot::SpotServiceError::Repository(format!(
            "invalid numeric {field} `{value}`: {error}"
        ))
    })
}
