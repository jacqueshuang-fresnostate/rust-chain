//! wallet bounded context service layer.
//!
//! 服务层：封装钱包相关业务动作与不依赖持久化细节的规则编排。

use super::{
    BalanceChange, LedgerBatch, LockPosition, LockSchedule, WalletAccount, WalletRepository,
    WalletServiceError,
};
use bigdecimal::BigDecimal;

#[derive(Debug, Clone)]
pub struct WalletService<R> {
    repository: R,
}

impl<R> WalletService<R> {
    /// 使用指定钱包仓储构造领域服务，事务能力由仓储实现提供。
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    /// 借用当前钱包仓储以读取适配器状态。
    pub fn repository(&self) -> &R {
        &self.repository
    }

    /// 可变借用当前钱包仓储以执行同一工作单元内的操作。
    pub fn repository_mut(&mut self) -> &mut R {
        &mut self.repository
    }

    /// 消费钱包服务并取回其仓储实例。
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
    /// 加载账户后应用三桶增量，并由变更后快照生成镜像账本批次。
    /// 仓储必须将账户与流水原子保存；负余额或持久化失败均不得留下部分资金变化。
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

    /// 把正数金额从 available 桶迁移到 frozen 桶，并写入调用方提供的业务引用流水。
    /// 三桶总额保持不变；仓储事务失败或余额不足时账户与流水一并回滚。
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

    /// 把正数金额从 frozen 桶退回 available 桶，并沿用业务引用生成双桶流水。
    /// 冻结余额不足会在领域校验阶段失败，重放策略由上层稳定引用和仓储唯一性共同保证。
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

    /// 结算时扣减 frozen 并增加目标资产 available；同资产时合并为一次账户与流水原子保存。
    /// 跨资产时先保存扣款资产、再保存收款资产，当前仓储接口没有跨调用事务；第二腿失败不会自动撤销已提交的 frozen 扣减。
    /// 因此该通用服务只适用于能在外层提供同一工作单元的仓储，真实跨资产结算不得把这里误当作全成全败保证。
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

    /// 先按解锁计划汇总正数来源，再把总额从 available 迁入 locked 并写三桶流水，最后持久化锁仓明细。
    /// 当前接口先提交账户/流水、后单独插入锁仓；锁仓写入失败不会自动回滚已增加的 locked。
    /// 调用方若要求账户 locked 与活动锁仓明细原子一致，必须提供外层事务或使用专用基础设施用例。
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
