//! 已落库未结业务指标；本金、条件赔付、冻结、抵押分别列示，禁止相加为净负债。

use super::*;

/// 单资产当前业务快照；每个查询只聚合原始资产一致的事实，不调用结算或实时估值。
/// 条件赔付逐单按资产精度截断后求和；它不是全部订单必然同时获胜的应付承诺。
pub(super) async fn read(
    tx: &mut MySqlConnection,
    asset_id: u64,
    precision: i32,
) -> AppResult<Vec<OpenObligation>> {
    let mut rows = Vec::new();
    for (kind, sql) in [
        (
            "seconds_stake",
            "SELECT COUNT(*) AS record_count, CAST(COALESCE(SUM(stake_amount), 0) AS CHAR) AS amount FROM seconds_contract_orders WHERE stake_asset = ? AND status = 'opened'",
        ),
        (
            "prediction_stake",
            "SELECT COUNT(*) AS record_count, CAST(COALESCE(SUM(stake_amount), 0) AS CHAR) AS amount FROM prediction_orders WHERE asset_id = ? AND status = 'open'",
        ),
        (
            "earn_principal",
            "SELECT COUNT(*) AS record_count, CAST(COALESCE(SUM(amount), 0) AS CHAR) AS amount FROM earn_subscriptions WHERE asset_id = ? AND status = 'subscribed'",
        ),
        (
            "loan_principal_receivable",
            "SELECT COUNT(*) AS record_count, CAST(COALESCE(SUM(amount), 0) AS CHAR) AS amount FROM loan_orders WHERE asset_id = ? AND status IN ('disbursed', 'overdue')",
        ),
        (
            "loan_collateral",
            "SELECT COUNT(*) AS record_count, CAST(COALESCE(SUM(collateral_amount), 0) AS CHAR) AS amount FROM loan_orders WHERE collateral_asset_id = ? AND status IN ('pending', 'disbursed', 'overdue') AND collateral_released_at IS NULL",
        ),
        (
            "margin_collateral",
            "SELECT COUNT(*) AS record_count, CAST(COALESCE(SUM(margin_amount), 0) AS CHAR) AS amount FROM margin_positions WHERE margin_asset = ? AND status = 'opened'",
        ),
        (
            "margin_recorded_interest",
            "SELECT COUNT(*) AS record_count, CAST(COALESCE(SUM(interest_amount), 0) AS CHAR) AS amount FROM margin_positions WHERE margin_asset = ? AND status = 'opened'",
        ),
        (
            "withdrawal_reserved",
            "SELECT COUNT(*) AS record_count, CAST(COALESCE(SUM(total_reserved), 0) AS CHAR) AS amount FROM wallet_withdrawal_requests WHERE asset_id = ? AND status IN ('pending_review', 'approved', 'broadcasting', 'broadcasted', 'unknown_broadcast', 'manual_review')",
        ),
        (
            "new_coin_frozen_quote",
            "SELECT COUNT(*) AS record_count, CAST(COALESCE(SUM(frozen_quote_amount), 0) AS CHAR) AS amount FROM new_coin_subscriptions WHERE quote_asset = ? AND settlement_mode = 'manual_distribution' AND frozen_quote_amount > 0",
        ),
        (
            "commission_pending",
            "SELECT COUNT(*) AS record_count, CAST(COALESCE(SUM(commission_amount), 0) AS CHAR) AS amount FROM agent_commission_records WHERE payout_asset_id = ? AND status = 'pending'",
        ),
        (
            "spot_unfilled_base",
            "SELECT COUNT(*) AS record_count, CAST(COALESCE(SUM(o.quantity - o.filled_quantity), 0) AS CHAR) AS amount FROM spot_orders o JOIN trading_pairs p ON p.id = o.pair_id WHERE p.base_asset = ? AND o.status IN ('pending', 'open', 'partially_filled')",
        ),
    ] {
        let (record_count, amount): (i64, String) = sqlx::query_as(sql)
            .bind(asset_id)
            .fetch_one(&mut *tx)
            .await?;
        rows.push(OpenObligation {
            kind: kind.into(),
            record_count,
            amount,
        });
    }
    for (kind, sql) in [
        (
            "seconds_conditional_payout",
            "SELECT COUNT(*), CAST(COALESCE(SUM(TRUNCATE(stake_amount * (1 + payout_rate), ?)), 0) AS CHAR) FROM seconds_contract_orders WHERE stake_asset = ? AND status = 'opened'",
        ),
        (
            "prediction_conditional_payout",
            "SELECT COUNT(*), CAST(COALESCE(SUM(TRUNCATE(CASE WHEN effective_payout_cap > 0 THEN LEAST(theoretical_payout, effective_payout_cap) ELSE theoretical_payout END, ?)), 0) AS CHAR) FROM prediction_orders WHERE asset_id = ? AND status = 'open'",
        ),
    ] {
        let (record_count, amount): (i64, String) = sqlx::query_as(sql)
            .bind(precision)
            .bind(asset_id)
            .fetch_one(&mut *tx)
            .await?;
        rows.push(OpenObligation {
            kind: kind.into(),
            record_count,
            amount,
        });
    }
    Ok(rows)
}
