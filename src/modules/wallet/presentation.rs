//! wallet bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。

use super::WithdrawFeeTier;
use crate::modules::security::SecurityVerificationMethod;
use crate::time::{option_unix_millis, unix_millis};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct DepositAddressRequest {
    pub asset_symbol: String,
    pub network: String,
}

#[derive(Debug, Serialize)]
pub struct DepositAddressResponse {
    pub id: u64,
    pub asset_symbol: String,
    pub network: String,
    pub address: String,
    pub memo: Option<String>,
    #[serde(with = "unix_millis")]
    pub assigned_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateWithdrawalRequest {
    pub asset_symbol: String,
    pub network: Option<String>,
    pub address: String,
    pub amount: BigDecimal,
    pub fee: BigDecimal,
    pub idempotency_key: String,
    pub fund_password: Option<String>,
    pub totp_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WithdrawalRequestResponse {
    pub id: u64,
    pub status: String,
    pub total_reserved: BigDecimal,
    pub security_method: SecurityVerificationMethod,
}

#[derive(Debug, Deserialize)]
pub struct WalletWithdrawalQuery {
    pub status: Option<String>,
    pub user_id: Option<u64>,
    pub limit: Option<u32>,
}

/// 后台充提列表查询参数：用户端不暴露 offset，避免公开接口承诺未实现的翻页语义。
#[derive(Debug, Deserialize)]
pub struct AdminWalletListQuery {
    pub status: Option<String>,
    pub user_id: Option<u64>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct WalletWithdrawalsResponse {
    pub withdrawals: Vec<WalletWithdrawalResponse>,
}

#[derive(Debug, Serialize)]
pub struct AdminWalletWithdrawalsResponse {
    pub withdrawals: Vec<WalletWithdrawalResponse>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct WalletWithdrawalResponse {
    pub id: u64,
    pub user_id: u64,
    pub asset_id: u64,
    pub asset_symbol: String,
    pub network: Option<String>,
    pub address: String,
    pub amount: BigDecimal,
    pub fee: BigDecimal,
    pub total_reserved: BigDecimal,
    pub status: String,
    pub security_method: String,
    pub idempotency_key: String,
    pub gateway_request_id: String,
    pub tx_hash: Option<String>,
    pub block_height: Option<u64>,
    pub confirmations: u32,
    pub failure_reason: Option<String>,
    pub review_reason: Option<String>,
    pub reviewed_by: Option<u64>,
    pub broadcasted_by: Option<u64>,
    pub confirmed_by: Option<u64>,
    pub failed_by: Option<u64>,
    #[serde(default, with = "option_unix_millis")]
    pub reviewed_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub broadcast_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub confirmed_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub failed_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub released_at: Option<DateTime<Utc>>,
    #[serde(with = "unix_millis")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ReviewWithdrawalRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BroadcastWithdrawalRequest {
    pub tx_hash: String,
    pub block_height: Option<u64>,
    pub confirmations: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct ConfirmWithdrawalRequest {
    pub block_height: Option<u64>,
    pub confirmations: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct FailWithdrawalRequest {
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct ObserveDepositRequest {
    pub asset_symbol: String,
    pub network: String,
    pub address: String,
    pub memo: Option<String>,
    pub tx_hash: String,
    #[serde(default)]
    pub event_index: u32,
    pub amount: BigDecimal,
    pub block_height: Option<u64>,
    #[serde(default)]
    pub confirmations: u32,
}

#[derive(Debug, Deserialize)]
pub struct ReverseDepositRequest {
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct WalletDepositsResponse {
    pub deposits: Vec<WalletDepositEventResponse>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct WalletDepositEventResponse {
    pub id: u64,
    pub user_id: u64,
    pub asset_id: u64,
    pub asset_symbol: String,
    pub network: String,
    pub address: String,
    pub memo: Option<String>,
    pub tx_hash: String,
    pub event_index: u32,
    pub amount: BigDecimal,
    pub block_height: Option<u64>,
    pub confirmations: u32,
    pub required_confirmations: u32,
    pub status: String,
    pub failure_reason: Option<String>,
    #[serde(default, with = "option_unix_millis")]
    pub credited_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub reversed_at: Option<DateTime<Utc>>,
    #[serde(with = "unix_millis")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct WalletAccountsResponse {
    pub accounts: Vec<WalletAccountResponse>,
}

#[derive(Debug, Serialize)]
pub struct WalletAccountResponse {
    pub user_id: u64,
    pub asset_id: u64,
    pub symbol: String,
    pub logo_url: Option<String>,
    pub available: BigDecimal,
    pub frozen: BigDecimal,
    pub locked: BigDecimal,
}

#[derive(Debug, Deserialize)]
pub struct WalletLedgerQuery {
    pub asset_id: Option<u64>,
    pub asset_symbol: Option<String>,
    pub change_type: Option<String>,
    pub ref_type: Option<String>,
    pub ref_id: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct WalletLedgerResponse {
    pub entries: Vec<WalletLedgerEntryResponse>,
    pub page: WalletLedgerPageResponse,
}

#[derive(Debug, Serialize)]
pub struct WalletLedgerPageResponse {
    pub number: u32,
    pub size: u32,
    pub total_elements: u64,
    pub total_pages: u32,
}

#[derive(Debug, Serialize)]
pub struct WalletLedgerEntryResponse {
    pub id: u64,
    pub user_id: u64,
    pub asset_id: u64,
    pub symbol: String,
    pub change_type: String,
    pub amount: BigDecimal,
    pub balance_type: String,
    pub balance_after: BigDecimal,
    pub available_after: BigDecimal,
    pub frozen_after: BigDecimal,
    pub locked_after: BigDecimal,
    pub fee: BigDecimal,
    pub ref_type: String,
    pub ref_id: String,
    #[serde(with = "unix_millis")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct DepositNetworksQuery {
    pub asset_symbol: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DepositAssetsResponse {
    pub assets: Vec<DepositAssetResponse>,
}

#[derive(Debug, Serialize)]
pub struct DepositNetworksResponse {
    pub networks: Vec<DepositNetworkResponse>,
}

#[derive(Debug, Serialize)]
pub struct DepositNetworkResponse {
    pub network: String,
    pub display_name: String,
    pub address_group_code: String,
    pub address_group_name: Option<String>,
    pub asset_symbols: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DepositAssetResponse {
    pub symbol: String,
    pub name: String,
    pub logo_url: Option<String>,
    pub precision_scale: i32,
    pub deposit_enabled: bool,
    pub withdraw_enabled: bool,
    pub min_deposit_amount: BigDecimal,
    pub deposit_fee: BigDecimal,
    pub withdraw_fee: BigDecimal,
    pub withdraw_fee_tiers: Vec<WithdrawFeeTier>,
}
