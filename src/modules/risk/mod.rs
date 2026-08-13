//! risk bounded context module root.
//!
//! 风控规则引擎限界上下文：后台按目标类型与目标 ID 配置规则行，规则内容以 JSON 描述限频、
//! 单笔金额上限、价格偏离上限和禁止操作四类维度。业务侧在执行动作前调用统一闸门，
//! 闸门实时读取启用规则、合并出本次生效的最严阈值、评估后决定放行还是以 403 阻断请求。
//! 命中拒绝时会写入风控事件留痕；未配置规则或缺少判定所需事实时一律放行。

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod repository;
pub mod service;
pub use application::{RiskGuardInput, enforce_risk_control};
pub use domain::{RiskDecision, RiskReject, RiskRequest, RiskRules, evaluate_risk};
pub use service::{RiskPolicy, RiskScope, StoredRiskRule, resolve_risk_policy};

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_risk_mod_tests.rs"]
mod tests;
