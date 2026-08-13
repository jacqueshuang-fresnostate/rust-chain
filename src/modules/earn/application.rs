//! earn bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。
//!
//! 这里定义理财的三条事务骨架。配置类用例：开启事务、锁定目标行取前快照、写入、
//! 读回后快照、追加管理员审计，最后一次性提交，因此不存在没有审计记录的后台改动。
//! 申购用例：先在事务外只读探一次幂等键快速返回重放，未命中才开事务，
//! 锁产品、插订阅、锁钱包、扣 available，唯一键冲突则回滚并回读旧订阅。
//! 赎回用例：锁订阅、判状态、按订阅快照算金额、锁钱包、入账、置为 redeemed。
//! 两条资金用例的锁序都是「业务行在前、钱包在后」，且事件一律在事务提交成功后才发布，
//! 广播失败不回滚已提交的资金变更。

use crate::{
    error::{AppError, AppResult},
    modules::{
        earn::{
            infrastructure,
            presentation::{
                AdminCategoriesQuery, AdminEarnProductsResponse, AdminEarnSubscriptionsResponse,
                AdminProductsQuery, AdminSubscriptionsQuery, CreateEarnCategoryRequest,
                CreateEarnProductRequest, EarnCategoriesResponse, EarnCategoryResponse,
                EarnProductResponse, EarnProductsResponse, EarnSubscriptionResponse,
                EarnSubscriptionsResponse, ListQuery, RedeemEarnResponse, SubscribeEarnRequest,
                SubscribeEarnResponse, UpdateEarnCategoryRequest, UpdateEarnCategoryStatusRequest,
                UpdateEarnProductRequest, UpdateEarnProductStatusRequest,
            },
            repository::{EarnCategoryWrite, EarnProductWrite},
            service::{
                admin_id_from_subject, category_audit_json, earn_matures_at,
                ensure_existing_subscription_matches_request, normalize_idempotency_key,
                normalized_category_name_json, normalized_category_status,
                normalized_introduction_json, normalized_product_category,
                normalized_product_status, normalized_required_category_code, optional_image_url,
                optional_string, product_audit_json, product_fee_config_from_create_request,
                product_fee_config_from_update_request, redemption_amounts_for_subscription,
                required_reason, route_limit, route_offset, user_id_from_subject, validate_amount,
                validate_create_product_request, validate_product_amount,
                validate_update_product_request,
            },
        },
        events::{EventBroadcastHub, EventBroadcastMessage},
    },
};
use bigdecimal::BigDecimal;
use chrono::Utc;
use serde_json::json;
use sqlx::{MySql, Pool};

/// 按产品编号倒序列出可申购的理财产品，状态过滤硬编码为 active，下架产品对用户不可见。
/// 数量默认 50、夹紧到 1..=100，不支持偏移翻页，产品较多时只能看到编号最大的一批。
/// 返回的费率是产品当前配置，仅供展示；真正参与结算的是申购时复制进订阅的快照。
/// 不读取调用者的订阅记录或钱包余额，因此无法据此判断用户是否有足够余额申购。
pub(crate) async fn list_active_earn_products(
    pool: Option<Pool<MySql>>,
    query: ListQuery,
) -> AppResult<EarnProductsResponse> {
    let pool = earn_mysql_pool(pool)?;
    infrastructure::list_products(&pool, Some("active"), route_limit(query.limit)).await
}

/// 读取后台理财产品分页与总数，状态参数固定传 None，因此上下架产品一并返回。
/// 与用户端相比多了偏移翻页能力，limit 夹紧到 1..=100，offset 截断到十万。
/// 行查询与计数查询由同一组谓词构建，total 始终与列表口径一致而非全表行数。
/// 该只读用例不锁产品或分类，也不改写任何已有订阅的费用快照。
pub(crate) async fn list_admin_earn_products(
    pool: Option<Pool<MySql>>,
    query: AdminProductsQuery,
) -> AppResult<AdminEarnProductsResponse> {
    let pool = earn_mysql_pool(pool)?;
    let (products, total) = infrastructure::list_admin_products(
        &pool,
        None,
        route_limit(query.limit),
        route_offset(query.offset),
    )
    .await?;
    Ok(AdminEarnProductsResponse { products, total })
}

/// 在一个短只读事务中加载指定理财产品的完整配置，编号不存在时返回未找到。
/// 之所以包一层事务，是为了复用基础设施层只接受 `Transaction` 的加载函数，而非出于一致性需要。
/// 事务内不加行锁，因此返回值只是即时快照，不能作为并发申购的条款依据。
/// 只读用例不写审计，也不重算历史订阅收益或触发任何钱包资金变化。
pub(crate) async fn get_admin_earn_product(
    pool: Option<Pool<MySql>>,
    product_id: u64,
) -> AppResult<EarnProductResponse> {
    let pool = earn_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let product = infrastructure::load_product_by_id(&mut tx, product_id).await?;
    tx.commit().await?;
    Ok(product)
}

/// 从鉴权主体解析用户后列出其理财订阅，user_id 固定拼入 SQL 条件，结果不含他人记录。
/// 按创建时间倒序再按编号倒序，只支持限制条数，不提供状态过滤和偏移翻页。
/// APR 与四项费率一律返回订阅行的持久化快照，不会用产品当前配置替换。
/// 查询不加行锁、不锁钱包，也不实时计算持有期收益，更不会触发赎回。
pub(crate) async fn list_earn_subscriptions(
    pool: Option<Pool<MySql>>,
    subject: &str,
    query: ListQuery,
) -> AppResult<EarnSubscriptionsResponse> {
    let user_id = user_id_from_subject(subject)?;
    let pool = earn_mysql_pool(pool)?;
    infrastructure::list_user_subscriptions(&pool, user_id, route_limit(query.limit)).await
}

/// 归一邮箱与状态两项文本筛选后查询后台订阅分页与总数，用户编号为数值无需归一。
/// 空白筛选值被折成不过滤，未知状态不报错只是查不到数据，本层不做枚举校验。
/// 邮箱走 EXISTS 子查询的精确等值匹配而非模糊匹配，必须填完整邮箱才能命中。
/// 三项筛选之间为「与」关系，全部为空时退化为全量分页。
/// 该只读用例不锁订阅或钱包，也不触发赎回与收益计算，费率字段仍是申购时的快照。
pub(crate) async fn list_admin_earn_subscriptions(
    pool: Option<Pool<MySql>>,
    query: AdminSubscriptionsQuery,
) -> AppResult<AdminEarnSubscriptionsResponse> {
    let pool = earn_mysql_pool(pool)?;
    let (subscriptions, total) = infrastructure::list_admin_subscriptions(
        &pool,
        route_limit(query.limit),
        route_offset(query.offset),
        query.user_id,
        optional_string(query.email),
        optional_string(query.status),
    )
    .await?;
    Ok(AdminEarnSubscriptionsResponse {
        subscriptions,
        total,
    })
}

/// 在一个短只读事务中按编号加载订阅详情，不带用户条件，因此后台可查看任意用户的订阅。
/// 加载函数不加行锁，返回的是订阅行原值，包含申购时固化的费率快照与各阶段时间点。
/// 编号不存在时返回未找到；该用例不写审计、不计息、不改任何状态。
pub(crate) async fn get_admin_earn_subscription(
    pool: Option<Pool<MySql>>,
    subscription_id: u64,
) -> AppResult<EarnSubscriptionResponse> {
    let pool = earn_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let subscription = infrastructure::load_subscription_by_id(&mut tx, subscription_id).await?;
    tx.commit().await?;
    Ok(subscription)
}

/// 按可选启停状态读取理财分类分页与总数，排序为 sort_order 升序再按编号升序。
/// 与产品和订阅列表的倒序不同，分类按运营配置的权重正序展示，便于控制前端展示顺序。
/// 状态筛选先裁剪归一，空白等价于不过滤，本层不校验枚举合法性。
/// 查询不锁分类或产品，也不会因为某个分类没有关联产品就改写其状态。
pub(crate) async fn list_admin_earn_categories(
    pool: Option<Pool<MySql>>,
    query: AdminCategoriesQuery,
) -> AppResult<EarnCategoriesResponse> {
    let pool = earn_mysql_pool(pool)?;
    let (categories, total) = infrastructure::list_admin_categories(
        &pool,
        route_limit(query.limit),
        route_offset(query.offset),
        optional_string(query.status),
    )
    .await?;
    Ok(EarnCategoriesResponse { categories, total })
}

/// 在一个短只读事务中加载分类详情，使用不加锁的读取而非配置更新路径上的 FOR UPDATE 版本。
/// 因此返回值不能用作后续写入的前快照，更新用例会自行重新锁行取值。
/// 响应中的 default_name 由 SQL 取多语言结构首个条目标题，取不到时回退为分类代码。
/// 该读取不触发管理员审计，编号不存在时直接返回未找到且无任何副作用。
pub(crate) async fn get_admin_earn_category(
    pool: Option<Pool<MySql>>,
    category_id: u64,
) -> AppResult<EarnCategoryResponse> {
    let pool = earn_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let category = infrastructure::load_category_by_id(&mut tx, category_id).await?;
    tx.commit().await?;
    Ok(category)
}

/// 创建理财分类：先在事务外完成全部纯校验与缺省填充，再开事务写入并追加审计。
/// 代码必须显式给出且符合字符集与长度约束，status 缺省 active，sort_order 缺省 0，
/// 多语言名称缺省时按分类代码生成简体中文兜底条目。
/// reason 在此为必填，与管理员编号和新建后的完整快照一起写入审计日志，前快照为空。
/// 分类写入与审计共用同一事务原子提交，任一步失败都不会留下分类或孤立的审计记录。
/// 代码重复由唯一约束拦截并转为冲突错误，本用例不做「已存在则返回旧行」的幂等处理。
pub(crate) async fn create_earn_category(
    pool: Option<Pool<MySql>>,
    subject: &str,
    request: CreateEarnCategoryRequest,
) -> AppResult<EarnCategoryResponse> {
    let code = normalized_required_category_code(&request.code)?;
    let status = normalized_category_status(request.status.as_deref().unwrap_or("active"))?;
    let name_json = normalized_category_name_json(request.name_json, &code)?;
    let sort_order = request.sort_order.unwrap_or(0);
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(subject)?;
    let pool = earn_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let category_id = infrastructure::insert_category_in_tx(
        &mut tx,
        &EarnCategoryWrite {
            code,
            name_json,
            sort_order,
            status,
        },
    )
    .await?;
    let category = infrastructure::load_category_by_id(&mut tx, category_id).await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "earn_category.create",
        "earn_category",
        category.id,
        None,
        Some(category_audit_json(&category)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(category)
}

/// 更新分类的多语言名称、排序权重和启停状态；分类代码不可变，写回时原样沿用锁到的旧值。
/// 事务内先 FOR UPDATE 锁定旧行取得前快照，这样并发更新会被串行化，审计不会出现错配的前后值。
/// 多语言名称缺省时用锁到的旧代码生成兜底条目，而非用请求体中不存在的代码。
/// 更新后重新读回作为后快照，与前快照、管理员编号和必填 reason 一并写入审计。
/// 配置改动与审计原子提交，任一步失败全部回滚，不会出现无审计记录的分类变更。
pub(crate) async fn update_earn_category(
    pool: Option<Pool<MySql>>,
    subject: &str,
    category_id: u64,
    request: UpdateEarnCategoryRequest,
) -> AppResult<EarnCategoryResponse> {
    let status = normalized_category_status(&request.status)?;
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(subject)?;
    let pool = earn_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = infrastructure::lock_category_by_id(&mut tx, category_id).await?;
    let name_json = normalized_category_name_json(request.name_json, &before.code)?;
    infrastructure::update_category_in_tx(
        &mut tx,
        category_id,
        &EarnCategoryWrite {
            code: before.code.clone(),
            name_json,
            sort_order: request.sort_order,
            status,
        },
    )
    .await?;
    let after = infrastructure::load_category_by_id(&mut tx, category_id).await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "earn_category.update",
        "earn_category",
        category_id,
        Some(category_audit_json(&before)),
        Some(category_audit_json(&after)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

/// 只切换分类启停状态，相比整体更新无需重传名称与排序，适合运营快速上下架某一类产品。
/// 同样先 FOR UPDATE 锁行取前快照，改完读回后快照，与管理员编号和必填 reason 一并落审计。
/// 置为 disabled 只阻断新产品引用该分类，已引用它的存量产品照常展示与申购。
/// 状态变更与审计共用同一事务，提交失败不会留下未审计的启停结果。
pub(crate) async fn update_earn_category_status(
    pool: Option<Pool<MySql>>,
    subject: &str,
    category_id: u64,
    request: UpdateEarnCategoryStatusRequest,
) -> AppResult<EarnCategoryResponse> {
    let status = normalized_category_status(&request.status)?;
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(subject)?;
    let pool = earn_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = infrastructure::lock_category_by_id(&mut tx, category_id).await?;
    infrastructure::update_category_status_in_tx(&mut tx, category_id, &status).await?;
    let after = infrastructure::load_category_by_id(&mut tx, category_id).await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "earn_category.update_status",
        "earn_category",
        category_id,
        Some(category_audit_json(&before)),
        Some(category_audit_json(&after)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

/// 创建理财产品：先在事务外完成全部纯校验与缺省填充，再开事务校验引用、写入并追加审计。
/// 校验顺序为字段级规则、四项费率归一、必填 reason、状态与分类缺省、图片长度、富文本介绍结构。
/// 名称在此裁剪一次并作为介绍缺省生成的依据，因此兜底介绍用的是裁剪后的名称。
/// 事务内先确认资产存在、分类存在且处于 active，两项引用完整性检查失败即整体回滚。
/// 产品写入后读回完整快照作为审计的后快照，前快照为空；产品与审计原子提交。
/// 本用例只写配置，不创建订阅、不移动任何用户余额，费率此刻定稿并将在申购时被复制进订阅。
pub(crate) async fn create_earn_product(
    pool: Option<Pool<MySql>>,
    subject: &str,
    request: CreateEarnProductRequest,
) -> AppResult<EarnProductResponse> {
    validate_create_product_request(&request)?;
    let fee_config = product_fee_config_from_create_request(&request)?;
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(subject)?;
    let status = normalized_product_status(request.status.as_deref().unwrap_or("active"))?;
    let category = normalized_product_category(request.category.as_deref())?;
    let banner_url = optional_image_url(request.banner_url, "earn product banner_url")?;
    let small_logo_url = optional_image_url(request.small_logo_url, "earn product small_logo_url")?;
    let name = request.name.trim().to_owned();
    let introduction_json = normalized_introduction_json(request.introduction_json, &name)?;
    let write = EarnProductWrite {
        asset_id: request.asset_id,
        name,
        banner_url,
        small_logo_url,
        category,
        introduction_json,
        term_days: request.term_days,
        apr_rate: request.apr_rate,
        redemption_fee_rate: fee_config.redemption_fee_rate,
        maturity_profit_fee_rate: fee_config.maturity_profit_fee_rate,
        early_redeem_fee_basis: fee_config.early_redeem_fee_basis,
        early_redeem_fee_rate: fee_config.early_redeem_fee_rate,
        min_subscribe: request.min_subscribe,
        max_subscribe: request.max_subscribe,
        status,
    };
    let pool = earn_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    infrastructure::ensure_asset_exists(&mut tx, write.asset_id).await?;
    infrastructure::ensure_active_category_exists(&mut tx, &write.category).await?;
    let product_id = infrastructure::insert_product_in_tx(&mut tx, &write).await?;
    let product = infrastructure::load_product_by_id(&mut tx, product_id).await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "earn_product.create",
        "earn_product",
        product.id,
        None,
        Some(product_audit_json(&product)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(product)
}

/// 整体覆盖理财产品配置，语义是替换而非合并：请求体未给出的可选字段按各自缺省规则重新取值。
/// 校验与创建完全同源，差别只在 status 为必填，从而避免两条入口出现宽严不一的口径。
/// 事务内先 FOR UPDATE 锁定产品取前快照，再校验资产与目标分类，然后覆盖并读回后快照。
/// 该用例最关键的语义是费率快照的边界：修改后的费率只对此后新建的订阅生效，
/// 既有订阅仍按申购时复制进 `earn_subscriptions` 的那份费率结算，本次修改不会回溯改写。
/// 配置更新与审计原子提交，任一步失败都保留原有产品配置且不留审计记录。
pub(crate) async fn update_earn_product(
    pool: Option<Pool<MySql>>,
    subject: &str,
    product_id: u64,
    request: UpdateEarnProductRequest,
) -> AppResult<EarnProductResponse> {
    validate_update_product_request(&request)?;
    let fee_config = product_fee_config_from_update_request(&request)?;
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(subject)?;
    let status = normalized_product_status(&request.status)?;
    let category = normalized_product_category(request.category.as_deref())?;
    let banner_url = optional_image_url(request.banner_url, "earn product banner_url")?;
    let small_logo_url = optional_image_url(request.small_logo_url, "earn product small_logo_url")?;
    let name = request.name.trim().to_owned();
    let introduction_json = normalized_introduction_json(request.introduction_json, &name)?;
    let write = EarnProductWrite {
        asset_id: request.asset_id,
        name,
        banner_url,
        small_logo_url,
        category,
        introduction_json,
        term_days: request.term_days,
        apr_rate: request.apr_rate,
        redemption_fee_rate: fee_config.redemption_fee_rate,
        maturity_profit_fee_rate: fee_config.maturity_profit_fee_rate,
        early_redeem_fee_basis: fee_config.early_redeem_fee_basis,
        early_redeem_fee_rate: fee_config.early_redeem_fee_rate,
        min_subscribe: request.min_subscribe,
        max_subscribe: request.max_subscribe,
        status,
    };
    let pool = earn_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = infrastructure::lock_product_by_id(&mut tx, product_id).await?;
    infrastructure::ensure_asset_exists(&mut tx, write.asset_id).await?;
    infrastructure::ensure_active_category_exists(&mut tx, &write.category).await?;
    infrastructure::update_product_in_tx(&mut tx, product_id, &write).await?;
    let after = infrastructure::load_product_by_id(&mut tx, product_id).await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "earn_product.update",
        "earn_product",
        product_id,
        Some(product_audit_json(&before)),
        Some(product_audit_json(&after)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

/// 只切换理财产品的上下架状态，无需重传费率、期限和额度等完整配置。
/// 事务内先 FOR UPDATE 锁定产品取前快照，更新后读回后快照，与管理员编号和必填 reason 一并落审计。
/// 置为 disabled 会让产品从用户端列表消失并阻断新申购，存量订阅则完全不受影响：
/// 照常按各自快照计息，到期后仍可赎回，自动赎回任务也不检查产品状态。
/// 状态更新与审计同事务提交，不会出现无审计记录的上下架操作。
pub(crate) async fn update_earn_product_status(
    pool: Option<Pool<MySql>>,
    subject: &str,
    product_id: u64,
    request: UpdateEarnProductStatusRequest,
) -> AppResult<EarnProductResponse> {
    let status = normalized_product_status(&request.status)?;
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(subject)?;
    let pool = earn_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = infrastructure::lock_product_by_id(&mut tx, product_id).await?;
    infrastructure::update_product_status_in_tx(&mut tx, product_id, &status).await?;
    let after = infrastructure::load_product_by_id(&mut tx, product_id).await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "earn_product.update_status",
        "earn_product",
        product_id,
        Some(product_audit_json(&before)),
        Some(product_audit_json(&after)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

/// 创建理财订阅时先快速查询用户幂等键；未命中后由内部事务锁 active 产品、插订阅，再锁钱包。
/// 申购从 available 扣除本金，frozen/locked 不变，只写一条 `earn_subscribe` available 负流水并引用 subscription id。
/// 订阅保存产品 APR、期限和全部费用快照；金额按数据库 18 位规则校验，不按资产 precision_scale 另行截断。
/// 用户幂等键重放须匹配产品和金额，匹配时返回旧订阅且不二次扣款；唯一键并发冲突回滚当前事务后回读旧记录。
/// 私有事件仅在资金事务提交且确为新订阅后发布；未配置或无人接收广播不回滚已提交申购。
pub(crate) async fn subscribe_earn_product_with_events(
    pool: Option<Pool<MySql>>,
    event_broadcast_hub: Option<&EventBroadcastHub>,
    subject: &str,
    request: SubscribeEarnRequest,
) -> AppResult<SubscribeEarnResponse> {
    // 应用层负责提交订阅后事件的统一编排，路由层只负责请求参数与鉴权。
    let user_id = user_id_from_subject(subject)?;
    let (response, is_new_subscription) =
        subscribe_earn_product_with_internal(pool, subject, request).await?;
    if is_new_subscription && let Some(hub) = event_broadcast_hub {
        hub.publish(EventBroadcastMessage::private_user(
            user_id,
            json!({
                "type": "earn.subscription.created",
                "subscription_id": response.subscription.id,
                "product_id": response.subscription.product_id,
                "asset_id": response.subscription.asset_id,
                "amount": response.subscription.amount,
                "status": response.subscription.status,
            })
            .to_string(),
        ));
    }
    Ok(response)
}

/// 申购的内部入口，负责鉴权解析、幂等键归一与金额存储精度校验，再交给资金事务执行。
/// 相比对外版本额外返回是否为新建订阅的布尔值，供调用方判断该不该发布事件。
/// 金额此处只按数据库 18 位存储口径校验，产品额度区间的判定要等锁定产品后才进行。
/// 拆出该层是为了让事件发布与资金逻辑解耦，本函数自身不广播任何消息。
async fn subscribe_earn_product_with_internal(
    pool: Option<Pool<MySql>>,
    subject: &str,
    request: SubscribeEarnRequest,
) -> AppResult<(SubscribeEarnResponse, bool)> {
    let user_id = user_id_from_subject(subject)?;
    let idempotency_key = normalize_idempotency_key(&request.idempotency_key)?;
    validate_amount(&request.amount)?;
    let pool = earn_mysql_pool(pool)?;
    let (subscription, is_new_subscription) = subscribe_in_tx(
        &pool,
        user_id,
        request.product_id,
        request.amount,
        idempotency_key,
    )
    .await?;
    Ok((SubscribeEarnResponse { subscription }, is_new_subscription))
}

/// 赎回只使用订阅时 APR、期限和费用快照计算本金、毛收益、赎回费、到期收益费及提前赎回费。
/// 内部事务锁序为订阅→钱包；净赎回额增加 available，frozen/locked 不变，只写一条 `earn_redeem` 正流水。
/// 订阅状态、余额与流水同事务提交；已 redeemed 重放从最早申购/赎回流水恢复本金与净到账，不再次计息或入账。
/// 私有事件只对首次赎回在提交后发布；广播缺失或无人接收不回滚已提交余额和订阅状态。
pub(crate) async fn redeem_earn_subscription_with_events(
    pool: Option<Pool<MySql>>,
    event_broadcast_hub: Option<&EventBroadcastHub>,
    subject: &str,
    subscription_id: u64,
) -> AppResult<RedeemEarnResponse> {
    // 应用层负责赎回成功后的事件推送，路由层只透传上下文与 idempotency 结果。
    let user_id = user_id_from_subject(subject)?;
    let (response, is_new_redemption) =
        redeem_earn_subscription_with_internal(pool, subject, subscription_id).await?;
    if is_new_redemption && let Some(hub) = event_broadcast_hub {
        hub.publish(EventBroadcastMessage::private_user(
            user_id,
            json!({
                "type": "earn.subscription.redeemed",
                "subscription_id": response.subscription.id,
                "product_id": response.subscription.product_id,
                "asset_id": response.subscription.asset_id,
                "principal_amount": response.principal_amount,
                "gross_yield_amount": response.gross_yield_amount,
                "yield_amount": response.yield_amount,
                "redemption_fee_amount": response.redemption_fee_amount,
                "maturity_profit_fee_amount": response.maturity_profit_fee_amount,
                "early_redeem_fee_amount": response.early_redeem_fee_amount,
                "fee_amount": response.fee_amount,
                "redeem_amount": response.redeem_amount,
                "status": response.subscription.status,
            })
            .to_string(),
        ));
    }
    Ok(response)
}

/// 赎回的内部入口，只做鉴权主体解析与连接池解包，随后把工作全部交给资金事务。
/// 返回值第二项标识本次是否为首次赎回，已 redeemed 的重放会得到假值从而不重复发布事件。
/// 用户编号在此解析一次并下传，赎回事务据此过滤订阅归属，他人订阅一律按未找到处理。
async fn redeem_earn_subscription_with_internal(
    pool: Option<Pool<MySql>>,
    subject: &str,
    subscription_id: u64,
) -> AppResult<(RedeemEarnResponse, bool)> {
    let user_id = user_id_from_subject(subject)?;
    let pool = earn_mysql_pool(pool)?;
    let (response, is_new_redemption) =
        redeem_subscription_in_tx(&pool, user_id, subscription_id).await?;
    Ok((response, is_new_redemption))
}

/// 执行一次申购的完整资金流程，返回订阅快照与「是否为本次新建」标志。
/// 开事务前先做一次不加锁的幂等键只读探测，命中且产品与金额一致就直接返回旧订阅，
/// 这条快路径让重复提交不必付出开事务和锁产品的代价。
/// 未命中才开事务，锁序固定为：FOR UPDATE 锁 active 产品，插入订阅，再锁钱包。
/// 产品在探测与加锁之间被下架时会拿到 NotFound，此时回滚并再查一次幂等键，
/// 因为该订阅可能是产品下架前由并发请求成功创建的；确实不存在才向上返回 NotFound。
/// 订阅插入遇唯一键冲突返回 None，说明并发请求已抢先创建，同样回滚后回读旧订阅。
/// 插入位置刻意在锁钱包之前，保证冲突分支尚未扣减任何 available。
/// 余额不足在锁行后判定并直接返回错误，事务随之回滚，不会留下没有扣款的订阅。
/// 扣款只动 available，frozen 与 locked 不变，订阅、余额与流水一并提交。
async fn subscribe_in_tx(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    amount: BigDecimal,
    idempotency_key: String,
) -> AppResult<(EarnSubscriptionResponse, bool)> {
    if let Some(existing) = infrastructure::existing_subscription_for_idempotency_key_readonly(
        pool,
        user_id,
        &idempotency_key,
    )
    .await?
    {
        ensure_existing_subscription_matches_request(&existing, product_id, &amount)?;
        return Ok((existing, false));
    }

    let mut tx = pool.begin().await?;
    let product = match infrastructure::lock_active_product(&mut tx, product_id).await {
        Ok(product) => product,
        Err(AppError::NotFound) => {
            tx.rollback().await?;
            if let Some(existing) = replay_existing_subscription_if_present(
                pool,
                user_id,
                product_id,
                &amount,
                &idempotency_key,
            )
            .await?
            {
                return Ok((existing, false));
            }
            return Err(AppError::NotFound);
        }
        Err(error) => return Err(error),
    };
    validate_product_amount(&amount, &product)?;
    let matures_at = earn_matures_at(product.term_days)?;
    let Some(subscription_id) = infrastructure::insert_subscription_in_tx(
        &mut tx,
        user_id,
        &product,
        &amount,
        &idempotency_key,
        matures_at,
    )
    .await?
    else {
        tx.rollback().await?;
        return replay_existing_subscription(pool, user_id, product_id, &amount, &idempotency_key)
            .await
            .map(|subscription| (subscription, false));
    };

    let wallet = infrastructure::lock_wallet_row(&mut tx, user_id, product.asset_id).await?;
    if wallet.available < amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for earn subscription: requested {}, available {}, locked {}",
            amount, wallet.available, wallet.locked
        )));
    }
    infrastructure::debit_wallet_for_subscription_in_tx(
        &mut tx,
        user_id,
        product.asset_id,
        &amount,
        &wallet,
        subscription_id,
    )
    .await?;

    let subscription = infrastructure::load_subscription_by_id(&mut tx, subscription_id).await?;
    tx.commit().await?;
    Ok((subscription, true))
}

/// 执行一次赎回的完整资金流程，返回金额明细与「是否为首次赎回」标志。
/// 事务锁序为先 FOR UPDATE 锁订阅再锁钱包，订阅锁把并发赎回串行化，杜绝双重入账。
/// 状态机只允许 subscribed 迁移到 redeemed：命中 redeemed 走幂等分支，
/// 从历史流水恢复金额后提交并返回假值；其余状态一律返回冲突。
/// 计息基准取事务内的 UTC 当前时间，全部算式只依赖订阅快照，与产品当前配置无关。
/// 净到账额计入 available，frozen 与 locked 不变，随后把订阅置为 redeemed 并记录赎回时刻。
/// 三类费用不生成独立钱包流水，只体现在返回的明细字段里。
/// 余额、流水与订阅状态同事务提交，任一步失败都整体回滚不留部分写入。
async fn redeem_subscription_in_tx(
    pool: &Pool<MySql>,
    user_id: u64,
    subscription_id: u64,
) -> AppResult<(RedeemEarnResponse, bool)> {
    let mut tx = pool.begin().await?;
    let subscription =
        infrastructure::lock_subscription_by_id(&mut tx, user_id, subscription_id).await?;

    if subscription.status == "redeemed" {
        let response = redeemed_response_from_existing_subscription(&mut tx, subscription).await?;
        tx.commit().await?;
        return Ok((response, false));
    }
    if subscription.status != "subscribed" {
        return Err(AppError::Conflict(
            "earn subscription is not redeemable".to_owned(),
        ));
    }

    let now = Utc::now();
    let amounts = redemption_amounts_for_subscription(&subscription, now);
    let wallet =
        infrastructure::lock_wallet_row(&mut tx, subscription.user_id, subscription.asset_id)
            .await?;
    infrastructure::credit_wallet_for_redemption_in_tx(
        &mut tx,
        &subscription,
        &wallet,
        &amounts.redeem_amount,
    )
    .await?;
    infrastructure::mark_subscription_redeemed_in_tx(&mut tx, subscription.id).await?;
    let redeemed_subscription =
        infrastructure::load_subscription_by_id(&mut tx, subscription.id).await?;
    tx.commit().await?;
    Ok((
        RedeemEarnResponse {
            subscription: redeemed_subscription,
            principal_amount: amounts.principal_amount,
            gross_yield_amount: amounts.gross_yield_amount,
            yield_amount: amounts.yield_amount,
            redemption_fee_amount: amounts.redemption_fee_amount,
            maturity_profit_fee_amount: amounts.maturity_profit_fee_amount,
            early_redeem_fee_amount: amounts.early_redeem_fee_amount,
            fee_amount: amounts.fee_amount,
            redeem_amount: amounts.redeem_amount,
        },
        true,
    ))
}

/// 在唯一键冲突分支上强制回读旧订阅，查不到即返回冲突而非未找到。
/// 走到这里说明插入确实被唯一约束拒绝，旧行必然存在，只是可能尚未被并发事务提交，
/// 此时回读为空是暂态而非数据缺失，因此以「正在提交中」的冲突错误提示客户端稍后重试。
/// 回读到的订阅仍要与本次请求的产品和金额一致，否则由内部函数抛出冲突。
async fn replay_existing_subscription(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    amount: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<EarnSubscriptionResponse> {
    replay_existing_subscription_if_present(pool, user_id, product_id, amount, idempotency_key)
        .await?
        .ok_or_else(|| AppError::Conflict("earn idempotency key is being committed".to_owned()))
}

/// 以独立事务按幂等键回读订阅，存在则核对产品与金额后返回，不存在则返回空值。
/// 这里用 FOR UPDATE 版本的查询，目的是等待并发插入事务提交后再读，从而避免读到中间态。
/// 请求内容不一致时直接抛出冲突，此时事务因未显式提交而在离开作用域时回滚，
/// 由于全程只有读操作，回滚不会丢失任何数据。
/// 与强制版本的区别是查不到时返回空值而非报错，供产品下架分支判断是否还有别的失败原因。
async fn replay_existing_subscription_if_present(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    amount: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<Option<EarnSubscriptionResponse>> {
    let mut tx = pool.begin().await?;
    let Some(existing) = infrastructure::existing_subscription_for_idempotency_key(
        &mut tx,
        user_id,
        idempotency_key,
    )
    .await?
    else {
        return Ok(None);
    };
    ensure_existing_subscription_matches_request(&existing, product_id, amount)?;
    tx.commit().await?;
    Ok(Some(existing))
}

/// 为已赎回订阅的重放请求重建一份响应，本金与实际到账额从历史钱包流水恢复而非重新计算。
/// 这样能保证重放看到的金额与当初真正入账的一致，即便期间产品配置或算式发生过变化。
/// 净收益由到账额减本金反推，因此它包含了当初扣除的通用赎回费，与首次赎回时的口径存在差异。
/// 毛收益与三项费用明细无法从流水还原，只能按订阅的 redeemed_at 时刻重算，
/// redeemed_at 意外缺失时退化为当前时刻，此时费用明细可能与历史值不完全一致。
/// 缺少申购或赎回流水视为账务异常并返回内部错误，不会用零值蒙混过关。
/// 全程只读，不追加第二笔赎回，也不修改订阅状态或钱包余额。
async fn redeemed_response_from_existing_subscription(
    tx: &mut sqlx::Transaction<'_, MySql>,
    subscription: EarnSubscriptionResponse,
) -> AppResult<RedeemEarnResponse> {
    let (principal_amount, yield_amount, redeem_amount) =
        infrastructure::load_redeemed_amounts_from_ledger(tx, &subscription).await?;
    let redeemed_at = subscription.redeemed_at.unwrap_or_else(Utc::now);
    let amounts = redemption_amounts_for_subscription(&subscription, redeemed_at);
    Ok(RedeemEarnResponse {
        subscription,
        principal_amount,
        gross_yield_amount: amounts.gross_yield_amount,
        yield_amount,
        redemption_fee_amount: amounts.redemption_fee_amount,
        maturity_profit_fee_amount: amounts.maturity_profit_fee_amount,
        early_redeem_fee_amount: amounts.early_redeem_fee_amount,
        fee_amount: amounts.fee_amount,
        redeem_amount,
    })
}

/// 把可选的 MySQL 池解包为必需依赖，缺失时按内部错误处理而不是静默返回空列表。
/// 理财的配置写入与资金事务都无法降级运行，未配置数据库属于部署故障而非业务校验失败。
/// 所有理财用例都以此作为第一步依赖装配，因此不存在绕过该检查直接访问数据库的路径。
fn earn_mysql_pool(pool: Option<Pool<MySql>>) -> AppResult<Pool<MySql>> {
    pool.ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for earn routes".to_owned())
    })
}
