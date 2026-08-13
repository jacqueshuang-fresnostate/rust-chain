//! margin bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。
//! 本文件只保存杠杆的资金口径计算：逐仓返还额、全仓账户组合风险评估和账户权益的分仓分摊。
//! 所有函数都是纯计算，不访问数据库、Redis 或行情，也不发布事件；调用方需自行保证输入取自同一时点快照。
//! 金额一律以 `with_scale(18)` 归一到十八位小数，与 `DECIMAL(38,18)` 资金列精度保持一致。

use bigdecimal::BigDecimal;

/// 计算逐仓仓位平仓时可返还用户的金额，口径为保证金加已实现盈亏再扣除累计利息。
/// `realized_pnl` 为 None 表示仓位尚未成交也没有盈亏结果，直接返回十八位精度的零而非退回本金。
/// 结果按非负截断：亏损吃穿保证金时只返还零，穿仓缺口不在这里体现，由全仓账户结算或坏账登记承担。
/// 纯计算函数，不读写钱包余额、流水与仓位状态，调用方仍须在结算事务内自行完成入账。
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

/// 一个全仓账户中的单仓风险输入，三项都以保证金币种计价并已按当前标记价估过值。
#[derive(Debug, Clone)]
pub(crate) struct CrossMarginPositionRisk {
    /// 该仓位按最新标记价计算的浮动盈亏，做多做空均可正可负。
    pub(crate) unrealized_pnl: BigDecimal,
    /// 该仓位截至当前已计提但尚未结算的借款利息，恒为非负并直接抵减权益。
    pub(crate) interest_amount: BigDecimal,
    /// 该仓位的维持保证金要求，等于名义价值乘以产品维持保证金率。
    pub(crate) maintenance_margin: BigDecimal,
}

/// 全仓账户组合风险快照，同一用户同一保证金币种下所有全仓仓位共享这组指标。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CrossMarginRiskState {
    /// 账户总权益，含杠杆钱包可用余额、已占用仓位保证金、浮盈与应付利息。
    pub(crate) equity: BigDecimal,
    /// 组合权益，仅统计这批仓位自身净值，是强平时回写共享钱包的有符号增量。
    pub(crate) portfolio_equity: BigDecimal,
    /// 各仓位浮动盈亏之和。
    pub(crate) unrealized_pnl: BigDecimal,
    /// 各仓位应付利息之和。
    pub(crate) interest_amount: BigDecimal,
    /// 各仓位维持保证金之和，是判定强平线的分母。
    pub(crate) maintenance_margin: BigDecimal,
    /// 账户权益与维持保证金之比；无仓位或维持保证金为零时为 None，表示该比率无意义。
    pub(crate) margin_ratio: Option<BigDecimal>,
    /// 是否已触发账户级强平，由存在仓位且权益不高于维持保证金共同决定。
    pub(crate) should_liquidate: bool,
}

/// 汇总同一用户、同一保证金币种下全部全仓仓位的估值，产出账户级共享风险快照。
/// 浮盈、利息与维持保证金按仓位逐项求和；账户权益为钱包权益加已占用仓位保证金加浮盈再减利息，
/// 组合权益剔除钱包部分，只表示这批仓位的净值，两者与各项汇总均归一到十八位小数。
/// 维持保证金为零时保证金率返回 None 以避免除零；仓位非空且权益不高于维持保证金即置强平标记。
/// 纯计算函数，不查询行情、不加行锁、不落库，输入必须由调用方取自同一时点的仓位与钱包快照。
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

/// 将账户级可返还权益按各仓位正权益的占比拆分到每个仓位，仅用于强平记录和事件展示。
///
/// 钱包只按组合权益整体变更一次；这里的分摊结果之和被严格约束为不超过组合正权益，
/// 避免盈利仓按自身权益返还、亏损仓被截零后凭空多分出资产。
/// 组合权益非正或所有仓位权益都非正时，整批返回十八位精度的零，长度与输入一一对应。
/// 权益非正的仓位一律分到零；最后一个正权益仓位领取剩余尾差，吸收逐笔按比例相除产生的十八位截断误差。
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
