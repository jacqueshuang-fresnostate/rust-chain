//! wallet bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。

use super::WithdrawFeeTier;
use crate::modules::security::SecurityVerificationMethod;
use crate::time::unix_millis;
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
    pub fund_password: Option<String>,
    pub totp_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WithdrawalRequestResponse {
    pub id: u64,
    pub status: String,
    pub security_method: SecurityVerificationMethod,
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
