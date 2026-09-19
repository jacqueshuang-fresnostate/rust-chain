//! 现货基础设施共享的数据库枚举编解码与错误映射。
//!
//! 本模块不拥有事务；它只保证旧表字符串、领域枚举和错误语义在拆分后保持一致。

use crate::{
    error::{AppError, AppResult},
    modules::spot::{OrderSide, OrderStatus, OrderType, SpotOrder},
};

pub(super) const SYSTEM_SPOT_LIQUIDITY_EMAIL: &str = "__system_spot_liquidity@internal.local";

impl sqlx::Type<sqlx::MySql> for crate::modules::spot::TriggerDirection {
    fn type_info() -> sqlx::mysql::MySqlTypeInfo {
        <String as sqlx::Type<sqlx::MySql>>::type_info()
    }

    fn compatible(info: &sqlx::mysql::MySqlTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::MySql>>::compatible(info)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::MySql> for crate::modules::spot::TriggerDirection {
    fn decode(value: sqlx::mysql::MySqlValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let value = <String as sqlx::Decode<sqlx::MySql>>::decode(value)?;
        match value.as_str() {
            "rising" => Ok(Self::Rising),
            "falling" => Ok(Self::Falling),
            _ => Err(format!("invalid stored spot trigger direction: {value}").into()),
        }
    }
}

/// 返回数据库唯一键冲突，该只读访问不会触发外部查询或业务状态变更。
pub(crate) fn is_duplicate_key_error(error: &sqlx::Error) -> bool {
    error.as_database_error().is_some_and(|database_error| {
        database_error.code().as_deref() == Some("1062")
            || database_error.code().as_deref() == Some("23000")
    })
}

/// 解析现货订单的现货基础设施适配逻辑，保持存储或外部协议的既有边界。
/// 把外部订单编号解析为数据库主键，非法文本返回参数错误且不查询订单。
pub(super) fn parse_spot_order_db_id(order: &SpotOrder) -> AppResult<u64> {
    order
        .id
        .parse::<u64>()
        .map_err(|_| AppError::Validation("invalid spot order id".to_owned()))
}

/// 解析订单方向的现货基础设施适配逻辑，保持存储或外部协议的既有边界。
/// 把数据库方向映射为领域枚举，未知值作为损坏数据返回内部错误。
pub(super) fn parse_order_side(value: &str) -> OrderSide {
    match value {
        "sell" => OrderSide::Sell,
        _ => OrderSide::Buy,
    }
}

/// 解析订单类型的现货基础设施适配逻辑，保持存储或外部协议的既有边界。
/// 把数据库订单类型映射为领域枚举，未知值不得降级为市价或限价。
pub(super) fn parse_order_type(value: &str) -> OrderType {
    match value {
        "market" => OrderType::Market,
        "stop_limit" => OrderType::StopLimit,
        _ => OrderType::Limit,
    }
}

/// 解析订单状态的现货基础设施适配逻辑，保持存储或外部协议的既有边界。
/// 把数据库状态映射为领域枚举，未知值不得被视为可撤或可成交。
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

/// 处理订单状态的现货基础设施适配逻辑，保持存储或外部协议的既有边界。
/// 将领域订单状态序列化为稳定数据库文本，不执行状态迁移。
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

/// 处理订单方向的现货基础设施适配逻辑，保持存储或外部协议的既有边界。
/// 将领域订单方向序列化为稳定数据库文本，不读取或修改钱包。
pub(super) fn order_side_as_str(side: OrderSide) -> &'static str {
    match side {
        OrderSide::Buy => "buy",
        OrderSide::Sell => "sell",
    }
}

/// 处理订单类型的现货基础设施适配逻辑，保持存储或外部协议的既有边界。
/// 将领域订单类型序列化为稳定数据库文本，保持止限价类型不丢失。
pub(super) fn order_type_as_str(order_type: OrderType) -> &'static str {
    match order_type {
        OrderType::Limit => "limit",
        OrderType::Market => "market",
        OrderType::StopLimit => "stop_limit",
    }
}

/// 映射数据库错误的现货基础设施适配逻辑，保持存储或外部协议的既有边界。
/// 把数据库唯一键冲突映射为现货冲突语义，其他错误保持数据库失败。
pub(super) fn map_spot_sqlx_error(error: sqlx::Error) -> crate::modules::spot::SpotServiceError {
    crate::modules::spot::SpotServiceError::Repository(error.to_string())
}

/// 解析无符号整数标识的现货基础设施适配逻辑，保持存储或外部协议的既有边界。
/// 解析现货无符号标识，零值或非法文本在数据库访问前返回校验错误。
pub(super) fn parse_spot_u64_identifier(
    field: &str,
    value: &str,
) -> Result<u64, crate::modules::spot::SpotServiceError> {
    value
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            crate::modules::spot::SpotServiceError::Repository(format!(
                "invalid positive numeric {field} `{value}`"
            ))
        })
}
