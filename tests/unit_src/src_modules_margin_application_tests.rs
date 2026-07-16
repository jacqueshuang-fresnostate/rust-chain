#[test]
fn margin_capabilities_only_advertise_implemented_order_and_risk_modes() {
    let capabilities = super::application::margin_trading_capabilities();

    assert_eq!(capabilities.order_types, vec!["market"]);
    assert_eq!(capabilities.margin_modes, vec!["isolated"]);
}
