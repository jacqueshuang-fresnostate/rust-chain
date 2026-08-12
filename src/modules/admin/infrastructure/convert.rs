use super::*;

#[derive(Debug)]
pub(crate) struct AdminConvertOrderListFilter {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug)]
pub(crate) struct AdminConvertPairInsert {
    pub(crate) from_asset_id: u64,
    pub(crate) to_asset_id: u64,
    pub(crate) pricing_mode: String,
    pub(crate) spread_rate: BigDecimal,
    pub(crate) fee_rate: BigDecimal,
    pub(crate) min_amount: BigDecimal,
    pub(crate) max_amount: Option<BigDecimal>,
    pub(crate) target_min_amount: BigDecimal,
    pub(crate) target_max_amount: Option<BigDecimal>,
    pub(crate) enabled: bool,
}

#[derive(Debug)]
pub(crate) struct AdminConvertPairUpdate {
    pub(crate) from_asset_id: u64,
    pub(crate) to_asset_id: u64,
    pub(crate) pricing_mode: String,
    pub(crate) spread_rate: BigDecimal,
    pub(crate) fee_rate: BigDecimal,
    pub(crate) min_amount: BigDecimal,
    pub(crate) max_amount: Option<BigDecimal>,
    pub(crate) target_min_amount: BigDecimal,
    pub(crate) target_max_amount: Option<BigDecimal>,
    pub(crate) enabled: bool,
}

/// 分页读取全部闪兑交易对及两端资产代码，并返回相同联表口径下的总数。
/// 列表按交易对 ID 倒序且不加锁；连接池查询期间的并发增删可能使列表与 COUNT 不处于同一快照，SQL 或映射失败直接返回错误。
pub(crate) async fn list_admin_convert_pairs(
    pool: &Pool<MySql>,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<ConvertPairResponse>, i64)> {
    let total = QueryBuilder::<MySql>::new(
        r#"SELECT COUNT(*)
           FROM convert_pairs pairs
           INNER JOIN assets from_assets ON from_assets.id = pairs.from_asset
           INNER JOIN assets to_assets ON to_assets.id = pairs.to_asset"#,
    );
    fetch_admin_page(
        pool,
        admin_convert_pair_query(),
        total,
        " ORDER BY pairs.id DESC",
        limit,
        offset,
    )
    .await
}

/// 按闪兑交易对 ID 读取两端资产代码、定价参数、限额和启用状态。
/// 连接池查询不加锁且无审计副作用；交易对不存在返回未找到，SQL 或十进制映射失败返回数据库错误。
pub(crate) async fn load_admin_convert_pair(
    pool: &Pool<MySql>,
    pair_id: u64,
) -> AppResult<ConvertPairResponse> {
    let mut builder = admin_convert_pair_query();
    builder.push(" WHERE pairs.id = ");
    builder.push_bind(pair_id);
    builder.push(" LIMIT 1");
    builder
        .build_query_as::<ConvertPairResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

/// 按用户、邮箱和订单状态筛选闪兑订单，分页返回成交数量、汇率、费用及匹配总数。
/// 列表与 COUNT 共用谓词并按订单 ID 倒序；两次连接池读取不加锁，并发下页数据与总数可能来自不同快照。
pub(crate) async fn list_admin_convert_orders(
    pool: &Pool<MySql>,
    filter: AdminConvertOrderListFilter,
) -> AppResult<(Vec<ConvertOrderResponse>, i64)> {
    let mut rows = admin_convert_order_query();
    let mut total = QueryBuilder::<MySql>::new(
        r#"SELECT COUNT(*)
           FROM convert_orders orders
           INNER JOIN users ON users.id = orders.user_id
           INNER JOIN assets from_assets ON from_assets.id = orders.from_asset
           INNER JOIN assets to_assets ON to_assets.id = orders.to_asset"#,
    );
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(user_id) = filter.user_id {
            push_user_id_filter(builder, "orders.user_id", user_id);
        }
        push_user_email_filter(builder, "orders.user_id", filter.email.clone());
        if let Some(status) = filter.status.clone() {
            builder.push(" AND orders.status = ");
            builder.push_bind(status);
        }
    }

    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY orders.id DESC",
        filter.limit,
        filter.offset,
    )
    .await
}

/// 按闪兑订单 ID 读取用户邮箱、资产代码、兑换金额、汇率、费用和当前状态。
/// 查询不锁订单或关联资产，也不写审计；记录缺失返回未找到，SQL 或字段解码失败直接返回错误。
pub(crate) async fn load_admin_convert_order(
    pool: &Pool<MySql>,
    order_id: u64,
) -> AppResult<ConvertOrderResponse> {
    let mut builder = admin_convert_order_query();
    builder.push(" WHERE orders.id = ");
    builder.push_bind(order_id);
    builder.push(" LIMIT 1");
    builder
        .build_query_as::<ConvertOrderResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

/// 在调用方事务中插入完整的闪兑交易对定价、费用、限额和启用配置，并返回自增 ID。
/// 输入须已由服务层规范化；重复资产方向映射为冲突，函数不锁既有交易对、不提交事务，审计由创建用例在同一事务追加。
pub(crate) async fn insert_admin_convert_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminConvertPairInsert,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO convert_pairs
           (from_asset, to_asset, pricing_mode, spread_rate, fee_rate, min_amount, max_amount,
            target_min_amount, target_max_amount, enabled)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(input.from_asset_id)
    .bind(input.to_asset_id)
    .bind(&input.pricing_mode)
    .bind(&input.spread_rate)
    .bind(&input.fee_rate)
    .bind(&input.min_amount)
    .bind(&input.max_amount)
    .bind(&input.target_min_amount)
    .bind(&input.target_max_amount)
    .bind(input.enabled)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_convert_pair_error)?;
    Ok(result.last_insert_id())
}

/// 在调用方事务中按 ID 覆盖闪兑交易对的资产方向、定价、费用、限额和启用状态。
/// 函数不校验受影响行数，调用方须先锁定并确认交易对存在；资产方向唯一键冲突映射为业务冲突，提交与审计仍由更新用例负责。
pub(crate) async fn update_admin_convert_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
    input: AdminConvertPairUpdate,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE convert_pairs
           SET from_asset = ?, to_asset = ?, pricing_mode = ?, spread_rate = ?, fee_rate = ?,
               min_amount = ?, max_amount = ?, target_min_amount = ?,
               target_max_amount = ?, enabled = ?
           WHERE id = ?"#,
    )
    .bind(input.from_asset_id)
    .bind(input.to_asset_id)
    .bind(&input.pricing_mode)
    .bind(&input.spread_rate)
    .bind(&input.fee_rate)
    .bind(&input.min_amount)
    .bind(&input.max_amount)
    .bind(&input.target_min_amount)
    .bind(&input.target_max_amount)
    .bind(input.enabled)
    .bind(pair_id)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_convert_pair_error)?;
    Ok(())
}

/// 在调用方事务中按 ID 物理删除闪兑交易对。
/// SQL 不检查受影响行数；调用方须先锁定交易对、确认已停用并检查报价/订单/新币规则引用，删除审计与事务提交由删除用例完成。
pub(crate) async fn delete_admin_convert_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<()> {
    sqlx::query("DELETE FROM convert_pairs WHERE id = ?")
        .bind(pair_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 在调用方事务中统计闪兑交易对关联的报价、订单和新币闪兑规则，确认其可被物理删除。
/// 三项计数查询不加锁，任一计数大于零返回校验错误；调用方应已锁定交易对并负责在同一事务中执行删除，SQL 失败触发上层回滚。
pub(crate) async fn ensure_convert_pair_has_no_references_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<()> {
    let (quote_count, order_count, rule_count): (i64, i64, i64) = sqlx::query_as(
        r#"SELECT
                  (SELECT COUNT(*) FROM convert_quotes WHERE convert_pair_id = ?) AS quote_count,
                  (SELECT COUNT(*) FROM convert_orders WHERE convert_pair_id = ?) AS order_count,
                  (SELECT COUNT(*) FROM new_coin_convert_rules WHERE convert_pair_id = ?) AS rule_count"#,
    )
    .bind(pair_id)
    .bind(pair_id)
    .bind(pair_id)
    .fetch_one(&mut **tx)
    .await?;

    if quote_count > 0 || order_count > 0 || rule_count > 0 {
        return Err(AppError::Validation(
            "convert pair with related records cannot be deleted".to_owned(),
        ));
    }
    Ok(())
}

/// 在调用方事务快照中回读闪兑交易对及两端资产代码，供写后响应和审计取值。
/// 查询本身不追加锁；记录缺失返回未找到，读取失败由外层事务处理，函数不会提交或补写审计。
pub(crate) async fn load_admin_convert_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<ConvertPairResponse> {
    let mut builder = admin_convert_pair_query();
    builder.push(" WHERE pairs.id = ");
    builder.push_bind(pair_id);
    builder.push(" LIMIT 1");
    builder
        .build_query_as::<ConvertPairResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

/// 在调用方事务中锁定指定闪兑交易对的联表详情并返回修改前快照。
/// 查询以交易对为条件并在联接资产后执行 `FOR UPDATE`，命中记录的锁持有至事务结束；交易对缺失返回未找到，调用方随后完成状态/引用校验和写入。
pub(crate) async fn lock_admin_convert_pair_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<ConvertPairResponse> {
    let mut builder = admin_convert_pair_query();
    builder.push(" WHERE pairs.id = ");
    builder.push_bind(pair_id);
    builder.push(" LIMIT 1 FOR UPDATE");
    builder
        .build_query_as::<ConvertPairResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

fn admin_convert_pair_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT pairs.id,
                  pairs.from_asset AS from_asset_id,
                  from_assets.symbol AS from_asset_symbol,
                  pairs.to_asset AS to_asset_id,
                  to_assets.symbol AS to_asset_symbol,
                  pairs.pricing_mode, pairs.spread_rate, pairs.fee_rate, pairs.min_amount,
                  pairs.max_amount, pairs.target_min_amount, pairs.target_max_amount,
                  pairs.enabled
           FROM convert_pairs pairs
           INNER JOIN assets from_assets ON from_assets.id = pairs.from_asset
           INNER JOIN assets to_assets ON to_assets.id = pairs.to_asset"#,
    )
}

fn admin_convert_order_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT orders.id, users.email AS user_email,
                  from_assets.symbol AS from_asset_symbol,
                  to_assets.symbol AS to_asset_symbol,
                  orders.from_amount, orders.to_amount, orders.rate,
                  orders.fee_rate, orders.fee_amount, orders.status, orders.created_at
           FROM convert_orders orders
           INNER JOIN users ON users.id = orders.user_id
           INNER JOIN assets from_assets ON from_assets.id = orders.from_asset
           INNER JOIN assets to_assets ON to_assets.id = orders.to_asset"#,
    )
}

fn map_duplicate_convert_pair_error(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("convert pair already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}
