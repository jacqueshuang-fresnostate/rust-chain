//! 自动扫描只选择候选，真正释放仍复用用户路径的加锁事务及证据规则。
use super::unlock_eligibility::*;
use crate::error::AppResult;
use chrono::{DateTime, Utc};
use sqlx::{MySql, Pool};

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct ScannedUnlock {
    pub(crate) user_id: u64,
    pub(crate) lock_position_id: u64,
    pub(crate) idempotency_key: String,
}

/// 候选与实际释放共用身份、到期和完整缴费凭证谓词；本查询不持锁，写事务必须重新验证。
pub(crate) async fn scan_due_unlocks(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<Vec<ScannedUnlock>> {
    let query = format!(
        r#"SELECT unlocks.user_id, unlocks.lock_position_id, unlocks.idempotency_key
        FROM asset_unlock_records unlocks
        JOIN asset_lock_positions positions ON positions.id = unlocks.lock_position_id
        WHERE {UNLOCK_IDENTITY_SQL} AND {UNLOCK_MATURITY_SQL} AND {UNLOCK_FEE_EVIDENCE_SQL}
        ORDER BY positions.unlock_at, unlocks.id LIMIT ?"#
    );
    Ok(sqlx::query_as(&query)
        .bind(now.naive_utc())
        .bind(now.naive_utc())
        .bind(i64::from(limit.clamp(1, 100)))
        .fetch_all(pool)
        .await?)
}

/// 全量统计到期但缺有效费用证据的记录，包括 paid 标记伪成功、残缺分录及伪零费用。
pub(crate) async fn count_fee_blocked_unlocks(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
) -> AppResult<u32> {
    let query = format!(
        r#"SELECT COUNT(*) FROM asset_unlock_records unlocks
        JOIN asset_lock_positions positions ON positions.id = unlocks.lock_position_id
        WHERE {UNLOCK_IDENTITY_SQL} AND {UNLOCK_MATURITY_SQL}
          AND NOT COALESCE({UNLOCK_FEE_EVIDENCE_SQL}, FALSE)"#
    );
    let count: i64 = sqlx::query_scalar(&query)
        .bind(now.naive_utc())
        .bind(now.naive_utc())
        .fetch_one(pool)
        .await?;
    Ok(count.try_into().unwrap_or(u32::MAX))
}
