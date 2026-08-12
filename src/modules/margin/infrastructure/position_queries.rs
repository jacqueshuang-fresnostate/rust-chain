use super::query_support::{fetch_admin_page, push_user_email_filter};
use crate::{
    error::{AppError, AppResult},
    modules::margin::presentation::{
        AdminInterestSummaryItem, AdminMarginPositionResponse, MarginCrossAccountResponse,
        MarginPositionResponse, MarginWalletAccountResponse,
    },
};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool, QueryBuilder};

#[derive(Debug, sqlx::FromRow)]
/// 风险快照所需的用户仓位、产品费率和行情交易对字段集合。
pub(crate) struct MarginRiskPositionRow {
    pub(crate) id: u64,
    pub(crate) pair_id: u64,
    pub(crate) symbol: String,
    pub(crate) margin_asset: u64,
    pub(crate) direction: String,
    pub(crate) margin_amount: BigDecimal,
    pub(crate) notional_amount: BigDecimal,
    pub(crate) interest_amount: BigDecimal,
    pub(crate) entry_price: Option<BigDecimal>,
    pub(crate) maintenance_margin_rate: BigDecimal,
    pub(crate) status: String,
}
/// 按用户和仓位标识读取风险计算所需快照，防止跨账户读取；该查询不加资金锁。
pub(crate) async fn load_user_risk_position_by_id(
    pool: &Pool<MySql>,
    user_id: u64,
    position_id: u64,
) -> AppResult<Option<MarginRiskPositionRow>> {
    sqlx::query_as::<_, MarginRiskPositionRow>(
        r#"SELECT positions.id, positions.pair_id, pairs.symbol, positions.margin_asset,
                  positions.direction, positions.margin_amount, positions.notional_amount,
                  positions.interest_amount, positions.entry_price,
                  products.maintenance_margin_rate, positions.status
           FROM margin_positions positions
           INNER JOIN margin_products products ON products.id = positions.product_id
           INNER JOIN trading_pairs pairs ON pairs.id = positions.pair_id
           WHERE positions.id = ? AND positions.user_id = ?
           LIMIT 1"#,
    )
    .bind(position_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

/// 按用户、状态和上限查询保证金仓位读模型；只读失败不返回部分结果。
pub(crate) async fn list_user_margin_positions(
    pool: &Pool<MySql>,
    user_id: u64,
    status: Option<&str>,
    limit: u32,
) -> AppResult<Vec<MarginPositionResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT positions.id, positions.user_id, positions.product_id, positions.pair_id,
                  positions.margin_asset, positions.wallet_scope, positions.margin_mode,
                  positions.direction, positions.margin_amount, positions.leverage,
                  positions.notional_amount, positions.borrowed_amount, positions.interest_amount,
                  positions.entry_price, positions.exit_price, positions.realized_pnl,
                  positions.closed_at, positions.status, positions.idempotency_key
           FROM margin_positions positions
           WHERE positions.user_id = "#,
    );
    builder.push_bind(user_id);
    if let Some(status) = status {
        builder.push(" AND positions.status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY positions.id DESC LIMIT ");
    builder.push_bind(limit as i64);
    builder
        .build_query_as::<MarginPositionResponse>()
        .fetch_all(pool)
        .await
        .map_err(AppError::from)
}

/// 读取用户保证金钱包及资产标识；该查询不修改余额，也不生成流水。
pub(crate) async fn list_margin_wallet_accounts(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Vec<MarginWalletAccountResponse>> {
    sqlx::query_as::<_, MarginWalletAccountResponse>(
        r#"SELECT wallets.asset_id, assets.symbol AS asset_symbol, assets.logo_url,
                  wallets.available, wallets.frozen, wallets.locked
           FROM margin_wallet_accounts wallets
           INNER JOIN assets ON assets.id = wallets.asset_id
           WHERE wallets.user_id = ?
           ORDER BY wallets.asset_id ASC"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

/// 读取用户全仓账户最近一次组合风险快照；风险 worker 会持续刷新这些字段。
/// 读取用户全仓账户风险快照并按保证金币种排序，不执行重新估值或强平。
pub(crate) async fn list_user_cross_margin_accounts(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Vec<MarginCrossAccountResponse>> {
    sqlx::query_as::<_, MarginCrossAccountResponse>(
        r#"SELECT margin_asset, status, last_equity AS equity,
                  last_unrealized_pnl AS unrealized_pnl,
                  last_interest_amount AS interest_amount,
                  last_maintenance_margin AS maintenance_margin,
                  last_margin_ratio AS margin_ratio
           FROM margin_cross_accounts
           WHERE user_id = ?
           ORDER BY margin_asset ASC"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

/// 按用户和仓位标识读取详情，记录缺失返回空以便应用层映射 NotFound。
pub(crate) async fn load_user_position_by_id(
    pool: &Pool<MySql>,
    user_id: u64,
    position_id: u64,
) -> AppResult<Option<MarginPositionResponse>> {
    sqlx::query_as::<_, MarginPositionResponse>(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, wallet_scope, margin_mode, direction, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
                  exit_price, realized_pnl, closed_at, status, idempotency_key
           FROM margin_positions
           WHERE id = ? AND user_id = ?
           LIMIT 1"#,
    )
    .bind(position_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

/// 后台仓位列表：行查询与 COUNT 共用同一组谓词，总数才会跟随当前筛选。
/// 后台仓位行与总数共享用户、邮箱、交易对及状态筛选，不修改仓位。
pub(crate) async fn list_admin_margin_positions(
    pool: &Pool<MySql>,
    user_id: Option<u64>,
    email: Option<String>,
    pair_id: Option<u64>,
    status: Option<&str>,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<AdminMarginPositionResponse>, i64)> {
    let mut rows = QueryBuilder::<MySql>::new(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, wallet_scope, margin_mode, direction, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
                  exit_price, realized_pnl, closed_at, liquidated_at, liquidation_reason, status,
                  idempotency_key
           FROM margin_positions"#,
    );
    let mut total = QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM margin_positions");
    for builder in [&mut rows, &mut total] {
        push_admin_margin_position_filters(builder, user_id, email.clone(), pair_id, status);
    }

    fetch_admin_page(pool, rows, total, " ORDER BY id DESC", limit, offset).await
}

fn push_admin_margin_position_filters(
    builder: &mut QueryBuilder<'_, MySql>,
    user_id: Option<u64>,
    email: Option<String>,
    pair_id: Option<u64>,
    status: Option<&str>,
) {
    builder.push(" WHERE 1 = 1");
    if let Some(user_id) = user_id {
        builder.push(" AND user_id = ");
        builder.push_bind(user_id);
    }
    push_user_email_filter(builder, "user_id", email);
    if let Some(pair_id) = pair_id {
        builder.push(" AND pair_id = ");
        builder.push_bind(pair_id);
    }
    if let Some(status) = status {
        builder.push(" AND status = ");
        builder.push_bind(status.to_owned());
    }
}

/// 读取后台仓位详情及用户、产品信息；该只读查询不触发利息计提或结算。
pub(crate) async fn load_admin_margin_position_by_id(
    pool: &Pool<MySql>,
    position_id: u64,
) -> AppResult<Option<AdminMarginPositionResponse>> {
    sqlx::query_as::<_, AdminMarginPositionResponse>(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, wallet_scope, margin_mode, direction, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
                  exit_price, realized_pnl, closed_at, liquidated_at, liquidation_reason, status,
                  idempotency_key
           FROM margin_positions
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(position_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

/// 后台资金费汇总：分组行与分组总数共用同一组谓词，总数按分组键去重统计。
/// 按同一筛选聚合仓位利息与分页总数，该查询不执行计提。
pub(crate) async fn list_admin_interest_summary(
    pool: &Pool<MySql>,
    user_id: Option<u64>,
    email: Option<String>,
    pair_id: Option<u64>,
    status: Option<&str>,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<AdminInterestSummaryItem>, i64)> {
    let mut rows = QueryBuilder::<MySql>::new(
        r#"SELECT margin_asset, status, COUNT(*) AS position_count,
                  COALESCE(SUM(borrowed_amount), 0) AS borrowed_amount,
                  COALESCE(SUM(interest_amount), 0) AS interest_amount
           FROM margin_positions"#,
    );
    let mut total = QueryBuilder::<MySql>::new(
        "SELECT COUNT(DISTINCT margin_asset, status) FROM margin_positions",
    );
    for builder in [&mut rows, &mut total] {
        push_admin_margin_position_filters(builder, user_id, email.clone(), pair_id, status);
    }
    // 分组键 (margin_asset, status) 本身唯一，排序无需再补主键。
    rows.push(" GROUP BY margin_asset, status");

    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY margin_asset ASC, status ASC",
        limit,
        offset,
    )
    .await
}
