//! 全仓账户的串行化行锁、版本栅栏与风险仓位快照。
//!
//! 所有会改变全仓权益的路径先锁 `(user_id, margin_asset)` 账户行，再锁仓位，最后锁钱包。
//! 账户版本用于连接事务外行情预取与事务内权威快照；任一并发开平仓、计息或强平都会使旧版本失效。

use super::position_queries::MarginRiskPositionRow;
use crate::{
    error::{AppError, AppResult},
    modules::margin::domain::CrossMarginRiskState,
};
use chrono::{DateTime, Utc};
use sqlx::{MySql, Pool, Transaction, types::Json as SqlxJson};

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct CrossMarginAccountLock {
    pub(crate) status: String,
    pub(crate) version: u64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct MarginPositionAccountScope {
    pub(crate) user_id: u64,
    pub(crate) margin_asset: u64,
    pub(crate) margin_mode: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct MarginOpenProductAccountScope {
    pub(crate) margin_asset: u64,
    pub(crate) margin_mode: String,
    pub(crate) margin_modes: SqlxJson<Vec<String>>,
    pub(crate) status: String,
}

/// 只读账户状态供行情预取与 read model 使用；不存在表示该用户尚未建立全仓账户。
pub(crate) async fn load_cross_margin_account(
    pool: &Pool<MySql>,
    user_id: u64,
    margin_asset: u64,
) -> AppResult<Option<CrossMarginAccountLock>> {
    sqlx::query_as::<_, CrossMarginAccountLock>(
        r#"SELECT status, version
           FROM margin_cross_accounts
           WHERE user_id = ? AND margin_asset = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(margin_asset)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

/// 惰性创建并锁定账户行；不会重置 liquidating/liquidated 状态。
pub(crate) async fn ensure_and_lock_cross_margin_account(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    margin_asset: u64,
) -> AppResult<CrossMarginAccountLock> {
    ensure_and_lock_cross_margin_account_with_creation(tx, user_id, margin_asset)
        .await
        .map(|(account, _)| account)
}

/// 与普通账户锁入口相同，但同时告知调用方本事务是否新建了账户行。
///
/// 仅开仓路径需要这个标记：未成交的 cross 限价挂单不能留下账户行，因此它会在持锁事务结束前
/// 删除自己刚创建的空账户；既有 active 账户则保持原样。其他资金路径不应根据该标记改变账户生命周期。
pub(crate) async fn ensure_and_lock_cross_margin_account_with_creation(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    margin_asset: u64,
) -> AppResult<(CrossMarginAccountLock, bool)> {
    let insert = sqlx::query(
        "INSERT IGNORE INTO margin_cross_accounts (user_id, margin_asset) VALUES (?, ?)",
    )
    .bind(user_id)
    .bind(margin_asset)
    .execute(&mut **tx)
    .await?;
    let account = sqlx::query_as::<_, CrossMarginAccountLock>(
        r#"SELECT status, version
           FROM margin_cross_accounts
           WHERE user_id = ? AND margin_asset = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(margin_asset)
    .fetch_one(&mut **tx)
    .await
    .map_err(AppError::from)?;
    Ok((account, insert.rows_affected() == 1))
}

/// 删除本事务刚为未成交 cross 限价挂单创建的空账户，恢复“成交时才建账户”的既有合同。
pub(crate) async fn discard_new_cross_margin_account_for_pending_order(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    margin_asset: u64,
    expected_version: u64,
) -> AppResult<()> {
    let delete = sqlx::query(
        r#"DELETE FROM margin_cross_accounts
           WHERE user_id = ? AND margin_asset = ? AND status = 'active' AND version = ?"#,
    )
    .bind(user_id)
    .bind(margin_asset)
    .bind(expected_version)
    .execute(&mut **tx)
    .await?;
    if delete.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "new cross margin account changed before pending order commit".to_owned(),
        ));
    }
    Ok(())
}

/// 转出和普通风险变更只接受 active；liquidating/liquidated 均保持关闭状态。
pub(crate) fn require_active_cross_margin_account(
    account: &CrossMarginAccountLock,
) -> AppResult<()> {
    if account.status == "active" {
        Ok(())
    } else {
        Err(AppError::Conflict(format!(
            "cross margin account is {}",
            account.status
        )))
    }
}

/// 新开全仓允许在既有清算已完成后重新激活账户，但 liquidating 中仍拒绝。
pub(crate) async fn activate_cross_margin_account_for_open(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    margin_asset: u64,
    account: &mut CrossMarginAccountLock,
) -> AppResult<()> {
    if account.status == "active" {
        return Ok(());
    }
    if account.status == "liquidating" {
        return Err(AppError::Conflict(
            "cross margin account is liquidating".to_owned(),
        ));
    }
    let update = sqlx::query(
        r#"UPDATE margin_cross_accounts
           SET status = 'active', last_bad_debt = 0, version = version + 1
           WHERE user_id = ? AND margin_asset = ? AND version = ? AND status = 'liquidated'"#,
    )
    .bind(user_id)
    .bind(margin_asset)
    .bind(account.version)
    .execute(&mut **tx)
    .await?;
    if update.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "cross margin account changed while reopening".to_owned(),
        ));
    }
    account.status = "active".to_owned();
    account.version += 1;
    Ok(())
}

/// 对已锁账户执行版本条件递增，作为开平仓、成交和钱包变化的提交栅栏。
pub(crate) async fn bump_cross_margin_account_version(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    margin_asset: u64,
    expected_version: u64,
) -> AppResult<u64> {
    let update = sqlx::query(
        r#"UPDATE margin_cross_accounts
           SET version = version + 1
           WHERE user_id = ? AND margin_asset = ? AND version = ?"#,
    )
    .bind(user_id)
    .bind(margin_asset)
    .bind(expected_version)
    .execute(&mut **tx)
    .await?;
    if update.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "cross margin account version changed concurrently".to_owned(),
        ));
    }
    Ok(expected_version + 1)
}

/// 在持有账户锁时写入同一批行情算出的风险字段并递增版本，状态保持不变。
pub(crate) async fn update_locked_cross_margin_risk(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    margin_asset: u64,
    expected_version: u64,
    risk: &CrossMarginRiskState,
    observed_at: DateTime<Utc>,
) -> AppResult<u64> {
    let update = sqlx::query(
        r#"UPDATE margin_cross_accounts
           SET last_equity = ?, last_unrealized_pnl = ?, last_interest_amount = ?,
               last_maintenance_margin = ?, last_margin_ratio = ?, last_risk_at = ?,
               version = version + 1
           WHERE user_id = ? AND margin_asset = ? AND version = ?"#,
    )
    .bind(&risk.equity)
    .bind(&risk.unrealized_pnl)
    .bind(&risk.interest_amount)
    .bind(&risk.maintenance_margin)
    .bind(&risk.margin_ratio)
    .bind(observed_at.naive_utc())
    .bind(user_id)
    .bind(margin_asset)
    .bind(expected_version)
    .execute(&mut **tx)
    .await?;
    if update.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "cross margin account risk version changed concurrently".to_owned(),
        ));
    }
    Ok(expected_version + 1)
}

/// 按主键稳定顺序锁住账户中全部已成交仓位及其利息负债，并联出风险公式所需产品参数。
pub(crate) async fn lock_cross_margin_risk_positions(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    margin_asset: u64,
) -> AppResult<Vec<MarginRiskPositionRow>> {
    sqlx::query_as::<_, MarginRiskPositionRow>(
        r#"SELECT positions.id, positions.pair_id, pairs.symbol, pairs.price_precision,
                  positions.margin_asset, positions.direction, positions.margin_mode,
                  positions.margin_amount, positions.notional_amount, positions.interest_amount,
                  positions.entry_price, products.maintenance_margin_rate, positions.status
           FROM margin_positions positions
           INNER JOIN margin_products products ON products.id = positions.product_id
           INNER JOIN trading_pairs pairs ON pairs.id = positions.pair_id
           WHERE positions.user_id = ? AND positions.margin_asset = ?
             AND positions.margin_mode = 'cross' AND positions.status = 'opened'
             AND positions.entry_price IS NOT NULL
           ORDER BY positions.id ASC
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(margin_asset)
    .fetch_all(&mut **tx)
    .await
    .map_err(AppError::from)
}

/// 读取仓位不可变的账户归属，供写事务在加仓位锁前先取得全仓账户锁。
pub(crate) async fn load_margin_position_account_scope(
    pool: &Pool<MySql>,
    position_id: u64,
) -> AppResult<Option<MarginPositionAccountScope>> {
    sqlx::query_as::<_, MarginPositionAccountScope>(
        r#"SELECT user_id, margin_asset, margin_mode
           FROM margin_positions
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(position_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

/// 事务前读取产品模式和保证金币种，只用于确定账户锁键；事务内仍会锁产品并完整复核。
pub(crate) async fn load_margin_open_product_account_scope(
    pool: &Pool<MySql>,
    product_id: u64,
) -> AppResult<Option<MarginOpenProductAccountScope>> {
    sqlx::query_as::<_, MarginOpenProductAccountScope>(
        r#"SELECT margin_asset, margin_mode, margin_modes, status
           FROM margin_products
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(product_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}
