use crate::error::{AppError, AppResult};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Transaction};

#[derive(Debug, sqlx::FromRow)]
/// 钱包行锁读取的可用、冻结与锁定三桶快照，后续资金流水必须引用同一版本。
pub(super) struct MarginWalletRow {
    pub(super) available: BigDecimal,
    pub(super) frozen: BigDecimal,
    pub(super) locked: BigDecimal,
}
/// 在调用方事务内对用户现货钱包执行 FOR UPDATE，固定保证金扣款或退款前的三桶余额。
/// 账户不存在或锁失败即终止；本函数不改余额，调用方须按统一锁序继续并同事务写流水。
pub(super) async fn lock_spot_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<MarginWalletRow> {
    sqlx::query_as::<_, MarginWalletRow>(
        r#"SELECT available, frozen, locked
           FROM wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Validation("wallet account is required for margin".to_owned()))
}

/// 在调用方事务内用唯一键补齐零余额保证金钱包，供后续行锁与划转使用。
/// 并发或重放不会创建重复账户；该步骤不记资金流水，也不独立提交。
pub(super) async fn ensure_margin_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT IGNORE INTO margin_wallet_accounts (user_id, asset_id, available, frozen, locked)
           VALUES (?, ?, 0, 0, 0)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在调用方事务内确保并锁定用户保证金钱包，固定划转、开仓或结算使用的余额快照。
/// 调用方须遵守现货后保证金的锁序；锁取失败时不得继续余额与流水写入。
pub(super) async fn lock_margin_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<MarginWalletRow> {
    ensure_margin_wallet_row(tx, user_id, asset_id).await?;
    sqlx::query_as::<_, MarginWalletRow>(
        r#"SELECT available, frozen, locked
           FROM margin_wallet_accounts
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

/// 在调用方事务内尝试锁定既有保证金钱包；账户不存在返回空而不自动建账。
/// 该读锁不改变余额，调用方根据结果选择资金域并负责同事务提交或回滚。
pub(super) async fn lock_existing_margin_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<Option<MarginWalletRow>> {
    sqlx::query_as::<_, MarginWalletRow>(
        r#"SELECT available, frozen, locked
           FROM margin_wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

#[allow(clippy::too_many_arguments)] // 账本必须显式记录三桶快照和业务引用，聚合会降低资金审计可读性。
/// 在调用方事务内追加现货钱包流水，三桶余额后快照必须对应同次保证金资金变更。
/// 写入失败由调用方连同钱包与仓位回滚；同一业务重放不得产生第二笔流水。
pub(super) async fn insert_spot_wallet_ledger(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    change_type: &str,
    amount: &BigDecimal,
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
           VALUES (?, ?, ?, ?, 'available', ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(change_type)
    .bind(amount)
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

#[allow(clippy::too_many_arguments)] // 账本必须显式记录三桶快照和业务引用，聚合会降低资金审计可读性。
/// 在调用方事务内追加保证金钱包流水，金额及三桶快照必须与余额更新保持一致。
/// 写入失败由调用方整体回滚；幂等键或仓位终态须阻止重复记账。
pub(super) async fn insert_margin_wallet_ledger(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    change_type: &str,
    amount: &BigDecimal,
    balance_after: &BigDecimal,
    available_after: &BigDecimal,
    frozen_after: &BigDecimal,
    locked_after: &BigDecimal,
    ref_type: &str,
    ref_id: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO margin_wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, ?, ?, 'available', ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(change_type)
    .bind(amount)
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
