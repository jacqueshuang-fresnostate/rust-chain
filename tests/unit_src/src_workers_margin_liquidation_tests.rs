use super::*;
use std::str::FromStr;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

#[test]
fn margin_liquidation_limit_is_clamped() {
    assert_eq!(margin_liquidation_limit(0), 1);
    assert_eq!(margin_liquidation_limit(50), 50);
    assert_eq!(margin_liquidation_limit(500), 100);
}

#[test]
fn margin_liquidation_scan_limit_scans_past_broken_rows() {
    assert_eq!(margin_liquidation_scan_limit(0), 10);
    assert_eq!(margin_liquidation_scan_limit(1), 10);
    assert_eq!(margin_liquidation_scan_limit(50), 500);
    assert_eq!(margin_liquidation_scan_limit(500), 500);
}

#[test]
fn margin_liquidation_risk_state_rejects_invalid_direction() {
    let error = margin_liquidation_risk_state(
        "sideways",
        &decimal("20"),
        &decimal("100"),
        &decimal("0"),
        &decimal("100"),
        &decimal("90"),
        &decimal("0.05"),
    )
    .unwrap_err();
    assert!(error.to_string().contains("long or short"));
}
