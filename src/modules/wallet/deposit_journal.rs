//! 链上充值业务的平台对手腿构造。
//!
//! 用户钱包流水只记录用户一侧的余额变动，平台托管到的链上毛额、对用户的负债
//! 以及留存的手续费收入需要落在 `platform_financial_journal`，否则无法按资产证明托管与损益守恒。
//! 本文件只做无 I/O 的纯计算：输入已按资产口径处理的毛额与净额，输出该业务应当写入的平台分录腿。
//! 手续费不单独传入，而是由毛额与净额相减现算，因此同一 transaction_key 内的腿必然求和为零。
//! 冲正是入账腿的严格取反，两者共用同一组科目代码，靠 transaction_key 区分正向与反向。

use bigdecimal::BigDecimal;

/// 充值平台分录的业务上下文标识，写入 `platform_financial_journal.context`。
pub(crate) const WALLET_DEPOSIT_JOURNAL_CONTEXT: &str = "wallet_deposit_event";

/// 平台分录的一条腿：科目代码加带符号金额。
pub(crate) struct WalletPlatformJournalLeg {
    /// 平台会计科目或对账腿代码，同一 transaction_key 内不得重复。
    pub(crate) account_code: &'static str,
    /// 带符号金额，双方均使用资产口径的定点值。
    pub(crate) amount: BigDecimal,
}

/// 充值入账的平台腿：平台收到链上毛额形成托管流入，同时对用户建立净额负债并确认留存手续费。
/// 手续费由毛额减净额现算，因此三腿之和恒为零；净额等于毛额时手续费腿为零并在构造阶段省略。
/// 调用方传入的毛额与净额必须已经是同一资产口径的定点值，函数本身不再做任何截断或舍入。
pub(crate) fn deposit_credit_journal_legs(
    gross_amount: &BigDecimal,
    net_amount: &BigDecimal,
) -> Vec<WalletPlatformJournalLeg> {
    let fee_amount = gross_amount.clone() - net_amount.clone();
    balanced_legs([
        ("platform_deposit_cash_received", gross_amount.clone()),
        ("user_deposit_liability_open", -net_amount.clone()),
        ("platform_deposit_fee_income", -fee_amount),
    ])
}

/// 充值冲正的平台腿：入账三腿的严格取反，托管流出、对用户负债减少、手续费收入回退。
/// 与入账共用同一组科目代码和同一套金额来源，因此链重组冲正后平台账精确回到入账前的状态。
/// 冲正腿使用独立的 transaction_key，避免与入账腿在同一唯一键下冲突。
pub(crate) fn deposit_reversal_journal_legs(
    gross_amount: &BigDecimal,
    net_amount: &BigDecimal,
) -> Vec<WalletPlatformJournalLeg> {
    let fee_amount = gross_amount.clone() - net_amount.clone();
    balanced_legs([
        ("platform_deposit_cash_received", -gross_amount.clone()),
        ("user_deposit_liability_open", net_amount.clone()),
        ("platform_deposit_fee_income", fee_amount),
    ])
}

/// 过滤掉金额为零的腿，保留调用方给出的科目顺序以便对账时按固定次序读取。
fn balanced_legs<const N: usize>(
    legs: [(&'static str, BigDecimal); N],
) -> Vec<WalletPlatformJournalLeg> {
    legs.into_iter()
        .filter(|(_, amount)| *amount != 0)
        .map(|(account_code, amount)| WalletPlatformJournalLeg {
            account_code,
            amount,
        })
        .collect()
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_wallet_deposit_journal_tests.rs"]
mod tests;
