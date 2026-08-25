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
use std::fmt;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalletChainGatewayErrorClass {
    DeterministicRejected,
    Unknown,
    RetryableBeforeAcceptance,
}

impl WalletChainGatewayErrorClass {
    /// 返回写入审计日志和数据库状态的稳定错误分类代码。
    ///
    /// 该字符串属于网关状态机契约：确定拒绝才允许释放冻结资金，结果未知必须保留冻结并转人工复核，
    /// 接收前可重试只允许复用原请求号重试。调用方不得通过错误消息文本推断资金处理语义。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DeterministicRejected => "deterministic_rejected",
            Self::Unknown => "unknown",
            Self::RetryableBeforeAcceptance => "retryable_before_acceptance",
        }
    }
}

#[derive(Debug, Clone)]
pub struct WalletChainGatewayError {
    pub class: WalletChainGatewayErrorClass,
    pub message: String,
}

impl WalletChainGatewayError {
    /// 创建带确定状态机分类的链网关错误，并保留经脱敏的诊断消息。
    ///
    /// 分类由网关适配器在 HTTP/网络边界完成，worker 只消费分类而不重新猜测远端是否已经受理；
    /// 这可避免 timeout、5xx 或损坏响应被误当成确定失败并自动解冻。
    pub fn new(class: WalletChainGatewayErrorClass, message: impl Into<String>) -> Self {
        Self {
            class,
            message: message.into(),
        }
    }
}

impl fmt::Display for WalletChainGatewayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for WalletChainGatewayError {}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WalletChainWithdrawalQueryStatus {
    NotAccepted,
    Rejected,
    Pending,
    Accepted,
    Broadcasted,
    Confirmed,
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WalletChainWithdrawalQueryResult {
    pub status: WalletChainWithdrawalQueryStatus,
    pub tx_hash: Option<String>,
    pub block_height: Option<u64>,
    #[serde(default)]
    pub confirmations: u32,
    pub failure_reason: Option<String>,
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
    ) -> Result<WalletChainBroadcastResult, WalletChainGatewayError>;

    /// 以与广播相同的稳定 request_id 查询远端受理状态。
    /// 只有显式 `not_accepted`/`rejected` 才是可释放冻结的权威证据；查询故障本身不具备该语义。
    async fn query_withdrawal(
        &self,
        endpoint: &str,
        bearer_token: Option<&str>,
        request_id: &str,
    ) -> Result<WalletChainWithdrawalQueryResult, WalletChainGatewayError>;

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
