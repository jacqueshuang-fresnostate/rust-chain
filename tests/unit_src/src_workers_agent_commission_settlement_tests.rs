use super::{agent_commission_scan_limit, agent_commission_settle_limit, eligible_created_before};
use chrono::{Duration, TimeZone, Utc};

#[test]
fn numeric_safety_commission_age_does_not_panic_or_become_future_time() {
    for seconds in [u64::MAX, i64::MAX as u64, 10_000_000_000_000] {
        assert_eq!(
            eligible_created_before(Utc::now(), seconds),
            chrono::DateTime::<Utc>::MIN_UTC,
        );
    }
}

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
