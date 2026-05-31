use crate::{
    error::{AppError, AppResult},
    modules::{
        events::{EventBroadcastHub, EventBroadcastMessage},
        market::market_ticker_redis_key,
    },
    state::AppState,
    time::unix_millis,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use redis::{AsyncCommands, aio::ConnectionManager};
use serde::Deserialize;
use serde_json::json;
use sqlx::{MySql, Pool, Transaction};
use std::env;
use tokio::time::{Duration, interval};
use tracing::{error, info, warn};

pub struct SecondsContractSettlementWorker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecondsContractSettlementWorkerConfig {
    pub enabled: bool,
    pub interval_seconds: u64,
    pub batch_limit: u32,
}

impl SecondsContractSettlementWorkerConfig {
    pub fn from_env() -> Self {
        Self {
            enabled: env_bool("SECONDS_CONTRACT_SETTLEMENT_ENABLED", true),
            interval_seconds: env_u64("SECONDS_CONTRACT_SETTLEMENT_INTERVAL_SECONDS", 5),
            batch_limit: env_u32("SECONDS_CONTRACT_SETTLEMENT_BATCH_LIMIT", 100),
        }
    }
}

impl SecondsContractSettlementWorker {
    pub async fn run_once(
        &self,
        state: &AppState,
        now: DateTime<Utc>,
        limit: u32,
    ) -> AppResult<SecondsContractSettlementSummary> {
        run_once(state, now, limit).await
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SecondsContractSettlementSummary {
    pub scanned: u32,
    pub settled: u32,
    pub skipped: u32,
    pub failed: u32,
}

#[derive(Debug, sqlx::FromRow)]
struct DueSecondsContractOrder {
    order_id: u64,
    symbol: String,
    direction: String,
    entry_price: Option<BigDecimal>,
}

#[derive(Debug, sqlx::FromRow)]
struct LockedSecondsContractOrder {
    id: u64,
    user_id: u64,
    product_id: u64,
    pair_id: u64,
    stake_asset: u64,
    direction: String,
    stake_amount: BigDecimal,
    payout_rate: BigDecimal,
    status: String,
    result: Option<String>,
    entry_price: Option<BigDecimal>,
}

#[derive(Debug, sqlx::FromRow)]
struct WalletBalanceRow {
    available: BigDecimal,
    frozen: BigDecimal,
    locked: BigDecimal,
}

#[derive(Debug, Deserialize)]
struct CachedTickerPayload {
    last_price: BigDecimal,
    #[serde(with = "unix_millis")]
    observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecondsContractSettlementEvent {
    user_id: u64,
    order_id: u64,
    product_id: u64,
    pair_id: u64,
    stake_asset: u64,
    direction: String,
    stake_amount: BigDecimal,
    payout_amount: BigDecimal,
    result: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SettlementOutcome {
    Settled(SecondsContractSettlementEvent),
    Skipped,
}

pub fn seconds_contract_settlement_result(
    direction: &str,
    entry_price: &BigDecimal,
    exit_price: &BigDecimal,
) -> AppResult<&'static str> {
    match direction {
        "up" if exit_price > entry_price => Ok("win"),
        "up" => Ok("loss"),
        "down" if exit_price < entry_price => Ok("win"),
        "down" => Ok("loss"),
        _ => Err(AppError::Validation(
            "seconds contract direction must be up or down".to_owned(),
        )),
    }
}

pub async fn run_once(
    state: &AppState,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<SecondsContractSettlementSummary> {
    let pool = state.mysql.as_ref().ok_or_else(|| {
        AppError::Internal("mysql pool is required for seconds contract settlement".to_owned())
    })?;
    let redis = state.redis.as_ref().ok_or_else(|| {
        AppError::Internal(
            "redis connection is required for seconds contract settlement".to_owned(),
        )
    })?;
    run_once_with_broadcast(pool, redis, state.event_broadcast_hub.as_ref(), now, limit).await
}

pub async fn run_once_with_dependencies(
    pool: &Pool<MySql>,
    redis: &ConnectionManager,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<SecondsContractSettlementSummary> {
    run_once_with_broadcast(pool, redis, None, now, limit).await
}

pub async fn run_once_with_broadcast(
    pool: &Pool<MySql>,
    redis: &ConnectionManager,
    hub: Option<&EventBroadcastHub>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<SecondsContractSettlementSummary> {
    let settlement_limit = seconds_contract_settlement_limit(limit);
    let rows = fetch_due_orders(pool, now, seconds_contract_settlement_scan_limit(limit)).await?;
    let mut summary = SecondsContractSettlementSummary::default();

    for row in rows {
        if summary.settled >= settlement_limit {
            break;
        }
        summary.scanned += 1;
        let Some(entry_price) = row.entry_price.as_ref() else {
            summary.failed += 1;
            reschedule_settlement_attempt(pool, row.order_id, now).await?;
            warn!(order_id = row.order_id, "秒合约结算跳过缺失开仓价订单");
            continue;
        };
        let exit_price = match cached_ticker_price(redis, &row.symbol, now).await {
            Ok(Some(price)) => price,
            Ok(None) => {
                summary.skipped += 1;
                reschedule_settlement_attempt(pool, row.order_id, now).await?;
                warn!(order_id = row.order_id, symbol = %row.symbol, "秒合约结算跳过缺失行情订单");
                continue;
            }
            Err(error) => {
                summary.failed += 1;
                reschedule_settlement_attempt(pool, row.order_id, now).await?;
                warn!(order_id = row.order_id, symbol = %row.symbol, %error, "秒合约结算读取行情失败");
                continue;
            }
        };
        let result =
            match seconds_contract_settlement_result(&row.direction, entry_price, &exit_price) {
                Ok(result) => result,
                Err(error) => {
                    summary.failed += 1;
                    reschedule_settlement_attempt(pool, row.order_id, now).await?;
                    warn!(order_id = row.order_id, %error, "秒合约结算结果计算失败");
                    continue;
                }
            };
        match settle_order_by_id(pool, row.order_id, result).await {
            Ok(SettlementOutcome::Settled(event)) => {
                summary.settled += 1;
                publish_settlement_event(hub, &event);
            }
            Ok(SettlementOutcome::Skipped) => summary.skipped += 1,
            Err(error) => {
                summary.failed += 1;
                reschedule_settlement_attempt(pool, row.order_id, now).await?;
                warn!(order_id = row.order_id, %error, "秒合约结算失败");
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
                settled = summary.settled,
                skipped = summary.skipped,
                failed = summary.failed,
                "秒合约结算周期完成"
            ),
            Err(error) => error!(%error, "秒合约结算周期失败"),
        }
    }
}

async fn fetch_due_orders(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<Vec<DueSecondsContractOrder>> {
    sqlx::query_as::<_, DueSecondsContractOrder>(
        r#"SELECT orders.id AS order_id,
                  pairs.symbol,
                  orders.direction,
                  orders.entry_price
           FROM seconds_contract_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           WHERE orders.status = 'opened'
             AND orders.expires_at <= ?
             AND (orders.next_settlement_attempt_at IS NULL OR orders.next_settlement_attempt_at <= ?)
           ORDER BY orders.expires_at ASC, orders.id ASC
           LIMIT ?"#,
    )
    .bind(now.naive_utc())
    .bind(now.naive_utc())
    .bind(limit.clamp(1, 500) as i64)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

async fn reschedule_settlement_attempt(
    pool: &Pool<MySql>,
    order_id: u64,
    now: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE seconds_contract_orders SET next_settlement_attempt_at = ? WHERE id = ? AND status = 'opened'",
    )
    .bind((now + chrono::TimeDelta::seconds(60)).naive_utc())
    .bind(order_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn cached_ticker_price(
    redis: &ConnectionManager,
    symbol: &str,
    now: DateTime<Utc>,
) -> AppResult<Option<BigDecimal>> {
    let mut connection = redis.clone();
    let payload: Option<String> = connection.get(market_ticker_redis_key(symbol)).await?;
    let Some(payload) = payload else {
        return Ok(None);
    };
    let ticker = serde_json::from_str::<CachedTickerPayload>(&payload).map_err(|error| {
        AppError::Internal(format!(
            "invalid cached seconds contract ticker payload: {error}"
        ))
    })?;
    if ticker.last_price <= 0 {
        return Err(AppError::Validation(
            "seconds contract exit price must be positive".to_owned(),
        ));
    }
    if ticker.observed_at < now - chrono::TimeDelta::seconds(60) {
        return Err(AppError::Validation(
            "seconds contract ticker is stale".to_owned(),
        ));
    }
    Ok(Some(ticker.last_price))
}

async fn settle_order_by_id(
    pool: &Pool<MySql>,
    order_id: u64,
    result: &str,
) -> AppResult<SettlementOutcome> {
    let mut tx = pool.begin().await?;
    let Some(order) = lock_order_by_id(&mut tx, order_id).await? else {
        tx.rollback().await?;
        return Ok(SettlementOutcome::Skipped);
    };
    if order.status == "settled" {
        if order.result.as_deref() != Some(result) {
            return Err(AppError::Conflict(
                "seconds contract order was settled with a different result".to_owned(),
            ));
        }
        tx.commit().await?;
        return Ok(SettlementOutcome::Skipped);
    }
    if order.status != "opened" {
        tx.rollback().await?;
        return Ok(SettlementOutcome::Skipped);
    }
    if order.entry_price.is_none() {
        return Err(AppError::Validation(
            "seconds contract entry price is required for settlement".to_owned(),
        ));
    }

    let payout_amount = settlement_payout_amount(&order, result);
    if payout_amount > 0 {
        let wallet = lock_wallet_row(&mut tx, order.user_id, order.stake_asset).await?;
        let available_after = wallet.available.clone() + payout_amount.clone();
        let wallet_update = sqlx::query(
            "UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?",
        )
        .bind(&available_after)
        .bind(order.user_id)
        .bind(order.stake_asset)
        .execute(&mut *tx)
        .await?;
        if wallet_update.rows_affected() != 1 {
            tx.rollback().await?;
            return Ok(SettlementOutcome::Skipped);
        }
        sqlx::query(
            r#"INSERT INTO wallet_ledger
               (user_id, asset_id, change_type, amount, balance_type, balance_after,
                available_after, frozen_after, locked_after, ref_type, ref_id)
               VALUES (?, ?, 'seconds_contract_settle_win', ?, 'available', ?, ?, ?, ?, 'seconds_contract_order', ?)"#,
        )
        .bind(order.user_id)
        .bind(order.stake_asset)
        .bind(&payout_amount)
        .bind(&available_after)
        .bind(&available_after)
        .bind(&wallet.frozen)
        .bind(&wallet.locked)
        .bind(order.id.to_string())
        .execute(&mut *tx)
        .await?;
    }

    let update = sqlx::query(
        "UPDATE seconds_contract_orders SET status = 'settled', result = ?, settled_at = CURRENT_TIMESTAMP(6) WHERE id = ? AND status = 'opened'",
    )
    .bind(result)
    .bind(order.id)
    .execute(&mut *tx)
    .await?;
    if update.rows_affected() != 1 {
        tx.rollback().await?;
        return Ok(SettlementOutcome::Skipped);
    }

    let event = SecondsContractSettlementEvent {
        user_id: order.user_id,
        order_id: order.id,
        product_id: order.product_id,
        pair_id: order.pair_id,
        stake_asset: order.stake_asset,
        direction: order.direction,
        stake_amount: order.stake_amount,
        payout_amount,
        result: result.to_owned(),
    };
    tx.commit().await?;
    Ok(SettlementOutcome::Settled(event))
}

fn publish_settlement_event(
    hub: Option<&EventBroadcastHub>,
    event: &SecondsContractSettlementEvent,
) {
    if let Some(hub) = hub {
        hub.publish(EventBroadcastMessage::private_user(
            event.user_id,
            json!({
                "type": "seconds_contract.order.settled",
                "order_id": event.order_id,
                "product_id": event.product_id,
                "pair_id": event.pair_id,
                "stake_asset": event.stake_asset,
                "direction": event.direction,
                "stake_amount": event.stake_amount,
                "payout_amount": event.payout_amount,
                "result": event.result,
                "status": "settled",
            })
            .to_string(),
        ));
    }
}

async fn lock_order_by_id(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
) -> AppResult<Option<LockedSecondsContractOrder>> {
    sqlx::query_as::<_, LockedSecondsContractOrder>(
        r#"SELECT id, user_id, product_id, pair_id, stake_asset, direction,
                  stake_amount, payout_rate, status, result, entry_price
           FROM seconds_contract_orders
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(order_id)
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
        AppError::Validation(
            "wallet account is required for seconds contract settlement".to_owned(),
        )
    })
}

fn settlement_payout_amount(order: &LockedSecondsContractOrder, result: &str) -> BigDecimal {
    if result == "win" {
        order.stake_amount.clone() + order.stake_amount.clone() * order.payout_rate.clone()
    } else {
        BigDecimal::from(0)
    }
}

fn seconds_contract_settlement_limit(limit: u32) -> u32 {
    limit.clamp(1, 100)
}

fn seconds_contract_settlement_scan_limit(limit: u32) -> u32 {
    seconds_contract_settlement_limit(limit).clamp(1, 100)
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
    use std::str::FromStr;

    fn decimal(value: &str) -> BigDecimal {
        BigDecimal::from_str(value).unwrap()
    }

    #[test]
    fn settlement_result_treats_equal_price_as_loss() {
        assert_eq!(
            seconds_contract_settlement_result("up", &decimal("1"), &decimal("1")).unwrap(),
            "loss"
        );
        assert_eq!(
            seconds_contract_settlement_result("down", &decimal("1"), &decimal("1")).unwrap(),
            "loss"
        );
    }

    #[test]
    fn seconds_contract_settlement_limit_is_clamped() {
        assert_eq!(seconds_contract_settlement_limit(0), 1);
        assert_eq!(seconds_contract_settlement_limit(50), 50);
        assert_eq!(seconds_contract_settlement_limit(500), 100);
    }

    #[test]
    fn seconds_contract_settlement_scan_limit_matches_settlement_limit() {
        assert_eq!(seconds_contract_settlement_scan_limit(0), 1);
        assert_eq!(seconds_contract_settlement_scan_limit(1), 1);
        assert_eq!(seconds_contract_settlement_scan_limit(50), 50);
        assert_eq!(seconds_contract_settlement_scan_limit(100), 100);
        assert_eq!(seconds_contract_settlement_scan_limit(500), 100);
    }
}
