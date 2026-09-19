//! 杠杆按实际抵押、结算和账户级强平生成平台分录，部分平仓只核销本次份额。

use crate::modules::wallet::platform_journal::WalletPlatformJournalLeg;
use bigdecimal::BigDecimal;

/// 开仓是用户钱包负债转为抵押负债，未成交委托也不确认平台收入。
pub(crate) fn opening_legs(amount: &BigDecimal) -> Vec<WalletPlatformJournalLeg> {
    legs([
        ("user_margin_wallet_liability", amount.clone()),
        ("platform_margin_collateral_liability", -amount.clone()),
    ])
}

/// 撤单原路核销未成交抵押，不产生损益或费用。
pub(crate) fn cancellation_legs(amount: &BigDecimal) -> Vec<WalletPlatformJournalLeg> {
    opening_legs(amount)
        .into_iter()
        .map(|leg| WalletPlatformJournalLeg {
            account_code: leg.account_code,
            amount: -leg.amount,
        })
        .collect()
}

/// 每个关闭份额只记一次抵押核销、实际钱包增量、交易损益与应收利息；逐仓负权益截零差额记为坏账。
pub(crate) fn closing_legs(
    collateral: &BigDecimal,
    payout: &BigDecimal,
    pnl: &BigDecimal,
    interest: &BigDecimal,
) -> Vec<WalletPlatformJournalLeg> {
    let shortfall = payout - (collateral + pnl - interest);
    legs([
        ("platform_margin_collateral_liability", collateral.clone()),
        ("user_margin_wallet_liability", -payout.clone()),
        (
            "platform_margin_trading_expense",
            pnl.clone().max(BigDecimal::from(0)),
        ),
        (
            "platform_margin_trading_income",
            pnl.clone().min(BigDecimal::from(0)),
        ),
        ("platform_margin_interest_income", -interest.clone()),
        ("platform_margin_bad_debt_expense", shortfall),
    ])
}

/// 全仓强平按账户一次核销抵押与共享余额；坏账和正剩余权益使用账户汇总，不能逐仓重复相加。
pub(crate) fn cross_liquidation_legs(
    collateral: &BigDecimal,
    wallet_available: &BigDecimal,
    pnl: &BigDecimal,
    interest: &BigDecimal,
) -> Vec<WalletPlatformJournalLeg> {
    let equity = collateral + wallet_available + pnl - interest;
    legs([
        ("platform_margin_collateral_liability", collateral.clone()),
        ("user_margin_wallet_liability", wallet_available.clone()),
        (
            "platform_margin_trading_expense",
            pnl.clone().max(BigDecimal::from(0)),
        ),
        (
            "platform_margin_trading_income",
            pnl.clone().min(BigDecimal::from(0)),
        ),
        ("platform_margin_interest_income", -interest.clone()),
        (
            "platform_margin_bad_debt_expense",
            (-equity.clone()).max(BigDecimal::from(0)),
        ),
        (
            "platform_margin_liquidation_income",
            -equity.max(BigDecimal::from(0)),
        ),
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
#[path = "../../../tests/unit_src/src_modules_margin_journal_tests.rs"]
mod tests;
