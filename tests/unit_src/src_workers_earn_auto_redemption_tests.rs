use super::*;

#[test]
fn earn_auto_redemption_limit_is_clamped() {
    assert_eq!(earn_auto_redemption_limit(0), 1);
    assert_eq!(earn_auto_redemption_limit(50), 50);
    assert_eq!(earn_auto_redemption_limit(500), 100);
}

#[test]
fn earn_auto_redemption_scan_limit_scans_past_bad_rows() {
    assert_eq!(earn_auto_redemption_scan_limit(0), 10);
    assert_eq!(earn_auto_redemption_scan_limit(1), 10);
    assert_eq!(earn_auto_redemption_scan_limit(50), 500);
    assert_eq!(earn_auto_redemption_scan_limit(500), 500);
}
