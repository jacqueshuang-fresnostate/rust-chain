use super::*;
use std::str::FromStr;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

#[test]
fn withdrawal_policy_defaults_never_enable_a_monetary_rule() {
    let policy = WithdrawalPolicy::default();
    assert!(!policy.enabled);
    assert!(policy.allowances.is_empty());
    assert!(policy.review_tiers.is_empty());
    assert!(policy.address_cooling_seconds.is_none());
    assert!(policy.security_cooling_seconds.is_none());
    assert_eq!(policy.required_approvals(&decimal("1000000")), 1);
}

#[test]
fn withdrawal_policy_requires_explicit_precision_windows_and_review_boundaries() {
    let mut policy = WithdrawalPolicy {
        enabled: true,
        review_tiers: vec![
            WithdrawalReviewTier {
                min_amount: decimal("0"),
                required_approvals: 1,
            },
            WithdrawalReviewTier {
                min_amount: decimal("2"),
                required_approvals: 2,
            },
        ],
        ..WithdrawalPolicy::default()
    };
    assert!(policy.validate(8).is_ok());
    assert_eq!(policy.required_approvals(&decimal("1.99999999")), 1);
    assert_eq!(policy.required_approvals(&decimal("2")), 2);
    policy.review_tiers[1].required_approvals = 0;
    assert!(policy.validate(8).is_err());
    policy.review_tiers.clear();
    policy.allowances.push(WithdrawalAllowance {
        user_id: None,
        kyc_level: None,
        window: WithdrawalWindow::Rolling,
        window_seconds: 10,
        pending_mode: WithdrawalPendingMode::AllOutstanding,
        max_amount: decimal("0.000000001"),
    });
    assert!(policy.validate(8).is_err());
    policy.allowances[0].max_amount = decimal("0");
    assert!(
        policy.validate(8).is_ok(),
        "zero is an explicit deny-all cap"
    );
    policy.allowances[0].window_seconds = 0;
    assert!(policy.validate(8).is_err());
}

#[test]
fn withdrawal_policy_amount_basis_and_unknown_fields_are_explicit() {
    let mut policy = WithdrawalPolicy::default();
    let amount = decimal("2");
    let reserved = decimal("2.1");
    assert_eq!(policy.measured_amount(&amount, &reserved), &amount);
    policy.amount_basis = WithdrawalAmountBasis::TotalReserved;
    assert_eq!(policy.measured_amount(&amount, &reserved), &reserved);
    let mut value = serde_json::to_value(policy).unwrap();
    value["daily_cap"] = serde_json::json!("1");
    assert!(serde_json::from_value::<WithdrawalPolicy>(value).is_err());
}
