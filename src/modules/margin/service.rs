//! margin bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务服务逐步迁入。

use crate::error::{AppError, AppResult};
use crate::{
    architecture::ServiceLayer,
    modules::{
        events::{EventBroadcastHub, EventBroadcastMessage},
        margin::{
            application::margin_position_payout_amount,
            presentation::{MarginPositionResponse, MarginProductResponse},
        },
    },
};
use bigdecimal::BigDecimal;
use serde_json::{Value, json};

#[derive(Debug)]
pub struct ServiceLayerMarker;

impl ServiceLayer for ServiceLayerMarker {}

fn decimal_amount_string(amount: &BigDecimal) -> String {
    format!("{amount:.18}")
}

/// 推送保证金仓位开仓成功事件到用户私有频道。
pub(crate) fn publish_margin_position_opened_event(
    hub: &EventBroadcastHub,
    user_id: u64,
    position: &MarginPositionResponse,
) {
    hub.publish(EventBroadcastMessage::private_user(
        user_id,
        json!({
            "type": "margin.position.opened",
            "position_id": position.id,
            "product_id": position.product_id,
            "pair_id": position.pair_id,
            "margin_asset": position.margin_asset,
            "margin_mode": position.margin_mode,
            "direction": position.direction,
            "margin_amount": position.margin_amount,
            "leverage": position.leverage,
            "notional_amount": position.notional_amount,
            "borrowed_amount": decimal_amount_string(&position.borrowed_amount),
            "interest_amount": decimal_amount_string(&position.interest_amount),
            "entry_price": position.entry_price,
            "status": position.status,
        })
        .to_string(),
    ));
}

/// 在仓位新创建且广播通道可用时推送开仓事件。
pub(crate) fn publish_margin_position_opened_event_if_needed(
    hub: Option<&EventBroadcastHub>,
    user_id: u64,
    position: &MarginPositionResponse,
    is_new_position: bool,
) {
    if is_new_position {
        if let Some(hub) = hub {
            publish_margin_position_opened_event(hub, user_id, position);
        }
    }
}

/// 推送保证金仓位平仓成功事件到用户私有频道。
pub(crate) fn publish_margin_position_closed_event(
    hub: &EventBroadcastHub,
    user_id: u64,
    position: &MarginPositionResponse,
) {
    hub.publish(EventBroadcastMessage::private_user(
        user_id,
        json!({
            "type": "margin.position.closed",
            "position_id": position.id,
            "product_id": position.product_id,
            "pair_id": position.pair_id,
            "margin_asset": position.margin_asset,
            "direction": position.direction,
            "margin_amount": position.margin_amount,
            "exit_price": position.exit_price,
            "realized_pnl": position.realized_pnl,
            "interest_amount": decimal_amount_string(&position.interest_amount),
            "payout_amount": decimal_amount_string(&margin_position_payout_amount(
                &position.margin_amount,
                position.realized_pnl.as_ref(),
                &position.interest_amount,
            )),
            "closed_at": position.closed_at.map(|closed_at| closed_at.timestamp_millis()),
            "status": position.status,
        })
        .to_string(),
    ));
}

/// 在仓位已平仓且广播通道可用时推送平仓事件。
pub(crate) fn publish_margin_position_closed_event_if_needed(
    hub: Option<&EventBroadcastHub>,
    user_id: u64,
    position: &MarginPositionResponse,
    is_new_close: bool,
) {
    if is_new_close {
        if let Some(hub) = hub {
            publish_margin_position_closed_event(hub, user_id, position);
        }
    }
}

/// 推送保证金仓位取消成功事件到用户私有频道。
pub(crate) fn publish_margin_position_canceled_event(
    hub: &EventBroadcastHub,
    user_id: u64,
    position: &MarginPositionResponse,
) {
    hub.publish(EventBroadcastMessage::private_user(
        user_id,
        json!({
            "type": "margin.position.canceled",
            "position_id": position.id,
            "product_id": position.product_id,
            "pair_id": position.pair_id,
            "margin_asset": position.margin_asset,
            "direction": position.direction,
            "margin_amount": position.margin_amount,
            "closed_at": position.closed_at.map(|closed_at| closed_at.timestamp_millis()),
            "status": position.status,
        })
        .to_string(),
    ));
}

/// 在仓位已取消且广播通道可用时推送取消事件。
pub(crate) fn publish_margin_position_canceled_event_if_needed(
    hub: Option<&EventBroadcastHub>,
    user_id: u64,
    position: &MarginPositionResponse,
    is_new_cancel: bool,
) {
    if is_new_cancel {
        if let Some(hub) = hub {
            publish_margin_position_canceled_event(hub, user_id, position);
        }
    }
}

/// 解析 JWT subject 中的管理员标识。
pub(crate) fn admin_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("admin:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

pub(crate) fn margin_product_audit_json(product: &MarginProductResponse) -> Value {
    json!({
        "id": product.id,
        "pair_id": product.pair_id,
        "symbol": product.symbol,
        "margin_asset": product.margin_asset,
        "margin_asset_symbol": product.margin_asset_symbol,
        "logo_url": product.logo_url,
        "margin_mode": product.margin_mode,
        "margin_modes": product.margin_modes.0,
        "leverage_levels": product.leverage_levels.0,
        "max_leverage": product.max_leverage,
        "min_margin": product.min_margin,
        "max_margin": product.max_margin,
        "maintenance_margin_rate": product.maintenance_margin_rate,
        "hourly_interest_rate": product.hourly_interest_rate,
        "status": product.status,
    })
}
