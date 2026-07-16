use crate::modules::risk::{RiskDecision, RiskReject, RiskRequest, RiskRules, evaluate_risk};
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
