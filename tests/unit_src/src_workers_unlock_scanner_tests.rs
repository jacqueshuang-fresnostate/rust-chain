use super::*;
use chrono::{TimeZone, Utc};

fn position(
    id: &str,
    unlock_at: chrono::DateTime<Utc>,
    status: LockPositionStatus,
) -> UnlockScanPosition {
    UnlockScanPosition {
        id: id.to_owned(),
        unlock_at,
        status,
    }
}

#[test]
fn due_unlock_positions_include_active_positions_at_or_before_now() {
    let now = Utc.with_ymd_and_hms(2026, 5, 26, 10, 0, 0).unwrap();
    let positions = vec![
        position(
            "past-active",
            now - chrono::TimeDelta::seconds(1),
            LockPositionStatus::Active,
        ),
        position("now-active", now, LockPositionStatus::Active),
        position(
            "future-active",
            now + chrono::TimeDelta::seconds(1),
            LockPositionStatus::Active,
        ),
        position(
            "past-released",
            now - chrono::TimeDelta::seconds(1),
            LockPositionStatus::Released,
        ),
    ];

    let due = due_unlock_positions(&positions, now);

    assert_eq!(
        due.iter()
            .map(|position| position.id.as_str())
            .collect::<Vec<_>>(),
        vec!["past-active", "now-active"]
    );
}

#[test]
fn unlock_scan_limit_is_clamped() {
    assert_eq!(unlock_scan_limit(0), 1);
    assert_eq!(unlock_scan_limit(50), 50);
    assert_eq!(unlock_scan_limit(500), 100);
}
