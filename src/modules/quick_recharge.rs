//! quick_recharge bounded context 聚合模块。
//!
//! 将快速充值功能的领域、服务与路由汇聚到统一入口，遵循 DDD 边界约束。
pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod repository;
pub mod routes;
pub mod service;

pub use presentation::{
    CreateQuickRechargeOrderRequest, DeleteQuickRechargeOrderRequest, QuickRechargeConfigResponse,
    QuickRechargeOrderResponse, QuickRechargeOrdersQuery, QuickRechargeOrdersResponse,
    QuickRechargeReturnTarget, SaveQuickRechargeConfigRequest, TestQuickRechargeConfigRequest,
    TestQuickRechargeConfigResponse, UserQuickRechargeConfigResponse,
};
pub use service::gmpay_signature;

pub use routes::{admin_routes, public_routes, user_routes};

#[cfg(test)]
#[path = "../../tests/unit_src/src_modules_quick_recharge_tests.rs"]
mod tests;
