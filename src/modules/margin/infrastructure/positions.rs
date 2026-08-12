use super::query_support::zero_amount;
use crate::{
    error::{AppError, AppResult},
    modules::margin::presentation::MarginPositionResponse,
};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};

#[derive(Debug, sqlx::FromRow)]
/// 开仓事务锁定的产品、交易对、保证金币种、模式和杠杆规则快照。
pub(crate) struct MarginOpenProductRule {
    pub(crate) id: u64,
    pub(crate) pair_id: u64,
    pub(crate) symbol: String,
    pub(crate) margin_asset: u64,
    pub(crate) margin_mode: String,
    pub(crate) margin_modes: SqlxJson<Vec<String>>,
    pub(crate) leverage_levels: SqlxJson<Vec<String>>,
    pub(crate) min_margin: BigDecimal,
    pub(crate) max_margin: Option<BigDecimal>,
    pub(crate) hourly_interest_rate: BigDecimal,
    pub(crate) status: String,
}

#[derive(Debug, sqlx::FromRow)]
/// 关闭或取消事务锁定的仓位快照，包含资金域、入场价、利息和当前状态。
pub(crate) struct LockedMarginPositionRow {
    pub(crate) id: u64,
    pub(crate) pair_id: u64,
    pub(crate) symbol: String,
    pub(crate) margin_asset: u64,
    pub(crate) wallet_scope: String,
    pub(crate) margin_mode: String,
    pub(crate) direction: String,
    pub(crate) margin_amount: BigDecimal,
    pub(crate) notional_amount: BigDecimal,
    pub(crate) interest_amount: BigDecimal,
    pub(crate) entry_price: Option<BigDecimal>,
    pub(crate) status: String,
}
/// 按用户及可选产品查询可平仓主键并稳定升序返回，供批处理逐笔加锁。
pub(crate) async fn load_open_position_ids(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: Option<u64>,
) -> AppResult<Vec<u64>> {
    let mut builder =
        QueryBuilder::<MySql>::new("SELECT id FROM margin_positions WHERE user_id = ");
    builder.push_bind(user_id);
    builder.push(" AND status = 'opened' AND entry_price IS NOT NULL");
    if let Some(product_id) = product_id {
        builder.push(" AND product_id = ");
        builder.push_bind(product_id);
    }
    builder.push(" ORDER BY id ASC");
    builder
        .build_query_scalar::<u64>()
        .fetch_all(pool)
        .await
        .map_err(AppError::from)
}

/// 按用户及可选产品查询未成交可取消仓位主键，避免批处理触碰已成交仓位。
pub(crate) async fn load_cancelable_position_ids(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: Option<u64>,
) -> AppResult<Vec<u64>> {
    let mut builder =
        QueryBuilder::<MySql>::new("SELECT id FROM margin_positions WHERE user_id = ");
    builder.push_bind(user_id);
    builder.push(" AND status = 'opened' AND entry_price IS NULL");
    if let Some(product_id) = product_id {
        builder.push(" AND product_id = ");
        builder.push_bind(product_id);
    }
    builder.push(" ORDER BY id ASC");
    builder
        .build_query_scalar::<u64>()
        .fetch_all(pool)
        .await
        .map_err(AppError::from)
}

/// 在开仓事务内按用户与幂等键锁定既有仓位，用于并发重复请求的逐字段核对。
/// 未命中不产生写入；命中后调用方必须复用原结果或在异参时返回冲突。
pub(crate) async fn existing_position_for_idempotency_key(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<MarginPositionResponse>> {
    sqlx::query_as::<_, MarginPositionResponse>(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, wallet_scope, margin_mode, direction, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
                  exit_price, realized_pnl, closed_at, status, idempotency_key
           FROM margin_positions
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

/// 在事务前只读检查用户幂等键对应仓位，便于重放绕过行情和钱包扣款。
/// 异参判断由应用层执行；查询失败不得继续创建新仓位。
pub(crate) async fn existing_position_for_idempotency_key_readonly(
    pool: &Pool<MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<MarginPositionResponse>> {
    sqlx::query_as::<_, MarginPositionResponse>(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, wallet_scope, margin_mode, direction, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
                  exit_price, realized_pnl, closed_at, status, idempotency_key
           FROM margin_positions
           WHERE user_id = ? AND idempotency_key = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

/// 在开仓事务内锁定启用产品及交易对规则，固定杠杆、模式和保证金币种快照。
/// 产品不存在或停用即失败；调用方在该锁后写仓位并扣抵押。
pub(crate) async fn lock_active_open_product(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<MarginOpenProductRule> {
    let product = sqlx::query_as::<_, MarginOpenProductRule>(
        r#"SELECT products.id, products.pair_id, pairs.symbol, products.margin_asset,
                  products.margin_mode, products.margin_modes, products.leverage_levels, products.min_margin,
                  products.max_margin, products.hourly_interest_rate, products.status
           FROM margin_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           WHERE products.id = ?
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

#[allow(clippy::too_many_arguments)] // 仓位快照字段与 SQL 列一一对应，事务边界由应用层持有。
/// 在调用方事务内写入开仓快照并占用用户幂等键，入场价来自服务端行情缓存。
/// 唯一键冲突交由应用层重放核对；插入失败不得继续扣抵押或写资金流水。
pub(crate) async fn insert_margin_position(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    product: &MarginOpenProductRule,
    margin_mode: &str,
    direction: &str,
    margin_amount: &BigDecimal,
    leverage: &BigDecimal,
    notional_amount: &BigDecimal,
    borrowed_amount: &BigDecimal,
    entry_price: &BigDecimal,
    idempotency_key: &str,
) -> Result<u64, sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO margin_positions
           (user_id, product_id, pair_id, margin_asset, margin_mode, direction, margin_amount,
            leverage, notional_amount, borrowed_amount, interest_amount, interest_accrued_at,
            entry_price, status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP(6), ?, 'opened', ?)"#,
    )
    .bind(user_id)
    .bind(product.id)
    .bind(product.pair_id)
    .bind(product.margin_asset)
    .bind(margin_mode)
    .bind(direction)
    .bind(margin_amount)
    .bind(leverage)
    .bind(notional_amount)
    .bind(borrowed_amount)
    .bind(zero_amount())
    .bind(entry_price)
    .bind(idempotency_key)
    .execute(&mut **tx)
    .await
    .map(|result| result.last_insert_id())
}

/// 在开仓事务内记录抵押实际来源为 spot 或 margin，供关闭、取消和强平原路返还。
/// 更新失败须回滚仓位、钱包与流水，禁止以默认资金域替代缺失快照。
pub(crate) async fn set_margin_position_wallet_scope(
    tx: &mut Transaction<'_, MySql>,
    position_id: u64,
    wallet_scope: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE margin_positions SET wallet_scope = ? WHERE id = ?")
        .bind(wallet_scope)
        .bind(position_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 确保全仓账户存在；账户按用户和保证金资产唯一，避免不同交易对各自分账。
pub(crate) async fn ensure_cross_margin_account(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    margin_asset: u64,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO margin_cross_accounts (user_id, margin_asset) VALUES (?, ?) ON DUPLICATE KEY UPDATE status = 'active'",
    )
    .bind(user_id)
    .bind(margin_asset)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 按用户和仓位标识执行 FOR UPDATE，固定关闭或取消前的状态与资金来源。
/// 未命中返回空；调用方必须先锁仓位再锁钱包，并在同一事务更新终态。
pub(crate) async fn lock_user_position_by_id(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    position_id: u64,
) -> AppResult<Option<LockedMarginPositionRow>> {
    sqlx::query_as::<_, LockedMarginPositionRow>(
        r#"SELECT positions.id, positions.pair_id, pairs.symbol,
                  positions.margin_asset, positions.wallet_scope, positions.margin_mode,
                  positions.direction, positions.margin_amount,
                  positions.notional_amount, positions.interest_amount, positions.entry_price,
                  positions.status
           FROM margin_positions positions
           INNER JOIN trading_pairs pairs ON pairs.id = positions.pair_id
           WHERE positions.id = ? AND positions.user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(position_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}
