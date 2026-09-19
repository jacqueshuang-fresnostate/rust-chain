use super::{
    cancellation::validate_admin_cancel_spot_order_request,
    settlement::validate_fill_spot_order_request,
};
use crate::{
    error::AppError,
    modules::spot::presentation::{AdminCancelSpotOrderRequest, FillSpotOrdersRequest},
};
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
        reason: " verified orders ".to_owned(),
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
        reason: "verified orders".to_owned(),
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
        reason: "verified orders".to_owned(),
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
        reason: "verified orders".to_owned(),
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

#[test]
fn manual_spot_fill_requires_bounded_reason_and_key() {
    let body = serde_json::json!({
        "buy_order_id": "1", "sell_order_id": "2", "price": "10", "quantity": "1",
        "idempotency_key": "fill-key"
    });
    assert!(serde_json::from_value::<FillSpotOrdersRequest>(body.clone()).is_err());
    for reason in ["", " \n ", &"x".repeat(513)] {
        let mut body = body.clone();
        body["reason"] = reason.into();
        assert!(validate_fill_spot_order_request(serde_json::from_value(body).unwrap()).is_err());
    }
    let mut body = body;
    body["reason"] = " checked orders ".into();
    let valid =
        validate_fill_spot_order_request(serde_json::from_value(body.clone()).unwrap()).unwrap();
    assert_eq!(valid.reason, "checked orders");
    body["idempotency_key"] = "k".repeat(256).into();
    assert!(validate_fill_spot_order_request(serde_json::from_value(body).unwrap()).is_err());
}

#[tokio::test]
async fn manual_spot_fill_rejects_missing_actor_before_database_access() {
    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .connect_lazy("mysql://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    let request = serde_json::from_value(serde_json::json!({
        "buy_order_id": "1", "sell_order_id": "2", "price": "10", "quantity": "1",
        "idempotency_key": "fill-key", "reason": "checked orders"
    }))
    .unwrap();
    assert!(matches!(
        super::settlement::fill_spot_orders_with_events_with_request(&pool, 0, request, None).await,
        Err(AppError::Unauthorized)
    ));
}
