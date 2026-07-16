use super::{AgentCommissionRateTier, allocate_differential_agent_commissions};
use bigdecimal::BigDecimal;
use std::str::FromStr;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).expect("valid decimal")
}

#[test]
fn differential_commission_allocates_owner_child_and_root_rates() {
    let allocations = allocate_differential_agent_commissions(
        &[
            AgentCommissionRateTier {
                agent_id: 3,
                cumulative_rate: decimal("0.05"),
            },
            AgentCommissionRateTier {
                agent_id: 2,
                cumulative_rate: decimal("0.08"),
            },
            AgentCommissionRateTier {
                agent_id: 1,
                cumulative_rate: decimal("0.10"),
            },
        ],
        &decimal("100"),
        8,
    );

    assert_eq!(allocations.len(), 3);
    assert_eq!(allocations[0].agent_id, 3);
    assert_eq!(allocations[0].commission_rate, decimal("0.05"));
    assert_eq!(allocations[0].commission_amount, decimal("5.00000000"));
    assert_eq!(allocations[1].agent_id, 2);
    assert_eq!(allocations[1].commission_rate, decimal("0.03"));
    assert_eq!(allocations[1].commission_amount, decimal("3.00000000"));
    assert_eq!(allocations[2].agent_id, 1);
    assert_eq!(allocations[2].commission_rate, decimal("0.02"));
    assert_eq!(allocations[2].commission_amount, decimal("2.00000000"));
}

#[test]
fn differential_commission_skips_missing_or_inverted_tiers_without_overpaying() {
    let allocations = allocate_differential_agent_commissions(
        &[
            AgentCommissionRateTier {
                agent_id: 3,
                cumulative_rate: decimal("0.06"),
            },
            AgentCommissionRateTier {
                agent_id: 2,
                cumulative_rate: decimal("0.04"),
            },
            AgentCommissionRateTier {
                agent_id: 1,
                cumulative_rate: decimal("0.10"),
            },
        ],
        &decimal("10.123456789"),
        4,
    );

    assert_eq!(allocations.len(), 2);
    assert_eq!(allocations[0].agent_id, 3);
    assert_eq!(allocations[0].commission_amount, decimal("0.6074"));
    assert_eq!(allocations[1].agent_id, 1);
    assert_eq!(allocations[1].commission_rate, decimal("0.04"));
    assert_eq!(allocations[1].commission_amount, decimal("0.4049"));
    assert_eq!(
        allocations
            .iter()
            .map(|allocation| allocation.commission_amount.clone())
            .sum::<BigDecimal>(),
        decimal("1.0123")
    );
}

#[test]
fn differential_commission_ignores_zero_source_and_invalid_rates() {
    let tiers = [
        AgentCommissionRateTier {
            agent_id: 3,
            cumulative_rate: decimal("0"),
        },
        AgentCommissionRateTier {
            agent_id: 2,
            cumulative_rate: decimal("1.1"),
        },
    ];

    assert!(allocate_differential_agent_commissions(&tiers, &decimal("100"), 8).is_empty());
    assert!(allocate_differential_agent_commissions(&tiers, &decimal("0"), 8).is_empty());
}
