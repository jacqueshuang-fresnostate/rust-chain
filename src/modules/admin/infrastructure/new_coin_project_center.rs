//! 项目中心的统计和发行参数持久化；不读写用户钱包。
use super::*;

/// 在项目读取事务中统计订单；终态异常残留冻结同样阻止上市。
pub(crate) async fn new_coin_project_order_counts_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
) -> AppResult<(i64, i64)> {
    sqlx::query_as("SELECT COUNT(*), CAST(COALESCE(SUM(settlement_mode = 'manual_distribution' AND (status = 'pending' OR frozen_quote_amount > 0)), 0) AS SIGNED) FROM new_coin_subscriptions WHERE project_id = ?")
        .bind(project_id).fetch_one(&mut **tx).await.map_err(AppError::from)
}

/// 调用方持项目锁且已确认无订单/供给占用；剩余额度与总量同时更新，保持供给恒等式。
pub(crate) async fn update_new_coin_issuance_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
    total: &BigDecimal,
    price: &BigDecimal,
) -> AppResult<()> {
    sqlx::query("UPDATE new_coin_projects SET total_supply = ?, remaining_supply = ?, issue_price = ? WHERE id = ?")
        .bind(total).bind(total).bind(price).bind(project_id).execute(&mut **tx).await?;
    Ok(())
}
