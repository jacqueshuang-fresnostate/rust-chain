//! wallet bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。

use crate::{
    config::Settings,
    error::{AppError, AppResult},
    modules::{
        security::{SecurityAction, SecurityVerificationInput, verify_user_security_action},
        wallet::{
            amount_fits_asset_precision, infrastructure,
            infrastructure::WalletLedgerFilter,
            presentation::{
                BroadcastWithdrawalRequest, ConfirmWithdrawalRequest, CreateWithdrawalRequest,
                DepositAddressRequest, DepositAddressResponse, DepositAssetResponse,
                DepositNetworkResponse, DepositNetworksQuery, FailWithdrawalRequest,
                ObserveDepositRequest, ReverseDepositRequest, ReviewWithdrawalRequest,
                WalletAccountResponse, WalletDepositEventResponse, WalletLedgerQuery,
                WalletLedgerResponse, WalletWithdrawalQuery, WalletWithdrawalResponse,
                WithdrawalRequestResponse,
            },
        },
    },
    state::AppState,
};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool};

pub(crate) async fn list_deposit_assets(
    pool: &Pool<MySql>,
) -> AppResult<Vec<DepositAssetResponse>> {
    infrastructure::list_deposit_assets(pool).await
}

pub(crate) async fn list_withdraw_assets(
    pool: &Pool<MySql>,
) -> AppResult<Vec<DepositAssetResponse>> {
    infrastructure::list_withdraw_assets(pool).await
}

pub(crate) async fn list_deposit_networks(
    pool: &Pool<MySql>,
    asset_symbol: Option<&str>,
) -> AppResult<Vec<DepositNetworkResponse>> {
    infrastructure::list_active_deposit_networks(pool, asset_symbol).await
}

/// 路由层只传 DTO，本函数在应用层统一完成 `asset_symbol` 的规范化与校验。
pub(crate) async fn list_deposit_networks_by_query(
    pool: &Pool<MySql>,
    query: &DepositNetworksQuery,
) -> AppResult<Vec<DepositNetworkResponse>> {
    let asset_symbol = normalize_deposit_networks_query_asset(query)?;

    list_deposit_networks(pool, asset_symbol.as_deref()).await
}

/// 仅做查询参数归一化与校验，不触达数据库，用于路由前置校验。
pub(crate) fn normalize_deposit_networks_query_asset(
    query: &DepositNetworksQuery,
) -> AppResult<Option<String>> {
    query
        .asset_symbol
        .as_deref()
        .map(normalize_asset_symbol)
        .transpose()
}

pub(crate) async fn get_or_assign_deposit_address(
    pool: &Pool<MySql>,
    user_id: u64,
    request: DepositAddressRequest,
) -> AppResult<DepositAddressResponse> {
    let request = normalize_deposit_address_request(request)?;
    let network_config = infrastructure::load_active_deposit_network_config(
        pool,
        &request.network,
        &request.asset_symbol,
    )
    .await?;
    infrastructure::ensure_deposit_enabled_asset(pool, &request.asset_symbol).await?;

    if let Some(mut address) = infrastructure::load_user_deposit_address(
        pool,
        user_id,
        &request.asset_symbol,
        &network_config.address_group_code,
        &request.network,
    )
    .await?
    {
        address.network = request.network;
        return Ok(address);
    }

    // 地址池库存锁定、用户邮箱读取和分配写入必须在同一个事务中完成，避免同一地址被并发分配。
    let mut tx = pool.begin().await?;
    let candidate_id = infrastructure::lock_available_deposit_address(
        &mut tx,
        &request.asset_symbol,
        &network_config.address_group_code,
        &request.network,
    )
    .await?;
    let assigned_user_email = infrastructure::load_user_email_in_tx(&mut tx, user_id).await?;
    infrastructure::assign_deposit_address_in_tx(
        &mut tx,
        candidate_id,
        user_id,
        assigned_user_email,
        &request.asset_symbol,
    )
    .await?;
    let mut address = infrastructure::load_deposit_address_in_tx(&mut tx, candidate_id).await?;
    tx.commit().await?;
    address.network = request.network;
    Ok(address)
}

pub(crate) async fn list_wallet_accounts(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Vec<WalletAccountResponse>> {
    infrastructure::list_wallet_accounts(pool, user_id).await
}

pub(crate) async fn list_wallet_ledger(
    pool: &Pool<MySql>,
    user_id: u64,
    filter: WalletLedgerFilter,
) -> AppResult<WalletLedgerResponse> {
    infrastructure::list_wallet_ledger(pool, user_id, filter).await
}

/// 标准化查询分页参数，避免路由层重复实现同样边界规则。
pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

/// 标准化查询偏移参数，路由层不再承担边界裁剪职责。
pub(crate) fn route_offset(offset: Option<u32>) -> u32 {
    offset.unwrap_or(0).min(100_000)
}

/// 标准化可选字符串查询参数，保留 `trim` 与空值过滤规则。
pub(crate) fn normalize_optional_query_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 校验并规范化资产符号输入。
pub(crate) fn normalize_asset_symbol(value: &str) -> AppResult<String> {
    let symbol = value.trim();
    if symbol.is_empty() {
        return Err(AppError::Validation("asset_symbol is required".to_owned()));
    }
    if symbol.len() > 32 || !symbol.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        return Err(AppError::Validation(
            "asset_symbol format is invalid".to_owned(),
        ));
    }
    Ok(symbol.to_ascii_uppercase())
}

/// 校验并规范化网络标识输入。
pub(crate) fn normalize_deposit_network(value: &str) -> AppResult<String> {
    let network = value.trim().to_ascii_lowercase();
    match network.as_str() {
        "eth" | "ethereum" | "erc20" => Ok("eth".to_owned()),
        "base" => Ok("base".to_owned()),
        "tron" | "trx" | "trc20" => Ok("tron".to_owned()),
        "btc" | "bitcoin" => Ok("btc".to_owned()),
        "sol" | "solana" => Ok("solana".to_owned()),
        _ => Err(AppError::Validation(
            "unsupported deposit network".to_owned(),
        )),
    }
}

/// 将外层路由层传入的账本查询 DTO 转换为基础设施可执行的过滤器。
pub(crate) fn build_wallet_ledger_filter(
    query: WalletLedgerQuery,
) -> AppResult<WalletLedgerFilter> {
    Ok(WalletLedgerFilter {
        asset_id: query.asset_id,
        asset_symbol: query
            .asset_symbol
            .map(|value| normalize_asset_symbol(&value))
            .transpose()?,
        change_type: normalize_optional_query_string(query.change_type),
        ref_type: normalize_optional_query_string(query.ref_type),
        ref_id: normalize_optional_query_string(query.ref_id),
        start_time: normalize_optional_query_string(query.start_time),
        end_time: normalize_optional_query_string(query.end_time),
        limit: route_limit(query.limit),
        offset: route_offset(query.offset),
    })
}

pub(crate) async fn create_withdrawal_request(
    pool: &Pool<MySql>,
    settings: &Settings,
    user_id: u64,
    request: CreateWithdrawalRequest,
) -> AppResult<WithdrawalRequestResponse> {
    let request = validate_withdrawal_request(request)?;
    let asset =
        infrastructure::load_withdrawal_asset_rule(pool, &request.asset_symbol, &request.amount)
            .await?;
    if !amount_fits_asset_precision(&request.amount, asset.precision_scale) {
        return Err(AppError::Validation(format!(
            "withdrawal amount supports at most {} decimal places",
            asset.precision_scale
        )));
    }
    if let Some(existing) =
        infrastructure::load_withdrawal_by_user_key(pool, user_id, &request.idempotency_key).await?
    {
        ensure_withdrawal_replay_matches(&existing, &request, &asset.fee)?;
        return withdrawal_request_response(existing);
    }
    let security_method = verify_user_security_action(
        pool,
        settings,
        user_id,
        SecurityAction::Withdraw,
        SecurityVerificationInput {
            fund_password: request.fund_password.as_deref(),
            totp_code: request.totp_code.as_deref(),
        },
    )
    .await?;

    // 请求、余额冻结和账本必须同事务提交；唯一键冲突时只允许返回完全一致的历史请求。
    let withdrawal = match infrastructure::reserve_withdrawal_request(
        pool,
        user_id,
        &asset,
        &request.asset_symbol,
        request.network.as_deref(),
        &request.address,
        &request.amount,
        &request.idempotency_key,
        security_method.as_str(),
    )
    .await
    {
        Ok(withdrawal) => withdrawal,
        Err(AppError::Database(error)) if is_duplicate_key_error(&error) => {
            let existing = infrastructure::load_withdrawal_by_user_key(
                pool,
                user_id,
                &request.idempotency_key,
            )
            .await?
            .ok_or_else(|| {
                AppError::Conflict("withdrawal idempotency key was used concurrently".to_owned())
            })?;
            ensure_withdrawal_replay_matches(&existing, &request, &asset.fee)?;
            existing
        }
        Err(error) => return Err(error),
    };
    withdrawal_request_response(withdrawal)
}

pub(crate) async fn list_user_withdrawals(
    pool: &Pool<MySql>,
    user_id: u64,
    query: WalletWithdrawalQuery,
) -> AppResult<Vec<WalletWithdrawalResponse>> {
    let status = normalize_withdrawal_status(query.status)?;
    infrastructure::list_wallet_withdrawals(
        pool,
        Some(user_id),
        status.as_deref(),
        route_limit(query.limit),
    )
    .await
}

pub(crate) async fn list_admin_withdrawals(
    pool: &Pool<MySql>,
    query: WalletWithdrawalQuery,
) -> AppResult<Vec<WalletWithdrawalResponse>> {
    let status = normalize_withdrawal_status(query.status)?;
    infrastructure::list_wallet_withdrawals(
        pool,
        query.user_id,
        status.as_deref(),
        route_limit(query.limit).min(200),
    )
    .await
}

pub(crate) async fn approve_withdrawal(
    pool: &Pool<MySql>,
    admin_id: u64,
    withdrawal_id: u64,
    request: ReviewWithdrawalRequest,
) -> AppResult<WalletWithdrawalResponse> {
    let reason = normalize_optional_query_string(request.reason);
    let mut tx = pool.begin().await?;
    let withdrawal = infrastructure::approve_withdrawal_in_tx(
        &mut tx,
        withdrawal_id,
        admin_id,
        reason.as_deref(),
    )
    .await?;
    tx.commit().await?;
    Ok(withdrawal)
}

pub(crate) async fn reject_withdrawal(
    pool: &Pool<MySql>,
    admin_id: u64,
    withdrawal_id: u64,
    request: ReviewWithdrawalRequest,
) -> AppResult<WalletWithdrawalResponse> {
    let reason = required_reason(request.reason, "rejection reason")?;
    let mut tx = pool.begin().await?;
    let withdrawal = infrastructure::release_withdrawal_in_tx(
        &mut tx,
        withdrawal_id,
        Some(admin_id),
        "rejected",
        &reason,
    )
    .await?;
    tx.commit().await?;
    Ok(withdrawal)
}

pub(crate) async fn broadcast_withdrawal(
    pool: &Pool<MySql>,
    admin_id: u64,
    withdrawal_id: u64,
    request: BroadcastWithdrawalRequest,
) -> AppResult<WalletWithdrawalResponse> {
    let tx_hash = normalize_chain_identifier(request.tx_hash, "tx_hash")?;
    let mut tx = pool.begin().await?;
    let withdrawal = infrastructure::mark_withdrawal_broadcasted_in_tx(
        &mut tx,
        withdrawal_id,
        Some(admin_id),
        &tx_hash,
        request.block_height,
        request.confirmations.unwrap_or(0),
    )
    .await?;
    tx.commit().await?;
    Ok(withdrawal)
}

pub(crate) async fn confirm_withdrawal(
    pool: &Pool<MySql>,
    admin_id: u64,
    withdrawal_id: u64,
    request: ConfirmWithdrawalRequest,
) -> AppResult<WalletWithdrawalResponse> {
    let mut tx = pool.begin().await?;
    let withdrawal = infrastructure::confirm_withdrawal_in_tx(
        &mut tx,
        withdrawal_id,
        Some(admin_id),
        request.block_height,
        request.confirmations.unwrap_or(1),
    )
    .await?;
    tx.commit().await?;
    Ok(withdrawal)
}

pub(crate) async fn fail_withdrawal(
    pool: &Pool<MySql>,
    admin_id: u64,
    withdrawal_id: u64,
    request: FailWithdrawalRequest,
) -> AppResult<WalletWithdrawalResponse> {
    let reason = required_reason(Some(request.reason), "failure reason")?;
    let mut tx = pool.begin().await?;
    let withdrawal = infrastructure::release_withdrawal_in_tx(
        &mut tx,
        withdrawal_id,
        Some(admin_id),
        "failed",
        &reason,
    )
    .await?;
    tx.commit().await?;
    Ok(withdrawal)
}

pub(crate) async fn observe_deposit(
    pool: &Pool<MySql>,
    request: ObserveDepositRequest,
) -> AppResult<WalletDepositEventResponse> {
    let request = normalize_observe_deposit_request(request)?;
    infrastructure::observe_deposit_event(pool, &request).await
}

pub(crate) async fn reverse_deposit(
    pool: &Pool<MySql>,
    deposit_id: u64,
    request: ReverseDepositRequest,
) -> AppResult<WalletDepositEventResponse> {
    let reason = required_reason(Some(request.reason), "reversal reason")?;
    infrastructure::reverse_deposit_event(pool, deposit_id, &reason).await
}

pub(crate) async fn list_admin_deposits(
    pool: &Pool<MySql>,
    query: WalletWithdrawalQuery,
) -> AppResult<Vec<WalletDepositEventResponse>> {
    infrastructure::list_deposit_events(pool, query.user_id, route_limit(query.limit).min(200))
        .await
}

/// 统一从应用状态中获取数据库连接池。
pub(crate) fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for wallet routes".to_owned())
    })
}

/// 解析后台 JWT subject，避免管理路由信任请求体中的管理员标识。
pub(crate) fn admin_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("admin:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

fn validate_withdrawal_request(
    request: CreateWithdrawalRequest,
) -> AppResult<CreateWithdrawalRequest> {
    if request.asset_symbol.trim().is_empty() {
        return Err(AppError::Validation("asset_symbol is required".to_owned()));
    }
    if request.address.trim().is_empty() {
        return Err(AppError::Validation("address is required".to_owned()));
    }
    if request.amount <= BigDecimal::from(0) {
        return Err(AppError::Validation("amount must be positive".to_owned()));
    }
    if request.fee < BigDecimal::from(0) {
        return Err(AppError::Validation("fee must be non-negative".to_owned()));
    }
    let idempotency_key = request.idempotency_key.trim();
    if idempotency_key.is_empty()
        || idempotency_key.len() > 128
        || !idempotency_key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':' | '.'))
    {
        return Err(AppError::Validation(
            "idempotency_key format is invalid".to_owned(),
        ));
    }

    Ok(CreateWithdrawalRequest {
        asset_symbol: request.asset_symbol.trim().to_ascii_uppercase(),
        network: request
            .network
            .map(|network| normalize_deposit_network(&network))
            .transpose()?,
        address: request.address.trim().to_owned(),
        amount: request.amount,
        fee: request.fee,
        idempotency_key: idempotency_key.to_owned(),
        fund_password: request.fund_password,
        totp_code: request.totp_code,
    })
}

fn ensure_withdrawal_replay_matches(
    existing: &WalletWithdrawalResponse,
    request: &CreateWithdrawalRequest,
    configured_fee: &BigDecimal,
) -> AppResult<()> {
    if existing.asset_symbol != request.asset_symbol
        || existing.network != request.network
        || existing.address != request.address
        || existing.amount != request.amount
        || existing.fee != *configured_fee
    {
        return Err(AppError::Conflict(
            "withdrawal idempotency key was reused with different parameters".to_owned(),
        ));
    }
    Ok(())
}

fn withdrawal_request_response(
    withdrawal: WalletWithdrawalResponse,
) -> AppResult<WithdrawalRequestResponse> {
    let security_method = match withdrawal.security_method.as_str() {
        "fund_password" => crate::modules::security::SecurityVerificationMethod::FundPassword,
        "two_factor" => crate::modules::security::SecurityVerificationMethod::TwoFactor,
        "fund_password_and_two_factor" => {
            crate::modules::security::SecurityVerificationMethod::FundPasswordAndTwoFactor
        }
        _ => {
            return Err(AppError::Internal(
                "withdrawal security method is invalid".to_owned(),
            ));
        }
    };
    Ok(WithdrawalRequestResponse {
        id: withdrawal.id,
        status: withdrawal.status,
        total_reserved: withdrawal.total_reserved,
        security_method,
    })
}

fn normalize_withdrawal_status(status: Option<String>) -> AppResult<Option<String>> {
    let status = normalize_optional_query_string(status);
    if let Some(status) = status.as_deref()
        && !matches!(
            status,
            "pending_review"
                | "approved"
                | "broadcasting"
                | "broadcasted"
                | "confirmed"
                | "manual_review"
                | "rejected"
                | "failed"
        )
    {
        return Err(AppError::Validation(
            "withdrawal status is invalid".to_owned(),
        ));
    }
    Ok(status)
}

fn required_reason(reason: Option<String>, label: &str) -> AppResult<String> {
    let reason = normalize_optional_query_string(reason)
        .ok_or_else(|| AppError::Validation(format!("{label} is required")))?;
    if reason.len() > 512 {
        return Err(AppError::Validation(format!(
            "{label} must not exceed 512 characters"
        )));
    }
    Ok(reason)
}

fn normalize_chain_identifier(value: String, label: &str) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 255 || value.chars().any(char::is_whitespace) {
        return Err(AppError::Validation(format!("{label} format is invalid")));
    }
    Ok(value.to_owned())
}

fn normalize_observe_deposit_request(
    request: ObserveDepositRequest,
) -> AppResult<ObserveDepositRequest> {
    if request.amount <= BigDecimal::from(0) {
        return Err(AppError::Validation(
            "deposit amount must be positive".to_owned(),
        ));
    }
    let address = normalize_chain_identifier(request.address, "address")?;
    let tx_hash = normalize_chain_identifier(request.tx_hash, "tx_hash")?;
    Ok(ObserveDepositRequest {
        asset_symbol: normalize_asset_symbol(&request.asset_symbol)?,
        network: normalize_deposit_network(&request.network)?,
        address,
        memo: optional_string(request.memo),
        tx_hash,
        event_index: request.event_index,
        amount: request.amount,
        block_height: request.block_height,
        confirmations: request.confirmations,
    })
}

fn is_duplicate_key_error(error: &sqlx::Error) -> bool {
    error.as_database_error().is_some_and(|database_error| {
        matches!(database_error.code().as_deref(), Some("1062" | "23000"))
    })
}

fn normalize_deposit_address_request(
    request: DepositAddressRequest,
) -> AppResult<DepositAddressRequest> {
    let asset_symbol = normalize_asset_symbol(&request.asset_symbol)?;
    let network = normalize_deposit_network(&request.network)?;
    Ok(DepositAddressRequest {
        asset_symbol,
        network,
    })
}

fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_wallet_application_tests.rs"]
mod tests;
