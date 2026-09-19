//! 预测订单平台分录：冻结本金不改变总负债，收费、终局损益及退费按实际金额记录。

use crate::modules::wallet::platform_journal::WalletPlatformJournalLeg;
use bigdecimal::BigDecimal;

/// 只确认实际扣除的手续费；本金仍在用户 frozen 桶，不能在开仓时确认本金收入。
pub(crate) fn fee_legs(fee: &BigDecimal) -> Vec<WalletPlatformJournalLeg> {
    legs([
        ("user_prediction_wallet_liability", fee.clone()),
        ("platform_prediction_fee_income", -fee.clone()),
    ])
}

/// 已结算订单核销冻结本金、记入实际派奖，以净差额确认收入或费用，零赔付也写完整终局分录。
pub(crate) fn settlement_legs(
    stake: &BigDecimal,
    payout: &BigDecimal,
) -> Vec<WalletPlatformJournalLeg> {
    let net = payout - stake;
    legs([
        ("user_prediction_frozen_liability", stake.clone()),
        ("user_prediction_wallet_liability", -payout.clone()),
        (
            "platform_prediction_payout_expense",
            net.clone().max(BigDecimal::from(0)),
        ),
        ("platform_prediction_income", net.min(BigDecimal::from(0))),
    ])
}

/// 本金原路解冻不产生平台资金移动，仅按既定政策实际退还的手续费反向确认收入。
pub(crate) fn refund_legs(fee: &BigDecimal) -> Vec<WalletPlatformJournalLeg> {
    fee_legs(fee)
        .into_iter()
        .map(|leg| WalletPlatformJournalLeg {
            account_code: leg.account_code,
            amount: -leg.amount,
        })
        .collect()
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
#[path = "../../../tests/unit_src/src_modules_prediction_journal_tests.rs"]
mod tests;
