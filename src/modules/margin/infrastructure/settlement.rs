use super::ledger::{
    insert_margin_wallet_ledger, insert_spot_wallet_ledger, lock_existing_margin_wallet_row,
    lock_margin_wallet_row, lock_spot_wallet_row,
};
use crate::{
    error::{AppError, AppResult},
    modules::margin::presentation::MarginPositionResponse,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use sqlx::{MySql, Transaction};
/// 在开仓事务内按模式选择保证金或现货钱包，锁行后扣减抵押并写对应流水。
/// 全仓必须使用保证金钱包；逐仓优先既有保证金余额再回退现货，余额不足或任一步失败整体回滚。
pub(crate) async fn debit_margin_position_open_collateral(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    position_id: u64,
    margin_mode: &str,
) -> AppResult<String> {
    if margin_mode == "cross" {
        let margin_wallet = lock_margin_wallet_row(tx, user_id, asset_id).await?;
        if margin_wallet.available < *amount {
            return Err(AppError::Validation(format!(
                "insufficient margin wallet balance for cross position: requested {}, available {}",
                amount, margin_wallet.available
            )));
        }
        let available_after = margin_wallet.available.clone() - amount.clone();
        sqlx::query(
            "UPDATE margin_wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?",
        )
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
        insert_margin_wallet_ledger(
            tx,
            user_id,
            asset_id,
            "margin_position_open",
            &(-amount.clone()),
            &available_after,
            &available_after,
            &margin_wallet.frozen,
            &margin_wallet.locked,
            "margin_position",
            &position_id.to_string(),
        )
        .await?;
        return Ok("margin".to_owned());
    }

    if let Some(margin_wallet) = lock_existing_margin_wallet_row(tx, user_id, asset_id).await?
        && margin_wallet.available >= *amount
    {
        let available_after = margin_wallet.available.clone() - amount.clone();
        sqlx::query(
            "UPDATE margin_wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?",
        )
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
        insert_margin_wallet_ledger(
            tx,
            user_id,
            asset_id,
            "margin_position_open",
            &(-amount.clone()),
            &available_after,
            &available_after,
            &margin_wallet.frozen,
            &margin_wallet.locked,
            "margin_position",
            &position_id.to_string(),
        )
        .await?;
        return Ok("margin".to_owned());
    }

    let wallet = lock_spot_wallet_row(tx, user_id, asset_id).await?;
    if wallet.available < *amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for margin position: requested {}, available {}, locked {}",
            amount, wallet.available, wallet.locked
        )));
    }
    let available_after = wallet.available.clone() - amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    insert_spot_wallet_ledger(
        tx,
        user_id,
        asset_id,
        "margin_position_open",
        &(-amount.clone()),
        &available_after,
        &available_after,
        &wallet.frozen,
        &wallet.locked,
        "margin_position",
        &position_id.to_string(),
    )
    .await?;
    Ok("spot".to_owned())
}

/// 在调用方事务内回读仓位快照；记录缺失返回 NotFound，不改变锁序或资金状态。
pub(crate) async fn load_position_by_id(
    tx: &mut Transaction<'_, MySql>,
    position_id: u64,
) -> AppResult<MarginPositionResponse> {
    sqlx::query_as::<_, MarginPositionResponse>(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, wallet_scope, margin_mode, direction, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
                  exit_price, realized_pnl, closed_at, status, idempotency_key
           FROM margin_positions
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(position_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 在仓位结算事务内按已记录的 `wallet_scope` 返还正金额；零或负金额不产生余额与流水变更。
/// `margin` 锁杠杆钱包，`spot` 锁现货钱包，未知资金域直接报错，禁止静默回退到任一账户。
/// 可用余额增量、余额快照与指定结算流水必须保持同额并由调用方连同仓位终态一起提交。
/// 本函数不独立去重或提交；调用方必须先锁仓位并确认首次结算，提交后没有外部副作用。
pub(crate) async fn credit_margin_position_amount(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    wallet_scope: &str,
    amount: &BigDecimal,
    change_type: &str,
    position_id: u64,
) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        return Ok(());
    }
    match wallet_scope {
        "margin" => {
            let wallet = lock_margin_wallet_row(tx, user_id, asset_id).await?;
            let available_after = wallet.available.clone() + amount.clone();
            sqlx::query(
                "UPDATE margin_wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?",
            )
            .bind(&available_after)
            .bind(user_id)
            .bind(asset_id)
            .execute(&mut **tx)
            .await?;
            insert_margin_wallet_ledger(
                tx,
                user_id,
                asset_id,
                change_type,
                amount,
                &available_after,
                &available_after,
                &wallet.frozen,
                &wallet.locked,
                "margin_position",
                &position_id.to_string(),
            )
            .await
        }
        "spot" => {
            let wallet = lock_spot_wallet_row(tx, user_id, asset_id).await?;
            let available_after = wallet.available.clone() + amount.clone();
            sqlx::query(
                "UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?",
            )
            .bind(&available_after)
            .bind(user_id)
            .bind(asset_id)
            .execute(&mut **tx)
            .await?;
            insert_spot_wallet_ledger(
                tx,
                user_id,
                asset_id,
                change_type,
                amount,
                &available_after,
                &available_after,
                &wallet.frozen,
                &wallet.locked,
                "margin_position",
                &position_id.to_string(),
            )
            .await
        }
        _ => Err(AppError::Validation(
            "margin position wallet_scope must be spot or margin".to_owned(),
        )),
    }
}

/// 全仓单仓主动平仓使用有符号权益更新共享钱包，余额不足时必须交给账户级强平处理。
/// 调用方须先锁定账户与仓位；任一余额或流水写入失败必须回滚，禁止留下半结算状态。
pub(crate) async fn apply_cross_margin_position_settlement(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    position_id: u64,
) -> AppResult<()> {
    let wallet = lock_margin_wallet_row(tx, user_id, asset_id).await?;
    let available_after = (wallet.available.clone() + amount.clone()).with_scale(18);
    if available_after < 0 {
        return Err(AppError::Validation(
            "cross margin position loss exceeds shared available equity; account liquidation is required"
                .to_owned(),
        ));
    }
    if amount == &BigDecimal::from(0) {
        return Ok(());
    }
    sqlx::query(
        "UPDATE margin_wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&available_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    insert_margin_wallet_ledger(
        tx,
        user_id,
        asset_id,
        "margin_cross_position_close",
        amount,
        &available_after,
        &available_after,
        &wallet.frozen,
        &wallet.locked,
        "margin_position",
        &position_id.to_string(),
    )
    .await
}

#[derive(Debug)]
/// 全仓账户级结算结果，同时返回归零后的可用余额与极端跳空坏账。
pub(crate) struct CrossMarginAccountSettlement {
    pub(crate) available_after: BigDecimal,
    pub(crate) bad_debt: BigDecimal,
}

/// 全仓强平只在共享钱包上结算一次；极端跳空超过可用余额的部分单独记为坏账。
/// 账户、仓位、钱包和流水必须同事务提交；失败时不关闭部分仓位，也不产生外部结算事件。
pub(crate) async fn apply_cross_margin_account_settlement(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    portfolio_equity: &BigDecimal,
    reference_id: &str,
) -> AppResult<CrossMarginAccountSettlement> {
    let wallet = lock_margin_wallet_row(tx, user_id, asset_id).await?;
    let raw_available_after = (wallet.available.clone() + portfolio_equity.clone()).with_scale(18);
    let (available_after, bad_debt) = if raw_available_after < 0 {
        (
            BigDecimal::from(0).with_scale(18),
            (-raw_available_after).with_scale(18),
        )
    } else {
        (raw_available_after, BigDecimal::from(0).with_scale(18))
    };
    let applied_delta = (available_after.clone() - wallet.available.clone()).with_scale(18);
    if applied_delta != 0 {
        sqlx::query(
            "UPDATE margin_wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?",
        )
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
        insert_margin_wallet_ledger(
            tx,
            user_id,
            asset_id,
            "margin_cross_account_liquidate",
            &applied_delta,
            &available_after,
            &available_after,
            &wallet.frozen,
            &wallet.locked,
            "margin_cross_account",
            reference_id,
        )
        .await?;
    }
    Ok(CrossMarginAccountSettlement {
        available_after,
        bad_debt,
    })
}

/// 在结算事务内以 opened 条件原子写入关闭时间、退出价和已实现盈亏。
/// 受影响行不是一行即返回并发冲突，调用方必须回滚钱包结算与流水。
pub(crate) async fn mark_position_closed(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    position_id: u64,
    closed_at: DateTime<Utc>,
    exit_price: &BigDecimal,
    realized_pnl: &BigDecimal,
) -> AppResult<()> {
    let update_position = sqlx::query(
        r#"UPDATE margin_positions
           SET status = 'closed', closed_at = ?, exit_price = ?, realized_pnl = ?,
               next_liquidation_attempt_at = NULL
           WHERE id = ? AND user_id = ? AND status = 'opened'"#,
    )
    .bind(closed_at.naive_utc())
    .bind(exit_price)
    .bind(realized_pnl)
    .bind(position_id)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    if update_position.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "margin position close status changed concurrently".to_owned(),
        ));
    }
    Ok(())
}

/// 在取消事务内仅把未成交 opened 仓位原子迁移为 canceled 并记录关闭时间。
/// 状态条件不再满足时返回冲突，调用方必须回滚保证金退款及流水。
pub(crate) async fn mark_position_canceled(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    position_id: u64,
    closed_at: DateTime<Utc>,
) -> AppResult<()> {
    let update_position = sqlx::query(
        r#"UPDATE margin_positions
           SET status = 'canceled', closed_at = ?, next_liquidation_attempt_at = NULL
           WHERE id = ? AND user_id = ? AND status = 'opened' AND entry_price IS NULL"#,
    )
    .bind(closed_at.naive_utc())
    .bind(position_id)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    if update_position.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "margin position cancel status changed concurrently".to_owned(),
        ));
    }
    Ok(())
}
