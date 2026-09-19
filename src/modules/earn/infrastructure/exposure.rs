use crate::{
    error::{AppError, AppResult},
    modules::earn::{
        presentation::EarnSubscriptionResponse, repository::EarnProductRuleRow,
        service::earn_liability_reservation,
    },
};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Transaction};

/// 产品锁后重新检查幂等键；只读快照避免给不存在的键加间隙锁，同产品创建已被产品锁串行化。
/// 跨产品复用键仍由唯一约束和原有冲突回放处理，不提前锁其他订阅或钱包。
pub(crate) async fn load_subscription_replay(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    key: &str,
) -> AppResult<Option<EarnSubscriptionResponse>> {
    Ok(sqlx::query_as(
        r#"SELECT id, user_id, product_id, asset_id, amount, apr_rate,
                  redemption_fee_rate, maturity_profit_fee_rate, early_redeem_fee_basis,
                  early_redeem_fee_rate, term_days, status, idempotency_key,
                  subscribed_at, matures_at, redeemed_at
           FROM earn_subscriptions WHERE user_id = ? AND idempotency_key = ? LIMIT 1"#,
    )
    .bind(user_id)
    .bind(key)
    .fetch_optional(&mut **tx)
    .await?)
}

/// 已持有产品行锁后读取同币种全部在持订阅，按其原始 APR/期限保守预留全期毛兑付义务。
/// 调用方不得在取得产品锁前建立一致性快照；赎回不取产品锁，期间并发释放最多造成保守多计。
/// 此处不锁订阅，避免与手动/自动赎回的订阅→钱包锁序形成等待环；余额失败会由外层回滚申购。
pub(crate) async fn ensure_subscription_capacity(
    tx: &mut Transaction<'_, MySql>,
    product: &EarnProductRuleRow,
    amount: &BigDecimal,
) -> AppResult<()> {
    if product.principal_capacity.is_none() && product.liability_capacity.is_none() {
        return Ok(());
    }
    let rows: Vec<(BigDecimal, BigDecimal, u32)> = sqlx::query_as(
        "SELECT amount, apr_rate, term_days FROM earn_subscriptions WHERE product_id = ? AND asset_id = ? AND status = 'subscribed'",
    )
    .bind(product.id)
    .bind(product.asset_id)
    .fetch_all(&mut **tx)
    .await?;
    let mut principal = amount.clone();
    let mut liability = earn_liability_reservation(amount, &product.apr_rate, product.term_days);
    for (amount, apr, days) in rows {
        principal += &amount;
        liability += earn_liability_reservation(&amount, &apr, days);
    }
    for (total, limit, field) in [
        (
            principal,
            product.principal_capacity.as_ref(),
            "principal_capacity",
        ),
        (
            liability,
            product.liability_capacity.as_ref(),
            "liability_capacity",
        ),
    ] {
        if let Some(limit) = limit
            && total > *limit
        {
            return Err(AppError::Validation(format!(
                "earn exposure exceeds {field}"
            )));
        }
    }
    Ok(())
}

/// 产品配置事务中禁止对有在持订阅的产品切换币种，避免容量金额与历史义务单位混用。
pub(crate) async fn ensure_product_exposure_asset(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    asset_id: u64,
) -> AppResult<()> {
    let incompatible: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM earn_subscriptions WHERE product_id = ? AND asset_id <> ? AND status = 'subscribed')",
    )
    .bind(product_id)
    .bind(asset_id)
    .fetch_one(&mut **tx)
    .await?;
    if incompatible {
        return Err(AppError::Conflict(
            "cannot change earn asset while subscriptions remain".to_owned(),
        ));
    }
    Ok(())
}
