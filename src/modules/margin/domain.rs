//! margin bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。

use bigdecimal::BigDecimal;

/// 计算逐仓仓位的非负返还额；没有已实现盈亏的非终态仓位返回零。
/// 输入违反保证金状态机或数值不变量时返回既有领域错误，调用方不得据此产生资金副作用。
pub(crate) fn margin_position_payout_amount(
    margin_amount: &BigDecimal,
    realized_pnl: Option<&BigDecimal>,
    interest_amount: &BigDecimal,
) -> BigDecimal {
    realized_pnl
        .map(|pnl| {
            let payout_amount = margin_amount + pnl - interest_amount;
            if payout_amount > 0 {
                payout_amount.with_scale(18)
            } else {
                BigDecimal::from(0).with_scale(18)
            }
        })
        .unwrap_or_else(|| BigDecimal::from(0).with_scale(18))
}

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
    pub(crate) portfolio_equity: BigDecimal,
    pub(crate) unrealized_pnl: BigDecimal,
    pub(crate) interest_amount: BigDecimal,
    pub(crate) maintenance_margin: BigDecimal,
    pub(crate) margin_ratio: Option<BigDecimal>,
    pub(crate) should_liquidate: bool,
}

/// 计算同一用户、同一保证金资产下的共享权益和组合风险。
/// 输入违反保证金状态机或数值不变量时返回既有领域错误，调用方不得据此产生资金副作用。
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
    let portfolio_equity =
        (position_margin.clone() + unrealized_pnl.clone() - interest_amount.clone()).with_scale(18);
    let margin_ratio = if maintenance_margin > 0 {
        Some((equity.clone() / maintenance_margin.clone()).with_scale(18))
    } else {
        None
    };
    CrossMarginRiskState {
        should_liquidate: !positions.is_empty() && equity <= maintenance_margin,
        equity,
        portfolio_equity,
        unrealized_pnl,
        interest_amount,
        maintenance_margin,
        margin_ratio,
    }
}

/// 将账户级可返还权益按各仓位正权益比例分配，仅用于清算记录和事件展示。
///
/// 钱包只按组合权益变更一次；这里的分配结果之和不得超过组合正权益，
/// 避免盈利仓返还、亏损仓截零后制造额外资产。
pub(crate) fn allocate_cross_margin_payouts(
    position_equities: &[BigDecimal],
    portfolio_equity: &BigDecimal,
) -> Vec<BigDecimal> {
    let zero = BigDecimal::from(0).with_scale(18);
    let payout_total = if portfolio_equity > &zero {
        portfolio_equity.clone().with_scale(18)
    } else {
        zero.clone()
    };
    let positive_total = position_equities
        .iter()
        .filter(|equity| *equity > &zero)
        .fold(zero.clone(), |total, equity| total + equity.clone())
        .with_scale(18);
    if payout_total == zero || positive_total == zero {
        return vec![zero; position_equities.len()];
    }

    let mut allocated = zero.clone();
    let last_positive_index = position_equities
        .iter()
        .rposition(|equity| equity > &zero)
        .expect("positive total requires one positive position");
    position_equities
        .iter()
        .enumerate()
        .map(|(index, equity)| {
            if equity <= &zero {
                return zero.clone();
            }
            if index == last_positive_index {
                return (payout_total.clone() - allocated.clone()).with_scale(18);
            }
            let amount =
                (payout_total.clone() * equity.clone() / positive_total.clone()).with_scale(18);
            allocated += amount.clone();
            amount
        })
        .collect()
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_margin_domain_tests.rs"]
mod tests;
