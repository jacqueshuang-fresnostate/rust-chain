use super::ledger::{
    insert_margin_wallet_ledger, insert_spot_wallet_ledger, lock_margin_wallet_row,
    lock_spot_wallet_row,
};
use crate::{
    error::{AppError, AppResult},
    modules::margin::presentation::MarginWalletAccountSnapshot,
};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool, Transaction};

#[derive(Debug, sqlx::FromRow)]
/// 保证金划转使用的启用资产标识、符号与金额精度规则。
pub(crate) struct MarginTransferAssetRule {
    pub(crate) id: u64,
    pub(crate) precision_scale: i32,
}

#[derive(Debug, sqlx::FromRow)]
/// 已持久化划转请求快照，用于同键同参重放及异参冲突判断。
pub(crate) struct MarginTransferRecord {
    pub(crate) transfer_id: String,
    pub(crate) asset_id: u64,
    pub(crate) from_account: String,
    pub(crate) to_account: String,
    pub(crate) amount: BigDecimal,
}
/// 按现货后保证金的稳定顺序锁定两侧钱包，将同额资金从现货转入保证金并各写流水。
/// 两侧余额、两笔流水与划转记录同事务提交；余额不足或任一步失败整体回滚。
pub(crate) async fn transfer_spot_to_margin_wallets(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    transfer_id: &str,
) -> AppResult<(MarginWalletAccountSnapshot, MarginWalletAccountSnapshot)> {
    // 双向划转统一先锁现货、再锁杠杆钱包，避免反向请求形成交叉等待。
    let spot_wallet = lock_spot_wallet_row(tx, user_id, asset_id).await?;
    if spot_wallet.available < *amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for margin transfer: requested {}, available {}, locked {}",
            amount, spot_wallet.available, spot_wallet.locked
        )));
    }
    let margin_wallet = lock_margin_wallet_row(tx, user_id, asset_id).await?;
    let spot_available_after = spot_wallet.available.clone() - amount.clone();
    let margin_available_after = margin_wallet.available.clone() + amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&spot_available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        "UPDATE margin_wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&margin_available_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    insert_spot_wallet_ledger(
        tx,
        user_id,
        asset_id,
        "margin_transfer_out",
        &(-amount.clone()),
        &spot_available_after,
        &spot_available_after,
        &spot_wallet.frozen,
        &spot_wallet.locked,
        "margin_transfer",
        transfer_id,
    )
    .await?;
    insert_margin_wallet_ledger(
        tx,
        user_id,
        asset_id,
        "margin_transfer_in",
        amount,
        &margin_available_after,
        &margin_available_after,
        &margin_wallet.frozen,
        &margin_wallet.locked,
        "margin_transfer",
        transfer_id,
    )
    .await?;
    Ok((
        MarginWalletAccountSnapshot {
            asset_id,
            available: spot_available_after,
            frozen: spot_wallet.frozen,
            locked: spot_wallet.locked,
        },
        MarginWalletAccountSnapshot {
            asset_id,
            available: margin_available_after,
            frozen: margin_wallet.frozen,
            locked: margin_wallet.locked,
        },
    ))
}

/// 在调用方事务内完成杠杆到现货划转；金额精度和资产有效性应已由应用层校验。
/// 即使资金方向相反也固定先锁现货钱包、再锁杠杆钱包，随后校验杠杆侧可用余额。
/// 杠杆可用余额扣减、现货可用余额增加及两条配对流水必须同事务提交，余额快照与流水一致。
/// 本函数不提交事务也不独立处理重放；调用方以划转幂等记录阻止重复动账。
pub(crate) async fn transfer_margin_to_spot_wallets(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    transfer_id: &str,
) -> AppResult<(MarginWalletAccountSnapshot, MarginWalletAccountSnapshot)> {
    // 与 spot -> margin 保持相同锁序。
    let spot_wallet = lock_spot_wallet_row(tx, user_id, asset_id).await?;
    let margin_wallet = lock_margin_wallet_row(tx, user_id, asset_id).await?;
    if margin_wallet.available < *amount {
        return Err(AppError::Validation(format!(
            "insufficient margin available balance for transfer: requested {}, available {}, locked {}",
            amount, margin_wallet.available, margin_wallet.locked
        )));
    }
    let margin_available_after = margin_wallet.available.clone() - amount.clone();
    let spot_available_after = spot_wallet.available.clone() + amount.clone();
    sqlx::query(
        "UPDATE margin_wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&margin_available_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&spot_available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    insert_margin_wallet_ledger(
        tx,
        user_id,
        asset_id,
        "margin_transfer_out",
        &(-amount.clone()),
        &margin_available_after,
        &margin_available_after,
        &margin_wallet.frozen,
        &margin_wallet.locked,
        "margin_transfer",
        transfer_id,
    )
    .await?;
    insert_spot_wallet_ledger(
        tx,
        user_id,
        asset_id,
        "margin_transfer_in",
        amount,
        &spot_available_after,
        &spot_available_after,
        &spot_wallet.frozen,
        &spot_wallet.locked,
        "margin_transfer",
        transfer_id,
    )
    .await?;
    Ok((
        MarginWalletAccountSnapshot {
            asset_id,
            available: spot_available_after,
            frozen: spot_wallet.frozen,
            locked: spot_wallet.locked,
        },
        MarginWalletAccountSnapshot {
            asset_id,
            available: margin_available_after,
            frozen: margin_wallet.frozen,
            locked: margin_wallet.locked,
        },
    ))
}

/// 在划转事务内解析并锁定启用资产，显式 id 与 symbol 不一致时拒绝请求。
pub(crate) async fn resolve_active_transfer_asset(
    tx: &mut Transaction<'_, MySql>,
    asset_id: Option<u64>,
    asset_symbol: Option<&str>,
) -> AppResult<MarginTransferAssetRule> {
    if let Some(asset_id) = asset_id {
        return sqlx::query_as::<_, MarginTransferAssetRule>(
            "SELECT id, precision_scale FROM assets WHERE id = ? AND status = 'active' LIMIT 1",
        )
        .bind(asset_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound);
    }
    let Some(symbol) = asset_symbol
        .map(str::trim)
        .filter(|symbol| !symbol.is_empty())
    else {
        return Err(AppError::Validation(
            "margin transfer asset_id or asset_symbol is required".to_owned(),
        ));
    };
    sqlx::query_as::<_, MarginTransferAssetRule>(
        r#"SELECT id, precision_scale
           FROM assets
           WHERE UPPER(symbol) = UPPER(?) AND status = 'active'
           LIMIT 1"#,
    )
    .bind(symbol)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 为幂等重放解析资产标识，即使资产后来停用也允许核对原请求但不新增划转。
pub(crate) async fn resolve_transfer_asset_id_for_replay(
    pool: &Pool<MySql>,
    asset_id: Option<u64>,
    asset_symbol: Option<&str>,
) -> AppResult<u64> {
    if let Some(asset_id) = asset_id {
        return sqlx::query_scalar::<_, u64>("SELECT id FROM assets WHERE id = ? LIMIT 1")
            .bind(asset_id)
            .fetch_optional(pool)
            .await?
            .ok_or(AppError::NotFound);
    }
    let Some(symbol) = asset_symbol
        .map(str::trim)
        .filter(|symbol| !symbol.is_empty())
    else {
        return Err(AppError::Validation(
            "margin transfer asset_id or asset_symbol is required".to_owned(),
        ));
    };
    sqlx::query_scalar::<_, u64>("SELECT id FROM assets WHERE UPPER(symbol) = UPPER(?) LIMIT 1")
        .bind(symbol)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

#[allow(clippy::too_many_arguments)]
/// 在资金变更前写入划转请求快照并占用用户幂等键，唯一键阻止并发重复动账。
pub(crate) async fn insert_margin_transfer(
    tx: &mut Transaction<'_, MySql>,
    transfer_id: &str,
    user_id: u64,
    asset_id: u64,
    from_account: &str,
    to_account: &str,
    amount: &BigDecimal,
    idempotency_key: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO margin_transfers
           (transfer_id, user_id, asset_id, from_account, to_account, amount, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(transfer_id)
    .bind(user_id)
    .bind(asset_id)
    .bind(from_account)
    .bind(to_account)
    .bind(amount)
    .bind(idempotency_key)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 按用户和幂等键读取既有划转请求，供同参重放与异参冲突判断。
pub(crate) async fn load_margin_transfer_by_idempotency_key(
    pool: &Pool<MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<MarginTransferRecord>> {
    sqlx::query_as::<_, MarginTransferRecord>(
        r#"SELECT transfer_id, asset_id, from_account, to_account, amount
           FROM margin_transfers
           WHERE user_id = ? AND idempotency_key = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

/// 读取既有划转两侧流水的余额后快照，重建原响应而不再次移动资金。
pub(crate) async fn load_margin_transfer_wallet_snapshots(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    transfer_id: &str,
) -> AppResult<(MarginWalletAccountSnapshot, MarginWalletAccountSnapshot)> {
    // 幂等重放必须使用原划转流水的 after 快照，不能泄漏后续交易形成的当前余额。
    let spot_wallet = sqlx::query_as::<_, MarginWalletAccountSnapshot>(
        r#"SELECT asset_id, available_after AS available, frozen_after AS frozen,
                  locked_after AS locked
           FROM wallet_ledger
           WHERE user_id = ? AND asset_id = ?
             AND ref_type = 'margin_transfer' AND ref_id = ?
           ORDER BY id ASC
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(transfer_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| {
        AppError::Internal(format!(
            "margin transfer {transfer_id} is missing its spot wallet ledger snapshot"
        ))
    })?;
    let margin_wallet = sqlx::query_as::<_, MarginWalletAccountSnapshot>(
        r#"SELECT asset_id, available_after AS available, frozen_after AS frozen,
                  locked_after AS locked
           FROM margin_wallet_ledger
           WHERE user_id = ? AND asset_id = ?
             AND ref_type = 'margin_transfer' AND ref_id = ?
           ORDER BY id ASC
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(transfer_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| {
        AppError::Internal(format!(
            "margin transfer {transfer_id} is missing its margin wallet ledger snapshot"
        ))
    })?;
    Ok((spot_wallet, margin_wallet))
}
