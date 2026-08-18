//! 风控规则与全局安全策略的纯业务规则层。
//!
//! 风控规则侧只做最外层形状校验，规则配置 JSON 的内部语义交由 risk 上下文在运行时解释，
//! 因此这里既不解析阈值也不判断规则之间的优先级。安全策略侧通过逐个动作取策略来确认所有资金动作
//! 都能映射到受支持的验证方式。三个审计快照函数分别覆盖风控规则、安全策略和用户双因素设置，
//! 其中双因素快照刻意不含密钥与恢复材料，安全策略走序列化因而可能返回内部错误。

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ValidatedRiskRuleTarget {
    pub(crate) target_type: String,
    pub(crate) target_id: Option<String>,
}

/// 校验风控规则类型、目标范围、阈值、时间窗和启停状态等请求形状。
/// 对象范围只允许运行时真正传入的 global/user/pair/asset 四维；用户与交易对必须是正数 ID，
/// 资产使用大写符号而不是数据库 ID，以与钱包风控上下文的 scope value 保持一致。资源是否存在且处于 active
/// 由应用事务在写规则前锁行确认；本函数不读库、不解释 config_json 内部阈值。
pub(crate) fn validate_create_risk_rule(
    request: &CreateRiskRuleRequest,
) -> AppResult<ValidatedRiskRuleTarget> {
    if optional_string(Some(request.rule_type.clone())).is_none() {
        return Err(AppError::Validation("rule_type is required".to_owned()));
    }
    if request.config_json.is_null() {
        return Err(AppError::Validation("config_json is required".to_owned()));
    }

    let target_type = optional_string(Some(request.target_type.clone()))
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| AppError::Validation("target_type is required".to_owned()))?;
    let target_id = optional_string(request.target_id.clone());
    match target_type.as_str() {
        "global" => {
            if target_id.is_some() {
                return Err(AppError::Validation(
                    "global risk target must not include target_id".to_owned(),
                ));
            }
            Ok(ValidatedRiskRuleTarget {
                target_type,
                target_id: None,
            })
        }
        "user" | "pair" => {
            let normalized = target_id
                .as_deref()
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|value| *value > 0)
                .ok_or_else(|| {
                    AppError::Validation(
                        "user and pair risk targets require a positive target_id".to_owned(),
                    )
                })?;
            Ok(ValidatedRiskRuleTarget {
                target_type,
                target_id: Some(normalized.to_string()),
            })
        }
        "asset" => {
            let symbol = target_id
                .filter(|value| {
                    value
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric())
                })
                .ok_or_else(|| {
                    AppError::Validation(
                        "asset risk target requires an alphanumeric asset symbol".to_owned(),
                    )
                })?;
            Ok(ValidatedRiskRuleTarget {
                target_type,
                target_id: Some(symbol.to_ascii_uppercase()),
            })
        }
        _ => Err(AppError::Validation(
            "unsupported risk target_type".to_owned(),
        )),
    }
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
