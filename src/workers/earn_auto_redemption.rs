use crate::{
    error::{AppError, AppResult},
    modules::events::{EventBroadcastHub, EventBroadcastMessage},
    state::AppState,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::{MySql, Pool, Transaction};
use std::env;
use tokio::time::{Duration, interval};
use tracing::{error, info, warn};

pub struct EarnAutoRedemptionWorker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EarnAutoRedemptionWorkerConfig {
    pub enabled: bool,
    pub interval_seconds: u64,
    pub batch_limit: u32,
}

impl EarnAutoRedemptionWorkerConfig {
    pub fn from_env() -> Self {
        Self {
            enabled: env_bool("EARN_AUTO_REDEMPTION_ENABLED", true),
            interval_seconds: env_u64("EARN_AUTO_REDEMPTION_INTERVAL_SECONDS", 60),
            batch_limit: env_u32("EARN_AUTO_REDEMPTION_BATCH_LIMIT", 100),
        }
    }
}

impl EarnAutoRedemptionWorker {
    pub async fn run_once(
        &self,
        state: &AppState,
        now: DateTime<Utc>,
        limit: u32,
    ) -> AppResult<EarnAutoRedemptionSummary> {
        run_once(state, now, limit).await
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EarnAutoRedemptionSummary {
    pub scanned: u32,
    pub redeemed: u32,
    pub skipped: u32,
    pub failed: u32,
}

#[derive(Debug, sqlx::FromRow)]
struct DueEarnSubscription {
    subscription_id: u64,
}

#[derive(Debug, sqlx::FromRow)]
struct LockedEarnSubscription {
    id: u64,
    user_id: u64,
    asset_id: u64,
    product_id: u64,
    amount: BigDecimal,
    apr_rate: BigDecimal,
    term_days: u32,
    status: String,
    matures_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct WalletBalanceRow {
    available: BigDecimal,
    frozen: BigDecimal,
    locked: BigDecimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarnRedemptionEvent {
    user_id: u64,
    subscription_id: u64,
    product_id: u64,
    asset_id: u64,
    principal_amount: BigDecimal,
    yield_amount: BigDecimal,
    redeem_amount: BigDecimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum EarnRedemptionOutcome {
    Redeemed(EarnRedemptionEvent),
    Skipped,
}

pub async fn run_once(
    state: &AppState,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<EarnAutoRedemptionSummary> {
    let pool = state.mysql.as_ref().ok_or_else(|| {
        AppError::Internal("mysql pool is required for earn auto redemption".to_owned())
    })?;
    run_once_with_broadcast(pool, state.event_broadcast_hub.as_ref(), now, limit).await
}

pub async fn run_once_with_dependencies(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<EarnAutoRedemptionSummary> {
    run_once_with_broadcast(pool, None, now, limit).await
}

pub async fn run_once_with_broadcast(
    pool: &Pool<MySql>,
    hub: Option<&EventBroadcastHub>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<EarnAutoRedemptionSummary> {
    let redemption_limit = earn_auto_redemption_limit(limit);
    let candidates =
        fetch_due_subscriptions(pool, now, earn_auto_redemption_scan_limit(limit)).await?;
    let mut summary = EarnAutoRedemptionSummary::default();

    for candidate in candidates {
        if summary.redeemed >= redemption_limit {
            break;
        }
        summary.scanned += 1;
        match redeem_subscription_by_id(pool, candidate.subscription_id, now).await {
            Ok(EarnRedemptionOutcome::Redeemed(event)) => {
                summary.redeemed += 1;
                publish_redemption_event(hub, &event);
            }
            Ok(EarnRedemptionOutcome::Skipped) => summary.skipped += 1,
            Err(error) => {
                summary.failed += 1;
                warn!(subscription_id = candidate.subscription_id, %error, "理财自动赎回失败");
            }
        }
    }

    Ok(summary)
}

pub async fn run_loop(state: AppState, interval_seconds: u64, limit: u32) -> AppResult<()> {
    let mut ticker = interval(Duration::from_secs(interval_seconds.max(1)));

    loop {
        ticker.tick().await;
        match run_once(&state, Utc::now(), limit).await {
            Ok(summary) => info!(
                scanned = summary.scanned,
                redeemed = summary.redeemed,
                skipped = summary.skipped,
                failed = summary.failed,
                "理财自动赎回周期完成"
            ),
            Err(error) => error!(%error, "理财自动赎回周期失败"),
        }
    }
}

async fn fetch_due_subscriptions(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<Vec<DueEarnSubscription>> {
    sqlx::query_as::<_, DueEarnSubscription>(
        r#"SELECT id AS subscription_id
           FROM earn_subscriptions
           WHERE status = 'subscribed'
             AND matures_at <= ?
           ORDER BY matures_at ASC, id ASC
           LIMIT ?"#,
    )
    .bind(now.naive_utc())
    .bind(limit.clamp(1, 500) as i64)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

async fn redeem_subscription_by_id(
    pool: &Pool<MySql>,
    subscription_id: u64,
    now: DateTime<Utc>,
) -> AppResult<EarnRedemptionOutcome> {
    let mut tx = pool.begin().await?;
    let Some(subscription) = lock_subscription_by_id(&mut tx, subscription_id).await? else {
        tx.rollback().await?;
        return Ok(EarnRedemptionOutcome::Skipped);
    };

    // 只处理已经到期且仍处于 subscribed 的理财订单。
    if subscription.status != "subscribed" || subscription.matures_at > now {
        tx.rollback().await?;
        return Ok(EarnRedemptionOutcome::Skipped);
    }

    let principal_amount = subscription.amount.clone();
    let yield_amount = earn_redeem_yield_amount(&subscription);
    let redeem_amount = principal_amount.clone() + yield_amount.clone();
    let wallet = lock_wallet_row(&mut tx, subscription.user_id, subscription.asset_id).await?;
    let available_after = wallet.available.clone() + redeem_amount.clone();

    // 先更新钱包，再写流水，最后标记订单已赎回，保证资金和状态在同一事务内完成。
    let wallet_update =
        sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
            .bind(&available_after)
            .bind(subscription.user_id)
            .bind(subscription.asset_id)
            .execute(&mut *tx)
            .await?;
    if wallet_update.rows_affected() != 1 {
        tx.rollback().await?;
        return Ok(EarnRedemptionOutcome::Skipped);
    }

    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, 'earn_redeem', ?, 'available', ?, ?, ?, ?, 'earn_subscription', ?)"#,
    )
    .bind(subscription.user_id)
    .bind(subscription.asset_id)
    .bind(&redeem_amount)
    .bind(&available_after)
    .bind(&available_after)
    .bind(&wallet.frozen)
    .bind(&wallet.locked)
    .bind(subscription.id.to_string())
    .execute(&mut *tx)
    .await?;

    let subscription_update = sqlx::query(
        "UPDATE earn_subscriptions SET status = 'redeemed', redeemed_at = ? WHERE id = ? AND status = 'subscribed'",
    )
    .bind(now.naive_utc())
    .bind(subscription.id)
    .execute(&mut *tx)
    .await?;
    if subscription_update.rows_affected() != 1 {
        tx.rollback().await?;
        return Ok(EarnRedemptionOutcome::Skipped);
    }

    let event = EarnRedemptionEvent {
        user_id: subscription.user_id,
        subscription_id: subscription.id,
        product_id: subscription.product_id,
        asset_id: subscription.asset_id,
        principal_amount,
        yield_amount,
        redeem_amount,
    };
    tx.commit().await?;
    Ok(EarnRedemptionOutcome::Redeemed(event))
}

fn publish_redemption_event(hub: Option<&EventBroadcastHub>, event: &EarnRedemptionEvent) {
    if let Some(hub) = hub {
        hub.publish(EventBroadcastMessage::private_user(
            event.user_id,
            json!({
                "type": "earn.subscription.redeemed",
                "subscription_id": event.subscription_id,
                "product_id": event.product_id,
                "asset_id": event.asset_id,
                "principal_amount": event.principal_amount,
                "yield_amount": event.yield_amount,
                "redeem_amount": event.redeem_amount,
                "status": "redeemed",
            })
            .to_string(),
        ));
    }
}

async fn lock_subscription_by_id(
    tx: &mut Transaction<'_, MySql>,
    subscription_id: u64,
) -> AppResult<Option<LockedEarnSubscription>> {
    sqlx::query_as::<_, LockedEarnSubscription>(
        r#"SELECT id, user_id, asset_id, product_id, amount, apr_rate, term_days, status, matures_at
           FROM earn_subscriptions
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(subscription_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

async fn lock_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<WalletBalanceRow> {
    sqlx::query_as::<_, WalletBalanceRow>(
        r#"SELECT available, frozen, locked
           FROM wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| {
        AppError::Validation("wallet account is required for earn auto redemption".to_owned())
    })
}

fn earn_redeem_yield_amount(subscription: &LockedEarnSubscription) -> BigDecimal {
    let yearly_yield = subscription.amount.clone() * subscription.apr_rate.clone();
    (yearly_yield * BigDecimal::from(subscription.term_days) / BigDecimal::from(365)).with_scale(18)
}

fn earn_auto_redemption_limit(limit: u32) -> u32 {
    limit.clamp(1, 100)
}

fn earn_auto_redemption_scan_limit(limit: u32) -> u32 {
    earn_auto_redemption_limit(limit)
        .saturating_mul(10)
        .clamp(1, 500)
}

fn env_bool(key: &str, default: bool) -> bool {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<bool>().ok())
        .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

fn env_u32(key: &str, default: u32) -> u32 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn earn_auto_redemption_limit_is_clamped() {
        assert_eq!(earn_auto_redemption_limit(0), 1);
        assert_eq!(earn_auto_redemption_limit(50), 50);
        assert_eq!(earn_auto_redemption_limit(500), 100);
    }

    #[test]
    fn earn_auto_redemption_scan_limit_scans_past_bad_rows() {
        assert_eq!(earn_auto_redemption_scan_limit(0), 10);
        assert_eq!(earn_auto_redemption_scan_limit(1), 10);
        assert_eq!(earn_auto_redemption_scan_limit(50), 500);
        assert_eq!(earn_auto_redemption_scan_limit(500), 500);
    }
}
