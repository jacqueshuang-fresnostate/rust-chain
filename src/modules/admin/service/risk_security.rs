use super::*;

pub(crate) fn validate_create_risk_rule(request: &CreateRiskRuleRequest) -> AppResult<()> {
    if optional_string(Some(request.rule_type.clone())).is_none() {
        return Err(AppError::Validation("rule_type is required".to_owned()));
    }
    if optional_string(Some(request.target_type.clone())).is_none() {
        return Err(AppError::Validation("target_type is required".to_owned()));
    }
    if request.config_json.is_null() {
        return Err(AppError::Validation("config_json is required".to_owned()));
    }
    Ok(())
}

pub(crate) fn risk_rule_audit_json(rule: &RiskRuleResponse) -> Value {
    json!({
        "id": rule.id,
        "rule_type": rule.rule_type,
        "target_type": rule.target_type,
        "target_id": rule.target_id,
        "config_json": rule.config_json.0,
        "enabled": rule.enabled,
        "created_by": rule.created_by,
    })
}

pub(crate) fn validate_security_policy(policies: &PaymentPolicies) -> AppResult<()> {
    let _ = policies.policy_for(SecurityAction::Withdraw);
    let _ = policies.policy_for(SecurityAction::SpotOrder);
    let _ = policies.policy_for(SecurityAction::Convert);
    let _ = policies.policy_for(SecurityAction::EarnSubscribe);
    Ok(())
}

pub(crate) fn security_policy_audit_json(policy: &UserSecurityPolicy) -> AppResult<Value> {
    serde_json::to_value(policy).map_err(|error| {
        AppError::Internal(format!("failed to serialize security policy: {error}"))
    })
}

pub(crate) fn two_factor_audit_json(settings: &UserTwoFactorSettings) -> Value {
    json!({
        "user_id": settings.user_id,
        "totp_enabled": settings.totp_enabled,
        "login_2fa_enabled": settings.login_2fa_enabled,
        "confirmed_at": settings.confirmed_at.map(|value| value.timestamp_millis()),
        "last_verified_at": settings.last_verified_at.map(|value| value.timestamp_millis()),
    })
}
