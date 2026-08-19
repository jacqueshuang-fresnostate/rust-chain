use super::{MarginOrderType, OpenMarginPositionRequest, validate_open_order_semantics};
use bigdecimal::BigDecimal;
use std::str::FromStr;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).expect("valid decimal")
}

fn request(
    order_type: Option<&str>,
    price: Option<&str>,
    trigger_price: Option<&str>,
) -> OpenMarginPositionRequest {
    OpenMarginPositionRequest {
        product_id: 1,
        direction: "long".to_owned(),
        order_type: order_type.map(str::to_owned),
        price: price.map(decimal),
        trigger_price: trigger_price.map(decimal),
        margin_mode: None,
        margin_amount: decimal("20"),
        leverage: decimal("3"),
        idempotency_key: "margin-open-semantics".to_owned(),
    }
}

#[test]
fn missing_order_type_keeps_legacy_market_semantics_without_client_price() {
    let order = validate_open_order_semantics(&request(None, None, None)).unwrap();

    assert_eq!(order.order_type, MarginOrderType::Market);
    assert_eq!(order.limit_price, None);
    assert!(validate_open_order_semantics(&request(Some(" market "), None, None)).is_ok());
}

#[test]
fn market_orders_reject_price_and_every_order_rejects_trigger_price() {
    assert!(validate_open_order_semantics(&request(Some("market"), Some("100"), None)).is_err());
    assert!(validate_open_order_semantics(&request(None, None, Some("99"))).is_err());
    assert!(
        validate_open_order_semantics(&request(Some("limit"), Some("100"), Some("99"))).is_err()
    );
}

#[test]
fn limit_orders_require_a_positive_price_and_preserve_the_exact_decimal() {
    assert!(validate_open_order_semantics(&request(Some("limit"), None, None)).is_err());
    assert!(validate_open_order_semantics(&request(Some("limit"), Some("0"), None)).is_err());
    assert!(validate_open_order_semantics(&request(Some("limit"), Some("-1"), None)).is_err());

    let order =
        validate_open_order_semantics(&request(Some(" LIMIT "), Some("1.2300"), None)).unwrap();
    assert_eq!(order.order_type, MarginOrderType::Limit);
    assert_eq!(order.limit_price, Some(decimal("1.2300")));
}
