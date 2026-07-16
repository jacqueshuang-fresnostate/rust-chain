pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod repository;
pub mod service;
pub use domain::{RiskDecision, RiskReject, RiskRequest, RiskRules, evaluate_risk};

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_risk_mod_tests.rs"]
mod tests;
