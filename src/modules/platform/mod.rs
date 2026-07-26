//! platform bounded context 聚合模块。
//!
//! 统一管理平台品牌/配置相关的领域与应用服务接入，保持上下文内职责内聚。
pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod repository;
pub mod routes;
pub mod service;

pub use application::{PlatformBrandChange, load_platform_brand, save_platform_brand_in_tx};
pub use presentation::{
    PlatformBrandResponse, SavePlatformBrandRequest, platform_brand_audit_json,
};
pub use routes::routes;
