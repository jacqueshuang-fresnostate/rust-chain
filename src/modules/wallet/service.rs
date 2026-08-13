//! wallet bounded context service layer.
//!
//! 服务层：封装钱包相关业务动作与不依赖持久化细节的规则编排。
//! 冻结、解冻、结算和锁仓四类动作统一走「加载账户、应用三桶增量、由账后快照生成镜像流水、交仓储保存」的编排路径。
//! 本层只认识 `WalletRepository` 端口，不持有 SQLx、Redis 或事务对象；真正的行锁、幂等键与提交时机由仓储实现决定。
//! 因此跨资产结算与锁仓明细写入无法在此获得全成全败保证，需要原子性的资金链路应改用基础设施层的专用事务用例。

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
    /// 使用指定钱包仓储构造领域服务，事务能力、行锁与提交时机全部由仓储实现提供。
    /// 构造过程不加载账户、不校验资产存在，也不产生任何数据库连接或资金副作用。
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    /// 只读借用内部仓储，用于读取适配器自身状态而非发起资金变更。
    /// 借用期间无法调用需要可变仓储的余额动作，因此不存在并发写入同一工作单元的风险。
    pub fn repository(&self) -> &R {
        &self.repository
    }

    /// 可变借用内部仓储，便于调用方在同一工作单元内追加本服务未覆盖的持久化步骤。
    /// 绕过服务编排意味着领域非负校验和镜像流水不会自动执行，调用方须自行保证账务一致。
    pub fn repository_mut(&mut self) -> &mut R {
        &mut self.repository
    }

    /// 消费钱包服务并交还仓储所有权，通常用于把工作单元移交给外层事务收尾。
    /// 归还后本服务不再可用，尚未提交的资金变更是否生效完全取决于仓储实现的事务状态。
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
    /// 金额必须严格大于零，零或负数在触碰仓储前就以 `NonPositiveAmount` 拒绝，不会加载账户。
    /// available 减该额、frozen 加同额、locked 不变，三桶总额保持守恒；账本因此产生两条镜像条目。
    /// available 不足会在领域非负校验阶段失败；仓储事务失败时账户与流水一并回滚，不留部分冻结。
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

    /// 把正数金额从 frozen 桶退回 available 桶，是 `freeze` 的反向动作且同样保持三桶总额守恒。
    /// 金额需严格为正，非正数直接返回 `NonPositiveAmount`；locked 桶在解冻路径上始终不参与。
    /// 冻结余额不足会在领域非负校验阶段失败，不会出现把 frozen 扣成负数的部分退款。
    /// 本方法自身没有幂等键，重复解冻会真实执行第二次，重放安全依赖上层稳定引用和仓储唯一约束。
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

/// 在触碰仓储之前拦截零和负数金额，防止反向资金动作被伪装成正常冻结或解冻。
/// 零金额同样视为非法，因为它只会产生无意义的空流水；边界判断使用定点比较，不做精度截断。
fn ensure_positive_amount(amount: &BigDecimal) -> Result<(), WalletServiceError> {
    if amount <= &BigDecimal::from(0) {
        Err(WalletServiceError::NonPositiveAmount)
    } else {
        Ok(())
    }
}
