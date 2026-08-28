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
        presentation::{
            CloseMarginPositionResponse, MarginPositionCloseExecutionResponse,
            MarginPositionResponse, MarginProductResponse,
        },
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
            "order_type": position.order_type,
            "limit_price": position.limit_price,
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

/// 仅当调用方已经首次提交了真实成交、且进程配置了广播中心时才推送开仓事件。
/// 幂等重放、未成交限价单和重复 ticker 都传入假，保证同一仓位只在入场价首次落库后通知一次。
/// 未配置 `hub` 属于既有降级形态，事件丢弃但开仓事务已提交，不视为错误。
pub(crate) fn publish_margin_position_opened_event_if_needed(
    hub: Option<&EventBroadcastHub>,
    user_id: u64,
    position: &MarginPositionResponse,
    is_new_fill: bool,
) {
    if is_new_fill && let Some(hub) = hub {
        publish_margin_position_opened_event(hub, user_id, position);
    }
}

/// 向用户私有频道推送平仓成功事件，载荷含退出价、已实现盈亏、应付利息和平仓时间毫秒戳。
/// `payout_amount` 由领域函数按保证金加盈亏减利息重算并非负截断，只作展示，不代表本次真实入账额；
/// 全仓仓位实际是以有符号组合权益更新共享钱包，逐仓才与该返还额一致。
/// 调用方须在平仓事务提交后调用；本函数不读库、不重算钱包余额，也不会修改仓位终态。
#[cfg(test)]
pub(crate) fn publish_margin_position_closed_event(
    hub: &EventBroadcastHub,
    user_id: u64,
    position: &MarginPositionResponse,
) {
    publish_margin_position_closed_event_with_settlement(hub, user_id, position, None);
}

/// 组装终态平仓提示；逐仓优先使用应用层传入的真实本次结算额，全仓同时保留有符号结算上下文。
/// `settlement_amount` 只可能来自刚提交的事务或不可变执行记录，缺失时才回退历史仓位推导口径。
fn publish_margin_position_closed_event_with_settlement(
    hub: &EventBroadcastHub,
    user_id: u64,
    position: &MarginPositionResponse,
    settlement_amount: Option<&BigDecimal>,
) {
    let derived_payout = margin_position_payout_amount(
        &position.margin_amount,
        position.realized_pnl.as_ref(),
        &position.interest_amount,
    );
    let payout_amount = if position.margin_mode == "isolated" {
        settlement_amount.unwrap_or(&derived_payout)
    } else {
        &derived_payout
    };
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
            "payout_amount": decimal_amount_string(payout_amount),
            "settlement_amount": settlement_amount.map(decimal_amount_string),
            "closed_at": position.closed_at.map(|closed_at| closed_at.timestamp_millis()),
            "status": position.status,
        })
        .to_string(),
    ));
}

/// 向用户私有频道发送部分平仓后的 REST 对账提示，金额字段只描述已提交执行而不指示客户端本地加减余额。
/// 事件同时带剩余仓位金额和不可变执行主键，便于多端诊断；真正钱包、仓位和风险状态仍以 REST 为准。
pub(crate) fn publish_margin_position_partially_closed_event(
    hub: &EventBroadcastHub,
    user_id: u64,
    position: &MarginPositionResponse,
    execution: &MarginPositionCloseExecutionResponse,
) {
    hub.publish(EventBroadcastMessage::private_user(
        user_id,
        json!({
            "type": "margin.position.partially_closed",
            "position_id": position.id,
            "product_id": position.product_id,
            "pair_id": position.pair_id,
            "margin_asset": position.margin_asset,
            "execution_id": execution.id,
            "close_percentage": execution.close_percentage,
            "close_margin_amount": decimal_amount_string(&execution.close_margin_amount),
            "close_notional_amount": decimal_amount_string(&execution.close_notional_amount),
            "realized_pnl": decimal_amount_string(&execution.realized_pnl),
            "settlement_amount": decimal_amount_string(&execution.settlement_amount),
            "remaining_margin_amount": decimal_amount_string(&position.margin_amount),
            "remaining_notional_amount": decimal_amount_string(&position.notional_amount),
            "fully_closed": false,
            "status": position.status,
        })
        .to_string(),
    ));
}

/// 在主动平仓事务提交后发布一次刷新提示：部分执行使用专用事件，终态执行沿用 closed 事件。
/// 幂等重放、终态兼容重放和未配置广播中心都不发送；广播失败不会回滚已经提交的资金事务。
pub(crate) fn publish_margin_position_close_event_if_needed(
    hub: Option<&EventBroadcastHub>,
    user_id: u64,
    response: &CloseMarginPositionResponse,
    is_new_execution: bool,
) {
    if !is_new_execution {
        return;
    }
    let Some(hub) = hub else {
        return;
    };
    if let Some(execution) = response
        .execution
        .as_ref()
        .filter(|execution| !execution.fully_closed)
    {
        publish_margin_position_partially_closed_event(hub, user_id, &response.position, execution);
    } else {
        publish_margin_position_closed_event_with_settlement(
            hub,
            user_id,
            &response.position,
            response.settlement_amount.as_ref(),
        );
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
        "price_precision": product.price_precision,
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
