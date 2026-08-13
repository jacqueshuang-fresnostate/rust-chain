//! 代理层级、门户账号与代理佣金的应用用例层。
//!
//! 写用例统一采用「开事务、按固定顺序加锁、写入、回读、写审计、提交」的编排，因此审计前后值必然同源。
//! 需要特别注意三处副作用边界：代理改密与用户封禁在事务提交后才撤销在线会话，属于不可回滚的后置动作；
//! 用户改派代理会连带重算全部后代的邀请路径与根代理归属；佣金结算会真实向代理所属用户的钱包入账。
//! 批量佣金处理刻意为每条记录开独立事务，以失败隔离换取整体原子性，因此可能出现部分成功。

use super::*;
use crate::modules::auth::domain::login_failure_key;

/// 按代理/用户/父级/根代理/层级、代理码、邮箱和状态筛选代理，并返回当前页与匹配总数。
/// 代理码和状态会去除空白，limit 裁剪到 1～100、offset 上限 100000；查询不加锁也不写审计。
/// 邮箱按原值下推而不做去空白，与代理码和状态的处理方式不同，因此带空格的邮箱会匹配不到记录。
/// 响应中的团队人数等聚合值由查询即时统计，不同页之间可能因并发变动而略有出入。
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

/// 按代理 ID 读取层级、团队统计和门户账号信息组成的后台代理详情。
/// 查询使用连接池且不加锁；代理不存在返回未找到，SQL 或聚合映射失败直接返回错误，不改变登录状态。
pub(crate) async fn get_admin_agent(
    pool: Option<Pool<MySql>>,
    agent_id: u64,
) -> AppResult<AdminAgentResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_agent_from_store(&pool, agent_id).await
}

/// 创建代理主记录和门户账号，完成层级放置后返回包含路径及账号状态的代理快照。
/// 调用方提供已鉴权管理员 ID；请求须含用户、代理码、门户用户名及密码，父代理存在时按其 ID 加锁并要求 active。
/// 事务依次确认用户、锁父代理、插入代理、回填根节点/路径、插入门户账号、回读并写审计；唯一键或任一步失败整体回滚。
/// 本用例没有请求幂等键，重复创建依赖数据库唯一约束报错；提交后不发布事件或操作外部会话。
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

/// 更新代理及其全部门户账号的目标状态，并返回同步后的代理快照。
/// 调用方提供管理员 ID；状态只接受 active、suspended 或 disabled，本函数不执行权限判断。
/// 事务先锁代理行，再更新代理主表、批量同步门户账号、回读并写 before/after 审计；记录缺失或 SQL 失败整体回滚。
/// 相同状态重放仍会执行更新并新增审计，不撤销现有登录会话。
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

/// 确认代理存在后分页读取其路径覆盖的团队用户，并返回用户列表与匹配总数。
/// 两次查询不共享事务快照，分页限制为 1～100、offset 最大 100000；代理缺失或任一查询失败返回错误。
/// 先做存在性检查是为了把「代理不存在」与「代理存在但暂无下级」区分开，前者返回未找到而后者返回空列表。
/// 团队范围按邀请路径前缀匹配，因此包含全部层级的下级而不只是直属用户。
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

/// 把用户改派到指定启用代理，并重算该用户及其后代邀请路径、深度和根代理归属。
/// 用户、目标代理、原邀请关系与审计同事务锁定和写入；任一步失败整体回滚，重复改派仍会产生审计。
/// 加锁顺序固定为先用户、再目标代理层级节点与代理主行、最后原邀请关系，以避免并发改派互相形成环等待。
/// 目标代理编号为 0 直接判为校验错误，代理存在但状态不是 active 则返回冲突，因此停用代理无法接收新用户。
/// 后代迁移只在该用户原本已有邀请关系时触发，首次建立归属的用户不存在需要重挂的下级。
/// 本用例不校验目标代理是否处于该用户自身的下级链路中，因此环形改派需由调用方自行避免。
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

/// 按代理、产品类型和状态筛选佣金规则，并返回费率规则当前页与匹配总数。
/// 产品类型和状态只去空白而不在此枚举校验，分页执行统一裁剪；查询不锁规则，也不触发佣金计算。
/// 不做枚举校验意味着传入未知产品类型会得到空结果而非报错，排查配置缺失时需注意区分这两种情况。
/// 返回的是当前生效的费率配置，不反映历史佣金实际使用过的费率，后者需查佣金记录本身。
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

/// 为指定代理创建产品佣金规则，并返回数据库保存的费率、状态和时间戳。
/// 请求须含有效代理 ID、受支持产品、0～1 费率和审计原因；缺省状态为 active，管理员权限由调用方保证。
/// 应用事务先确认代理存在，再插入规则、回读和写审计；唯一规则冲突或数据库失败整体回滚。
/// 本用例无幂等键，重复请求不会复用旧规则，也不立即结算历史佣金。
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

/// 局部更新代理佣金规则的费率或启停状态，并返回锁后写入的最终快照。
/// 请求必须提供审计原因；费率若出现须位于 0～1，状态若出现仅接受 active/disabled，空更新仍按底层 SQL 语义执行。
/// 事务先锁规则，再更新可选字段、回读并写 before/after 审计；记录缺失或任一步失败整体回滚。
/// 成功重放会再次产生审计，不追溯重算已经生成的佣金。
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

/// 按代理、用户、邮箱和状态筛选已生成佣金，并返回来源金额与佣金金额的分页结果和总数。
/// 状态仅去除空白，分页限制为 1～100 且 offset 最大 100000；查询不锁定待结算佣金或修改钱包。
/// 每条记录保留生成时的来源类型、来源单号和当时使用的费率，因此后续调整规则不会改变历史记录的口径。
/// 由于不加锁，列出的待处理佣金可能在响应送达前已被并发结算，批量操作时应以逐条返回的结果为准。
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

/// 校验单笔代理佣金目标状态并委托状态迁移用例，管理员身份与原因用于生成后台审计。
/// 底层事务会锁定佣金并在结算时同步余额和流水；非 pending、记录缺失或数据库失败会返回错误。
/// 本函数自身只做目标状态的枚举校验和连接池解析，真正的加锁、入账与审计都在被委托的共享实现里完成。
/// 审计原因在此为可选，与代理创建等入口强制要求原因的做法不同，因此结算记录可能没有文字说明。
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

/// 批量校验佣金编号和目标状态，并逐条调用单笔代理佣金状态用例汇总成功与失败结果。
/// 每条记录使用独立事务，单条失败不回滚其他结果；重放已处理佣金会得到冲突而不会重复入账。
/// 编号列表先整体校验非空、不超过 200 条且无重复，任一条不合法则整批拒绝，尚未开始处理。
/// 进入循环后按列表原顺序串行处理，失败项把错误文本原样收进结果条目，因此响应必然与请求条数一一对应。
/// 整体返回成功不代表全部处理成功，调用方必须逐条检查结果状态后再决定是否重试失败项。
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

/// 锁定待处理代理佣金并执行结算或拒绝；结算时把佣金金额记入代理用户钱包并更新状态。
/// 钱包余额、流水、状态与可选审计共用同一事务；仅允许从 pending 转移，重放不会二次入账。
/// 这是单笔与批量两条入口共用的实现，行锁与状态前置判断构成防重复入账的唯一屏障：
/// 并发请求中只有一个能拿到锁并看到 pending，另一个在锁释放后读到终态并返回冲突。
/// 管理员编号为可选，缺省时跳过审计写入，供无人工主体的内部调用路径复用。
/// 拒绝分支不触碰任何钱包，只推进状态，因此被拒佣金不会产生资金流水。
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

/// 在调用方事务内把一笔佣金真正打给代理所属用户的钱包可用余额，并写出同额流水。
/// 先按佣金来源解析出收款用户与结算资产，来源不支持返佣时把未找到改判为冲突，
/// 给出的信息是「该来源无法结算」而不是「佣金不存在」，避免误导排查方向。
/// 入账以佣金编号作为流水引用键，因此同一笔佣金重复入账会在流水层面留下可识别痕迹；
/// 但真正防重复的仍是调用方的行锁与 pending 状态判断，本函数自身不做幂等检查。
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
