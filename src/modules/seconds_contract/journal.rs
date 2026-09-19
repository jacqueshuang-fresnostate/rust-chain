//! 秒合约已扣本金先记待结算负债，终局再确认平台损益，不把全部开仓本金当收入。

use crate::modules::wallet::platform_journal::WalletPlatformJournalLeg;
use bigdecimal::BigDecimal;

/// 开仓只把钱包负债转成合约待结算负债；与订单扣款同额，不新增托管现金。
pub(crate) fn open_legs(stake: &BigDecimal) -> Vec<WalletPlatformJournalLeg> {
    legs([
        ("user_seconds_wallet_liability", stake.clone()),
        ("platform_seconds_pending_liability", -stake.clone()),
    ])
}

/// 终局释放全部本金负债，按实际赔付建立钱包负债，并仅将净差额列为收入或赔付费用。
/// 零赔付的输单也必须记账；人工恢复与自动结算共用相同分录，旧单不伪造历史开仓分录。
pub(crate) fn settlement_legs(
    stake: &BigDecimal,
    payout: &BigDecimal,
) -> Vec<WalletPlatformJournalLeg> {
    let net = payout - stake;
    legs([
        ("platform_seconds_pending_liability", stake.clone()),
        ("user_seconds_wallet_liability", -payout.clone()),
        (
            "platform_seconds_payout_expense",
            net.clone().max(BigDecimal::from(0)),
        ),
        ("platform_seconds_income", net.min(BigDecimal::from(0))),
    ])
}

fn legs<const N: usize>(entries: [(&'static str, BigDecimal); N]) -> Vec<WalletPlatformJournalLeg> {
    entries
        .into_iter()
        .filter(|(_, amount)| *amount != 0)
        .map(|(account_code, amount)| WalletPlatformJournalLeg {
            account_code,
            amount,
        })
        .collect()
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_seconds_contract_journal_tests.rs"]
mod tests;
