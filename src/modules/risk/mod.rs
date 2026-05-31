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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskRequest {
    pub operation: String,
    pub request_count: u32,
    pub amount: BigDecimal,
    pub price: BigDecimal,
    pub reference_price: BigDecimal,
}

pub fn evaluate_risk(request: &RiskRequest, rules: &RiskRules) -> RiskDecision {
    if request.request_count > rules.max_requests {
        return RiskDecision::Rejected(RiskReject::RateLimit);
    }

    if request.amount > rules.max_amount {
        return RiskDecision::Rejected(RiskReject::AmountLimit);
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use bigdecimal::BigDecimal;

    fn amount(value: i64) -> BigDecimal {
        BigDecimal::from(value)
    }

    fn request(operation: &str, request_amount: i64, price: i64) -> RiskRequest {
        RiskRequest {
            operation: operation.to_owned(),
            request_count: 1,
            amount: amount(request_amount),
            price: amount(price),
            reference_price: amount(100),
        }
    }

    #[test]
    fn risk_guard_approves_request_within_limits() {
        let rules = RiskRules {
            max_requests: 3,
            max_amount: amount(1_000),
            max_price_deviation_bps: 500,
            allowed_operations: vec!["spot.order.create".to_owned()],
        };

        let decision = evaluate_risk(&request("spot.order.create", 100, 104), &rules);

        assert_eq!(decision, RiskDecision::Approved);
        assert!(decision.is_approved());
    }

    #[test]
    fn risk_guard_rejects_rate_amount_price_and_disallowed_operation() {
        let rules = RiskRules {
            max_requests: 3,
            max_amount: amount(1_000),
            max_price_deviation_bps: 500,
            allowed_operations: vec!["spot.order.create".to_owned()],
        };
        let mut rate_limited = request("spot.order.create", 100, 100);
        rate_limited.request_count = 4;

        assert_eq!(
            evaluate_risk(&rate_limited, &rules),
            RiskDecision::Rejected(RiskReject::RateLimit)
        );
        assert_eq!(
            evaluate_risk(&request("spot.order.create", 1_001, 100), &rules),
            RiskDecision::Rejected(RiskReject::AmountLimit)
        );
        assert_eq!(
            evaluate_risk(&request("spot.order.create", 100, 106), &rules),
            RiskDecision::Rejected(RiskReject::PriceDeviation)
        );
        assert_eq!(
            evaluate_risk(&request("market_strategy.stop", 100, 100), &rules),
            RiskDecision::Rejected(RiskReject::OperationNotAllowed)
        );
    }
}
