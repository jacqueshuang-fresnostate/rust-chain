//! 佣金冲正的原支付证据、只追加凭证和钱包扣减。

use super::*;
use crate::modules::admin::{
    presentation::AdminAgentCommissionReversalResponse, service::agent_commission_audit_json,
};

/// 原钱包支付流水是冲正的收款用户、资产和实际支付金额依据，不重新解析当前代理绑定。
#[derive(sqlx::FromRow)]
pub(crate) struct CommissionPayoutEvidence {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    pub(crate) asset_id: u64,
    pub(crate) amount: BigDecimal,
}

/// 读取唯一不可变原支付流水；缺失、多笔、金额或资产与原佣金快照不符一律拒绝。
/// 调用方先锁佣金以串行化付款/冲正；账本仅追加，不对引用范围加间隙锁，避免与同钱包流水插入互锁。
/// 不按当前资产精度截断历史金额，原账本外键在凭证插入时保留支付证据。
pub(crate) async fn load_commission_payout_evidence_in_tx(
    tx: &mut Transaction<'_, MySql>,
    commission: &AdminAgentCommissionResponse,
) -> AppResult<CommissionPayoutEvidence> {
    let mut rows = sqlx::query_as::<_, CommissionPayoutEvidence>(
        r#"SELECT id, user_id, asset_id, amount FROM wallet_ledger
           WHERE ref_type = 'agent_commission' AND ref_id = ?
             AND change_type = 'agent_commission_payout' AND balance_type = 'available'
           ORDER BY id LIMIT 2"#,
    )
    .bind(commission.id.to_string())
    .fetch_all(&mut **tx)
    .await?;
    if rows.len() != 1
        || rows[0].amount <= 0
        || rows[0].amount != commission.commission_amount
        || Some(rows[0].asset_id) != commission.payout_asset_id
    {
        return Err(AppError::Conflict(
            "agent commission reversal requires matching unique payout evidence".to_owned(),
        ));
    }
    Ok(rows.remove(0))
}

/// 在佣金行锁保护下读取已完成冲正凭证；返回原始操作者、原因、金额与时间，不覆盖历史。
pub(crate) async fn load_commission_reversal_in_tx(
    tx: &mut Transaction<'_, MySql>,
    commission_id: u64,
) -> AppResult<Option<AdminAgentCommissionReversalResponse>> {
    Ok(sqlx::query_as::<_, AdminAgentCommissionReversalResponse>(
        r#"SELECT commission_id, admin_id, idempotency_key, payout_ledger_id,
                  agent_user_id, asset_id, amount, original_commission, reason, created_at
           FROM agent_commission_reversals WHERE commission_id = ?"#,
    )
    .bind(commission_id)
    .fetch_optional(&mut **tx)
    .await?)
}

/// 在资金事务中只追加一张全额冲正凭证；同管理员跨佣金复用键及重复原支付引用均报冲突。
/// 来源和计佣口径以 JSON 原样固化，不以当前规则重算；调用方必须连同扣款和审计一起提交。
pub(crate) async fn insert_commission_reversal_in_tx(
    tx: &mut Transaction<'_, MySql>,
    commission: &AdminAgentCommissionResponse,
    payout: &CommissionPayoutEvidence,
    admin_id: u64,
    key: &str,
    reason: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO agent_commission_reversals
           (commission_id, admin_id, idempotency_key, payout_ledger_id, agent_user_id,
            asset_id, amount, original_commission, reason)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(commission.id)
    .bind(admin_id)
    .bind(key)
    .bind(payout.id)
    .bind(payout.user_id)
    .bind(payout.asset_id)
    .bind(&payout.amount)
    .bind(sqlx::types::Json(agent_commission_audit_json(commission)))
    .bind(reason)
    .execute(&mut **tx)
    .await
    .map_err(|error| {
        if is_mysql_duplicate_key(&error) {
            AppError::Conflict("agent commission reversal idempotency conflict".to_owned())
        } else {
            AppError::Database(error)
        }
    })?;
    Ok(())
}

/// 锁定原收款钱包后仅扣可用余额，冻结和锁仓不动；余额不足直接拒绝，不建负余额或债务。
/// 钱包和同额负流水均由调用方事务提交，任何后续凭证/审计/平台账失败都会回滚。
pub(crate) async fn debit_commission_payout_in_tx(
    tx: &mut Transaction<'_, MySql>,
    commission_id: u64,
    payout: &CommissionPayoutEvidence,
) -> AppResult<()> {
    let wallet = sqlx::query_as::<_, (BigDecimal, BigDecimal, BigDecimal)>(
        "SELECT available, frozen, locked FROM wallet_accounts WHERE user_id = ? AND asset_id = ? FOR UPDATE",
    )
    .bind(payout.user_id)
    .bind(payout.asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Conflict("commission payout wallet is missing".to_owned()))?;
    if wallet.0 < payout.amount {
        return Err(AppError::Conflict(
            "insufficient available balance for commission reversal".to_owned(),
        ));
    }
    let available = wallet.0 - &payout.amount;
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available)
        .bind(payout.user_id)
        .bind(payout.asset_id)
        .execute(&mut **tx)
        .await?;
    insert_admin_wallet_ledger_in_tx(
        tx,
        payout.user_id,
        payout.asset_id,
        -payout.amount.clone(),
        "available",
        &available,
        &available,
        &wallet.1,
        &wallet.2,
        "agent_commission_reversal",
        "agent_commission",
        &commission_id.to_string(),
    )
    .await
}
