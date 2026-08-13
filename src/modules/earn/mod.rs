//! earn bounded context 聚合模块。
//!
//! 理财业务按 DDD 分层拆分，入口模块只做子模块声明，不承载任何业务逻辑。
//! 该限界上下文覆盖三类对象：产品分类、理财产品配置，以及用户申购产生的订阅。
//! 贯穿全模块的核心约定是费率快照：订阅在申购时把产品的 APR、期限和四项费率字段
//! 逐一复制进 `earn_subscriptions`，此后后台改配置不会回溯影响任何既有订阅。
//! `redemption` 为 crate 内可见，因为赎回算式只服务于本上下文，不对外暴露。

pub mod application;
pub mod infrastructure;
pub mod presentation;
pub(crate) mod redemption;
pub mod repository;
pub mod routes;
pub mod service;
