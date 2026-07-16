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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskDecision {
    Approved,
    Rejected(RiskReject),
}

impl RiskDecision {
    pub fn is_approved(&self) -> bool {
        matches!(self, Self::Approved)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskRules {
    pub max_requests: u32,
    pub max_amount: BigDecimal,
    pub max_price_deviation_bps: u32,
    pub allowed_operations: Vec<String>,
}

impl DomainLayer for RiskRules {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskRequest {
    pub operation: String,
    pub request_count: u32,
    pub amount: BigDecimal,
    pub price: BigDecimal,
    pub reference_price: BigDecimal,
}

impl DomainLayer for RiskRequest {}

pub fn evaluate_risk(request: &RiskRequest, rules: &RiskRules) -> RiskDecision {
    if request.request_count > rules.max_requests {
        return RiskDecision::Rejected(RiskReject::RateLimit);
    }

    if request.amount > rules.max_amount {
        return RiskDecision::Rejected(RiskReject::AmountLimit);
    }

    // 价格偏离按基准价折算为 bps，避免不同币种价格精度影响风控阈值。
    if price_deviation_bps(&request.price, &request.reference_price) > rules.max_price_deviation_bps
    {
        return RiskDecision::Rejected(RiskReject::PriceDeviation);
    }

    if !rules
        .allowed_operations
        .iter()
        .any(|operation| operation == &request.operation)
    {
        return RiskDecision::Rejected(RiskReject::OperationNotAllowed);
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
