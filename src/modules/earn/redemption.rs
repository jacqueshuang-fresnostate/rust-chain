//! 理财赎回金额的纯计算模块。
//!
//! 这里是理财收益与费用的唯一算式来源，全部输入由调用方以订阅快照形式传入，
//! 函数不读数据库、不读系统时钟、不移动任何资金，因此可脱离环境单测。
//! 计算分两步：先按是否到期得出毛收益，再依次扣除三类费用得到净到账额。
//! 三类费用互相独立且可同时生效：通用赎回费始终按本金加毛收益计，
//! 到期收益费只在到期后按毛收益计，提前赎回费只在未到期时按配置基准计。
//! 所有中间量与结果统一保留 18 位小数，与钱包账本列精度一致。

use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};

/// 提前赎回费基准：不收取，此时费率被强制归零。
pub(crate) const EARLY_REDEEM_FEE_BASIS_NONE: &str = "none";
/// 提前赎回费基准：按本金计费，无论收益多少都按申购金额乘费率收取。
pub(crate) const EARLY_REDEEM_FEE_BASIS_PRINCIPAL: &str = "principal";
/// 提前赎回费基准：按毛收益计费，收益为零时该项费用也为零。
pub(crate) const EARLY_REDEEM_FEE_BASIS_PROFIT: &str = "profit";

/// 一次赎回计算的全部输入，字段应全部取自订阅行的快照而非产品当前配置。
/// 以借用形式持有金额是为了避免在高频调用路径上克隆 BigDecimal。
pub(crate) struct EarnRedemptionTerms<'a> {
    /// 申购本金，同时是计息基数与本金类费用的计费基准。
    pub amount: &'a BigDecimal,
    /// 年化收益率快照。
    pub apr_rate: &'a BigDecimal,
    /// 产品期限天数快照，到期计息时按该天数占一年的比例折算。
    pub term_days: u32,
    /// 申购时刻，提前赎回按从该时刻起的实际秒数计息。
    pub subscribed_at: DateTime<Utc>,
    /// 到期时刻，同时是「提前」与「到期」两种计费口径的分界点。
    pub matures_at: DateTime<Utc>,
    /// 通用赎回费率，对本金加毛收益整体计费，到期与提前都收取。
    pub redemption_fee_rate: &'a BigDecimal,
    /// 到期利润手续费率，只在到期赎回时对毛收益计费。
    pub maturity_profit_fee_rate: &'a BigDecimal,
    /// 提前赎回费基准，取 none、principal 或 profit 之一。
    pub early_redeem_fee_basis: &'a str,
    /// 提前赎回费率，仅在未到期且基准非 none 时生效。
    pub early_redeem_fee_rate: &'a BigDecimal,
}

/// 赎回计算的完整输出，各项均保留 18 位小数，便于对外展示逐项费用明细。
/// 只有 `redeem_amount` 会真正进入钱包，其余字段仅用于响应与事件载荷。
pub(crate) struct EarnRedemptionAmounts {
    /// 申购本金，原样回传不做任何加工。
    pub principal_amount: BigDecimal,
    /// 未扣任何费用的毛收益。
    pub gross_yield_amount: BigDecimal,
    /// 通用赎回费，基数为本金加毛收益。
    pub redemption_fee_amount: BigDecimal,
    /// 到期利润手续费，提前赎回时恒为零。
    pub maturity_profit_fee_amount: BigDecimal,
    /// 提前赎回费，到期赎回或基准为 none 时恒为零。
    pub early_redeem_fee_amount: BigDecimal,
    /// 三类费用之和，即本次赎回被扣掉的总金额。
    pub fee_amount: BigDecimal,
    /// 展示口径的净收益，只扣除以毛收益为基准的那部分费用，不扣通用赎回费。
    pub yield_amount: BigDecimal,
    /// 实际入账 available 的净到账额，等于本金加毛收益减总费用，下限为零。
    pub redeem_amount: BigDecimal,
}

/// 依据订阅快照与给定时刻算出赎回的本金、毛收益、三类费用与净到账额。
/// 是否到期由 `now` 与 `matures_at` 比较决定，等于到期时刻即按到期处理，费用口径随之切换。
/// 通用赎回费对本金加毛收益整体计费，到期与提前都收；到期利润手续费只在到期后对毛收益计；
/// 提前赎回费只在未到期时生效，按配置对本金或毛收益二选一计费，基准为 none 时不收。
/// 净到账额为本金加毛收益减三项费用，若费用超过总额则截断为零，绝不产生负数入账。
/// 展示用的净收益只扣以毛收益为基准的费用，即到期利润手续费和基准为 profit 的提前赎回费，
/// 因此当通用赎回费不为零时，`principal + yield_amount` 会大于 `redeem_amount`。
/// 全程只做计算，不读数据库、不写钱包、不改订阅状态。
pub(crate) fn calculate_earn_redemption_amounts(
    terms: EarnRedemptionTerms<'_>,
    now: DateTime<Utc>,
) -> EarnRedemptionAmounts {
    let principal_amount = terms.amount.clone();
    let gross_yield_amount = earn_gross_yield_amount(&terms, now);
    let gross_redeem_amount = principal_amount.clone() + gross_yield_amount.clone();
    let is_early = now < terms.matures_at;

    let redemption_fee_amount =
        scaled_amount(gross_redeem_amount.clone() * terms.redemption_fee_rate.clone());
    let maturity_profit_fee_amount = if is_early {
        zero_amount()
    } else {
        scaled_amount(gross_yield_amount.clone() * terms.maturity_profit_fee_rate.clone())
    };
    let early_redeem_fee_amount = match terms.early_redeem_fee_basis {
        EARLY_REDEEM_FEE_BASIS_PRINCIPAL if is_early => {
            scaled_amount(principal_amount.clone() * terms.early_redeem_fee_rate.clone())
        }
        EARLY_REDEEM_FEE_BASIS_PROFIT if is_early => {
            scaled_amount(gross_yield_amount.clone() * terms.early_redeem_fee_rate.clone())
        }
        _ => zero_amount(),
    };
    let fee_amount = redemption_fee_amount.clone()
        + maturity_profit_fee_amount.clone()
        + early_redeem_fee_amount.clone();
    let raw_redeem_amount = gross_redeem_amount - fee_amount.clone();
    let redeem_amount = if raw_redeem_amount < 0 {
        zero_amount()
    } else {
        scaled_amount(raw_redeem_amount)
    };
    let profit_fee_amount = maturity_profit_fee_amount.clone()
        + if matches!(terms.early_redeem_fee_basis, EARLY_REDEEM_FEE_BASIS_PROFIT) {
            early_redeem_fee_amount.clone()
        } else {
            zero_amount()
        };
    let yield_amount = scaled_amount(gross_yield_amount.clone() - profit_fee_amount);

    EarnRedemptionAmounts {
        principal_amount,
        gross_yield_amount,
        redemption_fee_amount,
        maturity_profit_fee_amount,
        early_redeem_fee_amount,
        fee_amount,
        yield_amount,
        redeem_amount,
    }
}

/// 计算未扣任何费用的毛收益，到期与未到期使用两套不同的计息口径。
/// 到期时按整期计：年化收益乘以 `term_days / 365`，与实际持有多久无关。
/// 未到期时按实际持有秒数计：年化收益乘以 `elapsed_seconds / (365*24*60*60)`。
/// 两个分母都用 365 天而非自然年长度，因此闰年不做特殊处理。
/// 系统时钟回拨导致 now 早于申购时刻时，经过秒数被夹到零，收益按零计而不会为负。
/// 结果统一保留 18 位小数，不做四舍五入之外的额外处理。
fn earn_gross_yield_amount(terms: &EarnRedemptionTerms<'_>, now: DateTime<Utc>) -> BigDecimal {
    let yearly_yield = terms.amount.clone() * terms.apr_rate.clone();
    if now >= terms.matures_at {
        return scaled_amount(
            yearly_yield * BigDecimal::from(terms.term_days) / BigDecimal::from(365),
        );
    }

    let elapsed_seconds = now
        .signed_duration_since(terms.subscribed_at)
        .num_seconds()
        .max(0);
    scaled_amount(
        yearly_yield * BigDecimal::from(elapsed_seconds) / BigDecimal::from(365 * 24 * 60 * 60),
    )
}

/// 把中间结果统一压到 18 位小数，与钱包账本列的精度对齐。
/// `with_scale` 走整数除法，多余位数直接向零截断而不做进位，因此结果不会因舍入而变大。
/// 每一步费用与收益都经过本函数，避免高精度中间值在后续相减时残留无法对账的尾差。
fn scaled_amount(amount: BigDecimal) -> BigDecimal {
    amount.with_scale(18)
}

/// 构造带 18 位小数的零值，用于费用不适用的分支和净额下限。
/// 显式指定 scale 是为了让所有输出字段保持一致的小数位表现，避免序列化结果时零值与非零值格式不一。
fn zero_amount() -> BigDecimal {
    BigDecimal::from(0).with_scale(18)
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_earn_redemption_tests.rs"]
mod tests;
