//! 杠杆交易限界上下文入口。
//!
//! 该上下文覆盖杠杆产品配置、用户杠杆倍数与保证金模式设置、现货与杠杆账户互转、
//! 市价即时开仓、限价挂单与权威 ticker 触发成交、逐仓/全仓平仓结算、未成交撤单，以及仓位与利息读模型。
//! 子模块按 DDD 分层：`domain` 放不依赖 I/O 的风险与返还计算，`application` 编排事务和幂等，
//! `infrastructure` 承担 MySQL 与 Redis 适配，`presentation` 定义传输 DTO，`routes` 只做参数转发。
//! 利息计提与强平由 `crate::workers` 下的独立后台任务驱动，不在本上下文的请求路径内执行。

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod routes;
pub mod service;

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_margin_application_tests.rs"]
mod tests;
