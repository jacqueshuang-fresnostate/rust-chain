//! wallet bounded context repository layer.
//!
//! 仓储层：定义钱包账户的聚合仓储接口。
//! 具体持久化实现由 infrastructure 层承载，仓储层仅定义边界和行为。

use crate::{
    error::AppResult,
    modules::wallet::{LedgerBatch, LockPosition, WalletAccount, WalletServiceError},
};
use axum::async_trait;
use serde::{Deserialize, Serialize};

pub trait WalletRepository: Send {
    fn load_account(
        &mut self,
        user_id: &str,
        asset_id: &str,
    ) -> Result<WalletAccount, WalletServiceError>;

    fn save_account_with_ledger(
        &mut self,
        account: WalletAccount,
        ledger: LedgerBatch,
    ) -> Result<(), WalletServiceError>;

    fn insert_lock_positions(
        &mut self,
        positions: Vec<LockPosition>,
    ) -> Result<(), WalletServiceError>;
}

#[derive(Debug, Clone, Serialize)]
pub struct WalletChainBroadcastCommand {
    pub request_id: String,
    pub network: String,
    pub asset_symbol: String,
    pub address: String,
    pub amount: String,
    pub fee: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WalletChainBroadcastResult {
    pub tx_hash: String,
    pub block_height: Option<u64>,
    #[serde(default)]
    pub confirmations: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WalletChainDepositObservation {
    pub asset_symbol: String,
    pub network: String,
    pub address: String,
    pub memo: Option<String>,
    pub tx_hash: String,
    #[serde(default)]
    pub event_index: u32,
    pub amount: String,
    pub block_height: Option<u64>,
    #[serde(default)]
    pub confirmations: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WalletChainWithdrawalObservation {
    pub request_id: String,
    pub network: String,
    pub tx_hash: Option<String>,
    pub block_height: Option<u64>,
    #[serde(default)]
    pub confirmations: u32,
    pub status: String,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WalletChainPollPage {
    pub next_cursor: Option<String>,
    #[serde(default)]
    pub deposits: Vec<WalletChainDepositObservation>,
    #[serde(default)]
    pub withdrawals: Vec<WalletChainWithdrawalObservation>,
}

#[async_trait]
pub trait WalletChainGateway: Send + Sync {
    async fn broadcast_withdrawal(
        &self,
        endpoint: &str,
        bearer_token: Option<&str>,
        command: &WalletChainBroadcastCommand,
    ) -> AppResult<WalletChainBroadcastResult>;

    async fn poll_chain_events(
        &self,
        endpoint: &str,
        bearer_token: Option<&str>,
        cursor: Option<&str>,
        limit: u32,
    ) -> AppResult<WalletChainPollPage>;
}
