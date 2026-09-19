//! 只读对账用例；所有金额证据来自同一可重复读快照，不调用任何资金执行入口。

use super::admin_mysql_pool;
use crate::{
    error::{AppError, AppResult},
    modules::admin::{
        infrastructure::financial_reconciliation as store,
        presentation::financial_reconciliation::*,
    },
};
use chrono::Utc;
use sqlx::{Connection, MySqlPool};

pub(crate) mod snapshots;

fn validate(query: &FinancialReconciliationQuery) -> AppResult<(u32, u32)> {
    let limit = query.limit.unwrap_or(20);
    let offset = query.offset.unwrap_or(0);
    if query.asset_id == Some(0) || !(1..=100).contains(&limit) || offset > 100_000 {
        return Err(AppError::Validation(
            "资产 ID 须为正整数，分页条数须为 1–100，偏移不得超过 100000".into(),
        ));
    }
    Ok((limit, offset))
}

/// 在数据库访问前校验参数，再显式建立只读一致快照；任何依赖失败均返回错误而非零差异。
/// 不持有写锁、不追加审计、不修复余额；目录分页与差异明细截断均保留真实总数。
pub(crate) async fn get_financial_reconciliation(
    pool: Option<MySqlPool>,
    query: FinancialReconciliationQuery,
) -> AppResult<FinancialReconciliationResponse> {
    let (limit, offset) = validate(&query)?;
    let pool = admin_mysql_pool(pool)?;
    let mut connection = pool.acquire().await?;
    store::prepare_snapshot(&mut connection).await?;
    let mut tx = connection
        .begin_with("START TRANSACTION WITH CONSISTENT SNAPSHOT, READ ONLY")
        .await?;
    let checked_at = Utc::now().timestamp_millis();
    let (assets, total) = store::assets(&mut tx, limit, offset).await?;
    let report = if let Some(asset_id) = query.asset_id {
        Some(store::report(&mut tx, asset_id).await?)
    } else {
        None
    };
    tx.commit().await?;
    Ok(FinancialReconciliationResponse {
        assets,
        total,
        limit,
        offset,
        report,
        checked_at,
    })
}

#[cfg(test)]
#[path = "../../../../tests/unit_src/src_modules_admin_financial_reconciliation_tests.rs"]
mod tests;
