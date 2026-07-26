pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod repository;
pub mod service;
pub use application::{RiskGuardInput, enforce_risk_control};
pub use domain::{RiskDecision, RiskReject, RiskRequest, RiskRules, evaluate_risk};
pub use service::{RiskPolicy, RiskScope, StoredRiskRule, resolve_risk_policy};

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_risk_mod_tests.rs"]
mod tests;
