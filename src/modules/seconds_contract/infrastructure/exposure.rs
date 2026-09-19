use super::*;
use crate::modules::seconds_contract::service::{maximum_gross_payout, validate_payout_capacity};

/// 产品锁后首次只读查幂等键，不锁订单或不存在键的间隙；完整响应复用原有读取投影。
/// 必须在预算判断和取行情之前调用，使已占满预算不改变同请求重放合同。
pub(crate) async fn replay_after_product_lock(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    key: &str,
) -> AppResult<Option<SecondsContractOrderResponse>> {
    let id: Option<u64> = sqlx::query_scalar(
        "SELECT id FROM seconds_contract_orders WHERE user_id = ? AND idempotency_key = ?",
    )
    .bind(user_id)
    .bind(key)
    .fetch_optional(&mut **tx)
    .await?;
    match id {
        Some(id) => Ok(Some(load_order_by_id(tx, id).await?)),
        None => Ok(None),
    }
}

/// 已持产品锁后按本产品同押注币种的订单快照累加最大毛兑付，候选取已选周期净收益率。
/// opened 与 manual_review 都尚有兑付义务；settled 或原子 refunded 终态才释放。
/// 不得在产品锁前建立快照。
/// 不锁其他订单，避免与结算的订单→资产/钱包锁序反转；并发结算释放至多保守多计。
pub(crate) async fn ensure_open_capacity(
    tx: &mut Transaction<'_, MySql>,
    product: &SecondsContractProductRuleRow,
    amount: &BigDecimal,
) -> AppResult<()> {
    let Some(capacity) = &product.open_payout_capacity else {
        return Ok(());
    };
    let rows: Vec<(BigDecimal, BigDecimal)> = sqlx::query_as(
        "SELECT stake_amount,payout_rate FROM seconds_contract_orders WHERE product_id=? AND stake_asset=? AND status IN ('opened','manual_review')",
    ).bind(product.id).bind(product.stake_asset).fetch_all(&mut **tx).await?;
    let mut total = maximum_gross_payout(amount, &product.payout_rate);
    for (stake, rate) in rows {
        total += maximum_gross_payout(&stake, &rate);
    }
    if total > *capacity {
        return Err(AppError::Validation(
            "seconds open gross payout capacity exceeded".into(),
        ));
    }
    Ok(())
}

/// 配置事务持产品锁后校验币种和预算精度；未结义务不允许通过换币种逃离预算。
/// 本函数仅验证配置，不结算、不退款，也不改写原订单赔率。
pub(crate) async fn validate_capacity_config(
    tx: &mut Transaction<'_, MySql>,
    product_id: Option<u64>,
    asset_id: u64,
    capacity: Option<&BigDecimal>,
) -> AppResult<()> {
    let precision = load_asset_precision_scale(tx, asset_id).await?;
    validate_payout_capacity(capacity, precision)?;
    if let Some(product_id) = product_id {
        let incompatible: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM seconds_contract_orders WHERE product_id=? AND stake_asset<>? AND status IN ('opened','manual_review'))",
        ).bind(product_id).bind(asset_id).fetch_one(&mut **tx).await?;
        if incompatible {
            return Err(AppError::Conflict(
                "cannot change seconds stake asset with unsettled obligations".into(),
            ));
        }
    }
    Ok(())
}
