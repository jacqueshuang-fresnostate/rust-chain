//! 平台账务只读证据；聚合保持资产隔离，不从平台科目净变动推导托管资产或偿付能力。

use crate::{
    error::{AppError, AppResult},
    modules::admin::presentation::financial_reconciliation::*,
};
use sqlx::MySqlConnection;

mod obligations;
pub(crate) mod snapshots;
mod wallets;

const DETAIL_LIMIT: u32 = 100;

/// 下一事务使用可重复读；仅设置连接属性，不更改全局配置、余额或业务记录。
pub(crate) async fn prepare_snapshot(connection: &mut MySqlConnection) -> AppResult<()> {
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(connection)
        .await?;
    Ok(())
}

fn check_precision(asset: &ReconciliationAsset) -> AppResult<()> {
    if !(0..=18).contains(&asset.precision_scale) {
        return Err(AppError::Internal(
            "invalid reconciliation asset precision".into(),
        ));
    }
    Ok(())
}

/// 目录包含停用资产，避免隐藏历史负债；全量计数和页内行共享调用者快照。
pub(crate) async fn assets(
    tx: &mut MySqlConnection,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<ReconciliationAsset>, i64)> {
    let total = sqlx::query_scalar("SELECT COUNT(*) FROM assets")
        .fetch_one(&mut *tx)
        .await?;
    let rows: Vec<ReconciliationAsset> = sqlx::query_as(
        "SELECT id AS asset_id, symbol, precision_scale FROM assets ORDER BY id LIMIT ? OFFSET ?",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&mut *tx)
    .await?;
    for asset in &rows {
        check_precision(asset)?;
    }
    Ok((rows, total))
}

/// 已有分录按交易键与资产独立校验；未知历史保持 partial，首次分录时间不解释为覆盖起点。
/// 所有明细最多 100 条并返回全量计数；数据库错误原样上抛，绝不伪造“平衡”结论。
pub(crate) async fn report(
    tx: &mut MySqlConnection,
    asset_id: u64,
) -> AppResult<FinancialReconciliationReport> {
    let asset: ReconciliationAsset =
        sqlx::query_as("SELECT id AS asset_id, symbol, precision_scale FROM assets WHERE id = ?")
            .bind(asset_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;
    check_precision(&asset)?;
    let journal = sqlx::query_as(
        r#"SELECT
          (SELECT COUNT(*) FROM platform_financial_journal WHERE asset_id = ?) AS entry_count,
          COUNT(*) AS transaction_count,
          CAST(COALESCE(SUM(net_amount <> 0), 0) AS SIGNED) AS imbalanced_transaction_count,
          CAST(UNIX_TIMESTAMP(MIN(first_at)) * 1000 AS SIGNED) AS first_entry_at,
          CAST(UNIX_TIMESTAMP(MAX(last_at)) * 1000 AS SIGNED) AS last_entry_at
          FROM (SELECT SUM(amount) AS net_amount, MIN(created_at) AS first_at,
            MAX(created_at) AS last_at FROM platform_financial_journal
            WHERE asset_id = ? GROUP BY transaction_key) grouped"#,
    )
    .bind(asset_id)
    .bind(asset_id)
    .fetch_one(&mut *tx)
    .await?;
    let journal_differences = sqlx::query_as(
        r#"SELECT transaction_key, COUNT(*) AS entry_count, CAST(SUM(amount) AS CHAR) AS net_amount
          FROM platform_financial_journal WHERE asset_id = ?
          GROUP BY transaction_key HAVING SUM(amount) <> 0
          ORDER BY transaction_key LIMIT ?"#,
    )
    .bind(asset_id)
    .bind(DETAIL_LIMIT)
    .fetch_all(&mut *tx)
    .await?;
    let journal_movement_count = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM (SELECT context, account_code FROM platform_financial_journal
           WHERE asset_id = ? GROUP BY context, account_code) grouped"#,
    )
    .bind(asset_id)
    .fetch_one(&mut *tx)
    .await?;
    let journal_movements = sqlx::query_as(
        r#"SELECT context, account_code, COUNT(*) AS entry_count,
          CAST(SUM(amount) AS CHAR) AS net_movement
          FROM platform_financial_journal WHERE asset_id = ?
          GROUP BY context, account_code ORDER BY context, account_code LIMIT ?"#,
    )
    .bind(asset_id)
    .bind(DETAIL_LIMIT)
    .fetch_all(&mut *tx)
    .await?;
    let (wallets, wallet_differences, wallet_difference_count) =
        wallets::read(tx, asset_id, DETAIL_LIMIT).await?;
    let obligations = obligations::read(tx, asset_id, asset.precision_scale).await?;
    Ok(FinancialReconciliationReport {
        asset,
        coverage: "partial",
        limitations: vec![
            "历史覆盖不完整：仅检查当前已存在的分录，未补记历史；首次分录时间不代表完整覆盖起点，零分录不代表零业务。",
            "分录净变动不是实际托管库存；没有期初余额、链上或托管机构余额核验，不能据此声明偿付能力或储备证明。",
            "钱包仅与同一账户最新流水 ID 的三桶快照比较，不重建全历史；缺失流水或钱包单独列出，不按零余额推断。",
            "钱包汇总包含数据库中的内部账户，未区分客户与平台自有资金，不能将汇总直接称为客户负债。",
            "未结义务按原始资产分别列示，含冻结、抵押和条件赔付等重叠指标，不能与钱包余额相加，也不能跨币种求总额。",
            "理财仅列未赎回本金，未估算收益及赎回费；贷款仅列已放款未还本金，未估算未入账利息；杠杆不估值未实现盈亏或未来利息。",
            "现货未成交量按交易对基础资产列示，不推导报价资产剩余冻结；新币仅列人工申购冻结计价款，未覆盖全部待发行与锁仓履约义务。",
            "佣金仅列已有结算资产快照的待结记录；缺失资产身份及尚未生成的佣金不计入。未列业务不等于不存在义务。",
            "本报告不核对每笔业务是否已生成全部应有科目；人工采集仅保存读取时证据，不是日终结账，跟进备注不代表差异已解决，不执行补账或资金操作。",
        ],
        journal,
        journal_differences,
        journal_movements,
        journal_movement_count,
        wallets,
        wallet_differences,
        wallet_difference_count,
        obligations,
        detail_limit: DETAIL_LIMIT,
    })
}
