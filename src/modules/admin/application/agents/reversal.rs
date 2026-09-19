//! 已支付佣金的显式全额冲正；不重算历史费率或来源基数，也不创建债务。

use super::*;
use crate::modules::{
    admin::{
        infrastructure::{
            debit_commission_payout_in_tx, insert_commission_reversal_in_tx,
            load_commission_payout_evidence_in_tx, load_commission_reversal_in_tx,
        },
        presentation::{AdminAgentCommissionReversalResponse, ReverseAgentCommissionRequest},
        service::validate_commission_reversal_key,
    },
    wallet::{
        infrastructure::insert_wallet_platform_journal_legs_in_tx,
        platform_journal::WalletPlatformJournalLeg,
    },
};

/// 已鉴权管理员显式冲回已付佣金，秒合约先锁来源，随后锁佣金、钱包，原付款依据为只追加账本。
/// 原 actor/key/reason 重放返回不可变凭证；其他重放冲突，余额不足拒绝而不动冻结或锁仓资金。
/// 原佣金来源、费率和金额保持不变；钱包、流水、冲正凭证、平台账和管理员审计同事务提交。
pub(crate) async fn reverse_admin_agent_commission(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    commission_id: u64,
    request: ReverseAgentCommissionRequest,
) -> AppResult<AdminAgentCommissionReversalResponse> {
    validate_commission_reversal_key(&request.idempotency_key)?;
    let reason = required_admin_audit_reason(request.reason)?;
    let pool = admin_mysql_pool(pool)?;
    let hint = commission_source_hint(&pool, commission_id).await?;
    let mut tx = pool.begin().await?;
    let before = lock_agent_commission_with_source_in_tx(&mut tx, commission_id, hint).await?;
    if let Some(receipt) = load_commission_reversal_in_tx(&mut tx, commission_id).await? {
        if receipt.admin_id != admin_id
            || receipt.idempotency_key != request.idempotency_key
            || receipt.reason != reason
        {
            return Err(AppError::Conflict(
                "agent commission reversal request conflicts with existing receipt".to_owned(),
            ));
        }
        tx.commit().await?;
        return Ok(receipt);
    }
    if before.status != "settled" {
        return Err(AppError::Conflict(
            "only settled agent commissions can be reversed".to_owned(),
        ));
    }
    let payout = load_commission_payout_evidence_in_tx(&mut tx, &before).await?;
    insert_commission_reversal_in_tx(
        &mut tx,
        &before,
        &payout,
        admin_id,
        &request.idempotency_key,
        &reason,
    )
    .await?;
    debit_commission_payout_in_tx(&mut tx, commission_id, &payout).await?;
    write_commission_journal_in_tx(
        &mut tx,
        commission_id,
        payout.asset_id,
        &payout.amount,
        true,
    )
    .await?;
    update_agent_commission_status_in_tx(&mut tx, commission_id, "reversed").await?;
    let receipt = load_commission_reversal_in_tx(&mut tx, commission_id)
        .await?
        .ok_or(AppError::NotFound)?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "agent_commission.reverse",
            target_type: "agent_commission",
            target_id: commission_id,
            before_json: Some(agent_commission_audit_json(&before)),
            after_json: Some(json!({ "status": "reversed", "reversal": receipt })),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(receipt)
}

/// 佣金支付确认费用和用户负债，冲正使用相同科目严格取反，金额来自原支付证据。
/// 每个资产独立零和，支付与冲正采用不同事务键；共享写入器遇重复腿会让整个资金事务失败。
pub(super) async fn write_commission_journal_in_tx(
    tx: &mut sqlx::Transaction<'_, MySql>,
    commission_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    reversal: bool,
) -> AppResult<()> {
    let signed = if reversal {
        -amount.clone()
    } else {
        amount.clone()
    };
    let legs = [
        WalletPlatformJournalLeg {
            account_code: "platform_commission_expense",
            amount: signed.clone(),
        },
        WalletPlatformJournalLeg {
            account_code: "user_commission_wallet_liability",
            amount: -signed,
        },
    ];
    let action = if reversal { "reverse" } else { "payout" };
    insert_wallet_platform_journal_legs_in_tx(
        tx,
        "agent_commission",
        &format!("agent_commission:{commission_id}:{action}"),
        asset_id,
        "agent_commission",
        commission_id,
        &legs,
    )
    .await
}
