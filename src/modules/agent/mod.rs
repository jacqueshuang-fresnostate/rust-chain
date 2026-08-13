//! agent bounded context module root.
//!
//! 多级代理分销限界上下文：管理最多三级的代理层级树、代理专属邀请码与下级用户归属关系，
//! 并在现货、杠杆、秒合约、竞猜、闪兑五类业务结算时按累计比例向上逐层计提差额返佣。
//! 层级关系用物化路径表达，代理自助后台的一切查询都以该路径为可见边界；
//! 待结算佣金由 `workers::agent_commission_settlement` 定时打入代理钱包。

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod repository;
pub mod routes;
pub mod service;

// 兼容导出，保持现有根模块 API 稳定，业务行为集中在 domain/service/repository 层。
pub use domain::{AgentScope, AgentTeamUser};

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_agent_mod_tests.rs"]
mod tests;
