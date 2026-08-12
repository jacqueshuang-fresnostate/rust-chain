use super::*;
use crate::modules::auth::domain::login_failure_key;

pub(crate) async fn list_admin_agents(
    pool: Option<Pool<MySql>>,
    query: AdminAgentQuery,
) -> AppResult<AdminAgentsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let (agents, total) = list_admin_agents_from_store(
        &pool,
        AdminAgentListFilter {
            agent_id: query.agent_id,
            user_id: query.user_id,
            parent_agent_id: query.parent_agent_id,
            root_agent_id: query.root_agent_id,
            level: query.level,
            agent_code: query.agent_code.and_then(optional_string),
            email: query.email,
            status: query.status.and_then(optional_string),
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(AdminAgentsResponse { agents, total })
}

pub(crate) async fn get_admin_agent(
    pool: Option<Pool<MySql>>,
    agent_id: u64,
) -> AppResult<AdminAgentResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_agent_from_store(&pool, agent_id).await
}

pub(crate) async fn create_admin_agent(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateAgentRequest,
) -> AppResult<AdminAgentResponse> {
    validate_create_agent_request(&request)?;
    let admin_password_hash = agent_password_hash(&request)?;
    let CreateAgentRequest {
        user_id,
        parent_agent_id,
        agent_code,
        admin_username,
        level,
        reason,
        ..
    } = request;
    let agent_code = optional_string(agent_code).expect("agent_code validated");
    let admin_username = optional_string(admin_username).expect("admin_username validated");
    let pool = admin_mysql_pool(pool)?;

    // 创建代理主表、代理后台账号和审计日志必须同事务提交，避免半成品代理账号。
    let mut tx = pool.begin().await?;
    ensure_admin_user_exists_in_tx(&mut tx, user_id).await?;
    let parent = match parent_agent_id {
        Some(parent_agent_id) => {
            Some(lock_active_agent_hierarchy_node_in_tx(&mut tx, parent_agent_id).await?)
        }
        None => None,
    };
    let placement = derive_agent_placement(parent.as_ref(), level)?;
    let agent_id = insert_admin_agent_in_tx(
        &mut tx,
        AdminAgentWrite {
            user_id,
            parent_agent_id: placement.parent_agent_id,
            root_agent_id: placement.root_agent_id,
            agent_code,
            level: placement.level,
        },
    )
    .await?;
    let root_agent_id = placement.root_agent_id.unwrap_or(agent_id);
    let hierarchy_path = agent_path(placement.path_prefix.as_deref(), agent_id);
    finalize_admin_agent_hierarchy_in_tx(&mut tx, agent_id, root_agent_id, &hierarchy_path).await?;
    insert_agent_admin_user_in_tx(
        &mut tx,
        AdminAgentAdminUserWrite {
            agent_id,
            username: admin_username,
            password_hash: admin_password_hash,
        },
    )
    .await?;
    let after = load_admin_agent_in_tx(&mut tx, agent_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "agent.create",
            target_type: "agent",
            target_id: agent_id,
            before_json: None,
            after_json: Some(agent_audit_json(&after)),
            reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn update_admin_agent_status(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    agent_id: u64,
    request: UpdateAgentStatusRequest,
) -> AppResult<AdminAgentResponse> {
    let status = validate_agent_status(&request.status)?;
    let pool = admin_mysql_pool(pool)?;

    // 锁定代理行后同步代理后台账号状态，确保审计 before/after 与实际可登录状态一致。
    let mut tx = pool.begin().await?;
    let before = lock_admin_agent_in_tx(&mut tx, agent_id).await?;
    update_admin_agent_status_in_tx(&mut tx, agent_id, &status).await?;
    update_agent_admin_users_status_in_tx(&mut tx, agent_id, &status).await?;
    let after = load_admin_agent_in_tx(&mut tx, agent_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "agent.status.update",
            target_type: "agent",
            target_id: agent_id,
            before_json: Some(agent_audit_json(&before)),
            after_json: Some(agent_audit_json(&after)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

/// 为已有门户账号的代理重置登录口令，并强制其重新登录。
/// 调用方须已完成管理员鉴权，且必须提供审计原因；未绑定门户账号的代理会冲突失败。
/// 事务内先锁定代理，再原子更新口令、撤销刷新令牌、清理失败计数并写入后台审计。
/// 提交后再撤销在线访问会话；该外部会话操作失败时数据库改密仍已生效，重试撤销是安全的。
pub(crate) async fn reset_admin_agent_password(
    state: AppState,
    admin_id: u64,
    agent_id: u64,
    request: ResetAgentPasswordRequest,
) -> AppResult<AdminAgentPasswordResetResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let password_hash = agent_admin_password_hash(request.password)?;
    let pool = admin_mysql_pool(state.mysql.clone())?;

    // 改密、吊销刷新令牌和审计同事务提交，避免泄露的旧口令或旧令牌在重置后仍可用。
    let mut tx = pool.begin().await?;
    let agent = lock_admin_agent_in_tx(&mut tx, agent_id).await?;
    let (Some(admin_user_id), Some(admin_username)) =
        (agent.admin_user_id, agent.admin_username.clone())
    else {
        return Err(AppError::Conflict(
            "agent has no portal account to reset".to_owned(),
        ));
    };
    update_agent_admin_password_in_tx_from_agent(&mut tx, admin_user_id, &password_hash).await?;
    revoke_agent_admin_refresh_tokens_in_tx_from_agent(&mut tx, admin_user_id).await?;
    // 一并清除登录失败计数：口令泄露后重置若仍受锁定窗口约束，代理最长要等 15 分钟才能恢复访问。
    sqlx::query("DELETE FROM login_failure_counters WHERE actor_type = 'agent' AND identifier = ?")
        .bind(login_failure_key(&admin_username))
        .execute(&mut *tx)
        .await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "agent_admin_user.password.reset",
            target_type: "agent_admin_user",
            target_id: admin_user_id,
            before_json: None,
            after_json: Some(agent_password_reset_audit_json(&agent)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;

    revoke_actor_auth_sessions(
        &state,
        &AuthActor::new(ActorType::Agent, admin_user_id, None),
    )
    .await?;
    Ok(AdminAgentPasswordResetResponse {
        agent_id,
        admin_user_id,
        admin_username,
        requires_relogin: true,
    })
}

pub(crate) async fn list_admin_agent_users(
    pool: Option<Pool<MySql>>,
    agent_id: u64,
    query: AdminAgentUsersQuery,
) -> AppResult<AdminAgentUsersResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_agent_from_store(&pool, agent_id).await?;
    let (users, total) = list_admin_agent_users_from_store(
        &pool,
        agent_id,
        route_limit(query.limit),
        route_offset(query.offset),
    )
    .await?;
    Ok(AdminAgentUsersResponse { users, total })
}

pub(crate) async fn assign_admin_user_agent(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    user_id: u64,
    request: AssignUserAgentRequest,
) -> AppResult<AdminUserReferralResponse> {
    if request.agent_id == 0 {
        return Err(AppError::Validation("agent_id is required".to_owned()));
    }
    let pool = admin_mysql_pool(pool)?;

    // 改派用户代理归属时同时锁定用户、代理和既有邀请关系，防止并发覆盖团队树。
    let mut tx = pool.begin().await?;
    ensure_admin_user_exists_in_tx(&mut tx, user_id).await?;
    lock_active_agent_hierarchy_node_in_tx(&mut tx, request.agent_id).await?;
    let agent = lock_admin_agent_in_tx(&mut tx, request.agent_id).await?;
    if agent.status != "active" {
        return Err(AppError::Conflict(
            "only active agents can receive assigned users".to_owned(),
        ));
    }
    let before = lock_user_referral_in_tx(&mut tx, user_id).await?;
    let previous_tree = before.as_ref().map(|referral| {
        (
            referral.path.clone(),
            referral.depth,
            referral.root_agent_id,
        )
    });
    let path = format!("/{}/{}/{}", request.agent_id, request.agent_id, user_id);
    upsert_user_agent_referral_in_tx(
        &mut tx,
        UserAgentReferralWrite {
            user_id,
            agent_id: request.agent_id,
            path: path.clone(),
        },
    )
    .await?;
    if let Some((old_path, old_depth, old_root_agent_id)) = previous_tree.as_ref() {
        migrate_user_referral_descendants_in_tx(
            &mut tx,
            user_id,
            old_path,
            *old_depth,
            *old_root_agent_id,
            request.agent_id,
            &path,
        )
        .await?;
    }
    let after = load_user_referral_in_tx(&mut tx, user_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "user_referral.assign_agent",
            target_type: "user_referral",
            target_id: user_id,
            before_json: before.as_ref().map(user_referral_audit_json),
            after_json: Some(user_referral_audit_json(&after)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn list_admin_agent_commission_rules(
    pool: Option<Pool<MySql>>,
    query: AdminAgentCommissionRuleQuery,
) -> AppResult<AdminAgentCommissionRulesResponse> {
    let product_type = query.product_type.and_then(optional_string);
    let status = query.status.and_then(optional_string);
    let pool = admin_mysql_pool(pool)?;
    let (rules, total) = list_admin_agent_commission_rules_from_store(
        &pool,
        AdminAgentCommissionRuleListFilter {
            agent_id: query.agent_id,
            product_type,
            status,
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(AdminAgentCommissionRulesResponse { rules, total })
}

pub(crate) async fn create_admin_agent_commission_rule(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateAgentCommissionRuleRequest,
) -> AppResult<AdminAgentCommissionRuleResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let product_type = validate_agent_commission_rule_product_type(&request.product_type)?;
    let status = request
        .status
        .as_deref()
        .map(validate_agent_commission_rule_status)
        .transpose()?
        .unwrap_or_else(|| "active".to_owned());
    validate_agent_commission_rate(&request.commission_rate)?;
    if request.agent_id == 0 {
        return Err(AppError::Validation("agent_id is required".to_owned()));
    }
    let pool = admin_mysql_pool(pool)?;

    // 代理存在性检查、佣金规则写入和后台审计必须同事务提交，避免孤立规则或缺失审计。
    let mut tx = pool.begin().await?;
    ensure_agent_exists_in_tx(&mut tx, request.agent_id).await?;
    let rule_id = insert_agent_commission_rule_in_tx(
        &mut tx,
        AdminAgentCommissionRuleWrite {
            agent_id: request.agent_id,
            product_type,
            commission_rate: request.commission_rate,
            status,
        },
    )
    .await?;
    let after = load_agent_commission_rule_in_tx(&mut tx, rule_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "agent_commission_rule.create",
            target_type: "agent_commission_rule",
            target_id: rule_id,
            before_json: None,
            after_json: Some(agent_commission_rule_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn update_admin_agent_commission_rule(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    rule_id: u64,
    request: UpdateAgentCommissionRuleRequest,
) -> AppResult<AdminAgentCommissionRuleResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let commission_rate = if let Some(commission_rate) = request.commission_rate {
        validate_agent_commission_rate(&commission_rate)?;
        Some(commission_rate)
    } else {
        None
    };
    let status = request
        .status
        .as_deref()
        .map(validate_agent_commission_rule_status)
        .transpose()?;
    let pool = admin_mysql_pool(pool)?;

    // 先锁定旧规则再更新，确保代理佣金规则审计 before/after 与本次事务一致。
    let mut tx = pool.begin().await?;
    let before = lock_agent_commission_rule_in_tx(&mut tx, rule_id).await?;
    update_agent_commission_rule_in_tx(
        &mut tx,
        rule_id,
        commission_rate.as_ref(),
        status.as_deref(),
    )
    .await?;
    let after = load_agent_commission_rule_in_tx(&mut tx, rule_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "agent_commission_rule.update",
            target_type: "agent_commission_rule",
            target_id: rule_id,
            before_json: Some(agent_commission_rule_audit_json(&before)),
            after_json: Some(agent_commission_rule_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn list_admin_agent_commissions(
    pool: Option<Pool<MySql>>,
    query: AdminAgentCommissionQuery,
) -> AppResult<AdminAgentCommissionsResponse> {
    let status = query.status.and_then(optional_string);
    let pool = admin_mysql_pool(pool)?;
    let (commissions, total) = list_admin_agent_commissions_from_store(
        &pool,
        AdminAgentCommissionListFilter {
            agent_id: query.agent_id,
            user_id: query.user_id,
            email: query.email,
            status,
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(AdminAgentCommissionsResponse { commissions, total })
}

pub(crate) async fn update_admin_agent_commission_status(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    commission_id: u64,
    request: UpdateAgentCommissionStatusRequest,
) -> AppResult<AdminAgentCommissionResponse> {
    let status = validate_agent_commission_status(&request.status)?;
    let pool = admin_mysql_pool(pool)?;
    apply_admin_agent_commission_status(
        &pool,
        Some(admin_id),
        commission_id,
        &status,
        request.reason,
    )
    .await
}

pub(crate) async fn update_admin_agent_commission_statuses(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: BatchUpdateAgentCommissionStatusRequest,
) -> AppResult<AdminAgentCommissionBatchStatusResponse> {
    let status = validate_agent_commission_status(&request.status)?;
    let ids = validate_agent_commission_batch_ids(&request.ids)?;
    let pool = admin_mysql_pool(pool)?;

    // 每条佣金独立事务处理，单条失败不影响其余记录的结算/拒绝。
    let mut results = Vec::with_capacity(ids.len());
    for commission_id in ids {
        let outcome = apply_admin_agent_commission_status(
            &pool,
            Some(admin_id),
            commission_id,
            &status,
            request.reason.clone(),
        )
        .await;
        results.push(match outcome {
            Ok(_) => AdminAgentCommissionBatchStatusItemResponse {
                id: commission_id,
                status: "ok".to_owned(),
                error: None,
            },
            Err(error) => AdminAgentCommissionBatchStatusItemResponse {
                id: commission_id,
                status: "failed".to_owned(),
                error: Some(error.to_string()),
            },
        });
    }
    Ok(AdminAgentCommissionBatchStatusResponse { results })
}

pub(crate) async fn apply_admin_agent_commission_status(
    pool: &Pool<MySql>,
    admin_id: Option<u64>,
    commission_id: u64,
    status: &str,
    reason: Option<String>,
) -> AppResult<AdminAgentCommissionResponse> {
    // 锁定佣金记录后只允许 pending 进入结算/拒绝，防止重复给代理钱包入账。
    let mut tx = pool.begin().await?;
    let before = lock_agent_commission_in_tx(&mut tx, commission_id).await?;
    if before.status != "pending" {
        return Err(AppError::Conflict(
            "agent commission status can only be updated from pending".to_owned(),
        ));
    }
    if status == "settled" {
        settle_agent_commission_payout_in_tx(&mut tx, &before).await?;
    }
    update_agent_commission_status_in_tx(&mut tx, commission_id, status).await?;
    let after = load_agent_commission_in_tx(&mut tx, commission_id).await?;
    if let Some(admin_id) = admin_id {
        insert_admin_audit_log_entry_in_tx(
            &mut tx,
            admin_id,
            AdminAuditLogEntry {
                action: "agent_commission.status.update",
                target_type: "agent_commission",
                target_id: commission_id,
                before_json: Some(agent_commission_audit_json(&before)),
                after_json: Some(agent_commission_audit_json(&after)),
                reason,
            },
        )
        .await?;
    }
    tx.commit().await?;
    Ok(after)
}

async fn settle_agent_commission_payout_in_tx(
    tx: &mut sqlx::Transaction<'_, MySql>,
    commission: &AdminAgentCommissionResponse,
) -> AppResult<()> {
    let target = load_agent_commission_payout_target_in_tx(tx, commission.id)
        .await
        .map_err(|error| match error {
            AppError::NotFound => AppError::Conflict(
                "agent commission source cannot be settled without payout support".to_owned(),
            ),
            other => other,
        })?;
    credit_admin_wallet_available_in_tx(
        tx,
        target.agent_user_id,
        target.asset_id,
        &commission.commission_amount,
        "agent_commission_payout",
        "agent_commission",
        &commission.id.to_string(),
    )
    .await
}
