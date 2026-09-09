use super::*;

#[test]
fn recovery_interval_is_clamped_away_from_busy_or_silent_loops() {
    assert_eq!(recovery_interval_seconds(0), 1);
    assert_eq!(recovery_interval_seconds(30), 30);
    assert_eq!(recovery_interval_seconds(3_601), 3_600);
}
