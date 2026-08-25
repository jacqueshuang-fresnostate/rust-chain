//! 抵押贷周期健康扫描与幂等强制清算 worker。
//!
//! 每个候选先按订单固化的符号、来源和最大年龄读取权威 Redis ticker，再进入单订单事务。
//! 陈旧或缺失价格只记失败并保持订单和资金不变；并发还款、逾期推进与重复扫描由订单行锁串行化。

use crate::{
    error::{AppError, AppResult},
    modules::loan::{
        liquidation::{
            LoanLiquidationOutcome, fetch_loan_liquidation_candidates,
            liquidate_loan_order_if_required,
        },
        oracle::load_fresh_loan_oracle_price,
    },
};
use chrono::{DateTime, Utc};
use redis::aio::ConnectionManager;
use sqlx::{MySql, Pool};
use tracing::warn;

/// 单轮健康扫描的可观测计数。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LoanHealthSummary {
    pub scanned: u32,
    pub liquidated: u32,
    pub healthy: u32,
    pub skipped: u32,
    pub failed: u32,
}

/// 扫描并处理最多 limit 笔抵押债务；单笔故障被隔离，不回滚已经提交的其他清算。
pub async fn run_once_with_dependencies(
    pool: &Pool<MySql>,
    redis: &ConnectionManager,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<LoanHealthSummary> {
    let candidates = fetch_loan_liquidation_candidates(pool, limit).await?;
    let mut summary = LoanHealthSummary::default();
    for candidate in candidates {
        summary.scanned += 1;
        let outcome = async {
            // 大批次扫描不能永久复用轮次起点；每单取更晚时钟，避免尾部订单使用已过期报价。
            let candidate_now = now.max(Utc::now());
            let oracle_source = candidate.oracle_source.as_deref().ok_or_else(|| {
                AppError::Internal("loan order is missing oracle_source snapshot".to_owned())
            })?;
            let oracle_symbol = candidate.oracle_symbol.as_deref().ok_or_else(|| {
                AppError::Internal("loan order is missing oracle_symbol snapshot".to_owned())
            })?;
            let oracle_max_age_seconds = candidate.oracle_max_age_seconds.ok_or_else(|| {
                AppError::Internal(
                    "loan order is missing oracle_max_age_seconds snapshot".to_owned(),
                )
            })?;
            let ticker = load_fresh_loan_oracle_price(
                Some(redis),
                oracle_source,
                oracle_symbol,
                oracle_max_age_seconds,
                candidate_now,
            )
            .await?;
            liquidate_loan_order_if_required(pool, candidate.order_id, &ticker, candidate_now).await
        }
        .await;
        match outcome {
            Ok(LoanLiquidationOutcome::Liquidated(_)) => summary.liquidated += 1,
            Ok(LoanLiquidationOutcome::NotRequired { .. }) => summary.healthy += 1,
            Ok(
                LoanLiquidationOutcome::AlreadyLiquidated(_)
                | LoanLiquidationOutcome::SkippedTerminal,
            ) => summary.skipped += 1,
            Err(error) => {
                summary.failed += 1;
                warn!(
                    order_id = candidate.order_id,
                    %error,
                    "贷款健康扫描单项失败"
                );
            }
        }
    }
    Ok(summary)
}
