use crate::{
    error::{AppError, AppResult},
    infra::secrets::decrypt_optional_secret,
    modules::wallet::{
        application::{normalize_deposit_network, observe_deposit},
        infrastructure::{
            HttpWalletChainGateway, confirm_withdrawal_in_tx,
            load_withdrawal_by_gateway_request_for_update, mark_withdrawal_broadcasted_in_tx,
            mark_withdrawal_manual_review_in_tx, release_withdrawal_in_tx,
            update_withdrawal_chain_progress_in_tx,
        },
        presentation::ObserveDepositRequest,
        repository::{
            WalletChainBroadcastCommand, WalletChainGateway, WalletChainWithdrawalObservation,
        },
    },
    state::AppState,
};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool};
use std::{env, str::FromStr};
use tokio::time::{Duration, interval};
use tracing::{error, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalletChainWorkerConfig {
    pub enabled: bool,
    pub interval_seconds: u64,
    pub batch_limit: u32,
    pub max_attempts: u32,
}

impl WalletChainWorkerConfig {
    pub fn from_env() -> Self {
        Self {
            enabled: env_bool("WALLET_CHAIN_WORKER_ENABLED", true),
            interval_seconds: env_u64("WALLET_CHAIN_WORKER_INTERVAL_SECONDS", 5),
            batch_limit: env_u32("WALLET_CHAIN_WORKER_BATCH_LIMIT", 50).clamp(1, 200),
            max_attempts: env_u32("WALLET_CHAIN_WORKER_MAX_ATTEMPTS", 5).clamp(1, 20),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WalletChainWorkerSummary {
    pub withdrawal_scanned: u32,
    pub withdrawal_broadcasted: u32,
    pub withdrawal_retried: u32,
    pub withdrawal_failed: u32,
    pub withdrawal_confirmed: u32,
    pub withdrawal_manual_review: u32,
    pub deposit_observed: u32,
    pub gateway_failed: u32,
}

#[derive(Debug, sqlx::FromRow)]
struct WithdrawalCandidate {
    id: u64,
    gateway_request_id: String,
    network: String,
    asset_symbol: String,
    address: String,
    amount: BigDecimal,
    fee: BigDecimal,
    retry_count: u32,
    broadcast_url: String,
    auth_token_encrypted: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct ChainGatewayConfig {
    id: u64,
    network: String,
    event_poll_url: String,
    auth_token_encrypted: Option<String>,
    last_deposit_cursor: Option<String>,
}

pub async fn run_once(
    state: &AppState,
    config: WalletChainWorkerConfig,
) -> AppResult<WalletChainWorkerSummary> {
    let pool = state.mysql.as_ref().ok_or_else(|| {
        AppError::Internal("mysql pool is required for wallet chain worker".to_owned())
    })?;
    let gateway = HttpWalletChainGateway::default();
    run_once_with_gateway(
        pool,
        state.settings.exposed_credential_encryption_key(),
        &gateway,
        config,
    )
    .await
}

pub async fn run_once_with_gateway(
    pool: &Pool<MySql>,
    encryption_key: Option<&str>,
    gateway: &dyn WalletChainGateway,
    config: WalletChainWorkerConfig,
) -> AppResult<WalletChainWorkerSummary> {
    let mut summary = WalletChainWorkerSummary::default();
    let candidates = load_withdrawal_candidates(pool, config.batch_limit).await?;
    for candidate in candidates {
        summary.withdrawal_scanned += 1;
        if !claim_withdrawal(pool, candidate.id).await? {
            continue;
        }
        let bearer_token =
            decrypt_gateway_token(candidate.auth_token_encrypted.as_deref(), encryption_key);
        let command = WalletChainBroadcastCommand {
            request_id: candidate.gateway_request_id,
            network: candidate.network,
            asset_symbol: candidate.asset_symbol,
            address: candidate.address,
            amount: candidate.amount.to_string(),
            fee: candidate.fee.to_string(),
        };
        let broadcast = match bearer_token {
            Ok(token) => {
                gateway
                    .broadcast_withdrawal(&candidate.broadcast_url, token.as_deref(), &command)
                    .await
            }
            Err(error) => Err(error),
        };
        match broadcast {
            Ok(result) => {
                let tx_hash = normalize_gateway_identifier(&result.tx_hash, "tx_hash", 255)?;
                let mut tx = pool.begin().await?;
                mark_withdrawal_broadcasted_in_tx(
                    &mut tx,
                    candidate.id,
                    None,
                    &tx_hash,
                    result.block_height,
                    result.confirmations,
                )
                .await?;
                tx.commit().await?;
                summary.withdrawal_broadcasted += 1;
            }
            Err(error) => {
                let attempts = candidate.retry_count.saturating_add(1);
                if attempts >= config.max_attempts {
                    let mut tx = pool.begin().await?;
                    let reason = bounded_failure_reason(&format!(
                        "wallet gateway broadcast failed: {error}"
                    ));
                    release_withdrawal_in_tx(&mut tx, candidate.id, None, "failed", &reason)
                        .await?;
                    tx.commit().await?;
                    summary.withdrawal_failed += 1;
                } else {
                    schedule_withdrawal_retry(pool, candidate.id, &error.to_string(), attempts)
                        .await?;
                    summary.withdrawal_retried += 1;
                }
                warn!(withdrawal_id = candidate.id, %error, "提现链上广播失败");
            }
        }
    }

    let chain_gateways = load_chain_event_gateways(pool).await?;
    for chain_gateway in chain_gateways {
        let bearer_token = match decrypt_gateway_token(
            chain_gateway.auth_token_encrypted.as_deref(),
            encryption_key,
        ) {
            Ok(token) => token,
            Err(error) => {
                summary.gateway_failed += 1;
                warn!(network = %chain_gateway.network, %error, "链事件网关凭据解密失败");
                continue;
            }
        };
        let page = match gateway
            .poll_chain_events(
                &chain_gateway.event_poll_url,
                bearer_token.as_deref(),
                chain_gateway.last_deposit_cursor.as_deref(),
                config.batch_limit,
            )
            .await
        {
            Ok(page) => page,
            Err(error) => {
                summary.gateway_failed += 1;
                warn!(network = %chain_gateway.network, %error, "链事件轮询失败");
                continue;
            }
        };
        let expected_network = normalize_deposit_network(&chain_gateway.network)?;
        let mut page_failed = false;
        for observation in page.withdrawals {
            if let Err(error) =
                process_withdrawal_observation(pool, &expected_network, observation, &mut summary)
                    .await
            {
                page_failed = true;
                summary.gateway_failed += 1;
                warn!(network = %expected_network, %error, "链上提现回执处理失败");
                break;
            }
        }
        if page_failed {
            continue;
        }
        for observation in page.deposits {
            let observed_network = normalize_deposit_network(&observation.network)?;
            if observed_network != expected_network {
                page_failed = true;
                summary.gateway_failed += 1;
                warn!(
                    configured_network = %expected_network,
                    observed_network = %observed_network,
                    tx_hash = %observation.tx_hash,
                    "链事件网络与网关配置不一致"
                );
                break;
            }
            let amount = BigDecimal::from_str(&observation.amount).map_err(|_| {
                AppError::Validation("wallet gateway deposit amount is invalid".to_owned())
            })?;
            if let Err(error) = observe_deposit(
                pool,
                ObserveDepositRequest {
                    asset_symbol: observation.asset_symbol,
                    network: observed_network,
                    address: observation.address,
                    memo: observation.memo,
                    tx_hash: observation.tx_hash,
                    event_index: observation.event_index,
                    amount,
                    block_height: observation.block_height,
                    confirmations: observation.confirmations,
                },
            )
            .await
            {
                page_failed = true;
                summary.gateway_failed += 1;
                warn!(network = %expected_network, %error, "链上充值事件处理失败");
                break;
            }
            summary.deposit_observed += 1;
        }
        if !page_failed && let Some(cursor) = page.next_cursor {
            update_gateway_cursor(pool, chain_gateway.id, &cursor).await?;
        }
    }
    Ok(summary)
}

pub async fn run_loop(state: AppState, config: WalletChainWorkerConfig) -> AppResult<()> {
    let mut ticker = interval(Duration::from_secs(config.interval_seconds.max(1)));
    loop {
        ticker.tick().await;
        match run_once(&state, config).await {
            Ok(summary) => info!(
                withdrawal_scanned = summary.withdrawal_scanned,
                withdrawal_broadcasted = summary.withdrawal_broadcasted,
                withdrawal_retried = summary.withdrawal_retried,
                withdrawal_failed = summary.withdrawal_failed,
                withdrawal_confirmed = summary.withdrawal_confirmed,
                withdrawal_manual_review = summary.withdrawal_manual_review,
                deposit_observed = summary.deposit_observed,
                gateway_failed = summary.gateway_failed,
                "钱包链任务周期完成"
            ),
            Err(error) => error!(%error, "钱包链任务周期失败"),
        }
    }
}

async fn process_withdrawal_observation(
    pool: &Pool<MySql>,
    expected_network: &str,
    observation: WalletChainWithdrawalObservation,
    summary: &mut WalletChainWorkerSummary,
) -> AppResult<()> {
    let observed_network = normalize_deposit_network(&observation.network)?;
    if observed_network != expected_network {
        return Err(AppError::Validation(
            "withdrawal event network does not match gateway".to_owned(),
        ));
    }
    let receipt_status = normalize_withdrawal_receipt_status(&observation.status)?;
    let request_id = normalize_gateway_identifier(&observation.request_id, "request_id", 128)?;
    let tx_hash = observation
        .tx_hash
        .as_deref()
        .map(|value| normalize_gateway_identifier(value, "tx_hash", 255))
        .transpose()?;
    let mut tx = pool.begin().await?;
    let mut withdrawal =
        load_withdrawal_by_gateway_request_for_update(&mut tx, &request_id).await?;
    let withdrawal_network = normalize_deposit_network(
        withdrawal
            .network
            .as_deref()
            .ok_or_else(|| AppError::Conflict("withdrawal network is missing".to_owned()))?,
    )?;
    if withdrawal_network != expected_network {
        return Err(AppError::Conflict(
            "withdrawal does not belong to the configured gateway".to_owned(),
        ));
    }

    match receipt_status {
        WithdrawalReceiptStatus::Broadcasted | WithdrawalReceiptStatus::Confirmed => {
            let tx_hash = tx_hash.ok_or_else(|| {
                AppError::Validation("withdrawal receipt tx_hash is required".to_owned())
            })?;
            if matches!(withdrawal.status.as_str(), "approved" | "broadcasting") {
                withdrawal = mark_withdrawal_broadcasted_in_tx(
                    &mut tx,
                    withdrawal.id,
                    None,
                    &tx_hash,
                    observation.block_height,
                    observation.confirmations,
                )
                .await?;
            } else {
                withdrawal = update_withdrawal_chain_progress_in_tx(
                    &mut tx,
                    withdrawal.id,
                    &tx_hash,
                    observation.block_height,
                    observation.confirmations,
                )
                .await?;
            }
            if receipt_status == WithdrawalReceiptStatus::Confirmed
                && withdrawal.status != "confirmed"
            {
                confirm_withdrawal_in_tx(
                    &mut tx,
                    withdrawal.id,
                    None,
                    observation.block_height,
                    observation.confirmations.max(1),
                )
                .await?;
                summary.withdrawal_confirmed += 1;
            }
        }
        WithdrawalReceiptStatus::Failed => {
            let reason = bounded_failure_reason(
                observation
                    .failure_reason
                    .as_deref()
                    .unwrap_or("wallet gateway reported terminal failure"),
            );
            if matches!(withdrawal.status.as_str(), "approved" | "broadcasting") {
                release_withdrawal_in_tx(&mut tx, withdrawal.id, None, "failed", &reason).await?;
                summary.withdrawal_failed += 1;
            } else if withdrawal.status == "broadcasted" {
                mark_withdrawal_manual_review_in_tx(&mut tx, withdrawal.id, &reason).await?;
                summary.withdrawal_manual_review += 1;
            } else if !matches!(
                withdrawal.status.as_str(),
                "failed" | "manual_review" | "confirmed"
            ) {
                return Err(AppError::Conflict(format!(
                    "withdrawal failure receipt cannot update status {}",
                    withdrawal.status
                )));
            }
        }
    }
    tx.commit().await?;
    Ok(())
}

async fn load_withdrawal_candidates(
    pool: &Pool<MySql>,
    limit: u32,
) -> AppResult<Vec<WithdrawalCandidate>> {
    sqlx::query_as::<_, WithdrawalCandidate>(
        r#"SELECT requests.id, requests.gateway_request_id, requests.network,
                  requests.asset_symbol, requests.address, requests.amount, requests.fee,
                  requests.retry_count, gateways.broadcast_url,
                  gateways.auth_token_encrypted
           FROM wallet_withdrawal_requests requests
           INNER JOIN wallet_chain_gateways gateways
                   ON gateways.network = requests.network
                  AND gateways.status = 'active'
                  AND gateways.broadcast_url IS NOT NULL
           WHERE requests.status IN ('approved', 'broadcasting')
             AND (requests.next_attempt_at IS NULL OR requests.next_attempt_at <= CURRENT_TIMESTAMP(6))
           ORDER BY requests.id ASC
           LIMIT ?"#,
    )
    .bind(limit.clamp(1, 200) as i64)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

async fn claim_withdrawal(pool: &Pool<MySql>, withdrawal_id: u64) -> AppResult<bool> {
    let result = sqlx::query(
        r#"UPDATE wallet_withdrawal_requests
           SET status = 'broadcasting', broadcasting_at = CURRENT_TIMESTAMP(6),
               retry_count = retry_count + 1,
               next_attempt_at = DATE_ADD(CURRENT_TIMESTAMP(6), INTERVAL 30 SECOND)
           WHERE id = ?
             AND status IN ('approved', 'broadcasting')
             AND (next_attempt_at IS NULL OR next_attempt_at <= CURRENT_TIMESTAMP(6))"#,
    )
    .bind(withdrawal_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

async fn schedule_withdrawal_retry(
    pool: &Pool<MySql>,
    withdrawal_id: u64,
    reason: &str,
    attempts: u32,
) -> AppResult<()> {
    let result = sqlx::query(
        r#"UPDATE wallet_withdrawal_requests
           SET status = 'approved', failure_reason = ?,
               next_attempt_at = DATE_ADD(CURRENT_TIMESTAMP(6), INTERVAL ? SECOND)
           WHERE id = ? AND status = 'broadcasting'"#,
    )
    .bind(bounded_failure_reason(reason))
    .bind(retry_backoff_seconds(attempts) as i64)
    .bind(withdrawal_id)
    .execute(pool)
    .await?;
    if result.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "withdrawal retry state changed concurrently".to_owned(),
        ));
    }
    Ok(())
}

async fn load_chain_event_gateways(pool: &Pool<MySql>) -> AppResult<Vec<ChainGatewayConfig>> {
    sqlx::query_as::<_, ChainGatewayConfig>(
        r#"SELECT id, network, event_poll_url, auth_token_encrypted, last_deposit_cursor
           FROM wallet_chain_gateways
           WHERE status = 'active' AND event_poll_url IS NOT NULL
           ORDER BY id ASC"#,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

async fn update_gateway_cursor(pool: &Pool<MySql>, gateway_id: u64, cursor: &str) -> AppResult<()> {
    if cursor.len() > 500 {
        return Err(AppError::Validation(
            "wallet gateway cursor exceeds 500 characters".to_owned(),
        ));
    }
    sqlx::query("UPDATE wallet_chain_gateways SET last_deposit_cursor = ? WHERE id = ?")
        .bind(cursor)
        .bind(gateway_id)
        .execute(pool)
        .await?;
    Ok(())
}

fn decrypt_gateway_token(
    ciphertext: Option<&str>,
    encryption_key: Option<&str>,
) -> AppResult<Option<String>> {
    let Some(ciphertext) = ciphertext else {
        return Ok(None);
    };
    let key = encryption_key.ok_or_else(|| {
        AppError::Config(config::ConfigError::Message(
            "credential_encryption_key is required for wallet gateway token".to_owned(),
        ))
    })?;
    decrypt_optional_secret(Some(ciphertext), key)
}

fn retry_backoff_seconds(attempts: u32) -> u64 {
    5_u64.saturating_mul(2_u64.saturating_pow(attempts.saturating_sub(1).min(6)))
}

fn bounded_failure_reason(reason: &str) -> String {
    reason.chars().take(500).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WithdrawalReceiptStatus {
    Broadcasted,
    Confirmed,
    Failed,
}

fn normalize_withdrawal_receipt_status(status: &str) -> AppResult<WithdrawalReceiptStatus> {
    match status.trim().to_ascii_lowercase().as_str() {
        "broadcasted" => Ok(WithdrawalReceiptStatus::Broadcasted),
        "confirmed" => Ok(WithdrawalReceiptStatus::Confirmed),
        "failed" => Ok(WithdrawalReceiptStatus::Failed),
        _ => Err(AppError::Validation(
            "wallet gateway withdrawal status is invalid".to_owned(),
        )),
    }
}

fn normalize_gateway_identifier(value: &str, label: &str, max_length: usize) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > max_length || value.chars().any(char::is_whitespace) {
        return Err(AppError::Validation(format!(
            "wallet gateway {label} format is invalid"
        )));
    }
    Ok(value.to_owned())
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
#[path = "../../tests/unit_src/src_workers_wallet_chain_tests.rs"]
mod tests;
