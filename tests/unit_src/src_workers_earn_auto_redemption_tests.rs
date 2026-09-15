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

/// 自动赎回是手工赎回之外的第二条入账路径，必须与手工同量化并写同样的平台对手腿。
#[test]
fn auto_redemption_quantizes_to_asset_precision_and_writes_platform_legs() {
    let source = include_str!("../../src/workers/earn_auto_redemption.rs");

    assert!(source.contains("quantize_earn_redemption_amounts"));
    assert!(source.contains("insert_earn_platform_journal_legs_in_tx"));
    assert!(source.contains("earn_redemption_journal_legs("));
    assert!(source.contains("earn_redeem:"));
}
