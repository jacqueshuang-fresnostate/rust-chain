#[test]
fn margin_capabilities_advertise_isolated_and_cross_risk_modes() {
    let capabilities = super::application::margin_trading_capabilities();

    assert_eq!(capabilities.order_types, vec!["market", "limit"]);
    assert_eq!(capabilities.margin_modes, vec!["isolated", "cross"]);
}
