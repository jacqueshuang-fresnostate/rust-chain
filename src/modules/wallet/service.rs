//! wallet bounded context service layer.
//!
//! 服务层：封装钱包相关业务动作与不依赖持久化细节的规则编排。

use super::{
    BalanceChange, LedgerBatch, LockPosition, LockSchedule, WalletAccount, WalletRepository,
    WalletServiceError,
};
use crate::architecture::ServiceLayer;
use bigdecimal::BigDecimal;

#[derive(Debug)]
pub struct ServiceLayerMarker;

impl ServiceLayer for ServiceLayerMarker {}

#[derive(Debug, Clone)]
pub struct WalletService<R> {
    repository: R,
}

impl<R> WalletService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn repository(&self) -> &R {
        &self.repository
    }

    pub fn repository_mut(&mut self) -> &mut R {
        &mut self.repository
    }

    pub fn into_repository(self) -> R {
        self.repository
    }
}

#[derive(Debug, Clone)]
pub struct BalanceUpdateCommand {
    pub user_id: String,
    pub asset_id: String,
    pub change: BalanceChange,
    pub ledger: super::LedgerMetadata,
}

#[derive(Debug, Clone)]
pub struct FreezeBalanceCommand {
    pub user_id: String,
    pub asset_id: String,
    pub amount: BigDecimal,
    pub ledger: super::LedgerMetadata,
}

#[derive(Debug, Clone)]
pub struct UnfreezeBalanceCommand {
    pub user_id: String,
    pub asset_id: String,
    pub amount: BigDecimal,
    pub ledger: super::LedgerMetadata,
}

#[derive(Debug, Clone)]
pub struct SettleBalanceCommand {
    pub user_id: String,
    pub debit_frozen_asset_id: String,
    pub debit_frozen_amount: BigDecimal,
    pub credit_available_asset_id: String,
    pub credit_available_amount: BigDecimal,
    pub ledger: super::LedgerMetadata,
}

#[derive(Debug, Clone)]
pub struct LockPositionCreationCommand {
    pub user_id: String,
    pub asset_id: String,
    pub schedule: LockSchedule,
    pub sources: Vec<super::LockPositionSource>,
    pub ledger: super::LedgerMetadata,
}

impl<R: WalletRepository> WalletService<R> {
    pub fn apply_balance_update(
        &mut self,
        command: BalanceUpdateCommand,
    ) -> Result<WalletAccount, WalletServiceError> {
        let mut account = self
            .repository
            .load_account(&command.user_id, &command.asset_id)?;
        account.apply_balance_change(command.change.clone())?;
        let ledger = LedgerBatch::from_account_change(&account, command.change, &command.ledger);
        self.repository
            .save_account_with_ledger(account.clone(), ledger)?;
        Ok(account)
    }

    pub fn freeze(
        &mut self,
        command: FreezeBalanceCommand,
    ) -> Result<WalletAccount, WalletServiceError> {
        ensure_positive_amount(&command.amount)?;
        self.apply_balance_update(BalanceUpdateCommand {
            user_id: command.user_id,
            asset_id: command.asset_id,
            change: BalanceChange::new(
                -command.amount.clone(),
                command.amount,
                BigDecimal::from(0),
            ),
            ledger: command.ledger,
        })
    }

    pub fn unfreeze(
        &mut self,
        command: UnfreezeBalanceCommand,
    ) -> Result<WalletAccount, WalletServiceError> {
        ensure_positive_amount(&command.amount)?;
        self.apply_balance_update(BalanceUpdateCommand {
            user_id: command.user_id,
            asset_id: command.asset_id,
            change: BalanceChange::new(
                command.amount.clone(),
                -command.amount,
                BigDecimal::from(0),
            ),
            ledger: command.ledger,
        })
    }

    pub fn settle(&mut self, command: SettleBalanceCommand) -> Result<(), WalletServiceError> {
        ensure_positive_amount(&command.debit_frozen_amount)?;
        ensure_positive_amount(&command.credit_available_amount)?;

        if command.debit_frozen_asset_id == command.credit_available_asset_id {
            self.apply_balance_update(BalanceUpdateCommand {
                user_id: command.user_id,
                asset_id: command.debit_frozen_asset_id,
                change: BalanceChange::new(
                    command.credit_available_amount,
                    -command.debit_frozen_amount,
                    BigDecimal::from(0),
                ),
                ledger: command.ledger,
            })?;
            return Ok(());
        }

        self.apply_balance_update(BalanceUpdateCommand {
            user_id: command.user_id.clone(),
            asset_id: command.debit_frozen_asset_id,
            change: BalanceChange::new(
                BigDecimal::from(0),
                -command.debit_frozen_amount,
                BigDecimal::from(0),
            ),
            ledger: command.ledger.clone(),
        })?;
        self.apply_balance_update(BalanceUpdateCommand {
            user_id: command.user_id,
            asset_id: command.credit_available_asset_id,
            change: BalanceChange::new(
                command.credit_available_amount,
                BigDecimal::from(0),
                BigDecimal::from(0),
            ),
            ledger: command.ledger,
        })?;
        Ok(())
    }

    pub fn create_lock_positions(
        &mut self,
        command: LockPositionCreationCommand,
    ) -> Result<Vec<LockPosition>, WalletServiceError> {
        let positions = super::create_lock_positions(
            &command.user_id,
            &command.asset_id,
            command.schedule,
            command.sources,
        )?;
        let total_locked = positions.iter().fold(BigDecimal::from(0), |sum, position| {
            sum + position.remaining_amount.clone()
        });
        ensure_positive_amount(&total_locked)?;

        self.apply_balance_update(BalanceUpdateCommand {
            user_id: command.user_id,
            asset_id: command.asset_id,
            change: BalanceChange::new(-total_locked.clone(), BigDecimal::from(0), total_locked),
            ledger: command.ledger,
        })?;
        self.repository.insert_lock_positions(positions.clone())?;
        Ok(positions)
    }
}

fn ensure_positive_amount(amount: &BigDecimal) -> Result<(), WalletServiceError> {
    if amount <= &BigDecimal::from(0) {
        Err(WalletServiceError::NonPositiveAmount)
    } else {
        Ok(())
    }
}
