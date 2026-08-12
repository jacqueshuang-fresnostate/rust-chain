//! risk bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。

use crate::architecture::DomainLayer;
use bigdecimal::BigDecimal;
use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum RiskReject {
    #[error("rate limit exceeded")]
    RateLimit,
    #[error("amount exceeds limit")]
    AmountLimit,
    #[error("price deviation exceeded")]
    PriceDeviation,
    #[error("operation is not allowed")]
    OperationNotAllowed,
}

impl RiskReject {
    /// 返回稳定风控错误码，供 HTTP 和审计层保持拒绝原因一致。
    pub fn code(&self) -> &'static str {
        match self {
            Self::RateLimit => "risk_rate_limit",
            Self::AmountLimit => "risk_amount_limit",
            Self::PriceDeviation => "risk_price_deviation",
            Self::OperationNotAllowed => "risk_operation_not_allowed",
        }
    }

    /// 返回与风控拒绝类型对应的中文用户提示，不泄露内部规则配置。
    pub fn message(&self) -> &'static str {
        match self {
            Self::RateLimit => "操作过于频繁，请稍后再试",
            Self::AmountLimit => "金额超出风控限额",
            Self::PriceDeviation => "价格偏离市场价过大",
            Self::OperationNotAllowed => "该操作已被风控规则限制",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskDecision {
    Approved,
    Rejected(RiskReject),
}

impl RiskDecision {
    /// 判断评估结果是否放行；该只读检查不改变限频计数或审计状态。
    pub fn is_approved(&self) -> bool {
        matches!(self, Self::Approved)
    }
}

/// 后台配置出的风控阈值；`None` 表示该维度没有规则，必须放行。
/// 操作维度用黑名单而非白名单：每条规则只拒绝自己列出的操作，叠加只会拒绝更多具名操作，
/// 不会像白名单取交集那样把两条各自合理的规则合成"全面拒绝"。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RiskRules {
    pub max_requests: Option<u32>,
    pub max_amount: Option<BigDecimal>,
    pub max_price_deviation_bps: Option<u32>,
    pub blocked_operations: Option<Vec<String>>,
}

impl DomainLayer for RiskRules {}

impl RiskRules {
    /// 判断所有阈值和黑名单均为空，用于在应用层跳过计数及审计 I/O。
    pub fn is_unrestricted(&self) -> bool {
        self.max_requests.is_none()
            && self.max_amount.is_none()
            && self.max_price_deviation_bps.is_none()
            && self.blocked_operations.is_none()
    }
}

/// 待评估的业务请求；`None` 表示该路径无法诚实取到对应事实，相应校验必须跳过。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskRequest {
    pub operation: String,
    pub request_count: Option<u32>,
    pub amount: Option<BigDecimal>,
    pub price: Option<BigDecimal>,
    pub reference_price: Option<BigDecimal>,
}

impl DomainLayer for RiskRequest {}

/// 按操作黑名单、限频、金额上限和价格偏离的稳定优先级评估请求。
/// 评估仅消费已取得的事实；缺少某维度时跳过该规则，不执行 I/O 或资金副作用。
pub fn evaluate_risk(request: &RiskRequest, rules: &RiskRules) -> RiskDecision {
    // 被禁用的操作优先拒绝，拒绝原因才不会被限频等次要维度盖掉。
    if let Some(blocked_operations) = rules.blocked_operations.as_ref()
        && blocked_operations
            .iter()
            .any(|operation| operation.eq_ignore_ascii_case(&request.operation))
    {
        return RiskDecision::Rejected(RiskReject::OperationNotAllowed);
    }

    if let (Some(max_requests), Some(request_count)) = (rules.max_requests, request.request_count)
        && request_count > max_requests
    {
        return RiskDecision::Rejected(RiskReject::RateLimit);
    }

    if let (Some(max_amount), Some(amount)) = (rules.max_amount.as_ref(), request.amount.as_ref())
        && amount > max_amount
    {
        return RiskDecision::Rejected(RiskReject::AmountLimit);
    }

    // 价格偏离按基准价折算为 bps，避免不同币种价格精度影响风控阈值。
    if let (Some(max_deviation_bps), Some(price), Some(reference_price)) = (
        rules.max_price_deviation_bps,
        request.price.as_ref(),
        request.reference_price.as_ref(),
    ) && price_deviation_bps(price, reference_price) > max_deviation_bps
    {
        return RiskDecision::Rejected(RiskReject::PriceDeviation);
    }

    RiskDecision::Approved
}

fn price_deviation_bps(price: &BigDecimal, reference_price: &BigDecimal) -> BigDecimal {
    if reference_price == &BigDecimal::from(0) {
        return BigDecimal::from(i64::MAX);
    }

    let deviation = if price >= reference_price {
        price - reference_price
    } else {
        reference_price - price
    };

    deviation * BigDecimal::from(10_000) / reference_price.clone()
}
