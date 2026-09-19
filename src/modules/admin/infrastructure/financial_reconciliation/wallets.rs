//! 钱包最新账本桶快照对比；按账户、用户和资产隔离，差额互抵不能掩盖异常账户。

use super::*;

fn evidence_cte(wallet: &str, ledger: &str) -> String {
    format!(
        r#"WITH ranked AS (
          SELECT id, user_id, available_after, frozen_after, locked_after,
            ROW_NUMBER() OVER (PARTITION BY user_id ORDER BY id DESC) AS seq
          FROM {ledger} WHERE asset_id = ?
        ), latest AS (SELECT * FROM ranked WHERE seq = 1),
        wallet AS (SELECT * FROM {wallet} WHERE asset_id = ?),
        identities AS (SELECT user_id FROM wallet UNION SELECT user_id FROM latest),
        evidence AS (
          SELECT ids.user_id, w.id AS wallet_id, l.id AS ledger_id,
            w.available, w.frozen, w.locked,
            l.available_after, l.frozen_after, l.locked_after,
            CASE WHEN w.id IS NULL THEN 'missing_wallet'
              WHEN l.id IS NULL THEN 'missing_ledger'
              WHEN w.available <> l.available_after OR w.frozen <> l.frozen_after
                OR w.locked <> l.locked_after THEN 'mismatch' ELSE 'matched' END AS issue
          FROM identities ids LEFT JOIN wallet w ON w.user_id = ids.user_id
          LEFT JOIN latest l ON l.user_id = ids.user_id
        ) "#
    )
}

/// 最新流水以每账户自增 ID 排序，时间戳相同也确定；缺少对侧记录不参与可比差额。
/// 表名来自固定两组内部常量，金额与全量差异计数在数据库聚合，明细仅有界返回。
pub(super) async fn read(
    tx: &mut MySqlConnection,
    asset_id: u64,
    limit: u32,
) -> AppResult<(Vec<WalletEvidence>, Vec<WalletDifference>, i64)> {
    let mut summaries = Vec::new();
    let mut differences = Vec::new();
    let mut total = 0;
    for (scope, wallet, ledger) in [
        ("spot", "wallet_accounts", "wallet_ledger"),
        ("margin", "margin_wallet_accounts", "margin_wallet_ledger"),
    ] {
        let cte = evidence_cte(wallet, ledger);
        let sql = format!(
            r#"{cte} SELECT ? AS account_type,
              COUNT(wallet_id) AS wallet_count,
              CAST(COALESCE(SUM(issue = 'missing_ledger'), 0) AS SIGNED) AS missing_ledger_count,
              CAST(COALESCE(SUM(issue = 'missing_wallet'), 0) AS SIGNED) AS missing_wallet_count,
              CAST(COALESCE(SUM(issue = 'mismatch'), 0) AS SIGNED) AS mismatch_count,
              CAST(COALESCE(SUM(available), 0) AS CHAR) AS available,
              CAST(COALESCE(SUM(frozen), 0) AS CHAR) AS frozen,
              CAST(COALESCE(SUM(locked), 0) AS CHAR) AS locked,
              CAST(COALESCE(SUM(available - available_after), 0) AS CHAR) AS comparable_available_delta,
              CAST(COALESCE(SUM(frozen - frozen_after), 0) AS CHAR) AS comparable_frozen_delta,
              CAST(COALESCE(SUM(locked - locked_after), 0) AS CHAR) AS comparable_locked_delta
              FROM evidence"#
        );
        let summary: WalletEvidence = sqlx::query_as(&sql)
            .bind(asset_id)
            .bind(asset_id)
            .bind(scope)
            .fetch_one(&mut *tx)
            .await?;
        total +=
            summary.missing_ledger_count + summary.missing_wallet_count + summary.mismatch_count;
        summaries.push(summary);
        let sql = format!(
            r#"{cte} SELECT ? AS account_type, user_id, issue, ledger_id,
              CAST(available AS CHAR) AS available, CAST(frozen AS CHAR) AS frozen,
              CAST(locked AS CHAR) AS locked, CAST(available_after AS CHAR) AS available_after,
              CAST(frozen_after AS CHAR) AS frozen_after, CAST(locked_after AS CHAR) AS locked_after
              FROM evidence WHERE issue <> 'matched' ORDER BY user_id LIMIT ?"#
        );
        let remaining = limit.saturating_sub(differences.len() as u32);
        let rows: Vec<WalletDifference> = sqlx::query_as(&sql)
            .bind(asset_id)
            .bind(asset_id)
            .bind(scope)
            .bind(remaining)
            .fetch_all(&mut *tx)
            .await?;
        differences.extend(rows);
    }
    Ok((summaries, differences, total))
}
