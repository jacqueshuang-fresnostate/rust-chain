//! 全局安全策略与风控规则、风控事件的应用用例层。
//!
//! 安全策略是整体替换语义，且旧值在事务外读取、写入时不锁配置行，因此并发提交下审计的前值可能略陈旧。
//! 风控规则的创建与启停都在事务内连同审计一起提交，规则配置 JSON 原样落库，其语义由 risk 上下文运行时解释。
//! 本层不撤销任何在线会话、不刷新独立风控缓存，也不重放历史风控事件，配置收紧只对后续判定生效。

use super::*;

/// 读取登录二次验证、注册邀请、用户名登录、支付动作和第三方绑定组成的全局用户安全策略。
/// 查询不加配置锁；底层可按既有缺省语义返回策略，连接池或 JSON 解码失败返回错误，不读取单个用户设置。
pub(crate) async fn get_admin_security_policy(
    pool: Option<Pool<MySql>>,
) -> AppResult<UserSecurityPolicy> {
    let pool = admin_mysql_pool(pool)?;
    load_security_policy(&pool).await
}

/// 替换全局用户安全策略，并返回本次请求构造的登录、注册、支付和第三方绑定配置。
/// 请求须提供审计原因；调用方负责管理员权限，当前支付策略结构校验不会读取任何用户安全状态。
/// 旧策略在事务外读取，随后事务写入新策略和 before/after 审计但不锁配置行；数据库或序列化失败会回滚写入。
/// 并发更新可能以较早读取值作为审计 before；提交后不主动撤销会话，也不重算既有用户设置。
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

/// 按规则类型、目标类型和 enabled 标记筛选风控规则，并返回原始配置 JSON 的分页结果与总数。
/// 文本筛选只去空白，分页统一裁剪；查询不解析规则配置或锁定规则，也不执行风险评估。
/// 配置 JSON 原样回传而不做结构解析，因此后台能查看当前服务版本尚未识别的新字段，便于灰度期间排查。
/// 规则类型与目标类型不做枚举校验，传入未知值得到的是空结果而非报错。
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

/// 创建一条可立即生效的后台风控规则，并返回持久化后的规则内容。
/// 调用方须已完成管理员鉴权；规则类型、目标范围和 JSON 配置必须先通过结构校验。
/// 规则插入、事务内回读与管理员审计原子提交，避免风控已生效但操作来源不可追溯。
/// 本用例没有业务幂等键；失败全部回滚，重复成功请求可能创建语义相同的多条规则。
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

/// 切换单条风控规则的 enabled 标记，并返回包含原始配置的最终规则快照。
/// 调用方提供管理员 ID；实现不校验显式审计原因，也不解析规则 config_json 或检查目标资源。
/// 事务先锁规则，再更新启用位、回读并写 before/after 审计；记录缺失或 SQL 失败整体回滚。
/// 相同值重放仍新增审计，提交后不主动刷新独立风控缓存或重放历史事件。
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

/// 按用户、邮箱、决策和风险等级筛选风控事件，并返回命中规则与详情的分页记录和总数。
/// 决策和等级仅去空白，查询不锁事件或重新评分；读取审计型事件不会产生新的事件或后台审计。
/// 事件记录的是判定发生当时命中的规则与依据，因此事后停用或修改规则都不会改写已产生的历史事件。
/// 该入口只用于复盘拦截原因，无法据此撤销某次拦截，放行需要通过调整规则或人工处理另行完成。
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
