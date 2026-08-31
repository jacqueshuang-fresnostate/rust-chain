//! 后台资金指令收据持久化。
//!
//! 人工充值在动账前先以 `(admin_id, idempotency_key)` 占用唯一收据，
//! 再在同一事务中写钱包、流水、审计和首次响应快照。

use crate::{
    error::{AppError, AppResult},
    modules::admin::presentation::AdminUserRechargeResponse,
};
use serde_json::{Value, json};
use sqlx::{MySql, Pool, Transaction, types::Json as SqlxJson};

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct AdminWalletRechargeReceipt {
    pub(crate) request_fingerprint: String,
    pub(crate) response_snapshot_json: SqlxJson<Value>,
}

/// 按管理员作用域读取首次充值收据；未命中时由新事务去竞争唯一键。
pub(crate) async fn load_admin_wallet_recharge_receipt(
    pool: &Pool<MySql>,
    admin_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<AdminWalletRechargeReceipt>> {
    sqlx::query_as::<_, AdminWalletRechargeReceipt>(
        r#"SELECT request_fingerprint, response_snapshot_json
           FROM admin_wallet_recharges
           WHERE admin_id = ? AND idempotency_key = ?
           LIMIT 1"#,
    )
    .bind(admin_id)
    .bind(idempotency_key)
    .fetch_optional(pool)
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
