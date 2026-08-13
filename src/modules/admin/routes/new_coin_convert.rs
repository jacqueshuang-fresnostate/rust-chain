//! 承载新币项目生命周期治理、认购派发锁仓查询与闪兑交易对管理的 HTTP 传输入口。
//!
//! 新币侧的读接口分为「按项目维度」和「全站扁平维度」两套路径，前者从路径段补齐 project_id 后
//! 复用同一批用例；写接口只解析管理员审计主体并转发，项目行锁、生命周期迁移图校验、派发幂等键判重、
//! 锁仓与钱包入账全部由 application 层在单事务内完成。闪兑侧同样只做提取与转发，
//! 其中删除入口在用例成功后返回 204 空响应，其余入口回传 JSON 快照。

use super::*;

/// 构建新币项目生命周期、分发/解锁查询与闪兑管理的后台传输路由。
///
/// 每个入口保持管理员鉴权及原有 DTO，写操作仅解析管理员审计主体并转发应用用例；
/// 分发、解锁、购后开放和闪兑对删除等事务与幂等规则不进入路由层。闪兑对删除成功
/// 仍返回 204，领域或持久化失败继续沿既有错误映射传播。
/// 注意新币子资源同时注册了带项目路径段和不带路径段的两种检索路径，二者最终落到同一批列表用例，
/// 区别只在于项目编号是被路径强制指定还是作为可选查询条件。
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/new-coins",
            get(list_new_coin_projects).post(create_new_coin_project),
        )
        .route("/new-coins/:id/lifecycle", patch(update_new_coin_lifecycle))
        .route("/new-coins/:id/distribute", post(distribute_new_coin))
        .route(
            "/new-coins/:id/unlock-rule",
            patch(update_new_coin_unlock_rule),
        )
        .route(
            "/new-coins/:id/post-listing-purchase",
            patch(update_new_coin_post_listing_purchase),
        )
        .route(
            "/new-coins/:id/unlock-fee-rule",
            patch(update_new_coin_unlock_fee_rule),
        )
        .route(
            "/new-coins/:id/subscriptions",
            get(list_new_coin_subscriptions),
        )
        .route(
            "/new-coins/:id/distributions",
            get(list_new_coin_distributions),
        )
        .route(
            "/new-coins/subscriptions",
            get(list_all_new_coin_subscriptions),
        )
        .route(
            "/new-coins/distributions",
            get(list_all_new_coin_distributions),
        )
        .route("/new-coins/purchases", get(list_new_coin_purchases))
        .route(
            "/new-coins/lock-positions",
            get(list_new_coin_lock_positions),
        )
        .route("/new-coins/unlocks", get(list_new_coin_unlocks))
        .route(
            "/convert/pairs",
            get(list_convert_pairs).post(create_convert_pair),
        )
        .route(
            "/convert/pairs/:id",
            get(get_convert_pair)
                .patch(update_convert_pair)
                .delete(delete_convert_pair),
        )
        .route(
            "/convert/new-coin-rules",
            post(upsert_new_coin_convert_rule),
        )
        .route("/convert/orders", get(list_convert_orders))
        .route("/convert/orders/:id", get(get_convert_order))
}

/// 处理 GET /new-coins，分页读取新币项目的发行量、发行价、生命周期、解锁与手续费配置。
/// 该查询目前只支持 limit 与 offset，不接受业务筛选条件；读取不加项目锁，也不汇总认购或派发金额。
async fn list_new_coin_projects(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewCoinProjectQuery>,
) -> AppResult<Json<NewCoinProjectsResponse>> {
    Ok(Json(
        list_new_coin_projects_use_case(state.mysql.clone(), query).await?,
    ))
}

/// 处理 GET /convert/pairs，分页读取全部闪兑交易对及其源、目标资产的展示信息。
/// 同样只接受分页参数而无筛选条件；返回的是数据库中保存的定价与限额配置，不会实时计算兑换报价。
async fn list_convert_pairs(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminConvertPairQuery>,
) -> AppResult<Json<ConvertPairsResponse>> {
    Ok(Json(
        list_convert_pairs_use_case(state.mysql.clone(), query).await?,
    ))
}

/// 处理 GET /convert/pairs/:id，读取单个闪兑交易对的资产、定价模式、点差费率与双边限额。
/// 只验证管理员登录态，查询不加锁；交易对不存在时返回未找到，本入口也不产出即时兑换报价。
async fn get_convert_pair(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(pair_id): Path<u64>,
) -> AppResult<Json<ConvertPairResponse>> {
    Ok(Json(
        get_convert_pair_use_case(state.mysql.clone(), pair_id).await?,
    ))
}

/// 处理 GET /convert/orders，按用户、邮箱和订单状态筛选闪兑成交流水。
/// 返回资产、金额、成交汇率、手续费与时间构成的分页集合；查询不锁订单或钱包，也不会补偿失败订单。
async fn list_convert_orders(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminConvertOrdersQuery>,
) -> AppResult<Json<ConvertOrdersResponse>> {
    Ok(Json(
        list_convert_orders_use_case(state.mysql.clone(), query).await?,
    ))
}

/// 处理 GET /convert/orders/:id，读取单笔闪兑订单及其关联用户、资产与最终定价结果。
/// 查询既不加订单锁也不加钱包锁，订单缺失返回未找到；本入口不会重试或改写任何订单状态。
async fn get_convert_order(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<ConvertOrderResponse>> {
    Ok(Json(
        get_convert_order_use_case(state.mysql.clone(), order_id).await?,
    ))
}

/// 处理 GET /new-coins/:id/subscriptions，按项目维度分页检索该项目下的认购记录。
/// 与全站扁平入口的差异在于 project_id 来自路径段并强制覆盖查询条件，用户、邮箱和状态仍可继续叠加筛选。
async fn list_new_coin_subscriptions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Query(query): Query<AdminNewCoinScopedListQuery>,
) -> AppResult<Json<NewCoinSubscriptionsResponse>> {
    Ok(Json(
        list_new_coin_subscriptions_for_project_use_case(state.mysql.clone(), project_id, query)
            .await?,
    ))
}

/// 处理 GET /new-coins/subscriptions，跨项目检索认购记录，项目编号作为可选查询条件出现。
/// 与按项目维度的入口共用同一套底层筛选，但允许不指定项目而拉取全站认购；查询不锁记录也不结算认购款。
async fn list_all_new_coin_subscriptions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewCoinFlatListQuery>,
) -> AppResult<Json<NewCoinSubscriptionsResponse>> {
    Ok(Json(
        list_new_coin_subscriptions_use_case(state.mysql.clone(), query).await?,
    ))
}

/// 处理 GET /new-coins/:id/distributions，按项目维度分页检索该项目已产生的派发记录。
/// 响应含派发数量、关联锁仓头寸与幂等键，可据此核对是否重复发币；本入口不锁派发行也不触发补发。
async fn list_new_coin_distributions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Query(query): Query<AdminNewCoinScopedListQuery>,
) -> AppResult<Json<NewCoinDistributionsResponse>> {
    Ok(Json(
        list_new_coin_distributions_for_project_use_case(state.mysql.clone(), project_id, query)
            .await?,
    ))
}

/// 处理 GET /new-coins/distributions，跨项目检索派发记录并按用户、邮箱、状态叠加筛选。
/// 项目编号在此为可选条件，适合做全站派发对账；查询只读，不会重放任何失败的派发事务。
async fn list_all_new_coin_distributions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewCoinFlatListQuery>,
) -> AppResult<Json<NewCoinDistributionsResponse>> {
    Ok(Json(
        list_new_coin_distributions_use_case(state.mysql.clone(), query).await?,
    ))
}

/// 处理 GET /new-coins/purchases，检索新币上市后通过指定交易对产生的购买记录。
/// 与认购记录属于不同阶段：认购发生在上市前，本入口统计的是开放购买后的成交；查询不触发任何兑换结算。
async fn list_new_coin_purchases(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewCoinPurchaseQuery>,
) -> AppResult<Json<NewCoinPurchasesResponse>> {
    Ok(Json(
        list_new_coin_purchases_use_case(state.mysql.clone(), query).await?,
    ))
}

/// 处理 GET /new-coins/lock-positions，按用户、邮箱、资产和状态检索派发形成的锁仓头寸。
/// 响应包含解锁时间、剩余数量与来源派发信息；本入口只读，既不推进解锁也不释放任何锁仓余额。
async fn list_new_coin_lock_positions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewCoinLockPositionQuery>,
) -> AppResult<Json<NewCoinLockPositionsResponse>> {
    Ok(Json(
        list_new_coin_lock_positions_use_case(state.mysql.clone(), query).await?,
    ))
}

/// 处理 GET /new-coins/unlocks，检索锁仓解禁记录。
/// 该查询比锁仓头寸多一个费用支付状态筛选维度，可用于排查解禁手续费未扣的记录；
/// 本入口不会执行解锁，也不会补扣任何解禁费用。
async fn list_new_coin_unlocks(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewCoinUnlockQuery>,
) -> AppResult<Json<NewCoinUnlocksResponse>> {
    Ok(Json(
        list_new_coin_unlocks_use_case(state.mysql.clone(), query).await?,
    ))
}

/// 处理 POST /new-coins，登记新币项目的发行参数、初始生命周期与解锁手续费配置。
/// 应用层在同一事务内插入项目、写一条生命周期事件并记录后台审计，因此项目一旦可见就必然带有追溯记录；
/// 该接口没有幂等键，创建成功也不会自动开放认购、派发资产或上线交易对。
async fn create_new_coin_project(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateNewCoinProjectRequest>,
) -> AppResult<Json<NewCoinProjectResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_new_coin_project_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

/// 处理 PATCH /new-coins/:id/lifecycle，按领域迁移图把项目推进到预热、认购、派发或上市。
/// 应用层先锁项目行再用锁后的旧状态校验迁移合法性，因此并发推进只有一方成功，另一方得到校验错误；
/// 进入上市状态时缺省以当前时间写入 listed_at，本入口不会顺带触发派发或交易对上线。
async fn update_new_coin_lifecycle(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Json(request): Json<UpdateNewCoinLifecycleRequest>,
) -> AppResult<Json<NewCoinProjectResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_new_coin_lifecycle_use_case(state.mysql.clone(), admin_id, project_id, request)
            .await?,
    ))
}

/// 处理 PATCH /new-coins/:id/unlock-rule，整体替换项目的解锁模式与对应时间参数。
/// 上市即解锁、固定时间解锁和相对周期解锁三种形状各有必填字段组合，由应用层校验后覆盖写入；
/// 已经生成的锁仓头寸不会按新规则重算，改规则只对后续派发生效。
async fn update_new_coin_unlock_rule(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Json(request): Json<UpdateNewCoinUnlockRuleRequest>,
) -> AppResult<Json<NewCoinProjectResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_new_coin_unlock_rule_use_case(state.mysql.clone(), admin_id, project_id, request)
            .await?,
    ))
}

/// 处理 PATCH /new-coins/:id/unlock-fee-rule，开启或关闭解禁手续费并设定费率、计费依据与收费资产。
/// 关闭收费时应用层会一并清空费率、计费依据和费用资产，避免旧配置在下次开启前被误用；
/// 与解锁规则入口相互独立，本入口不改变解锁时间点，只影响解禁时的扣费口径。
async fn update_new_coin_unlock_fee_rule(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Json(request): Json<UpdateNewCoinUnlockFeeRuleRequest>,
) -> AppResult<Json<NewCoinProjectResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_new_coin_unlock_fee_rule_use_case(
            state.mysql.clone(),
            admin_id,
            project_id,
            request,
        )
        .await?,
    ))
}

/// 处理 PATCH /new-coins/:id/post-listing-purchase，开启或关闭新币上市后的二级购买通道。
/// 开启时必须提供交易对编号，应用层会校验该交易对确实关联本项目资产，并同时激活交易对与项目开关；
/// 关闭路径只清除项目侧开关，不会把此前被激活的交易对回退为停用，需要另行调整交易对状态。
async fn update_new_coin_post_listing_purchase(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Json(request): Json<UpdateNewCoinPostListingPurchaseRequest>,
) -> AppResult<Json<NewCoinProjectResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_new_coin_post_listing_purchase_use_case(
            state.mysql.clone(),
            admin_id,
            project_id,
            request,
        )
        .await?,
    ))
}

/// 处理 POST /new-coins/:id/distribute，向指定用户派发新币并按解锁规则决定直接入账还是转为锁仓。
/// 这是本文件唯一带请求幂等键的写入口：应用层锁项目后先查重幂等键，已存在则返回冲突而不会二次发币；
/// 派发数量、钱包余额流水、锁仓头寸、生命周期事件与后台审计共用一个事务，任一步失败整体回滚。
async fn distribute_new_coin(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Json(request): Json<DistributeNewCoinRequest>,
) -> AppResult<Json<NewCoinDistributionResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        distribute_new_coin_use_case(state.mysql.clone(), admin_id, project_id, request).await?,
    ))
}

/// 处理 POST /convert/new-coin-rules，为某个闪兑交易对创建或覆盖唯一的新币兑换规则。
/// 虽然使用 POST，语义上却是按 convert_pair_id 的 upsert：应用层先锁旧规则，存在则更新、不存在则插入，
/// 并据此把审计动作分别记为 update 或 create，因此重复提交不会产生同一交易对的多条规则。
async fn upsert_new_coin_convert_rule(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<UpsertNewCoinConvertRuleRequest>,
) -> AppResult<Json<NewCoinConvertRuleResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        upsert_new_coin_convert_rule_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

/// 处理 POST /convert/pairs，新增闪兑交易对的资产方向、定价模式、点差费率与双边限额。
/// 请求必须携带审计原因；启用标记缺省为 true、费率缺省为 0，目标侧最小额缺省沿用源侧最小额。
/// 该接口没有幂等键，提交结果不明时重试会撞唯一约束而不是静默复用既有交易对。
async fn create_convert_pair(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateConvertPairRequest>,
) -> AppResult<Json<ConvertPairResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_convert_pair_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

/// 处理 PATCH /convert/pairs/:id，在锁定的旧快照上合并局部字段并整体重新校验闪兑配置。
/// 请求必须携带审计原因；未提供的字段沿用旧值，合并后的资产、费率与限额组合会被再次校验以防跨字段矛盾。
/// 应用层依据本次是否改动了配置字段，把审计动作区分为配置更新与仅状态更新两种。
async fn update_convert_pair(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(pair_id): Path<u64>,
    Json(request): Json<UpdateConvertPairRequest>,
) -> AppResult<Json<ConvertPairResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_convert_pair_use_case(state.mysql.clone(), admin_id, pair_id, request).await?,
    ))
}

/// 处理 DELETE /convert/pairs/:id，删除已停用且无业务引用的闪兑交易对。
/// 该入口虽为 DELETE 却仍需 JSON 请求体来携带必填审计原因；仍处于启用状态的交易对直接返回校验错误。
/// 应用层锁定后会确认没有报价、订单和新币兑换规则引用才真正删除，删除不可重放，成功后重试将得到未找到。
async fn delete_convert_pair(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(pair_id): Path<u64>,
    Json(request): Json<DeleteConvertPairRequest>,
) -> AppResult<StatusCode> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    delete_convert_pair_use_case(state.mysql.clone(), admin_id, pair_id, request).await?;
    Ok(StatusCode::NO_CONTENT)
}
