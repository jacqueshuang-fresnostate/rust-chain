//! 钱包链上出入金后台任务。
//!
//! 每轮先认领到期的已批准提现并调用链网关广播，对广播中/结果不明的请求只用稳定 request id 查询，再按每个启用网关的持久游标轮询链事件。
//! 广播与对账认领都用带条件的原子更新实现互斥，配合下次尝试时刻形成三十秒可见性窗口，多实例并发下同一申请只会被一个进程处理。
//! 只有确定性拒绝、明确的受理前失败或权威未受理查询才能释放冻结；超时、断连、5xx和无效响应始终保留 frozen，预算耗尽后转人工复核。
//! 链事件按错误性质分流：基础设施类错误停在当前页保留旧游标等待下轮，确定性拒绝写入死信后越过，避免毒性事件卡死游标。
//! 只有整页处理完毕才推进网关游标，因此事件至少处理一次，重复由链事件唯一键与提现状态机各自幂等消化。
//! 网关凭据只在调用前解密且不落日志；本任务不直接改写余额，所有资金变更都委托钱包上下文的事务入口完成。

use crate::{
    error::{AppError, AppResult},
    infra::secrets::decrypt_optional_secret,
    modules::wallet::{
        application::{normalize_deposit_network, observe_deposit},
        infrastructure::{
            HttpWalletChainGateway, NewWalletChainEventDeadLetter, confirm_withdrawal_in_tx,
            insert_wallet_chain_event_dead_letter, insert_withdrawal_broadcast_audit_in_tx,
            load_withdrawal_by_gateway_request_for_update,
            mark_withdrawal_acceptance_evidence_for_manual_review_in_tx,
            mark_withdrawal_broadcasted_in_tx, mark_withdrawal_manual_review_in_tx,
            mark_withdrawal_unknown_broadcast_in_tx, release_authoritatively_not_accepted_in_tx,
            schedule_withdrawal_after_not_accepted_in_tx, update_withdrawal_chain_progress_in_tx,
        },
        presentation::ObserveDepositRequest,
        repository::{
            WalletChainBroadcastCommand, WalletChainDepositObservation, WalletChainGateway,
            WalletChainGatewayError, WalletChainGatewayErrorClass,
            WalletChainWithdrawalObservation, WalletChainWithdrawalQueryResult,
            WalletChainWithdrawalQueryStatus,
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
    /// 读取钱包链 worker 环境配置；默认启用、周期 5 秒、批量 50、最多 5 次，批量硬限 1..=200、尝试次数 1..=20。
    /// 缺失或不可解析值使用默认值，防止异常配置关闭重试边界或放大链网关压力。
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
    pub event_dead_lettered: u32,
}

#[derive(Debug, sqlx::FromRow)]
struct WithdrawalCandidate {
    id: u64,
    status: String,
    gateway_request_id: String,
    network: String,
    asset_symbol: String,
    address: String,
    amount: BigDecimal,
    fee: BigDecimal,
    retry_count: u32,
    gateway_query_count: u32,
    broadcast_url: Option<String>,
    withdrawal_status_url: Option<String>,
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

/// 使用生产 HTTP 链网关执行一轮：先处理有限提现广播，再按每个启用网关的持久游标轮询提现回执与充值事件。
/// 网关 token 仅在调用前用配置密钥解密，密钥/明文不落日志；数据库或加密配置缺失在外部请求前失败，状态与事件副作用委托核心入口。
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

/// 钱包链 worker 单轮核心：先认领并广播待处理提现，再按网关游标轮询提现回执和充值事件。
/// 候选提现按申请编号升序取一批，逐个用条件更新认领；认领失败说明已被其他实例抢走，直接跳过不计入处理。
/// 已批准请求的认领会把状态改为广播中、广播尝试次数加一并推后可见时刻；崩溃后该状态只进入查询对账，绝不盲目二次广播。
/// 广播成功即在独立事务中登记交易哈希与确认进度，此步不核销冻结，资金仍留在 frozen 等待确认。
/// 广播错误按类别决策：确定拒绝可当场释放，受理前可重试错误按预算重排/释放，结果不明则只查询并在预算耗尽后保持冻结转人工。
/// 链事件阶段逐个网关处理，凭据解密失败或轮询失败只记录并跳过该网关，其游标保持不变。
/// 提现回执与充值事件分两段处理，回执段一旦命中可重试错误立即停页且跳过本网关剩余充值事件，避免游标越过未处理数据。
/// 确定性拒绝的事件写入死信后继续处理同页后续事件；只有整页无停页时才推进游标，因此事件语义是至少一次。
/// 冻结释放、确认扣减和充值入账全部由钱包上下文的事务入口完成，本函数不直接改写任何余额或流水。
/// 重复回执依赖网关请求编号定位申请、充值依赖链事件唯一键去重；已有链上哈希的请求不得自动解冻，需确认或人工复核。
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
        if candidate.status == "approved" {
            if !claim_withdrawal_for_broadcast(pool, candidate.id).await? {
                continue;
            }
            process_broadcast_candidate(
                pool,
                encryption_key,
                gateway,
                &candidate,
                config,
                &mut summary,
            )
            .await?;
        } else {
            if !claim_withdrawal_for_reconciliation(pool, candidate.id).await? {
                continue;
            }
            process_reconciliation_candidate(
                pool,
                encryption_key,
                gateway,
                &candidate,
                config,
                &mut summary,
            )
            .await?;
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
        for observation in &page.withdrawals {
            if let Err(error) =
                process_withdrawal_observation(pool, &expected_network, observation, &mut summary)
                    .await
            {
                // 基础设施类错误停页重试以免丢事件；确定性拒绝进死信并越过，避免毒性事件卡死游标。
                if is_transient_chain_event_error(&error) {
                    page_failed = true;
                    summary.gateway_failed += 1;
                    warn!(
                        network = %expected_network,
                        request_id = %observation.request_id,
                        %error,
                        "链上提现回执处理失败，停页等待重试"
                    );
                    break;
                }
                dead_letter_withdrawal_observation(pool, &chain_gateway, observation, &error)
                    .await?;
                summary.event_dead_lettered += 1;
            }
        }
        if page_failed {
            continue;
        }
        for observation in &page.deposits {
            match process_deposit_observation(pool, &expected_network, observation).await {
                Ok(()) => summary.deposit_observed += 1,
                Err(error) if is_transient_chain_event_error(&error) => {
                    page_failed = true;
                    summary.gateway_failed += 1;
                    warn!(
                        network = %expected_network,
                        tx_hash = %observation.tx_hash,
                        event_index = observation.event_index,
                        %error,
                        "链上充值事件处理失败，停页等待重试"
                    );
                    break;
                }
                Err(error) => {
                    dead_letter_deposit_observation(pool, &chain_gateway, observation, &error)
                        .await?;
                    summary.event_dead_lettered += 1;
                }
            }
        }
        if !page_failed && let Some(cursor) = page.next_cursor {
            update_gateway_cursor(pool, chain_gateway.id, &cursor).await?;
        }
    }
    Ok(summary)
}

async fn process_broadcast_candidate(
    pool: &Pool<MySql>,
    encryption_key: Option<&str>,
    gateway: &dyn WalletChainGateway,
    candidate: &WithdrawalCandidate,
    config: WalletChainWorkerConfig,
    summary: &mut WalletChainWorkerSummary,
) -> AppResult<()> {
    let attempts = candidate.retry_count.saturating_add(1);
    let command = WalletChainBroadcastCommand {
        request_id: candidate.gateway_request_id.clone(),
        network: candidate.network.clone(),
        asset_symbol: candidate.asset_symbol.clone(),
        address: candidate.address.clone(),
        amount: candidate.amount.to_string(),
        fee: candidate.fee.to_string(),
    };
    let broadcast = match (
        candidate.broadcast_url.as_deref(),
        decrypt_gateway_token(candidate.auth_token_encrypted.as_deref(), encryption_key),
    ) {
        (Some(endpoint), Ok(token)) => {
            gateway
                .broadcast_withdrawal(endpoint, token.as_deref(), &command)
                .await
        }
        (None, _) => Err(WalletChainGatewayError::new(
            WalletChainGatewayErrorClass::RetryableBeforeAcceptance,
            "wallet gateway broadcast endpoint is not configured",
        )),
        (_, Err(error)) => Err(WalletChainGatewayError::new(
            WalletChainGatewayErrorClass::RetryableBeforeAcceptance,
            format!("wallet gateway credentials are unavailable: {error}"),
        )),
    };

    let event_key = format!("broadcast:{attempts}");
    let mut tx = pool.begin().await?;
    match broadcast {
        Ok(result) => {
            let tx_hash = match normalize_gateway_identifier(&result.tx_hash, "tx_hash", 255) {
                Ok(tx_hash) => tx_hash,
                Err(error) => {
                    let reason = bounded_failure_reason(&format!(
                        "wallet gateway broadcast response is invalid: {error}"
                    ));
                    insert_withdrawal_broadcast_audit_in_tx(
                        &mut tx,
                        candidate.id,
                        &candidate.gateway_request_id,
                        &event_key,
                        "broadcast",
                        "unknown",
                        None,
                        Some(&reason),
                    )
                    .await?;
                    // HTTP 2xx 且已解析为“广播成功”结构本身就是受理证据；
                    // 即使哈希格式损坏，也只能铆住证据转人工，不得遗忘后再被退冻。
                    mark_withdrawal_acceptance_evidence_for_manual_review_in_tx(
                        &mut tx,
                        candidate.id,
                        &reason,
                        None,
                        result.block_height,
                        result.confirmations,
                    )
                    .await?;
                    summary.withdrawal_manual_review += 1;
                    tx.commit().await?;
                    return Ok(());
                }
            };
            mark_withdrawal_broadcasted_in_tx(
                &mut tx,
                candidate.id,
                None,
                &tx_hash,
                result.block_height,
                result.confirmations,
            )
            .await?;
            insert_withdrawal_broadcast_audit_in_tx(
                &mut tx,
                candidate.id,
                &candidate.gateway_request_id,
                &event_key,
                "broadcast",
                "accepted",
                Some(&tx_hash),
                None,
            )
            .await?;
            summary.withdrawal_broadcasted += 1;
        }
        Err(error) => {
            let reason = bounded_failure_reason(&format!(
                "wallet gateway broadcast failed: {}",
                error.message
            ));
            insert_withdrawal_broadcast_audit_in_tx(
                &mut tx,
                candidate.id,
                &candidate.gateway_request_id,
                &event_key,
                "broadcast",
                error.class.as_str(),
                None,
                Some(&reason),
            )
            .await?;
            match error.class {
                WalletChainGatewayErrorClass::DeterministicRejected => {
                    release_authoritatively_not_accepted_in_tx(&mut tx, candidate.id, &reason)
                        .await?;
                    summary.withdrawal_failed += 1;
                }
                WalletChainGatewayErrorClass::RetryableBeforeAcceptance
                    if attempts >= config.max_attempts =>
                {
                    release_authoritatively_not_accepted_in_tx(&mut tx, candidate.id, &reason)
                        .await?;
                    summary.withdrawal_failed += 1;
                }
                WalletChainGatewayErrorClass::RetryableBeforeAcceptance => {
                    schedule_withdrawal_after_not_accepted_in_tx(
                        &mut tx,
                        candidate.id,
                        &reason,
                        retry_backoff_seconds(attempts),
                    )
                    .await?;
                    summary.withdrawal_retried += 1;
                }
                WalletChainGatewayErrorClass::Unknown => {
                    let manual_review = attempts >= config.max_attempts;
                    mark_withdrawal_unknown_broadcast_in_tx(
                        &mut tx,
                        candidate.id,
                        error.class.as_str(),
                        &reason,
                        retry_backoff_seconds(attempts),
                        manual_review,
                    )
                    .await?;
                    if manual_review {
                        summary.withdrawal_manual_review += 1;
                    } else {
                        summary.withdrawal_retried += 1;
                    }
                }
            }
            warn!(
                withdrawal_id = candidate.id,
                error_class = error.class.as_str(),
                error = %error,
                "提现链上广播未成功"
            );
        }
    }
    tx.commit().await?;
    Ok(())
}

async fn process_reconciliation_candidate(
    pool: &Pool<MySql>,
    encryption_key: Option<&str>,
    gateway: &dyn WalletChainGateway,
    candidate: &WithdrawalCandidate,
    config: WalletChainWorkerConfig,
    summary: &mut WalletChainWorkerSummary,
) -> AppResult<()> {
    let query_attempt = candidate.gateway_query_count.saturating_add(1);
    let event_key = format!("query:{query_attempt}");
    let query = match (
        candidate.withdrawal_status_url.as_deref(),
        decrypt_gateway_token(candidate.auth_token_encrypted.as_deref(), encryption_key),
    ) {
        (Some(endpoint), Ok(token)) => {
            gateway
                .query_withdrawal(endpoint, token.as_deref(), &candidate.gateway_request_id)
                .await
        }
        (None, _) => Err(WalletChainGatewayError::new(
            WalletChainGatewayErrorClass::Unknown,
            "wallet gateway status endpoint is not configured",
        )),
        (_, Err(error)) => Err(WalletChainGatewayError::new(
            WalletChainGatewayErrorClass::Unknown,
            format!("wallet gateway credentials are unavailable: {error}"),
        )),
    };

    let mut tx = pool.begin().await?;
    match query {
        Err(error) => {
            let reason = bounded_failure_reason(&format!(
                "wallet gateway status query failed: {}",
                error.message
            ));
            insert_withdrawal_broadcast_audit_in_tx(
                &mut tx,
                candidate.id,
                &candidate.gateway_request_id,
                &event_key,
                "query",
                "unknown",
                None,
                Some(&reason),
            )
            .await?;
            let manual_review =
                query_attempt >= config.max_attempts || candidate.withdrawal_status_url.is_none();
            mark_withdrawal_unknown_broadcast_in_tx(
                &mut tx,
                candidate.id,
                "unknown",
                &reason,
                retry_backoff_seconds(query_attempt),
                manual_review,
            )
            .await?;
            if manual_review {
                summary.withdrawal_manual_review += 1;
            } else {
                summary.withdrawal_retried += 1;
            }
        }
        Ok(result) => {
            let explicitly_not_accepted = matches!(
                result.status,
                WalletChainWithdrawalQueryStatus::NotAccepted
                    | WalletChainWithdrawalQueryStatus::Rejected
            );
            if explicitly_not_accepted && query_result_has_acceptance_evidence(&result) {
                // “未受理”与交易哈希/区块/确认数同时出现是自相矛盾的远端响应，不能作为退冻证据。
                let reason = bounded_failure_reason(
                    "wallet gateway reported non-acceptance together with chain acceptance evidence",
                );
                let audit_tx_hash = result
                    .tx_hash
                    .as_deref()
                    .and_then(|value| normalize_gateway_identifier(value, "tx_hash", 255).ok());
                insert_withdrawal_broadcast_audit_in_tx(
                    &mut tx,
                    candidate.id,
                    &candidate.gateway_request_id,
                    &event_key,
                    "query",
                    "unknown",
                    audit_tx_hash.as_deref(),
                    Some(&reason),
                )
                .await?;
                mark_withdrawal_acceptance_evidence_for_manual_review_in_tx(
                    &mut tx,
                    candidate.id,
                    &reason,
                    audit_tx_hash.as_deref(),
                    result.block_height,
                    result.confirmations,
                )
                .await?;
                summary.withdrawal_manual_review += 1;
            } else if explicitly_not_accepted {
                let reason = bounded_failure_reason(
                    result
                        .failure_reason
                        .as_deref()
                        .unwrap_or("wallet gateway authoritatively reported not accepted"),
                );
                insert_withdrawal_broadcast_audit_in_tx(
                    &mut tx,
                    candidate.id,
                    &candidate.gateway_request_id,
                    &event_key,
                    "query",
                    "authoritative_not_accepted",
                    None,
                    Some(&reason),
                )
                .await?;
                release_authoritatively_not_accepted_in_tx(&mut tx, candidate.id, &reason).await?;
                summary.withdrawal_failed += 1;
            } else if let Some(raw_tx_hash) = result.tx_hash.as_deref() {
                let tx_hash = match normalize_gateway_identifier(raw_tx_hash, "tx_hash", 255) {
                    Ok(tx_hash) => tx_hash,
                    Err(error) => {
                        let reason = bounded_failure_reason(&format!(
                            "wallet gateway status response is invalid: {error}"
                        ));
                        insert_withdrawal_broadcast_audit_in_tx(
                            &mut tx,
                            candidate.id,
                            &candidate.gateway_request_id,
                            &event_key,
                            "query",
                            "unknown",
                            None,
                            Some(&reason),
                        )
                        .await?;
                        // 查询结果已经携带 tx_hash 字段，即使值格式损坏也不能把这份
                        // 受理证据降级成可释放的普通 manual_review。
                        mark_withdrawal_acceptance_evidence_for_manual_review_in_tx(
                            &mut tx,
                            candidate.id,
                            &reason,
                            None,
                            result.block_height,
                            result.confirmations,
                        )
                        .await?;
                        summary.withdrawal_manual_review += 1;
                        tx.commit().await?;
                        return Ok(());
                    }
                };
                mark_withdrawal_broadcasted_in_tx(
                    &mut tx,
                    candidate.id,
                    None,
                    &tx_hash,
                    result.block_height,
                    result.confirmations,
                )
                .await?;
                insert_withdrawal_broadcast_audit_in_tx(
                    &mut tx,
                    candidate.id,
                    &candidate.gateway_request_id,
                    &event_key,
                    "query",
                    "accepted",
                    Some(&tx_hash),
                    None,
                )
                .await?;
                summary.withdrawal_broadcasted += 1;
                if result.status == WalletChainWithdrawalQueryStatus::Confirmed {
                    confirm_withdrawal_in_tx(
                        &mut tx,
                        candidate.id,
                        None,
                        result.block_height,
                        result.confirmations.max(1),
                    )
                    .await?;
                    summary.withdrawal_confirmed += 1;
                }
            } else if result.block_height.is_some() || result.confirmations > 0 {
                let reason = bounded_failure_reason(
                    "wallet gateway reported chain progress without a transaction hash",
                );
                insert_withdrawal_broadcast_audit_in_tx(
                    &mut tx,
                    candidate.id,
                    &candidate.gateway_request_id,
                    &event_key,
                    "query",
                    "unknown",
                    None,
                    Some(&reason),
                )
                .await?;
                mark_withdrawal_acceptance_evidence_for_manual_review_in_tx(
                    &mut tx,
                    candidate.id,
                    &reason,
                    None,
                    result.block_height,
                    result.confirmations,
                )
                .await?;
                summary.withdrawal_manual_review += 1;
            } else {
                let status_claims_acceptance = matches!(
                    result.status,
                    WalletChainWithdrawalQueryStatus::Accepted
                        | WalletChainWithdrawalQueryStatus::Broadcasted
                        | WalletChainWithdrawalQueryStatus::Confirmed
                );
                let reason = if status_claims_acceptance {
                    "wallet gateway reported acceptance without a transaction hash"
                } else {
                    "wallet gateway status remains pending or unknown"
                };
                insert_withdrawal_broadcast_audit_in_tx(
                    &mut tx,
                    candidate.id,
                    &candidate.gateway_request_id,
                    &event_key,
                    "query",
                    "unknown",
                    None,
                    Some(reason),
                )
                .await?;
                if status_claims_acceptance {
                    // accepted/broadcasted/confirmed 状态本身就是远端受理声明。
                    // 缺失 tx_hash 不能让后到的 not_accepted 回执自动退冻。
                    mark_withdrawal_acceptance_evidence_for_manual_review_in_tx(
                        &mut tx,
                        candidate.id,
                        reason,
                        None,
                        None,
                        0,
                    )
                    .await?;
                    summary.withdrawal_manual_review += 1;
                } else {
                    let manual_review = query_attempt >= config.max_attempts;
                    mark_withdrawal_unknown_broadcast_in_tx(
                        &mut tx,
                        candidate.id,
                        "unknown",
                        reason,
                        retry_backoff_seconds(query_attempt),
                        manual_review,
                    )
                    .await?;
                    if manual_review {
                        summary.withdrawal_manual_review += 1;
                    } else {
                        summary.withdrawal_retried += 1;
                    }
                }
            }
        }
    }
    tx.commit().await?;
    Ok(())
}

/// 以配置周期至少 1 秒持续运行钱包链任务；周期级数据库、解密或网关错误只记录并进入下一轮，单个链事件按核心入口的可重试/死信规则隔离。
/// 提现状态、next-attempt、链事件死信与网关游标承担跨重启恢复；循环不缓存密钥，也不越过未完整处理的页面推进游标。
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
                event_dead_lettered = summary.event_dead_lettered,
                "钱包链任务周期完成"
            ),
            Err(error) => error!(%error, "钱包链任务周期失败"),
        }
    }
}

/// 在事务中核对网关网络、请求 ID、交易哈希与回执状态，再推进提现链上进度、确认扣款或人工复核。
/// 先归一回执网络并与网关配置网络比对，再归一回执状态与请求编号，交易哈希可缺省但一旦提供必须通过格式校验。
/// 随后按网关请求编号对申请加排他锁，并二次核对申请自身的网络确实属于该网关，防止跨网关串改他人申请。
/// 已广播与已确认两类回执都要求带交易哈希：申请仍在已批准或广播中则登记广播，否则只单调推进区块高度与确认数。
/// 已确认回执在进度更新后若申请尚未确认，再调用确认入口从 frozen 永久扣除预留额并写确认流水。
/// 只有回执明确标记 not_accepted/rejected 才调用权威未受理释放入口；通用 failed 表示结果仍有歧义，一律转人工复核并保留冻结。
/// 若申请已处于失败、人工审核或已确认则视为终态静默接受，其余状态返回冲突交由上层按确定性错误进死信。
/// 全部状态迁移共用同一事务，任一步失败整体回滚；重复回执只提高确认数，不会重复扣减 frozen。
async fn process_withdrawal_observation(
    pool: &Pool<MySql>,
    expected_network: &str,
    observation: &WalletChainWithdrawalObservation,
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
    let tx_hash = match observation
        .tx_hash
        .as_deref()
        .map(|value| normalize_gateway_identifier(value, "tx_hash", 255))
        .transpose()
    {
        Ok(tx_hash) => tx_hash,
        Err(error) => {
            // 回执携带了交易哈希字段，但值格式损坏。这不是可以忽略的纯解析错误：
            // 它至少表示远端声称存在链上交易，必须持久化证据闸门并保留冻结。
            let reason = bounded_failure_reason(&format!(
                "wallet gateway withdrawal receipt contains an invalid transaction hash: {error}"
            ));
            if matches!(
                withdrawal.status.as_str(),
                "approved"
                    | "broadcasting"
                    | "unknown_broadcast"
                    | "broadcasted"
                    | "manual_review"
                    | "confirmed"
            ) {
                let was_manual_or_confirmed =
                    matches!(withdrawal.status.as_str(), "manual_review" | "confirmed");
                mark_withdrawal_acceptance_evidence_for_manual_review_in_tx(
                    &mut tx,
                    withdrawal.id,
                    &reason,
                    None,
                    observation.block_height,
                    observation.confirmations,
                )
                .await?;
                insert_withdrawal_broadcast_audit_in_tx(
                    &mut tx,
                    withdrawal.id,
                    &request_id,
                    &format!(
                        "receipt:{}:invalid_tx_hash",
                        observation.status.trim().to_ascii_lowercase()
                    ),
                    "receipt",
                    "unknown",
                    None,
                    Some(&reason),
                )
                .await?;
                if !was_manual_or_confirmed {
                    summary.withdrawal_manual_review += 1;
                }
                tx.commit().await?;
                return Ok(());
            }
            return Err(error);
        }
    };

    match receipt_status {
        WithdrawalReceiptStatus::Broadcasted | WithdrawalReceiptStatus::Confirmed => {
            let tx_hash = tx_hash.ok_or_else(|| {
                AppError::Validation("withdrawal receipt tx_hash is required".to_owned())
            })?;
            if matches!(
                withdrawal.status.as_str(),
                "approved" | "broadcasting" | "unknown_broadcast" | "manual_review"
            ) {
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
            insert_withdrawal_broadcast_audit_in_tx(
                &mut tx,
                withdrawal.id,
                &request_id,
                &format!(
                    "receipt:{}:{}:{}",
                    observation.status.to_ascii_lowercase(),
                    tx_hash,
                    observation.confirmations
                ),
                "receipt",
                if receipt_status == WithdrawalReceiptStatus::Confirmed {
                    "confirmed"
                } else {
                    "accepted"
                },
                Some(&tx_hash),
                None,
            )
            .await?;
        }
        WithdrawalReceiptStatus::NotAccepted => {
            let acceptance_evidence = tx_hash.is_some()
                || observation.block_height.is_some()
                || observation.confirmations > 0
                || withdrawal.tx_hash.is_some()
                || withdrawal.acceptance_evidence_at.is_some()
                || matches!(withdrawal.status.as_str(), "broadcasted" | "confirmed");
            if acceptance_evidence {
                let reason = bounded_failure_reason(
                    "wallet gateway reported non-acceptance together with chain acceptance evidence",
                );
                let evidence_tx_hash = tx_hash.clone().or_else(|| withdrawal.tx_hash.clone());
                let was_manual_review = withdrawal.status == "manual_review";
                if matches!(
                    withdrawal.status.as_str(),
                    "approved"
                        | "broadcasting"
                        | "unknown_broadcast"
                        | "broadcasted"
                        | "manual_review"
                ) {
                    mark_withdrawal_acceptance_evidence_for_manual_review_in_tx(
                        &mut tx,
                        withdrawal.id,
                        &reason,
                        tx_hash.as_deref(),
                        observation.block_height,
                        observation.confirmations,
                    )
                    .await?;
                    if !was_manual_review {
                        summary.withdrawal_manual_review += 1;
                    }
                }
                insert_withdrawal_broadcast_audit_in_tx(
                    &mut tx,
                    withdrawal.id,
                    &request_id,
                    "receipt:not_accepted:contradictory",
                    "receipt",
                    "unknown",
                    evidence_tx_hash.as_deref(),
                    Some(&reason),
                )
                .await?;
            } else {
                let reason = bounded_failure_reason(
                    observation
                        .failure_reason
                        .as_deref()
                        .unwrap_or("wallet gateway authoritatively reported not accepted"),
                );
                release_authoritatively_not_accepted_in_tx(&mut tx, withdrawal.id, &reason).await?;
                insert_withdrawal_broadcast_audit_in_tx(
                    &mut tx,
                    withdrawal.id,
                    &request_id,
                    "receipt:not_accepted",
                    "receipt",
                    "authoritative_not_accepted",
                    None,
                    Some(&reason),
                )
                .await?;
                summary.withdrawal_failed += 1;
            }
        }
        WithdrawalReceiptStatus::Failed => {
            let reason =
                bounded_failure_reason(observation.failure_reason.as_deref().unwrap_or(
                    "wallet gateway reported terminal failure with uncertain acceptance",
                ));
            if matches!(
                withdrawal.status.as_str(),
                "approved" | "broadcasting" | "unknown_broadcast" | "broadcasted"
            ) {
                if tx_hash.is_some()
                    || observation.block_height.is_some()
                    || observation.confirmations > 0
                {
                    mark_withdrawal_acceptance_evidence_for_manual_review_in_tx(
                        &mut tx,
                        withdrawal.id,
                        &reason,
                        tx_hash.as_deref(),
                        observation.block_height,
                        observation.confirmations,
                    )
                    .await?;
                } else {
                    mark_withdrawal_manual_review_in_tx(&mut tx, withdrawal.id, &reason).await?;
                }
                insert_withdrawal_broadcast_audit_in_tx(
                    &mut tx,
                    withdrawal.id,
                    &request_id,
                    "receipt:failed",
                    "receipt",
                    "unknown",
                    tx_hash.as_deref(),
                    Some(&reason),
                )
                .await?;
                summary.withdrawal_manual_review += 1;
            } else if !matches!(withdrawal.status.as_str(), "manual_review" | "confirmed") {
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

/// 规范化链网关充值回执并调用钱包充值观察用例，把网关侧的字符串金额转成定点数后交给幂等入账。
/// 回执网络先归一再与网关配置网络比对，不一致返回冲突，避免把某条链的到账记到另一条链的地址上。
/// 金额解析失败返回校验错误，属于确定性拒绝，会由调用方写入死信而不是无限重试。
/// 链事件唯一键由网络、交易哈希和事件序号构成，重放去重与确认阈值判定都在钱包上下文内完成。
/// 入账事务由钱包上下文持有，本函数不加锁、不改余额，也不判断确认数是否达标。
async fn process_deposit_observation(
    pool: &Pool<MySql>,
    expected_network: &str,
    observation: &WalletChainDepositObservation,
) -> AppResult<()> {
    let observed_network = normalize_deposit_network(&observation.network)?;
    if observed_network != expected_network {
        return Err(AppError::Conflict(format!(
            "deposit event network {observed_network} does not match gateway {expected_network}"
        )));
    }
    let amount = BigDecimal::from_str(&observation.amount)
        .map_err(|_| AppError::Validation("wallet gateway deposit amount is invalid".to_owned()))?;
    observe_deposit(
        pool,
        ObserveDepositRequest {
            asset_symbol: observation.asset_symbol.clone(),
            network: observed_network,
            address: observation.address.clone(),
            memo: observation.memo.clone(),
            tx_hash: observation.tx_hash.clone(),
            event_index: observation.event_index,
            amount,
            block_height: observation.block_height,
            confirmations: observation.confirmations,
        },
    )
    .await?;
    Ok(())
}

/// 判定链事件处理错误是否属于可重试的基础设施故障，据此在停页重试与写入死信之间二选一。
/// 配置、数据库、Mongo、Redis、消息队列和内部错误视为暂时性，停页保留旧游标，下轮重新拉取同一批事件。
/// 校验、冲突、未找到等业务性错误视为确定性拒绝，重试不会改变结果，因此写入死信并越过以免卡死游标。
fn is_transient_chain_event_error(error: &AppError) -> bool {
    matches!(
        error,
        AppError::Config(_)
            | AppError::Database(_)
            | AppError::Mongo(_)
            | AppError::Redis(_)
            | AppError::RabbitMq(_)
            | AppError::Internal(_)
    )
}

/// 把确定性失败的提现回执归档为死信，保留网关原始字段与失败原因供人工重放。
/// 去重键由网关网络、请求编号与回执状态拼成并截断到五百一十二字符，因此同一请求的不同回执状态各占一条死信。
/// 请求编号与交易哈希分别按各自列宽截断后单独存列，便于按标识检索；充值特有的事件序号在此留空。
/// 死信写入使用连接池独立执行，不参与业务事务，因此归档不会因随后的业务回滚而丢失。
/// 归档成功后打印告警日志并返回成功，让调用方继续处理同页后续事件；归档本身失败才向上冒泡中断本轮。
async fn dead_letter_withdrawal_observation(
    pool: &Pool<MySql>,
    gateway: &ChainGatewayConfig,
    observation: &WalletChainWithdrawalObservation,
    error: &AppError,
) -> AppResult<()> {
    let payload = serde_json::json!({
        "request_id": observation.request_id,
        "network": observation.network,
        "tx_hash": observation.tx_hash,
        "block_height": observation.block_height,
        "confirmations": observation.confirmations,
        "status": observation.status,
        "failure_reason": observation.failure_reason,
    });
    insert_wallet_chain_event_dead_letter(
        pool,
        &NewWalletChainEventDeadLetter {
            gateway_id: gateway.id,
            network: &gateway.network,
            event_kind: "withdrawal",
            dedup_key: bounded_chain_value(
                &format!(
                    "withdrawal:{}:{}:{}",
                    gateway.network, observation.request_id, observation.status
                ),
                512,
            ),
            request_id: Some(bounded_chain_value(&observation.request_id, 128)),
            tx_hash: observation
                .tx_hash
                .as_deref()
                .map(|value| bounded_chain_value(value, 255)),
            event_index: None,
            payload_json: payload.to_string(),
            failure_reason: bounded_failure_reason(&error.to_string()),
        },
    )
    .await?;
    warn!(
        network = %gateway.network,
        request_id = %observation.request_id,
        tx_hash = observation.tx_hash.as_deref().unwrap_or(""),
        %error,
        "链上提现回执确定性失败，已记录死信并跳过"
    );
    Ok(())
}

/// 把确定性失败的充值事件归档为死信，完整保留资产、网络、地址、备注、金额与确认数等原始观测字段。
/// 去重键由网关网络、交易哈希与事件序号拼成并截断，与链上充值的幂等身份一致，同一事件反复失败只覆盖不新增。
/// 交易哈希与事件序号另存独立列以便检索，提现特有的请求编号在此留空。
/// 载荷按原样序列化归档，仅供人工核对与重放，不代表任何金额已经入账。
/// 归档成功后记录告警并返回成功，使调用方能越过该事件继续处理同页剩余充值。
async fn dead_letter_deposit_observation(
    pool: &Pool<MySql>,
    gateway: &ChainGatewayConfig,
    observation: &WalletChainDepositObservation,
    error: &AppError,
) -> AppResult<()> {
    let payload = serde_json::json!({
        "asset_symbol": observation.asset_symbol,
        "network": observation.network,
        "address": observation.address,
        "memo": observation.memo,
        "tx_hash": observation.tx_hash,
        "event_index": observation.event_index,
        "amount": observation.amount,
        "block_height": observation.block_height,
        "confirmations": observation.confirmations,
    });
    insert_wallet_chain_event_dead_letter(
        pool,
        &NewWalletChainEventDeadLetter {
            gateway_id: gateway.id,
            network: &gateway.network,
            event_kind: "deposit",
            dedup_key: bounded_chain_value(
                &format!(
                    "deposit:{}:{}:{}",
                    gateway.network, observation.tx_hash, observation.event_index
                ),
                512,
            ),
            request_id: None,
            tx_hash: Some(bounded_chain_value(&observation.tx_hash, 255)),
            event_index: Some(observation.event_index),
            payload_json: payload.to_string(),
            failure_reason: bounded_failure_reason(&error.to_string()),
        },
    )
    .await?;
    warn!(
        network = %gateway.network,
        tx_hash = %observation.tx_hash,
        event_index = observation.event_index,
        %error,
        "链上充值事件确定性失败，已记录死信并跳过"
    );
    Ok(())
}

/// 按字符数截断链上标识以适配死信表列宽，超长时直接丢弃尾部而不是拒绝归档。
/// 与提现路径的严格校验不同，这里刻意宽松：死信的首要目标是把坏数据落盘留痕，不能因格式问题再次失败。
fn bounded_chain_value(value: &str, max_length: usize) -> String {
    value.chars().take(max_length).collect()
}

/// 取出本轮可尝试广播的提现候选，同时联出所属链网关的广播地址与加密凭据。
/// 候选限定为已批准或广播中，且下次尝试时刻为空或已到期，因此退避中的申请不会被提前捞出。
/// 网关必须处于启用状态且配置了广播地址，未接入链网关的网络其申请不会进入自动广播流程。
/// 按申请编号升序取批并把条数钳制在一到二百之间，保证多实例扫描顺序一致且单轮压力可控。
/// 这里只做无锁读取，真正的互斥由随后的条件认领更新完成，因此多个实例可能读到同一批候选。
async fn load_withdrawal_candidates(
    pool: &Pool<MySql>,
    limit: u32,
) -> AppResult<Vec<WithdrawalCandidate>> {
    sqlx::query_as::<_, WithdrawalCandidate>(
        r#"SELECT requests.id, requests.status, requests.gateway_request_id, requests.network,
                  requests.asset_symbol, requests.address, requests.amount, requests.fee,
                  requests.retry_count, requests.gateway_query_count, gateways.broadcast_url,
                  gateways.withdrawal_status_url,
                  gateways.auth_token_encrypted
           FROM wallet_withdrawal_requests requests
           INNER JOIN wallet_chain_gateways gateways
                   ON gateways.network = requests.network
                  AND gateways.status = 'active'
           WHERE requests.status IN ('approved', 'broadcasting', 'unknown_broadcast')
             AND (requests.next_attempt_at IS NULL OR requests.next_attempt_at <= CURRENT_TIMESTAMP(6))
           ORDER BY requests.id ASC
           LIMIT ?"#,
    )
    .bind(limit.clamp(1, 200) as i64)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

/// 以单条带条件更新原子认领一笔待广播提现，返回是否认领成功，是多实例互斥的唯一依据。
/// 更新条件重复校验状态与下次尝试时刻，因此读取候选到实际认领之间的竞态会体现为受影响行数为零。
/// 认领同时把状态置为广播中、尝试次数加一并把下次可见时刻推后三十秒，形成崩溃后可自动重试的可见性窗口。
/// 尝试次数在此自增而非广播失败时才加，意味着进程在广播过程中崩溃同样会消耗一次尝试额度。
/// 该更新不开显式事务，单语句自身即为原子操作，也不触碰任何余额或流水。
async fn claim_withdrawal_for_broadcast(pool: &Pool<MySql>, withdrawal_id: u64) -> AppResult<bool> {
    let result = sqlx::query(
        r#"UPDATE wallet_withdrawal_requests
           SET status = 'broadcasting', broadcasting_at = CURRENT_TIMESTAMP(6),
               retry_count = retry_count + 1,
               next_attempt_at = DATE_ADD(CURRENT_TIMESTAMP(6), INTERVAL 30 SECOND)
           WHERE id = ?
             AND status = 'approved'
             AND (next_attempt_at IS NULL OR next_attempt_at <= CURRENT_TIMESTAMP(6))"#,
    )
    .bind(withdrawal_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// 认领广播结果待对账的请求。此路径只增加查询计数，绝不增加广播次数也不重发请求。
async fn claim_withdrawal_for_reconciliation(
    pool: &Pool<MySql>,
    withdrawal_id: u64,
) -> AppResult<bool> {
    let result = sqlx::query(
        r#"UPDATE wallet_withdrawal_requests
           SET status = 'unknown_broadcast', gateway_query_count = gateway_query_count + 1,
               last_gateway_query_at = CURRENT_TIMESTAMP(6),
               next_attempt_at = DATE_ADD(CURRENT_TIMESTAMP(6), INTERVAL 30 SECOND)
           WHERE id = ?
             AND status IN ('broadcasting', 'unknown_broadcast')
             AND tx_hash IS NULL
             AND (next_attempt_at IS NULL OR next_attempt_at <= CURRENT_TIMESTAMP(6))"#,
    )
    .bind(withdrawal_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// 广播失败但未达最大尝试次数时把提现退回已批准，并按累计尝试次数计算退避时长排入下次尝试。
/// 失败原因截断后写入申请，仅作为可观测信息，不改变状态机含义，也不影响后续能否重试。
/// 更新以状态仍为广播中为条件，受影响行数不为一即判定并发抢先并返回冲突，避免覆盖他人已推进的状态。
/// 该操作只调整调度字段，冻结资金原封不动留在 frozen，既不释放也不核销。
/// 读取所有启用且配置了事件轮询地址的链网关，连同加密凭据与上次充值游标一起取出。
/// 按主键升序返回，使多实例的网关处理顺序一致，便于对照日志排查某个网络的停页原因。
/// 未配置轮询地址的网关被排除在外，因此只能广播不能收事件的网络不会进入本轮事件处理。
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

/// 在整页事件处理成功后推进网关游标，是链事件至少一次语义中唯一的进度提交点。
/// 超过五百字符的游标返回校验错误而不截断，因为截断后的游标会让网关从错误位置继续，可能整段跳过事件。
/// 写入不参与业务事务：游标提交晚于事件处理，进程在此刻崩溃只会导致下轮重复拉取同一页，由幂等消化。
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

/// 在每次调用网关前解密其访问凭据，密文缺省表示该网关免鉴权，直接返回空值放行。
/// 密文存在但未配置凭据加密密钥时返回配置错误，属于可重试类别，因此会停页等待运维补齐配置而非丢弃事件。
/// 解密结果只在本次调用的作用域内存活，既不缓存也不写日志，密钥与明文都不会进入任何输出。
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

/// 按累计尝试次数计算广播重试的退避秒数，以五秒为基数逐次翻倍。
/// 指数上限封顶在六次翻倍，因此退避最长为三百二十秒，不会因失败次数增长而无限拉长。
/// 全程使用饱和运算，尝试次数为零或极大值都不会溢出，退避时长始终落在五到三百二十秒之间。
fn retry_backoff_seconds(attempts: u32) -> u64 {
    5_u64.saturating_mul(2_u64.saturating_pow(attempts.saturating_sub(1).min(6)))
}

/// 把失败原因按字符截断到五百个以内，以适配提现申请和死信表的原因列宽。
/// 按字符而非字节截断，保证多字节文本不会被切成非法编码；截断只影响留痕文本，不改变任何状态判定。
fn bounded_failure_reason(reason: &str) -> String {
    reason.chars().take(500).collect()
}

/// 查询响应只要携带交易哈希、区块高度或正确认数，就已经包含远端受理证据。
/// 该证据与 `not_accepted`/`rejected` 同时出现时必须转人工，禁止自动释放。
fn query_result_has_acceptance_evidence(result: &WalletChainWithdrawalQueryResult) -> bool {
    result.tx_hash.is_some() || result.block_height.is_some() || result.confirmations > 0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WithdrawalReceiptStatus {
    Broadcasted,
    Confirmed,
    NotAccepted,
    Failed,
}

/// 把网关回执状态文本收敛为内部枚举，只接受已广播、已确认、权威未受理和歧义失败四类取值。
/// 比对前裁剪空白并转小写，因此大小写与多余空格不影响识别；未知状态返回校验错误。
/// 未知状态属于确定性拒绝，会被归档为死信而非反复重试，防止网关新增状态时无声卡死游标。
fn normalize_withdrawal_receipt_status(status: &str) -> AppResult<WithdrawalReceiptStatus> {
    match status.trim().to_ascii_lowercase().as_str() {
        "broadcasted" => Ok(WithdrawalReceiptStatus::Broadcasted),
        "confirmed" => Ok(WithdrawalReceiptStatus::Confirmed),
        "not_accepted" | "rejected" => Ok(WithdrawalReceiptStatus::NotAccepted),
        "failed" => Ok(WithdrawalReceiptStatus::Failed),
        _ => Err(AppError::Validation(
            "wallet gateway withdrawal status is invalid".to_owned(),
        )),
    }
}

/// 校验网关返回的请求编号或交易哈希，裁剪首尾空白后拒绝空串、超长以及任何内嵌空白字符。
/// 长度按字节比较以对齐数据库列宽，标识保持原始大小写，避免破坏部分链地址与哈希的校验和语义。
/// 校验失败返回校验错误，属于确定性拒绝，据此把网关返回的坏数据挡在状态迁移之外。
fn normalize_gateway_identifier(value: &str, label: &str, max_length: usize) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > max_length || value.chars().any(char::is_whitespace) {
        return Err(AppError::Validation(format!(
            "wallet gateway {label} format is invalid"
        )));
    }
    Ok(value.to_owned())
}

/// 读取布尔型环境变量，仅接受可被标准布尔解析的文本，变量缺失或取值无法解析时回落到给定默认值。
/// 静默回落而非报错，是为了避免部署时误填开关值导致整个后台任务无法启动。
fn env_bool(key: &str, default: bool) -> bool {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<bool>().ok())
        .unwrap_or(default)
}

/// 读取六十四位无符号整数环境变量，用于承载轮询周期这类可能较大的秒数配置。
/// 负值与非数字文本都无法解析，会连同变量缺失一起回落到默认值，调用方再自行施加下限。
fn env_u64(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

/// 读取三十二位无符号整数环境变量，用于批量条数与最大尝试次数这类有明确上界的配置。
/// 解析失败按默认值处理；真正的安全边界由调用处的钳制完成，本函数不施加任何范围限制。
fn env_u32(key: &str, default: u32) -> u32 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(default)
}

#[cfg(test)]
#[path = "../../tests/unit_src/src_workers_wallet_chain_tests.rs"]
mod tests;
