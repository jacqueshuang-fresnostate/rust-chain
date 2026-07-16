//! wallet bounded context repository layer.
//!
//! 仓储层：定义钱包账户的聚合仓储接口。
//! 具体持久化实现由 infrastructure 层承载，仓储层仅定义边界和行为。

use crate::architecture::RepositoryLayer;
use crate::modules::wallet::{LedgerBatch, LockPosition, WalletAccount, WalletServiceError};

#[derive(Debug)]
pub struct RepositoryLayerMarker;

impl RepositoryLayer for RepositoryLayerMarker {}

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
