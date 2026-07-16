use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};

pub(crate) const EARLY_REDEEM_FEE_BASIS_NONE: &str = "none";
pub(crate) const EARLY_REDEEM_FEE_BASIS_PRINCIPAL: &str = "principal";
pub(crate) const EARLY_REDEEM_FEE_BASIS_PROFIT: &str = "profit";

pub(crate) struct EarnRedemptionTerms<'a> {
    pub amount: &'a BigDecimal,
    pub apr_rate: &'a BigDecimal,
    pub term_days: u32,
    pub subscribed_at: DateTime<Utc>,
    pub matures_at: DateTime<Utc>,
    pub redemption_fee_rate: &'a BigDecimal,
    pub maturity_profit_fee_rate: &'a BigDecimal,
    pub early_redeem_fee_basis: &'a str,
    pub early_redeem_fee_rate: &'a BigDecimal,
}

pub(crate) struct EarnRedemptionAmounts {
    pub principal_amount: BigDecimal,
    pub gross_yield_amount: BigDecimal,
    pub redemption_fee_amount: BigDecimal,
    pub maturity_profit_fee_amount: BigDecimal,
    pub early_redeem_fee_amount: BigDecimal,
    pub fee_amount: BigDecimal,
    pub yield_amount: BigDecimal,
    pub redeem_amount: BigDecimal,
}

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
    let redeem_amount = if raw_redeem_amount < BigDecimal::from(0) {
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

fn scaled_amount(amount: BigDecimal) -> BigDecimal {
    amount.with_scale(18)
}

fn zero_amount() -> BigDecimal {
    BigDecimal::from(0).with_scale(18)
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_earn_redemption_tests.rs"]
mod tests;
