//! countries bounded context 聚合模块。
//!
//! 统一导出国家与本地化相关的 DDD 分层入口，并保持内部边界清晰。
//! 该上下文维护国家配置：国家代码、显示名、默认语言、支持语言列表、注册开关与启用状态。
//! 对外只提供开放注册的国家清单，供注册页填充选项并决定初始界面语言；
//! 国家代码与语言代码的归一化规则也在此导出，供新闻等需要按地区过滤的上下文复用同一套取值口径。
pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod routes;

pub use domain::{
    ensure_default_locale_supported, normalize_country_code, normalize_locale,
    normalize_supported_locales,
};

pub use routes::routes;
