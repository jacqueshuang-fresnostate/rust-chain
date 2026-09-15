//! 理财业务的平台对手腿构造。
//!
//! 用户钱包流水只记录用户一侧的余额变动，平台持有本金、支付收益和收取费用的对手方
//! 需要落在 `platform_financial_journal`，否则无法按资产证明托管与损益守恒。
//! 本文件只做无 I/O 的纯计算：输入订阅快照金额，输出该业务应当写入的平台分录腿。
//! 每个 transaction_key 内的腿必须严格求和为零，且金额为零的腿必须省略，
//! 因为数据库对金额有非零约束，零腿既无意义也会让对账口径出现空行。
//! 符号沿用既有借贷与清算腿的约定：正值表示平台侧价值流出或负债增加，
//! 负值表示平台侧资产或权益增加，因此同一 transaction_key 的腿相加恒为零。

use bigdecimal::BigDecimal;

/// 理财平台分录的业务上下文标识，写入 `platform_financial_journal.context`。
pub(crate) const EARN_PLATFORM_JOURNAL_CONTEXT: &str = "earn_subscription";

/// 平台分录的一条腿：科目代码加带符号金额。
pub(crate) struct EarnPlatformJournalLeg {
    /// 平台会计科目或对账腿代码，同一 transaction_key 内不得重复。
    pub(crate) account_code: &'static str,
    /// 带符号金额，未经量化的订阅快照金额。
    pub(crate) amount: BigDecimal,
}

/// 申购的平台腿：平台收到本金形成的托管流入，与对用户的应付本金同时建立。
/// 两腿金额相等符号相反，因此本金在任何精度下都严格守恒，不产生尾差。
pub(crate) fn earn_subscription_journal_legs(
    principal: &BigDecimal,
) -> Vec<EarnPlatformJournalLeg> {
    balanced_legs([
        ("platform_earn_cash_received", principal.clone()),
        ("earn_principal_payable_open", -principal.clone()),
    ])
}

/// 赎回的平台腿：结转应付本金、支付净到账额、确认收益成本与手续费收入。
/// 净到账额等于本金加毛收益减总费用，因此四腿之和恒为零；
/// 毛收益或费用为零时对应腿被省略，避免触发分录非零约束。
/// 三项费率叠加后可能超过本金加毛收益，此时用户实收被兜底到零，平台能确认的费用收入也只能到本金加毛收益为止；
/// 因此费用腿先按可收上限截断，否则零下限会让分录变成不闭合的差额。
pub(crate) fn earn_redemption_journal_legs(
    principal: &BigDecimal,
    gross_yield: &BigDecimal,
    fee_amount: &BigDecimal,
    redeem_amount: &BigDecimal,
) -> Vec<EarnPlatformJournalLeg> {
    let collectible = principal.clone() + gross_yield.clone();
    let effective_fee = if *fee_amount > collectible {
        collectible
    } else {
        fee_amount.clone()
    };
    balanced_legs([
        ("earn_principal_payable_close", principal.clone()),
        ("platform_earn_redemption_cash", -redeem_amount.clone()),
        ("earn_yield_expense", gross_yield.clone()),
        ("platform_earn_fee_income", -effective_fee),
    ])
}

/// 过滤掉金额为零的腿，保留调用方给出的科目顺序以便对账时按固定次序读取。
fn balanced_legs<const N: usize>(
    legs: [(&'static str, BigDecimal); N],
) -> Vec<EarnPlatformJournalLeg> {
    legs.into_iter()
        .filter(|(_, amount)| *amount != 0)
        .map(|(account_code, amount)| EarnPlatformJournalLeg {
            account_code,
            amount,
        })
        .collect()
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_earn_journal_tests.rs"]
mod tests;
