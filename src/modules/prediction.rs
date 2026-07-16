//! prediction bounded context 聚合模块。
//!
//! 统一导出预测订单与资金相关的 DDD 分层 API。
pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod repository;
pub mod routes;
pub mod service;

pub use application::run_sync_loop;
pub use routes::{admin_routes, user_routes};

#[cfg(test)]
#[path = "../../tests/unit_src/src_modules_prediction_tests.rs"]
mod tests;
