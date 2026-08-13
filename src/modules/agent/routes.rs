//! agent bounded context route layer.
//!
//! 路由层：声明代理端 HTTP 端点并把请求转交给应用层用例。
//! 本文件全部处理器都强制 `AgentAuth` 提取器，代理身份只从令牌主体解析，绝不接受路径或查询参数指定代理，
//! 可见范围与分页上限由应用层按物化路径统一收敛，路由本身不做业务校验也不拼装响应体。

use super::{
    application::{
        change_agent_password, create_agent_invite_code, get_agent_convert_stats,
        get_agent_dashboard, get_agent_me, list_agent_commissions, list_agent_invite_codes,
        list_agent_sub_agents, list_agent_team_tree, list_agent_users,
        update_agent_invite_code_status,
    },
    presentation::{
        AgentCommissionsResponse, AgentConvertStatsResponse, AgentDashboardResponse,
        AgentInviteCodeResponse, AgentInviteCodesResponse, AgentListQuery, AgentMeResponse,
        AgentPasswordChangeResponse, AgentSubAgentsResponse, AgentTeamTreeResponse,
        AgentUsersResponse, ChangeAgentPasswordRequest, CreateInviteCodeRequest,
        UpdateInviteCodeStatusRequest,
    },
};
use crate::{error::AppResult, modules::auth::AgentAuth, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, patch, post},
};

/// 装配代理自助后台的全部端点，由上层挂载到代理专属前缀之下。
/// 邀请码集合路径同时承载查询与创建两个方法，状态切换单独走子路径的 PATCH，其余均为只读 GET，改密使用 POST。
/// 这里只声明路由表，鉴权由各处理器的 `AgentAuth` 提取器完成，本函数不注册中间件也不做限流。
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/me", get(me))
        .route("/dashboard", get(dashboard))
        .route("/users", get(list_users))
        .route(
            "/invite-codes",
            get(list_invite_codes).post(create_invite_code),
        )
        .route("/invite-codes/:id/status", patch(update_invite_code_status))
        .route("/commissions", get(list_commissions))
        .route("/convert/stats", get(convert_stats))
        .route("/sub-agents", get(sub_agents))
        .route("/team-tree", get(team_tree))
        .route("/password/change", post(change_password))
}

/// 承接代理自助修改登录口令的请求，需要完整 `AppState` 以便在改密后清理 Redis 侧会话。
/// 目标账号只取自令牌主体，请求体仅提供新旧口令；成功后旧刷新令牌被吊销，响应提示必须重新登录。
async fn change_password(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
    Json(request): Json<ChangeAgentPasswordRequest>,
) -> AppResult<Json<AgentPasswordChangeResponse>> {
    Ok(Json(
        change_agent_password(state, &claims.sub, request).await?,
    ))
}

/// 返回当前代理的身份档案，供前端展示代理编码、所处层级与上级归属关系。
/// 无请求参数，账号或任一祖先代理停用时应用层直接判为未授权，因此该接口也可用作登录态有效性探测。
async fn me(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
) -> AppResult<Json<AgentMeResponse>> {
    Ok(Json(get_agent_me(state.mysql.clone(), &claims.sub).await?))
}

/// 返回代理首页看板：整棵子树的团队人数、本级有效邀请码数量，以及按发放资产分组的佣金汇总。
/// 各项统计分多次无事务查询拼装，并发归属变更或结算落账时可能来自不同快照，仅供概览不作对账依据。
async fn dashboard(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
) -> AppResult<Json<AgentDashboardResponse>> {
    Ok(Json(
        get_agent_dashboard(state.mysql.clone(), &claims.sub).await?,
    ))
}

/// 返回子树内闪兑业务的订单聚合，含总单量、待处理与已完成单量以及转出转入金额合计。
/// 金额跨币种直接求和，仅用于运营量级观察；无入参也无分页，团队较大时为一次全子树扫描。
async fn convert_stats(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
) -> AppResult<Json<AgentConvertStatsResponse>> {
    Ok(Json(
        get_agent_convert_stats(state.mysql.clone(), &claims.sub).await?,
    ))
}

/// 分页返回子树内的业务用户明细，含账号状态、KYC 等级与归属代理编码。
/// 查询串只接受 limit 与 offset，页大小被应用层压到一百以内，代理无法通过参数扩大可见范围。
async fn list_users(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
    Query(query): Query<AgentListQuery>,
) -> AppResult<Json<AgentUsersResponse>> {
    Ok(Json(
        list_agent_users(state.mysql.clone(), &claims.sub, query).await?,
    ))
}

/// 返回团队关系树的两个分支数据：下级代理列表与带邀请深度的用户节点列表，并附服务端认定的根代理 ID。
/// 同一分页参数同时作用于两个列表，页大小上限五百；根 ID 来自令牌解析出的 scope，不接受客户端指定。
async fn team_tree(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
    Query(query): Query<AgentListQuery>,
) -> AppResult<Json<AgentTeamTreeResponse>> {
    Ok(Json(
        list_agent_team_tree(state.mysql.clone(), &claims.sub, query).await?,
    ))
}

/// 分页返回当前节点之下的下级代理及其直属用户数与子树用户数，页大小上限五百。
/// 与团队树接口相比这里只给代理节点不含业务用户，且结果一定不包含调用者自身节点。
async fn sub_agents(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
    Query(query): Query<AgentListQuery>,
) -> AppResult<Json<AgentSubAgentsResponse>> {
    Ok(Json(
        list_agent_sub_agents(state.mysql.clone(), &claims.sub, query).await?,
    ))
}

/// 分页返回本级代理实得的返佣流水，含业务来源类型与来源单号、计佣基数、分成比例及结算状态。
/// 已结算记录附带钱包入账流水与入账后余额，便于代理自查到账；下级代理的佣金不在此列出。
async fn list_commissions(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
    Query(query): Query<AgentListQuery>,
) -> AppResult<Json<AgentCommissionsResponse>> {
    Ok(Json(
        list_agent_commissions(state.mysql.clone(), &claims.sub, query).await?,
    ))
}

/// 分页返回当前代理名下的邀请码及其使用上限、已用次数与启用状态，页大小上限一百。
/// 仅列出本级自建的码，下级代理各自持有的邀请码需由其本人登录查看。
async fn list_invite_codes(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
    Query(query): Query<AgentListQuery>,
) -> AppResult<Json<AgentInviteCodesResponse>> {
    Ok(Json(
        list_agent_invite_codes(state.mysql.clone(), &claims.sub, query).await?,
    ))
}

/// 为当前代理创建一枚新邀请码，请求体只允许指定可选使用上限，码文本由服务端生成不接受自定义。
/// 上限必须为正数或缺省；插入成功后回读完整记录返回，回读落空按未找到处理。该接口无幂等键，重复提交会生成多枚码。
async fn create_invite_code(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateInviteCodeRequest>,
) -> AppResult<Json<AgentInviteCodeResponse>> {
    Ok(Json(
        create_agent_invite_code(state.mysql.clone(), &claims.sub, request).await?,
    ))
}

/// 在启用与停用之间切换指定邀请码状态，路径 ID 必须属于当前代理，否则一律返回未找到而不提示归属他人。
/// 状态只接受 active 与 disabled；停用不回收已建立的邀请关系，也不清零已用次数，仅阻止后续新用户使用该码注册。
async fn update_invite_code_status(
    AgentAuth(claims): AgentAuth,
    State(state): State<AppState>,
    Path(invite_code_id): Path<u64>,
    Json(request): Json<UpdateInviteCodeStatusRequest>,
) -> AppResult<Json<AgentInviteCodeResponse>> {
    Ok(Json(
        update_agent_invite_code_status(state.mysql.clone(), &claims.sub, invite_code_id, request)
            .await?,
    ))
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_agent_routes_tests.rs"]
mod tests;
