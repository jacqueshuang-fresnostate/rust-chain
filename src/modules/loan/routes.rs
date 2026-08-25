//! loan 路由层。
//!
//! 只承担“HTTP 入口 -> 应用层用例”的薄适配职责，复用既有的身份解析与参数结构。
//!
//! 本层做且只做三件事：装配依赖、把鉴权主体解析成数值编号、把用例结果包成 JSON。
//! 金额校验、KYC 判定、抵押冻结、利息计算与事务边界全部下沉在应用层与基础设施层。
//! 用户端接口一律经 `UserAuth` 并把 user_id 拼进 SQL 条件实现归属隔离；
//! 管理端写操作经 `AdminAuth`，其中审批和拒绝会把管理员编号记入订单审计字段。
//! 涉及状态迁移的接口统一返回 `LoanOrderActionResponse`，其中 `changed` 为假表示本次是幂等重放。

use crate::{
    error::AppResult,
    modules::auth::{AdminAuth, UserAuth},
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, patch, post},
};

use super::service::{admin_id_from_subject, user_id_from_subject};

use super::presentation::{
    AdminLoanOrdersQuery, AdminLoanOrdersResponse, AdminLoanProductsQuery,
    AdminLoanProductsResponse, CreateLoanOrderRequest, CreateLoanProductRequest, ListQuery,
    LoanOrderActionResponse, LoanOrderHealthResponse, LoanOrderResponse, LoanOrdersResponse,
    LoanProductResponse, LoanProductsResponse, ReviewLoanOrderRequest, UpdateLoanProductRequest,
    UpdateLoanProductStatusRequest, UserLoanOrdersQuery,
};

use super::application::{
    approve_loan_order_use_case, cancel_loan_order_use_case, create_loan_order_use_case,
    create_loan_product_use_case, get_admin_order_use_case, get_admin_product_use_case,
    get_loan_order_health_use_case, get_user_order_use_case, list_active_products_use_case,
    list_admin_orders_use_case, list_admin_products_use_case, list_user_orders_use_case,
    reject_loan_order_use_case, repay_loan_order_use_case, update_loan_product_status_use_case,
    update_loan_product_use_case,
};

/// 从 HTTP 运行时状态提取借贷用例所需的 MySQL 连接池。
///
/// 该适配只处理依赖装配，数据库未配置时返回稳定内部错误，不承载借贷业务规则。
fn mysql_pool(state: &AppState) -> AppResult<crate::state::MySqlPool> {
    state.mysql.clone().ok_or_else(|| {
        crate::error::AppError::Internal("mysql pool is not configured for loan routes".to_owned())
    })
}

/// 装配用户端借贷入口：产品列表、订单列表与创建、订单详情、取消和还款。
/// 产品列表无需登录，其余接口在处理函数内经 `UserAuth` 取得主体并转成 user_id 做归属隔离。
/// 创建订单以请求体携带的 idempotency_key 做用户级幂等，取消与还款则以订单终态本身充当幂等边界。
/// 该函数只登记路由，不接触数据库；`:id` 路径段由 axum 解析为 u64，非数值直接被提取器拒绝。
pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/loan/products", get(list_active_products))
        .route("/loan/orders", get(list_user_orders).post(create_order))
        .route("/loan/orders/:id", get(get_user_order))
        .route("/loan/orders/:id/health", get(get_order_health))
        .route("/loan/orders/:id/cancel", post(cancel_order))
        .route("/loan/orders/:id/repay", post(repay_order))
}

/// 装配管理端借贷入口：产品的增改查与启停、订单分页与详情、以及审批和拒绝两个审核动作。
/// 产品与订单的只读接口未挂 `AdminAuth` 提取器，鉴权依赖该 Router 被挂载时所在的管理端中间件层。
/// 产品创建、整体更新和状态切换三个写接口在处理函数内显式要求 `AdminAuth`，并把管理员编号传入审计事务。
/// 审批与拒绝同样要求 `AdminAuth`，并把解析出的管理员编号写入订单的 approved_by 或 rejected_by。
/// 该函数只登记路由，产品配置改动不会回溯改写既有订单已快照的利率、期限和额度。
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/loan/products",
            get(list_admin_products).post(create_product),
        )
        .route(
            "/loan/products/:id",
            get(get_admin_product).patch(update_product),
        )
        .route("/loan/products/:id/status", patch(update_product_status))
        .route("/loan/orders", get(list_admin_orders))
        .route("/loan/orders/:id", get(get_admin_order))
        .route("/loan/orders/:id/approve", post(approve_order))
        .route("/loan/orders/:id/reject", post(reject_order))
}

/// 返回面向终端用户的可申请借贷产品，固定只包含 status 为 active 的配置。
/// 无需登录，`limit` 缺省 50 并被夹紧到 1..=200，不支持偏移翻页。
/// 只读产品配置，不读取调用者的 KYC 等级、既有订单或钱包余额。
async fn list_active_products(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<LoanProductsResponse>> {
    let products = list_active_products_use_case(&mysql_pool(&state)?, query).await?;
    Ok(Json(products))
}

/// 返回后台产品分页，可按借贷类型和状态筛选，并附带与当前筛选一致的匹配总数。
/// 与用户端不同，这里不强制只看 active，disabled 产品同样可见以便运营核对。
/// loan_type 与 status 会先做枚举校验，非法取值在执行 SQL 前就返回参数错误。
/// offset 上限为十万，避免超大偏移把订单类大表拖成全表扫描。
async fn list_admin_products(
    State(state): State<AppState>,
    Query(query): Query<AdminLoanProductsQuery>,
) -> AppResult<Json<AdminLoanProductsResponse>> {
    let products = list_admin_products_use_case(&mysql_pool(&state)?, query).await?;
    Ok(Json(products))
}

/// 按编号返回单个借贷产品的当前配置及其关联资产符号，不存在时返回 NotFound。
/// 读取不加行锁，因此返回值只是即时快照，不能当作并发下单时的条款依据。
/// 下单流程会在事务内用 FOR UPDATE 重新锁定产品并使用彼时的条款。
async fn get_admin_product(
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
) -> AppResult<Json<LoanProductResponse>> {
    Ok(Json(
        get_admin_product_use_case(&mysql_pool(&state)?, product_id).await?,
    ))
}

/// 新建借贷产品配置，要求管理员身份并把 claims 中的管理员编号记录到同事务审计。
/// 请求体的类型、计息模式、状态、期限、利率、KYC 门槛和额度区间会先整体校验，
/// 名称多语言结构缺省时按简体中文自动补全，reason 裁剪后必须非空，随后按贷款资产精度校验额度。
/// 写入、revision=1 的响应回读和 before 为空的管理员审计在同一事务，任一步失败整体回滚。
/// 新配置只影响此后创建的订单，不改写任何既有订单的条款快照。
async fn create_product(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateLoanProductRequest>,
) -> AppResult<Json<LoanProductResponse>> {
    let pool = mysql_pool(&state)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_loan_product_use_case(&pool, admin_id, request).await?,
    ))
}

/// 以整体覆盖方式更新指定产品，请求体必须携带全部字段，缺字段等同于置空而非保留原值。
/// 与创建接口的差别在于 status 和客户端 revision 为必填，reason 同样必须裁剪后非空。
/// 产品编号不存在时返回 NotFound，不会退化成插入新产品。
/// 事务内锁定旧快照并执行 revision 条件更新，旧版本返回 409；配置、版本递增与审计原子提交。
/// 覆盖只作用于产品表，已创建订单快照的利率、期限、额度和抵押资金状态不受影响。
async fn update_product(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
    Json(request): Json<UpdateLoanProductRequest>,
) -> AppResult<Json<LoanProductResponse>> {
    let pool = mysql_pool(&state)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_loan_product_use_case(&pool, admin_id, product_id, request).await?,
    ))
}

/// 单独切换产品上下架状态，只接受 active 与 disabled 两个取值。
/// 相比整体更新接口，这里不要求重传利率、额度等配置，适合运营快速停售。
/// 置为 disabled 只阻断后续下单，已 pending 的订单仍可被审批，已放款订单也照常计息和还款。
/// 请求必须携带非空 reason 和客户端 revision；旧版本返回 409，状态、版本递增与审计同事务提交。
async fn update_product_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
    Json(request): Json<UpdateLoanProductStatusRequest>,
) -> AppResult<Json<LoanProductResponse>> {
    let pool = mysql_pool(&state)?;
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_loan_product_status_use_case(&pool, admin_id, product_id, request).await?,
    ))
}

/// 提交一笔借款申请，订单落库后状态为 pending，等待管理端审批才会真正放款。
/// 抵押类产品必须同时给出抵押资产和正数抵押金额，申请成功即在同一事务把抵押从 available 冻结到 frozen。
/// 用户维度取自 JWT 而非请求体，产品条款在事务内锁定产品行后快照进订单，不随后续配置改动。
/// idempotency_key 在用户维度唯一，重放会回滚本次事务并回读旧订单，响应中 `changed` 为假且不重复冻结。
/// 当前实现不校验重放请求的产品与金额是否与旧订单一致，调用方必须保证同一键只代表同一笔申请。
async fn create_order(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateLoanOrderRequest>,
) -> AppResult<Json<LoanOrderActionResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let (order, changed) =
        create_loan_order_use_case(&mysql_pool(&state)?, state.redis.as_ref(), user_id, request)
            .await?;
    Ok(Json(LoanOrderActionResponse { order, changed }))
}

/// 按订单编号倒序返回当前用户的借款订单，可用 status 过滤 pending、disbursed、repaid 等状态。
/// 用户条件由 JWT 主体固定拼入 SQL，请求参数无法覆盖，因此不存在跨用户读取。
/// 返回的是订单快照：interest_amount 与 repayment_amount 只在还款成功后才被写入，
/// 未还款订单的这两项不代表当前应计利息，本接口也不会即时计算。
async fn list_user_orders(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<UserLoanOrdersQuery>,
) -> AppResult<Json<LoanOrdersResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    Ok(Json(
        list_user_orders_use_case(&mysql_pool(&state)?, user_id, query).await?,
    ))
}

/// 返回当前用户名下单笔订单的完整详情，含产品名称、抵押资产符号和各阶段状态时间戳。
/// 归属判断放在 SQL 的 user_id 条件里，别人的订单一律按 NotFound 处理而不是返回禁止访问，
/// 因此调用方无法通过错误码区分订单不存在与订单属于他人。
/// 只读且不加行锁，不触发取消、还款或抵押释放中的任何一种状态迁移。
async fn get_user_order(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<LoanOrderResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    Ok(Json(
        get_user_order_use_case(&mysql_pool(&state)?, user_id, order_id).await?,
    ))
}

/// 基于订单固化的行情来源与最大年龄返回当前抵押率；Redis 缺失或价格陈旧时失败关闭。
async fn get_order_health(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<LoanOrderHealthResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    Ok(Json(
        get_loan_order_health_use_case(
            &mysql_pool(&state)?,
            state.redis.as_ref(),
            user_id,
            order_id,
        )
        .await?,
    ))
}

/// 由用户主动撤回一笔尚未审批的借款申请，只允许 pending 状态迁移到 cancelled。
/// 已放款、已拒绝或已还款的订单会被拒绝并返回冲突，不能借取消绕过还款义务。
/// 抵押类订单在同一事务内先把冻结的抵押从 frozen 退回 available 再改状态，两者原子提交。
/// 对已 cancelled 的订单重放直接返回原订单且 `changed` 为假，不会二次释放抵押。
async fn cancel_order(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<LoanOrderActionResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let (order, changed) =
        cancel_loan_order_use_case(&mysql_pool(&state)?, user_id, order_id).await?;
    Ok(Json(LoanOrderActionResponse { order, changed }))
}

/// 一次性结清一笔已放款或已逾期的借款，本金加利息从贷款资产 available 全额扣除。
/// 利息在请求时刻实时计算：全期模式为本金乘利率，实际天数模式再按计费天数与期限的比例折算。
/// 逾期订单同样允许还款，否则抵押资产会被永久锁死，这一点与取消接口的状态限制不同。
/// 扣款、抵押释放和订单置为 repaid 在同一事务提交；available 不足则整体回滚不留部分写入。
/// 已 repaid 的订单重放返回原订单且 `changed` 为假，不会重复扣款或重复释放抵押。
async fn repay_order(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<LoanOrderActionResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let (order, changed) =
        repay_loan_order_use_case(&mysql_pool(&state)?, user_id, order_id).await?;
    Ok(Json(LoanOrderActionResponse { order, changed }))
}

/// 返回全平台借款订单分页，支持按用户编号、邮箱模糊、产品、借贷类型和状态组合筛选。
/// 邮箱走 LIKE 前后通配匹配，其余条件均为等值匹配，空白字符串等价于不加该条件。
/// 行查询与计数查询使用同一组谓词，因此 total 始终与当前筛选口径一致。
/// 只读且不加行锁，不会触发审批、还款或抵押释放。
async fn list_admin_orders(
    State(state): State<AppState>,
    Query(query): Query<AdminLoanOrdersQuery>,
) -> AppResult<Json<AdminLoanOrdersResponse>> {
    Ok(Json(
        list_admin_orders_use_case(&mysql_pool(&state)?, query).await?,
    ))
}

/// 按编号返回任意用户的借款订单详情，供后台审核和客服排查使用，不受归属限制。
/// 相比用户端详情接口少了 user_id 条件，因此订单不存在与无权访问不再需要区分。
/// 响应含审批人、拒绝人与拒绝原因等审计字段，以及放款、到期、逾期、还清等时间点。
/// 只读且不加行锁，返回值不能作为并发审批的判断依据，审批会在事务内重新锁行。
async fn get_admin_order(
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<LoanOrderResponse>> {
    Ok(Json(
        get_admin_order_use_case(&mysql_pool(&state)?, order_id).await?,
    ))
}

/// 审批通过一笔 pending 借款并立即放款，把本金计入用户贷款资产的 available。
/// 到期时间按审批时刻加产品期限天数写入 due_at，逾期扫描任务据此判定订单是否超期。
/// 管理员编号从 JWT 主体解析后写入 approved_by，构成放款操作的审计线索。
/// 入账流水与订单状态在同一事务提交，不会出现钱包已加钱但订单仍待审核的中间态。
/// 对已 disbursed 或已 repaid 的订单重放返回 `changed` 为假，不会二次放款。
async fn approve_order(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<LoanOrderActionResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let (order, changed) = approve_loan_order_use_case(
        &mysql_pool(&state)?,
        state.redis.as_ref(),
        admin_id,
        order_id,
    )
    .await?;
    Ok(Json(LoanOrderActionResponse { order, changed }))
}

/// 驳回一笔 pending 借款申请，不发生任何本金放款，仅退回此前冻结的抵押。
/// 请求体的 reason 为可选，裁剪后为空则按未填写存入，不做长度或内容校验。
/// 管理员编号写入 rejected_by，与拒绝时间和原因共同构成审计记录。
/// 抵押从 frozen 退回 available 与订单置为 rejected 同事务提交，任一步失败整体回滚。
/// 对已 rejected 的订单重放返回 `changed` 为假，不会重复释放抵押或覆盖原有拒绝原因。
async fn reject_order(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
    Json(request): Json<ReviewLoanOrderRequest>,
) -> AppResult<Json<LoanOrderActionResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let (order, changed) =
        reject_loan_order_use_case(&mysql_pool(&state)?, admin_id, order_id, request.reason)
            .await?;
    Ok(Json(LoanOrderActionResponse { order, changed }))
}
