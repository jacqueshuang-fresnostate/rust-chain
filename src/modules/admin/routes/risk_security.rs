//! 承载全局安全策略、风控规则与事件、用户双因素重置以及杠杆强平记录查询的 HTTP 传输入口。
//!
//! 这组入口按写入风险分为三档：安全策略与风控规则是会立即改变线上判定的配置写入；
//! 用户双因素重置是针对单个账号的高风险人工干预；风控事件与强平记录则是纯只读的事后取证视图。
//! 路由本身不解析规则配置 JSON、不执行任何风险评估、也不参与保证金结算事务，
//! 只负责鉴权、提取输入和解析管理员审计主体，随后把决策完全交给应用层。

use super::*;

/// 构建安全策略、风险规则/事件、管理员重置 2FA 与强平查询路由。
///
/// 读写入口均保持 `AdminAuth` 鉴权；敏感写操作从 subject 解析管理员编号后调用应用用例，
/// 风险规则、2FA 审计与强平数据的策略和持久化不在路由层执行。解析、确认及领域错误继续
/// 使用统一错误映射，避免拆分改变既有 HTTP 状态和响应 DTO。
/// 用户双因素重置虽以 /users 开头，但因属于安全域而与风控规则注册在同一路由集合内。
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

/// 处理 GET /security-policy，读取登录二次验证、注册邀请、用户名登录、支付动作与第三方绑定的全局策略。
/// 返回的是面向全站的策略而非某个用户的个人安全设置；读取不加配置锁，缺省值语义由下层决定。
async fn get_security_policy(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<UserSecurityPolicy>> {
    Ok(Json(
        get_security_policy_use_case(state.mysql.clone()).await?,
    ))
}

/// 处理 PATCH /security-policy，整体替换全局用户安全策略。
/// 请求必须携带审计原因；这是整体替换而非局部合并，未提交的分项会按请求构造的默认值落库。
/// 应用层在事务外读取旧策略且不锁配置行，因此并发提交可能让审计里的旧值略显陈旧；
/// 策略收紧后不会主动踢掉已在线的会话，只对后续鉴权判定生效。
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

/// 处理 POST /users/:id/2fa/reset，清除指定用户已绑定的双因素密钥与恢复码。
/// 这是本文件风险最高的入口，请求必须携带审计原因，重置与审计在同一事务内提交。
/// 重置只解除该用户的二次验证绑定，不会修改其登录口令、不会封禁账号，也不向用户发送任何通知。
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

/// 处理 GET /risk/rules，按规则类型、目标类型和启用标记检索风控规则。
/// 响应直接回传规则的原始配置 JSON 而不做结构解析，因此后台可以查看尚未被识别的新版配置字段。
async fn list_risk_rules(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminRiskRuleQuery>,
) -> AppResult<Json<RiskRulesResponse>> {
    Ok(Json(
        list_risk_rules_use_case(state.mysql.clone(), query).await?,
    ))
}

/// 处理 POST /risk/rules，新增一条风控规则，其配置 JSON 的结构由应用层校验。
/// 规则一经提交即可参与后续风险判定，因此规则写入与审计在同一事务提交以保证来源可追溯。
/// 该接口没有业务幂等键，重复提交会创建语义相同的多条规则，需要人工核对后停用多余项。
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

/// 处理 PATCH /risk/rules/:id/status，切换单条风控规则的启用标记。
/// 与同组其他写入口不同，该入口不强制要求审计原因；应用层也不会重新解析规则配置或校验目标资源是否仍存在。
/// 停用只影响后续判定，既不回溯撤销已经产生的风控事件，也不主动刷新独立的风控缓存。
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

/// 处理 GET /risk/events，按用户、邮箱、处置决策和风险等级检索已产生的风控事件。
/// 响应含命中规则与判定详情，用于复盘拦截原因；读取不会重新评分，也不会因查询而生成新的事件记录。
async fn list_risk_events(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminRiskEventQuery>,
) -> AppResult<Json<RiskEventsResponse>> {
    Ok(Json(
        list_risk_events_use_case(state.mysql.clone(), query).await?,
    ))
}

/// 处理 GET /margin/liquidations，按用户、邮箱、交易对和仓位检索杠杆强平记录。
/// 这是对已发生强平的事后查询，不锁仓位、不锁钱包，也不会触发任何重新结算或补偿。
async fn list_margin_liquidations(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminMarginLiquidationQuery>,
) -> AppResult<Json<AdminMarginLiquidationsResponse>> {
    Ok(Json(
        list_margin_liquidations_use_case(state.mysql.clone(), query).await?,
    ))
}

/// 处理 GET /margin/liquidations/:id，读取单笔强平的用户、仓位、交易对、成交价格、费用与时间。
/// 查询完全不参与保证金事务，记录缺失返回未找到；本入口不修改任何仓位状态或用户余额。
async fn get_margin_liquidation(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(liquidation_id): Path<u64>,
) -> AppResult<Json<AdminMarginLiquidationResponse>> {
    Ok(Json(
        get_margin_liquidation_use_case(state.mysql.clone(), liquidation_id).await?,
    ))
}
