use super::{LoanOverdueWorkerConfig, loan_overdue_limit, loan_overdue_scan_limit};

#[test]
fn loan_overdue_limits_are_bounded() {
    assert_eq!(loan_overdue_limit(0), 1);
    assert_eq!(loan_overdue_limit(100), 100);
    assert_eq!(loan_overdue_limit(1000), 200);
    assert_eq!(loan_overdue_scan_limit(0), 10);
    assert_eq!(loan_overdue_scan_limit(100), 1000);
    assert_eq!(loan_overdue_scan_limit(1000), 1000);
}

#[test]
fn loan_overdue_and_health_worker_is_enabled_by_default() {
    let config = LoanOverdueWorkerConfig::from_env();

    assert!(config.enabled);
    assert_eq!(config.interval_seconds, 300);
    assert_eq!(config.batch_limit, 100);
}

#[test]
fn overdue_scan_collects_due_orders_with_the_shared_repayment_path() {
    let source = include_str!("../../src/workers/loan_overdue.rs");
    assert!(source.contains("settle_locked_loan_order_repayment_in_tx"));
    assert!(source.contains("insufficient available balance for loan repayment"));
    assert!(source.contains("collected"));
    assert!(!source.contains("不改钱包"));
}
