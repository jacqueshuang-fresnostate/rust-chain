//! 杠杆主动平仓执行记录的 MySQL 适配器。
//!
//! 显式比例平仓以 `(user_id, idempotency_key)` 唯一占位，记录本次从仓位分配的四类金额、
//! 权威退出价、已实现盈亏和真实钱包结算额。记录与钱包、流水、仓位剩余值共用应用层事务，
//! 因此任何后续写入失败都会连同执行记录一起回滚；本模块不自行开启或提交事务。

use crate::{
    error::{AppError, AppResult},
    modules::margin::presentation::MarginPositionCloseExecutionResponse,
};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool, Transaction};

/// 新平仓执行的完整写入快照；所有引用在插入返回前都只读，调用方继续拥有原金额。
pub(crate) struct MarginCloseExecutionWrite<'a> {
    /// 发起请求并拥有仓位的用户。
    pub(crate) user_id: u64,
    /// 被部分或全部平掉的仓位主键。
    pub(crate) position_id: u64,
    /// 用户级幂等键，插入时由数据库唯一索引最终裁决并发竞态。
    pub(crate) idempotency_key: &'a str,
    /// 作用于加锁后剩余仓位的整数比例。
    pub(crate) close_percentage: u16,
    /// 本次参与结算的保证金份额。
    pub(crate) close_margin_amount: &'a BigDecimal,
    /// 本次用于计算盈亏的名义价值份额。
    pub(crate) close_notional_amount: &'a BigDecimal,
    /// 本次释放的借款本金份额。
    pub(crate) close_borrowed_amount: &'a BigDecimal,
    /// 本次从权益扣除的利息份额。
    pub(crate) close_interest_amount: &'a BigDecimal,
    /// 服务端权威标记价。
    pub(crate) exit_price: &'a BigDecimal,
    /// 仅针对本次名义价值计算的已实现盈亏。
    pub(crate) realized_pnl: &'a BigDecimal,
    /// 本次对钱包实际应用的增量；全仓允许为负，逐仓为非负。
    pub(crate) settlement_amount: &'a BigDecimal,
    /// 本次是否消费全部剩余仓位并进入终态。
    pub(crate) fully_closed: bool,
}

/// 在平仓事务内按用户级幂等键加锁读取既有执行，用于同键重放与异参冲突判断。
/// 查询必须发生在钱包写入之前；命中后调用方只回读仓位并提交只读事务，不得再次结算。
pub(crate) async fn lock_margin_close_execution_by_key(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<MarginPositionCloseExecutionResponse>> {
    sqlx::query_as::<_, MarginPositionCloseExecutionResponse>(
        r#"SELECT id, position_id, idempotency_key, close_percentage,
                  close_margin_amount, close_notional_amount, close_borrowed_amount,
                  close_interest_amount, exit_price, realized_pnl, settlement_amount,
                  fully_closed, created_at
           FROM margin_position_close_executions
           WHERE user_id = ? AND idempotency_key = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(idempotency_key)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

/// 在事务开始前只读检查用户级幂等键，命中时可直接绕过行情、账户与仓位行锁。
/// 并发首次请求尚未提交时可能读不到，应用层仍会在事务内复查并由唯一索引兜底。
pub(crate) async fn load_margin_close_execution_by_key_readonly(
    pool: &Pool<MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<MarginPositionCloseExecutionResponse>> {
    sqlx::query_as::<_, MarginPositionCloseExecutionResponse>(
        r#"SELECT id, position_id, idempotency_key, close_percentage,
                  close_margin_amount, close_notional_amount, close_borrowed_amount,
                  close_interest_amount, exit_price, realized_pnl, settlement_amount,
                  fully_closed, created_at
           FROM margin_position_close_executions
           WHERE user_id = ? AND idempotency_key = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

/// 把一次显式平仓结果写入调用方事务，成功返回自增主键；唯一冲突保留原始 SQLx 错误供应用层回滚重放。
/// 插入应位于所有金额计算之后、钱包结算之前，确保同键并发败方在产生资金写入前退出。
pub(crate) async fn insert_margin_close_execution(
    tx: &mut Transaction<'_, MySql>,
    execution: MarginCloseExecutionWrite<'_>,
) -> Result<u64, sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO margin_position_close_executions
           (user_id, position_id, idempotency_key, close_percentage,
            close_margin_amount, close_notional_amount, close_borrowed_amount,
            close_interest_amount, exit_price, realized_pnl, settlement_amount, fully_closed)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(execution.user_id)
    .bind(execution.position_id)
    .bind(execution.idempotency_key)
    .bind(execution.close_percentage)
    .bind(execution.close_margin_amount)
    .bind(execution.close_notional_amount)
    .bind(execution.close_borrowed_amount)
    .bind(execution.close_interest_amount)
    .bind(execution.exit_price)
    .bind(execution.realized_pnl)
    .bind(execution.settlement_amount)
    .bind(execution.fully_closed)
    .execute(&mut **tx)
    .await
    .map(|result| result.last_insert_id())
}

/// 在同一平仓事务内按主键回读刚插入的执行，保证响应与数据库最终序列化值一致。
/// 主键不存在表示事务内部状态异常，返回 NotFound 并由应用层回滚全部资金写入。
pub(crate) async fn load_margin_close_execution_by_id(
    tx: &mut Transaction<'_, MySql>,
    execution_id: u64,
) -> AppResult<MarginPositionCloseExecutionResponse> {
    sqlx::query_as::<_, MarginPositionCloseExecutionResponse>(
        r#"SELECT id, position_id, idempotency_key, close_percentage,
                  close_margin_amount, close_notional_amount, close_borrowed_amount,
                  close_interest_amount, exit_price, realized_pnl, settlement_amount,
                  fully_closed, created_at
           FROM margin_position_close_executions
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(execution_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}
