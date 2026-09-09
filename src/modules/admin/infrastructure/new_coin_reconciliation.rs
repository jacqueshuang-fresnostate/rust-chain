//! 新币派发/退款批次对账查询。
//!
//! 这里不执行任何补发、退款或钱包写入。所有聚合在同一个只执行 `SELECT` 的事务快照中读取，应用层
//! 只负责把守恒差额转换成可读异常。这样后台刷新对账不会改变新币生命周期，也不会与
//! 正在进行的人工派发共享写锁。

use super::*;

#[derive(Debug, Clone)]
pub(crate) struct AdminNewCoinReconciliationRecord {
    pub(crate) project_id: u64,
    pub(crate) symbol: String,
    pub(crate) lifecycle_status: String,
    pub(crate) total_supply: BigDecimal,
    pub(crate) reserved_supply: BigDecimal,
    pub(crate) allocated_supply: BigDecimal,
    pub(crate) remaining_supply: BigDecimal,
    pub(crate) subscription_count: i64,
    pub(crate) pending_manual_count: i64,
    pub(crate) requested_quantity: BigDecimal,
    pub(crate) subscription_allocated_quantity: BigDecimal,
    pub(crate) distribution_quantity: BigDecimal,
    pub(crate) linked_distribution_quantity: BigDecimal,
    pub(crate) unlinked_distribution_quantity: BigDecimal,
    pub(crate) distribution_ledger_quantity: BigDecimal,
    pub(crate) invalid_subscription_link_count: i64,
    pub(crate) manual_quote_amount: BigDecimal,
    pub(crate) manual_frozen_quote_amount: BigDecimal,
    pub(crate) manual_settled_quote_amount: BigDecimal,
    pub(crate) manual_refunded_quote_amount: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct ReconciliationProjectRow {
    id: u64,
    symbol: String,
    lifecycle_status: String,
    total_supply: BigDecimal,
    reserved_supply: BigDecimal,
    allocated_supply: BigDecimal,
    remaining_supply: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct ReconciliationSubscriptionRow {
    subscription_count: i64,
    pending_manual_count: i64,
    requested_quantity: BigDecimal,
    allocated_quantity: BigDecimal,
    manual_quote_amount: BigDecimal,
    manual_frozen_quote_amount: BigDecimal,
    manual_settled_quote_amount: BigDecimal,
    manual_refunded_quote_amount: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct ReconciliationDistributionRow {
    distribution_quantity: BigDecimal,
    linked_distribution_quantity: BigDecimal,
    unlinked_distribution_quantity: BigDecimal,
    invalid_subscription_link_count: i64,
}

/// 在单个一致性快照中读取项目及其申购、派发、退款和钱包流水汇总。
///
/// 对账查询只在事务中执行 `SELECT` 并以项目主键读取；聚合字段通过 `CAST(... AS DECIMAL)`
/// 保持与业务金额相同的十进制精度。钱包流水只统计 `new_coin_distribution` 的正向腿，
/// 因而锁仓和可用余额两种入账路径都能纳入同一总量，而退款收据（数量为零）不会虚增发行量。
pub(crate) async fn load_admin_new_coin_reconciliation(
    pool: &Pool<MySql>,
    project_id: u64,
) -> AppResult<AdminNewCoinReconciliationRecord> {
    let mut tx = pool.begin().await?;

    let project = sqlx::query_as::<_, ReconciliationProjectRow>(
        r#"SELECT id, symbol, lifecycle_status, total_supply, reserved_supply,
                  allocated_supply, remaining_supply
           FROM new_coin_projects
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let subscriptions = sqlx::query_as::<_, ReconciliationSubscriptionRow>(
        r#"SELECT COUNT(*) AS subscription_count,
                  CAST(COALESCE(SUM(
                    CASE WHEN settlement_mode = 'manual_distribution'
                              AND (status = 'pending' OR frozen_quote_amount > 0)
                         THEN 1 ELSE 0 END
                  ), 0) AS SIGNED) AS pending_manual_count,
                  CAST(COALESCE(SUM(requested_quantity), 0) AS DECIMAL(38,18)) AS requested_quantity,
                  CAST(COALESCE(SUM(allocated_quantity), 0) AS DECIMAL(38,18)) AS allocated_quantity,
                  CAST(COALESCE(SUM(CASE WHEN settlement_mode = 'manual_distribution' THEN quote_amount ELSE 0 END), 0) AS DECIMAL(38,18)) AS manual_quote_amount,
                  CAST(COALESCE(SUM(CASE WHEN settlement_mode = 'manual_distribution' THEN frozen_quote_amount ELSE 0 END), 0) AS DECIMAL(38,18)) AS manual_frozen_quote_amount,
                  CAST(COALESCE(SUM(CASE WHEN settlement_mode = 'manual_distribution' THEN COALESCE(settled_quote_amount, 0) ELSE 0 END), 0) AS DECIMAL(38,18)) AS manual_settled_quote_amount,
                  CAST(COALESCE(SUM(CASE WHEN settlement_mode = 'manual_distribution' THEN COALESCE(refunded_quote_amount, 0) ELSE 0 END), 0) AS DECIMAL(38,18)) AS manual_refunded_quote_amount
           FROM new_coin_subscriptions
           WHERE project_id = ?"#,
    )
    .bind(project_id)
    .fetch_one(&mut *tx)
    .await?;

    let distributions = sqlx::query_as::<_, ReconciliationDistributionRow>(
        r#"SELECT CAST(COALESCE(SUM(distributions.quantity), 0) AS DECIMAL(38,18)) AS distribution_quantity,
                  CAST(COALESCE(SUM(CASE
                      WHEN subscriptions.id IS NOT NULL
                       AND subscriptions.project_id = distributions.project_id
                       AND subscriptions.user_id = distributions.user_id
                       AND distributions.asset_id = projects.asset_id
                       AND (projects.quote_asset_id IS NULL OR subscriptions.quote_asset = projects.quote_asset_id)
                      THEN distributions.quantity ELSE 0 END), 0) AS DECIMAL(38,18)) AS linked_distribution_quantity,
                  CAST(COALESCE(SUM(CASE WHEN distributions.subscription_id IS NULL
                                         THEN distributions.quantity ELSE 0 END), 0) AS DECIMAL(38,18)) AS unlinked_distribution_quantity,
                  CAST(COALESCE(SUM(CASE
                      WHEN distributions.subscription_id IS NOT NULL
                       AND (subscriptions.id IS NULL
                            OR subscriptions.project_id <> distributions.project_id
                            OR subscriptions.user_id <> distributions.user_id
                            OR distributions.asset_id <> projects.asset_id
                            OR (projects.quote_asset_id IS NOT NULL AND subscriptions.quote_asset <> projects.quote_asset_id))
                      THEN 1 ELSE 0 END), 0) AS SIGNED) AS invalid_subscription_link_count
           FROM new_coin_distributions distributions
           INNER JOIN new_coin_projects projects
             ON projects.id = distributions.project_id
           LEFT JOIN new_coin_subscriptions subscriptions
             ON subscriptions.id = distributions.subscription_id
           WHERE distributions.project_id = ?"#,
    )
    .bind(project_id)
    .fetch_one(&mut *tx)
    .await?;

    // 同一派发幂等键可能有两条钱包腿（可用/锁定），这里只比较数量总和，不把退款或
    // 其他 ref_type 混入；如果历史上出现重复腿，差额会明确暴露在对账结果中。
    let distribution_ledger_quantity = sqlx::query_scalar::<_, BigDecimal>(
        r#"SELECT CAST(COALESCE(SUM(ledger.amount), 0) AS DECIMAL(38,18))
           FROM wallet_ledger ledger
           INNER JOIN new_coin_distributions distributions
             ON distributions.idempotency_key = ledger.ref_id
            AND distributions.project_id = ?
            AND distributions.user_id = ledger.user_id
            AND distributions.asset_id = ledger.asset_id
           WHERE ledger.ref_type = 'new_coin_distribution'
             AND ledger.amount > 0"#,
    )
    .bind(project_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(AdminNewCoinReconciliationRecord {
        project_id: project.id,
        symbol: project.symbol,
        lifecycle_status: project.lifecycle_status,
        total_supply: project.total_supply,
        reserved_supply: project.reserved_supply,
        allocated_supply: project.allocated_supply,
        remaining_supply: project.remaining_supply,
        subscription_count: subscriptions.subscription_count,
        pending_manual_count: subscriptions.pending_manual_count,
        requested_quantity: subscriptions.requested_quantity,
        subscription_allocated_quantity: subscriptions.allocated_quantity,
        distribution_quantity: distributions.distribution_quantity,
        linked_distribution_quantity: distributions.linked_distribution_quantity,
        unlinked_distribution_quantity: distributions.unlinked_distribution_quantity,
        distribution_ledger_quantity,
        invalid_subscription_link_count: distributions.invalid_subscription_link_count,
        manual_quote_amount: subscriptions.manual_quote_amount,
        manual_frozen_quote_amount: subscriptions.manual_frozen_quote_amount,
        manual_settled_quote_amount: subscriptions.manual_settled_quote_amount,
        manual_refunded_quote_amount: subscriptions.manual_refunded_quote_amount,
    })
}

#[cfg(test)]
#[path = "../../../../tests/unit_src/src_modules_admin_infrastructure_new_coin_reconciliation_tests.rs"]
mod tests;
