//! risk bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。
//! 把后台存储的风控规则行折算为领域阈值，不触碰任何 I/O。
//!
//! `config_json` 可识别的键（其余键一律忽略）：
//! - `operations`: 字符串数组，规则生效的操作；缺省时对所有操作生效。
//! - `blocked_operations`: 字符串数组，直接拒绝的操作，多条规则取并集。
//! - `max_amount`: 单笔金额上限，口径由 `operations` 唯一确定；未声明 `operations` 时忽略。
//! - `max_price_deviation_bps`: 相对市场价的偏离上限（基点）。
//! - `max_requests` / `window_seconds`: 固定窗口限频，窗口缺省 60 秒。
//!
//! 多条规则命中时的优先级：数值阈值取最严、`blocked_operations` 取并集、限频取
//! (`max_requests` 最小 → `window_seconds` 最大 → 作用域字典序最小) 的那一条。
//! 三者都与规则行的先后顺序无关，因此解析结果确定。

use crate::{architecture::ServiceLayer, modules::risk::domain::RiskRules};
use bigdecimal::BigDecimal;
use serde_json::Value;
use std::{cmp::Reverse, str::FromStr};

/// 限频窗口默认 60 秒，规则未显式配置 `window_seconds` 时生效。
pub const DEFAULT_RATE_LIMIT_WINDOW_SECONDS: u32 = 60;
/// 没有命中带作用域的限频规则时使用的计数键作用域。
pub const GLOBAL_RISK_SCOPE: &str = "global";
/// 现货下单；金额口径为计价币种名义额（价格 × 数量）。
pub const OPERATION_SPOT_ORDER_CREATE: &str = "spot.order.create";
/// 发起提现；金额口径为提币资产数量。
pub const OPERATION_WALLET_WITHDRAWAL_CREATE: &str = "wallet.withdrawal.create";

/// 金额口径。限额只能作用在口径一致的操作上，否则 1 BTC 的提现上限会被套到 1 USDT 的下单名义额上。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AmountUnit {
    QuoteNotional,
    AssetQuantity,
}

/// `risk_rules` 表中的一行；`config_json` 的键即风控维度，`rule_type` 仅供后台归类。
#[derive(Debug, Clone)]
pub struct StoredRiskRule {
    pub target_type: String,
    pub target_id: Option<String>,
    pub config: Value,
}

impl StoredRiskRule {
    fn target_id(&self) -> Option<&str> {
        self.target_id
            .as_deref()
            .map(str::trim)
            .filter(|target_id| !target_id.is_empty())
    }

    /// 规则作用域标识；同时作为限频计数键的一段，避免不同作用域的规则共用计数器。
    fn scope_key(&self) -> String {
        match self.target_id() {
            Some(target_id) => format!(
                "{}:{}",
                self.target_type.trim().to_ascii_lowercase(),
                target_id.to_ascii_lowercase()
            ),
            None => GLOBAL_RISK_SCOPE.to_owned(),
        }
    }
}

/// 请求所属的规则作用域；规则 `target_id` 为空时对所有作用域生效。
#[derive(Debug, Clone)]
pub struct RiskScope {
    pub dimension: &'static str,
    pub value: String,
}

impl RiskScope {
    pub fn new(dimension: &'static str, value: impl Into<String>) -> Self {
        Self {
            dimension,
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskPolicy {
    pub rules: RiskRules,
    pub rate_limit_window_seconds: u32,
    /// 生效限频规则的作用域，计数键按它隔离。
    pub rate_limit_scope: String,
}

impl ServiceLayer for RiskPolicy {}

impl Default for RiskPolicy {
    fn default() -> Self {
        Self {
            rules: RiskRules::default(),
            rate_limit_window_seconds: DEFAULT_RATE_LIMIT_WINDOW_SECONDS,
            rate_limit_scope: GLOBAL_RISK_SCOPE.to_owned(),
        }
    }
}

/// 限频候选：请求上限、窗口秒数、计数键作用域。
#[derive(Debug, Clone)]
struct RateLimitCandidate {
    max_requests: u32,
    window_seconds: u32,
    scope: String,
}

impl RateLimitCandidate {
    /// 排序键：请求数越小越严，同请求数下窗口越长越严，最后按作用域定序，保证结果与规则顺序无关。
    fn strictness_key(&self) -> (u32, Reverse<u32>, &str) {
        (
            self.max_requests,
            Reverse(self.window_seconds),
            self.scope.as_str(),
        )
    }
}

/// 合并同时命中当前操作和作用域的规则；没有命中规则时返回全放行策略。
pub fn resolve_risk_policy(
    stored: &[StoredRiskRule],
    operation: &str,
    scopes: &[RiskScope],
) -> RiskPolicy {
    let mut policy = RiskPolicy::default();
    let mut rate_limit: Option<RateLimitCandidate> = None;
    let mut blocked_operations: Vec<String> = Vec::new();

    for rule in stored
        .iter()
        .filter(|rule| rule_applies(rule, operation, scopes))
    {
        merge_amount_limit(&mut policy.rules, &rule.config, operation);
        if let Some(max_deviation_bps) = u32_field(&rule.config, "max_price_deviation_bps") {
            policy.rules.max_price_deviation_bps = Some(
                policy
                    .rules
                    .max_price_deviation_bps
                    .map_or(max_deviation_bps, |current| current.min(max_deviation_bps)),
            );
        }
        if let Some(candidate) = rate_limit_candidate(rule)
            && rate_limit
                .as_ref()
                .is_none_or(|current| candidate.strictness_key() < current.strictness_key())
        {
            rate_limit = Some(candidate);
        }
        merge_blocked_operations(&mut blocked_operations, &rule.config);
    }

    if let Some(rate_limit) = rate_limit {
        policy.rules.max_requests = Some(rate_limit.max_requests);
        policy.rate_limit_window_seconds = rate_limit.window_seconds;
        policy.rate_limit_scope = rate_limit.scope;
    }
    if !blocked_operations.is_empty() {
        blocked_operations.sort();
        policy.rules.blocked_operations = Some(blocked_operations);
    }
    policy
}

fn rule_applies(rule: &StoredRiskRule, operation: &str, scopes: &[RiskScope]) -> bool {
    rule_covers_operation(&rule.config, operation) && rule_covers_scope(rule, scopes)
}

/// 规则只约束自己列出的操作，缺省 `operations` 才对所有操作生效。
/// 阈值因此按操作叠加而不是跨操作取交集，两条各自合理的规则不可能叠加出全面拒绝。
fn rule_covers_operation(config: &Value, operation: &str) -> bool {
    let Some(operations) = string_list_field(config, "operations") else {
        return true;
    };
    operations
        .iter()
        .any(|listed| listed.eq_ignore_ascii_case(operation))
}

fn rule_covers_scope(rule: &StoredRiskRule, scopes: &[RiskScope]) -> bool {
    let Some(target_id) = rule.target_id() else {
        return true;
    };

    scopes.iter().any(|scope| {
        scope
            .dimension
            .eq_ignore_ascii_case(rule.target_type.trim())
            && scope.value.eq_ignore_ascii_case(target_id)
    })
}

/// 金额限额只在规则声明的操作口径与当前操作一致时生效；口径不明的规则宁可不生效，也不能按错误单位拦截或放行。
fn merge_amount_limit(rules: &mut RiskRules, config: &Value, operation: &str) {
    let Some(max_amount) = decimal_field(config, "max_amount") else {
        return;
    };
    if operation_amount_unit(operation).is_none_or(|unit| rule_amount_unit(config) != Some(unit)) {
        return;
    }

    rules.max_amount = Some(match rules.max_amount.take() {
        Some(current) => current.min(max_amount),
        None => max_amount,
    });
}

/// 规则的金额口径由 `operations` 唯一确定；未声明或跨口径时返回 `None`。
fn rule_amount_unit(config: &Value) -> Option<AmountUnit> {
    let operations = string_list_field(config, "operations")?;
    let mut unit: Option<AmountUnit> = None;
    for operation in &operations {
        let current = operation_amount_unit(operation)?;
        if unit.is_some_and(|existing| existing != current) {
            return None;
        }
        unit = Some(current);
    }
    unit
}

fn operation_amount_unit(operation: &str) -> Option<AmountUnit> {
    if operation.eq_ignore_ascii_case(OPERATION_SPOT_ORDER_CREATE) {
        Some(AmountUnit::QuoteNotional)
    } else if operation.eq_ignore_ascii_case(OPERATION_WALLET_WITHDRAWAL_CREATE) {
        Some(AmountUnit::AssetQuantity)
    } else {
        None
    }
}

fn rate_limit_candidate(rule: &StoredRiskRule) -> Option<RateLimitCandidate> {
    Some(RateLimitCandidate {
        max_requests: u32_field(&rule.config, "max_requests")?,
        window_seconds: u32_field(&rule.config, "window_seconds")
            .filter(|window| *window > 0)
            .unwrap_or(DEFAULT_RATE_LIMIT_WINDOW_SECONDS),
        scope: rule.scope_key(),
    })
}

fn merge_blocked_operations(blocked: &mut Vec<String>, config: &Value) {
    let Some(operations) = string_list_field(config, "blocked_operations") else {
        return;
    };
    for operation in operations {
        if !blocked.contains(&operation) {
            blocked.push(operation);
        }
    }
}

fn decimal_field(config: &Value, key: &str) -> Option<BigDecimal> {
    let text = match config.get(key)? {
        Value::String(text) => text.trim().to_owned(),
        Value::Number(number) => number.to_string(),
        _ => return None,
    };
    BigDecimal::from_str(&text)
        .ok()
        .filter(|amount| amount >= &BigDecimal::from(0))
}

fn u32_field(config: &Value, key: &str) -> Option<u32> {
    let value = match config.get(key)? {
        Value::String(text) => text.trim().parse::<u64>().ok()?,
        Value::Number(number) => number.as_u64()?,
        _ => return None,
    };
    u32::try_from(value).ok()
}

fn string_list_field(config: &Value, key: &str) -> Option<Vec<String>> {
    Some(
        config
            .get(key)?
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
    )
}
