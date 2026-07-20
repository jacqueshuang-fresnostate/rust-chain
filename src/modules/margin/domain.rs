//! margin bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。

use bigdecimal::BigDecimal;

/// 一个全仓账户中的仓位风险输入。
#[derive(Debug, Clone)]
pub(crate) struct CrossMarginPositionRisk {
    pub(crate) unrealized_pnl: BigDecimal,
    pub(crate) interest_amount: BigDecimal,
    pub(crate) maintenance_margin: BigDecimal,
}

/// 全仓账户组合风险快照。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CrossMarginRiskState {
    pub(crate) equity: BigDecimal,
    pub(crate) unrealized_pnl: BigDecimal,
    pub(crate) interest_amount: BigDecimal,
    pub(crate) maintenance_margin: BigDecimal,
    pub(crate) margin_ratio: Option<BigDecimal>,
    pub(crate) should_liquidate: bool,
}

/// 计算同一用户、同一保证金资产下的共享权益和组合风险。
pub(crate) fn evaluate_cross_margin(
    wallet_equity: &BigDecimal,
    position_margin: &BigDecimal,
    positions: &[CrossMarginPositionRisk],
) -> CrossMarginRiskState {
    let unrealized_pnl = positions
        .iter()
        .fold(BigDecimal::from(0), |total, position| {
            total + position.unrealized_pnl.clone()
        })
        .with_scale(18);
    let interest_amount = positions
        .iter()
        .fold(BigDecimal::from(0), |total, position| {
            total + position.interest_amount.clone()
        })
        .with_scale(18);
    let maintenance_margin = positions
        .iter()
        .fold(BigDecimal::from(0), |total, position| {
            total + position.maintenance_margin.clone()
        })
        .with_scale(18);
    let equity = (wallet_equity.clone() + position_margin.clone() + unrealized_pnl.clone()
        - interest_amount.clone())
    .with_scale(18);
    let margin_ratio = if maintenance_margin > 0 {
        Some((equity.clone() / maintenance_margin.clone()).with_scale(18))
    } else {
        None
    };
    CrossMarginRiskState {
        should_liquidate: !positions.is_empty() && equity <= maintenance_margin,
        equity,
        unrealized_pnl,
        interest_amount,
        maintenance_margin,
        margin_ratio,
    }
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_margin_domain_tests.rs"]
mod tests;
