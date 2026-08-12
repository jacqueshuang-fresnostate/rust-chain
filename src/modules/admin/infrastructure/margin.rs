use super::*;

#[derive(Debug)]
pub(crate) struct AdminMarginLiquidationListFilter {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) pair_id: Option<u64>,
    pub(crate) position_id: Option<u64>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

/// 按用户、邮箱、交易对和持仓筛选保证金强平记录，分页返回风险与结算字段及匹配总数。
/// 列表与 COUNT 共用谓词并按强平记录 ID 倒序；两次连接池查询不加锁，并发新增可能使页数据和总数不在同一快照。
pub(crate) async fn list_admin_margin_liquidations(
    pool: &Pool<MySql>,
    filter: AdminMarginLiquidationListFilter,
) -> AppResult<(Vec<AdminMarginLiquidationResponse>, i64)> {
    let mut rows = admin_margin_liquidation_query();
    let mut total = QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM margin_liquidation_records");
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(user_id) = filter.user_id {
            push_user_id_filter(builder, "user_id", user_id);
        }
        push_user_email_filter(builder, "user_id", filter.email.clone());
        if let Some(pair_id) = filter.pair_id {
            builder.push(" AND pair_id = ");
            builder.push_bind(pair_id);
        }
        if let Some(position_id) = filter.position_id {
            builder.push(" AND position_id = ");
            builder.push_bind(position_id);
        }
    }

    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY id DESC",
        filter.limit,
        filter.offset,
    )
    .await
}

/// 按强平记录 ID 读取持仓、方向、保证金、价格、权益、盈亏和结算原因等完整快照。
/// 查询不锁持仓或强平记录，也不重新执行清算；记录缺失返回未找到，SQL 或十进制映射失败返回错误。
pub(crate) async fn load_admin_margin_liquidation(
    pool: &Pool<MySql>,
    liquidation_id: u64,
) -> AppResult<AdminMarginLiquidationResponse> {
    let mut builder = admin_margin_liquidation_query();
    builder.push(" WHERE id = ");
    builder.push_bind(liquidation_id);
    builder.push(" LIMIT 1");
    builder
        .build_query_as::<AdminMarginLiquidationResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

fn admin_margin_liquidation_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT id, position_id, user_id, product_id, pair_id, margin_asset, direction,
                  margin_amount, notional_amount, interest_amount, entry_price, mark_price,
                  maintenance_margin_rate, equity, maintenance_margin, realized_pnl,
                  payout_amount, reason, liquidated_at, created_at
           FROM margin_liquidation_records"#,
    )
}
