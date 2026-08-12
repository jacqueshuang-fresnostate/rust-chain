use super::query_support::{MARGIN_PRODUCT_ORDER_BY, fetch_admin_page, optional_string};
use crate::{
    error::{AppError, AppResult},
    modules::margin::presentation::{MarginProductResponse, MarginUserSettingResponse},
};
use bigdecimal::BigDecimal;
use serde_json::Value;
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};

#[derive(Debug, sqlx::FromRow)]
/// 用户设置事务锁定的产品默认模式、支持模式与杠杆档位。
pub(crate) struct MarginProductSettingRule {
    pub(crate) margin_mode: String,
    pub(crate) margin_modes: SqlxJson<Vec<String>>,
    pub(crate) leverage_levels: SqlxJson<Vec<String>>,
}

#[derive(Debug)]
/// 完成领域校验后待写入保证金产品表的规范化配置值。
pub(crate) struct MarginProductUpsertValues<'a> {
    pub(crate) pair_id: u64,
    pub(crate) margin_asset: u64,
    pub(crate) logo_url: Option<String>,
    pub(crate) margin_mode: String,
    pub(crate) margin_modes: Vec<String>,
    pub(crate) leverage_levels: Vec<String>,
    pub(crate) max_leverage: &'a BigDecimal,
    pub(crate) min_margin: &'a BigDecimal,
    pub(crate) max_margin: Option<&'a BigDecimal>,
    pub(crate) maintenance_margin_rate: &'a BigDecimal,
    pub(crate) hourly_interest_rate: BigDecimal,
    pub(crate) status: &'a str,
}
/// 在产品配置事务内确认交易对存在；缺失时返回校验错误且不写产品或审计。
pub(crate) async fn ensure_pair_exists(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, u64>("SELECT id FROM trading_pairs WHERE id = ? LIMIT 1")
        .bind(pair_id)
        .fetch_optional(&mut **tx)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }
    Ok(())
}

/// 锁定启用保证金产品的模式与杠杆配置，固定用户设置更新所依据的规则。
pub(crate) async fn lock_active_product_setting_rule(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<MarginProductSettingRule> {
    let product = sqlx::query_as::<_, MarginProductSettingRule>(
        r#"SELECT margin_mode, margin_modes, leverage_levels
           FROM margin_products
           WHERE id = ? AND status = 'active'
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(product)
}

/// 在调用方事务内按用户和产品写入杠杆或模式设置，保留未提供字段。
/// 唯一键使重复设置覆盖同一记录；失败由应用层回滚产品锁和本次变更。
pub(crate) async fn upsert_user_margin_setting(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    product_id: u64,
    margin_mode: Option<&str>,
    leverage: Option<&BigDecimal>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO margin_user_settings (user_id, product_id, margin_mode, leverage)
           VALUES (?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE
             margin_mode = COALESCE(VALUES(margin_mode), margin_mode),
             leverage = COALESCE(VALUES(leverage), leverage)"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(margin_mode)
    .bind(leverage)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在调用方事务内回读用户保证金设置及产品默认值，不额外修改任何配置。
pub(crate) async fn load_user_margin_setting(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    product_id: u64,
) -> AppResult<MarginUserSettingResponse> {
    sqlx::query_as::<_, (Option<String>, Option<BigDecimal>)>(
        "SELECT margin_mode, leverage FROM margin_user_settings WHERE user_id = ? AND product_id = ? LIMIT 1",
    )
    .bind(user_id)
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .map(|(margin_mode, leverage)| MarginUserSettingResponse {
        product_id,
        margin_mode,
        leverage,
    })
    .ok_or(AppError::NotFound)
}

/// 只读加载用户保证金设置及产品默认值；产品不存在或停用沿用既有错误语义。
pub(crate) async fn load_user_margin_setting_from_pool(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
) -> AppResult<MarginUserSettingResponse> {
    sqlx::query_as::<_, (Option<String>, Option<BigDecimal>)>(
        "SELECT margin_mode, leverage FROM margin_user_settings WHERE user_id = ? AND product_id = ? LIMIT 1",
    )
    .bind(user_id)
    .bind(product_id)
    .fetch_optional(pool)
    .await?
    .map(|(margin_mode, leverage)| MarginUserSettingResponse {
        product_id,
        margin_mode,
        leverage,
    })
    .ok_or(AppError::NotFound)
}

/// 读取保证金产品完整配置及能力集合；该查询不加行锁，适合管理读路径。
pub(crate) async fn load_product_by_id(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<MarginProductResponse> {
    sqlx::query_as::<_, MarginProductResponse>(
        r#"SELECT products.id, products.pair_id, pairs.symbol,
                  products.margin_asset, assets.symbol AS margin_asset_symbol,
                  products.logo_url,
                  products.margin_mode, products.margin_modes, products.leverage_levels, products.max_leverage,
                  products.min_margin, products.max_margin, products.maintenance_margin_rate,
                  products.hourly_interest_rate, products.status
           FROM margin_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           INNER JOIN assets ON assets.id = products.margin_asset
           WHERE products.id = ?
           LIMIT 1"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 在管理事务内锁定保证金产品旧快照，保证更新与 before/after 审计对应同一版本。
pub(crate) async fn lock_product_by_id(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<MarginProductResponse> {
    sqlx::query_as::<_, MarginProductResponse>(
        r#"SELECT products.id, products.pair_id, pairs.symbol,
                  products.margin_asset, assets.symbol AS margin_asset_symbol,
                  products.logo_url,
                  products.margin_mode, products.margin_modes, products.leverage_levels, products.max_leverage,
                  products.min_margin, products.max_margin, products.maintenance_margin_rate,
                  products.hourly_interest_rate, products.status
           FROM margin_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           INNER JOIN assets ON assets.id = products.margin_asset
           WHERE products.id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 在调用方事务内写入保证金产品主记录，字段已由应用层完成精度与枚举校验。
pub(crate) async fn insert_margin_product(
    tx: &mut Transaction<'_, MySql>,
    values: &MarginProductUpsertValues<'_>,
) -> AppResult<u64> {
    sqlx::query(
        r#"INSERT INTO margin_products
           (pair_id, margin_asset, logo_url, margin_mode, margin_modes, leverage_levels, max_leverage, min_margin, max_margin,
            maintenance_margin_rate, hourly_interest_rate, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(values.pair_id)
    .bind(values.margin_asset)
    .bind(&values.logo_url)
    .bind(&values.margin_mode)
    .bind(SqlxJson(values.margin_modes.clone()))
    .bind(SqlxJson(values.leverage_levels.clone()))
    .bind(values.max_leverage)
    .bind(values.min_margin)
    .bind(values.max_margin)
    .bind(values.maintenance_margin_rate)
    .bind(&values.hourly_interest_rate)
    .bind(values.status)
    .execute(&mut **tx)
    .await
    .map(|result| result.last_insert_id())
    .map_err(AppError::from)
}

/// 在调用方事务内更新产品配置；受影响行异常或数据库失败应使审计一并回滚。
pub(crate) async fn update_margin_product(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    values: &MarginProductUpsertValues<'_>,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE margin_products
           SET pair_id = ?, margin_asset = ?, logo_url = ?, margin_mode = ?, margin_modes = ?,
               leverage_levels = ?, max_leverage = ?, min_margin = ?, max_margin = ?,
               maintenance_margin_rate = ?, hourly_interest_rate = ?, status = ?
           WHERE id = ?"#,
    )
    .bind(values.pair_id)
    .bind(values.margin_asset)
    .bind(&values.logo_url)
    .bind(&values.margin_mode)
    .bind(SqlxJson(values.margin_modes.clone()))
    .bind(SqlxJson(values.leverage_levels.clone()))
    .bind(values.max_leverage)
    .bind(values.min_margin)
    .bind(values.max_margin)
    .bind(values.maintenance_margin_rate)
    .bind(&values.hourly_interest_rate)
    .bind(values.status)
    .bind(product_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在调用方事务内更新产品启停状态，调用方随后以同一事务写入审计记录。
pub(crate) async fn update_margin_product_status(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    status: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE margin_products SET status = ? WHERE id = ?")
        .bind(status)
        .bind(product_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 在产品管理事务内追加管理员操作、原因及前后快照，确保配置变更可追溯。
pub(crate) async fn insert_admin_audit_log(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    action: &str,
    target_id: u64,
    before_json: Option<Value>,
    after_json: Option<Value>,
    reason: Option<String>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, before_json, after_json, reason)
           VALUES (?, ?, 'margin_product', ?, ?, ?, ?)"#,
    )
    .bind(admin_id)
    .bind(action)
    .bind(target_id.to_string())
    .bind(before_json.map(SqlxJson))
    .bind(after_json.map(SqlxJson))
    .bind(optional_string(reason))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 按状态筛选并查询保证金产品能力读模型；该路径不锁定产品或修改配置。
pub(crate) async fn list_margin_products(
    pool: &Pool<MySql>,
    status: Option<&str>,
    limit: u32,
) -> AppResult<Vec<MarginProductResponse>> {
    let mut builder = margin_product_query();
    push_margin_product_filters(&mut builder, status);
    builder.push(MARGIN_PRODUCT_ORDER_BY);
    builder.push(" LIMIT ");
    builder.push_bind(limit as i64);
    builder
        .build_query_as::<MarginProductResponse>()
        .fetch_all(pool)
        .await
        .map_err(AppError::from)
}

/// 后台杠杆产品列表：行查询与 COUNT 共用同一组谓词，总数才会跟随当前筛选。
/// 后台产品行与总数共享状态筛选，读取不锁定或更新产品。
pub(crate) async fn list_admin_margin_products(
    pool: &Pool<MySql>,
    status: Option<&str>,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<MarginProductResponse>, i64)> {
    let mut rows = margin_product_query();
    let mut total = QueryBuilder::<MySql>::new(
        r#"SELECT COUNT(*)
           FROM margin_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           INNER JOIN assets ON assets.id = products.margin_asset"#,
    );
    for builder in [&mut rows, &mut total] {
        push_margin_product_filters(builder, status);
    }

    fetch_admin_page(pool, rows, total, MARGIN_PRODUCT_ORDER_BY, limit, offset).await
}

fn margin_product_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT products.id, products.pair_id, pairs.symbol,
                  products.margin_asset, assets.symbol AS margin_asset_symbol,
                  products.logo_url,
                  products.margin_mode, products.margin_modes, products.leverage_levels, products.max_leverage,
                  products.min_margin, products.max_margin, products.maintenance_margin_rate,
                  products.hourly_interest_rate, products.status
           FROM margin_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           INNER JOIN assets ON assets.id = products.margin_asset"#,
    )
}

fn push_margin_product_filters(builder: &mut QueryBuilder<'_, MySql>, status: Option<&str>) {
    builder.push(" WHERE 1 = 1");
    if let Some(status) = status {
        builder.push(" AND products.status = ");
        builder.push_bind(status.to_owned());
    }
}

/// 在产品配置事务内确认保证金币种存在；失败时产品及审计均不得提交。
pub(crate) async fn ensure_asset_exists(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM assets WHERE id = ?")
        .bind(asset_id)
        .fetch_one(&mut **tx)
        .await?;
    if exists == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}
