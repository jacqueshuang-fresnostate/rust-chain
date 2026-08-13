//! agent bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。
//! 本文件集中代理分销的全部 SQL：一类是各业务结算时向上生成分层返佣记录的写入口，
//! 另一类是代理自助后台的子树只读查询。所有子树查询都以服务端解析出的物化路径为边界，
//! 前缀匹配统一带斜杠分隔符防止同名前缀越权；带 in_tx 后缀的函数复用调用方事务，既不提交也不回滚。

use crate::{
    error::{AppError, AppResult},
    modules::agent::{
        domain::{AgentCommissionRateTier, allocate_differential_agent_commissions},
        presentation::{
            AgentCommissionResponse, AgentDashboardAssetSummaryResponse, AgentInviteCodeResponse,
            AgentMeResponse, AgentSubAgentResponse, AgentTeamTreeNodeResponse,
            AgentTeamUserResponse,
        },
        repository::{
            AgentAccessScope, AgentAdminCredentialRecord, AgentBusinessCommissionWrite,
            AgentCommissionRuleRecord, AgentConvertStatsRecord, AgentDashboardCountsRecord,
            AgentInviteCodeWrite, AgentListPage,
        },
    },
};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool, Transaction};

/// 在业务结算事务中，按用户所属代理链生成各层级的差额返佣待结算记录。
/// 调用方须传入正数业务基数、权威来源标识和发放资产；零或负数基数直接忽略。
/// 仅采用启用代理及其最新启用规则，并按发放资产精度量化累计返佣后再计算层级差额。
/// 记录写入复用调用方事务，不直接变更代理钱包；失败须随原业务结算一起回滚。
/// 同一代理、业务来源依靠唯一键忽略重放，保证结算重试不会重复生成待发返佣。
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

/// 按代理管理员 ID 读取账号与代理节点，本节点或任一祖先停用时不返回记录。
/// 查询不锁行且无写入副作用；未命中由应用层统一映射为未授权。
/// 祖先状态用路径前缀子查询整体核验，只要链路上任一级停用整条线即失效，下级不会退化成孤立可用节点。
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

/// 在调用方事务内用排他行锁读取代理管理员的密码哈希与账号状态，作为改密流程的第一步。
/// 加锁是为了让旧口令校验、新哈希写入与刷新令牌吊销串行化，防止两个并发改密请求互相覆盖出中间态。
/// 本函数只取锁不做任何判定，账号缺失返回空值、状态是否可用由应用层裁决；不自行提交或回滚事务。
pub(crate) async fn lock_agent_admin_credential_in_tx(
    tx: &mut Transaction<'_, MySql>,
    agent_admin_id: u64,
) -> AppResult<Option<AgentAdminCredentialRecord>> {
    let credential = sqlx::query_as::<_, AgentAdminCredentialRecord>(
        "SELECT password_hash, status FROM agent_admin_users WHERE id = ? LIMIT 1 FOR UPDATE",
    )
    .bind(agent_admin_id)
    .fetch_optional(&mut **tx)
    .await?;

    Ok(credential)
}

/// 在已持有凭证行锁的同一事务中覆盖代理管理员的密码哈希，调用前必须完成旧口令比对。
/// 只改哈希列，不改账号状态、不写改密审计、不清理任何会话，令牌吊销由后续独立语句在同事务内完成。
/// 目标行不存在时语句静默成功，因此该函数不能用于判断账号是否存在。
pub(crate) async fn update_agent_admin_password_in_tx(
    tx: &mut Transaction<'_, MySql>,
    agent_admin_id: u64,
    password_hash: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE agent_admin_users SET password_hash = ? WHERE id = ?")
        .bind(password_hash)
        .bind(agent_admin_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 在改密事务中把该代理主体名下所有尚未撤销的刷新令牌打上撤销时间戳，阻断旧凭证继续续期。
/// 过滤条件限定 actor 类型为 agent 且撤销时间为空，因此重复执行不会覆盖首次撤销时间，天然幂等。
/// 与密码更新同事务提交，保证不会出现新密码已生效而旧刷新令牌仍可用的窗口；不影响短期访问令牌，
/// 那部分需由应用层在事务提交后另行清理 Redis 侧会话。
pub(crate) async fn revoke_agent_admin_refresh_tokens_in_tx(
    tx: &mut Transaction<'_, MySql>,
    agent_admin_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE refresh_tokens
           SET revoked_at = CURRENT_TIMESTAMP(6)
           WHERE actor_type = 'agent' AND actor_id = ? AND revoked_at IS NULL"#,
    )
    .bind(agent_admin_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 为已认证代理管理员加载服务端权威子树范围，同时校验账号与全部祖先状态。
/// 返回的路径用于后续 SQL 边界，未命中不接受客户端提供的根 ID 替代。
/// 与身份档案查询共用同一套状态校验，但只取代理主键、根节点与物化路径三列，供子树查询直接绑定使用。
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

/// 按 scope 路径统计子树用户，并仅计数当前代理自有的启用邀请码。
/// 查询只读且不加锁，子树范围只接受服务端已验证路径，SQL 失败不返回部分计数。
/// 两项计数由两个独立子查询在同一条语句内完成，人数覆盖整棵子树，邀请码数只算本级且状态启用的。
pub(crate) async fn load_agent_dashboard_counts(
    pool: &Pool<MySql>,
    scope: &AgentAccessScope,
) -> AppResult<AgentDashboardCountsRecord> {
    let counts = sqlx::query_as::<_, AgentDashboardCountsRecord>(
        r#"SELECT (SELECT COUNT(*)
                   FROM user_referrals team_referrals
                   INNER JOIN agents owner_agents
                     ON owner_agents.id = team_referrals.root_agent_id
                   WHERE owner_agents.path = ?
                      OR owner_agents.path LIKE CONCAT(?, '/%')) AS team_user_count,
                  (SELECT COUNT(*)
                   FROM invite_codes
                   WHERE owner_type = 'agent' AND owner_id = ? AND status = 'active')
                   AS active_invite_code_count"#,
    )
    .bind(&scope.path)
    .bind(&scope.path)
    .bind(scope.agent_id)
    .fetch_one(pool)
    .await?;

    Ok(counts)
}

/// 仅汇总当前代理在授权子树内的佣金，并按发放资产分组避免跨资产相加。
/// 统计包含待结算、已结算与总额，查询只读且不修改佣金或钱包状态。
/// 三项金额由条件求和在同一次分组中得出，没有任何佣金记录时返回空列表而不是一行零值。
pub(crate) async fn load_agent_dashboard_asset_summaries(
    pool: &Pool<MySql>,
    scope: &AgentAccessScope,
) -> AppResult<Vec<AgentDashboardAssetSummaryResponse>> {
    // 佣金按发放资产逐一聚合，不同资产的金额不能直接相加。
    let summaries = sqlx::query_as::<_, AgentDashboardAssetSummaryResponse>(
        r#"SELECT records.payout_asset_id,
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
               OR owner_agents.path LIKE CONCAT(?, '/%'))
           GROUP BY records.payout_asset_id
           ORDER BY records.payout_asset_id"#,
    )
    .bind(scope.agent_id)
    .bind(&scope.path)
    .bind(&scope.path)
    .fetch_all(pool)
    .await?;

    Ok(summaries)
}

/// 聚合授权代理子树的兑换订单数、状态数和原目标金额，无订单时返回零值记录。
/// 本查询无行锁和写入副作用，路径必须来自已验证的代理 scope。
/// 状态计数由条件求和得出因而是十进制类型，需由服务层转回整数；两项金额跨币种直接相加，仅供量级观察。
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

/// 按代理物化路径分页读取子树用户，同时返回直属邀请人与归属代理两维关系。
/// 查询仅使用服务端 scope 和已限制分页，无锁、无写入，不包含父级或兄弟树。
/// 同时返回账号状态与 KYC 等级，排序固定为归属代理层级在前、用户主键在后，保证翻页结果不重不漏。
pub(crate) async fn list_agent_team_users(
    pool: &Pool<MySql>,
    scope: &AgentAccessScope,
    page: AgentListPage,
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
           LIMIT ? OFFSET ?"#,
    )
    .bind(&scope.path)
    .bind(&scope.path)
    .bind(page.limit as i64)
    .bind(page.offset as i64)
    .fetch_all(pool)
    .await?;

    Ok(users)
}

/// 按子树路径与邀请深度读取团队树用户节点，保留直属邀请人和公司归属。
/// 分页结果只读且无事务副作用，排序稳定为代理层级、邀请深度和用户 ID。
/// 与团队用户查询相比额外返回邀请关系的物化路径，客户端据此还原多级邀请链而无需再次请求。
pub(crate) async fn list_agent_team_tree_nodes(
    pool: &Pool<MySql>,
    scope: &AgentAccessScope,
    page: AgentListPage,
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
           LIMIT ? OFFSET ?"#,
    )
    .bind(&scope.path)
    .bind(&scope.path)
    .bind(page.limit as i64)
    .bind(page.offset as i64)
    .fetch_all(pool)
    .await?;

    Ok(nodes)
}

/// 列出当前 scope 真正后代代理，同时统计直属和整个子树用户数。
/// 当前节点不在结果中，路径前缀带分隔符以避免文本前缀越权，查询无写入。
/// 两项用户数由两个相关子查询逐行实时聚合而非读取冗余计数列，团队规模越大单次查询开销越高。
pub(crate) async fn list_agent_sub_agents(
    pool: &Pool<MySql>,
    scope: &AgentAccessScope,
    page: AgentListPage,
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
           LIMIT ? OFFSET ?"#,
    )
    .bind(scope.agent_id)
    .bind(&scope.path)
    .bind(page.limit as i64)
    .bind(page.offset as i64)
    .fetch_all(pool)
    .await?;
    Ok(agents)
}

/// 仅读取当前代理拥有且业务用户仍属授权子树的佣金记录。
/// 已结算记录左连对应钱包流水，缺失时保留空值；查询不补发佣金也不改状态。
/// 佣金归属与业务用户归属两个条件同时生效，缺任一都可能把他人佣金或子树外用户的数据带出。
/// 结果按佣金主键倒序分页，最新计提的返佣排在最前。
pub(crate) async fn list_agent_commissions(
    pool: &Pool<MySql>,
    scope: &AgentAccessScope,
    page: AgentListPage,
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
           ORDER BY records.id DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(scope.agent_id)
    .bind(&scope.path)
    .bind(&scope.path)
    .bind(page.limit as i64)
    .bind(page.offset as i64)
    .fetch_all(pool)
    .await?;

    Ok(commissions)
}

/// 按所有者类型和代理 ID 分页读取自有邀请码，不混入用户或子代理记录。
/// 结果按主键稳定升序，查询不锁行、不修改邀请码状态或已用次数。
/// 返回使用上限、已用次数与启用状态三项运营关注的字段，下级代理自建的邀请码不会混入本结果。
pub(crate) async fn list_agent_invite_codes(
    pool: &Pool<MySql>,
    agent_id: u64,
    page: AgentListPage,
) -> AppResult<Vec<AgentInviteCodeResponse>> {
    let invite_codes = sqlx::query_as::<_, AgentInviteCodeResponse>(
        r#"SELECT id, owner_id, code, usage_limit, used_count, status, created_at
           FROM invite_codes
           WHERE owner_type = 'agent' AND owner_id = ?
           ORDER BY id ASC
           LIMIT ? OFFSET ?"#,
    )
    .bind(agent_id)
    .bind(page.limit as i64)
    .bind(page.offset as i64)
    .fetch_all(pool)
    .await?;

    Ok(invite_codes)
}

/// 为指定代理插入新邀请码和可选使用上限，返回数据库生成主键。
/// 唯一键或其他 SQL 失败直接上抛；本操作使用连接池单语句提交，不重试生成码。
/// 使用上限为空即写入不限次数，状态与已用次数交由数据库默认值填充，因此需另行回读才能拿到完整快照。
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

/// 按主键、代理所有者和固定 owner 类型更新邀请码状态，防止跨代理修改。
/// 返回值来自 MySQL 受影响行数；同值更新可能返回 `false`，不能区分记录缺失与数据库视为未变更。
/// 本语句不修改使用次数或既有邀请关系，状态取值已由服务层收敛为启用或停用两种。
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

/// 按主键与代理所有权回读邀请码，不属于当前代理的记录按未命中处理。
/// 查询只读且无行锁，用于写入后返回权威快照，SQL 错误直接上抛。
/// 所有权条件与主键同时参与匹配，因此跨代理读取会被当成记录不存在，而不是暴露出存在但无权访问。
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
