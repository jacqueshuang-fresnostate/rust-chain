use super::*;

/// 构建安全策略、风险规则/事件、管理员重置 2FA 与强平查询路由。
///
/// 读写入口均保持 `AdminAuth` 鉴权；敏感写操作从 subject 解析管理员编号后调用应用用例，
/// 风险规则、2FA 审计与强平数据的策略和持久化不在路由层执行。解析、确认及领域错误继续
/// 使用统一错误映射，避免拆分改变既有 HTTP 状态和响应 DTO。
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/risk/rules", get(list_risk_rules).post(create_risk_rule))
        .route("/risk/rules/:id/status", patch(update_risk_rule_status))
        .route("/risk/events", get(list_risk_events))
        .route(
            "/security-policy",
            get(get_security_policy).patch(update_security_policy),
        )
        .route("/users/:id/2fa/reset", post(reset_admin_user_two_factor))
        .route("/margin/liquidations", get(list_margin_liquidations))
        .route("/margin/liquidations/:id", get(get_margin_liquidation))
}

async fn get_security_policy(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<UserSecurityPolicy>> {
    Ok(Json(
        get_security_policy_use_case(state.mysql.clone()).await?,
    ))
}

async fn update_security_policy(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<UpdateSecurityPolicyRequest>,
) -> AppResult<Json<UserSecurityPolicy>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_security_policy_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn reset_admin_user_two_factor(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(user_id): Path<u64>,
    Json(request): Json<ResetUserTwoFactorRequest>,
) -> AppResult<Json<AdminUserTwoFactorResetResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        reset_admin_user_two_factor_use_case(state.mysql.clone(), admin_id, user_id, request)
            .await?,
    ))
}

async fn list_risk_rules(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminRiskRuleQuery>,
) -> AppResult<Json<RiskRulesResponse>> {
    Ok(Json(
        list_risk_rules_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn create_risk_rule(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateRiskRuleRequest>,
) -> AppResult<Json<RiskRuleResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_risk_rule_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn update_risk_rule_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(rule_id): Path<u64>,
    Json(request): Json<UpdateRiskRuleStatusRequest>,
) -> AppResult<Json<RiskRuleResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_risk_rule_status_use_case(state.mysql.clone(), admin_id, rule_id, request).await?,
    ))
}

async fn list_risk_events(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminRiskEventQuery>,
) -> AppResult<Json<RiskEventsResponse>> {
    Ok(Json(
        list_risk_events_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn list_margin_liquidations(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminMarginLiquidationQuery>,
) -> AppResult<Json<AdminMarginLiquidationsResponse>> {
    Ok(Json(
        list_margin_liquidations_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn get_margin_liquidation(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(liquidation_id): Path<u64>,
) -> AppResult<Json<AdminMarginLiquidationResponse>> {
    Ok(Json(
        get_margin_liquidation_use_case(state.mysql.clone(), liquidation_id).await?,
    ))
}
