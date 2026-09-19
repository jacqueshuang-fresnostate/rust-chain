//! 实际现货成交的逐资产平台分录；行情源不是资金对手方，内部预充值账户才代表库存。

use super::common::SYSTEM_SPOT_LIQUIDITY_EMAIL;
use crate::{
    error::{AppError, AppResult},
    modules::{
        spot::SpotTrade,
        wallet::{
            infrastructure::insert_platform_journal_with_reference_in_tx,
            platform_journal::WalletPlatformJournalLeg,
        },
    },
};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Transaction};

/// 在已完成四条钱包资金腿的成交事务内，按基础/报价资产分别记录实际负债和库存变动。
/// 复用成交 ID 作为唯一引用；调用者负责成交占位和重放短路，任何分录失败均回滚整笔成交。
/// 当前结算不收手续费；非零 fee 缺乏收费资产及扣款合同，必须拒绝而不是虚构收入或补扣钱包。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn insert_spot_fill_journal_in_tx(
    tx: &mut Transaction<'_, MySql>,
    trade: &SpotTrade,
    buyer_id: u64,
    seller_id: u64,
    base_asset_id: u64,
    quote_asset_id: u64,
    fill_quote_amount: &BigDecimal,
) -> AppResult<()> {
    if trade.fee != 0 {
        return Err(AppError::Internal(
            "spot fill fee requires an explicit charged-asset settlement contract".to_owned(),
        ));
    }
    let liquidity_id: Option<u64> =
        sqlx::query_scalar("SELECT id FROM users WHERE email = ? AND id IN (?, ?) LIMIT 1")
            .bind(SYSTEM_SPOT_LIQUIDITY_EMAIL)
            .bind(buyer_id)
            .bind(seller_id)
            .fetch_optional(&mut **tx)
            .await?;
    let buyer_is_platform = liquidity_id == Some(buyer_id);
    let seller_is_platform = liquidity_id == Some(seller_id);
    let transaction_key = format!("spot:{}:fill", trade.id);
    for (asset_id, amount, source_is_platform, target_is_platform) in [
        (
            base_asset_id,
            &trade.quantity,
            seller_is_platform,
            buyer_is_platform,
        ),
        (
            quote_asset_id,
            fill_quote_amount,
            buyer_is_platform,
            seller_is_platform,
        ),
    ] {
        let legs = asset_transfer_legs(amount, source_is_platform, target_is_platform);
        insert_platform_journal_with_reference_in_tx(
            tx,
            "spot",
            &transaction_key,
            asset_id,
            "spot_trade",
            &trade.id,
            &legs,
        )
        .await?;
    }
    Ok(())
}

fn asset_transfer_legs(
    amount: &BigDecimal,
    source_is_platform: bool,
    target_is_platform: bool,
) -> Vec<WalletPlatformJournalLeg> {
    let source_liability = if source_is_platform {
        BigDecimal::from(0)
    } else {
        amount.clone()
    };
    let target_liability = if target_is_platform {
        BigDecimal::from(0)
    } else {
        -amount.clone()
    };
    let inventory = -(&source_liability + &target_liability);
    [
        ("user_spot_source_liability", source_liability),
        ("user_spot_target_liability", target_liability),
        ("platform_spot_inventory", inventory),
    ]
    .into_iter()
    .filter(|(_, amount)| *amount != 0)
    .map(|(account_code, amount)| WalletPlatformJournalLeg {
        account_code,
        amount,
    })
    .collect()
}

#[cfg(test)]
#[path = "../../../../tests/unit_src/src_modules_spot_fill_journal_tests.rs"]
mod tests;
