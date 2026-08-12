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

async fn list_agents(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminAgentQuery>,
) -> AppResult<Json<AdminAgentsResponse>> {
    Ok(Json(
        list_agents_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn get_agent(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(agent_id): Path<u64>,
) -> AppResult<Json<AdminAgentResponse>> {
    Ok(Json(
        get_agent_use_case(state.mysql.clone(), agent_id).await?,
    ))
}

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

async fn list_agent_commission_rules(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminAgentCommissionRuleQuery>,
) -> AppResult<Json<AdminAgentCommissionRulesResponse>> {
    Ok(Json(
        list_agent_commission_rules_use_case(state.mysql.clone(), query).await?,
    ))
}

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

async fn list_agent_commissions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminAgentCommissionQuery>,
) -> AppResult<Json<AdminAgentCommissionsResponse>> {
    Ok(Json(
        list_agent_commissions_use_case(state.mysql.clone(), query).await?,
    ))
}

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

async fn list_admin_users(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminUserQuery>,
) -> AppResult<Json<AdminUsersResponse>> {
    Ok(Json(
        list_admin_users_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn get_admin_user(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(user_id): Path<u64>,
) -> AppResult<Json<AdminUserResponse>> {
    Ok(Json(
        get_admin_user_use_case(state.mysql.clone(), user_id).await?,
    ))
}

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

async fn get_kyc_config(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<KycConfigResponse>> {
    Ok(Json(get_kyc_config_use_case(state.mysql.clone()).await?))
}

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

async fn list_kyc_submission_routes(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminKycSubmissionQuery>,
) -> AppResult<Json<KycSubmissionsResponse>> {
    Ok(Json(
        list_kyc_submissions_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn get_kyc_submission(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(submission_id): Path<u64>,
) -> AppResult<Json<KycSubmissionResponse>> {
    Ok(Json(
        get_kyc_submission_use_case(state.mysql.clone(), submission_id).await?,
    ))
}

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
