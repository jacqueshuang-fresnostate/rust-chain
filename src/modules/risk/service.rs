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
    /// 取出规则的作用域目标标识，去掉首尾空白后把空串一并视为未设置。
    /// 归一化是必要的：后台把目标留空和填入空白字符串都表示这是一条全局规则，
    /// 若不折叠会让空白目标既匹配不上任何具体作用域，也享受不到全局生效的语义，从而静默失效。
    fn target_id(&self) -> Option<&str> {
        self.target_id
            .as_deref()
            .map(str::trim)
            .filter(|target_id| !target_id.is_empty())
    }

    /// 规则作用域标识；同时作为限频计数键的一段，避免不同作用域的规则共用计数器。
    /// 有具体目标时拼成类型与目标的小写组合，无目标的全局规则统一落到 global 常量。
    /// 大小写归一保证后台录入的 USER 与 user 命中同一个计数桶，否则同一条规则会因录入习惯分裂出两份配额。
    /// 该字符串还参与限频候选的最终定序，因此必须是稳定的纯函数结果。
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
    /// 构造风控匹配维度和值，例如 user、asset 或 operation；值原样保留供策略比较。
    /// 本值对象不加载规则或执行限额判断，维度名称的合法性由调用方的策略合同保证。
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
    /// 构造完全放行的基线策略：所有阈值为空表示不限制，窗口取默认六十秒，计数作用域落在全局桶。
    /// 没有任何规则命中时直接返回该默认值，因此风控缺省是放行而非拒绝，规则表为空不会阻断线上交易。
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

/// 合并同时命中当前操作和作用域的规则；数值阈值取最严，黑名单取并集。
/// 结果与存储行顺序无关且不执行 I/O；无匹配时返回全放行策略。
/// 金额上限还额外要求规则声明的计量口径与当前操作一致，口径不明的规则在金额维度上直接不生效。
/// 限频只保留最严的一条候选并连同其窗口与作用域整体采用，不会把不同规则的请求数与窗口交叉组合。
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

/// 判定单条规则是否参与本次合并，要求操作维度与作用域维度同时命中，二者是与关系。
/// 两个维度各自缺省即视为通配，因此既未限定操作也未限定目标的规则对每个请求都生效。
/// 这里只做筛选不读取任何阈值，被筛掉的规则完全不影响最终策略。
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

/// 判定规则的作用域目标是否落在本次请求携带的维度集合内，目标为空的全局规则直接放行。
/// 匹配要求维度名与目标值都相等，且两侧均忽略大小写、规则侧的维度名先去空白，
/// 以兼容后台录入时的大小写与空格差异；只要有任一请求维度命中即算覆盖。
/// 请求未携带对应维度时该规则不生效，因此调用方必须把用户、资产等身份完整传入，否则会漏掉本应命中的限制。
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

/// 推断规则金额上限所使用的计量口径，取值由 `operations` 列出的操作反推。
/// 未声明操作列表、列表中出现口径未知的操作、或多个操作分属不同口径时一律返回空，
/// 使该规则的金额限额被判定为口径不明而不生效，避免把提币的资产数量上限误当作下单名义额上限执行。
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

/// 给出某个操作的金额计量口径：现货下单按计价币种名义额，提现按提币资产数量。
/// 名称比较忽略大小写；未在此登记的操作返回空，表示金额限额无法安全套用到它上面。
/// 新增需要限额的业务操作时必须同步在这里登记口径，否则针对它配置的金额上限会静默失效。
fn operation_amount_unit(operation: &str) -> Option<AmountUnit> {
    if operation.eq_ignore_ascii_case(OPERATION_SPOT_ORDER_CREATE) {
        Some(AmountUnit::QuoteNotional)
    } else if operation.eq_ignore_ascii_case(OPERATION_WALLET_WITHDRAWAL_CREATE) {
        Some(AmountUnit::AssetQuantity)
    } else {
        None
    }
}

/// 把一条规则折算成限频候选，请求数上限是必需项，缺失该键说明这条规则不参与限频。
/// 窗口秒数可选，未配置或配置为零都回落到默认六十秒，零值必须拦下否则会得到除零或永不重置的计数窗口。
/// 作用域取规则自身的 scope 键，使不同目标的限频各自独立计数而不互相消耗配额。
fn rate_limit_candidate(rule: &StoredRiskRule) -> Option<RateLimitCandidate> {
    Some(RateLimitCandidate {
        max_requests: u32_field(&rule.config, "max_requests")?,
        window_seconds: u32_field(&rule.config, "window_seconds")
            .filter(|window| *window > 0)
            .unwrap_or(DEFAULT_RATE_LIMIT_WINDOW_SECONDS),
        scope: rule.scope_key(),
    })
}

/// 把当前规则声明的禁止操作并入累积黑名单，多条规则之间取并集而非覆盖，越叠加越严格。
/// 追加前按完整字符串去重，因此同一操作被多条规则重复列出也只保留一份。
/// 这里的比较区分大小写，与操作匹配处的忽略大小写口径不同，后台录入需保持与业务侧一致的操作名写法。
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

/// 从规则配置中读取一个非负十进制数值，同时接受 JSON 字符串与数字两种写法。
/// 字符串会先去除首尾空白，数字则经文本中转再解析，以保留后台录入的完整精度而不退化为浮点。
/// 其他 JSON 类型、解析失败以及负数都返回空，让该维度按未配置处理而不是当成零上限把请求全部拦死。
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

/// 从规则配置中读取一个无符号整数，供请求数上限、窗口秒数和价格偏离基点这类计数型阈值使用。
/// 字符串写法去空白后按 u64 解析，数字写法必须是非负整数，浮点或负数会在此被拒。
/// 最后再收窄到 u32，超出范围返回空，使该维度按未配置处理而非截断成一个意义不明的小阈值。
fn u32_field(config: &Value, key: &str) -> Option<u32> {
    let value = match config.get(key)? {
        Value::String(text) => text.trim().parse::<u64>().ok()?,
        Value::Number(number) => number.as_u64()?,
        _ => return None,
    };
    u32::try_from(value).ok()
}

/// 读取规则配置中的字符串数组，用于操作生效范围与禁止操作两个列表。
/// 键缺失或值不是数组时返回空，表示该维度未声明；数组内的非字符串元素被逐个丢弃而不使整条规则失败。
/// 需要注意空数组会返回空列表而非空值，对操作范围而言意味着不匹配任何操作，等价于把这条规则关停。
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
