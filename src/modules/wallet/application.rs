//! wallet bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。

use crate::{
    config::Settings,
    error::{AppError, AppResult},
    modules::{
        security::{SecurityAction, SecurityVerificationInput, verify_user_security_action},
        wallet::{
            infrastructure,
            infrastructure::WalletLedgerFilter,
            presentation::{
                CreateWithdrawalRequest, DepositAddressRequest, DepositAddressResponse,
                DepositAssetResponse, DepositNetworkResponse, DepositNetworksQuery,
                WalletAccountResponse, WalletLedgerQuery, WalletLedgerResponse,
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
    let configured_fee =
        infrastructure::load_asset_withdraw_fee(pool, &request.asset_symbol, &request.amount)
            .await?;
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

    // 提现手续费必须以服务端资产配置重新计算，不能信任客户端提交的 fee 字段。
    let id = infrastructure::insert_withdrawal_request(
        pool,
        user_id,
        &request.asset_symbol,
        request.network.as_deref(),
        &request.address,
        &request.amount,
        &configured_fee,
        security_method.as_str(),
    )
    .await?;

    Ok(WithdrawalRequestResponse {
        id,
        status: "pending".to_owned(),
        security_method,
    })
}

/// 统一从应用状态中获取数据库连接池。
pub(crate) fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for wallet routes".to_owned())
    })
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

    Ok(CreateWithdrawalRequest {
        asset_symbol: request.asset_symbol.trim().to_ascii_uppercase(),
        network: optional_string(request.network),
        address: request.address.trim().to_owned(),
        amount: request.amount,
        fee: request.fee,
        fund_password: request.fund_password,
        totp_code: request.totp_code,
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
