//! 预测市场本地关盘 worker。
//!
//! 它不依赖上游同步开关：以 MySQL 当前时间扫描 `end_at <= now` 的开放市场，在市场行锁内再次核对
//! 边界并递增 `market_version`。报价与订单同样锁市场行，因此在 `end_at` 边界只有关盘或资金下单
//! 一方先提交，后者还会用数据库时间复查并 fail closed。

use crate::{
    error::AppResult,
    modules::prediction::{infrastructure, service},
    state::AppState,
};
use sqlx::{MySql, Pool};
use tokio::time::{Duration, interval};
use tracing::{info, warn};

/// 单轮本地关盘统计。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PredictionMarketCloseSummary {
    /// 扫描到的候选市场数。
    pub scanned: u32,
    /// 本轮真正推进到待确认终态的数量。
    pub closed: u32,
    /// 锁定后发现已被同步或其他实例处理的数量。
    pub skipped: u32,
}

/// 以数据库时间执行一轮本地关盘，批量上限收敛到 1..=500。
pub async fn run_once(pool: &Pool<MySql>, limit: u32) -> AppResult<PredictionMarketCloseSummary> {
    let ids = sqlx::query_scalar::<_, u64>(
        r#"SELECT id
           FROM prediction_markets
           WHERE settlement_status = 'open'
             AND end_at IS NOT NULL
             AND end_at <= CURRENT_TIMESTAMP(6)
           ORDER BY end_at ASC, id ASC
           LIMIT ?"#,
    )
    .bind(limit.clamp(1, 500) as i64)
    .fetch_all(pool)
    .await?;
    let mut summary = PredictionMarketCloseSummary::default();
    for market_id in ids {
        summary.scanned += 1;
        let mut tx = pool.begin().await?;
        let market = infrastructure::lock_market(&mut tx, market_id).await?;
        let now = infrastructure::database_now_in_tx(&mut tx).await?;
        let due = market.settlement_status == service::SETTLEMENT_OPEN
            && market.end_at.is_some_and(|end_at| now >= end_at);
        if !due {
            tx.rollback().await?;
            summary.skipped += 1;
            continue;
        }
        let update = sqlx::query(
            r#"UPDATE prediction_markets
               SET display_status = 'hidden',
                   settlement_status = 'pending_confirmation',
                   locally_closed_at = COALESCE(locally_closed_at, ?),
                   market_version = market_version + 1
               WHERE id = ?
                 AND settlement_status = 'open'
                 AND end_at IS NOT NULL
                 AND end_at <= ?"#,
        )
        .bind(now.naive_utc())
        .bind(market_id)
        .bind(now.naive_utc())
        .execute(&mut *tx)
        .await?;
        if update.rows_affected() == 1 {
            tx.commit().await?;
            summary.closed += 1;
        } else {
            tx.rollback().await?;
            summary.skipped += 1;
        }
    }
    Ok(summary)
}

/// 每秒独立推进本地关盘；上游同步关闭或故障不会停止该循环。
pub async fn run_loop(state: AppState) -> AppResult<()> {
    let Some(pool) = state.mysql.clone() else {
        return Ok(());
    };
    let mut ticker = interval(Duration::from_secs(1));
    loop {
        ticker.tick().await;
        match run_once(&pool, 100).await {
            Ok(summary) if summary.closed > 0 => info!(
                scanned = summary.scanned,
                closed = summary.closed,
                skipped = summary.skipped,
                "预测市场本地关盘完成"
            ),
            Ok(_) => {}
            Err(error) => warn!(%error, "预测市场本地关盘失败"),
        }
    }
}
