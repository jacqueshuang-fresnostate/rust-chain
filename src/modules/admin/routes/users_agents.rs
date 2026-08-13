//! 承载后台用户、KYC 审核、代理层级与代理佣金四组资源的 HTTP 传输入口。
//!
//! 本文件只负责 axum 提取器解析、管理员 subject 到审计主体编号的换算以及用例转发，
//! 不持有数据库连接、不开启事务、不做业务判定。只读入口用 `_auth: AdminAuth` 仅确认管理员已登录，
//! 写入口取出 claims 换算 admin_id 交给应用层写审计。字段校验、行锁顺序、状态机迁移和审计留痕
//! 全部发生在 application 与 service 层，产生的 `AppError` 按既有映射原样传播到 HTTP 边界。

use super::*;

/// 构建用户、KYC、代理层级及代理佣金的后台传输路由。
///
/// 每个入口继续由 `AdminAuth` 提取器执行管理员鉴权；路径、查询和 JSON DTO 解析后，
/// 写操作从管理员 subject 提取审计主体并转交应用用例。应用层返回的校验、鉴权与持久化错误
/// 按既有 `AppError` 映射原样向 HTTP 边界传播，路由本身不持有事务或业务状态。
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/users", get(list_admin_users).post(create_admin_user))
        .route("/users/:id", get(get_admin_user))
        .route("/users/:id/recharge", post(recharge_admin_user_wallet))
        .route("/users/:id/status", patch(update_admin_user_status))
        .route("/kyc/config", get(get_kyc_config).patch(save_kyc_config))
        .route("/kyc/submissions", get(list_kyc_submission_routes))
        .route("/kyc/submissions/:id", get(get_kyc_submission))
        .route("/kyc/submissions/:id/review", patch(review_kyc_submission))
        .route("/agents", get(list_agents).post(create_agent))
        .route("/agents/:id", get(get_agent))
        .route("/agents/:id/status", patch(update_agent_status))
        .route("/agents/:id/password/reset", post(reset_agent_password))
        .route("/agents/:id/users", get(list_agent_users))
        .route("/users/:id/agent", patch(assign_user_agent))
        .route(
            "/agent-commission-rules",
            get(list_agent_commission_rules).post(create_agent_commission_rule),
        )
        .route(
            "/agent-commission-rules/:id",
            patch(update_agent_commission_rule),
        )
        .route("/agent-commissions", get(list_agent_commissions))
        .route(
            "/agent-commissions/:id/status",
            patch(update_agent_commission_status),
        )
        .route(
            "/agent-commissions/batch-status",
            post(update_agent_commission_statuses),
        )
}

/// 处理 GET /agents，把代理编号、所属用户、父级、根代理、层级、代理码、邮箱与状态等条件交给列表用例。
/// 只要求管理员已登录，不解析 claims 也不写审计；分页裁剪、代理码去空白和团队统计聚合都在应用层完成，
/// 本入口仅回传当前页代理集合与匹配总数。
async fn list_agents(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminAgentQuery>,
) -> AppResult<Json<AdminAgentsResponse>> {
    Ok(Json(
        list_agents_use_case(state.mysql.clone(), query).await?,
    ))
}

/// 处理 GET /agents/:id，按路径中的代理编号读取层级路径、团队统计与门户账号状态组成的单条详情。
/// 只确认管理员登录态，查询不加锁；代理不存在时由应用层返回未找到，路由不补默认值也不隐藏该错误。
async fn get_agent(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(agent_id): Path<u64>,
) -> AppResult<Json<AdminAgentResponse>> {
    Ok(Json(
        get_agent_use_case(state.mysql.clone(), agent_id).await?,
    ))
}

/// 处理 POST /agents，从管理员 claims 解析审计主体后创建代理主记录及其门户登录账号。
/// 请求体中的所属用户、父代理、代理码、门户用户名与初始口令由应用层在同一事务内校验并完成层级放置，
/// 路由不预先查库；代理码或门户用户名撞唯一键会整笔回滚，此处只把冲突错误原样返回。
async fn create_agent(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateAgentRequest>,
) -> AppResult<Json<AdminAgentResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_agent_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

/// 处理 PATCH /agents/:id/status，把目标状态与可选原因交给代理状态迁移用例。
/// 应用层锁定代理行后同步更新代理主表与其名下全部门户账号状态，并写入 before/after 审计，
/// 路由不判断状态取值是否合法；重复提交相同状态仍会新增审计，且不会撤销代理已有的在线会话。
async fn update_agent_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(agent_id): Path<u64>,
    Json(request): Json<UpdateAgentStatusRequest>,
) -> AppResult<Json<AdminAgentResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_agent_status_use_case(state.mysql.clone(), admin_id, agent_id, request).await?,
    ))
}

/// 处理 POST /agents/:id/password/reset，重置代理门户口令并强制其重新登录。
/// 与同组其他代理入口只传连接池不同，这里必须传入完整 `AppState`，供用例在事务提交后撤销在线访问会话；
/// 请求强制要求审计原因，事务内改口令、吊销刷新令牌并清空登录失败计数，未绑定门户账号的代理返回冲突。
async fn reset_agent_password(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(agent_id): Path<u64>,
    Json(request): Json<ResetAgentPasswordRequest>,
) -> AppResult<Json<AdminAgentPasswordResetResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        reset_agent_password_use_case(state, admin_id, agent_id, request).await?,
    ))
}

/// 处理 GET /agents/:id/users，先确认代理存在再分页读取其邀请路径覆盖的团队用户。
/// 存在性检查与列表查询不共享同一事务快照，因此期间新增的下级可能落在两次读取之间；
/// 本入口只透传分页参数，不写审计也不修改任何邀请关系。
async fn list_agent_users(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(agent_id): Path<u64>,
    Query(query): Query<AdminAgentUsersQuery>,
) -> AppResult<Json<AdminAgentUsersResponse>> {
    Ok(Json(
        list_agent_users_use_case(state.mysql.clone(), agent_id, query).await?,
    ))
}

/// 处理 PATCH /users/:id/agent，把路径中的用户改派到请求体指定的启用代理名下。
/// 应用层事务会同时锁定用户、目标代理与原邀请关系，并重算该用户及其全部后代的邀请路径、深度与根代理归属；
/// 目标代理不是 active 时返回冲突，重复改派到同一代理仍会写一条新的审计记录。
async fn assign_user_agent(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(user_id): Path<u64>,
    Json(request): Json<AssignUserAgentRequest>,
) -> AppResult<Json<AdminUserReferralResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        assign_user_agent_use_case(state.mysql.clone(), admin_id, user_id, request).await?,
    ))
}

/// 处理 GET /agent-commission-rules，按代理、产品类型和状态筛选佣金费率规则。
/// 产品类型与状态在应用层只做去空白处理而不在此做枚举校验，因此非法取值会表现为空结果而非报错；
/// 该入口既不锁定规则行，也不触发任何历史佣金的重算。
async fn list_agent_commission_rules(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminAgentCommissionRuleQuery>,
) -> AppResult<Json<AdminAgentCommissionRulesResponse>> {
    Ok(Json(
        list_agent_commission_rules_use_case(state.mysql.clone(), query).await?,
    ))
}

/// 处理 POST /agent-commission-rules，为指定代理新增某一产品线的佣金费率规则。
/// 请求必须携带审计原因，费率须落在 0 到 1 之间，状态缺省为 active；应用层先确认代理存在再插入并写审计。
/// 该接口没有幂等键，重复调用会新建规则或因唯一约束冲突失败，且不会追溯结算已经产生的佣金。
async fn create_agent_commission_rule(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateAgentCommissionRuleRequest>,
) -> AppResult<Json<AdminAgentCommissionRuleResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_agent_commission_rule_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

/// 处理 PATCH /agent-commission-rules/:id，局部修改单条佣金规则的费率或启停状态。
/// 请求必须携带审计原因，费率与状态均为可选字段，两者都缺省时仍会走一次更新并留下审计；
/// 应用层锁定旧规则后写入并记录 before/after 对比，改动只影响后续计算，已生成佣金不重算。
async fn update_agent_commission_rule(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(rule_id): Path<u64>,
    Json(request): Json<UpdateAgentCommissionRuleRequest>,
) -> AppResult<Json<AdminAgentCommissionRuleResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_agent_commission_rule_use_case(state.mysql.clone(), admin_id, rule_id, request)
            .await?,
    ))
}

/// 处理 GET /agent-commissions，按代理、用户、邮箱和佣金状态筛选已生成的佣金记录。
/// 返回来源金额与佣金金额组成的分页集合及匹配总数；该入口不锁定待结算记录，也不改动任何钱包余额。
async fn list_agent_commissions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminAgentCommissionQuery>,
) -> AppResult<Json<AdminAgentCommissionsResponse>> {
    Ok(Json(
        list_agent_commissions_use_case(state.mysql.clone(), query).await?,
    ))
}

/// 处理 PATCH /agent-commissions/:id/status，对单笔佣金执行结算或拒绝的状态迁移。
/// 应用层锁定该佣金后只允许从 pending 迁出，结算分支会把佣金金额计入代理所属用户的钱包并写入流水；
/// 记录已不是 pending 时返回冲突，因此重复调用不会造成二次入账。
async fn update_agent_commission_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(commission_id): Path<u64>,
    Json(request): Json<UpdateAgentCommissionStatusRequest>,
) -> AppResult<Json<AdminAgentCommissionResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_agent_commission_status_use_case(
            state.mysql.clone(),
            admin_id,
            commission_id,
            request,
        )
        .await?,
    ))
}

/// 处理 POST /agent-commissions/batch-status，按同一目标状态批量处理请求体列出的佣金编号。
/// 与单笔入口的关键差异是每条记录走各自独立的事务，单条失败只在对应结果项里记 failed 与错误文本，
/// 不会回滚其余已成功的结算或拒绝；响应逐条返回处理结果，调用方必须自行核对失败项后重试。
async fn update_agent_commission_statuses(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<BatchUpdateAgentCommissionStatusRequest>,
) -> AppResult<Json<AdminAgentCommissionBatchStatusResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_agent_commission_statuses_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

/// 处理 POST /users，由管理员直接开户并返回新建用户的档案快照。
/// 请求必须携带审计原因；应用层在同一事务内写入用户、生成邀请码、投递用户创建 outbox 事件并记录审计，
/// 邮箱或手机号重复会撞唯一键并整体回滚，事件只有在事务提交之后才可能被投递出去。
async fn create_admin_user(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateAdminUserRequest>,
) -> AppResult<Json<AdminUserResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_admin_user_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

/// 处理 GET /users，按用户编号、邮箱和状态筛选平台用户列表。
/// include_internal 缺省为 false，即默认隐藏内部账号；响应不含口令散列或双因素密钥，查询也不加任何行锁。
async fn list_admin_users(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminUserQuery>,
) -> AppResult<Json<AdminUsersResponse>> {
    Ok(Json(
        list_admin_users_use_case(state.mysql.clone(), query).await?,
    ))
}

/// 处理 GET /users/:id，读取单个用户的联系方式、状态、KYC 等级与创建更新时间。
/// 与列表入口一样只验证管理员登录态且不返回任何凭据字段，用户缺失时由应用层直接返回未找到。
async fn get_admin_user(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(user_id): Path<u64>,
) -> AppResult<Json<AdminUserResponse>> {
    Ok(Json(
        get_admin_user_use_case(state.mysql.clone(), user_id).await?,
    ))
}

/// 处理 POST /users/:id/recharge，执行后台人工加币并返回加账后的钱包快照。
/// 请求必须携带审计原因；应用层在事务内校验资产处于启用状态、增加可用余额、写同额钱包流水并记录审计。
/// 每次调用都会生成新的充值编号且没有请求幂等键，响应超时后重试存在再次入账的风险。
async fn recharge_admin_user_wallet(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(user_id): Path<u64>,
    Json(request): Json<AdminUserRechargeRequest>,
) -> AppResult<Json<AdminUserRechargeResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        recharge_admin_user_wallet_use_case(state.mysql.clone(), admin_id, user_id, request)
            .await?,
    ))
}

/// 处理 PATCH /users/:id/status，变更用户启用状态并在封禁时切断其继续登录的能力。
/// 与同组只需连接池的用户入口不同，这里向用例传入完整 `AppState`，以便在事务提交后撤销在线访问会话；
/// 目标状态不是 active 时事务内同步吊销刷新令牌，会话撤销属于提交后副作用，其失败不回滚状态变更。
async fn update_admin_user_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(user_id): Path<u64>,
    Json(request): Json<UpdateUserStatusRequest>,
) -> AppResult<Json<AdminUserResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_admin_user_status_use_case(state, admin_id, user_id, request).await?,
    ))
}

/// 处理 GET /kyc/config，读取 KYC 上下文维护的全局审核开关与分国家证件规则。
/// 后台在此只做转发，不缓存配置也不写审计，返回结构与字段缺省语义完全由 KYC 模块决定。
async fn get_kyc_config(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<KycConfigResponse>> {
    Ok(Json(get_kyc_config_use_case(state.mysql.clone()).await?))
}

/// 处理 PATCH /kyc/config，保存 KYC 全局审核配置并在后台留下变更痕迹。
/// 请求必须携带审计原因；应用层先委托 KYC 用例在事务内校验并写配置，再把其返回的 before/after
/// 写进后台审计后统一提交。保存成功不会重新审核任何历史申请，相同内容重放依旧新增一条审计。
async fn save_kyc_config(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<SaveKycConfigRequest>,
) -> AppResult<Json<KycConfigResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        save_kyc_config_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

/// 处理 GET /kyc/submissions，按用户、邮箱和审核状态分页检索实名认证申请。
/// 函数名带 routes 后缀是为了避开同名用例导入造成的冲突，行为上只做只读检索，
/// 既不锁定申请行，也不推进任何审核状态。
async fn list_kyc_submission_routes(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminKycSubmissionQuery>,
) -> AppResult<Json<KycSubmissionsResponse>> {
    Ok(Json(
        list_kyc_submissions_use_case(state.mysql.clone(), query).await?,
    ))
}

/// 处理 GET /kyc/submissions/:id，读取单笔实名申请的身份资料、证件文档与当前审核结论。
/// 只验证管理员登录且不加审核锁，文档解码与脱敏规则沿用 KYC 上下文，本入口不会产生审核类审计。
async fn get_kyc_submission(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(submission_id): Path<u64>,
) -> AppResult<Json<KycSubmissionResponse>> {
    Ok(Json(
        get_kyc_submission_use_case(state.mysql.clone(), submission_id).await?,
    ))
}

/// 处理 PATCH /kyc/submissions/:id/review，提交实名审核结论并同步用户的实名等级。
/// 请求必须携带审计原因；应用层依据最终状态选择通过或驳回两种审计动作，与 KYC 状态迁移同事务提交。
/// 已进入终态的申请再次审核会返回冲突，从而避免覆盖既有结论。
async fn review_kyc_submission(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(submission_id): Path<u64>,
    Json(request): Json<ReviewKycSubmissionRequest>,
) -> AppResult<Json<KycSubmissionResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        review_kyc_submission_use_case(state.mysql.clone(), admin_id, submission_id, request)
            .await?,
    ))
}
