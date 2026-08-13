//! 代理层级、门户账号与代理佣金的纯业务规则层。
//!
//! 这里只做请求形状校验、口令散列和审计快照构造，不查库也不加锁：
//! 代理是否存在、父链是否处于启用状态、层级与父级是否自洽、佣金当前是否仍是待处理，
//! 这些依赖数据库状态的判定统一由 application 层锁行后复核。
//! 明文口令只在散列函数内部停留，任何审计快照都不会包含口令或其散列值。

use super::*;

/// 规范化佣金审核状态，仅允许 `settled` 或 `rejected`，拒绝空值和其他生命周期状态。
/// pending 被刻意排除在外，因为该状态是佣金生成时的初值而不是人工可设置的目标，
/// 从 pending 迁出是单向的，重复处理会在应用层被冲突拦截。
pub(crate) fn validate_agent_commission_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "settled" | "rejected" => Ok(status),
        _ => Err(AppError::Validation(
            "unsupported agent commission status".to_owned(),
        )),
    }
}

/// 校验批量佣金 ID：至少一项、最多 200 项且不能重复，保持原顺序返回供事务逐条锁定。
/// 这里只检查请求集合，不查询佣金是否存在或可结算；状态与代理归属由应用事务确认。
/// 保持原顺序而非排序是刻意的，批处理会按该顺序逐条开独立事务，因此提交顺序即实际处理顺序。
/// 批内重复直接整体报错而不是去重，避免调用方误以为重复项被静默合并而对不上结果条数。
pub(crate) fn validate_agent_commission_batch_ids(ids: &[u64]) -> AppResult<Vec<u64>> {
    if ids.is_empty() {
        return Err(AppError::Validation(
            "at least one agent commission id is required".to_owned(),
        ));
    }
    if ids.len() > 200 {
        return Err(AppError::Validation(
            "a single batch cannot contain more than 200 agent commissions".to_owned(),
        ));
    }
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(*id) {
            return Err(AppError::Validation(
                "duplicate agent commission id in batch".to_owned(),
            ));
        }
    }
    Ok(ids.to_vec())
}

/// 复用代理域的产品类型白名单，返回佣金规则持久化使用的标准产品代码。
/// 后台不另立枚举而是直接委托代理上下文的归一实现，确保后台配置的产品代码与实际计佣时的匹配口径同源。
pub(crate) fn validate_agent_commission_rule_product_type(value: &str) -> AppResult<String> {
    crate::modules::agent::service::normalize_agent_commission_product_type(value)
}

/// 规范化佣金规则启停状态，仅接受 `active` 或 `disabled`。
/// 注意与佣金记录状态是两套完全不同的枚举：这里描述的是费率规则是否生效，
/// 停用规则只影响后续计佣，已按旧规则生成的佣金记录不受影响。
pub(crate) fn validate_agent_commission_rule_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "active" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "unsupported agent commission rule status".to_owned(),
        )),
    }
}

/// 校验代理佣金率处于闭区间 0..=1；不在此处执行金额计算或资产精度截断。
/// 费率以小数而非百分数表达，因此 1 代表全额返佣、0 代表不返佣，两个端点均视为合法配置。
pub(crate) fn validate_agent_commission_rate(value: &BigDecimal) -> AppResult<()> {
    if value < &BigDecimal::from(0) || value > &BigDecimal::from(1) {
        return Err(AppError::Validation(
            "commission_rate must be between 0 and 1".to_owned(),
        ));
    }
    Ok(())
}

/// 将代理佣金来源、计费基数、费率、金额、状态和创建时间映射为结算审计快照。
/// 快照不含钱包余额或支付流水；调用方在佣金状态事务内把它作为 before/after 写入后台审计。
pub(crate) fn agent_commission_audit_json(commission: &AdminAgentCommissionResponse) -> Value {
    json!({
        "id": commission.id,
        "agent_id": commission.agent_id,
        "user_id": commission.user_id,
        "source_type": commission.source_type,
        "source_id": commission.source_id,
        "source_amount": commission.source_amount,
        "commission_rate": commission.commission_rate,
        "commission_amount": commission.commission_amount,
        "status": commission.status,
        "created_at": commission.created_at.timestamp_millis(),
    })
}

/// 将代理佣金规则的代理、产品类型、费率、状态和时间戳映射为审计快照。
/// 映射不重新校验费率或状态；调用方负责在规则写事务中保存前后值。
pub(crate) fn agent_commission_rule_audit_json(rule: &AdminAgentCommissionRuleResponse) -> Value {
    json!({
        "id": rule.id,
        "agent_id": rule.agent_id,
        "product_type": rule.product_type,
        "commission_rate": rule.commission_rate,
        "status": rule.status,
        "created_at": rule.created_at.timestamp_millis(),
        "updated_at": rule.updated_at.timestamp_millis(),
    })
}

/// 校验新建代理的用户、父级、层级、代理码和门户账号凭据，并要求提供后台审计原因。
/// 这里只检查请求形状；用户唯一性、父链 active 状态和层级一致性由应用事务锁行后复核。
/// 父代理编号可以缺省表示创建顶级代理，但显式传 0 会被判为非法，避免把「无父级」误写成零值。
/// 门户口令允许提交明文或已散列值，二者至少有一个非空即可通过；层级若提供必须落在 1 到 3 之间。
pub(crate) fn validate_create_agent_request(request: &CreateAgentRequest) -> AppResult<()> {
    if request.user_id == 0 {
        return Err(AppError::Validation("user_id is required".to_owned()));
    }
    if optional_string(Some(request.agent_code.clone())).is_none() {
        return Err(AppError::Validation("agent_code is required".to_owned()));
    }
    if optional_string(Some(request.admin_username.clone())).is_none() {
        return Err(AppError::Validation(
            "admin_username is required".to_owned(),
        ));
    }
    if request.parent_agent_id == Some(0) {
        return Err(AppError::Validation(
            "parent_agent_id must be positive".to_owned(),
        ));
    }
    if optional_string(request.admin_password.clone()).is_none()
        && optional_string(request.admin_password_hash.clone()).is_none()
    {
        return Err(AppError::Validation(
            "admin_password is required".to_owned(),
        ));
    }
    if request.level.is_some_and(|level| !(1..=3).contains(&level)) {
        return Err(AppError::Validation(
            "level must be between 1 and 3".to_owned(),
        ));
    }
    Ok(())
}

/// 提取并散列新代理门户账号密码；创建代理时密码必须存在且满足认证域强度要求。
/// 优先使用 admin_password 现场散列，否则接受已提供的 admin_password_hash；两者均缺失返回校验错误，函数不持久化账号。
pub(crate) fn agent_password_hash(request: &CreateAgentRequest) -> AppResult<String> {
    if let Some(password) = optional_string(request.admin_password.clone()) {
        return hash_password(&password);
    }
    optional_string(request.admin_password_hash.clone())
        .ok_or_else(|| AppError::Validation("admin_password is required".to_owned()))
}

/// 代理后台账号改密沿用平台统一的 6-20 位口令策略，明文口令绝不进入审计快照。
/// 与创建代理时的口令处理不同，重置入口只接受明文并强制走强度校验，不允许直接提交已散列值，
/// 因此无法借重置绕过口令策略；口令缺失时报必填错误而不是沿用旧口令。
pub(crate) fn agent_admin_password_hash(password: Option<String>) -> AppResult<String> {
    hash_password(&validate_reset_password(&required_string(
        password, "password",
    )?)?)
}

/// 将代理和门户账号标识映射为改密审计 JSON，明确会话需重新登录且绝不记录密码。
/// 快照只含代理和门户账号身份及账号状态；改密用例负责把它与口令更新同事务写入审计。
pub(crate) fn agent_password_reset_audit_json(agent: &AdminAgentResponse) -> Value {
    json!({
        "agent_id": agent.id,
        "agent_code": agent.agent_code,
        "admin_user_id": agent.admin_user_id,
        "admin_username": agent.admin_username,
        "admin_status": agent.admin_status,
    })
}

/// 规范化代理目标状态，仅允许 `active`、`suspended` 或 `disabled`，供代理与门户账号同步迁移。
/// 三态之间不设迁移限制，任意方向切换都会被放行；只有 active 的代理才能被指派新用户或作为父级挂接下级。
pub(crate) fn validate_agent_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "active" | "suspended" | "disabled" => Ok(status),
        _ => Err(AppError::Validation("unsupported agent status".to_owned())),
    }
}

/// 将代理用户、层级路径、门户账号及状态映射为审计 JSON，不包含密码散列。
/// 统计计数和创建时间也会进入快照；映射不重查邀请树，应用事务负责持久化准确的锁后值。
/// 直属用户数、团队用户数与下级代理数三项是随时变动的聚合值，因此仅改状态的操作也可能让前后值出现差异。
/// 代理状态与门户账号状态分别记录，可据此看出状态变更是否已同步到登录账号一侧。
pub(crate) fn agent_audit_json(agent: &AdminAgentResponse) -> Value {
    json!({
        "id": agent.id,
        "user_id": agent.user_id,
        "email": agent.email,
        "parent_agent_id": agent.parent_agent_id,
        "parent_agent_code": agent.parent_agent_code,
        "root_agent_id": agent.root_agent_id,
        "root_agent_code": agent.root_agent_code,
        "agent_code": agent.agent_code,
        "level": agent.level,
        "path": agent.path,
        "status": agent.status,
        "direct_user_count": agent.direct_user_count,
        "team_user_count": agent.team_user_count,
        "child_agent_count": agent.child_agent_count,
        "admin_user_id": agent.admin_user_id,
        "admin_username": agent.admin_username,
        "admin_status": agent.admin_status,
        "created_at": agent.created_at.timestamp_millis(),
    })
}

/// 将用户的直接邀请人、根代理、邀请深度和路径映射为改派审计快照。
/// 快照不包含后代迁移明细；应用层须在归属树更新事务中保存 before/after。
pub(crate) fn user_referral_audit_json(referral: &AdminUserReferralResponse) -> Value {
    json!({
        "user_id": referral.user_id,
        "direct_inviter_id": referral.direct_inviter_id,
        "direct_inviter_type": referral.direct_inviter_type,
        "root_agent_id": referral.root_agent_id,
        "depth": referral.depth,
        "path": referral.path,
        "created_at": referral.created_at.timestamp_millis(),
    })
}
