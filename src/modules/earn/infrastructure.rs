//! earn bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。

use crate::{
    error::{AppError, AppResult},
    modules::earn::{
        presentation::{
            EarnCategoryResponse, EarnProductResponse, EarnProductsResponse,
            EarnSubscriptionResponse, EarnSubscriptionsResponse,
        },
        repository::{EarnCategoryWrite, EarnProductRuleRow, EarnProductWrite, EarnWalletRow},
    },
};
use bigdecimal::BigDecimal;
use serde_json::Value;
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};

/// 分页排序必须带唯一列 id，否则同一排序值的行会在页间重复或丢失。
const EARN_PRODUCT_ORDER_BY: &str = " ORDER BY products.id DESC";

/// 行查询与 COUNT 查询必须由同一组过滤谓词构建，返回总数才能与当前筛选一致。
async fn fetch_admin_page<T>(
    pool: &Pool<MySql>,
    mut rows: QueryBuilder<'_, MySql>,
    mut total: QueryBuilder<'_, MySql>,
    order_by: &str,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<T>, i64)>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
{
    rows.push(order_by);
    rows.push(" LIMIT ");
    rows.push_bind(limit as i64);
    rows.push(" OFFSET ");
    rows.push_bind(offset as i64);

    let items = rows.build_query_as::<T>().fetch_all(pool).await?;
    let total = total.build_query_scalar::<i64>().fetch_one(pool).await?;

    Ok((items, total))
}

/// 按可选状态和数量上限读取理财产品，并保留历史分类代码的显示回退。
/// 查询只读产品和分类元数据，不锁定产品，也不触发订阅或资金变化。
pub(crate) async fn list_products(
    pool: &Pool<MySql>,
    status: Option<&str>,
    limit: u32,
) -> AppResult<EarnProductsResponse> {
    let mut builder = earn_product_query();
    push_earn_product_filters(&mut builder, status);
    builder.push(EARN_PRODUCT_ORDER_BY);
    builder.push(" LIMIT ");
    builder.push_bind(limit as i64);

    let products = builder
        .build_query_as::<EarnProductResponse>()
        .fetch_all(pool)
        .await?;
    Ok(EarnProductsResponse { products })
}

/// 使用同一状态谓词查询后台理财产品分页行与 COUNT，保证 total 对应当前筛选。
/// 该只读入口不锁产品，返回当前配置与费用规则，不修改分类、历史订阅快照或钱包。
pub(crate) async fn list_admin_products(
    pool: &Pool<MySql>,
    status: Option<&str>,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<EarnProductResponse>, i64)> {
    let mut rows = earn_product_query();
    let mut total = QueryBuilder::<MySql>::new(
        r#"SELECT COUNT(*)
           FROM earn_products products
           INNER JOIN assets ON assets.id = products.asset_id
           LEFT JOIN earn_product_categories categories ON categories.code = products.category"#,
    );
    for builder in [&mut rows, &mut total] {
        push_earn_product_filters(builder, status);
    }

    fetch_admin_page(pool, rows, total, EARN_PRODUCT_ORDER_BY, limit, offset).await
}

fn earn_product_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT products.id, products.asset_id, assets.symbol AS asset_symbol,
                  products.name, products.banner_url, products.small_logo_url,
                  products.category,
                  COALESCE(JSON_UNQUOTE(JSON_EXTRACT(categories.name_json, '$.items[0].title')), products.category) AS category_name,
                  categories.name_json AS category_name_json,
                  products.introduction_json,
                  products.term_days, products.apr_rate, products.redemption_fee_rate,
                  products.maturity_profit_fee_rate, products.early_redeem_fee_basis,
                  products.early_redeem_fee_rate,
                  products.min_subscribe, products.max_subscribe, products.status
           FROM earn_products products
           INNER JOIN assets ON assets.id = products.asset_id
           LEFT JOIN earn_product_categories categories ON categories.code = products.category"#,
    )
}

fn push_earn_product_filters(builder: &mut QueryBuilder<'_, MySql>, status: Option<&str>) {
    builder.push(" WHERE 1 = 1");
    if let Some(status) = status {
        builder.push(" AND products.status = ");
        builder.push_bind(status.to_owned());
    }
}

/// 按用户读取理财订阅快照，费用字段来自订阅时持久化条款。
/// 查询不读取当前产品费率，避免后台修改影响历史订阅展示。
pub(crate) async fn list_user_subscriptions(
    pool: &Pool<MySql>,
    user_id: u64,
    limit: u32,
) -> AppResult<EarnSubscriptionsResponse> {
    let subscriptions = sqlx::query_as::<_, EarnSubscriptionResponse>(
        r#"SELECT id, user_id, product_id, asset_id, amount, apr_rate,
                  redemption_fee_rate, maturity_profit_fee_rate, early_redeem_fee_basis,
                  early_redeem_fee_rate, term_days, status, idempotency_key,
                  subscribed_at, matures_at, redeemed_at
           FROM earn_subscriptions
           WHERE user_id = ?
           ORDER BY created_at DESC, id DESC
           LIMIT ?"#,
    )
    .bind(user_id)
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;
    Ok(EarnSubscriptionsResponse { subscriptions })
}

/// 使用一致的用户、邮箱和状态谓词读取后台订阅分页及 COUNT；费用字段取订阅时快照。
/// 查询不获取订阅或钱包行锁，也不计算新收益、执行赎回或迁移状态。
pub(crate) async fn list_admin_subscriptions(
    pool: &Pool<MySql>,
    limit: u32,
    offset: u32,
    user_id: Option<u64>,
    email: Option<String>,
    status: Option<String>,
) -> AppResult<(Vec<EarnSubscriptionResponse>, i64)> {
    let mut rows = QueryBuilder::<MySql>::new(
        r#"SELECT id, user_id, product_id, asset_id, amount, apr_rate,
                  redemption_fee_rate, maturity_profit_fee_rate, early_redeem_fee_basis,
                  early_redeem_fee_rate, term_days, status, idempotency_key,
                  subscribed_at, matures_at, redeemed_at
           FROM earn_subscriptions"#,
    );
    let mut total = QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM earn_subscriptions");
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(user_id) = user_id {
            builder.push(" AND user_id = ");
            builder.push_bind(user_id);
        }
        if let Some(email) = email.clone() {
            builder.push(
                " AND EXISTS (SELECT 1 FROM users WHERE users.id = user_id AND users.email = ",
            );
            builder.push_bind(email);
            builder.push(")");
        }
        if let Some(status) = status.clone() {
            builder.push(" AND status = ");
            builder.push_bind(status);
        }
    }

    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY created_at DESC, id DESC",
        limit,
        offset,
    )
    .await
}

/// 按状态筛选理财分类，并以同一谓词返回分页行与 COUNT。
/// 查询保持稳定分类代码和多语言名称原样，不因缺少关联产品而改写状态。
pub(crate) async fn list_admin_categories(
    pool: &Pool<MySql>,
    limit: u32,
    offset: u32,
    status: Option<String>,
) -> AppResult<(Vec<EarnCategoryResponse>, i64)> {
    let mut rows = QueryBuilder::<MySql>::new(
        r#"SELECT id, code, name_json,
                  COALESCE(JSON_UNQUOTE(JSON_EXTRACT(name_json, '$.items[0].title')), code) AS default_name,
                  sort_order, status
           FROM earn_product_categories"#,
    );
    let mut total = QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM earn_product_categories");
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(status) = status.clone() {
            builder.push(" AND status = ");
            builder.push_bind(status);
        }
    }

    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY sort_order ASC, id ASC",
        limit,
        offset,
    )
    .await
}

/// 在调用方事务中插入稳定分类代码及多语言名称，返回新分类编号。
/// 唯一代码冲突或写入失败时由应用层回滚，并且不得留下缺少审计的分类。
pub(crate) async fn insert_category_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: &EarnCategoryWrite,
) -> AppResult<u64> {
    match sqlx::query(
        r#"INSERT INTO earn_product_categories
           (code, name_json, sort_order, status)
           VALUES (?, ?, ?, ?)"#,
    )
    .bind(&input.code)
    .bind(SqlxJson(input.name_json.clone()))
    .bind(input.sort_order)
    .bind(&input.status)
    .execute(&mut **tx)
    .await
    {
        Ok(result) => Ok(result.last_insert_id()),
        Err(error) if is_duplicate_key_error(&error) => Err(AppError::Conflict(
            "earn product category code already exists".to_owned(),
        )),
        Err(error) => Err(AppError::Database(error)),
    }
}

/// 在调用方事务中更新分类名称、排序和状态，不修改不可变代码。
/// 更新结果须与前后快照审计一并提交，失败时继续保留原分类配置。
pub(crate) async fn update_category_in_tx(
    tx: &mut Transaction<'_, MySql>,
    category_id: u64,
    input: &EarnCategoryWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE earn_product_categories
           SET name_json = ?, sort_order = ?, status = ?
           WHERE id = ?"#,
    )
    .bind(SqlxJson(input.name_json.clone()))
    .bind(input.sort_order)
    .bind(&input.status)
    .bind(category_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在调用方事务中只更新分类启停状态，审计由应用层同事务追加。
/// 状态更新不改写分类代码，也不级联修改已有产品或订阅快照。
pub(crate) async fn update_category_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    category_id: u64,
    status: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE earn_product_categories SET status = ? WHERE id = ?")
        .bind(status)
        .bind(category_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 在当前事务快照中加载分类响应，缺失时返回未找到。
/// 该读取用于写入后的审计快照，不自行提交或释放调用方事务。
pub(crate) async fn load_category_by_id(
    tx: &mut Transaction<'_, MySql>,
    category_id: u64,
) -> AppResult<EarnCategoryResponse> {
    sqlx::query_as::<_, EarnCategoryResponse>(
        r#"SELECT id, code, name_json,
                  COALESCE(JSON_UNQUOTE(JSON_EXTRACT(name_json, '$.items[0].title')), code) AS default_name,
                  sort_order, status
           FROM earn_product_categories
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(category_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 按编号锁定理财分类，供配置更新与前后快照审计串行执行。
/// 行锁由调用方事务持有至提交，避免并发更新产生错配的审计快照。
pub(crate) async fn lock_category_by_id(
    tx: &mut Transaction<'_, MySql>,
    category_id: u64,
) -> AppResult<EarnCategoryResponse> {
    sqlx::query_as::<_, EarnCategoryResponse>(
        r#"SELECT id, code, name_json,
                  COALESCE(JSON_UNQUOTE(JSON_EXTRACT(name_json, '$.items[0].title')), code) AS default_name,
                  sort_order, status
           FROM earn_product_categories
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(category_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 确认产品资产存在；校验失败会阻止调用方事务写入理财产品。
/// 本入口只验证引用完整性，不创建钱包账户或修改任何资产配置。
pub(crate) async fn ensure_asset_exists(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, u64>("SELECT id FROM assets WHERE id = ? LIMIT 1")
        .bind(asset_id)
        .fetch_optional(&mut **tx)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }
    Ok(())
}

/// 确认分类代码存在且启用，避免新产品引用未知或禁用分类。
/// 校验与产品写入共用调用方事务，失败时不得持久化产品或管理员审计。
pub(crate) async fn ensure_active_category_exists(
    tx: &mut Transaction<'_, MySql>,
    code: &str,
) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, u64>(
        "SELECT id FROM earn_product_categories WHERE code = ? AND status = 'active' LIMIT 1",
    )
    .bind(code)
    .fetch_optional(&mut **tx)
    .await?;
    if exists.is_none() {
        return Err(AppError::Validation(
            "earn product category must reference an active category".to_owned(),
        ));
    }
    Ok(())
}

/// 在调用方事务中持久化完整产品和费用配置，返回新产品编号。
/// 产品与管理员审计必须原子提交，本入口不会创建订阅或移动用户余额。
pub(crate) async fn insert_product_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: &EarnProductWrite,
) -> AppResult<u64> {
    let product_id = sqlx::query(
        r#"INSERT INTO earn_products
           (asset_id, name, banner_url, small_logo_url, category, introduction_json, term_days,
            apr_rate, redemption_fee_rate, maturity_profit_fee_rate, early_redeem_fee_basis,
            early_redeem_fee_rate, min_subscribe, max_subscribe, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(input.asset_id)
    .bind(&input.name)
    .bind(&input.banner_url)
    .bind(&input.small_logo_url)
    .bind(&input.category)
    .bind(SqlxJson(input.introduction_json.clone()))
    .bind(input.term_days)
    .bind(&input.apr_rate)
    .bind(&input.redemption_fee_rate)
    .bind(&input.maturity_profit_fee_rate)
    .bind(&input.early_redeem_fee_basis)
    .bind(&input.early_redeem_fee_rate)
    .bind(&input.min_subscribe)
    .bind(&input.max_subscribe)
    .bind(&input.status)
    .execute(&mut **tx)
    .await?
    .last_insert_id();
    Ok(product_id)
}

/// 在调用方事务中覆盖产品及费用配置，既有订阅快照不会被级联修改。
/// 调用方将更新与管理员审计置于同一事务，任一步失败时保留原产品配置。
pub(crate) async fn update_product_in_tx(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    input: &EarnProductWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE earn_products
           SET asset_id = ?, name = ?, banner_url = ?, small_logo_url = ?, category = ?,
               introduction_json = ?, term_days = ?, apr_rate = ?, redemption_fee_rate = ?,
               maturity_profit_fee_rate = ?, early_redeem_fee_basis = ?,
               early_redeem_fee_rate = ?, min_subscribe = ?, max_subscribe = ?, status = ?
           WHERE id = ?"#,
    )
    .bind(input.asset_id)
    .bind(&input.name)
    .bind(&input.banner_url)
    .bind(&input.small_logo_url)
    .bind(&input.category)
    .bind(SqlxJson(input.introduction_json.clone()))
    .bind(input.term_days)
    .bind(&input.apr_rate)
    .bind(&input.redemption_fee_rate)
    .bind(&input.maturity_profit_fee_rate)
    .bind(&input.early_redeem_fee_basis)
    .bind(&input.early_redeem_fee_rate)
    .bind(&input.min_subscribe)
    .bind(&input.max_subscribe)
    .bind(&input.status)
    .bind(product_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在调用方事务中切换产品状态，订阅和钱包不发生变化。
/// 状态更新必须与管理员审计一起提交，既有订阅仍按原快照结算。
pub(crate) async fn update_product_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    status: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE earn_products SET status = ? WHERE id = ?")
        .bind(status)
        .bind(product_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 加载产品、资产及分类显示信息，分类缺失时回退到原始代码。
/// 该查询只读当前配置，不锁产品，也不重算任何既有订阅条款。
pub(crate) async fn load_product_by_id(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<EarnProductResponse> {
    sqlx::query_as::<_, EarnProductResponse>(
        r#"SELECT products.id, products.asset_id, assets.symbol AS asset_symbol,
                  products.name, products.banner_url, products.small_logo_url,
                  products.category,
                  COALESCE(JSON_UNQUOTE(JSON_EXTRACT(categories.name_json, '$.items[0].title')), products.category) AS category_name,
                  categories.name_json AS category_name_json,
                  products.introduction_json,
                  products.term_days, products.apr_rate, products.redemption_fee_rate,
                  products.maturity_profit_fee_rate, products.early_redeem_fee_basis,
                  products.early_redeem_fee_rate,
                  products.min_subscribe, products.max_subscribe, products.status
           FROM earn_products products
           INNER JOIN assets ON assets.id = products.asset_id
           LEFT JOIN earn_product_categories categories ON categories.code = products.category
           WHERE products.id = ?
           LIMIT 1"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 锁定产品配置行，保证更新期间前后审计快照来自同一事务。
/// 调用方负责保持锁至更新和审计完成，失败时整体回滚并释放行锁。
pub(crate) async fn lock_product_by_id(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<EarnProductResponse> {
    sqlx::query_as::<_, EarnProductResponse>(
        r#"SELECT products.id, products.asset_id, assets.symbol AS asset_symbol,
                  products.name, products.banner_url, products.small_logo_url,
                  products.category,
                  COALESCE(JSON_UNQUOTE(JSON_EXTRACT(categories.name_json, '$.items[0].title')), products.category) AS category_name,
                  categories.name_json AS category_name_json,
                  products.introduction_json,
                  products.term_days, products.apr_rate, products.redemption_fee_rate,
                  products.maturity_profit_fee_rate, products.early_redeem_fee_basis,
                  products.early_redeem_fee_rate,
                  products.min_subscribe, products.max_subscribe, products.status
           FROM earn_products products
           INNER JOIN assets ON assets.id = products.asset_id
           LEFT JOIN earn_product_categories categories ON categories.code = products.category
           WHERE products.id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 在当前事务中读取包含费用快照的理财订阅，缺失时返回未找到。
/// 返回值保持订阅时条款，不使用当前产品费率替换历史费用字段。
pub(crate) async fn load_subscription_by_id(
    tx: &mut Transaction<'_, MySql>,
    subscription_id: u64,
) -> AppResult<EarnSubscriptionResponse> {
    sqlx::query_as::<_, EarnSubscriptionResponse>(
        r#"SELECT id, user_id, product_id, asset_id, amount, apr_rate,
                  redemption_fee_rate, maturity_profit_fee_rate, early_redeem_fee_basis,
                  early_redeem_fee_rate, term_days, status, idempotency_key,
                  subscribed_at, matures_at, redeemed_at
           FROM earn_subscriptions
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(subscription_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 按用户与幂等键锁定既有订阅，串行处理并发重放。
/// 该入口只读取订阅；请求内容匹配与是否提交由应用层决定。
pub(crate) async fn existing_subscription_for_idempotency_key(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<EarnSubscriptionResponse>> {
    sqlx::query_as::<_, EarnSubscriptionResponse>(
        r#"SELECT id, user_id, product_id, asset_id, amount, apr_rate,
                  redemption_fee_rate, maturity_profit_fee_rate, early_redeem_fee_basis,
                  early_redeem_fee_rate, term_days, status, idempotency_key,
                  subscribed_at, matures_at, redeemed_at
           FROM earn_subscriptions
           WHERE user_id = ? AND idempotency_key = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(idempotency_key)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

/// 在开启资金事务前只读查找用户幂等订阅，用于快速重放。
/// 该查询不锁钱包；未命中后仍由事务内唯一键处理并发创建竞争。
pub(crate) async fn existing_subscription_for_idempotency_key_readonly(
    pool: &Pool<MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<EarnSubscriptionResponse>> {
    sqlx::query_as::<_, EarnSubscriptionResponse>(
        r#"SELECT id, user_id, product_id, asset_id, amount, apr_rate,
                  redemption_fee_rate, maturity_profit_fee_rate, early_redeem_fee_basis,
                  early_redeem_fee_rate, term_days, status, idempotency_key,
                  subscribed_at, matures_at, redeemed_at
           FROM earn_subscriptions
           WHERE user_id = ? AND idempotency_key = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

/// 按产品编号锁定启用产品及其费用、额度条款。
/// 调用方随后插入订阅再锁钱包，固定锁序避免与配置更新交错读取。
pub(crate) async fn lock_active_product(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<EarnProductRuleRow> {
    let product = sqlx::query_as::<_, EarnProductRuleRow>(
        r#"SELECT id, asset_id, term_days, apr_rate, redemption_fee_rate,
                  maturity_profit_fee_rate, early_redeem_fee_basis, early_redeem_fee_rate,
                  min_subscribe, max_subscribe, status
           FROM earn_products
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if product.status != "active" {
        return Err(AppError::NotFound);
    }
    Ok(product)
}

/// 在已锁产品的调用方事务中插入本金、APR、期限、到期时刻及四项费用快照。
/// 用户幂等键唯一冲突返回 `None`；插入发生在钱包锁之前，冲突分支不扣 available，应用层回滚后核对旧订阅。
pub(crate) async fn insert_subscription_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    product: &EarnProductRuleRow,
    amount: &BigDecimal,
    idempotency_key: &str,
    matures_at: chrono::DateTime<chrono::Utc>,
) -> AppResult<Option<u64>> {
    match sqlx::query(
        r#"INSERT INTO earn_subscriptions
           (user_id, product_id, asset_id, amount, apr_rate, redemption_fee_rate,
            maturity_profit_fee_rate, early_redeem_fee_basis, early_redeem_fee_rate,
            term_days, status, idempotency_key, matures_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'subscribed', ?, ?)"#,
    )
    .bind(user_id)
    .bind(product.id)
    .bind(product.asset_id)
    .bind(amount)
    .bind(&product.apr_rate)
    .bind(&product.redemption_fee_rate)
    .bind(&product.maturity_profit_fee_rate)
    .bind(&product.early_redeem_fee_basis)
    .bind(&product.early_redeem_fee_rate)
    .bind(product.term_days)
    .bind(idempotency_key)
    .bind(matures_at)
    .execute(&mut **tx)
    .await
    {
        Ok(result) => Ok(Some(result.last_insert_id())),
        Err(error) if is_duplicate_key_error(&error) => Ok(None),
        Err(error) => Err(AppError::Database(error)),
    }
}

/// 在调用方事务中锁定用户资产钱包三桶，账户缺失时禁止理财资金操作。
/// 申购先锁产品再锁钱包，赎回先锁订阅再锁钱包，固定顺序降低死锁风险。
pub(crate) async fn lock_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<EarnWalletRow> {
    sqlx::query_as::<_, EarnWalletRow>(
        r#"SELECT available, frozen, locked
           FROM wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Validation("wallet account is required for earn".to_owned()))
}

/// 按已锁钱包快照从 available 扣除申购本金，frozen/locked 保持不变。
/// 只追加一条 `earn_subscribe` available 负流水，ref_type 为 earn_subscription、ref_id 为订阅编号，三桶 after 对应同一账后快照。
/// 余额充足性由应用层锁行后先判定；订阅、余额和流水由调用方事务提交，SQL 失败回滚本次申购。
pub(crate) async fn debit_wallet_for_subscription_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    wallet: &EarnWalletRow,
    subscription_id: u64,
) -> AppResult<()> {
    let available_after = wallet.available.clone() - amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, 'earn_subscribe', ?, 'available', ?, ?, ?, ?, 'earn_subscription', ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(-amount.clone())
    .bind(&available_after)
    .bind(&available_after)
    .bind(&wallet.frozen)
    .bind(&wallet.locked)
    .bind(subscription_id.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 按用户与订阅编号锁定订阅，串行判定首次赎回或幂等重放。
/// 行锁由调用方持有至余额、流水和状态提交，阻止并发产生双重赎回。
pub(crate) async fn lock_subscription_by_id(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    subscription_id: u64,
) -> AppResult<EarnSubscriptionResponse> {
    sqlx::query_as::<_, EarnSubscriptionResponse>(
        r#"SELECT id, user_id, product_id, asset_id, amount, apr_rate,
                  redemption_fee_rate, maturity_profit_fee_rate, early_redeem_fee_basis,
                  early_redeem_fee_rate, term_days, status, idempotency_key,
                  subscribed_at, matures_at, redeemed_at
           FROM earn_subscriptions
           WHERE id = ? AND user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(subscription_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 按已锁钱包快照把已计算的净赎回额增加到 available，frozen/locked 保持不变。
/// 只写一条 `earn_redeem` available 正流水并引用订阅编号；本金、收益和费用拆分不另写钱包流水。
/// 金额来自订阅快照的 18 位计算；订阅状态、余额和流水由调用方事务提交，失败回滚本次赎回。
pub(crate) async fn credit_wallet_for_redemption_in_tx(
    tx: &mut Transaction<'_, MySql>,
    subscription: &EarnSubscriptionResponse,
    wallet: &EarnWalletRow,
    redeem_amount: &BigDecimal,
) -> AppResult<()> {
    let available_after = wallet.available.clone() + redeem_amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
        .bind(subscription.user_id)
        .bind(subscription.asset_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, 'earn_redeem', ?, 'available', ?, ?, ?, ?, 'earn_subscription', ?)"#,
    )
    .bind(subscription.user_id)
    .bind(subscription.asset_id)
    .bind(redeem_amount)
    .bind(&available_after)
    .bind(&available_after)
    .bind(&wallet.frozen)
    .bind(&wallet.locked)
    .bind(subscription.id.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在调用方赎回事务中标记订阅已赎回并记录完成时间。
/// 钱包入账流水须在同一事务完成，更新失败时余额和流水必须一起回滚。
pub(crate) async fn mark_subscription_redeemed_in_tx(
    tx: &mut Transaction<'_, MySql>,
    subscription_id: u64,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE earn_subscriptions SET status = 'redeemed', redeemed_at = CURRENT_TIMESTAMP(6) WHERE id = ?",
    )
    .bind(subscription_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 从最早 `earn_subscribe` 负流水和最早 `earn_redeem` 正流水恢复本金、净收益与实际到账额。
/// 该只读恢复用于已赎回重放；缺少任一流水视为账务异常，不用当前产品配置重算，也不追加第二笔赎回。
pub(crate) async fn load_redeemed_amounts_from_ledger(
    tx: &mut Transaction<'_, MySql>,
    subscription: &EarnSubscriptionResponse,
) -> AppResult<(BigDecimal, BigDecimal, BigDecimal)> {
    let ref_id = subscription.id.to_string();
    let principal_amount = sqlx::query_scalar::<_, BigDecimal>(
        r#"SELECT -amount
           FROM wallet_ledger
           WHERE user_id = ?
             AND asset_id = ?
             AND change_type = 'earn_subscribe'
             AND ref_type = 'earn_subscription'
             AND ref_id = ?
           ORDER BY id ASC
           LIMIT 1"#,
    )
    .bind(subscription.user_id)
    .bind(subscription.asset_id)
    .bind(&ref_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Internal("earn subscribe ledger is missing".to_owned()))?;

    let redeem_amount = sqlx::query_scalar::<_, BigDecimal>(
        r#"SELECT amount
           FROM wallet_ledger
           WHERE user_id = ?
             AND asset_id = ?
             AND change_type = 'earn_redeem'
             AND ref_type = 'earn_subscription'
             AND ref_id = ?
           ORDER BY id ASC
           LIMIT 1"#,
    )
    .bind(subscription.user_id)
    .bind(subscription.asset_id)
    .bind(&ref_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Internal("earn redeem ledger is missing".to_owned()))?;

    let yield_amount = redeem_amount.clone() - principal_amount.clone();
    Ok((principal_amount, yield_amount, redeem_amount))
}

#[allow(clippy::too_many_arguments)] // 审计记录字段与数据库列稳定对应，调用方事务负责原子提交。
/// 在调用方配置事务中写入管理员、目标、前后快照及原因。
/// 审计插入失败必须阻止分类或产品配置提交，避免无记录的后台变更。
pub(crate) async fn insert_admin_audit_log_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    action: &str,
    target_type: &str,
    target_id: u64,
    before_json: Option<Value>,
    after_json: Option<Value>,
    reason: Option<String>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, before_json, after_json, reason)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(admin_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id.to_string())
    .bind(before_json.map(SqlxJson))
    .bind(after_json.map(SqlxJson))
    .bind(reason)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn is_duplicate_key_error(error: &sqlx::Error) -> bool {
    let Some(database_error) = error.as_database_error() else {
        return false;
    };
    matches!(database_error.code().as_deref(), Some("1062"))
        || database_error.message().contains("Duplicate entry")
}
