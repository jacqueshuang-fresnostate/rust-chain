//! 显式提现策略；纯规则不读取数据库，也不替业务选择生效额度。

use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};

/// 金额口径必须由运营显式选择，所有额度与复核阶梯沿用同一口径。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WithdrawalAmountBasis {
    Principal,
    TotalReserved,
}

/// 未终结申请可选择跨窗口持续占用，或仅计创建时间落在窗口内的金额。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WithdrawalPendingMode {
    AllOutstanding,
    WithinWindow,
}

/// 当前仅支持按秒滚动窗口；不把滚动二十四小时冒充某个时区的自然日。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WithdrawalWindow {
    Rolling,
}

/// 每条匹配的用户/KYC 规则都独立约束累计金额；用户规则不会覆盖较严的公共规则。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WithdrawalAllowance {
    pub user_id: Option<u64>,
    pub kyc_level: Option<i32>,
    pub window: WithdrawalWindow,
    pub window_seconds: u32,
    pub pending_mode: WithdrawalPendingMode,
    #[serde(deserialize_with = "crate::numeric::deserialize_decimal")]
    pub max_amount: BigDecimal,
}

/// 阶梯下界包含，所需人数为不同管理员数量，最低仍为一次人工审核。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WithdrawalReviewTier {
    #[serde(deserialize_with = "crate::numeric::deserialize_decimal")]
    pub min_amount: BigDecimal,
    pub required_approvals: u32,
}

/// 每资产一份完整策略；空配置默认停用且不携带任何线上金额阈值。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WithdrawalPolicy {
    pub enabled: bool,
    pub amount_basis: WithdrawalAmountBasis,
    pub allowances: Vec<WithdrawalAllowance>,
    pub address_cooling_seconds: Option<u32>,
    pub security_cooling_seconds: Option<u32>,
    pub review_tiers: Vec<WithdrawalReviewTier>,
}

impl Default for WithdrawalPolicy {
    /// 缺省只保留原先单人审核合同；不会隐式开启额度或冷静期。
    fn default() -> Self {
        Self {
            enabled: false,
            amount_basis: WithdrawalAmountBasis::Principal,
            allowances: Vec::new(),
            address_cooling_seconds: None,
            security_cooling_seconds: None,
            review_tiers: Vec::new(),
        }
    }
}

impl WithdrawalPolicy {
    /// 保存及执行时都校验配置；拒绝截断金额、零时窗和递减审核要求，不排序或修改原文。
    pub fn validate(&self, precision: i32) -> Result<(), String> {
        if self.allowances.len() > 100 || self.review_tiers.len() > 50 {
            return Err("withdrawal policy has too many rules".into());
        }
        if self.address_cooling_seconds == Some(0) || self.security_cooling_seconds == Some(0) {
            return Err("withdrawal cooling seconds must be positive or null".into());
        }
        for rule in &self.allowances {
            if rule.user_id == Some(0)
                || rule.kyc_level.is_some_and(|level| level < 0)
                || rule.window_seconds == 0
                || !valid_amount(&rule.max_amount, precision)
            {
                return Err("withdrawal allowance scope, window or amount is invalid".into());
            }
        }
        for (index, tier) in self.review_tiers.iter().enumerate() {
            if !valid_amount(&tier.min_amount, precision)
                || !(1..=20).contains(&tier.required_approvals)
                || (index == 0 && tier.min_amount != 0)
                || (index > 0
                    && (tier.min_amount <= self.review_tiers[index - 1].min_amount
                        || tier.required_approvals
                            < self.review_tiers[index - 1].required_approvals))
            {
                return Err("withdrawal review tiers must start at zero with increasing bounds and nondecreasing reviewers".into());
            }
        }
        Ok(())
    }

    /// 按已明确选择的计量口径取值，不执行任何手续费或汇率换算。
    pub fn measured_amount<'a>(
        &self,
        principal: &'a BigDecimal,
        total_reserved: &'a BigDecimal,
    ) -> &'a BigDecimal {
        match self.amount_basis {
            WithdrawalAmountBasis::Principal => principal,
            WithdrawalAmountBasis::TotalReserved => total_reserved,
        }
    }

    /// 创建时冻结复核人数；停用或未配置阶梯保留历史单人审核，不允许零人自动放行。
    pub fn required_approvals(&self, amount: &BigDecimal) -> u32 {
        if !self.enabled {
            return 1;
        }
        self.review_tiers
            .iter()
            .rev()
            .find(|tier| amount >= &tier.min_amount)
            .map_or(1, |tier| tier.required_approvals)
    }
}

fn valid_amount(value: &BigDecimal, precision: i32) -> bool {
    value >= &BigDecimal::from(0)
        && value < &BigDecimal::from(10).powi(20)
        && super::amount_fits_asset_precision(value, precision)
}

#[cfg(test)]
#[path = "../../../../tests/unit_src/src_modules_wallet_withdrawal_policy_tests.rs"]
mod tests;
