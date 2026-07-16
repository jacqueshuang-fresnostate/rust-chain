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
