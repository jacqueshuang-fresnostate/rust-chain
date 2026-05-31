use crate::{
    error::{AppError, AppResult},
    modules::events::{EventBroadcastHub, EventBroadcastMessage},
    state::AppState,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::{MySql, Pool};
use tokio::time::{Duration, interval};
use tracing::{error, info, warn};

pub struct UnlockScannerWorker;

impl UnlockScannerWorker {
    pub async fn run_once(
        &self,
        state: &AppState,
        now: DateTime<Utc>,
        limit: u32,
    ) -> AppResult<UnlockScannerSummary> {
        run_once(state, now, limit).await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnlockScannerSummary {
    pub scanned: u32,
    pub released: u32,
    pub blocked_fee: u32,
    pub skipped: u32,
}

impl UnlockScannerSummary {
    fn empty() -> Self {
        Self {
            scanned: 0,
            released: 0,
            blocked_fee: 0,
            skipped: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockPositionStatus {
    Active,
    Released,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockScanPosition {
    pub id: String,
    pub unlock_at: DateTime<Utc>,
    pub status: LockPositionStatus,
}

#[derive(Debug, sqlx::FromRow)]
struct DueUnlockCandidate {
    unlock_id: u64,
}

#[derive(Debug, sqlx::FromRow)]
struct ReleasableUnlockRow {
    unlock_id: u64,
    user_id: u64,
    asset_id: u64,
    lock_position_id: u64,
    unlock_quantity: BigDecimal,
    unlock_fee_enabled: bool,
    fee_paid_status: String,
    idempotency_key: String,
    remaining_amount: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct WalletBalanceRow {
    available: BigDecimal,
    frozen: BigDecimal,
    locked: BigDecimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockReleaseEvent {
    user_id: u64,
    unlock_id: String,
    lock_position_id: u64,
    asset_id: u64,
    released_amount: BigDecimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum UnlockReleaseOutcome {
    Released(UnlockReleaseEvent),
    FeeBlocked,
    Skipped,
}

pub fn due_unlock_positions(
    positions: &[UnlockScanPosition],
    now: DateTime<Utc>,
) -> Vec<&UnlockScanPosition> {
    positions
        .iter()
        .filter(|position| {
            position.status == LockPositionStatus::Active && position.unlock_at <= now
        })
        .collect()
}

pub async fn run_once(
    state: &AppState,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<UnlockScannerSummary> {
    let pool = state.mysql.as_ref().ok_or_else(|| {
        AppError::Internal("mysql pool is required for unlock scanner".to_owned())
    })?;
    release_due_unlock_positions_with_broadcast(
        pool,
        state.event_broadcast_hub.as_ref(),
        now,
        limit,
    )
    .await
}

pub async fn release_due_unlock_positions(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<UnlockScannerSummary> {
    release_due_unlock_positions_with_broadcast(pool, None, now, limit).await
}

pub async fn release_due_unlock_positions_with_broadcast(
    pool: &Pool<MySql>,
    hub: Option<&EventBroadcastHub>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<UnlockScannerSummary> {
    let candidates = due_unlock_candidates(pool, now, limit).await?;
    let mut summary = UnlockScannerSummary::empty();
    summary.scanned = candidates.len() as u32;

    for candidate in candidates {
        match release_due_unlock_by_id(pool, candidate.unlock_id, now).await? {
            UnlockReleaseOutcome::Released(event) => {
                summary.released += 1;
                publish_unlock_release_event(hub, &event);
            }
            UnlockReleaseOutcome::FeeBlocked => summary.blocked_fee += 1,
            UnlockReleaseOutcome::Skipped => summary.skipped += 1,
        }
    }

    summary.blocked_fee += count_fee_blocked_due_unlocks(pool, now).await?;
    Ok(summary)
}

pub async fn run_loop(state: AppState, interval_seconds: u64, limit: u32) -> AppResult<()> {
    let mut ticker = interval(Duration::from_secs(interval_seconds.max(1)));

    loop {
        ticker.tick().await;
        match run_once(&state, Utc::now(), limit).await {
            Ok(summary) => info!(
                scanned = summary.scanned,
                released = summary.released,
                blocked_fee = summary.blocked_fee,
                skipped = summary.skipped,
                "解禁扫描周期完成"
            ),
            Err(error) => error!(%error, "解禁扫描周期失败"),
        }
    }
}

async fn due_unlock_candidates(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<Vec<DueUnlockCandidate>> {
    sqlx::query_as::<_, DueUnlockCandidate>(
        r#"SELECT unlocks.id AS unlock_id
           FROM asset_unlock_records unlocks
           INNER JOIN asset_lock_positions positions ON positions.id = unlocks.lock_position_id
           WHERE unlocks.status = 'pending'
             AND unlocks.user_id = positions.user_id
             AND unlocks.asset_id = positions.asset_id
             AND unlocks.unlock_quantity > 0
             AND positions.status = 'active'
             AND positions.unlock_at <= ?
             AND positions.remaining_amount >= unlocks.unlock_quantity
             AND (unlocks.unlock_fee_enabled = false OR unlocks.fee_paid_status IN ('paid', 'not_required'))
           ORDER BY positions.unlock_at ASC, unlocks.id ASC
           LIMIT ?"#,
    )
    .bind(now.naive_utc())
    .bind(unlock_scan_limit(limit) as i64)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

async fn count_fee_blocked_due_unlocks(pool: &Pool<MySql>, now: DateTime<Utc>) -> AppResult<u32> {
    let (blocked,): (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*)
           FROM asset_unlock_records unlocks
           INNER JOIN asset_lock_positions positions ON positions.id = unlocks.lock_position_id
           WHERE unlocks.status = 'pending'
             AND unlocks.user_id = positions.user_id
             AND unlocks.asset_id = positions.asset_id
             AND unlocks.unlock_quantity > 0
             AND positions.status = 'active'
             AND positions.unlock_at <= ?
             AND positions.remaining_amount >= unlocks.unlock_quantity
             AND unlocks.unlock_fee_enabled = true
             AND unlocks.fee_paid_status NOT IN ('paid', 'not_required')"#,
    )
    .bind(now.naive_utc())
    .fetch_one(pool)
    .await?;
    Ok(blocked.try_into().unwrap_or(u32::MAX))
}

async fn release_due_unlock_by_id(
    pool: &Pool<MySql>,
    unlock_id: u64,
    now: DateTime<Utc>,
) -> AppResult<UnlockReleaseOutcome> {
    let mut tx = pool.begin().await?;
    let Some(row) = sqlx::query_as::<_, ReleasableUnlockRow>(
        r#"SELECT unlocks.id AS unlock_id,
                  unlocks.user_id,
                  unlocks.asset_id,
                  unlocks.lock_position_id,
                  unlocks.unlock_quantity,
                  unlocks.unlock_fee_enabled,
                  unlocks.fee_paid_status,
                  unlocks.idempotency_key,
                  positions.remaining_amount
           FROM asset_unlock_records unlocks
           INNER JOIN asset_lock_positions positions ON positions.id = unlocks.lock_position_id
           WHERE unlocks.id = ?
             AND unlocks.status = 'pending'
             AND unlocks.user_id = positions.user_id
             AND unlocks.asset_id = positions.asset_id
             AND unlocks.unlock_quantity > 0
             AND positions.status = 'active'
             AND positions.unlock_at <= ?
             AND positions.remaining_amount >= unlocks.unlock_quantity
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(unlock_id)
    .bind(now.naive_utc())
    .fetch_optional(&mut *tx)
    .await?
    else {
        tx.rollback().await?;
        return Ok(UnlockReleaseOutcome::Skipped);
    };

    if requires_fee_payment(row.unlock_fee_enabled, &row.fee_paid_status) {
        tx.rollback().await?;
        return Ok(UnlockReleaseOutcome::FeeBlocked);
    }

    let Some(wallet) = sqlx::query_as::<_, WalletBalanceRow>(
        r#"SELECT available, frozen, locked
           FROM wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(row.user_id)
    .bind(row.asset_id)
    .fetch_optional(&mut *tx)
    .await?
    else {
        return Err(AppError::Validation(
            "wallet account is required before unlock scanner release".to_owned(),
        ));
    };

    if wallet.locked < row.unlock_quantity {
        return Err(AppError::Validation(
            "wallet locked balance is insufficient for unlock scanner release".to_owned(),
        ));
    }

    let available_after = wallet.available.clone() + row.unlock_quantity.clone();
    let locked_after = wallet.locked.clone() - row.unlock_quantity.clone();
    let remaining_after = row.remaining_amount - row.unlock_quantity.clone();
    let lock_status = if remaining_after == 0 {
        "released"
    } else {
        "active"
    };

    let lock_update = sqlx::query(
        r#"UPDATE asset_lock_positions
           SET released_amount = released_amount + ?,
               remaining_amount = ?,
               status = ?
           WHERE id = ? AND remaining_amount >= ?"#,
    )
    .bind(&row.unlock_quantity)
    .bind(&remaining_after)
    .bind(lock_status)
    .bind(row.lock_position_id)
    .bind(&row.unlock_quantity)
    .execute(&mut *tx)
    .await?;
    if lock_update.rows_affected() != 1 {
        tx.rollback().await?;
        warn!(unlock_id = row.unlock_id, "解禁扫描跳过过期锁仓更新");
        return Ok(UnlockReleaseOutcome::Skipped);
    }

    let unlock_update = sqlx::query(
        "UPDATE asset_unlock_records SET status = 'released' WHERE id = ? AND status = 'pending'",
    )
    .bind(row.unlock_id)
    .execute(&mut *tx)
    .await?;
    if unlock_update.rows_affected() != 1 {
        tx.rollback().await?;
        warn!(unlock_id = row.unlock_id, "解禁扫描跳过过期解禁记录更新");
        return Ok(UnlockReleaseOutcome::Skipped);
    }

    let wallet_update = sqlx::query(
        "UPDATE wallet_accounts SET available = ?, locked = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&available_after)
    .bind(&locked_after)
    .bind(row.user_id)
    .bind(row.asset_id)
    .execute(&mut *tx)
    .await?;
    if wallet_update.rows_affected() != 1 {
        tx.rollback().await?;
        warn!(unlock_id = row.unlock_id, "解禁扫描跳过缺失钱包更新");
        return Ok(UnlockReleaseOutcome::Skipped);
    }

    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, 'new_coin_unlock_release', ?, 'locked', ?, ?, ?, ?, 'new_coin_unlock', ?),
                  (?, ?, 'new_coin_unlock_release', ?, 'available', ?, ?, ?, ?, 'new_coin_unlock', ?)"#,
    )
    .bind(row.user_id)
    .bind(row.asset_id)
    .bind(-row.unlock_quantity.clone())
    .bind(&locked_after)
    .bind(&available_after)
    .bind(&wallet.frozen)
    .bind(&locked_after)
    .bind(&row.idempotency_key)
    .bind(row.user_id)
    .bind(row.asset_id)
    .bind(&row.unlock_quantity)
    .bind(&available_after)
    .bind(&available_after)
    .bind(&wallet.frozen)
    .bind(&locked_after)
    .bind(&row.idempotency_key)
    .execute(&mut *tx)
    .await?;

    let event = UnlockReleaseEvent {
        user_id: row.user_id,
        unlock_id: row.idempotency_key,
        lock_position_id: row.lock_position_id,
        asset_id: row.asset_id,
        released_amount: row.unlock_quantity,
    };
    tx.commit().await?;
    Ok(UnlockReleaseOutcome::Released(event))
}

fn publish_unlock_release_event(hub: Option<&EventBroadcastHub>, event: &UnlockReleaseEvent) {
    if let Some(hub) = hub {
        hub.publish(EventBroadcastMessage::private_user(
            event.user_id,
            json!({
                "type": "new_coin.unlock.released",
                "unlock_id": event.unlock_id,
                "unlock_idempotency_key": event.unlock_id,
                "lock_position_id": event.lock_position_id,
                "asset_id": event.asset_id,
                "released_amount": event.released_amount,
                "unlock_quantity": event.released_amount,
                "released": true,
                "status": "released",
            })
            .to_string(),
        ));
    }
}

fn requires_fee_payment(unlock_fee_enabled: bool, fee_paid_status: &str) -> bool {
    unlock_fee_enabled && !matches!(fee_paid_status, "paid" | "not_required")
}

fn unlock_scan_limit(limit: u32) -> u32 {
    limit.clamp(1, 100)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn position(
        id: &str,
        unlock_at: chrono::DateTime<Utc>,
        status: LockPositionStatus,
    ) -> UnlockScanPosition {
        UnlockScanPosition {
            id: id.to_owned(),
            unlock_at,
            status,
        }
    }

    #[test]
    fn due_unlock_positions_include_active_positions_at_or_before_now() {
        let now = Utc.with_ymd_and_hms(2026, 5, 26, 10, 0, 0).unwrap();
        let positions = vec![
            position(
                "past-active",
                now - chrono::TimeDelta::seconds(1),
                LockPositionStatus::Active,
            ),
            position("now-active", now, LockPositionStatus::Active),
            position(
                "future-active",
                now + chrono::TimeDelta::seconds(1),
                LockPositionStatus::Active,
            ),
            position(
                "past-released",
                now - chrono::TimeDelta::seconds(1),
                LockPositionStatus::Released,
            ),
        ];

        let due = due_unlock_positions(&positions, now);

        assert_eq!(
            due.iter()
                .map(|position| position.id.as_str())
                .collect::<Vec<_>>(),
            vec!["past-active", "now-active"]
        );
    }

    #[test]
    fn unlock_scan_limit_is_clamped() {
        assert_eq!(unlock_scan_limit(0), 1);
        assert_eq!(unlock_scan_limit(50), 50);
        assert_eq!(unlock_scan_limit(500), 100);
    }
}
