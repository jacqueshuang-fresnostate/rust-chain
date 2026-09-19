use super::LoanProductTermsRow;
use crate::error::{AppError, AppResult};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool, Transaction};

/// 借贷准入专用用户互斥行，无外键，避免与既有订单先锁的钱包/用户外键产生反向依赖。
/// 申请和审批必须先调用本函数，再锁产品；本事务此前不得进行任何一致性读取。
pub(crate) async fn lock_loan_exposure_user(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO loan_user_exposure_locks (user_id) VALUES (?) ON DUPLICATE KEY UPDATE user_id = user_id",
    )
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 事务外仅定位不可变的用户和产品编号；状态与准入条款仍必须在串行锁之后重读。
pub(crate) async fn load_loan_exposure_identity(
    pool: &Pool<MySql>,
    order_id: u64,
) -> AppResult<(u64, u64)> {
    sqlx::query_as("SELECT user_id, product_id FROM loan_orders WHERE id = ?")
        .bind(order_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

/// 持有产品锁时禁止把仍有待审或未结本金的产品改为其他币种，防止限额单位被偷换。
/// 不锁订单，终态并发最多造成保守拒绝，不与还款/拒绝的订单锁发生反向等待。
pub(crate) async fn ensure_loan_product_exposure_asset(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    asset_id: u64,
) -> AppResult<()> {
    let incompatible: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM loan_orders WHERE product_id = ? AND asset_id <> ? AND status IN ('pending', 'disbursed', 'overdue'))",
    )
    .bind(product_id)
    .bind(asset_id)
    .fetch_one(&mut **tx)
    .await?;
    if incompatible {
        return Err(AppError::Conflict(
            "cannot change loan asset while principal exposure remains".to_owned(),
        ));
    }
    Ok(())
}

/// 查询待审预留及未结本金；用户跨产品按同币种合计，产品容量也严格限制在同币种。
/// 调用方先持有用户和产品锁，之后才建立一致性快照，保证并发增加敞口可见。
/// 不锁聚合订单，避免持有产品锁等待另一用户审批订单造成死锁；并发终态只会保守多计。
pub(crate) async fn load_loan_principal_exposure(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    product: &LoanProductTermsRow,
    candidate_id: Option<u64>,
) -> AppResult<(BigDecimal, BigDecimal, bool)> {
    let (user_total, product_total): (BigDecimal, BigDecimal) = sqlx::query_as(
        r#"SELECT
             COALESCE(SUM(CASE WHEN user_id = ? THEN amount ELSE 0 END), 0),
             COALESCE(SUM(CASE WHEN product_id = ? THEN amount ELSE 0 END), 0)
           FROM loan_orders
           WHERE asset_id = ? AND (user_id = ? OR product_id = ?)
             AND status IN ('pending', 'disbursed', 'overdue')
             AND (? IS NULL OR id <> ?)"#,
    )
    .bind(user_id)
    .bind(product.id)
    .bind(product.asset_id)
    .bind(user_id)
    .bind(product.id)
    .bind(candidate_id)
    .bind(candidate_id)
    .fetch_one(&mut **tx)
    .await?;
    let overdue = if product.deny_borrowing_while_overdue {
        sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM loan_orders WHERE user_id = ?
               AND (status = 'overdue' OR (status = 'disbursed' AND due_at <= CURRENT_TIMESTAMP(6))))"#,
        )
        .bind(user_id)
        .fetch_one(&mut **tx)
        .await?
    } else {
        false
    };
    Ok((user_total, product_total, overdue))
}
