//! 钱包事务共享 SQL 原语。
//!
//! 资金不变量：锁账户后才允许更新三桶余额；任何桶不得变负；余额更新与对应流水必须在调用方持有的同一事务中执行。

use crate::error::{AppError, AppResult};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool, QueryBuilder, Transaction};

/// 行查询与 COUNT 查询必须由同一组过滤谓词构建，返回总数才能与当前筛选一致。
pub(super) async fn fetch_admin_page<T>(
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
#[derive(Debug, sqlx::FromRow)]
pub(super) struct WalletBalanceRow {
    pub(super) available: BigDecimal,
    pub(super) frozen: BigDecimal,
    pub(super) locked: BigDecimal,
}
pub(super) async fn lock_wallet_balance(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<WalletBalanceRow> {
    sqlx::query(
        r#"INSERT INTO wallet_accounts (user_id, asset_id, available, frozen, locked)
           VALUES (?, ?, 0, 0, 0)
           ON DUPLICATE KEY UPDATE updated_at = updated_at"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query_as::<_, WalletBalanceRow>(
        r#"SELECT available, frozen, locked
           FROM wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(AppError::from)
}

pub(super) async fn update_wallet_balance(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    available: &BigDecimal,
    frozen: &BigDecimal,
    locked: &BigDecimal,
) -> AppResult<()> {
    if available < &BigDecimal::from(0)
        || frozen < &BigDecimal::from(0)
        || locked < &BigDecimal::from(0)
    {
        return Err(AppError::Conflict(
            "wallet balance mutation would produce a negative bucket".to_owned(),
        ));
    }
    sqlx::query(
        r#"UPDATE wallet_accounts
           SET available = ?, frozen = ?, locked = ?
           WHERE user_id = ? AND asset_id = ?"#,
    )
    .bind(available)
    .bind(frozen)
    .bind(locked)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn insert_wallet_ledger_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    change_type: &str,
    amount: &BigDecimal,
    balance_type: &str,
    balance_after: &BigDecimal,
    available_after: &BigDecimal,
    frozen_after: &BigDecimal,
    locked_after: &BigDecimal,
    ref_type: &str,
    ref_id: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(change_type)
    .bind(amount)
    .bind(balance_type)
    .bind(balance_after)
    .bind(available_after)
    .bind(frozen_after)
    .bind(locked_after)
    .bind(ref_type)
    .bind(ref_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
