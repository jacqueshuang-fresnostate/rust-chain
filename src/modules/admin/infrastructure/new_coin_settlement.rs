//! 人工派发的申购结算持久化；订单、资金、供给与派发由应用层同一事务提交。
use super::*;
use crate::modules::new_coin::service::manual_new_coin_settlement_amounts;

#[derive(sqlx::FromRow)]
struct SubscriptionSettlement {
    settlement_mode: String,
    status: String,
    quote_asset: u64,
    issue_price: BigDecimal,
    quote_amount: BigDecimal,
    requested_quantity: BigDecimal,
    allocated_quantity: BigDecimal,
    frozen_quote_amount: BigDecimal,
}

/// 项目锁后读取已提交派发收据；同键重放在生命周期校验前处理，避免上市后丢失首次结果。
pub(crate) async fn find_admin_new_coin_distribution_in_tx(
    tx: &mut Transaction<'_, MySql>,
    key: &str,
) -> AppResult<Option<NewCoinDistributionResponse>> {
    sqlx::query_as("SELECT id, project_id, user_id, subscription_id, asset_id, quantity, lock_position_id, status, idempotency_key, created_at FROM new_coin_distributions WHERE idempotency_key = ? LIMIT 1")
        .bind(key).fetch_optional(&mut **tx).await.map_err(AppError::from)
}

/// 上市前锁定查询未结算的新模式订单；调用方持有项目锁，阻止申购/派发竞争越过检查。
pub(crate) async fn ensure_manual_new_coin_subscriptions_settled_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
) -> AppResult<()> {
    let pending: Option<(u64,)> = sqlx::query_as("SELECT id FROM new_coin_subscriptions WHERE project_id = ? AND settlement_mode = 'manual_distribution' AND (status = 'pending' OR frozen_quote_amount > 0) LIMIT 1 FOR UPDATE")
        .bind(project_id).fetch_optional(&mut **tx).await?;
    if pending.is_some() {
        return Err(AppError::Conflict(
            "new coin subscriptions must be distributed or refunded before listing".into(),
        ));
    }
    Ok(())
}

/// 一次最终结算新模式订单：按价格快照扣冻结款、退差额并释放未派发预留。
/// 旧模式返回 None，沿用原派发路径；新模式只允许 pending，不能重复结算或借用其他订单冻结款。
/// 调用方须已按资产 ID 顺序锁定资产/钱包并校验数量精度，后续发币失败必须回滚本次全部更新。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn settle_manual_new_coin_subscription_in_tx(
    tx: &mut Transaction<'_, MySql>,
    subscription_id: u64,
    project_id: u64,
    user_id: u64,
    quote_asset: u64,
    quantity: &BigDecimal,
    quote_precision: i32,
) -> AppResult<Option<BigDecimal>> {
    let order: SubscriptionSettlement = sqlx::query_as("SELECT settlement_mode, status, quote_asset, issue_price, quote_amount, requested_quantity, allocated_quantity, frozen_quote_amount FROM new_coin_subscriptions WHERE id = ? AND project_id = ? AND user_id = ? FOR UPDATE")
        .bind(subscription_id).bind(project_id).bind(user_id).fetch_optional(&mut **tx).await?.ok_or(AppError::NotFound)?;
    if order.settlement_mode == "legacy_instant" {
        return Ok(None);
    }
    if order.settlement_mode != "manual_distribution" || order.status != "pending" {
        return Err(AppError::Conflict(
            "new coin subscription has already been settled".into(),
        ));
    }
    if order.quote_asset != quote_asset
        || order.allocated_quantity != 0
        || order.frozen_quote_amount != order.quote_amount
    {
        return Err(AppError::Conflict(
            "new coin subscription frozen obligation is inconsistent".into(),
        ));
    }
    let (payment, refund) = manual_new_coin_settlement_amounts(
        &order.requested_quantity,
        quantity,
        &order.issue_price,
        &order.quote_amount,
        quote_precision,
    )?;
    let wallet = lock_or_create_admin_wallet_row_in_tx(tx, user_id, quote_asset).await?;
    if wallet.frozen < order.frozen_quote_amount {
        return Err(AppError::Conflict(
            "insufficient frozen balance for new coin settlement".into(),
        ));
    }
    let frozen_after_payment = &wallet.frozen - &payment;
    let frozen_after = &wallet.frozen - &order.frozen_quote_amount;
    let available_after = &wallet.available + &refund;
    sqlx::query(
        "UPDATE wallet_accounts SET available = ?, frozen = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&available_after)
    .bind(&frozen_after)
    .bind(user_id)
    .bind(quote_asset)
    .execute(&mut **tx)
    .await?;
    let reference = subscription_id.to_string();
    if payment > 0 {
        insert_admin_wallet_ledger_in_tx(
            tx,
            user_id,
            quote_asset,
            -payment.clone(),
            "frozen",
            &frozen_after_payment,
            &wallet.available,
            &frozen_after_payment,
            &wallet.locked,
            "new_coin_subscription_payment",
            "new_coin_subscription",
            &reference,
        )
        .await?;
    }
    if refund > 0 {
        for (amount, bucket, balance) in [
            (-refund.clone(), "frozen", &frozen_after),
            (refund.clone(), "available", &available_after),
        ] {
            insert_admin_wallet_ledger_in_tx(
                tx,
                user_id,
                quote_asset,
                amount,
                bucket,
                balance,
                &available_after,
                &frozen_after,
                &wallet.locked,
                "new_coin_subscription_refund",
                "new_coin_subscription",
                &reference,
            )
            .await?;
        }
    }
    let status = if quantity == &BigDecimal::from(0) {
        "refunded"
    } else if quantity == &order.requested_quantity {
        "allocated"
    } else {
        "partial_allocated"
    };
    sqlx::query("UPDATE new_coin_subscriptions SET allocated_quantity = ?, frozen_quote_amount = 0, settled_quote_amount = ?, refunded_quote_amount = ?, status = ? WHERE id = ?")
        .bind(quantity).bind(&payment).bind(&refund).bind(status).bind(subscription_id).execute(&mut **tx).await?;
    let unused = &order.requested_quantity - quantity;
    // 只退回该订单未使用的预留；实际派发数量仍由既有 finalize 转为已入账供给。
    if unused > 0 {
        let result = sqlx::query("UPDATE new_coin_projects SET reserved_supply = reserved_supply - ?, remaining_supply = remaining_supply + ? WHERE id = ? AND reserved_supply >= ?")
            .bind(&unused).bind(&unused).bind(project_id).bind(&unused).execute(&mut **tx).await?;
        if result.rows_affected() != 1 {
            return Err(AppError::Conflict(
                "new coin subscription supply reservation is inconsistent".into(),
            ));
        }
    }
    Ok(Some(payment))
}
