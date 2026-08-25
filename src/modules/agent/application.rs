//! agent bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。
//! 代理端全部只读用例都遵循同一条前置链路：先从令牌主体解析代理管理员，再由服务端查出物化路径 scope，
//! 后续 SQL 一律以该路径为边界，客户端无法通过传参跨越到父级或兄弟代理团队。
//! 除改密外本文件不开显式事务，多次查询之间不保证同一数据库快照。

use crate::{
    error::{AppError, AppResult},
    modules::{
        agent::{
            infrastructure,
            presentation::{
                AgentCommissionsResponse, AgentConvertStatsResponse, AgentDashboardResponse,
                AgentInviteCodeResponse, AgentInviteCodesResponse, AgentListQuery, AgentMeResponse,
                AgentPasswordChangeResponse, AgentSubAgentsResponse, AgentTeamTreeResponse,
                AgentUsersResponse, ChangeAgentPasswordRequest, CreateInviteCodeRequest,
                UpdateInviteCodeStatusRequest,
            },
            repository::{AgentAccessScope, AgentInviteCodeWrite},
            service::{
                agent_admin_id_from_subject, agent_commissions_response,
                agent_convert_stats_response, agent_dashboard_response, agent_list_page,
                generated_agent_invite_code, validate_agent_invite_code_status,
                validate_agent_invite_code_usage_limit, validate_agent_password_change,
            },
        },
        auth::{ActorType, AuthActor, hash_password, revoke_actor_auth_sessions, verify_password},
    },
    state::AppState,
};
use sqlx::{MySql, Pool};

/// 返回当前登录代理的账号与节点档案，含代理编码、父级、根节点、层级、物化路径及双侧状态。
/// 校验链要求管理员账号、自身代理节点以及路径上每一级祖先都处于启用状态，任一环节停用即整条线视为失效。
/// 因此上级被封禁时下级会立即失去登录态，而不是继续以孤立节点身份访问；未命中统一映射为未授权，不区分账号不存在与被停用。
/// 只读查询，不刷新最后登录时间，也不触发任何会话或归属写入。
pub(crate) async fn get_agent_me(
    mysql: Option<Pool<MySql>>,
    subject: &str,
) -> AppResult<AgentMeResponse> {
    let agent_admin_id = agent_admin_id_from_subject(subject)?;
    let pool = agent_mysql_pool(mysql)?;

    infrastructure::load_agent_me(&pool, agent_admin_id)
        .await?
        .ok_or(AppError::Unauthorized)
}

/// 在当前代理子树内汇总团队用户、有效邀请码和分资产佣金。
/// scope、计数和资产汇总分别查询且不开事务，并发归属/结算时各块数据可能来自不同快照。
pub(crate) async fn get_agent_dashboard(
    mysql: Option<Pool<MySql>>,
    subject: &str,
) -> AppResult<AgentDashboardResponse> {
    let (pool, scope) = agent_context(mysql, subject).await?;
    let counts = infrastructure::load_agent_dashboard_counts(&pool, &scope).await?;
    let assets = infrastructure::load_agent_dashboard_asset_summaries(&pool, &scope).await?;
    Ok(agent_dashboard_response(scope.agent_id, counts, assets))
}

/// 先验证代理账号及祖先状态，再按物化路径统计整个子树的兑换订单数与金额。
/// scope 与聚合是两个无事务查询；两者之间发生代理停用时，本次调用不会再次校验权限快照。
pub(crate) async fn get_agent_convert_stats(
    mysql: Option<Pool<MySql>>,
    subject: &str,
) -> AppResult<AgentConvertStatsResponse> {
    let (pool, scope) = agent_context(mysql, subject).await?;
    let row = infrastructure::load_agent_convert_stats(&pool, &scope).await?;
    agent_convert_stats_response(row)
}

/// 分页列出归属当前代理或其后代节点的业务用户，返回账号状态、KYC 等级、归属代理与直属邀请人两维关系。
/// 页大小上限一百，范围由服务端 scope 路径限定，客户端既无法放大分页也无法越权看到父级或兄弟团队。
/// 只读用例，不改归属也不补建邀请关系；未归属任何代理的用户天然不在结果集中。
pub(crate) async fn list_agent_users(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    query: AgentListQuery,
) -> AppResult<AgentUsersResponse> {
    let (pool, scope) = agent_context(mysql, subject).await?;
    let page = agent_list_page(query.limit, query.offset, 100);
    let users = infrastructure::list_agent_team_users(&pool, &scope, page).await?;
    Ok(AgentUsersResponse { users })
}

/// 分页列出当前节点之下的下级代理，逐条附带直属用户数与整棵子树用户数两个口径的统计。
/// 结果显式排除自身，路径前缀带分隔符以避免同名文本前缀误命中，页大小上限五百。
/// 统计随查询实时聚合而非读取冗余计数列，团队规模较大时开销明显；只读用例，不改层级关系也不重算返佣。
pub(crate) async fn list_agent_sub_agents(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    query: AgentListQuery,
) -> AppResult<AgentSubAgentsResponse> {
    let (pool, scope) = agent_context(mysql, subject).await?;
    let page = agent_list_page(query.limit, query.offset, 500);
    let agents = infrastructure::list_agent_sub_agents(&pool, &scope, page).await?;
    Ok(AgentSubAgentsResponse { agents })
}

/// 先读取服务端代理 scope，再分别读取子代理和用户邀请节点，组装团队树而不修改归属。
/// 三次查询不开事务，不保证同一数据库快照；任一失败则整体失败，根 ID 不接受客户端覆盖。
/// 同一页参数同时作用于代理与用户两个列表，上限五百；用户节点自带邀请深度与关系路径，层级结构由客户端还原。
pub(crate) async fn list_agent_team_tree(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    query: AgentListQuery,
) -> AppResult<AgentTeamTreeResponse> {
    let (pool, scope) = agent_context(mysql, subject).await?;
    let page = agent_list_page(query.limit, query.offset, 500);
    let agents = infrastructure::list_agent_sub_agents(&pool, &scope, page).await?;
    let nodes = infrastructure::list_agent_team_tree_nodes(&pool, &scope, page).await?;

    Ok(AgentTeamTreeResponse {
        root_agent_id: scope.root_agent_id,
        agents,
        nodes,
    })
}

/// 分页列出归属当前代理本人的佣金记录，并额外用子树路径约束产生佣金的业务用户仍在可见范围内。
/// 只返回本级实得的差额返佣，不汇总下级代理的佣金；已结算记录会左连出对应钱包流水，未结算时流水字段为空。
/// 响应中的合计金额只对本页记录求和，不代表历史总额，跨发放资产时也不做换算。
/// 只读用例，不触发结算、不改记录状态、不补发漏算佣金。
pub(crate) async fn list_agent_commissions(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    query: AgentListQuery,
) -> AppResult<AgentCommissionsResponse> {
    let (pool, scope) = agent_context(mysql, subject).await?;
    let page = agent_list_page(query.limit, query.offset, 100);
    let commissions = infrastructure::list_agent_commissions(&pool, &scope, page).await?;
    Ok(agent_commissions_response(scope.agent_id, commissions))
}

/// 分页读取当前代理名下的邀请码及其使用上限、已用次数与启用状态，页大小上限一百。
/// 查询按所有者类型和代理主键精确匹配，只返回本级自建的码，不合并下级代理的邀请码，也不混入普通用户码。
/// 虽然先解析了子树 scope 用于身份校验，但列表范围仅限当前节点自身；只读用例，不新建码也不改用量。
pub(crate) async fn list_agent_invite_codes(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    query: AgentListQuery,
) -> AppResult<AgentInviteCodesResponse> {
    let (pool, scope) = agent_context(mysql, subject).await?;
    let page = agent_list_page(query.limit, query.offset, 100);
    let invite_codes = infrastructure::list_agent_invite_codes(&pool, scope.agent_id, page).await?;
    Ok(AgentInviteCodesResponse { invite_codes })
}

/// 校验使用上限后为当前代理生成新邀请码，插入成功后再按所有权回读完整快照。
/// 冲突或数据库错误直接失败；本用例未开显式事务，回读缺失按未找到处理。
/// 码文本由服务端按时间有序的 UUID 生成，请求体只能指定使用上限；没有幂等键，重复提交会各生成一枚新码。
pub(crate) async fn create_agent_invite_code(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    request: CreateInviteCodeRequest,
) -> AppResult<AgentInviteCodeResponse> {
    let (pool, scope) = agent_context(mysql, subject).await?;
    validate_agent_invite_code_usage_limit(request.usage_limit)?;

    let write = AgentInviteCodeWrite {
        agent_id: scope.agent_id,
        code: generated_agent_invite_code(),
        usage_limit: request.usage_limit,
    };
    let invite_code_id = infrastructure::insert_agent_invite_code(&pool, write).await?;

    infrastructure::load_agent_invite_code_by_id(&pool, scope.agent_id, invite_code_id)
        .await?
        .ok_or(AppError::NotFound)
}

/// 仅允许邀请码所属代理在启用与停用之间切换，不匹配所有权时返回未找到。
/// 单语句更新后另行回读权威快照；重复设置不改使用次数或邀请关系，但数据库若对同值更新
/// 报告零受影响行，本用例会返回未找到，因此不承诺同值请求幂等成功。
pub(crate) async fn update_agent_invite_code_status(
    mysql: Option<Pool<MySql>>,
    subject: &str,
    invite_code_id: u64,
    request: UpdateInviteCodeStatusRequest,
) -> AppResult<AgentInviteCodeResponse> {
    let status = validate_agent_invite_code_status(&request.status)?;
    let (pool, scope) = agent_context(mysql, subject).await?;
    let updated = infrastructure::update_agent_invite_code_status(
        &pool,
        scope.agent_id,
        invite_code_id,
        status,
    )
    .await?;

    if !updated {
        return Err(AppError::NotFound);
    }

    infrastructure::load_agent_invite_code_by_id(&pool, scope.agent_id, invite_code_id)
        .await?
        .ok_or(AppError::NotFound)
}

/// 校验新旧口令后锁定代理管理员凭证，同事务更新哈希并撤销 MySQL 刷新令牌。
/// 旧口令或账号状态不符时不写入；提交后尝试撤销 Sa-Token/Redis 会话且不签发新令牌。
/// 外部失败时新密码已生效；会话枚举或登出失败会向上报告，避免把未完成的撤销伪装成成功。
pub(crate) async fn change_agent_password(
    state: AppState,
    subject: &str,
    request: ChangeAgentPasswordRequest,
) -> AppResult<AgentPasswordChangeResponse> {
    let agent_admin_id = agent_admin_id_from_subject(subject)?;
    let (current_password, new_password) =
        validate_agent_password_change(request.current_password, request.new_password)?;
    let password_hash = hash_password(&new_password)?;
    let pool = agent_mysql_pool(state.mysql.clone())?;

    // 校验旧密码、写入新密码和吊销刷新令牌同事务提交，避免旧凭证在改密后仍可续期。
    let mut tx = pool.begin().await?;
    let credential = infrastructure::lock_agent_admin_credential_in_tx(&mut tx, agent_admin_id)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if credential.status != "active" {
        return Err(AppError::Unauthorized);
    }
    if !verify_password(&credential.password_hash, &current_password)? {
        return Err(AppError::Validation(
            "current_password is incorrect".to_owned(),
        ));
    }
    infrastructure::update_agent_admin_password_in_tx(&mut tx, agent_admin_id, &password_hash)
        .await?;
    infrastructure::revoke_agent_admin_refresh_tokens_in_tx(&mut tx, agent_admin_id).await?;
    tx.commit().await?;

    revoke_actor_auth_sessions(
        &state,
        &AuthActor::new(ActorType::Agent, agent_admin_id, None),
    )
    .await?;
    Ok(AgentPasswordChangeResponse {
        changed: true,
        requires_relogin: true,
    })
}

/// 解析令牌主体并加载服务端权威的代理可见范围，是本文件所有只读用例共用的鉴权入口。
/// 依次完成三件事：从主体串取出代理管理员主键，取出 MySQL 连接池，再按管理员查出代理主键、根节点与物化路径。
/// 该查询同时校验管理员账号、自身节点及全部祖先节点均为启用状态，任一环节不满足都返回未授权。
/// 返回的路径是后续 SQL 的唯一边界来源，调用方不得改用请求参数中的代理 ID，否则会形成越权查询。
async fn agent_context(
    mysql: Option<Pool<MySql>>,
    subject: &str,
) -> AppResult<(Pool<MySql>, AgentAccessScope)> {
    let agent_admin_id = agent_admin_id_from_subject(subject)?;
    let pool = agent_mysql_pool(mysql)?;
    // 每个代理只能访问自己的 materialized-path 子树，不能借用顶级代理 ID 越权查看兄弟团队。
    let scope = infrastructure::load_agent_access_scope_for_admin(&pool, agent_admin_id)
        .await?
        .ok_or(AppError::Unauthorized)?;
    Ok((pool, scope))
}

/// 为其他限界上下文解析当前代理令牌对应的精确 active 代理 ID。
/// 本入口复用代理门户的权威身份链：先解析 `agent:<admin_id>` subject，
/// 再回查代理管理员、自身节点与全部祖先均为 active。返回值只是当前节点 ID，
/// 不暴露 path 也不授予子树权限，在线客服等精确所有者业务必须使用这个值做等值筛选。
pub(crate) async fn resolve_active_agent_id(
    mysql: Option<Pool<MySql>>,
    subject: &str,
) -> AppResult<u64> {
    Ok(agent_context(mysql, subject).await?.1.agent_id)
}

/// 取出代理路由所需的 MySQL 连接池，未配置数据库时归类为内部错误而非校验错误。
/// 代理端全部接口都强依赖持久化，缺少连接池属于部署配置缺失，不应向调用方暴露为可重试的业务失败。
fn agent_mysql_pool(mysql: Option<Pool<MySql>>) -> AppResult<Pool<MySql>> {
    mysql.ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for agent routes".to_owned())
    })
}
