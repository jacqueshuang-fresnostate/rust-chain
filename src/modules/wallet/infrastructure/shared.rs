//! 钱包事务共享 SQL 原语。
//!
//! 资金不变量：锁账户后才允许更新三桶余额；任何桶不得变负；余额更新与对应流水必须在调用方持有的同一事务中执行。

use crate::error::{AppError, AppResult};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool, QueryBuilder, Transaction};

/// 行查询与 COUNT 查询必须由同一组过滤谓词构建，返回总数才能与当前筛选一致。
/// 用同一组既定谓词分别执行后台行查询和计数查询。
/// 函数只追加排序与分页，确保 total 描述当前过滤结果而非全表。
/// 排序子句由调用方以字符串直接拼接，必须是代码内的常量且包含唯一列，否则同值行会在页间重复或丢失。
/// 两次查询各自独立取连接、不在同一事务内，因此并发写入下总数与当页行可能来自略有差异的快照。
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
/// 在调用方事务中按用户和资产锁定钱包三桶余额。
/// 先以幂等插入补齐缺失账户行，再用排他行锁回读 available、frozen、locked，保证后续扣减基于最新余额。
/// 幂等插入命中重复键时只空转更新时间戳，既不会覆盖既有余额，也不会把已有账户重置为零。
/// 资金流程应先锁业务单据再锁钱包；账户缺失时终止，避免隐式创建改变锁序。
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

/// 在调用方事务中一次写回 available、frozen、locked 三桶余额。
/// 入参是三桶的绝对目标值而非增量，未变化的桶也必须原样传入，否则会被覆盖成错误余额。
/// 写库前复核三桶均非负，任一桶为负立刻返回冲突错误并且不执行 UPDATE，作为领域校验之外的最后一道兜底。
/// 调用方须先持有账户行锁并完成非负校验；写入与对应流水必须同事务提交。
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
/// 在当前资金事务中追加一条含三桶后快照的账本记录。
/// amount 是有符号增量，balance_type 标明本条描述哪个桶，balance_after 必须与该桶的账后值一致。
/// 三桶 after 需要同时传入且取自同一次余额写回，账本因此可以独立还原任意时刻的完整账户状态。
/// 该函数只做单条插入，不校验金额符号与桶是否自洽，也不检测同一业务引用是否已存在流水。
/// 调用方负责保证余额更新先后顺序和业务引用幂等，插入失败必须回滚账户变更。
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
