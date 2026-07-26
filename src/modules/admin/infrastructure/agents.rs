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

pub(crate) async fn list_admin_agents(
    pool: &Pool<MySql>,
    filter: AdminAgentListFilter,
) -> AppResult<Vec<AdminAgentResponse>> {
    let mut builder = admin_agent_query();
    builder.push(" WHERE 1 = 1");
    if let Some(agent_id) = filter.agent_id {
        builder.push(" AND agents.id = ");
        builder.push_bind(agent_id);
    }
    if let Some(user_id) = filter.user_id {
        push_user_id_filter(&mut builder, "agents.user_id", user_id);
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
    if let Some(agent_code) = filter.agent_code {
        builder.push(" AND agents.agent_code = ");
        builder.push_bind(agent_code);
    }
    push_user_email_filter(&mut builder, "agents.user_id", filter.email);
    if let Some(status) = filter.status {
        builder.push(" AND agents.status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY agents.id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);
    builder.push(" OFFSET ");
    builder.push_bind(filter.offset as i64);

    Ok(builder
        .build_query_as::<AdminAgentResponse>()
        .fetch_all(pool)
        .await?)
}

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

pub(crate) async fn list_admin_agent_users(
    pool: &Pool<MySql>,
    agent_id: u64,
    limit: u32,
) -> AppResult<Vec<AdminAgentUserResponse>> {
    Ok(sqlx::query_as::<_, AdminAgentUserResponse>(
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
           LIMIT ?"#,
    )
    .bind(agent_id)
    .bind(limit as i64)
    .fetch_all(pool)
    .await?)
}

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

pub(crate) async fn list_admin_agent_commission_rules(
    pool: &Pool<MySql>,
    filter: AdminAgentCommissionRuleListFilter,
) -> AppResult<Vec<AdminAgentCommissionRuleResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, agent_id, product_type, commission_rate, status, created_at, updated_at
           FROM agent_commission_rules
           WHERE 1 = 1"#,
    );
    if let Some(agent_id) = filter.agent_id {
        builder.push(" AND agent_id = ");
        builder.push_bind(agent_id);
    }
    if let Some(product_type) = filter.product_type {
        builder.push(" AND product_type = ");
        builder.push_bind(product_type);
    }
    if let Some(status) = filter.status {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);
    builder.push(" OFFSET ");
    builder.push_bind(filter.offset as i64);

    Ok(builder
        .build_query_as::<AdminAgentCommissionRuleResponse>()
        .fetch_all(pool)
        .await?)
}

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

pub(crate) async fn list_admin_agent_commissions(
    pool: &Pool<MySql>,
    filter: AdminAgentCommissionListFilter,
) -> AppResult<Vec<AdminAgentCommissionResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, agent_id, user_id, source_type, source_id, source_amount, payout_asset_id,
                  commission_rate, commission_amount, status, created_at
           FROM agent_commission_records
           WHERE 1 = 1"#,
    );
    if let Some(agent_id) = filter.agent_id {
        builder.push(" AND agent_id = ");
        builder.push_bind(agent_id);
    }
    if let Some(user_id) = filter.user_id {
        push_user_id_filter(&mut builder, "user_id", user_id);
    }
    push_user_email_filter(&mut builder, "user_id", filter.email);
    if let Some(status) = filter.status {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);

    Ok(builder
        .build_query_as::<AdminAgentCommissionResponse>()
        .fetch_all(pool)
        .await?)
}

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

fn map_duplicate_agent_error(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("agent already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}
