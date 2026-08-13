//! agent bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的纯业务规则。
//! 本文件承载多级代理分销的两条核心规则：代理层级树的物化路径推导与子树归属判定，
//! 以及按累计比例向上逐层分配的差额返佣算法。所有函数均为纯计算，不访问数据库、
//! 缓存或钱包，调用方需自行保证传入的代理节点与比例来自权威查询结果。

use crate::{
    error::{AppError, AppResult},
    modules::wallet::truncate_amount_to_asset_precision,
};
use bigdecimal::BigDecimal;

/// 代理分销树允许的最大层级，取值一到三，超过该深度的下级不再单独建代理节点。
pub const MAX_AGENT_LEVEL: i32 = 3;

/// 父代理节点在推导落位时所需的最小快照，字段来自代理表的权威查询结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentHierarchyNode {
    pub(crate) id: u64,
    pub(crate) parent_agent_id: Option<u64>,
    pub(crate) root_agent_id: u64,
    /// 层级序号，一级为顶级代理，合法区间是一到 `MAX_AGENT_LEVEL`。
    pub(crate) level: i32,
    /// 物化路径，形如 `/agent:1/agent:7`，用于子树鉴权与前缀聚合。
    pub(crate) path: String,
    /// 代理状态，只有 active 才允许在其下继续挂接新代理。
    pub(crate) status: String,
}

/// 新代理的落位结果，根节点与路径前缀为空表示这是一条新的顶级代理线。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentHierarchyPlacement {
    pub(crate) parent_agent_id: Option<u64>,
    pub(crate) root_agent_id: Option<u64>,
    pub(crate) level: i32,
    pub(crate) path_prefix: Option<String>,
}

/// 根据可选父代理推导层级、根节点与路径前缀；超级管理员仅是虚拟零级。
/// 父节点须启用且层级合法，调用方声明层级必须与服务端推导结果一致，本规则不查询代理表。
/// 无父代理时定位为一级，根节点与路径前缀留空，由写入方在拿到自增主键后自行补齐；
/// 有父代理时层级取父级加一，根节点直接继承父级的根，超过三级上限按参数校验失败拒绝。
/// 父级状态非 active 或层级越界属于数据面异常，返回冲突而非校验错误，以便与用户输入问题区分。
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

/// 拼出代理节点的物化路径：有父路径时在其后追加 agent 段，无父路径时生成以斜杠开头的根路径。
/// 该路径是后续子树鉴权与 LIKE 前缀统计的唯一依据，因此分隔符与段格式必须保持稳定，不做去重或存在性校验。
/// 调用方须传入数据库已分配的代理主键，否则会写出无法与真实节点对应的路径。
pub(crate) fn agent_path(path_prefix: Option<&str>, agent_id: u64) -> String {
    match path_prefix {
        Some(prefix) => format!("{prefix}/agent:{agent_id}"),
        None => format!("/agent:{agent_id}"),
    }
}

/// 判定候选路径是否落在授权子树内：等于范围路径本身，或剥离该前缀后剩余部分以斜杠开头才算真正后代。
/// 强制要求后续分隔符是为了阻断同名文本前缀越权，范围为 agent:1 时不能命中 agent:12 所属的另一条团队线。
/// 比较为纯字符串匹配，不校验节点是否存在或仍处于启用状态。
pub(crate) fn is_same_or_descendant_path(scope_path: &str, candidate_path: &str) -> bool {
    candidate_path == scope_path
        || candidate_path
            .strip_prefix(scope_path)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

/// 代理链上某一层的返佣档位，切片顺序必须从直属代理排到根代理。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AgentCommissionRateTier {
    pub(crate) agent_id: u64,
    /// 该代理配置的是从成交用户向上累计可分配的比例。
    pub(crate) cumulative_rate: BigDecimal,
}

/// 单层代理实际可得的返佣，比例与金额都是扣除下层占用后的差额部分。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AgentCommissionAllocation {
    pub(crate) agent_id: u64,
    /// 本层独得比例，等于本档累计比例减去下层已占用的累计比例。
    pub(crate) commission_rate: BigDecimal,
    /// 已按发放资产精度截断的返佣金额，恒为正数。
    pub(crate) commission_amount: BigDecimal,
}

/// 按直属代理到根代理的累计比例，计算每层实际可分配的差额返佣。
/// 非正业务基数、倒挂或超过一的比例被跳过；金额先按发放资产精度量化，无持久化副作用。
/// 输入切片必须按直属代理在前、根代理在后排序，每档配置的是从成交用户向上累计的总分成比例，
/// 因此本层实得比例等于本档累计比例减去下层已占用比例，各层合计恒等于最高一档，不会超发。
/// 金额侧先用累计比例乘业务基数并截断到发放资产精度，再减去已累计发放额，避免逐层独立截断累积出碎屑差额。
/// 差额金额为零的层不产出分配项；返回结果仅供调用方落库，本函数不写记录也不动钱包余额。
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

/// 当前登录代理的可见范围，路径由服务端按凭证解析得出，不接受客户端传入覆盖。
#[derive(Debug, Clone)]
pub struct AgentScope {
    pub agent_id: String,
    pub agent_path: String,
}

impl AgentScope {
    /// 判断该业务用户是否处于本代理可见范围：仅当用户已记录归属路径且落在当前节点或其后代子树时放行。
    /// 归属路径缺失的用户一律拒绝，父级与兄弟团队同样不可见；本方法只做内存比较，不回查代理表确认节点仍启用。
    pub fn can_access_user(&self, user: &AgentTeamUser) -> bool {
        user.agent_path
            .as_deref()
            .is_some_and(|path| is_same_or_descendant_path(&self.agent_path, path))
    }
}

/// 团队用户的归属快照，路径为空表示该用户尚未挂接到任何代理线下。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTeamUser {
    pub user_id: String,
    pub agent_path: Option<String>,
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_agent_domain_tests.rs"]
mod tests;
