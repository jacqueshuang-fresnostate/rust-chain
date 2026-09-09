//! 后台资金指令收据持久化与全局幂等/结算审计查询。
//!
//! 人工充值在动账前先以 `(admin_id, idempotency_key)` 占用唯一收据，
//! 再在同一事务中写钱包、流水、审计和首次响应快照。治理审计只做跨业务表的只读聚合，
//! 用于发现重复键、缺失键、孤儿流水和超时未结算队列，不替代各业务自身的唯一约束。

use crate::{
    error::{AppError, AppResult},
    modules::admin::presentation::AdminUserRechargeResponse,
};
use serde_json::{Value, json};
use sqlx::{Executor, MySql, Transaction, types::Json as SqlxJson};

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct AdminWalletRechargeReceipt {
    pub(crate) request_fingerprint: String,
    pub(crate) response_snapshot_json: SqlxJson<Value>,
}

/// 按管理员作用域读取首次充值收据；兼容连接池快路径与已锁用户事务内的重放复查。
/// 查询不追加收据锁，事务内调用须在取得用户锁后且建立快照前执行，未命中由唯一键裁决首单。
pub(crate) async fn load_admin_wallet_recharge_receipt<'e, E>(
    executor: E,
    admin_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<AdminWalletRechargeReceipt>>
where
    E: Executor<'e, Database = MySql>,
{
    sqlx::query_as::<_, AdminWalletRechargeReceipt>(
        r#"SELECT request_fingerprint, response_snapshot_json
           FROM admin_wallet_recharges
           WHERE admin_id = ? AND idempotency_key = ?
           LIMIT 1"#,
    )
    .bind(admin_id)
    .bind(idempotency_key)
    .fetch_optional(executor)
    .await
    .map_err(AppError::from)
}

/// 在动账前占用管理员级幂等键；原始 SQL 错误保留给应用层识别并发唯一键竞争。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn insert_admin_wallet_recharge_receipt_in_tx(
    tx: &mut Transaction<'_, MySql>,
    recharge_id: &str,
    admin_id: u64,
    user_id: u64,
    asset_id: u64,
    amount: &bigdecimal::BigDecimal,
    reason: &str,
    idempotency_key: &str,
    request_fingerprint: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO admin_wallet_recharges
           (recharge_id, admin_id, user_id, asset_id, amount, reason, idempotency_key,
            request_fingerprint, response_snapshot_json)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(recharge_id)
    .bind(admin_id)
    .bind(user_id)
    .bind(asset_id)
    .bind(amount)
    .bind(reason)
    .bind(idempotency_key)
    .bind(request_fingerprint)
    .bind(SqlxJson(json!({})))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在充值事务提交前固化首次响应，重放不读取之后变化过的钱包余额。
pub(crate) async fn store_admin_wallet_recharge_response_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    idempotency_key: &str,
    response: &AdminUserRechargeResponse,
) -> AppResult<()> {
    let snapshot = serde_json::to_value(response).map_err(|error| {
        AppError::Internal(format!("serialize admin recharge receipt: {error}"))
    })?;
    let result = sqlx::query(
        r#"UPDATE admin_wallet_recharges
           SET response_snapshot_json = ?
           WHERE admin_id = ? AND idempotency_key = ?"#,
    )
    .bind(SqlxJson(snapshot))
    .bind(admin_id)
    .bind(idempotency_key)
    .execute(&mut **tx)
    .await?;
    if result.rows_affected() != 1 {
        return Err(AppError::Internal(
            "admin recharge receipt snapshot update affected an unexpected row count".to_owned(),
        ));
    }
    Ok(())
}

/// 仅把 MySQL 唯一键错误视为可重放竞争，连接或约束故障继续失败关闭。
pub(crate) fn is_admin_wallet_recharge_duplicate_key(error: &sqlx::Error) -> bool {
    error.as_database_error().is_some_and(|database_error| {
        database_error.code().as_deref() == Some("1062")
            || database_error.code().as_deref() == Some("23000")
    })
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct AdminFinancialIdempotencyAuditRecord {
    pub(crate) duplicate_idempotency_groups: i64,
    pub(crate) missing_idempotency_keys: i64,
    pub(crate) orphan_ledger_entries: i64,
    pub(crate) expired_seconds_orders: i64,
    pub(crate) expired_prediction_orders: i64,
    pub(crate) pending_loan_orders: i64,
    pub(crate) pending_new_coin_subscriptions: i64,
}

/// 读取资金与结算幂等审计快照。
///
/// 该查询只读且不持有业务锁：它同时检查各订单表的重复/空幂等键、钱包流水对业务对象的
/// 反向引用、已过期仍未结算的订单以及待人工处理队列。重复请求不会因为打开审计页而触发
/// 任何结算或补偿动作；所有计数来自同一条 MySQL 语句，避免不同查询之间出现明显时间漂移。
pub(crate) async fn load_admin_financial_idempotency_audit(
    pool: &sqlx::Pool<MySql>,
) -> AppResult<AdminFinancialIdempotencyAuditRecord> {
    sqlx::query_as::<_, AdminFinancialIdempotencyAuditRecord>(
        r#"SELECT
             (SELECT COUNT(*) FROM (
                SELECT user_id, idempotency_key FROM seconds_contract_orders
                WHERE TRIM(idempotency_key) <> ''
                GROUP BY user_id, idempotency_key HAVING COUNT(*) > 1
              ) AS duplicate_seconds)
              + (SELECT COUNT(*) FROM (
                SELECT user_id, idempotency_key FROM prediction_orders
                WHERE TRIM(idempotency_key) <> ''
                GROUP BY user_id, idempotency_key HAVING COUNT(*) > 1
              ) AS duplicate_prediction)
              + (SELECT COUNT(*) FROM (
                SELECT user_id, idempotency_key FROM earn_subscriptions
                WHERE TRIM(idempotency_key) <> ''
                GROUP BY user_id, idempotency_key HAVING COUNT(*) > 1
              ) AS duplicate_earn)
              + (SELECT COUNT(*) FROM (
                SELECT user_id, idempotency_key FROM loan_orders
                WHERE TRIM(idempotency_key) <> ''
                GROUP BY user_id, idempotency_key HAVING COUNT(*) > 1
              ) AS duplicate_loan)
              + (SELECT COUNT(*) FROM (
                SELECT user_id, idempotency_key FROM margin_positions
                WHERE TRIM(idempotency_key) <> ''
                GROUP BY user_id, idempotency_key HAVING COUNT(*) > 1
              ) AS duplicate_margin)
              + (SELECT COUNT(*) FROM (
                SELECT user_id, idempotency_key FROM margin_transfers
                WHERE TRIM(idempotency_key) <> ''
                GROUP BY user_id, idempotency_key HAVING COUNT(*) > 1
              ) AS duplicate_margin_transfers)
              + (SELECT COUNT(*) FROM (
                SELECT user_id, idempotency_key FROM margin_position_close_executions
                WHERE TRIM(idempotency_key) <> ''
                GROUP BY user_id, idempotency_key HAVING COUNT(*) > 1
              ) AS duplicate_margin_closes)
              + (SELECT COUNT(*) FROM (
                SELECT user_id, idempotency_key FROM spot_orders
                WHERE idempotency_key IS NOT NULL AND TRIM(idempotency_key) <> ''
                GROUP BY user_id, idempotency_key HAVING COUNT(*) > 1
              ) AS duplicate_spot)
              + (SELECT COUNT(*) FROM (
                SELECT idempotency_key FROM spot_trades
                WHERE idempotency_key IS NOT NULL AND TRIM(idempotency_key) <> ''
                GROUP BY idempotency_key HAVING COUNT(*) > 1
              ) AS duplicate_spot_trades)
              + (SELECT COUNT(*) FROM (
                SELECT idempotency_key FROM new_coin_subscriptions
                WHERE TRIM(idempotency_key) <> ''
                GROUP BY idempotency_key HAVING COUNT(*) > 1
              ) AS duplicate_new_coin_subscriptions)
              + (SELECT COUNT(*) FROM (
                SELECT idempotency_key FROM new_coin_purchase_orders
                WHERE TRIM(idempotency_key) <> ''
                GROUP BY idempotency_key HAVING COUNT(*) > 1
              ) AS duplicate_new_coin_purchases)
              + (SELECT COUNT(*) FROM (
                SELECT idempotency_key FROM new_coin_distributions
                WHERE TRIM(idempotency_key) <> ''
                GROUP BY idempotency_key HAVING COUNT(*) > 1
              ) AS duplicate_new_coin_distributions)
              + (SELECT COUNT(*) FROM (
                SELECT idempotency_key FROM asset_unlock_records
                WHERE TRIM(idempotency_key) <> ''
                GROUP BY idempotency_key HAVING COUNT(*) > 1
              ) AS duplicate_asset_unlocks)
              + (SELECT COUNT(*) FROM (
                SELECT user_id, idempotency_key FROM wallet_withdrawal_requests
                WHERE TRIM(idempotency_key) <> ''
                GROUP BY user_id, idempotency_key HAVING COUNT(*) > 1
              ) AS duplicate_withdrawals)
              + (SELECT COUNT(*) FROM (
                SELECT admin_id, idempotency_key FROM admin_wallet_recharges
                WHERE TRIM(idempotency_key) <> ''
                GROUP BY admin_id, idempotency_key HAVING COUNT(*) > 1
              ) AS duplicate_admin_recharges) AS duplicate_idempotency_groups,
             (SELECT COUNT(*) FROM seconds_contract_orders WHERE idempotency_key IS NULL OR TRIM(idempotency_key) = '')
              + (SELECT COUNT(*) FROM prediction_orders WHERE idempotency_key IS NULL OR TRIM(idempotency_key) = '')
              + (SELECT COUNT(*) FROM earn_subscriptions WHERE idempotency_key IS NULL OR TRIM(idempotency_key) = '')
              + (SELECT COUNT(*) FROM loan_orders WHERE idempotency_key IS NULL OR TRIM(idempotency_key) = '')
              + (SELECT COUNT(*) FROM margin_positions WHERE idempotency_key IS NULL OR TRIM(idempotency_key) = '')
              + (SELECT COUNT(*) FROM margin_transfers WHERE idempotency_key IS NULL OR TRIM(idempotency_key) = '')
              + (SELECT COUNT(*) FROM margin_position_close_executions WHERE idempotency_key IS NULL OR TRIM(idempotency_key) = '')
              + (SELECT COUNT(*) FROM spot_orders WHERE idempotency_key IS NULL OR TRIM(idempotency_key) = '')
              + (SELECT COUNT(*) FROM spot_trades WHERE idempotency_key IS NULL OR TRIM(idempotency_key) = '')
              + (SELECT COUNT(*) FROM new_coin_subscriptions WHERE idempotency_key IS NULL OR TRIM(idempotency_key) = '')
              + (SELECT COUNT(*) FROM new_coin_purchase_orders WHERE idempotency_key IS NULL OR TRIM(idempotency_key) = '')
              + (SELECT COUNT(*) FROM new_coin_distributions WHERE idempotency_key IS NULL OR TRIM(idempotency_key) = '')
              + (SELECT COUNT(*) FROM asset_unlock_records WHERE idempotency_key IS NULL OR TRIM(idempotency_key) = '')
              + (SELECT COUNT(*) FROM wallet_withdrawal_requests WHERE idempotency_key IS NULL OR TRIM(idempotency_key) = '')
              + (SELECT COUNT(*) FROM admin_wallet_recharges WHERE idempotency_key IS NULL OR TRIM(idempotency_key) = '') AS missing_idempotency_keys,
             (SELECT COUNT(*) FROM wallet_ledger ledger
              WHERE (ledger.ref_type = 'seconds_contract_order' AND NOT EXISTS
                       (SELECT 1 FROM seconds_contract_orders orders WHERE orders.id = CAST(ledger.ref_id AS UNSIGNED)))
                 OR (ledger.ref_type = 'prediction_order' AND NOT EXISTS
                       (SELECT 1 FROM prediction_orders orders WHERE orders.id = CAST(ledger.ref_id AS UNSIGNED)))
                 OR (ledger.ref_type = 'earn_subscription' AND NOT EXISTS
                       (SELECT 1 FROM earn_subscriptions subscriptions WHERE subscriptions.id = CAST(ledger.ref_id AS UNSIGNED)))
                 OR (ledger.ref_type = 'loan_order' AND NOT EXISTS
                       (SELECT 1 FROM loan_orders orders WHERE orders.id = CAST(ledger.ref_id AS UNSIGNED)))
                 OR (ledger.ref_type = 'margin_position' AND NOT EXISTS
                       (SELECT 1 FROM margin_positions positions WHERE positions.id = CAST(ledger.ref_id AS UNSIGNED)))
                 OR (ledger.ref_type = 'spot_order' AND NOT EXISTS
                       (SELECT 1 FROM spot_orders orders WHERE orders.id = CAST(ledger.ref_id AS UNSIGNED)))
                 OR (ledger.ref_type = 'spot_trade' AND (
                       ledger.ref_id NOT REGEXP '^[0-9]+:[0-9]+$'
                       OR NOT EXISTS (
                           SELECT 1
                           FROM spot_trades trades
                           WHERE trades.buy_order_id = CAST(SUBSTRING_INDEX(ledger.ref_id, ':', 1) AS UNSIGNED)
                             AND trades.sell_order_id = CAST(SUBSTRING_INDEX(ledger.ref_id, ':', -1) AS UNSIGNED)
                       )
                    ))
                 OR (ledger.ref_type = 'margin_transfer' AND NOT EXISTS
                       (SELECT 1 FROM margin_transfers transfers WHERE transfers.transfer_id = ledger.ref_id))
                 OR (ledger.ref_type = 'new_coin_subscription' AND NOT EXISTS
                       (SELECT 1 FROM new_coin_subscriptions subscriptions WHERE subscriptions.idempotency_key = ledger.ref_id))
                 OR (ledger.ref_type = 'new_coin_purchase' AND NOT EXISTS
                       (SELECT 1 FROM new_coin_purchase_orders orders WHERE orders.idempotency_key = ledger.ref_id))
                 OR (ledger.ref_type = 'new_coin_distribution' AND NOT EXISTS
                       (SELECT 1 FROM new_coin_distributions distributions WHERE distributions.idempotency_key = ledger.ref_id))
                 OR (ledger.ref_type = 'new_coin_unlock' AND NOT EXISTS
                       (SELECT 1 FROM asset_unlock_records unlocks WHERE unlocks.idempotency_key = ledger.ref_id))
                 OR (ledger.ref_type = 'wallet_withdrawal_request' AND NOT EXISTS
                       (SELECT 1 FROM wallet_withdrawal_requests requests
                        WHERE ledger.ref_id REGEXP '^[0-9]+$'
                          AND requests.id = CAST(ledger.ref_id AS UNSIGNED)))
                 OR (ledger.ref_type = 'admin_recharge' AND NOT EXISTS
                       (SELECT 1 FROM admin_wallet_recharges recharges WHERE recharges.recharge_id = ledger.ref_id))
                 OR (ledger.ref_type = 'convert_order' AND NOT EXISTS
                       (SELECT 1 FROM convert_orders orders WHERE orders.quote_id = ledger.ref_id))
                 OR (ledger.ref_type = 'loan_liquidation' AND NOT EXISTS
                       (SELECT 1 FROM loan_liquidations liquidations
                        WHERE ledger.ref_id REGEXP '^[0-9]+$'
                          AND liquidations.order_id = CAST(ledger.ref_id AS UNSIGNED)))
                 OR (ledger.ref_type = 'wallet_deposit_event' AND NOT EXISTS
                       (SELECT 1 FROM wallet_deposit_events events
                        WHERE ledger.ref_id REGEXP '^[0-9]+$'
                          AND events.id = CAST(ledger.ref_id AS UNSIGNED)))
                 OR (ledger.ref_type = 'quick_recharge' AND NOT EXISTS
                       (SELECT 1 FROM quick_recharge_orders orders
                        WHERE orders.order_id = ledger.ref_id))
                 OR (ledger.ref_type = 'deposit_record' AND NOT EXISTS
                       (SELECT 1 FROM deposit_records records
                        WHERE ledger.ref_id REGEXP '^[0-9]+$'
                          AND records.id = CAST(ledger.ref_id AS UNSIGNED)))
                 OR (ledger.ref_type IN ('withdraw_record', 'withdrawal_record', 'withdraw') AND NOT EXISTS
                       (SELECT 1 FROM withdraw_records records
                        WHERE ledger.ref_id REGEXP '^[0-9]+$'
                          AND records.id = CAST(ledger.ref_id AS UNSIGNED))))
              + (SELECT COUNT(*) FROM margin_wallet_ledger ledger
                 WHERE (ledger.ref_type = 'margin_position' AND NOT EXISTS
                          (SELECT 1 FROM margin_positions positions
                           WHERE ledger.ref_id REGEXP '^[0-9]+$'
                             AND positions.id = CAST(ledger.ref_id AS UNSIGNED)))
                    OR (ledger.ref_type = 'margin_transfer' AND NOT EXISTS
                          (SELECT 1 FROM margin_transfers transfers WHERE transfers.transfer_id = ledger.ref_id))) AS orphan_ledger_entries,
             (SELECT COUNT(*) FROM seconds_contract_orders
              WHERE status = 'opened' AND expires_at < UTC_TIMESTAMP(6)) AS expired_seconds_orders,
             (SELECT COUNT(*) FROM prediction_orders orders
              INNER JOIN prediction_markets markets ON markets.id = orders.market_id
              WHERE orders.status = 'open' AND markets.end_at IS NOT NULL
                AND markets.end_at <= UTC_TIMESTAMP(6)) AS expired_prediction_orders,
             (SELECT COUNT(*) FROM loan_orders WHERE status = 'pending') AS pending_loan_orders,
             (SELECT COUNT(*) FROM new_coin_subscriptions
              WHERE settlement_mode = 'manual_distribution'
                AND (status = 'pending' OR frozen_quote_amount > 0)) AS pending_new_coin_subscriptions"#,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

#[cfg(test)]
#[path = "../../../../tests/unit_src/src_modules_admin_infrastructure_financial_idempotency_tests.rs"]
mod tests;
