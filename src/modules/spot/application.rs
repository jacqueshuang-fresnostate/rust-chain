//! Spot bounded context application façade.
//!
//! 本文件只保留职责子模块声明与兼容 re-export；订单查询、撤单、创建、触发撮合、
//! 成交结算及幂等实现分别位于 `application/` 下的专用模块。

mod cancellation;
mod idempotency;
mod order_creation;
mod queries;
mod settlement;
mod triggering;

pub(crate) use cancellation::{
    cancel_admin_spot_order_with_events, cancel_all_user_spot_orders_with_events,
    cancel_user_spot_order_with_events, validate_admin_cancel_spot_order_request,
};
pub(crate) use order_creation::create_spot_order_with_events;
pub(crate) use queries::{
    get_admin_spot_order, list_admin_spot_orders, list_admin_spot_trades, list_user_spot_orders,
    list_user_spot_trades, mysql_pool,
};
pub(crate) use settlement::fill_spot_orders_with_events_with_request;
pub use triggering::execute_triggered_spot_limit_orders_with_hub;

#[cfg(test)]
pub(crate) use order_creation::build_create_spot_order;
#[cfg(test)]
pub(crate) use queries::route_limit;

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_spot_application_tests.rs"]
mod tests;
