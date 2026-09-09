use super::*;

fn record() -> crate::modules::admin::infrastructure::AdminNewCoinReconciliationRecord {
    crate::modules::admin::infrastructure::AdminNewCoinReconciliationRecord {
        project_id: 7,
        symbol: "HIP".to_owned(),
        lifecycle_status: "distribution".to_owned(),
        total_supply: BigDecimal::from(100),
        reserved_supply: BigDecimal::from(10),
        allocated_supply: BigDecimal::from(20),
        remaining_supply: BigDecimal::from(70),
        subscription_count: 2,
        pending_manual_count: 0,
        requested_quantity: BigDecimal::from(30),
        subscription_allocated_quantity: BigDecimal::from(20),
        distribution_quantity: BigDecimal::from(20),
        linked_distribution_quantity: BigDecimal::from(20),
        unlinked_distribution_quantity: BigDecimal::from(0),
        distribution_ledger_quantity: BigDecimal::from(20),
        invalid_subscription_link_count: 0,
        manual_quote_amount: BigDecimal::from(50),
        manual_frozen_quote_amount: BigDecimal::from(0),
        manual_settled_quote_amount: BigDecimal::from(40),
        manual_refunded_quote_amount: BigDecimal::from(10),
    }
}

#[test]
fn balanced_snapshot_has_zero_deltas() {
    let response = build_new_coin_reconciliation_response(record(), Utc::now());
    assert_eq!(response.status, "balanced");
    assert_eq!(response.anomaly_count, 0);
    assert!(response.anomalies.is_empty());
    assert!(is_zero(&response.supply_delta));
    assert!(is_zero(&response.manual_quote_delta));
}

#[test]
fn inconsistent_snapshot_lists_actionable_anomalies() {
    let mut input = record();
    input.distribution_ledger_quantity = BigDecimal::from(19);
    input.pending_manual_count = 1;
    input.lifecycle_status = "listed".to_owned();
    let response = build_new_coin_reconciliation_response(input, Utc::now());
    assert_eq!(response.status, "attention");
    assert!(
        response
            .anomalies
            .iter()
            .any(|value| value.contains("钱包"))
    );
    assert!(
        response
            .anomalies
            .iter()
            .any(|value| value.contains("已上市"))
    );
}
