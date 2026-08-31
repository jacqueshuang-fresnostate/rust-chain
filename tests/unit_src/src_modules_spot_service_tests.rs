use super::*;
use crate::modules::spot::presentation::CreateSpotOrderRequest;
use std::str::FromStr;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

fn request(pair_id: &str, price: &str, quantity: &str) -> CreateSpotOrderRequest {
    CreateSpotOrderRequest {
        pair_id: pair_id.to_owned(),
        side: OrderSide::Buy,
        order_type: OrderType::Limit,
        price: Some(decimal(price)),
        trigger_price: None,
        quantity: decimal(quantity),
        reference_price: None,
        idempotency_key: "client-key".to_owned(),
    }
}

#[test]
fn spot_order_fingerprint_normalizes_pair_and_decimal_text() {
    let first = spot_order_request_fingerprint(7, &request(" btc-usdt ", "10.0", "2.000"));
    let equivalent = spot_order_request_fingerprint(7, &request("BTC-USDT", "10", "2"));
    let changed = spot_order_request_fingerprint(7, &request("BTC-USDT", "10", "3"));

    assert_eq!(first, equivalent);
    assert_ne!(first, changed);
    assert_eq!(first.len(), 64);
}

#[test]
fn spot_order_idempotency_key_is_required_and_bounded() {
    assert!(normalize_idempotency_key("   ").is_err());
    assert!(normalize_idempotency_key(&"x".repeat(129)).is_err());
    assert_eq!(normalize_idempotency_key(" key ").unwrap(), "key");
}
