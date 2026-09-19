//! 退款策略、仅新单快照和原扣款取证。调用方持有产品或来源订单锁后才写入。

use super::*;
use crate::modules::seconds_contract::presentation::refund::{
    PrincipalRefundReceipt, RefundPolicy,
};
use serde::Serialize;

/// 用产品主行串行化策略变更和开仓快照；不存在的产品不能凭空配置策略。
pub(crate) async fn lock_policy_product(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<()> {
    sqlx::query_scalar::<_, u64>(
        "SELECT id FROM seconds_contract_products WHERE id = ? FOR UPDATE",
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(())
}

/// 已配置退款策略的产品保留不可变修订外键，不能物理删除；运营可停用而不擦除审计依据。
pub(crate) async fn ensure_policy_history_preserved(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<()> {
    let exists: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM seconds_refund_policy_revisions WHERE product_id = ?",
    )
    .bind(product_id)
    .fetch_one(&mut **tx)
    .await?;
    if exists != 0 {
        return Err(AppError::Conflict(
            "product has immutable refund policy history; disable instead of deleting".into(),
        ));
    }
    Ok(())
}

/// 锁定当前策略读，避免开仓事务较早的一致性快照读到过时启用状态。
/// 产品锁先于策略锁，缺失策略返回明确关闭的版本零，不写默认记录。
pub(crate) async fn load_policy(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    locking: bool,
) -> AppResult<RefundPolicy> {
    let query = if locking {
        "SELECT product_id, version, enabled, wait_seconds FROM seconds_refund_policies WHERE product_id = ? FOR UPDATE"
    } else {
        "SELECT product_id, version, enabled, wait_seconds FROM seconds_refund_policies WHERE product_id = ?"
    };
    Ok(sqlx::query_as::<_, RefundPolicy>(query)
        .bind(product_id)
        .fetch_optional(&mut **tx)
        .await?
        .unwrap_or(RefundPolicy {
            product_id,
            version: 0,
            enabled: false,
            wait_seconds: None,
        }))
}

/// 配置头与不可变版本一起写；版本审计由应用层同事务追加，不回写任何订单快照。
pub(crate) async fn write_policy(
    tx: &mut Transaction<'_, MySql>,
    policy: &RefundPolicy,
    admin_id: u64,
    reason: &str,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO seconds_refund_policy_revisions (product_id, version, enabled, wait_seconds, admin_id, reason) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(policy.product_id).bind(policy.version).bind(policy.enabled)
    .bind(policy.wait_seconds).bind(admin_id).bind(reason)
    .execute(&mut **tx).await?;
    sqlx::query(
        "INSERT INTO seconds_refund_policies (product_id, version, enabled, wait_seconds) VALUES (?, ?, ?, ?) ON DUPLICATE KEY UPDATE version = VALUES(version), enabled = VALUES(enabled), wait_seconds = VALUES(wait_seconds)",
    )
    .bind(policy.product_id).bind(policy.version).bind(policy.enabled).bind(policy.wait_seconds)
    .execute(&mut **tx).await?;
    Ok(())
}

/// 仅由新订单插入分支调用，且调用方已持有产品主行锁；重放不得调用此函数。
/// 启用策略固化版本与等待间隔，关闭时不留快照，旧订单永远不通过后台补录。
pub(crate) async fn snapshot_refund_policy_in_tx(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    order_id: u64,
) -> AppResult<()> {
    let policy = load_policy(tx, product_id, true).await?;
    if policy.enabled {
        let wait = policy
            .wait_seconds
            .ok_or_else(|| AppError::Conflict("invalid seconds refund policy".into()))?;
        sqlx::query("INSERT INTO seconds_order_refund_snapshots (order_id, product_id, policy_version, wait_seconds) VALUES (?, ?, ?, ?)")
            .bind(order_id).bind(product_id).bind(policy.version).bind(wait)
            .execute(&mut **tx).await?;
    }
    Ok(())
}

/// 退款资格只读取订单已有快照及对应启用版本，不接受当前产品策略补足历史权利。
#[derive(sqlx::FromRow)]
pub(crate) struct RefundSnapshot {
    pub product_id: u64,
    pub policy_version: u64,
    pub wait_seconds: u32,
}

/// 校验版本和快照完全一致；损坏或已关闭的历史版本不能被当成启用资格。
pub(crate) async fn load_snapshot(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
) -> AppResult<Option<RefundSnapshot>> {
    Ok(sqlx::query_as::<_, RefundSnapshot>(
        "SELECT s.product_id, s.policy_version, s.wait_seconds FROM seconds_order_refund_snapshots s JOIN seconds_refund_policy_revisions r ON r.product_id = s.product_id AND r.version = s.policy_version WHERE s.order_id = ? AND r.enabled = TRUE AND r.wait_seconds = s.wait_seconds",
    ).bind(order_id).fetch_optional(&mut **tx).await?)
}

/// 首次转人工审核的追加式证据是计时依据；保留原窗口用于防篡改核对。
#[derive(Serialize, sqlx::FromRow)]
pub(crate) struct RefundException {
    pub failure_code: String,
    pub detected_at: DateTime<Utc>,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
}

/// 只读原异常记录，不把查询错误或缺失字段降级成可退款状态。
pub(crate) async fn load_exception(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
) -> AppResult<Option<RefundException>> {
    Ok(sqlx::query_as::<_, RefundException>(
        "SELECT failure_code, detected_at, window_start, window_end FROM seconds_contract_settlement_exceptions WHERE order_id = ?",
    ).bind(order_id).fetch_optional(&mut **tx).await?)
}

/// 获得专用连接并显式设置下一事务为 RR，使空历史窗口的锁定读阻挡迟到插入至提交。
pub(crate) async fn refund_connection(
    pool: &Pool<MySql>,
) -> AppResult<sqlx::pool::PoolConnection<MySql>> {
    let mut connection = pool.acquire().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *connection)
        .await?;
    Ok(connection)
}

/// 只锁来源订单主行，不扩散到用户、交易对和资产联表；所有秒合约打款先取得此锁。
pub(crate) async fn lock_source_order(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
) -> AppResult<()> {
    sqlx::query_scalar::<_, u64>("SELECT id FROM seconds_contract_orders WHERE id = ? FOR UPDATE")
        .bind(order_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(())
}

/// 原扣款流水固定退款收款人、资产和金额；只追加证据不加引用范围锁，避免钱包插入死锁。
pub(crate) async fn original_principal_debit(
    tx: &mut Transaction<'_, MySql>,
    order: &SecondsContractOrderResponse,
) -> AppResult<u64> {
    let rows = sqlx::query_as::<_, (u64, u64, u64, BigDecimal, String, String)>(
        "SELECT id, user_id, asset_id, amount, balance_type, change_type FROM wallet_ledger WHERE ref_type = 'seconds_contract_order' AND ref_id = ? ORDER BY id",
    ).bind(order.id.to_string()).fetch_all(&mut **tx).await?;
    if rows.len() != 1
        || rows[0].1 != order.user_id
        || rows[0].2 != order.stake_asset
        || rows[0].3 != -order.stake_amount.clone()
        || rows[0].4 != "available"
        || rows[0].5 != "seconds_contract_open"
        || order.stake_amount <= 0
    {
        return Err(AppError::Conflict(
            "seconds refund requires a unique matching principal debit and no payout".into(),
        ));
    }
    Ok(rows[0].0)
}

/// 先按主键顺序锁定来源佣金，再核查原支付流水；已付/冲正/未知状态都拒绝退款。
/// 未支付佣金在同一来源锁下转 rejected，金额与计佣依据保持原样；不自动回收代理资金。
pub(crate) async fn reject_unpaid_commissions(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
) -> AppResult<Vec<u64>> {
    let commissions = sqlx::query_as::<_, (u64, String)>(
        "SELECT id, status FROM agent_commission_records WHERE source_type = 'seconds_contract_order' AND source_id = ? ORDER BY id FOR UPDATE",
    ).bind(order_id.to_string()).fetch_all(&mut **tx).await?;
    for (id, status) in &commissions {
        let paid: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'agent_commission' AND ref_id = ? AND change_type = 'agent_commission_payout'",
        ).bind(id.to_string()).fetch_one(&mut **tx).await?;
        if !matches!(status.as_str(), "pending" | "rejected") || paid != 0 {
            return Err(AppError::Conflict(
                "seconds refund refuses paid commission or payout evidence".into(),
            ));
        }
    }
    let mut rejected = Vec::new();
    for (id, status) in commissions {
        if status == "pending" {
            sqlx::query("UPDATE agent_commission_records SET status = 'rejected' WHERE id = ? AND status = 'pending'")
                .bind(id).execute(&mut **tx).await?;
            rejected.push(id);
        }
    }
    Ok(rejected)
}

/// 同订单的成功凭证是唯一重放依据，查询不修改原请求或原资金证据。
pub(crate) async fn load_receipt(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
) -> AppResult<Option<PrincipalRefundReceipt>> {
    Ok(sqlx::query_as::<_, PrincipalRefundReceipt>(
        "SELECT order_id, admin_id, idempotency_key, debit_ledger_id, user_id, asset_id, amount, policy_version, original_order, eligibility_evidence, reason, created_at FROM seconds_principal_refunds WHERE order_id = ?",
    ).bind(order_id).fetch_optional(&mut **tx).await?)
}

/// 退款凭证只插入一次；digest 冲突一律返回冲突，不把唯一键异常误当成成功重放。
pub(crate) async fn insert_receipt(
    tx: &mut Transaction<'_, MySql>,
    receipt: &PrincipalRefundReceipt,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO seconds_principal_refunds (order_id, admin_id, idempotency_key, debit_ledger_id, user_id, asset_id, amount, policy_version, original_order, eligibility_evidence, reason) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    ).bind(receipt.order_id).bind(receipt.admin_id).bind(&receipt.idempotency_key)
    .bind(receipt.debit_ledger_id).bind(receipt.user_id).bind(receipt.asset_id).bind(&receipt.amount)
    .bind(receipt.policy_version).bind(sqlx::types::Json(&receipt.original_order))
    .bind(sqlx::types::Json(&receipt.eligibility_evidence)).bind(&receipt.reason)
    .execute(&mut **tx).await.map_err(|error| {
        if error.as_database_error().is_some_and(|e| e.is_unique_violation()) {
            AppError::Conflict("seconds principal refund idempotency conflict".into())
        } else { AppError::Database(error) }
    })?;
    Ok(())
}

/// 只允许人工审核进入本金退还终态，原胜负和价格为空，异常与历史订单字段不删不改。
pub(crate) async fn mark_refunded(tx: &mut Transaction<'_, MySql>, order_id: u64) -> AppResult<()> {
    let result = sqlx::query("UPDATE seconds_contract_orders SET status = 'refunded', next_settlement_attempt_at = NULL WHERE id = ? AND status = 'manual_review' AND result IS NULL AND settled_at IS NULL AND settlement_price IS NULL AND settlement_price_tick_id IS NULL")
        .bind(order_id).execute(&mut **tx).await?;
    if result.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "seconds order changed before principal refund".into(),
        ));
    }
    Ok(())
}
