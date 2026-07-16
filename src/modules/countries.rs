//! countries bounded context 聚合模块。
//!
//! 统一导出国家与本地化相关的 DDD 分层入口，并保持内部边界清晰。
pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod repository;
pub mod routes;
pub mod service;

pub use domain::{
    ensure_default_locale_supported, normalize_country_code, normalize_locale,
    normalize_supported_locales,
};

pub use routes::routes;
