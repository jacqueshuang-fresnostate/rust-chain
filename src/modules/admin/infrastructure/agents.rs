use super::*;

#[derive(Debug)]
pub(crate) struct AdminAgentListFilter {
    pub(crate) agent_id: Option<u64>,
    pub(crate) user_id: Option<u64>,
    pub(crate) parent_agent_id: Option<u64>,
    pub(crate) root_agent_id: Option<u64>,
    pub(crate) level: Option<i32>,
    pub(crate) agent_code: Option<String>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug, sqlx::FromRow)]
struct AgentHierarchyNodeRow {
    id: u64,
    parent_agent_id: Option<u64>,
    root_agent_id: Option<u64>,
    level: i32,
    path: String,
    status: String,
}

impl TryFrom<AgentHierarchyNodeRow> for AgentHierarchyNode {
    type Error = AppError;

    /// 将代理层级数据库行转换为领域节点，要求根代理 ID 已回填且 path 非空。
    /// 缺失层级字段返回冲突而不是构造半初始化节点；转换不查询数据库或修改原行。
    fn try_from(row: AgentHierarchyNodeRow) -> Result<Self, Self::Error> {
        let root_agent_id = row.root_agent_id.ok_or_else(|| {
            AppError::Conflict("agent root hierarchy is not initialized".to_owned())
        })?;
        if row.path.is_empty() {
            return Err(AppError::Conflict(
                "agent path hierarchy is not initialized".to_owned(),
            ));
        }
        Ok(Self {
            id: row.id,
            parent_agent_id: row.parent_agent_id,
            root_agent_id,
            level: row.level,
            path: row.path,
            status: row.status,
        })
    }
}

#[derive(Debug)]
pub(crate) struct AdminAgentCommissionListFilter {
    pub(crate) agent_id: Option<u64>,
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug)]
pub(crate) struct AdminAgentCommissionRuleListFilter {
    pub(crate) agent_id: Option<u64>,
    pub(crate) product_type: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug)]
pub(crate) struct AdminAgentCommissionRuleWrite {
    pub(crate) agent_id: u64,
    pub(crate) product_type: String,
    pub(crate) commission_rate: BigDecimal,
    pub(crate) status: String,
}

/// 按代理、用户、层级、代理码、邮箱和状态分页查询代理，并返回团队计数及首个门户账号信息。
/// 列表与 COUNT 复用同组谓词、按代理 ID 倒序；连接池查询不加锁，SQL 或聚合映射失败直接返回错误。
pub(crate) async fn list_admin_agents(
    pool: &Pool<MySql>,
    filter: AdminAgentListFilter,
) -> AppResult<(Vec<AdminAgentResponse>, i64)> {
    let mut rows = admin_agent_query();
    let mut total = admin_agent_count_query();
    for builder in [&mut rows, &mut total] {
        push_admin_agent_filters(builder, &filter);
    }

    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY agents.id DESC",
        filter.limit,
        filter.offset,
    )
    .await
}

fn push_admin_agent_filters(builder: &mut QueryBuilder<'_, MySql>, filter: &AdminAgentListFilter) {
    builder.push(" WHERE 1 = 1");
    if let Some(agent_id) = filter.agent_id {
        builder.push(" AND agents.id = ");
        builder.push_bind(agent_id);
    }
    if let Some(user_id) = filter.user_id {
        push_user_id_filter(builder, "agents.user_id", user_id);
    }
    if let Some(parent_agent_id) = filter.parent_agent_id {
        builder.push(" AND agents.parent_agent_id = ");
        builder.push_bind(parent_agent_id);
    }
    if let Some(root_agent_id) = filter.root_agent_id {
        builder.push(" AND COALESCE(agents.root_agent_id, agents.id) = ");
        builder.push_bind(root_agent_id);
    }
    if let Some(level) = filter.level {
        builder.push(" AND agents.level = ");
        builder.push_bind(level);
    }
    if let Some(agent_code) = filter.agent_code.clone() {
        builder.push(" AND agents.agent_code = ");
        builder.push_bind(agent_code);
    }
    push_user_email_filter(builder, "agents.user_id", filter.email.clone());
    if let Some(status) = filter.status.clone() {
        builder.push(" AND agents.status = ");
        builder.push_bind(status);
    }
}

/// 按代理 ID 读取层级、团队计数及最早创建的门户账号，返回单个后台代理响应。
/// 查询通过连接池执行且不加锁；无记录返回未找到，任一聚合或行映射失败返回数据库错误。
pub(crate) async fn load_admin_agent(
    pool: &Pool<MySql>,
    agent_id: u64,
) -> AppResult<AdminAgentResponse> {
    let mut builder = admin_agent_query();
    builder.push(" WHERE agents.id = ");
    builder.push_bind(agent_id);
    builder.push(" ORDER BY agent_admin_users.id ASC LIMIT 1");
    builder
        .build_query_as::<AdminAgentResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

/// 在调用方事务中插入 active 代理主记录，初始 path 为空并返回自增代理 ID。
/// user/父级/根级/代码/层级来自已校验写契约；重复代理映射为冲突，层级回填和审计须由调用方继续完成。
pub(crate) async fn insert_admin_agent_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminAgentWrite,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO agents
              (user_id, parent_agent_id, root_agent_id, agent_code, level, path, status)
           VALUES (?, ?, ?, ?, ?, '', 'active')"#,
    )
    .bind(input.user_id)
    .bind(input.parent_agent_id)
    .bind(input.root_agent_id)
    .bind(input.agent_code)
    .bind(input.level)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_agent_error)?;
    Ok(result.last_insert_id())
}

/// 在调用方事务中回填新代理的根代理 ID 和完整层级路径。
/// SQL 必须恰好更新一行，否则返回并发冲突；函数不提交事务，也不创建门户账号或审计记录。
pub(crate) async fn finalize_admin_agent_hierarchy_in_tx(
    tx: &mut Transaction<'_, MySql>,
    agent_id: u64,
    root_agent_id: u64,
    path: &str,
) -> AppResult<()> {
    let result = sqlx::query("UPDATE agents SET root_agent_id = ?, path = ? WHERE id = ?")
        .bind(root_agent_id)
        .bind(path)
        .bind(agent_id)
        .execute(&mut **tx)
        .await?;
    if result.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "agent hierarchy initialization changed concurrently".to_owned(),
        ));
    }
    Ok(())
}

/// 锁定目标父代理及其整条祖先链，并返回已初始化的 active 层级节点。
/// 先按目标 ID `FOR UPDATE`，再按 level、ID 升序锁定 path 上的祖先；任一节点非 active、层级未初始化或记录缺失即失败。
pub(crate) async fn lock_active_agent_hierarchy_node_in_tx(
    tx: &mut Transaction<'_, MySql>,
    agent_id: u64,
) -> AppResult<AgentHierarchyNode> {
    let row = sqlx::query_as::<_, AgentHierarchyNodeRow>(
        r#"SELECT id, parent_agent_id, root_agent_id, level, path, status
           FROM agents
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(agent_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    let node = AgentHierarchyNode::try_from(row)?;

    // 创建下级代理时锁定整条祖先链，避免父级并发停用后仍创建出可登录账号。
    let ancestor_statuses = sqlx::query_scalar::<_, String>(
        r#"SELECT status
           FROM agents
           WHERE path = ? OR ? LIKE CONCAT(path, '/%')
           ORDER BY level ASC, id ASC
           FOR UPDATE"#,
    )
    .bind(&node.path)
    .bind(&node.path)
    .fetch_all(&mut **tx)
    .await?;
    if ancestor_statuses.is_empty() || ancestor_statuses.iter().any(|status| status != "active") {
        return Err(AppError::Conflict(
            "parent agent hierarchy must be active".to_owned(),
        ));
    }
    Ok(node)
}

/// 在调用方事务中为代理插入 active 门户账号，并返回账号自增 ID。
/// 用户名和密码散列必须已由应用层校验；唯一键冲突映射为代理已存在，函数不提交或记录明文密码。
pub(crate) async fn insert_agent_admin_user_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminAgentAdminUserWrite,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO agent_admin_users (agent_id, username, password_hash, status)
           VALUES (?, ?, ?, 'active')"#,
    )
    .bind(input.agent_id)
    .bind(input.username)
    .bind(input.password_hash)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_agent_error)?;
    Ok(result.last_insert_id())
}

/// 在调用方事务快照中按代理 ID 回读层级、团队计数和首个门户账号响应，但不追加 `FOR UPDATE`。
/// 无记录返回未找到；该读取供写后响应和审计使用，不提交事务或改变账号状态。
pub(crate) async fn load_admin_agent_in_tx(
    tx: &mut Transaction<'_, MySql>,
    agent_id: u64,
) -> AppResult<AdminAgentResponse> {
    let mut builder = admin_agent_query();
    builder.push(" WHERE agents.id = ");
    builder.push_bind(agent_id);
    builder.push(" ORDER BY agent_admin_users.id ASC LIMIT 1");
    builder
        .build_query_as::<AdminAgentResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

/// 以代理详情查询附加 `FOR UPDATE` 锁定指定代理的当前响应快照。
/// 无记录返回未找到；查询包含门户账号和统计关联，调用方应先锁代理再执行后续账号更新并负责提交。
pub(crate) async fn lock_admin_agent_in_tx(
    tx: &mut Transaction<'_, MySql>,
    agent_id: u64,
) -> AppResult<AdminAgentResponse> {
    let mut builder = admin_agent_query();
    builder.push(" WHERE agents.id = ");
    builder.push_bind(agent_id);
    builder.push(" ORDER BY agent_admin_users.id ASC LIMIT 1 FOR UPDATE");
    builder
        .build_query_as::<AdminAgentResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

/// 在调用方事务中按代理 ID 覆盖主表状态。
/// 本函数不校验状态枚举也不检查受影响行数；调用方须先锁定代理，并继续同步门户账号和写审计。
pub(crate) async fn update_admin_agent_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    agent_id: u64,
    status: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE agents SET status = ? WHERE id = ?")
        .bind(status)
        .bind(agent_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 在调用方事务中把指定代理的全部门户账号状态批量覆盖为目标值。
/// SQL 允许更新零行或多行且不校验数量；调用方负责先更新代理主状态、统一提交并记录审计。
pub(crate) async fn update_agent_admin_users_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    agent_id: u64,
    status: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE agent_admin_users SET status = ? WHERE agent_id = ?")
        .bind(status)
        .bind(agent_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 按代理 path 分页读取该代理及所有下级代理归属的团队用户，并返回同范围总数。
/// 列表按代理层级、邀请深度和用户 ID 升序；两条连接池查询不加锁，期间并发改派可能使列表与总数处于不同快照。
pub(crate) async fn list_admin_agent_users(
    pool: &Pool<MySql>,
    agent_id: u64,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<AdminAgentUserResponse>, i64)> {
    let users = sqlx::query_as::<_, AdminAgentUserResponse>(
        r#"SELECT users.id AS user_id, users.email, users.phone, users.status, users.kyc_level,
                  owner_agents.id AS owner_agent_id, referrals.root_agent_id,
                  owner_agents.agent_code AS owner_agent_code,
                  owner_agents.level AS owner_agent_level,
                  referrals.direct_inviter_id, referrals.direct_inviter_type,
                  referrals.depth, referrals.path, referrals.created_at AS referred_at
           FROM user_referrals referrals
           INNER JOIN users ON users.id = referrals.user_id
           INNER JOIN agents owner_agents ON owner_agents.id = referrals.root_agent_id
           INNER JOIN agents scope_agent ON scope_agent.id = ?
           WHERE owner_agents.path = scope_agent.path
              OR owner_agents.path LIKE CONCAT(scope_agent.path, '/%')
           ORDER BY owner_agents.level ASC, referrals.depth ASC, users.id ASC
           LIMIT ? OFFSET ?"#,
    )
    .bind(agent_id)
    .bind(limit as i64)
    .bind(offset as i64)
    .fetch_all(pool)
    .await?;
    let total = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*)
           FROM user_referrals referrals
           INNER JOIN users ON users.id = referrals.user_id
           INNER JOIN agents owner_agents ON owner_agents.id = referrals.root_agent_id
           INNER JOIN agents scope_agent ON scope_agent.id = ?
           WHERE owner_agents.path = scope_agent.path
              OR owner_agents.path LIKE CONCAT(scope_agent.path, '/%')"#,
    )
    .bind(agent_id)
    .fetch_one(pool)
    .await?;

    Ok((users, total))
}

/// 按用户 ID `FOR UPDATE` 锁定其邀请归属，并以 Option 返回旧关系。
/// 从未归属的用户返回 None 而不是未找到；调用方负责随后 upsert、迁移后代和统一提交。
pub(crate) async fn lock_user_referral_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<Option<AdminUserReferralResponse>> {
    Ok(sqlx::query_as::<_, AdminUserReferralResponse>(
        r#"SELECT user_id, direct_inviter_id, direct_inviter_type,
                  root_agent_id, depth, path, created_at
           FROM user_referrals
           WHERE user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?)
}

/// 在调用方事务中按 user_id 新增或覆盖直接代理归属，将邀请类型固定为 agent、深度固定为 1。
/// 根代理与 path 使用应用层计算值；唯一键重放走 UPDATE，函数不迁移后代、不提交事务或写后台审计。
pub(crate) async fn upsert_user_agent_referral_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: UserAgentReferralWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO user_referrals
           (user_id, direct_inviter_id, direct_inviter_type, root_agent_id, depth, path)
           VALUES (?, ?, 'agent', ?, 1, ?)
           ON DUPLICATE KEY UPDATE direct_inviter_id = VALUES(direct_inviter_id),
                                   direct_inviter_type = VALUES(direct_inviter_type),
                                   root_agent_id = VALUES(root_agent_id),
                                   depth = VALUES(depth),
                                   path = VALUES(path)"#,
    )
    .bind(input.user_id)
    .bind(input.agent_id)
    .bind(input.agent_id)
    .bind(input.path)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在调用方事务中按用户 ID 回读直接邀请人、根代理、深度和 path，供改派后响应与审计使用。
/// 查询不追加行锁；无归属返回未找到，SQL 失败由外层事务回滚。
pub(crate) async fn load_user_referral_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<AdminUserReferralResponse> {
    sqlx::query_as::<_, AdminUserReferralResponse>(
        r#"SELECT user_id, direct_inviter_id, direct_inviter_type,
                  root_agent_id, depth, path, created_at
           FROM user_referrals
           WHERE user_id = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 把旧邀请 path 下且旧根代理匹配的后代批量迁到新根代理，并按深度差重写 path。
/// 更新排除被改派用户本身，使用旧 root 的 null-safe 比较避免误迁其他团队；允许无后代时更新零行，调用方负责审计。
pub(crate) async fn migrate_user_referral_descendants_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    old_path: &str,
    old_depth: i32,
    old_root_agent_id: Option<u64>,
    new_root_agent_id: u64,
    new_path: &str,
) -> AppResult<()> {
    // 使用旧 path 和旧 root_agent_id 同时定位子树，避免用户 id 与代理 id 前缀碰撞误迁移其他团队。
    sqlx::query(
        r#"UPDATE user_referrals
           SET root_agent_id = ?,
               depth = depth - ? + 1,
               path = CONCAT(?, SUBSTRING(path, CHAR_LENGTH(?) + 1))
           WHERE user_id <> ?
             AND path LIKE CONCAT(?, '/%')
             AND root_agent_id <=> ?"#,
    )
    .bind(new_root_agent_id)
    .bind(old_depth)
    .bind(new_path)
    .bind(old_path)
    .bind(user_id)
    .bind(old_path)
    .bind(old_root_agent_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 按代理、产品类型和状态分页读取佣金规则，并返回完全相同谓词的总数。
/// 结果按规则 ID 倒序，查询不加锁或解析费率业务含义；SQL/十进制映射失败直接返回错误。
pub(crate) async fn list_admin_agent_commission_rules(
    pool: &Pool<MySql>,
    filter: AdminAgentCommissionRuleListFilter,
) -> AppResult<(Vec<AdminAgentCommissionRuleResponse>, i64)> {
    let mut rows = QueryBuilder::<MySql>::new(
        r#"SELECT id, agent_id, product_type, commission_rate, status, created_at, updated_at
           FROM agent_commission_rules"#,
    );
    let mut total = QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM agent_commission_rules");
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(agent_id) = filter.agent_id {
            builder.push(" AND agent_id = ");
            builder.push_bind(agent_id);
        }
        if let Some(product_type) = filter.product_type.clone() {
            builder.push(" AND product_type = ");
            builder.push_bind(product_type);
        }
        if let Some(status) = filter.status.clone() {
            builder.push(" AND status = ");
            builder.push_bind(status);
        }
    }

    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY id DESC",
        filter.limit,
        filter.offset,
    )
    .await
}

/// 在调用方事务中插入代理、产品、费率和状态组成的佣金规则，并返回新规则 ID。
/// 函数不再次校验费率或代理存在性，也不映射唯一键错误；调用方负责同事务回读和写审计。
pub(crate) async fn insert_agent_commission_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminAgentCommissionRuleWrite,
) -> AppResult<u64> {
    let rule_id = sqlx::query(
        r#"INSERT INTO agent_commission_rules (agent_id, product_type, commission_rate, status)
           VALUES (?, ?, ?, ?)"#,
    )
    .bind(input.agent_id)
    .bind(&input.product_type)
    .bind(&input.commission_rate)
    .bind(&input.status)
    .execute(&mut **tx)
    .await?
    .last_insert_id();
    Ok(rule_id)
}

/// 在调用方事务中用 COALESCE 局部覆盖佣金规则费率和状态，None 表示保留原字段。
/// SQL 不检查受影响行数；调用方须先锁规则，随后回读并把前后值写入审计。
pub(crate) async fn update_agent_commission_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    rule_id: u64,
    commission_rate: Option<&BigDecimal>,
    status: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE agent_commission_rules
           SET commission_rate = COALESCE(?, commission_rate),
               status = COALESCE(?, status)
           WHERE id = ?"#,
    )
    .bind(commission_rate)
    .bind(status)
    .bind(rule_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在调用方事务中按规则 ID 回读代理、产品、费率、状态和时间戳，不追加行锁。
/// 无记录返回未找到；该函数不修改规则或计算佣金，读取失败交由外层回滚。
pub(crate) async fn load_agent_commission_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    rule_id: u64,
) -> AppResult<AdminAgentCommissionRuleResponse> {
    sqlx::query_as::<_, AdminAgentCommissionRuleResponse>(
        r#"SELECT id, agent_id, product_type, commission_rate, status, created_at, updated_at
           FROM agent_commission_rules
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(rule_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 按规则 ID `FOR UPDATE` 锁定代理佣金规则并返回更新前快照。
/// 无记录返回未找到；锁由调用方事务持有至提交，函数不校验目标费率或状态。
pub(crate) async fn lock_agent_commission_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    rule_id: u64,
) -> AppResult<AdminAgentCommissionRuleResponse> {
    sqlx::query_as::<_, AdminAgentCommissionRuleResponse>(
        r#"SELECT id, agent_id, product_type, commission_rate, status, created_at, updated_at
           FROM agent_commission_rules
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(rule_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 按代理、用户、邮箱和状态分页读取佣金来源、计费金额及待入账资产，并返回匹配总数。
/// 结果按记录 ID 倒序，列表与 COUNT 共享过滤谓词；查询不锁 pending 佣金或钱包。
pub(crate) async fn list_admin_agent_commissions(
    pool: &Pool<MySql>,
    filter: AdminAgentCommissionListFilter,
) -> AppResult<(Vec<AdminAgentCommissionResponse>, i64)> {
    let mut rows = QueryBuilder::<MySql>::new(
        r#"SELECT id, agent_id, user_id, source_type, source_id, source_amount, payout_asset_id,
                  commission_rate, commission_amount, status, created_at
           FROM agent_commission_records"#,
    );
    let mut total = QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM agent_commission_records");
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(agent_id) = filter.agent_id {
            builder.push(" AND agent_id = ");
            builder.push_bind(agent_id);
        }
        if let Some(user_id) = filter.user_id {
            push_user_id_filter(builder, "user_id", user_id);
        }
        push_user_email_filter(builder, "user_id", filter.email.clone());
        if let Some(status) = filter.status.clone() {
            builder.push(" AND status = ");
            builder.push_bind(status);
        }
    }

    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY id DESC",
        filter.limit,
        filter.offset,
    )
    .await
}

/// 在调用方事务中按佣金 ID 回读来源、费率、金额、入账资产和状态，不追加行锁。
/// 无记录返回未找到；读取仅供结算后响应或审计，SQL 失败由调用方回滚。
pub(crate) async fn load_agent_commission_in_tx(
    tx: &mut Transaction<'_, MySql>,
    commission_id: u64,
) -> AppResult<AdminAgentCommissionResponse> {
    sqlx::query_as::<_, AdminAgentCommissionResponse>(
        r#"SELECT id, agent_id, user_id, source_type, source_id, source_amount, payout_asset_id,
                  commission_rate, commission_amount, status, created_at
           FROM agent_commission_records
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(commission_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 按佣金 ID `FOR UPDATE` 锁定佣金记录并返回结算前快照。
/// 调用方据此限制 pending 状态并在需要时继续锁钱包；记录缺失返回未找到，锁随外层事务释放。
pub(crate) async fn lock_agent_commission_in_tx(
    tx: &mut Transaction<'_, MySql>,
    commission_id: u64,
) -> AppResult<AdminAgentCommissionResponse> {
    sqlx::query_as::<_, AdminAgentCommissionResponse>(
        r#"SELECT id, agent_id, user_id, source_type, source_id, source_amount, payout_asset_id,
                  commission_rate, commission_amount, status, created_at
           FROM agent_commission_records
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(commission_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 在调用方事务中按佣金 ID 覆盖结算状态。
/// 函数不校验 pending 前置状态或受影响行数；调用方须先锁佣金并把钱包入账、流水和审计一起提交。
pub(crate) async fn update_agent_commission_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    commission_id: u64,
    status: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE agent_commission_records SET status = ? WHERE id = ?")
        .bind(status)
        .bind(commission_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 在调用方事务中确认指定代理记录存在，阻止佣金规则关联到不存在的代理。
/// 查询使用 `FOR UPDATE` 持有代理行锁至事务结束；记录缺失返回未找到，但不会检查代理是否 active。
pub(crate) async fn ensure_agent_exists_in_tx(
    tx: &mut Transaction<'_, MySql>,
    agent_id: u64,
) -> AppResult<()> {
    sqlx::query_as::<_, (u64,)>("SELECT id FROM agents WHERE id = ? LIMIT 1 FOR UPDATE")
        .bind(agent_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(())
}

/// 连接佣金记录与代理主表，解析结算应入账的代理用户 ID 和 payout_asset_id。
/// 查询不加锁且要求佣金已设置入账资产；无匹配返回未找到，函数不创建钱包或写入余额。
pub(crate) async fn load_agent_commission_payout_target_in_tx(
    tx: &mut Transaction<'_, MySql>,
    commission_id: u64,
) -> AppResult<AgentCommissionPayoutTarget> {
    let target = sqlx::query_as::<_, (u64, u64)>(
        r#"SELECT agents.user_id AS agent_user_id, records.payout_asset_id AS asset_id
           FROM agent_commission_records records
           INNER JOIN agents ON agents.id = records.agent_id
           WHERE records.id = ? AND records.payout_asset_id IS NOT NULL
           LIMIT 1"#,
    )
    .bind(commission_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(AgentCommissionPayoutTarget {
        agent_user_id: target.0,
        asset_id: target.1,
    })
}

fn admin_agent_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT agents.id, agents.user_id, users.email,
                  agents.parent_agent_id, parent_agents.agent_code AS parent_agent_code,
                  COALESCE(agents.root_agent_id, agents.id) AS root_agent_id,
                  root_agents.agent_code AS root_agent_code,
                  agents.agent_code, agents.level, agents.path, agents.status,
                  (SELECT COUNT(*) FROM user_referrals direct_referrals
                   WHERE direct_referrals.root_agent_id = agents.id) AS direct_user_count,
                  (SELECT COUNT(*)
                   FROM user_referrals team_referrals
                   INNER JOIN agents owner_agents ON owner_agents.id = team_referrals.root_agent_id
                   WHERE owner_agents.path = agents.path
                      OR owner_agents.path LIKE CONCAT(agents.path, '/%')) AS team_user_count,
                  (SELECT COUNT(*) FROM agents child_agents
                   WHERE child_agents.parent_agent_id = agents.id) AS child_agent_count,
                  agent_admin_users.id AS admin_user_id,
                  agent_admin_users.username AS admin_username,
                  agent_admin_users.status AS admin_status,
                  agents.created_at
           FROM agents
           INNER JOIN users ON users.id = agents.user_id
           LEFT JOIN agents parent_agents ON parent_agents.id = agents.parent_agent_id
           INNER JOIN agents root_agents ON root_agents.id = COALESCE(agents.root_agent_id, agents.id)
           LEFT JOIN (
               SELECT agent_id, MIN(id) AS id
               FROM agent_admin_users
               GROUP BY agent_id
           ) first_agent_admin_users ON first_agent_admin_users.agent_id = agents.id
           LEFT JOIN agent_admin_users ON agent_admin_users.id = first_agent_admin_users.id"#,
    )
}

fn admin_agent_count_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT COUNT(*)
           FROM agents
           INNER JOIN users ON users.id = agents.user_id
           INNER JOIN agents root_agents ON root_agents.id = COALESCE(agents.root_agent_id, agents.id)"#,
    )
}

fn map_duplicate_agent_error(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("agent already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}
