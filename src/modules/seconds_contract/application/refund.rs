//! 前瞻性策略配置及无历史证据订单的显式本金退款，不激活默认策略或补录旧订单权利。

use super::*;
use crate::modules::seconds_contract::{
    infrastructure::refund as store,
    presentation::refund::{
        PrincipalRefundReceipt, RefundContext, RefundPolicy, RefundPrincipalRequest,
        UpdateRefundPolicy,
    },
};
use serde_json::json;
use sqlx::Acquire;

fn reason(value: &str) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 512 {
        return Err(AppError::Validation(
            "refund reason must contain 1 to 512 characters".into(),
        ));
    }
    Ok(value.into())
}

/// 返回产品退款策略，不写默认值，不触发退款或改变现有订单资格。
pub(crate) async fn get_policy(pool: &Pool<MySql>, product_id: u64) -> AppResult<RefundPolicy> {
    let mut tx = pool.begin().await?;
    infrastructure::load_product_by_id(&mut tx, product_id).await?;
    let policy = store::load_policy(&mut tx, product_id, false).await?;
    tx.commit().await?;
    Ok(policy)
}

/// 版本 CAS 配置只影响未来开仓；禁用必须清空等待时间，启用必须显式给出间隔（允许显式零）。
/// 产品锁与开仓串行化，版本记录和认证管理员审计原子提交，不改写历史订单。
pub(crate) async fn update_policy(
    pool: &Pool<MySql>,
    admin_id: u64,
    product_id: u64,
    request: UpdateRefundPolicy,
) -> AppResult<RefundPolicy> {
    let reason = reason(&request.reason)?;
    if request.enabled != request.wait_seconds.is_some() {
        return Err(AppError::Validation(
            "enabled refund policy requires explicit wait_seconds; disabled requires null".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    store::lock_policy_product(&mut tx, product_id).await?;
    let before = store::load_policy(&mut tx, product_id, true).await?;
    if before.version != request.expected_version {
        return Err(AppError::Conflict(
            "seconds refund policy version changed".into(),
        ));
    }
    let after = RefundPolicy {
        product_id,
        version: before
            .version
            .checked_add(1)
            .ok_or_else(|| AppError::Conflict("refund policy version exhausted".into()))?,
        enabled: request.enabled,
        wait_seconds: request.wait_seconds,
    };
    store::write_policy(&mut tx, &after, admin_id, &reason).await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "seconds_refund_policy.update",
        "seconds_contract_product",
        product_id,
        Some(json!(before)),
        Some(json!(after)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

/// 只读退款快照、原异常起点与成功回执；显示的到时不等同于证据/钱包已校验。
pub(crate) async fn get_refund_context(
    pool: &Pool<MySql>,
    order_id: u64,
) -> AppResult<RefundContext> {
    let mut tx = pool.begin().await?;
    infrastructure::load_order_by_id(&mut tx, order_id).await?;
    let snapshot = store::load_snapshot(&mut tx, order_id).await?;
    let exception = store::load_exception(&mut tx, order_id).await?;
    let eligible_at = snapshot
        .as_ref()
        .zip(exception.as_ref())
        .and_then(|(s, e)| {
            e.detected_at
                .checked_add_signed(chrono::TimeDelta::seconds(i64::from(s.wait_seconds)))
        });
    let context = RefundContext {
        order_id,
        policy_version: snapshot.as_ref().map(|s| s.policy_version),
        wait_seconds: snapshot.as_ref().map(|s| s.wait_seconds),
        eligible_at,
        checked_at: infrastructure::database_now(&mut tx).await?,
        receipt: store::load_receipt(&mut tx, order_id).await?,
    };
    tx.commit().await?;
    Ok(context)
}

/// 认证管理员退还唯一原本金；锁序为来源订单、历史窗口、佣金、钱包。
/// 同 actor/key/规范化原因返回原凭证；新请求须有开仓时策略且原窗口无可用证据，DB/证据错误拒绝。
/// 来源终态、待付佣金拒绝、钱包、账本、平台分录与审计同事务提交；不猜输赢或回收已付佣金。
async fn refund_principal(
    pool: &Pool<MySql>,
    admin_id: u64,
    order_id: u64,
    request: RefundPrincipalRequest,
) -> AppResult<(PrincipalRefundReceipt, bool)> {
    let reason = reason(&request.reason)?;
    crate::modules::admin::service::validate_commission_reversal_key(&request.idempotency_key)?;
    let mut connection = store::refund_connection(pool).await?;
    let mut tx = connection.begin().await?;
    store::lock_source_order(&mut tx, order_id).await?;
    if let Some(receipt) = store::load_receipt(&mut tx, order_id).await? {
        if receipt.admin_id != admin_id
            || receipt.idempotency_key != request.idempotency_key
            || receipt.reason != reason
        {
            return Err(AppError::Conflict(
                "seconds refund request conflicts with immutable receipt".into(),
            ));
        }
        tx.commit().await?;
        return Ok((receipt, false));
    }
    let order = infrastructure::load_order_by_id(&mut tx, order_id).await?;
    if order.status != "manual_review"
        || order.result.is_some()
        || order.settlement_price.is_some()
        || order.settlement_price_tick_id.is_some()
    {
        return Err(AppError::Conflict(
            "only unresolved manual_review orders may refund principal".into(),
        ));
    }
    let snapshot = store::load_snapshot(&mut tx, order_id)
        .await?
        .filter(|s| s.product_id == order.product_id)
        .ok_or_else(|| {
            AppError::Conflict("order has no opening-time refund policy snapshot".into())
        })?;
    let exception = store::load_exception(&mut tx, order_id)
        .await?
        .ok_or_else(|| AppError::Conflict("original manual review evidence is missing".into()))?;
    let now = infrastructure::database_now(&mut tx).await?;
    let window_end = order.expires_at + chrono::TimeDelta::seconds(SETTLEMENT_PRICE_WINDOW_SECONDS);
    let eligible_at = exception
        .detected_at
        .checked_add_signed(chrono::TimeDelta::seconds(i64::from(snapshot.wait_seconds)))
        .ok_or_else(|| AppError::Conflict("refund waiting interval is out of range".into()))?;
    if exception.failure_code != "missing_settlement_snapshot"
        || exception.window_start != order.expires_at
        || exception.window_end != window_end
        || exception.detected_at < window_end
        || now < eligible_at
    {
        return Err(AppError::Conflict(
            "refund exception evidence or waiting interval is not eligible".into(),
        ));
    }
    if infrastructure::select_settlement_price_snapshot(&mut tx, &order.symbol, order.expires_at)
        .await?
        .is_some()
    {
        return Err(AppError::Conflict(
            "authoritative settlement evidence exists; principal refund refused".into(),
        ));
    }
    let debit_ledger_id = store::original_principal_debit(&mut tx, &order).await?;
    let precision = infrastructure::load_asset_precision_scale(&mut tx, order.stake_asset).await?;
    if !crate::modules::wallet::amount_fits_asset_precision(&order.stake_amount, precision) {
        return Err(AppError::Conflict(
            "original principal does not fit current asset precision; no rounding allowed".into(),
        ));
    }
    let rejected_commissions = store::reject_unpaid_commissions(&mut tx, order_id).await?;
    let wallet = infrastructure::lock_wallet_row(&mut tx, order.user_id, order.stake_asset).await?;
    if wallet.available < 0 || wallet.frozen < 0 || wallet.locked < 0 {
        return Err(AppError::Conflict(
            "refund wallet contains invalid negative balances".into(),
        ));
    }
    let available = wallet.available + &order.stake_amount;
    let receipt = PrincipalRefundReceipt {
        order_id,
        admin_id,
        idempotency_key: request.idempotency_key,
        debit_ledger_id,
        user_id: order.user_id,
        asset_id: order.stake_asset,
        amount: order.stake_amount.clone(),
        policy_version: snapshot.policy_version,
        original_order: json!(order),
        eligibility_evidence: json!({ "exception": exception, "wait_seconds": snapshot.wait_seconds,
            "eligible_at": eligible_at.timestamp_millis(), "checked_at": now.timestamp_millis(),
            "rejected_commission_ids": rejected_commissions }),
        reason: reason.clone(),
        created_at: now,
    };
    store::insert_receipt(&mut tx, &receipt).await?;
    infrastructure::update_wallet_available(&mut tx, order.user_id, order.stake_asset, &available)
        .await?;
    infrastructure::insert_wallet_ledger(
        &mut tx,
        SecondsContractWalletLedgerWrite {
            user_id: order.user_id,
            asset_id: order.stake_asset,
            change_type: "seconds_contract_principal_refund",
            amount: order.stake_amount.clone(),
            available_after: available,
            frozen_after: wallet.frozen,
            locked_after: wallet.locked,
            ref_id: order_id.to_string(),
        },
    )
    .await?;
    crate::modules::wallet::infrastructure::insert_wallet_platform_journal_legs_in_tx(
        &mut tx,
        "seconds_contract",
        &format!("seconds_contract:{order_id}:refund"),
        order.stake_asset,
        "seconds_contract_order",
        order_id,
        &super::super::journal::settlement_legs(&order.stake_amount, &order.stake_amount),
    )
    .await?;
    store::mark_refunded(&mut tx, order_id).await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "seconds_contract_order.principal_refund",
        "seconds_contract_order",
        order_id,
        Some(json!(order)),
        Some(json!({ "status": "refunded", "receipt": receipt })),
        Some(reason),
    )
    .await?;
    let receipt = store::load_receipt(&mut tx, order_id)
        .await?
        .ok_or(AppError::NotFound)?;
    tx.commit().await?;
    Ok((receipt, true))
}

/// 沿用秒合约提交后私有事件封装；只对本次新退款发布，不对重放或失败发布。
/// 事件接收者取原扣款用户，载荷为原本金退款而非胜负结果；通道失败不撤销已提交资金。
pub(crate) async fn refund_principal_with_events(
    pool: &Pool<MySql>,
    admin_id: u64,
    order_id: u64,
    request: RefundPrincipalRequest,
    hub: Option<&crate::modules::events::EventBroadcastHub>,
) -> AppResult<PrincipalRefundReceipt> {
    let (receipt, is_new) = refund_principal(pool, admin_id, order_id, request).await?;
    super::super::service::publish_seconds_principal_refund_if_needed(hub, &receipt, is_new);
    Ok(receipt)
}
