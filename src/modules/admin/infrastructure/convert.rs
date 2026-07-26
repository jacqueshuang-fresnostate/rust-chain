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
