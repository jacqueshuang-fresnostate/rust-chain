use super::*;

pub(crate) async fn get_admin_security_policy(
    pool: Option<Pool<MySql>>,
) -> AppResult<UserSecurityPolicy> {
    let pool = admin_mysql_pool(pool)?;
    load_security_policy(&pool).await
}

pub(crate) async fn update_admin_security_policy(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: UpdateSecurityPolicyRequest,
) -> AppResult<UserSecurityPolicy> {
    let reason = required_admin_audit_reason(request.reason)?;
    validate_security_policy(&request.payment_policies)?;
    let after = UserSecurityPolicy {
        login_2fa_mode: request.login_2fa_mode,
        registration_invite_required: request.registration_invite_required,
        username_login_enabled: request.username_login_enabled,
        payment_policies: request.payment_policies,
        third_party_bindings: request.third_party_bindings,
    };
    let pool = admin_mysql_pool(pool)?;
    let before = load_security_policy(&pool).await?;

    // 安全策略配置和后台审计必须同事务提交，避免策略变更缺少可追溯记录。
    let mut tx = pool.begin().await?;
    save_admin_security_policy_in_tx(&mut tx, &after, admin_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "security_policy.update",
            target_type: "security_policy",
            target_id: 0,
            before_json: Some(security_policy_audit_json(&before)?),
            after_json: Some(security_policy_audit_json(&after)?),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn list_admin_risk_rules(
    pool: Option<Pool<MySql>>,
    query: AdminRiskRuleQuery,
) -> AppResult<RiskRulesResponse> {
    let pool = admin_mysql_pool(pool)?;
    let (rules, total) = list_admin_risk_rules_from_store(
        &pool,
        AdminRiskRuleListFilter {
            rule_type: query.rule_type.and_then(optional_string),
            target_type: query.target_type.and_then(optional_string),
            enabled: query.enabled,
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(RiskRulesResponse { rules, total })
}

pub(crate) async fn create_admin_risk_rule(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateRiskRuleRequest,
) -> AppResult<RiskRuleResponse> {
    validate_create_risk_rule(&request)?;
    let CreateRiskRuleRequest {
        rule_type,
        target_type,
        target_id,
        config_json,
        enabled,
        reason,
    } = request;
    let rule_type = optional_string(rule_type).expect("risk rule type validated");
    let target_type = optional_string(target_type).expect("risk target type validated");
    let target_id = target_id.and_then(optional_string);
    let pool = admin_mysql_pool(pool)?;

    // 风控规则变更和后台审计必须同事务提交，避免规则已生效但操作来源不可追踪。
    let mut tx = pool.begin().await?;
    let rule_id = insert_risk_rule_in_tx(
        &mut tx,
        RiskRuleWrite {
            rule_type,
            target_type,
            target_id,
            config_json,
            enabled: enabled.unwrap_or(true),
            created_by: admin_id,
        },
    )
    .await?;
    let rule = load_risk_rule_in_tx(&mut tx, rule_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "risk_rule.create",
            target_type: "risk_rule",
            target_id: rule_id,
            before_json: None,
            after_json: Some(risk_rule_audit_json(&rule)),
            reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(rule)
}

pub(crate) async fn update_admin_risk_rule_status(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    rule_id: u64,
    request: UpdateRiskRuleStatusRequest,
) -> AppResult<RiskRuleResponse> {
    let pool = admin_mysql_pool(pool)?;

    // 先锁定旧规则再更新状态，确保审计 before/after 对应同一次状态切换。
    let mut tx = pool.begin().await?;
    let before = lock_risk_rule_in_tx(&mut tx, rule_id).await?;
    update_risk_rule_status_in_tx(&mut tx, rule_id, request.enabled).await?;
    let after = load_risk_rule_in_tx(&mut tx, rule_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "risk_rule.status.update",
            target_type: "risk_rule",
            target_id: rule_id,
            before_json: Some(risk_rule_audit_json(&before)),
            after_json: Some(risk_rule_audit_json(&after)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn list_admin_risk_events(
    pool: Option<Pool<MySql>>,
    query: AdminRiskEventQuery,
) -> AppResult<RiskEventsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let (events, total) = list_admin_risk_events_from_store(
        &pool,
        AdminRiskEventListFilter {
            user_id: query.user_id,
            email: query.email,
            decision: query.decision.and_then(optional_string),
            risk_level: query.risk_level.and_then(optional_string),
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(RiskEventsResponse { events, total })
}
