pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod repository;
pub mod service;

pub mod market_feed_config;
pub mod routes;
pub mod smtp_config;
pub mod upload_config;

pub use self::domain::AdminScope;
pub use self::domain::{SensitiveConfirmationError, SensitiveOperationConfirmation};

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_admin_mod_tests.rs"]
mod tests;
