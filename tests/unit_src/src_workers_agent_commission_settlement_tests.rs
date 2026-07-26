use super::{
    AgentCommissionSettlementGuard, agent_commission_scan_limit, agent_commission_settle_limit,
    eligible_created_before,
};
use chrono::{Duration, TimeZone, Utc};

#[test]
fn agent_commission_settlement_limits_are_bounded() {
    assert_eq!(agent_commission_settle_limit(0), 1);
    assert_eq!(agent_commission_settle_limit(100), 100);
    assert_eq!(agent_commission_settle_limit(1000), 200);
    assert_eq!(agent_commission_scan_limit(0), 10);
    assert_eq!(agent_commission_scan_limit(100), 1000);
    assert_eq!(agent_commission_scan_limit(1000), 1000);
}

#[test]
fn agent_commission_eligibility_cutoff_subtracts_min_age() {
    let now = Utc.with_ymd_and_hms(2026, 7, 26, 12, 0, 0).unwrap();
    assert_eq!(eligible_created_before(now, 0), now);
    assert_eq!(
        eligible_created_before(now, 3600),
        now - Duration::seconds(3600)
    );
}

#[test]
fn agent_commission_settlement_guard_blocks_repeated_failures() {
    let mut guard = AgentCommissionSettlementGuard::default();
    assert!(guard.should_attempt(7));
    guard.record_failure(7);
    assert!(!guard.should_attempt(7));
    assert!(guard.should_attempt(8));
}

#[test]
fn agent_commission_settlement_guard_resets_after_capacity() {
    let mut guard = AgentCommissionSettlementGuard::default();
    for id in 0..10_000 {
        guard.record_failure(id);
    }
    assert!(!guard.should_attempt(0));
    guard.record_failure(10_000);
    assert!(guard.should_attempt(0));
    assert!(!guard.should_attempt(10_000));
}
