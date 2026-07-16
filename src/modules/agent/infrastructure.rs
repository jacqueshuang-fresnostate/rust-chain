//! agent bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。

use crate::{
    architecture::InfrastructureLayer,
    error::{AppError, AppResult},
    modules::agent::{
        domain::{AgentCommissionRateTier, allocate_differential_agent_commissions},
        presentation::{
            AgentCommissionResponse, AgentDashboardResponse, AgentInviteCodeResponse,
            AgentMeResponse, AgentSubAgentResponse, AgentTeamTreeNodeResponse,
            AgentTeamUserResponse,
        },
        repository::{
            AgentAccessScope, AgentBusinessCommissionWrite, AgentCommissionRuleRecord,
            AgentConvertStatsRecord, AgentInviteCodeWrite,
        },
    },
};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool, Transaction};

#[derive(Debug)]
pub struct InfrastructureLayerMarker;

impl InfrastructureLayer for InfrastructureLayerMarker {}

pub(crate) async fn insert_agent_business_commission_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AgentBusinessCommissionWrite<'_>,
) -> AppResult<()> {
    if input.source_amount <= &BigDecimal::from(0) {
        return Ok(());
    }

    // 用户归属仍由 referral 的 owner agent 决定，业务只声明来源和发放资产。
    let rules = sqlx::query_as::<_, AgentCommissionRuleRecord>(
        r#"SELECT ancestor_agents.id AS agent_id, rules.commission_rate
           FROM user_referrals referrals
           INNER JOIN agents owner_agents ON owner_agents.id = referrals.root_agent_id
           INNER JOIN agents ancestor_agents
             ON owner_agents.path = ancestor_agents.path
             OR owner_agents.path LIKE CONCAT(ancestor_agents.path, '/%')
           INNER JOIN agent_commission_rules rules
             ON rules.id = (
                 SELECT candidate.id
                 FROM agent_commission_rules candidate
                 WHERE candidate.agent_id = ancestor_agents.id
                   AND candidate.product_type = ?
                   AND candidate.status = 'active'
                 ORDER BY candidate.id DESC
                 LIMIT 1
             )
           WHERE referrals.user_id = ? AND referrals.root_agent_id IS NOT NULL
             AND owner_agents.status = 'active'
             AND ancestor_agents.status = 'active'
           ORDER BY ancestor_agents.level DESC, ancestor_agents.id DESC"#,
    )
    .bind(input.product_type)
    .bind(input.user_id)
    .fetch_all(&mut **tx)
    .await?;
    if rules.is_empty() {
        return Ok(());
    }

    let (precision_scale,): (i32,) = sqlx::query_as(
        "SELECT precision_scale FROM assets WHERE id = ? AND status = 'active' LIMIT 1",
    )
    .bind(input.payout_asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    let tiers = rules
        .into_iter()
        .map(|rule| AgentCommissionRateTier {
            agent_id: rule.agent_id,
            cumulative_rate: rule.commission_rate,
        })
        .collect::<Vec<_>>();
    let allocations =
        allocate_differential_agent_commissions(&tiers, input.source_amount, precision_scale);

    for allocation in allocations {
        // 每一级都使用同一业务来源幂等，重放不能重复生成任何层级的返佣。
        sqlx::query(
            r#"INSERT INTO agent_commission_records
               (agent_id, user_id, source_type, source_id, source_amount, payout_asset_id,
                commission_rate, commission_amount, status)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'pending')
               ON DUPLICATE KEY UPDATE id = agent_commission_records.id"#,
        )
        .bind(allocation.agent_id)
        .bind(input.user_id)
        .bind(input.source_type)
        .bind(input.source_id)
        .bind(input.source_amount)
        .bind(input.payout_asset_id)
        .bind(allocation.commission_rate)
        .bind(allocation.commission_amount)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub(crate) async fn load_agent_me(
    pool: &Pool<MySql>,
    agent_admin_id: u64,
) -> AppResult<Option<AgentMeResponse>> {
    let agent = sqlx::query_as::<_, AgentMeResponse>(
        r#"SELECT agent_admins.id AS agent_admin_id,
                  agents.id AS agent_id,
                  agent_admins.username,
                  agents.agent_code,
                  agents.parent_agent_id,
                  COALESCE(agents.root_agent_id, agents.id) AS root_agent_id,
                  agents.level,
                  agents.path,
                  agents.status AS agent_status,
                  agent_admins.status AS admin_status,
                  agent_admins.last_login_at
           FROM agent_admin_users agent_admins
           INNER JOIN agents ON agents.id = agent_admins.agent_id
           WHERE agent_admins.id = ?
             AND agent_admins.status = 'active'
             AND agents.status = 'active'
             AND NOT EXISTS (
                 SELECT 1
                 FROM agents ancestors
                 WHERE (ancestors.path = agents.path
                    OR agents.path LIKE CONCAT(ancestors.path, '/%'))
                   AND ancestors.status <> 'active'
             )
           LIMIT 1"#,
    )
    .bind(agent_admin_id)
    .fetch_optional(pool)
    .await?;

    Ok(agent)
}

pub(crate) async fn load_agent_access_scope_for_admin(
    pool: &Pool<MySql>,
    agent_admin_id: u64,
) -> AppResult<Option<AgentAccessScope>> {
    let scope = sqlx::query_as::<_, AgentAccessScope>(
        r#"SELECT agents.id AS agent_id,
                  COALESCE(agents.root_agent_id, agents.id) AS root_agent_id,
                  agents.path
           FROM agent_admin_users agent_admins
           INNER JOIN agents ON agents.id = agent_admins.agent_id
           WHERE agent_admins.id = ?
             AND agent_admins.status = 'active'
             AND agents.status = 'active'
             AND NOT EXISTS (
                 SELECT 1
                 FROM agents ancestors
                 WHERE (ancestors.path = agents.path
                    OR agents.path LIKE CONCAT(ancestors.path, '/%'))
                   AND ancestors.status <> 'active'
             )
           LIMIT 1"#,
    )
    .bind(agent_admin_id)
    .fetch_optional(pool)
    .await?;

    Ok(scope)
}

pub(crate) async fn load_agent_dashboard(
    pool: &Pool<MySql>,
    scope: &AgentAccessScope,
) -> AppResult<AgentDashboardResponse> {
    let dashboard = sqlx::query_as::<_, AgentDashboardResponse>(
        r#"SELECT ? AS agent_id,
                  (SELECT COUNT(*)
                   FROM user_referrals team_referrals
                   INNER JOIN agents owner_agents
                     ON owner_agents.id = team_referrals.root_agent_id
                   WHERE owner_agents.path = ?
                      OR owner_agents.path LIKE CONCAT(?, '/%')) AS team_user_count,
                  (SELECT COUNT(*)
                   FROM invite_codes
                   WHERE owner_type = 'agent' AND owner_id = ? AND status = 'active')
                   AS active_invite_code_count,
                  COUNT(records.id) AS commission_record_count,
                  COALESCE(SUM(CASE WHEN records.status = 'pending'
                                    THEN records.commission_amount ELSE 0 END), 0)
                   AS pending_commission_amount,
                  COALESCE(SUM(CASE WHEN records.status = 'settled'
                                    THEN records.commission_amount ELSE 0 END), 0)
                   AS settled_commission_amount,
                  COALESCE(SUM(records.commission_amount), 0) AS total_commission_amount
           FROM agent_commission_records records
           INNER JOIN user_referrals referrals ON referrals.user_id = records.user_id
           INNER JOIN agents owner_agents ON owner_agents.id = referrals.root_agent_id
           WHERE records.agent_id = ?
             AND (owner_agents.path = ?
               OR owner_agents.path LIKE CONCAT(?, '/%'))"#,
    )
    .bind(scope.agent_id)
    .bind(&scope.path)
    .bind(&scope.path)
    .bind(scope.agent_id)
    .bind(scope.agent_id)
    .bind(&scope.path)
    .bind(&scope.path)
    .fetch_one(pool)
    .await?;

    Ok(dashboard)
}

pub(crate) async fn load_agent_convert_stats(
    pool: &Pool<MySql>,
    scope: &AgentAccessScope,
) -> AppResult<AgentConvertStatsRecord> {
    let row = sqlx::query_as::<_, AgentConvertStatsRecord>(
        r#"SELECT ? AS agent_id,
                  COUNT(orders.id) AS total_orders,
                  COALESCE(SUM(CASE WHEN orders.status = 'pending' THEN 1 ELSE 0 END), 0)
                   AS pending_orders,
                  COALESCE(SUM(CASE WHEN orders.status = 'completed' THEN 1 ELSE 0 END), 0)
                   AS completed_orders,
                  COALESCE(SUM(orders.from_amount), 0) AS total_from_amount,
                  COALESCE(SUM(orders.to_amount), 0) AS total_to_amount
           FROM convert_orders orders
           INNER JOIN user_referrals referrals ON referrals.user_id = orders.user_id
           INNER JOIN agents owner_agents ON owner_agents.id = referrals.root_agent_id
           WHERE owner_agents.path = ?
              OR owner_agents.path LIKE CONCAT(?, '/%')"#,
    )
    .bind(scope.agent_id)
    .bind(&scope.path)
    .bind(&scope.path)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

pub(crate) async fn list_agent_team_users(
    pool: &Pool<MySql>,
    scope: &AgentAccessScope,
) -> AppResult<Vec<AgentTeamUserResponse>> {
    let users = sqlx::query_as::<_, AgentTeamUserResponse>(
        r#"SELECT u.id AS user_id, u.email, u.phone, u.status, u.kyc_level,
                  owner_agents.id AS owner_agent_id, ur.root_agent_id,
                  owner_agents.agent_code AS owner_agent_code,
                  owner_agents.level AS owner_agent_level,
                  ur.direct_inviter_id, ur.direct_inviter_type,
                  ur.depth, ur.created_at AS referred_at
           FROM user_referrals ur
           INNER JOIN users u ON u.id = ur.user_id
           INNER JOIN agents owner_agents ON owner_agents.id = ur.root_agent_id
           WHERE owner_agents.path = ?
              OR owner_agents.path LIKE CONCAT(?, '/%')
           ORDER BY owner_agents.level ASC, u.id ASC
           LIMIT 100"#,
    )
    .bind(&scope.path)
    .bind(&scope.path)
    .fetch_all(pool)
    .await?;

    Ok(users)
}

pub(crate) async fn list_agent_team_tree_nodes(
    pool: &Pool<MySql>,
    scope: &AgentAccessScope,
) -> AppResult<Vec<AgentTeamTreeNodeResponse>> {
    let nodes = sqlx::query_as::<_, AgentTeamTreeNodeResponse>(
        r#"SELECT u.id AS user_id, u.email, u.phone, u.status,
                  ur.direct_inviter_id, ur.direct_inviter_type,
                  owner_agents.id AS owner_agent_id,
                  owner_agents.agent_code AS owner_agent_code,
                  owner_agents.level AS owner_agent_level,
                  ur.depth,
                  ur.path, ur.created_at AS referred_at
           FROM user_referrals ur
           INNER JOIN users u ON u.id = ur.user_id
           INNER JOIN agents owner_agents ON owner_agents.id = ur.root_agent_id
           WHERE owner_agents.path = ?
              OR owner_agents.path LIKE CONCAT(?, '/%')
           ORDER BY owner_agents.level ASC, ur.depth ASC, u.id ASC
           LIMIT 500"#,
    )
    .bind(&scope.path)
    .bind(&scope.path)
    .fetch_all(pool)
    .await?;

    Ok(nodes)
}

pub(crate) async fn list_agent_sub_agents(
    pool: &Pool<MySql>,
    scope: &AgentAccessScope,
) -> AppResult<Vec<AgentSubAgentResponse>> {
    let agents = sqlx::query_as::<_, AgentSubAgentResponse>(
        r#"SELECT descendants.id, descendants.parent_agent_id,
                  COALESCE(descendants.root_agent_id, descendants.id) AS root_agent_id,
                  descendants.agent_code, descendants.level, descendants.path,
                  descendants.status,
                  (SELECT COUNT(*) FROM user_referrals direct_referrals
                   WHERE direct_referrals.root_agent_id = descendants.id) AS direct_user_count,
                  (SELECT COUNT(*)
                   FROM user_referrals team_referrals
                   INNER JOIN agents owner_agents ON owner_agents.id = team_referrals.root_agent_id
                   WHERE owner_agents.path = descendants.path
                      OR owner_agents.path LIKE CONCAT(descendants.path, '/%')) AS team_user_count
           FROM agents descendants
           WHERE descendants.id <> ?
             AND descendants.path LIKE CONCAT(?, '/%')
           ORDER BY descendants.level ASC, descendants.id ASC
           LIMIT 500"#,
    )
    .bind(scope.agent_id)
    .bind(&scope.path)
    .fetch_all(pool)
    .await?;
    Ok(agents)
}

pub(crate) async fn list_agent_commissions(
    pool: &Pool<MySql>,
    scope: &AgentAccessScope,
) -> AppResult<Vec<AgentCommissionResponse>> {
    let commissions = sqlx::query_as::<_, AgentCommissionResponse>(
        r#"SELECT records.id, records.user_id, users.email, records.source_type,
                  records.source_id, records.source_amount, records.commission_rate,
                  records.commission_amount,
                  records.status, referrals.depth,
                  payout.id AS payout_ledger_id,
                  COALESCE(payout.asset_id, records.payout_asset_id) AS payout_asset_id,
                  payout.amount AS payout_amount,
                  payout.balance_after AS payout_balance_after,
                  payout.created_at AS payout_created_at,
                  records.created_at
           FROM agent_commission_records records
           INNER JOIN user_referrals referrals ON referrals.user_id = records.user_id
           INNER JOIN users ON users.id = records.user_id
           INNER JOIN agents owner_agents ON owner_agents.id = referrals.root_agent_id
           LEFT JOIN agents ON agents.id = records.agent_id
           LEFT JOIN wallet_ledger payout
             ON payout.user_id = agents.user_id
            AND payout.ref_type = 'agent_commission'
            AND CAST(payout.ref_id AS UNSIGNED) = records.id
            AND payout.change_type = 'agent_commission_payout'
            AND records.status = 'settled'
           WHERE records.agent_id = ?
             AND (owner_agents.path = ?
               OR owner_agents.path LIKE CONCAT(?, '/%'))
           ORDER BY records.id ASC
           LIMIT 100"#,
    )
    .bind(scope.agent_id)
    .bind(&scope.path)
    .bind(&scope.path)
    .fetch_all(pool)
    .await?;

    Ok(commissions)
}

pub(crate) async fn list_agent_invite_codes(
    pool: &Pool<MySql>,
    agent_id: u64,
) -> AppResult<Vec<AgentInviteCodeResponse>> {
    let invite_codes = sqlx::query_as::<_, AgentInviteCodeResponse>(
        r#"SELECT id, owner_id, code, usage_limit, used_count, status, created_at
           FROM invite_codes
           WHERE owner_type = 'agent' AND owner_id = ?
           ORDER BY id ASC
           LIMIT 100"#,
    )
    .bind(agent_id)
    .fetch_all(pool)
    .await?;

    Ok(invite_codes)
}

pub(crate) async fn insert_agent_invite_code(
    pool: &Pool<MySql>,
    write: AgentInviteCodeWrite,
) -> AppResult<u64> {
    let insert = sqlx::query(
        r#"INSERT INTO invite_codes (owner_type, owner_id, code, usage_limit)
           VALUES ('agent', ?, ?, ?)"#,
    )
    .bind(write.agent_id)
    .bind(&write.code)
    .bind(write.usage_limit)
    .execute(pool)
    .await?;

    Ok(insert.last_insert_id())
}

pub(crate) async fn update_agent_invite_code_status(
    pool: &Pool<MySql>,
    agent_id: u64,
    invite_code_id: u64,
    status: &str,
) -> AppResult<bool> {
    let result = sqlx::query(
        r#"UPDATE invite_codes
           SET status = ?
           WHERE id = ? AND owner_type = 'agent' AND owner_id = ?"#,
    )
    .bind(status)
    .bind(invite_code_id)
    .bind(agent_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub(crate) async fn load_agent_invite_code_by_id(
    pool: &Pool<MySql>,
    agent_id: u64,
    invite_code_id: u64,
) -> AppResult<Option<AgentInviteCodeResponse>> {
    let invite_code = sqlx::query_as::<_, AgentInviteCodeResponse>(
        r#"SELECT id, owner_id, code, usage_limit, used_count, status, created_at
           FROM invite_codes
           WHERE id = ? AND owner_type = 'agent' AND owner_id = ?
           LIMIT 1"#,
    )
    .bind(invite_code_id)
    .bind(agent_id)
    .fetch_optional(pool)
    .await?;

    Ok(invite_code)
}
