use super::*;

/// 校验风控规则类型、目标范围、阈值、时间窗和启停状态等请求形状。
/// 不在此处解释规则优先级或读取当前策略；运行时合并语义由 risk 域负责。
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

/// 将风控规则类型、目标、原始配置、启用状态和创建人映射为审计快照。
/// 配置 JSON 会完整进入结果，调用方须避免在规则中保存密钥；应用层随规则变更写入审计。
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

/// 逐一解析提现、现货、闪兑、理财等资金动作的支付验证策略，确保所有动作都可映射为受支持方法。
/// 该纯校验不验证具体用户密码或 TOTP，也不写安全配置和管理员审计。
pub(crate) fn validate_security_policy(policies: &PaymentPolicies) -> AppResult<()> {
    let _ = policies.policy_for(SecurityAction::Withdraw);
    let _ = policies.policy_for(SecurityAction::SpotOrder);
    let _ = policies.policy_for(SecurityAction::Convert);
    let _ = policies.policy_for(SecurityAction::EarnSubscribe);
    Ok(())
}

/// 将完整用户安全策略序列化为后台审计 JSON，保留各支付动作的启用开关和验证方式。
/// 序列化失败返回内部错误；结果不含用户密码、TOTP 密钥或会话数据。
pub(crate) fn security_policy_audit_json(policy: &UserSecurityPolicy) -> AppResult<Value> {
    serde_json::to_value(policy).map_err(|error| {
        AppError::Internal(format!("failed to serialize security policy: {error}"))
    })
}

/// 将用户 TOTP、登录二次验证开关及确认/最近验证时间映射为重置审计快照。
/// 快照不包含 TOTP 密钥或恢复材料；用户安全事务负责保存前后值。
pub(crate) fn two_factor_audit_json(settings: &UserTwoFactorSettings) -> Value {
    json!({
        "user_id": settings.user_id,
        "totp_enabled": settings.totp_enabled,
        "login_2fa_enabled": settings.login_2fa_enabled,
        "confirmed_at": settings.confirmed_at.map(|value| value.timestamp_millis()),
        "last_verified_at": settings.last_verified_at.map(|value| value.timestamp_millis()),
    })
}
