//! 申购订单的冻结资金腿；不生成新币或解禁记录。
use super::*;

/// 在申购事务内从 available 等额转入 frozen，并保存两个桶的真实账后快照。
/// 调用方已锁项目与钱包、预留供给并插入 pending 订单；任一步失败须整体回滚。
pub(super) async fn freeze_subscription_quote_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    key: &str,
) -> AppResult<()> {
    let wallet = lock_wallet_row(tx, user_id, asset_id).await?;
    if wallet.available < *amount {
        return Err(AppError::Validation(
            "insufficient available balance for new coin subscription".into(),
        ));
    }
    let available = &wallet.available - amount;
    let frozen = &wallet.frozen + amount;
    sqlx::query(
        "UPDATE wallet_accounts SET available = ?, frozen = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&available)
    .bind(&frozen)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    for (delta, bucket, balance) in [
        (-amount.clone(), "available", &available),
        (amount.clone(), "frozen", &frozen),
    ] {
        insert_new_coin_wallet_ledger(
            tx,
            user_id,
            asset_id,
            delta,
            bucket,
            balance,
            &available,
            &frozen,
            &wallet.locked,
            "new_coin_subscription_freeze",
            "new_coin_subscription",
            key,
        )
        .await?;
    }
    Ok(())
}
