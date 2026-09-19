//! 杠杆金额边界：源金额拒绝超精度，新增计算值按资产一次量化；历史余额只检查存储容量。

use bigdecimal::BigDecimal;

use crate::{
    error::{AppError, AppResult},
    modules::wallet::{amount_fits_asset_precision, truncate_amount_to_asset_precision},
    numeric::ensure_amount_storage,
};

/// 确认权威资产精度合法，禁止把损坏配置静默钳制成另一种金额单位。
pub(crate) fn validate_precision(precision: i32) -> AppResult<()> {
    if !(0..=18).contains(&precision) {
        return Err(AppError::Internal(
            "invalid margin asset precision".to_owned(),
        ));
    }
    Ok(())
}

/// 校验新提交金额的数据库容量与资产精度，尾随零不影响合法性，绝不改写用户意图。
pub(crate) fn validate_input_amount(
    amount: &BigDecimal,
    precision: i32,
    label: &str,
) -> AppResult<()> {
    validate_precision(precision)?;
    ensure_amount_storage(amount, label)?;
    if !amount_fits_asset_precision(amount, precision) {
        return Err(AppError::Validation(format!(
            "{label} exceeds asset precision {precision}"
        )));
    }
    Ok(())
}

/// 将新增派生金额向零量化一次并检查落库容量；返回值必须复用于订单、钱包和平台分录。
/// 不得对历史余额、已计提利息或剩余本金调用本函数，否则会抹去旧有资产单位以下的余额。
pub(crate) fn generated_amount(
    amount: &BigDecimal,
    precision: i32,
    label: &str,
) -> AppResult<BigDecimal> {
    validate_precision(precision)?;
    let amount = truncate_amount_to_asset_precision(amount, precision);
    ensure_amount_storage(&amount, label)?;
    Ok(amount)
}

/// 强平触发仍沿用既有风险公式；确定执行后只量化本次新增盈亏并重算同一笔结算权益。
/// 历史保证金和已计提利息原样参与，不能通过末端返款截断抹掉历史尾差。
pub(crate) fn quantize_settlement_risk(
    risk: &mut super::domain::MarginPositionRiskState,
    margin: &BigDecimal,
    interest: &BigDecimal,
    precision: i32,
) -> AppResult<()> {
    risk.realized_pnl = generated_amount(&risk.realized_pnl, precision, "margin realized pnl")?;
    risk.equity = margin + &risk.realized_pnl - interest;
    for amount in [&risk.equity, &risk.maintenance_margin] {
        ensure_amount_storage(amount, "margin liquidation amount")?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_margin_amounts_tests.rs"]
mod tests;
