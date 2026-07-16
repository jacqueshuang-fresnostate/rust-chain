//! loan bounded context 聚合模块。
//!
//! 统一导出贷款领域的应用服务与领域常量，确保路由与具体实现之间的分层依赖清晰。
pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod repository;
pub mod routes;
pub mod service;

pub(crate) use self::domain::{
    LOAN_TYPE_COLLATERALIZED, STATUS_ACTIVE, STATUS_CANCELLED, STATUS_DISBURSED, STATUS_PENDING,
    STATUS_REJECTED, STATUS_REPAID,
};
pub(crate) use self::service::{
    ensure_amount_precision, ensure_amount_within_product_limits, ensure_non_negative_amount,
    ensure_positive_amount, normalized_product_name_json, optional_string, product_default_name,
    route_limit, validate_idempotency_key, validate_interest_mode, validate_loan_type,
    validate_product_status,
};

pub use routes::{admin_routes, user_routes};

#[cfg(test)]
#[path = "../../tests/unit_src/src_modules_loan_tests.rs"]
mod tests;
