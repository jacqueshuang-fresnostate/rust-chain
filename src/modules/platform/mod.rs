//! platform bounded context 聚合模块。
//!
//! 统一管理平台品牌/配置相关的领域与应用服务接入，保持上下文内职责内聚。
//! 该上下文只维护一份全局品牌配置：站点名称、Logo 地址与行情图表提供方，在库中固定为行名 default 的单行。
//! 前端启动时读取它初始化外观，后台在带行锁的事务中保存并留下前后快照供审计；
//! 首次部署由幂等补齐语句写入初始行，因此不存在配置缺失导致前端无法渲染的情况。
pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod routes;

pub use application::{PlatformBrandChange, load_platform_brand, save_platform_brand_in_tx};
pub use presentation::{
    PlatformBrandResponse, SavePlatformBrandRequest, platform_brand_audit_json,
};
pub use routes::routes;
