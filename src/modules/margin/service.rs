//! margin bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务服务逐步迁入。
//! 现有职责有三类：把仓位快照序列化成用户私有频道的开仓、平仓与撤销 WebSocket 事件，
//! 解析后台 JWT subject 中的管理员标识，以及生成杠杆产品的前后快照审计 JSON。
//! 事件发布一律在资金事务提交成功之后由应用层调用，本文件不持有事务、不访问数据库、不回滚。

use crate::error::{AppError, AppResult};
use crate::modules::{
    events::{EventBroadcastHub, EventBroadcastMessage},
    margin::{
        domain::margin_position_payout_amount,
        presentation::{MarginPositionResponse, MarginProductResponse},
    },
};
use bigdecimal::BigDecimal;
use serde_json::{Value, json};

/// 把资金类金额固定格式化为十八位小数字符串，与钱包和仓位列的存储精度一致。
/// 事件里的借款额、利息和返还额必须走这里，避免 JSON 数值序列化把大额十进制转成浮点后丢精度。
fn decimal_amount_string(amount: &BigDecimal) -> String {
    format!("{amount:.18}")
}

/// 向用户私有频道推送开仓成功事件，载荷含仓位标识、交易对、保证金模式、方向、杠杆和入场价。
/// 借款额与利息用十八位小数字符串输出，其余金额沿用仓位响应自身的序列化格式。
/// 调用方必须已提交开仓事务；发布只是通知，本函数不校验仓位状态也不补写任何资金记录。
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

/// 仅当本次调用真正新建了仓位、且进程配置了广播中心时才推送开仓事件。
/// 幂等键命中重放时 `is_new_position` 为假，直接静默返回，保证同一开仓请求不会被重复通知。
/// 未配置 `hub` 属于既有降级形态，事件丢弃但开仓事务已提交，不视为错误。
pub(crate) fn publish_margin_position_opened_event_if_needed(
    hub: Option<&EventBroadcastHub>,
    user_id: u64,
    position: &MarginPositionResponse,
    is_new_position: bool,
) {
    if is_new_position && let Some(hub) = hub {
        publish_margin_position_opened_event(hub, user_id, position);
    }
}

/// 向用户私有频道推送平仓成功事件，载荷含退出价、已实现盈亏、应付利息和平仓时间毫秒戳。
/// `payout_amount` 由领域函数按保证金加盈亏减利息重算并非负截断，只作展示，不代表本次真实入账额；
/// 全仓仓位实际是以有符号组合权益更新共享钱包，逐仓才与该返还额一致。
/// 调用方须在平仓事务提交后调用；本函数不读库、不重算钱包余额，也不会修改仓位终态。
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

/// 仅当本次调用真正完成了首次平仓结算、且广播中心可用时才推送平仓事件。
/// 对已是 closed、canceled 或 liquidated 的仓位重复发起平仓时 `is_new_close` 为假，不重复通知。
/// 批量平仓逐笔提交后立即调用，前序成功的通知不会被后续单笔失败吞掉。
pub(crate) fn publish_margin_position_closed_event_if_needed(
    hub: Option<&EventBroadcastHub>,
    user_id: u64,
    position: &MarginPositionResponse,
    is_new_close: bool,
) {
    if is_new_close && let Some(hub) = hub {
        publish_margin_position_closed_event(hub, user_id, position);
    }
}

/// 向用户私有频道推送撤单成功事件，载荷比平仓事件少了退出价、盈亏和返还额三项。
/// 因为撤销只针对没有入场价的未成交仓位，保证金原额退回，不存在成交价和已实现盈亏。
/// 调用方须在撤销事务提交后调用；本函数不查询仓位当前状态，也不产生任何资金写入。
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

/// 仅当本次调用真正把仓位迁移到 canceled、且广播中心可用时才推送撤单事件。
/// 对已撤销仓位重复请求时 `is_new_cancel` 为假，保证金不会二次退回，也不会再发一条通知。
/// 与平仓事件包装层的差别在于撤销路径不依赖 Redis 行情，因此没有行情失败导致的静默分支。
pub(crate) fn publish_margin_position_canceled_event_if_needed(
    hub: Option<&EventBroadcastHub>,
    user_id: u64,
    position: &MarginPositionResponse,
    is_new_cancel: bool,
) {
    if is_new_cancel && let Some(hub) = hub {
        publish_margin_position_canceled_event(hub, user_id, position);
    }
}

/// 从后台 JWT 的 subject 中剥离 `admin:` 前缀并解析出管理员数字标识，用于审计记录归属。
/// 缺少前缀或余下部分不是合法 u64 一律返回 Unauthorized，杜绝把用户令牌当成管理员令牌使用。
pub(crate) fn admin_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("admin:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 把杠杆产品完整配置摊平成审计 JSON，供后台创建、改配和启停写入 before/after 快照。
/// 覆盖交易对、保证金币种、可选保证金模式集合、杠杆档位、最大杠杆、最小最大保证金、
/// 维持保证金率、小时利率与启停状态，确保任何一项被改动都能在审计里逐字段比对出来。
/// 数值直接沿用 `BigDecimal` 的序列化结果以保留原始精度，函数本身不访问存储也不修改产品配置。
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

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_margin_service_tests.rs"]
mod tests;
