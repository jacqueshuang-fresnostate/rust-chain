//! 杠杆产品配置与用户设置的 MySQL 适配器。
//!
//! 承载 `margin_products`、`margin_user_settings` 和 `admin_audit_logs` 三张表的读写，
//! 以及产品写路径附带的交易对、资产存在性校验。
//! 需要与后续写入绑定同一版本的读取一律带 FOR UPDATE，纯展示读取则不加锁以免阻塞改配。
//! 本文件全部函数都在调用方给定的事务或连接池上执行，自己不 begin、不 commit、不 rollback，
//! 因此产品行与审计记录能否原子落地完全取决于应用层的事务边界。

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
    /// 产品默认保证金模式，用户未显式指定模式时以此为准。
    pub(crate) margin_mode: String,
    /// 产品允许的保证金模式集合，取值只会是 isolated 与 cross 的非空无重复子集。
    pub(crate) margin_modes: SqlxJson<Vec<String>>,
    /// 产品可选杠杆档位，以去尾零的十进制文本存 JSON 列，用户设置必须精确命中其中一项。
    pub(crate) leverage_levels: SqlxJson<Vec<String>>,
}

#[derive(Debug)]
/// 完成领域校验后待写入保证金产品表的规范化配置值。
pub(crate) struct MarginProductUpsertValues<'a> {
    /// 关联交易对主键，写入前由 `ensure_pair_exists` 在同一事务内确认存在。
    pub(crate) pair_id: u64,
    /// 保证金计价币种资产主键，仓位的抵押、盈亏和利息都以该币种结算。
    pub(crate) margin_asset: u64,
    /// 产品图标地址，已裁剪空白并限长两千零四十八字符，空白折叠为 None。
    pub(crate) logo_url: Option<String>,
    /// 默认保证金模式，取自模式集合的首个元素，集合为空时兜底为 isolated。
    pub(crate) margin_mode: String,
    /// 归一化去重后的支持模式集合，顺序保留调用方给定的原始次序。
    pub(crate) margin_modes: Vec<String>,
    /// 归一化去尾零后的杠杆档位文本，其中最大值必须等于 `max_leverage`。
    pub(crate) leverage_levels: Vec<String>,
    /// 最大杠杆倍数，严格大于一，最多八位小数十位整数。
    pub(crate) max_leverage: &'a BigDecimal,
    /// 单笔开仓的最小保证金额，严格为正，单位是保证金币种。
    pub(crate) min_margin: &'a BigDecimal,
    /// 单笔开仓的最大保证金额，None 表示不设上限；有值时不得小于最小额。
    pub(crate) max_margin: Option<&'a BigDecimal>,
    /// 维持保证金率，非负，乘名义价值得到强平判定所用的维持保证金。
    pub(crate) maintenance_margin_rate: &'a BigDecimal,
    /// 借款小时利率，非负，缺省补零表示免息；利息 worker 按它逐小时计提。
    pub(crate) hourly_interest_rate: BigDecimal,
    /// 产品启停状态，只会是 active 或 disabled，disabled 后不能再开新仓。
    pub(crate) status: &'a str,
}
/// 在产品配置事务内确认目标交易对真实存在，用 LIMIT 1 探测而非计数以避免全表统计。
/// 缺失时返回 NotFound，使产品行与审计记录都不会被提交，杜绝配出指向空交易对的杠杆产品。
/// 只做存在性检查，不校验交易对是否启用，也不对该行加锁。
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

/// 对启用中的杠杆产品加 FOR UPDATE 行锁，只取默认模式、支持模式集合和杠杆档位三项校验依据。
/// 状态条件写在 WHERE 里，因此产品不存在和已停用都返回 NotFound，用户无法给停用产品保存设置。
/// 加锁的目的是把校验依据与随后的用户设置写入绑定在同一版本，防止管理员并发改配导致校验后失效。
/// 该锁由调用方的事务持有到提交，期间同一产品的后台改配会被阻塞等待。
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
///
/// 两个业务字段都是 Option，ON DUPLICATE KEY UPDATE 用 COALESCE 保留旧值，
/// 因此只改倍数的请求不会把已保存的模式抹成 NULL，只改模式的请求同理不影响倍数。
/// 首次插入时未提供的那一侧落为 NULL，读取方会把它当作「未设置」并回落到产品默认值。
/// 依赖 (user_id, product_id) 唯一键实现幂等覆盖，重复提交同样的设置不会产生第二行记录。
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

/// 在调用方事务内回读刚写入的用户设置，用于把落库结果原样返回给客户端而不是回显请求值。
/// 只查用户设置表，不联产品表，因此两个字段保持数据库里的可空语义，NULL 表示该维度未设置。
/// 记录缺失返回 NotFound；在写入后立即调用的场景下不应出现，出现即说明同事务写入未生效。
/// 不加行锁也不修改任何配置，产品行的锁由调用方在更早的步骤持有。
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

/// 直接走连接池只读加载用户设置，供 GET 查询使用，不开事务也不占用任何行锁。
/// SQL 与事务内版本完全一致，同样只查设置表：不校验产品是否存在或启用，
/// 因此产品被停用后用户仍能读回此前保存的模式与倍数，是否可用由开仓路径另行判定。
/// 用户从未在该产品上设置过时返回 NotFound，调用方据此回落到产品默认配置。
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

/// 按主键读取杠杆产品完整配置，内联交易对与资产表补齐交易对符号和保证金币种符号。
/// 用 INNER JOIN 意味着交易对或资产被物理删除时该产品会直接查不到，返回 NotFound。
/// 不加 FOR UPDATE，因此既能用于纯展示读取，也能在写事务里回读 after 快照而不额外扩大锁范围；
/// 写路径下之所以安全，是因为同一事务此前已通过 `lock_product_by_id` 锁住了目标行。
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

/// 在后台改配事务内对产品行加 FOR UPDATE 并取出完整旧快照，作为审计的 before 记录。
/// 与 `load_product_by_id` 的 SQL 只差一个 FOR UPDATE，字段完全一致，便于 before 与 after 逐列比对。
/// 不带状态条件，因此停用中的产品也能被锁定和修改，这正是启停切换路径能复用它的原因。
/// 锁持有到调用方提交，期间同一产品的并发改配串行等待，避免两次更新互相覆盖或审计错配版本。
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

/// 在调用方事务内插入杠杆产品主记录并返回自增主键，供随后回读快照和写审计使用。
/// 入参已由应用层完成枚举、区间和十进制容量校验，这里不再重复判定，只负责逐字段绑定。
/// 模式集合与杠杆档位以 JSON 列存储，需要克隆一份 Vec 才能绑定，因此调用方保留原值不受影响。
/// 违反唯一键或外键时以数据库错误上抛，由调用方回滚整个事务，不会留下无审计的孤立产品行。
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

/// 在调用方事务内整行改写杠杆产品配置，包括交易对与保证金币种在内的全部十二列一次性覆盖。
/// 不检查受影响行数，因为调用方已在同一事务里用 FOR UPDATE 确认过目标行存在并持有锁；
/// 提交与否由调用方决定，数据库失败上抛后产品更新与审计写入会一并回滚。
/// 只改配置表，不追溯修改已有仓位的杠杆、维持保证金率或已计提利息。
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

/// 在调用方事务内只更新产品的 status 一列，其余配置保持原样，供后台启停切换使用。
/// 状态文本已由应用层限定为 active 或 disabled，这里不再校验；同样不检查受影响行数，
/// 因为调用方已先用 FOR UPDATE 锁定并确认该行存在。审计记录由调用方在同一事务内补写。
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

/// 在产品管理事务内追加一条后台审计，记录管理员标识、动作名、前后快照与变更原因。
/// `target_type` 硬编码为 `margin_product`，目标主键转成字符串存储以适配通用审计表结构。
/// 创建路径的 before 为 None，改配与启停两侧都有值，据此可在审计里逐字段还原改了什么。
/// 变更原因先做空白折叠，空串落成 NULL 而不是空文本；必填性由应用层在更早阶段保证。
/// 与产品写入处于同一事务，任一失败一起回滚，不会出现配置已生效却查不到操作人的情况。
pub(crate) async fn insert_admin_audit_log(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    action: &str,
    target_id: u64,
    before_json: Option<Value>,
    after_json: Option<Value>,
    reason: Option<String>,
) -> AppResult<()> {
    let request_context = crate::infra::admin_request_context::current_admin_request_context();
    sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, before_json, after_json, reason, ip, request_id)
           VALUES (?, ?, 'margin_product', ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(admin_id)
    .bind(action)
    .bind(target_id.to_string())
    .bind(before_json.map(SqlxJson))
    .bind(after_json.map(SqlxJson))
    .bind(optional_string(reason))
    .bind(
        request_context
            .as_ref()
            .and_then(|context| context.source_ip.as_deref()),
    )
    .bind(
        request_context
            .as_ref()
            .map(|context| context.request_id.as_str()),
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 按可选状态筛选查询杠杆产品列表，供用户端浏览，只做 LIMIT 截断不支持翻页偏移。
/// 排序固定按产品主键倒序，新配置的产品排在前面；主键唯一因此不会出现分页重复或漏行。
/// 走连接池直接执行，不开事务、不加行锁，不会阻塞后台正在进行的产品改配。
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
///
/// 两个 QueryBuilder 分别以完整 SELECT 和 COUNT(*) 起头，再由同一个循环追加相同筛选条件，
/// 这样新增筛选维度时不可能只改一边，从根上避免明细与分页总数口径分裂。
/// COUNT 侧同样保留三表 INNER JOIN，使联表导致的行过滤在两边表现一致。
/// 排序、LIMIT 与 OFFSET 的拼接和两次查询的执行都交给共享的分页助手完成。
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

/// 构造杠杆产品列表查询的公共 SELECT 前缀，内联交易对与资产表补齐两个符号字段。
/// 用户端列表和后台分页列表共用它，保证两处返回的列集合完全一致，前端可复用同一套解析。
/// 只产出前缀，筛选条件、排序和分页由调用方按需追加。
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

/// 向产品查询追加筛选条件，先固定写入恒真的 `WHERE 1 = 1` 以便后续条件都能无脑用 AND 拼接。
/// 状态值走 `push_bind` 参数化绑定而非字符串插值，因此不存在 SQL 注入面。
/// 行查询与 COUNT 查询都调用同一个它，是两侧筛选口径必然一致的实现保证。
fn push_margin_product_filters(builder: &mut QueryBuilder<'_, MySql>, status: Option<&str>) {
    builder.push(" WHERE 1 = 1");
    if let Some(status) = status {
        builder.push(" AND products.status = ");
        builder.push_bind(status.to_owned());
    }
}

/// 在产品配置事务内确认保证金币种资产存在，缺失返回 NotFound 使产品与审计均不提交。
/// 与交易对检查的写法不同，这里用 COUNT(*) 判零而非 LIMIT 1 探测，效果等价但多一次聚合。
/// 同样只验存在性，不要求资产处于 active，也不对资产行加锁。
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
