//! agent bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的纯业务规则。

use crate::{
    architecture::DomainLayer,
    error::{AppError, AppResult},
    modules::wallet::truncate_amount_to_asset_precision,
};
use bigdecimal::BigDecimal;

#[derive(Debug)]
pub struct DomainLayerMarker;

impl DomainLayer for DomainLayerMarker {}

pub const MAX_AGENT_LEVEL: i32 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentHierarchyNode {
    pub(crate) id: u64,
    pub(crate) parent_agent_id: Option<u64>,
    pub(crate) root_agent_id: u64,
    pub(crate) level: i32,
    pub(crate) path: String,
    pub(crate) status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentHierarchyPlacement {
    pub(crate) parent_agent_id: Option<u64>,
    pub(crate) root_agent_id: Option<u64>,
    pub(crate) level: i32,
    pub(crate) path_prefix: Option<String>,
}

/// 根据父代理推导新代理层级。超级管理员是虚拟 0 级，不写入 agents 表。
pub(crate) fn derive_agent_placement(
    parent: Option<&AgentHierarchyNode>,
    requested_level: Option<i32>,
) -> AppResult<AgentHierarchyPlacement> {
    let placement = match parent {
        None => AgentHierarchyPlacement {
            parent_agent_id: None,
            root_agent_id: None,
            level: 1,
            path_prefix: None,
        },
        Some(parent) => {
            if parent.status != "active" {
                return Err(AppError::Conflict("parent agent must be active".to_owned()));
            }
            if !(1..=MAX_AGENT_LEVEL).contains(&parent.level) {
                return Err(AppError::Conflict(
                    "parent agent hierarchy is invalid".to_owned(),
                ));
            }
            let level = parent.level + 1;
            if level > MAX_AGENT_LEVEL {
                return Err(AppError::Validation(
                    "agent hierarchy supports at most three levels".to_owned(),
                ));
            }
            AgentHierarchyPlacement {
                parent_agent_id: Some(parent.id),
                root_agent_id: Some(parent.root_agent_id),
                level,
                path_prefix: Some(parent.path.clone()),
            }
        }
    };

    if requested_level.is_some_and(|level| level != placement.level) {
        return Err(AppError::Validation(format!(
            "level must match the derived agent hierarchy level {}",
            placement.level
        )));
    }
    Ok(placement)
}

pub(crate) fn agent_path(path_prefix: Option<&str>, agent_id: u64) -> String {
    match path_prefix {
        Some(prefix) => format!("{prefix}/agent:{agent_id}"),
        None => format!("/agent:{agent_id}"),
    }
}

pub(crate) fn is_same_or_descendant_path(scope_path: &str, candidate_path: &str) -> bool {
    candidate_path == scope_path
        || candidate_path
            .strip_prefix(scope_path)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AgentCommissionRateTier {
    pub(crate) agent_id: u64,
    /// 该代理配置的是从成交用户向上累计可分配的比例。
    pub(crate) cumulative_rate: BigDecimal,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AgentCommissionAllocation {
    pub(crate) agent_id: u64,
    pub(crate) commission_rate: BigDecimal,
    pub(crate) commission_amount: BigDecimal,
}

pub(crate) fn allocate_differential_agent_commissions(
    tiers_from_owner_to_root: &[AgentCommissionRateTier],
    source_amount: &BigDecimal,
    precision_scale: i32,
) -> Vec<AgentCommissionAllocation> {
    if source_amount <= &BigDecimal::from(0) {
        return Vec::new();
    }

    let zero = BigDecimal::from(0);
    let one = BigDecimal::from(1);
    let mut allocated_rate = zero.clone();
    let mut allocated_amount = truncate_amount_to_asset_precision(&zero, precision_scale);
    let mut allocations = Vec::new();

    for tier in tiers_from_owner_to_root {
        // 非法或倒挂的累计比例不会阻断用户交易，也不能造成负返佣或超额分配。
        if tier.cumulative_rate <= allocated_rate || tier.cumulative_rate > one {
            continue;
        }

        let cumulative_amount = truncate_amount_to_asset_precision(
            &(source_amount.clone() * tier.cumulative_rate.clone()),
            precision_scale,
        );
        let commission_rate = tier.cumulative_rate.clone() - allocated_rate;
        let commission_amount = cumulative_amount.clone() - allocated_amount;
        allocated_rate = tier.cumulative_rate.clone();
        allocated_amount = cumulative_amount;

        if commission_amount > zero {
            allocations.push(AgentCommissionAllocation {
                agent_id: tier.agent_id,
                commission_rate,
                commission_amount,
            });
        }
    }

    allocations
}

#[derive(Debug, Clone)]
pub struct AgentScope {
    pub agent_id: String,
    pub agent_path: String,
}

impl AgentScope {
    /// 只允许访问当前代理节点及其后代代理归属的用户，避免越权查看父级或兄弟团队。
    pub fn can_access_user(&self, user: &AgentTeamUser) -> bool {
        user.agent_path
            .as_deref()
            .is_some_and(|path| is_same_or_descendant_path(&self.agent_path, path))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTeamUser {
    pub user_id: String,
    pub agent_path: Option<String>,
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_agent_domain_tests.rs"]
mod tests;
