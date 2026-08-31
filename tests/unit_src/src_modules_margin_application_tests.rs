use bigdecimal::BigDecimal;
use std::str::FromStr;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

#[test]
fn margin_capabilities_advertise_isolated_and_cross_risk_modes() {
    let capabilities = super::application::margin_trading_capabilities();

    assert_eq!(capabilities.order_types, vec!["market", "limit"]);
    assert_eq!(capabilities.margin_modes, vec!["isolated", "cross"]);
}

#[test]
fn margin_transfer_fingerprint_normalizes_decimal_scale() {
    let first = super::application::margin_transfer_request_fingerprint(
        9,
        12,
        "spot",
        "margin",
        &decimal("1.000"),
    );
    let equivalent = super::application::margin_transfer_request_fingerprint(
        9,
        12,
        "spot",
        "margin",
        &decimal("1"),
    );
    let changed = super::application::margin_transfer_request_fingerprint(
        9,
        12,
        "margin",
        "spot",
        &decimal("1"),
    );

    assert_eq!(first, equivalent);
    assert_ne!(first, changed);
    assert_eq!(first.len(), 64);
}
