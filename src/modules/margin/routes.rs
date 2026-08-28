//! 杠杆上下文的 HTTP 路由装配层。
//!
//! 本文件只做四件事：绑定 axum 路径与方法、通过 `UserAuth` 或 `AdminAuth` 完成鉴权、
//! 把 JWT subject 解析成用户或管理员标识、从 `AppState` 取出 MySQL 连接池与 Redis、事件广播中心，
//! 然后原样转发给 application 层用例并把结果包成 JSON。
//! 所有业务校验、事务边界、锁顺序、幂等判定与事件发布时机都在用例内部，本层不得自行拼装资金逻辑。
//! 用户路由前缀下的开仓、平仓、撤销为写资金入口，产品配置类写操作只出现在后台路由。

use crate::{
    error::AppResult,
    modules::margin::service::admin_id_from_subject,
    modules::user::service::user_id_from_subject,
    modules::{
        auth::{AdminAuth, UserAuth},
        margin::{
            application::{
                cancel_all_margin_positions_with_events as cancel_all_margin_positions_with_events_use_case,
                cancel_margin_position_with_events as cancel_margin_position_with_events_use_case,
                close_all_margin_positions_with_events as close_all_margin_positions_with_events_use_case,
                close_margin_position_with_events as close_margin_position_with_events_use_case,
                create_margin_product as create_margin_product_use_case,
                get_admin_margin_position as get_admin_margin_position_use_case,
                get_admin_margin_product as get_admin_margin_product_use_case,
                get_margin_position_risk_snapshot as get_margin_position_risk_snapshot_use_case,
                get_user_margin_position as get_user_margin_position_use_case,
                get_user_margin_setting as get_user_margin_setting_use_case,
                list_active_margin_products as list_active_margin_products_use_case,
                list_admin_margin_interest_summary as list_admin_margin_interest_summary_use_case,
                list_admin_margin_position_history as list_admin_margin_position_history_use_case,
                list_admin_margin_products as list_admin_margin_products_use_case,
                list_user_margin_positions as list_user_margin_positions_use_case,
                list_user_margin_wallets as list_user_margin_wallets_use_case, mysql_pool,
                open_margin_position_with_events as open_margin_position_with_events_use_case,
                route_limit, transfer_margin_funds as transfer_margin_funds_use_case,
                update_margin_product_config as update_margin_product_config_use_case,
                update_margin_product_status as update_margin_product_status_use_case,
                update_user_leverage as update_user_leverage_use_case,
                update_user_margin_mode as update_user_margin_mode_use_case,
            },
            presentation::{
                AdminInterestSummaryQuery, AdminInterestSummaryResponse, AdminListPositionsQuery,
                AdminMarginPositionResponse, AdminMarginPositionsResponse,
                AdminMarginProductsQuery, AdminMarginProductsResponse,
                CancelAllMarginPositionsResponse, CancelMarginPositionResponse,
                CloseAllMarginPositionsResponse, CloseMarginPositionRequest,
                CloseMarginPositionResponse, CreateMarginProductRequest, ListPositionsQuery,
                ListQuery, MarginPositionDetailResponse, MarginPositionsResponse,
                MarginProductResponse, MarginProductsResponse, MarginRiskSnapshotResponse,
                MarginUserSettingResponse, MarginWalletsResponse, OpenMarginPositionRequest,
                OpenMarginPositionResponse, ProductActionRequest, TransferMarginFundsRequest,
                TransferMarginFundsResponse, UpdateMarginProductRequest,
                UpdateMarginProductStatusRequest, UpdateUserLeverageRequest,
                UpdateUserMarginModeRequest,
            },
        },
    },
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, patch, post},
};

/// 装配用户侧杠杆路由，覆盖产品浏览、钱包查询、现货与杠杆互转、单产品设置和仓位全生命周期。
/// 批量入口 `close-all` 与 `cancel-all` 注册在 `:id` 之前，避免这两个字面量路径被主键参数吞掉。
/// 每个 handler 内部各自要求 `UserAuth`，本函数只声明路径映射，不挂载任何鉴权或限流中间件。
pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/margin/products", get(list_active_products))
        .route("/margin/wallets", get(list_margin_wallets))
        .route("/margin/transfers", post(transfer_margin_funds))
        .route("/margin/settings/:product_id", get(get_user_margin_setting))
        .route(
            "/margin/settings/:product_id/leverage",
            patch(update_user_leverage),
        )
        .route(
            "/margin/settings/:product_id/mode",
            patch(update_user_margin_mode),
        )
        .route("/margin/positions", get(list_positions).post(open_position))
        .route("/margin/positions/close-all", post(close_all_positions))
        .route("/margin/positions/cancel-all", post(cancel_all_positions))
        .route("/margin/positions/:id", get(get_position))
        .route("/margin/positions/:id/risk", get(get_position_risk))
        .route("/margin/positions/:id/close", post(close_position))
        .route("/margin/positions/:id/cancel", post(cancel_position))
}

/// 装配后台杠杆路由，包含产品的增查改和启停、全量仓位历史检索以及利息汇总报表。
/// 与用户路由的关键区别是这里没有任何直接动用户钱包的入口，写操作只落在产品配置与审计日志。
/// 全部 handler 走 `AdminAuth`，改配类还会把 JWT 中的管理员标识透传给用例用于审计归属。
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/margin/products",
            get(list_admin_products).post(create_product),
        )
        .route(
            "/margin/products/:id",
            get(get_admin_product).patch(update_product),
        )
        .route("/margin/products/:id/status", patch(update_product_status))
        .route("/margin/positions", get(list_admin_positions))
        .route("/margin/positions/:id", get(get_admin_position))
        .route("/margin/interest/summary", get(list_admin_interest_summary))
}
/// 公开返回启用杠杆产品清单及后端真实支持的下单、模式和持仓能力集。
/// 产品配置不含用户资金或设置，因此允许访客在登录前浏览；钱包、风险和所有写入口仍分别要求 `UserAuth`。
/// `limit` 会被夹到 1 到 100，停用产品由应用层固定过滤。
async fn list_active_products(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<MarginProductsResponse>> {
    Ok(Json(
        list_active_margin_products_use_case(&mysql_pool(&state)?, route_limit(query.limit))
            .await?,
    ))
}

/// 返回后台杠杆产品分页列表，附带与列表同筛选口径的总数，供管理端翻页展示。
/// 与用户侧的差别是包含 disabled 产品并支持 offset；只读路径不解析管理员标识也不写审计。
async fn list_admin_products(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminMarginProductsQuery>,
) -> AppResult<Json<AdminMarginProductsResponse>> {
    Ok(Json(
        list_admin_margin_products_use_case(&mysql_pool(&state)?, query).await?,
    ))
}

/// 按主键读取单个杠杆产品的完整配置，用于后台编辑表单回填。
/// 产品不存在时用例返回 NotFound 并映射为 404；该路径不锁行、不改配置、不写审计日志。
async fn get_admin_product(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
) -> AppResult<Json<MarginProductResponse>> {
    Ok(Json(
        get_admin_margin_product_use_case(&mysql_pool(&state)?, product_id).await?,
    ))
}

/// 新建杠杆产品；先解析管理员标识，再把请求体交给用例在单事务内写产品行和创建审计。
/// 这里直接传 `state.mysql` 的 Option 而非 `mysql_pool`，缺少连接池的报错由用例统一给出。
/// 交易对或保证金币种不存在、费率精度越界、缺少变更原因都会在事务提交前失败并整体回滚。
async fn create_product(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateMarginProductRequest>,
) -> AppResult<Json<MarginProductResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_margin_product_use_case(state.mysql.as_ref(), admin_id, request).await?,
    ))
}

/// 整体改写指定杠杆产品的配置，请求体是全量快照而非增量补丁，未传字段按缺省语义处理。
/// 用例会先锁定产品旧行取 before 快照，再更新并回读 after，两份快照连同变更原因写入同一审计事务。
/// 改动杠杆档位或维持保证金率只影响后续开仓，不会重算已存在仓位的风险参数。
async fn update_product(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
    Json(request): Json<UpdateMarginProductRequest>,
) -> AppResult<Json<MarginProductResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_margin_product_config_use_case(state.mysql.as_ref(), admin_id, product_id, request)
            .await?,
    ))
}

/// 只切换杠杆产品的 active 与 disabled 状态，不触碰杠杆档位、费率等其余配置。
/// 停用后开仓路径的产品锁定会判定为不可用，但已持有的仓位仍可正常平仓、撤销和计息。
/// 状态变更同样需要变更原因，并与 before/after 快照在同一事务内写入后台审计。
async fn update_product_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
    Json(request): Json<UpdateMarginProductStatusRequest>,
) -> AppResult<Json<MarginProductResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_margin_product_status_use_case(state.mysql.as_ref(), admin_id, product_id, request)
            .await?,
    ))
}

/// 按登录用户查询自己的杠杆仓位列表，可选传入状态筛选并按仓位主键倒序返回最近记录。
/// 用户标识只从 JWT 取，请求参数无法指定他人；非法状态值会在拼 SQL 前被规范化校验拒绝。
async fn list_positions(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListPositionsQuery>,
) -> AppResult<Json<MarginPositionsResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        list_user_margin_positions_use_case(&pool, user_id, query.status, route_limit(query.limit))
            .await?,
    ))
}

/// 汇总返回用户的杠杆钱包三桶余额、当前 opened 仓位以及各币种全仓账户风险快照。
/// 固定按默认上限五十条取仓位，接口不接受 `limit` 参数，避免钱包页拉取超大列表。
/// 全仓风险字段是强平 worker 上次刷新的落库值，不在这个请求里按最新行情重算。
async fn list_margin_wallets(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<MarginWalletsResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        list_user_margin_wallets_use_case(&pool, state.redis.as_ref(), user_id, route_limit(None))
            .await?,
    ))
}

/// 发起现货钱包与杠杆钱包之间的同资产划转，是本路由文件里第一个真正动用户余额的入口。
/// 请求体携带方向、金额和可选幂等键；用例在事务内先落划转记录占键，再按现货、杠杆的固定锁序动账。
/// 同键同参重放返回原划转编号与当时的余额快照且不再动账，同键异参返回冲突。
async fn transfer_margin_funds(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<TransferMarginFundsRequest>,
) -> AppResult<Json<TransferMarginFundsResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        transfer_margin_funds_use_case(&pool, state.redis.as_ref(), user_id, request).await?,
    ))
}

/// 保存用户在指定杠杆产品上的默认杠杆倍数，倍数必须精确命中产品配置的某个档位。
/// 用例会先锁定启用产品再落设置，只影响后续开仓的默认值，不改动已开仓位的杠杆和名义价值。
async fn update_user_leverage(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
    Json(request): Json<UpdateUserLeverageRequest>,
) -> AppResult<Json<MarginUserSettingResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        update_user_leverage_use_case(&pool, user_id, product_id, request).await?,
    ))
}

/// 读取用户在指定杠杆产品上已保存的保证金模式和杠杆倍数，供交易页初始化下单面板。
/// 用户从未设置过该产品时用例返回 NotFound，客户端应据此回退到产品自身的默认模式与档位。
async fn get_user_margin_setting(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
) -> AppResult<Json<MarginUserSettingResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    Ok(Json(
        get_user_margin_setting_use_case(&mysql_pool(&state)?, user_id, product_id).await?,
    ))
}

/// 保存用户在指定杠杆产品上的保证金模式，目标模式必须同时被产品配置和后端风控实现支持。
/// 与改杠杆倍数共用同一张用户设置表和同样的产品行锁，两者互不覆盖对方未提供的字段。
/// 切换模式只决定下一笔开仓走逐仓还是全仓，不会迁移已存在仓位的资金归属。
async fn update_user_margin_mode(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
    Json(request): Json<UpdateUserMarginModeRequest>,
) -> AppResult<Json<MarginUserSettingResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        update_user_margin_mode_use_case(&pool, user_id, product_id, request).await?,
    ))
}

/// 后台跨用户检索杠杆仓位历史，支持按用户标识、邮箱、交易对和状态组合筛选并分页。
/// 返回体比用户侧多出强平时间与强平原因两列，用于事后核查风控处置结果。
async fn list_admin_positions(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminListPositionsQuery>,
) -> AppResult<Json<AdminMarginPositionsResponse>> {
    let pool = mysql_pool(&state)?;
    Ok(Json(
        list_admin_margin_position_history_use_case(&pool, query).await?,
    ))
}

/// 后台按仓位主键读取单条仓位详情，不带用户维度约束，可查看任意账户的持仓。
/// 记录缺失返回 404；该只读路径既不加行锁，也不会顺带触发利息计提或强平判定。
async fn get_admin_position(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(position_id): Path<u64>,
) -> AppResult<Json<AdminMarginPositionResponse>> {
    Ok(Json(
        get_admin_margin_position_use_case(&mysql_pool(&state)?, position_id).await?,
    ))
}

/// 后台按保证金币种和仓位状态分组汇总借款额、已计提利息和仓位笔数，用于利息收入报表。
/// 与仓位列表共用同一套筛选谓词，因此报表口径可以和明细页逐条对上；总数按分组键去重统计。
/// 纯读聚合，不执行任何计提，数值取自 worker 已写入仓位行的 `interest_amount`。
async fn list_admin_interest_summary(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminInterestSummaryQuery>,
) -> AppResult<Json<AdminInterestSummaryResponse>> {
    let pool = mysql_pool(&state)?;
    Ok(Json(
        list_admin_margin_interest_summary_use_case(&pool, query).await?,
    ))
}

/// 读取当前用户名下单个杠杆仓位的详情，查询同时带上用户标识以防越权读取他人持仓。
/// 仓位不属于该用户或不存在都统一返回 404，不区分两种情况以免泄漏仓位主键是否有效。
async fn get_position(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(position_id): Path<u64>,
) -> AppResult<Json<MarginPositionDetailResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    Ok(Json(
        get_user_margin_position_use_case(&mysql_pool(&state)?, user_id, position_id).await?,
    ))
}

/// 按最新服务端行情实时计算仓位风险；逐仓保持原有顶层单仓字段。
/// 全仓另返回同用户、同保证金资产下全部已成交持仓的权威 `cross_account_risk`，
/// 并按被查 pair 求条件强平价，其他 pair 标记价固定。每个唯一 pair 只取一次 Redis，
/// 任一行情缺失、超过六十秒或价格非正都会使整个账户快照失败。
/// 要求仓位当前为 opened 且已有入场价；结果随行情变化，重复调用不保证返回相同数值。
/// 只做估算，不写余额也不触发强平，真正的处置由 `margin_liquidation` worker 独立执行。
async fn get_position_risk(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(position_id): Path<u64>,
) -> AppResult<Json<MarginRiskSnapshotResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    Ok(Json(
        get_margin_position_risk_snapshot_use_case(
            &mysql_pool(&state)?,
            state.redis.as_ref(),
            user_id,
            position_id,
        )
        .await?,
    ))
}

/// 杠杆市价/限价开仓入口，是本模块风险最高的资金写路径，同时依赖连接池、Redis 行情和事件广播中心。
/// 请求必须携带幂等键；用例先按键做只读重放检查，再在事务内锁产品、写仓位占键并扣抵押。
/// 市价单禁止 price；限价单要求正数 price 但它只是触发阈值，两者的真实成交价都一律取服务端新鲜 ticker，trigger_price 始终禁止。
/// 未触发限价单只冻结抵押并保留空入场价；只有首次真实成交提交后才记录返佣并向用户私有频道推送开仓事件。
async fn open_position(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<OpenMarginPositionRequest>,
) -> AppResult<Json<OpenMarginPositionResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        open_margin_position_with_events_use_case(
            &pool,
            state.redis.as_ref(),
            state.event_broadcast_hub.as_ref(),
            user_id,
            request,
        )
        .await?,
    ))
}

/// 主动平掉当前用户单个杠杆仓位的指定比例，缺省空请求保持历史 100% 全平语义。
/// 显式比例必须携带幂等键；应用层按加锁后的剩余敞口和 Redis 权威标记价结算，客户端不上传金额或价格。
/// 逐仓返还非负切片权益到原资金域，全仓以有符号切片权益更新共享钱包；同键重放不重复入账或发事件。
async fn close_position(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(position_id): Path<u64>,
    request: Option<Json<CloseMarginPositionRequest>>,
) -> AppResult<Json<CloseMarginPositionResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        close_margin_position_with_events_use_case(
            &pool,
            state.redis.as_ref(),
            state.event_broadcast_hub.as_ref(),
            user_id,
            position_id,
            request.map(|Json(request)| request).unwrap_or_default(),
        )
        .await?,
    ))
}

/// 批量平掉当前用户全部已成交仓位，可用请求体里的 `product_id` 限定只平某个产品。
/// 用例按仓位主键升序逐笔独立开事务，单笔失败进入 failures 列表后继续，不回滚已成功的结算。
/// 因此本接口可能返回部分成功：调用方必须同时读 positions 和 failures 才能判断整体结果。
async fn close_all_positions(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<ProductActionRequest>,
) -> AppResult<Json<CloseAllMarginPositionsResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        close_all_margin_positions_with_events_use_case(
            &pool,
            state.redis.as_ref(),
            state.event_broadcast_hub.as_ref(),
            user_id,
            request.product_id,
        )
        .await?,
    ))
}

/// 撤销当前用户一笔尚未成交的杠杆仓位，把冻结的保证金原额退回开仓时记录的资金域。
/// 只接受 opened 且入场价为空的仓位，已成交的必须走平仓接口，用例会直接返回参数错误。
/// 不需要 Redis：撤销不涉及行情估值，因此参数里没有传入 `state.redis`。
async fn cancel_position(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(position_id): Path<u64>,
) -> AppResult<Json<CancelMarginPositionResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        cancel_margin_position_with_events_use_case(
            &pool,
            state.event_broadcast_hub.as_ref(),
            user_id,
            position_id,
        )
        .await?,
    ))
}

/// 批量撤销当前用户全部未成交仓位，可按请求体里的 `product_id` 收窄范围。
/// 与批量平仓一样逐笔独立提交并汇总失败项，区别是候选集只取入场价为空的仓位且无需行情。
/// 重复调用不会二次退款，已撤销仓位在单笔用例里按终态重放处理。
async fn cancel_all_positions(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<ProductActionRequest>,
) -> AppResult<Json<CancelAllMarginPositionsResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        cancel_all_margin_positions_with_events_use_case(
            &pool,
            state.event_broadcast_hub.as_ref(),
            user_id,
            request.product_id,
        )
        .await?,
    ))
}
