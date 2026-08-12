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
    /// 按用户与资产加载 available/frozen/locked 快照；是否加锁由具体工作单元实现约定。
    /// 缺失账户或读取失败必须返回错误，不能用零余额掩盖不存在的聚合。
    fn load_account(
        &mut self,
        user_id: &str,
        asset_id: &str,
    ) -> Result<WalletAccount, WalletServiceError>;

    /// 原子保存账户三桶与同批镜像流水；实现方须保证任一步失败时全部回滚。
    /// 业务调用方负责提供稳定引用，重放不得生成第二批相同流水。
    fn save_account_with_ledger(
        &mut self,
        account: WalletAccount,
        ledger: LedgerBatch,
    ) -> Result<(), WalletServiceError>;

    /// 持久化已通过领域校验的锁仓明细；该调用与账户/流水保存是独立仓储步骤。
    /// 实现方须保证本批锁仓自身不部分写入，但接口不承诺回滚此前已经保存的 locked 余额。
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
    /// 向链网关提交一次提现广播命令，并返回交易哈希与初始确认进度。
    /// request_id 是外部幂等身份；超时或失败由调用方保留本地冻结并决定重试。
    async fn broadcast_withdrawal(
        &self,
        endpoint: &str,
        bearer_token: Option<&str>,
        command: &WalletChainBroadcastCommand,
    ) -> AppResult<WalletChainBroadcastResult>;

    /// 从给定游标分页拉取充值与提现链事件，并返回下一游标。
    /// 网关错误不得推进游标，调用方应在本地事务成功处理整页后再保存进度。
    async fn poll_chain_events(
        &self,
        endpoint: &str,
        bearer_token: Option<&str>,
        cursor: Option<&str>,
        limit: u32,
    ) -> AppResult<WalletChainPollPage>;
}
