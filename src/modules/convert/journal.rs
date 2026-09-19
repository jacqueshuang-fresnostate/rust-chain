//! 闪兑按资产分别记录用户负债、平台库存和已收手续费，禁止跨币种相抵。

use crate::modules::wallet::platform_journal::WalletPlatformJournalLeg;
use bigdecimal::BigDecimal;

/// 源资产全额扣款拆成净入库存及手续费；费用已含在 from_amount 内，不重复扣钱包。
pub(crate) fn source_legs(amount: &BigDecimal, fee: &BigDecimal) -> Vec<WalletPlatformJournalLeg> {
    [
        ("user_convert_wallet_liability", amount.clone()),
        ("platform_convert_inventory", -(amount - fee)),
        ("platform_convert_fee_income", -fee.clone()),
    ]
    .into_iter()
    .filter(|(_, amount)| *amount != 0)
    .map(|(account_code, amount)| WalletPlatformJournalLeg {
        account_code,
        amount,
    })
    .collect()
}

/// 目标资产的实际入账对应平台库存减少，单币种保持零和，不把源资产差额当作目标币利润。
pub(crate) fn target_legs(amount: &BigDecimal) -> Vec<WalletPlatformJournalLeg> {
    vec![
        WalletPlatformJournalLeg {
            account_code: "user_convert_wallet_liability",
            amount: -amount.clone(),
        },
        WalletPlatformJournalLeg {
            account_code: "platform_convert_inventory",
            amount: amount.clone(),
        },
    ]
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_convert_journal_tests.rs"]
mod tests;
