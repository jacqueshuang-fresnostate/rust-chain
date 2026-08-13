//! earn bounded context HTTP 路由层。
//!
//! 只做提取器解包与响应封装，金额校验、费率归一、事务边界和事件广播全在应用层。
//! 用户端三个接口都要求 `UserAuth`，其中订阅列表与赎回还会把主体解析成 user_id 做归属隔离。
//! 管理端接口全部要求 `AdminAuth`；写操作会把管理员编号连同变更前后快照写入审计日志，
//! 因此这些接口的请求体都带一个必填的 reason 字段。
//! 申购与赎回两个入口额外把 `event_broadcast_hub` 透传下去，事件只在事务提交后发布。

use super::{
    application::{
        create_earn_category, create_earn_product, get_admin_earn_category, get_admin_earn_product,
        get_admin_earn_subscription, list_active_earn_products, list_admin_earn_categories,
        list_admin_earn_products, list_admin_earn_subscriptions, list_earn_subscriptions,
        redeem_earn_subscription_with_events as redeem_earn_subscription_with_events_use_case,
        subscribe_earn_product_with_events as subscribe_earn_product_with_events_use_case,
        update_earn_category, update_earn_category_status, update_earn_product,
        update_earn_product_status,
    },
    presentation::{
        AdminCategoriesQuery, AdminEarnProductsResponse, AdminEarnSubscriptionsResponse,
        AdminProductsQuery, AdminSubscriptionsQuery, CreateEarnCategoryRequest,
        CreateEarnProductRequest, EarnCategoriesResponse, EarnCategoryResponse,
        EarnProductResponse, EarnProductsResponse, EarnSubscriptionResponse,
        EarnSubscriptionsResponse, ListQuery, RedeemEarnResponse, SubscribeEarnRequest,
        SubscribeEarnResponse, UpdateEarnCategoryRequest, UpdateEarnCategoryStatusRequest,
        UpdateEarnProductRequest, UpdateEarnProductStatusRequest,
    },
};
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

/// 装配用户端理财入口：产品列表、订阅列表与申购、以及按订阅编号赎回。
/// 三个入口都要求登录，产品列表虽不区分用户但同样挂了 `UserAuth`，未登录无法浏览在售产品。
/// 申购以请求体的 idempotency_key 做用户级幂等，赎回则以订阅的 redeemed 终态充当幂等边界。
/// 该函数只登记路由，不接触数据库、钱包或事件总线。
pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/earn/products", get(list_active_products))
        .route(
            "/earn/subscriptions",
            get(list_subscriptions).post(subscribe),
        )
        .route("/earn/subscriptions/:id/redeem", post(redeem_subscription))
}

/// 装配管理端理财入口：分类与产品各自的列表、详情、创建、整体更新和状态切换，加上订阅的只读查询。
/// 分类与产品共八个配置接口，五个写接口都会在同一事务内追加带前后快照的管理员审计记录。
/// 订阅侧只提供列表与详情两个只读接口，后台无法代替用户申购或赎回。
/// 所有处理函数各自显式要求 `AdminAuth`，写接口还会用其中的管理员编号作为审计主体。
/// 该函数只登记路由，配置改动不会回溯影响任何既有订阅的费率快照。
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/earn/categories",
            get(list_admin_categories).post(create_category),
        )
        .route(
            "/earn/categories/:id",
            get(get_admin_category).patch(update_category),
        )
        .route("/earn/categories/:id/status", patch(update_category_status))
        .route(
            "/earn/products",
            get(list_admin_products).post(create_product),
        )
        .route(
            "/earn/products/:id",
            get(get_admin_product).patch(update_product),
        )
        .route("/earn/products/:id/status", patch(update_product_status))
        .route("/earn/subscriptions", get(list_admin_subscriptions))
        .route("/earn/subscriptions/:id", get(get_admin_subscription))
}

/// 返回可申购的理财产品，状态过滤在应用层硬编码为 active，已下架产品对用户不可见。
/// 需要登录但不区分用户，claims 只用于准入，返回内容对所有已登录用户一致。
/// 响应含 APR、期限、额度区间和四项费率的当前配置，以及分类的多语言名称。
/// 只读且不支持偏移翻页，limit 缺省 50 并夹紧到 1..=100。
async fn list_active_products(
    UserAuth(_claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<EarnProductsResponse>> {
    Ok(Json(
        list_active_earn_products(state.mysql.clone(), query).await?,
    ))
}

/// 返回后台理财产品分页与匹配总数，不做状态过滤，已下架产品同样可见。
/// 与用户端相比多了 offset 翻页能力，偏移上限十万以避免深分页拖垮查询。
/// 只读接口，因此不要求 reason，也不写入任何审计记录。
async fn list_admin_products(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminProductsQuery>,
) -> AppResult<Json<AdminEarnProductsResponse>> {
    Ok(Json(
        list_admin_earn_products(state.mysql.clone(), query).await?,
    ))
}

/// 按编号返回单个理财产品的完整配置，分类名称缺失时回退显示原始分类代码。
/// 读取走一个短只读事务，但不加行锁，因此返回值不能作为并发申购的条款依据。
/// 申购流程会在自己的事务里用 FOR UPDATE 重新锁定产品并快照彼时的费率。
async fn get_admin_product(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
) -> AppResult<Json<EarnProductResponse>> {
    Ok(Json(
        get_admin_earn_product(state.mysql.clone(), product_id).await?,
    ))
}

/// 返回当前用户的理财订阅列表，按创建时间倒序，不支持状态过滤和偏移翻页。
/// 用户维度由 JWT 主体解析而来并固定拼入 SQL，请求参数无法覆盖。
/// 响应中的 APR 与四项费率是申购时固化的快照，不随产品配置变更，也不实时计算当前收益。
async fn list_subscriptions(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<EarnSubscriptionsResponse>> {
    Ok(Json(
        list_earn_subscriptions(state.mysql.clone(), &claims.sub, query).await?,
    ))
}

/// 返回全平台订阅分页与总数，可按用户编号、邮箱和订阅状态组合筛选。
/// 邮箱走精确等值匹配的 EXISTS 子查询而非模糊匹配，必须填完整邮箱才能命中。
/// 只读接口，不加行锁、不计算收益、不触发赎回，费率字段同样是申购时的快照。
async fn list_admin_subscriptions(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminSubscriptionsQuery>,
) -> AppResult<Json<AdminEarnSubscriptionsResponse>> {
    Ok(Json(
        list_admin_earn_subscriptions(state.mysql.clone(), query).await?,
    ))
}

/// 按编号返回任意用户的订阅详情，供后台核对费率快照与到期、赎回时间点。
/// 不带用户条件，因此不需要区分订阅不存在与无权访问。
/// 走短只读事务且不加行锁，返回的仍是订阅行原值，不重算当前应计收益。
async fn get_admin_subscription(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(subscription_id): Path<u64>,
) -> AppResult<Json<EarnSubscriptionResponse>> {
    Ok(Json(
        get_admin_earn_subscription(state.mysql.clone(), subscription_id).await?,
    ))
}

/// 返回理财产品分类分页与总数，可按启停状态筛选，排序为 sort_order 升序再按编号升序。
/// 与产品和订阅列表的倒序排列不同，分类按运营配置的权重正序展示。
/// default_name 由 SQL 从多语言结构取首个条目标题，取不到时回退为分类代码。
/// 只读接口，不锁分类，也不会因为没有关联产品而改写分类状态。
async fn list_admin_categories(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminCategoriesQuery>,
) -> AppResult<Json<EarnCategoriesResponse>> {
    Ok(Json(
        list_admin_earn_categories(state.mysql.clone(), query).await?,
    ))
}

/// 按编号返回单个分类的代码、多语言名称、排序权重与启停状态。
/// 走短只读事务且不加行锁，不产生审计记录，编号不存在时返回 NotFound。
/// 分类代码创建后不可变更，因此该字段可安全用作产品侧的稳定引用。
async fn get_admin_category(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(category_id): Path<u64>,
) -> AppResult<Json<EarnCategoryResponse>> {
    Ok(Json(
        get_admin_earn_category(state.mysql.clone(), category_id).await?,
    ))
}

/// 新建理财产品分类，代码只允许字母、数字、下划线和连字符且不超过 64 字符。
/// status 缺省为 active，sort_order 缺省为 0，多语言名称缺省时按代码生成中文兜底条目。
/// reason 为必填，管理员编号取自 JWT，二者与新建后的完整快照一并写入审计日志。
/// 分类写入与审计在同一事务提交；代码重复会返回冲突且不留下任何记录。
async fn create_category(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateEarnCategoryRequest>,
) -> AppResult<Json<EarnCategoryResponse>> {
    Ok(Json(
        create_earn_category(state.mysql.clone(), &claims.sub, request).await?,
    ))
}

/// 更新分类的多语言名称、排序权重和启停状态；分类代码不可修改，请求体中也不接受该字段。
/// 事务内先 FOR UPDATE 锁定旧行取得前快照，名称缺省时用锁到的旧代码生成兜底条目。
/// 更新后重新读回作为后快照，与前快照、管理员编号和必填 reason 一起写入审计。
/// 配置改动与审计原子提交，任一步失败都回滚，不会出现无审计记录的分类变更。
async fn update_category(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(category_id): Path<u64>,
    Json(request): Json<UpdateEarnCategoryRequest>,
) -> AppResult<Json<EarnCategoryResponse>> {
    Ok(Json(
        update_earn_category(state.mysql.clone(), &claims.sub, category_id, request).await?,
    ))
}

/// 只切换分类的启停状态，相比整体更新无需重传名称与排序。
/// 置为 disabled 后新产品不能再引用该分类，但已引用它的存量产品不受影响，仍可正常展示与申购。
/// 同样先锁行取前快照，改完再读后快照，与管理员编号和必填 reason 一并落审计。
/// 状态变更与审计共用同一事务，提交失败不会留下未审计的启停结果。
async fn update_category_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(category_id): Path<u64>,
    Json(request): Json<UpdateEarnCategoryStatusRequest>,
) -> AppResult<Json<EarnCategoryResponse>> {
    Ok(Json(
        update_earn_category_status(state.mysql.clone(), &claims.sub, category_id, request).await?,
    ))
}

/// 新建理财产品，落库前依次校验名称长度、期限上限、APR 与三项费率的取值范围和小数位。
/// 分类缺省回退到 fixed_term，介绍缺省按产品名生成中文兜底富文本，图片地址可空但限长。
/// 提前赎回费基准为 none 时，费率会被强制归零，避免配置出无法生效的费率。
/// 事务内先确认资产存在、分类存在且处于 active，再插入产品并读回完整快照写入审计。
/// 产品写入与审计原子提交，本接口不创建订阅也不移动任何用户余额。
async fn create_product(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateEarnProductRequest>,
) -> AppResult<Json<EarnProductResponse>> {
    Ok(Json(
        create_earn_product(state.mysql.clone(), &claims.sub, request).await?,
    ))
}

/// 整体覆盖理财产品配置，请求体必须携带全部字段，缺字段等同于置空而非保留旧值。
/// 与创建接口的差别只在 status 为必填，其余校验口径完全一致以避免两条入口宽严不一。
/// 事务内先 FOR UPDATE 锁定产品取前快照，再校验资产与目标分类，然后覆盖并读回后快照。
/// 最关键的语义是：修改费率只影响此后新建的订阅，既有订阅仍按申购时复制的快照结算。
/// 配置更新与审计原子提交，任一步失败都保留原有产品配置。
async fn update_product(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
    Json(request): Json<UpdateEarnProductRequest>,
) -> AppResult<Json<EarnProductResponse>> {
    Ok(Json(
        update_earn_product(state.mysql.clone(), &claims.sub, product_id, request).await?,
    ))
}

/// 只切换理财产品的上下架状态，取值限于 active 与 disabled。
/// 置为 disabled 会立即让该产品从用户端列表消失并阻断新申购，但不影响存量订阅。
/// 已持有的订阅照常计息，到期后仍可正常赎回，自动赎回任务也不受产品状态影响。
/// 先锁产品取前快照，改完读后快照，与管理员编号和必填 reason 一并写入审计后同事务提交。
async fn update_product_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(product_id): Path<u64>,
    Json(request): Json<UpdateEarnProductStatusRequest>,
) -> AppResult<Json<EarnProductResponse>> {
    Ok(Json(
        update_earn_product_status(state.mysql.clone(), &claims.sub, product_id, request).await?,
    ))
}

/// 申购理财产品：从贷记资产的 available 扣除本金并创建一笔 subscribed 状态的订阅。
/// 订阅落库时把产品的 APR、期限和四项费率逐一复制为快照，此后产品改配置不影响本笔收益结算。
/// 到期时刻按申购时的 UTC 当前时间加产品期限天数计算并一并写入订阅。
/// 扣款只动 available，frozen 与 locked 不变，同时写一条 earn_subscribe 负流水引用订阅编号。
/// idempotency_key 在用户维度唯一，重放必须与原请求的产品和金额完全一致，否则返回冲突；
/// 一致时直接返回旧订阅且不二次扣款。首次申购成功后才在事务提交后发布用户私有事件。
async fn subscribe(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<SubscribeEarnRequest>,
) -> AppResult<Json<SubscribeEarnResponse>> {
    Ok(Json(
        subscribe_earn_product_with_events_use_case(
            state.mysql.clone(),
            state.event_broadcast_hub.as_ref(),
            &claims.sub,
            request,
        )
        .await?,
    ))
}

/// 赎回一笔订阅，允许在到期前提前赎回，两种情形使用不同的计息与费用口径。
/// 全部算式只依赖订阅自身的快照，与产品当前配置无关；到期按整期计息，提前按实际持有秒数计息。
/// 净到账额为本金加毛收益减去通用赎回费、到期利润手续费和提前赎回费，下限为零。
/// 事务锁序为先锁订阅再锁钱包，净额计入 available 并写一条 earn_redeem 正流水，
/// 三类费用不单独生成钱包流水，只体现在响应的明细字段中。
/// 只有 subscribed 状态可赎回；已 redeemed 的订阅重放会从历史流水恢复金额，不再计息也不重复入账。
async fn redeem_subscription(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Path(subscription_id): Path<u64>,
) -> AppResult<Json<RedeemEarnResponse>> {
    Ok(Json(
        redeem_earn_subscription_with_events_use_case(
            state.mysql.clone(),
            state.event_broadcast_hub.as_ref(),
            &claims.sub,
            subscription_id,
        )
        .await?,
    ))
}
