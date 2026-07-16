use super::*;
use bigdecimal::BigDecimal;
use std::str::FromStr;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

#[test]
fn spot_fill_request_validation_trims_and_requires_idempotency_key() {
    let request = FillSpotOrdersRequest {
        buy_order_id: "1".to_owned(),
        sell_order_id: "2".to_owned(),
        price: decimal("10"),
        quantity: decimal("1"),
        idempotency_key: " fill-key ".to_owned(),
    };

    assert_eq!(
        validate_fill_spot_order_request(request)
            .unwrap()
            .idempotency_key,
        "fill-key"
    );

    let request = FillSpotOrdersRequest {
        buy_order_id: "1".to_owned(),
        sell_order_id: "2".to_owned(),
        price: decimal("10"),
        quantity: decimal("1"),
        idempotency_key: "   ".to_owned(),
    };
    assert!(matches!(
        validate_fill_spot_order_request(request),
        Err(AppError::Validation(message)) if message == "idempotency_key is required"
    ));
}

#[test]
fn spot_fill_request_rejects_non_positive_price_or_quantity() {
    let request = FillSpotOrdersRequest {
        buy_order_id: "1".to_owned(),
        sell_order_id: "2".to_owned(),
        price: decimal("0"),
        quantity: decimal("1"),
        idempotency_key: "fill-key".to_owned(),
    };
    assert!(matches!(
        validate_fill_spot_order_request(request),
        Err(AppError::Validation(message)) if message == "price must be positive"
    ));

    let request = FillSpotOrdersRequest {
        buy_order_id: "1".to_owned(),
        sell_order_id: "2".to_owned(),
        price: decimal("10"),
        quantity: decimal("0"),
        idempotency_key: "fill-key".to_owned(),
    };
    assert!(matches!(
        validate_fill_spot_order_request(request),
        Err(AppError::Validation(message)) if message == "quantity must be positive"
    ));
}

#[test]
fn spot_admin_cancel_request_trims_and_requires_reason() {
    let request = AdminCancelSpotOrderRequest {
        reason: Some(" manual cancel ".to_owned()),
    };
    assert_eq!(
        validate_admin_cancel_spot_order_request(request).unwrap(),
        "manual cancel"
    );

    let request = AdminCancelSpotOrderRequest {
        reason: Some("   ".to_owned()),
    };
    assert!(matches!(
        validate_admin_cancel_spot_order_request(request),
        Err(AppError::Validation(message)) if message == "reason is required"
    ));
}
