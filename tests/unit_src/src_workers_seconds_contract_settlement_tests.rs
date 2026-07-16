use super::*;
use std::str::FromStr;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

#[test]
fn settlement_result_treats_equal_price_as_loss() {
    assert_eq!(
        seconds_contract_settlement_result("up", &decimal("1"), &decimal("1")).unwrap(),
        "loss"
    );
    assert_eq!(
        seconds_contract_settlement_result("down", &decimal("1"), &decimal("1")).unwrap(),
        "loss"
    );
}

#[test]
fn seconds_contract_settlement_limit_is_clamped() {
    assert_eq!(seconds_contract_settlement_limit(0), 1);
    assert_eq!(seconds_contract_settlement_limit(50), 50);
    assert_eq!(seconds_contract_settlement_limit(500), 100);
}

#[test]
fn seconds_contract_settlement_scan_limit_matches_settlement_limit() {
    assert_eq!(seconds_contract_settlement_scan_limit(0), 1);
    assert_eq!(seconds_contract_settlement_scan_limit(1), 1);
    assert_eq!(seconds_contract_settlement_scan_limit(50), 50);
    assert_eq!(seconds_contract_settlement_scan_limit(100), 100);
    assert_eq!(seconds_contract_settlement_scan_limit(500), 100);
}
