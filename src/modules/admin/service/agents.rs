use super::*;

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

pub(crate) fn validate_agent_commission_rule_product_type(value: &str) -> AppResult<String> {
    crate::modules::agent::service::normalize_agent_commission_product_type(value)
}

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

pub(crate) fn validate_agent_commission_rate(value: &BigDecimal) -> AppResult<()> {
    if value < &BigDecimal::from(0) || value > &BigDecimal::from(1) {
        return Err(AppError::Validation(
            "commission_rate must be between 0 and 1".to_owned(),
        ));
    }
    Ok(())
}

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

pub(crate) fn agent_password_hash(request: &CreateAgentRequest) -> AppResult<String> {
    if let Some(password) = optional_string(request.admin_password.clone()) {
        return hash_password(&password);
    }
    optional_string(request.admin_password_hash.clone())
        .ok_or_else(|| AppError::Validation("admin_password is required".to_owned()))
}

/// 代理后台账号改密沿用平台统一的 6-20 位口令策略，明文口令绝不进入审计快照。
pub(crate) fn agent_admin_password_hash(password: Option<String>) -> AppResult<String> {
    hash_password(&validate_reset_password(&required_string(
        password, "password",
    )?)?)
}

pub(crate) fn agent_password_reset_audit_json(agent: &AdminAgentResponse) -> Value {
    json!({
        "agent_id": agent.id,
        "agent_code": agent.agent_code,
        "admin_user_id": agent.admin_user_id,
        "admin_username": agent.admin_username,
        "admin_status": agent.admin_status,
    })
}

pub(crate) fn validate_agent_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "active" | "suspended" | "disabled" => Ok(status),
        _ => Err(AppError::Validation("unsupported agent status".to_owned())),
    }
}

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
